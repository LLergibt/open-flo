import { ApiError } from './api';
import { isCalendar, type CalendarData } from './calendar';
import { isRecord, isUser, type User } from './types';

const maximumAge = 7 * 24 * 60 * 60 * 1000;
const key = 'current';

export interface CalendarSnapshot {
  account: User;
  data: CalendarData;
  savedAt: number;
}

let database: Promise<IDBDatabase> | undefined;

function open(): Promise<IDBDatabase> {
  if (!database) {
    const pending = new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open('flo-local', 1);
      request.onupgradeneeded = () => request.result.createObjectStore('calendar');
      request.onsuccess = () => {
        const connection = request.result;
        connection.onversionchange = () => { connection.close(); database = undefined; };
        resolve(connection);
      };
      request.onerror = () => reject(request.error);
    });
    database = pending.catch((error) => { database = undefined; throw error; });
  }
  return database;
}

async function access<T>(mode: IDBTransactionMode, operation: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const connection = await open();
  return new Promise((resolve, reject) => {
    const transaction = connection.transaction('calendar', mode);
    const request = operation(transaction.objectStore('calendar'));
    transaction.oncomplete = () => resolve(request.result);
    transaction.onabort = () => reject(transaction.error ?? request.error);
  });
}

export function unavailable(failure: unknown): boolean {
  return failure instanceof ApiError && (
    failure.code === 'network_error' || failure.code === 'timeout' ||
    failure.status >= 500 && failure.status < 600
  );
}

export async function forgetCalendar() {
  try { await access('readwrite', (store) => store.delete(key)); } catch { /* Storage may be unavailable. */ }
}

export async function rememberCalendar(account: User, data: CalendarData) {
  // Copy the API result before Svelte's state proxy wraps it.
  if (data.viewer_id !== account.id) return;
  const snapshot = { account: { ...account }, data, savedAt: Date.now() };
  try { await access('readwrite', (store) => store.put(snapshot, key)); } catch { /* Online access still works. */ }
}

export function roleMatches(account: User, data: CalendarData): boolean {
  return account.is_active && (account.role === 'tracker' && data.access === 'owner' ||
    account.role === 'partner' && data.access === 'partner');
}

export async function recallCalendar(account?: User): Promise<CalendarSnapshot | null> {
  try {
    const value: unknown = await access('readonly', (store) => store.get(key));
    if (!isRecord(value) || !isUser(value.account) || !isCalendar(value.data) || typeof value.savedAt !== 'number') return null;
    if (value.savedAt > Date.now() || Date.now() - value.savedAt >= maximumAge || !roleMatches(value.account, value.data) || value.data.viewer_id !== value.account.id) {
      await forgetCalendar();
      return null;
    }
    if (account && (account.id !== value.account.id || account.role !== value.account.role)) return null;
    return { account: value.account, data: value.data, savedAt: value.savedAt };
  } catch {
    return null;
  }
}
