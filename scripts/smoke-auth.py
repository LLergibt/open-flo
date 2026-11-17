#!/usr/bin/env python3
"""Black-box auth regression checks. Uses a temporary DB and synthetic accounts only."""
import base64
import concurrent.futures
import hashlib
import hmac
import http.client
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
ORIGIN = 'http://127.0.0.1:5173'


def main():
    binary = Path(os.environ.get('FLO_API_BIN', ROOT / '.tooling/target/debug/flo-api'))
    if not binary.is_file():
        raise SystemExit('Build API first, or set FLO_API_BIN to the flo-api executable.')
    password = secrets.token_urlsafe(24)
    jwt_secret = secrets.token_urlsafe(48)
    with tempfile.TemporaryDirectory(prefix='flo-auth-smoke-') as directory:
        env = dict(os.environ, DATABASE_URL=f'sqlite://{directory}/test.db',
                   JWT_SECRET=jwt_secret, COOKIE_SECURE='false', TRUSTED_ORIGINS=ORIGIN)
        bootstrap = subprocess.run([str(binary), 'create-admin', '--username', 'qa_admin',
                                    '--display-name', 'QA Administrator', '--password-stdin'],
                                   input=password + '\n', text=True, env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        assert bootstrap.returncode == 0, 'Local bootstrap failed (no credentials printed).'
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        env['BIND_ADDR'] = f'127.0.0.1:{port}'
        with open(Path(directory) / 'server.log', 'w+') as log:
            server = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
            try:
                def request(method, path, body=None, token=None, headers=None):
                    payload = None if body is None else json.dumps(body).encode()
                    request_headers = {'Accept': 'application/json'}
                    if payload is not None:
                        request_headers['Content-Type'] = 'application/json'
                    if token:
                        request_headers['Authorization'] = 'Bearer ' + token
                    request_headers.update(headers or {})
                    conn = http.client.HTTPConnection('127.0.0.1', port, timeout=12)
                    try:
                        conn.request(method, path, payload, request_headers)
                        result = conn.getresponse()
                        raw = result.read()
                        return result.status, (json.loads(raw) if raw else None), result.getheaders()
                    finally:
                        conn.close()

                for attempt in range(100):
                    try:
                        if request('GET', '/health')[0] == 200:
                            break
                    except (OSError, http.client.HTTPException):
                        pass
                    assert server.poll() is None, 'API exited during startup.'
                    time.sleep(.1)
                else:
                    raise AssertionError('API startup timed out.')

                def expect(status, method, path, body=None, token=None, headers=None):
                    actual, value, response_headers = request(method, path, body, token, headers)
                    assert actual == status, f'{method} {path}: expected {status}, got {actual}; error={value.get("error") if isinstance(value,dict) else None}'
                    return value, response_headers

                def login(username, secret=password):
                    value, _ = expect(200, 'POST', '/api/auth/login',
                                      {'username': username, 'password': secret, 'client': 'native'})
                    assert value['access_token'] and value['refresh_token']
                    assert 'password_hash' not in json.dumps(value)
                    return value

                expect(401, 'GET', '/api/me')
                unknown, _ = expect(401, 'POST', '/api/auth/login',
                                    {'username': 'missing_user', 'password': password, 'client': 'native'})
                wrong, _ = expect(401, 'POST', '/api/auth/login',
                                  {'username': 'qa_admin', 'password': 'Incorrect-test-password!', 'client': 'native'})
                assert unknown == wrong, 'Unknown user and wrong password reveal different errors.'
                admin = login('qa_admin')
                access = admin['access_token']
                expect(401, 'GET', '/api/me', token=access[:-5] + 'xxxxx')
                head, payload, signature = access.split('.')
                claims = json.loads(base64.urlsafe_b64decode(payload + '=' * (-len(payload) % 4)))
                claims['exp'] = int(time.time()) - 3600
                encode = lambda value: base64.urlsafe_b64encode(value).rstrip(b'=')
                expired_payload = encode(json.dumps(claims, separators=(',', ':')).encode()).decode()
                signed = f'{head}.{expired_payload}'
                expired = signed + '.' + encode(hmac.new(jwt_secret.encode(), signed.encode(), hashlib.sha256).digest()).decode()
                expect(401, 'GET', '/api/me', token=expired)
                print('PASS JWT signature/expiry and uniform invalid credentials', flush=True)

                users = {}
                for username, role in [('qa_tracker', 'tracker'), ('qa_partner', 'partner'), ('qa_other', 'tracker')]:
                    value, _ = expect(201, 'POST', '/api/admin/users',
                                      {'username': username, 'display_name': username, 'password': password, 'role': role}, access)
                    assert 'password_hash' not in json.dumps(value)
                    users[username] = value
                tracker = login('qa_tracker')
                partner = login('qa_partner')
                other = login('qa_other')
                expect(403, 'GET', '/api/admin/users', token=tracker['access_token'])
                expect(403, 'POST', '/api/admin/users',
                       {'username': 'forbidden_user', 'display_name': 'Forbidden', 'password': password, 'role': 'admin'}, tracker['access_token'])
                data = {'tracker_id': users['qa_tracker']['id'], 'partner_id': users['qa_partner']['id']}
                with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
                    results = list(pool.map(lambda _: request('POST', '/api/admin/pairs', data, access), range(2)))
                assert sorted(result[0] for result in results) == [201, 409], 'Concurrent duplicate pairing was not atomic.'
                tracker_pair, _ = expect(200, 'GET', '/api/me/pair', token=tracker['access_token'])
                assert tracker_pair['pair']['tracker']['id'] == users['qa_tracker']['id']
                other_pair, _ = expect(200, 'GET', '/api/me/pair', token=other['access_token'])
                assert other_pair['pair'] is None
                expect(204, 'DELETE', '/api/me/pair', token=partner['access_token'])
                for account in [tracker, partner]:
                    value, _ = expect(200, 'GET', '/api/me/pair', token=account['access_token'])
                    assert value['pair'] is None
                print('PASS admin roles, concurrent pairing, isolation and unlink', flush=True)

                rotated, _ = expect(200, 'POST', '/api/auth/refresh',
                                    {'refresh_token': tracker['refresh_token'], 'client': 'native'})
                assert rotated['refresh_token'] != tracker['refresh_token']
                expect(401, 'POST', '/api/auth/refresh',
                       {'refresh_token': tracker['refresh_token'], 'client': 'native'})
                expect(401, 'GET', '/api/me', token=rotated['access_token'])
                partner = login('qa_partner')
                expect(204, 'POST', '/api/auth/logout', {'refresh_token': partner['refresh_token'], 'client': 'native'}, partner['access_token'])
                expect(401, 'GET', '/api/me', token=partner['access_token'])
                partner = login('qa_partner')
                # Native logout must work even when the access JWT has expired:
                # send the valid refresh proof without an invalid Bearer header.
                expect(204, 'POST', '/api/auth/logout', {'refresh_token': partner['refresh_token'], 'client': 'native'})
                expect(401, 'GET', '/api/me', token=partner['access_token'])
                other = login('qa_other')
                expect(200, 'PATCH', '/api/admin/users/' + users['qa_other']['id'], {'is_active': False}, access)
                expect(401, 'GET', '/api/me', token=other['access_token'])
                expect(409, 'PATCH', '/api/admin/users/' + admin['user']['id'], {'is_active': False}, access)
                print('PASS refresh rotation/replay, logout, disable and admin protection', flush=True)

                expect(403, 'POST', '/api/auth/login',
                       {'username': 'qa_admin', 'password': password, 'client': 'web'},
                       headers={'Origin': 'https://untrusted.invalid'})
                web, headers = expect(200, 'POST', '/api/auth/login',
                                      {'username': 'qa_admin', 'password': password, 'client': 'web'}, headers={'Origin': ORIGIN})
                cookies = [value for key, value in headers if key.lower() == 'set-cookie']
                assert len(cookies) >= 2 and all('HttpOnly' in cookie and 'SameSite=Strict' in cookie for cookie in cookies)
                cookie_header = '; '.join(cookie.split(';', 1)[0] for cookie in cookies)
                expect(200, 'GET', '/api/me', headers={'Cookie': cookie_header})
                expect(401, 'GET', '/api/me', headers={'Cookie': cookie_header, 'Authorization': 'Bearer invalid'})
                expect(403, 'POST', '/api/auth/logout', {}, headers={'Cookie': cookie_header})
                expect(403, 'POST', '/api/auth/refresh', {'client': 'native'}, headers={'Cookie': cookie_header, 'Origin': 'https://untrusted.invalid'})
                expect(204, 'POST', '/api/auth/logout', {}, headers={'Cookie': cookie_header, 'Origin': ORIGIN})
                expect(401, 'GET', '/api/me', headers={'Cookie': cookie_header})
                print('PASS HttpOnly cookies, Origin/CSRF checks and bearer precedence', flush=True)
            finally:
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
    print('Auth HTTP smoke passed; temporary credentials/database removed.', flush=True)


if __name__ == '__main__':
    main()
