import { ApiError } from './api.ts';
import type { Invite } from './types.ts';

export function validInviteToken(token: string): boolean {
  return /^[a-zA-Z0-9_-]{32,256}$/.test(token);
}

export function consumeInviteFragment(location: Pick<Location, 'hash' | 'pathname' | 'search'>, history: Pick<History, 'replaceState' | 'state'>): { token: string | null; invalid: boolean } {
  const params = new URLSearchParams(location.hash.replace(/^#/, ''));
  if (!params.has('invite')) return { token: null, invalid: false };
  // Clear the fragment before preview or any asynchronous operation.
  history.replaceState(history.state, '', location.pathname + location.search);
  const token = params.get('invite') ?? '';
  if (params.getAll('invite').length !== 1 || !validInviteToken(token)) return { token: null, invalid: true };
  return { token, invalid: false };
}

export function inviteLink(invite: Invite, currentOrigin: string): string {
  if (!validInviteToken(invite.token)) throw new ApiError('invalid_response');
  if (!invite.url) return new URL('/#invite=' + encodeURIComponent(invite.token), currentOrigin).href;
  let url: URL;
  try { url = new URL(invite.url); } catch { throw new ApiError('invalid_response'); }
  const params = new URLSearchParams(url.hash.replace(/^#/, ''));
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password || url.search || url.pathname !== '/' || params.get('invite') !== invite.token) throw new ApiError('invalid_response');
  return url.href;
}

export function expiryLabel(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? 'Уточните срок у владельца ссылки' : date.toLocaleString('ru-RU', { dateStyle: 'medium', timeStyle: 'short' });
}
