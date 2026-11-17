import assert from 'node:assert/strict';
import { afterEach, test } from 'node:test';
import { api, ApiError, onSessionExpired } from '../src/lib/api.ts';

const originalFetch = globalThis.fetch;
const originalWindow = globalThis.window;
globalThis.window = { setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout };
const user = { id: 'test-user', username: 'admin', display_name: 'Тестовый администратор', role: 'admin', is_active: true };
const json = (value, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
afterEach(() => { globalThis.fetch = originalFetch; });

test('web login normalizes username, uses same-origin cookies and returns only User', async () => {
  let submitted;
  globalThis.fetch = async (path, options) => {
    assert.equal(path, '/api/auth/login');
    assert.equal(options.credentials, 'same-origin');
    assert.equal(options.cache, 'no-store');
    submitted = JSON.parse(options.body);
    return json({ user, access_token: 'synthetic-access-ignored', expires_in: 900 });
  };
  const result = await api.login(' ADMIN ', 'synthetic-test-password');
  assert.equal(submitted.username, 'admin');
  assert.equal(submitted.client, 'web');
  assert.deepEqual(result, user);
  assert.equal(Object.hasOwn(result, 'access_token'), false);
});

test('simultaneous unauthorized requests share one refresh and retry once', async () => {
  let refreshCount = 0;
  let refreshed = false;
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  globalThis.fetch = async (path) => {
    if (path === '/api/auth/refresh') {
      refreshCount += 1;
      await gate;
      refreshed = true;
      return json({ user, expires_in: 900 });
    }
    if (!refreshed) return json({ error: 'unauthorized' }, 401);
    if (path === '/api/admin/users') return json({ users: [user] });
    if (path === '/api/admin/pairs') return json({ pairs: [] });
    assert.fail(`Unexpected path ${path}`);
  };
  const pending = Promise.all([api.users(), api.pairs()]);
  await new Promise((resolve) => setTimeout(resolve, 0));
  release();
  const [users, pairs] = await pending;
  assert.equal(refreshCount, 1);
  assert.deepEqual(users, [user]);
  assert.deepEqual(pairs, []);
});

test('a second 401 ends the session without a refresh loop', async () => {
  const paths = [];
  let expired = 0;
  const unsubscribe = onSessionExpired(() => { expired += 1; });
  globalThis.fetch = async (path) => {
    paths.push(path);
    return path === '/api/auth/refresh' ? json({ user }) : json({ error: 'unauthorized' }, 401);
  };
  try {
    await assert.rejects(api.me(), (error) => error instanceof ApiError && error.status === 401);
    assert.deepEqual(paths, ['/api/me', '/api/auth/refresh', '/api/me']);
    assert.equal(expired, 1);
  } finally {
    unsubscribe();
  }
});

test('a forbidden response does not attempt refresh', async () => {
  const paths = [];
  globalThis.fetch = async (path) => {
    paths.push(path);
    return json({ error: 'forbidden' }, 403);
  };
  await assert.rejects(api.users(), (error) => error instanceof ApiError && error.status === 403);
  assert.deepEqual(paths, ['/api/admin/users']);
});

test('logout waits for pending refresh and prevents stale authenticated retry', async () => {
  const paths = [];
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  globalThis.fetch = async (path) => {
    paths.push(path);
    if (path === '/api/auth/refresh') { await gate; return json({ user }); }
    if (path === '/api/auth/logout') return new Response(null, { status: 204 });
    return json({ error: 'unauthorized' }, 401);
  };
  const stale = api.users().catch((error) => error);
  await new Promise((resolve) => setTimeout(resolve, 0));
  const logout = api.logout();
  release();
  await logout;
  assert.ok((await stale) instanceof ApiError);
  assert.deepEqual(paths, ['/api/admin/users', '/api/auth/refresh', '/api/auth/logout']);
});

test('malformed account data is rejected', async () => {
  globalThis.fetch = async () => json({ user: { username: 'admin' } });
  await assert.rejects(api.login('admin', 'synthetic-test-password'), (error) => error instanceof ApiError && error.code === 'invalid_response');
});

// Keep this file independent from DOM libraries and real credentials.
process.on('exit', () => { globalThis.window = originalWindow; });
