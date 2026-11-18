// Calendar dates carry no instant or time zone. The Safari fallback loads locally, never from a CDN.
type TemporalAPI = typeof import('temporal-polyfill').Temporal;
let temporal: TemporalAPI;
export async function initCalendarDates(): Promise<void> {
  temporal = (globalThis as unknown as { Temporal?: TemporalAPI }).Temporal ?? (await import('temporal-polyfill')).Temporal;
}
export function localToday(): string {
  const now = new Date();
  return [now.getFullYear(), String(now.getMonth() + 1).padStart(2, '0'), String(now.getDate()).padStart(2, '0')].join('-');
}
export function shiftDay(date: string, days: number): string { return temporal.PlainDate.from(date).add({ days }).toString(); }
export function shiftMonth(month: string, months: number): string { return temporal.PlainDate.from(month + '-01').add({ months }).toString().slice(0, 7); }
export function monthTitle(month: string): string { return temporal.PlainDate.from(month + '-01').toLocaleString('ru-RU', { month: 'long', year: 'numeric' }); }
export function dateTitle(date: string): string { return temporal.PlainDate.from(date).toLocaleString('ru-RU', { day: 'numeric', month: 'long', year: 'numeric' }); }
export function monthDays(month: string): string[] {
  const first = temporal.PlainDate.from(month + '-01');
  const start = first.subtract({ days: first.dayOfWeek - 1 });
  return Array.from({ length: Math.ceil((first.dayOfWeek - 1 + first.daysInMonth) / 7) * 7 }, (_, days) => start.add({ days }).toString());
}
export function inRange(date: string, start: string | null, end: string | null): boolean { return !!start && !!end && date >= start && date <= end; }
