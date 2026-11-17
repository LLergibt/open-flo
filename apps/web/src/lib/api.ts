import { isPair, isRecord, isUser } from './types.ts';
import type { CreateUser, Pair, User } from './types.ts';

export class ApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, status: number = 0) {
    super(code);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
  }
}

const messages: Record<string, string> = {
  unauthorized: 'Сессия завершена. Войдите снова.',
  forbidden: 'Доступ запрещён. Проверьте права аккаунта и настройки адреса сервера.',
  invalid_input: 'Проверьте заполненные поля и допустимую длину значений.',
  invalid_credentials: 'Неверный логин или пароль.',
  conflict: 'Действие недоступно: логин уже занят, аккаунт связан или данные изменились. Обновите список.',
  rate_limited: 'Слишком много запросов. Подождите немного и повторите попытку.',
  internal_error: 'Сервер не смог выполнить запрос. Попробуйте позже.',
  invalid_response: 'Сервер вернул ответ неожиданного формата.',
  network_error: 'Не удалось связаться с сервером. Проверьте подключение и повторите попытку.',
  timeout: 'Сервер не ответил за 15 секунд. Повторите попытку.',
};

export function errorMessage(error: unknown): string {
  return error instanceof ApiError
    ? messages[error.code] ?? 'Не удалось выполнить запрос. Повторите попытку.'
    : 'Не удалось выполнить запрос. Повторите попытку.';
}

let refreshPromise: Promise<void> | null = null;
let sessionExpired: (() => void) | null = null;
let authEpoch = 0;
let signingOut = false;

export function onSessionExpired(listener: () => void): () => void {
  sessionExpired = listener;
  return () => {
    if (sessionExpired === listener) sessionExpired = null;
  };
}

async function send(path: string, init: RequestInit = {}): Promise<unknown> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), 15_000);
  const headers = new Headers(init.headers);
  headers.set('Accept', 'application/json');
  if (init.body !== undefined) headers.set('Content-Type', 'application/json');

  try {
    const response = await fetch(path, {
      ...init,
      headers,
      credentials: 'same-origin',
      cache: 'no-store',
      signal: controller.signal,
    });
    if (response.status === 204) return undefined;
    let value: unknown;
    try {
      value = await response.json();
    } catch {
      if (!response.ok) throw new ApiError(response.status === 401 ? 'unauthorized' : 'internal_error', response.status);
      throw new ApiError('invalid_response', response.status);
    }
    if (!response.ok) {
      throw new ApiError(isRecord(value) && typeof value.error === 'string' ? value.error : 'internal_error', response.status);
    }
    return value;
  } catch (error) {
    if (error instanceof ApiError) throw error;
    throw new ApiError(controller.signal.aborted ? 'timeout' : 'network_error');
  } finally {
    window.clearTimeout(timeout);
  }
}

async function refresh(): Promise<void> {
  if (!refreshPromise) {
    refreshPromise = send('/api/auth/refresh', { method: 'POST', body: '{}' })
      .then(() => undefined)
      .finally(() => { refreshPromise = null; });
  }
  return refreshPromise;
}

async function authenticated(path: string, init: RequestInit = {}): Promise<unknown> {
  const epoch = authEpoch;
  if (signingOut) throw new ApiError('unauthorized', 401);
  try {
    const value = await send(path, init);
    if (epoch !== authEpoch) throw new ApiError('unauthorized', 401);
    return value;
  } catch (error) {
    if (!(error instanceof ApiError) || error.status !== 401) throw error;
  }

  if (signingOut || epoch !== authEpoch) throw new ApiError('unauthorized', 401);
  try {
    await refresh();
    if (signingOut || epoch !== authEpoch) throw new ApiError('unauthorized', 401);
    // Exactly one retry. A 403 never triggers a refresh.
    const value = await send(path, init);
    if (epoch !== authEpoch) throw new ApiError('unauthorized', 401);
    return value;
  } catch (error) {
    if (epoch === authEpoch && error instanceof ApiError && error.status === 401) sessionExpired?.();
    throw error;
  }
}

function parseUser(value: unknown): User {
  if (!isUser(value)) throw new ApiError('invalid_response');
  return value;
}

export const api = {
  async setupStatus(): Promise<boolean> {
    const value = await send('/api/setup/status');
    if (!isRecord(value) || typeof value.needs_setup !== 'boolean') throw new ApiError('invalid_response');
    return value.needs_setup;
  },
  async me(): Promise<User> {
    return parseUser(await authenticated('/api/me'));
  },
  async login(username: string, password: string): Promise<User> {
    if (signingOut) throw new ApiError('unauthorized', 401);
    authEpoch += 1;
    const value = await send('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username: username.trim().toLowerCase(), password, client: 'web' }),
    });
    if (!isRecord(value)) throw new ApiError('invalid_response');
    // Token fields in the response are intentionally ignored. The browser uses HttpOnly cookies.
    return parseUser(value.user);
  },
  async logout(): Promise<void> {
    signingOut = true;
    authEpoch += 1;
    try {
      // Do not allow a pending refresh response to restore cookies after logout.
      if (refreshPromise) await refreshPromise.catch(() => undefined);
      await send('/api/auth/logout', { method: 'POST', body: '{}' });
    } finally {
      signingOut = false;
    }
  },
  async users(): Promise<User[]> {
    const value = await authenticated('/api/admin/users');
    if (!isRecord(value) || !Array.isArray(value.users) || !value.users.every(isUser)) throw new ApiError('invalid_response');
    return value.users;
  },
  async createUser(input: CreateUser): Promise<User> {
    return parseUser(await authenticated('/api/admin/users', {
      method: 'POST',
      body: JSON.stringify({ ...input, username: input.username.trim().toLowerCase(), display_name: input.display_name.trim() }),
    }));
  },
  async updateUser(id: string, changes: { is_active?: boolean; password?: string }): Promise<User> {
    return parseUser(await authenticated(`/api/admin/users/${encodeURIComponent(id)}`, {
      method: 'PATCH', body: JSON.stringify(changes),
    }));
  },
  async pairs(): Promise<Pair[]> {
    const value = await authenticated('/api/admin/pairs');
    if (!isRecord(value) || !Array.isArray(value.pairs) || !value.pairs.every(isPair)) throw new ApiError('invalid_response');
    return value.pairs;
  },
  async createPair(trackerId: string, partnerId: string): Promise<Pair> {
    const value = await authenticated('/api/admin/pairs', {
      method: 'POST', body: JSON.stringify({ tracker_id: trackerId, partner_id: partnerId }),
    });
    if (!isPair(value)) throw new ApiError('invalid_response');
    return value;
  },
  async unlinkPair(id: string): Promise<void> {
    await authenticated(`/api/admin/pairs/${encodeURIComponent(id)}`, { method: 'DELETE' });
  },
  async myPair(): Promise<Pair | null> {
    const value = await authenticated('/api/me/pair');
    if (!isRecord(value) || !(value.pair === null || isPair(value.pair))) throw new ApiError('invalid_response');
    return value.pair;
  },
  async leavePair(): Promise<void> {
    await authenticated('/api/me/pair', { method: 'DELETE' });
  },
};
