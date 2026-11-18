import assert from 'node:assert/strict';
import { test } from 'node:test';
import { consumeInviteFragment, inviteLink } from '../src/lib/invites.ts';

const token = 'synthetic_invitation_token_1234567890';

test('fragment is removed immediately and unrelated URL path/query survive', () => {
  const replacements = [];
  const location = { hash: '#invite=' + token, pathname: '/', search: '?theme=retro' };
  const history = { state: { page: 1 }, replaceState: (...args) => replacements.push(args) };
  assert.deepEqual(consumeInviteFragment(location, history), { token, invalid: false });
  assert.deepEqual(replacements, [[{ page: 1 }, '', '/?theme=retro']]);
});

test('invalid or repeated invitation is cleared but not accepted', () => {
  for (const hash of ['#invite=short', '#invite=' + token + '&invite=' + token]) {
    let cleaned = false;
    const result = consumeInviteFragment({ hash, pathname: '/', search: '' }, { state: null, replaceState: () => { cleaned = true; } });
    assert.deepEqual(result, { token: null, invalid: true });
    assert.equal(cleaned, true);
  }
});

test('ordinary navigation fragment is preserved', () => {
  assert.deepEqual(consumeInviteFragment({ hash: '#profile', pathname: '/', search: '' }, { state: null, replaceState: () => assert.fail('Unexpected URL mutation') }), { token: null, invalid: false });
});

test('missing public app URL falls back to current origin with fragment only', () => {
  assert.equal(inviteLink({ token, url: null, expires_at: '' }, 'https://flo.example'), 'https://flo.example/#invite=' + token);
});

test('explicit public app URL is preserved, unsafe URL or query token is rejected', () => {
  assert.equal(inviteLink({ token, url: 'https://public.example/#invite=' + token, expires_at: '' }, 'https://local.example'), 'https://public.example/#invite=' + token);
  for (const url of ['javascript:alert(1)', 'https://public.example/?invite=' + token, 'https://public.example/#invite=wrong']) {
    assert.throws(() => inviteLink({ token, url, expires_at: '' }, 'https://local.example'));
  }
});
