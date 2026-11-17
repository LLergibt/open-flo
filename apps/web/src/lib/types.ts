export type Role = 'admin' | 'tracker' | 'partner';

export interface User {
  id: string;
  username: string;
  display_name: string;
  role: Role;
  is_active: boolean;
}

export interface Pair {
  id: string;
  tracker: User;
  partner: User;
  created_at: string;
}

export interface Invite {
  token: string;
  url: string | null;
  expires_at: string;
}

export interface InvitePreview {
  kind?: "pair" | "account";
  role?: "tracker" | "partner";
  tracker_display_name: string;
  expires_at: string;
}

export interface InviteRegistration {
  user: User;
  pair: Pair | null;
}

export interface CreateUser {
  username: string;
  display_name: string;
  password: string;
  role: Role;
}

export const roleNames: Record<Role, string> = {
  admin: 'Администратор',
  tracker: 'Владелец дневника',
  partner: 'Партнёр',
};

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function isUser(value: unknown): value is User {
  return record(value)
    && typeof value.id === 'string'
    && typeof value.username === 'string'
    && typeof value.display_name === 'string'
    && (value.role === 'admin' || value.role === 'tracker' || value.role === 'partner')
    && typeof value.is_active === 'boolean';
}

export function isPair(value: unknown): value is Pair {
  return record(value)
    && typeof value.id === 'string'
    && isUser(value.tracker)
    && isUser(value.partner)
    && typeof value.created_at === 'string';
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return record(value);
}
