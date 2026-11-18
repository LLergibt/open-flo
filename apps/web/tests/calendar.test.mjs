import assert from 'node:assert/strict';
import { test } from 'node:test';
import { dayDetails, isCalendar } from '../src/lib/calendar.ts';

const calendar = {
  access: 'partner', owner_display_name: 'Fixture', as_of: '2026-11-18',
  settings: null, periods: [{ id: 'period', start_date: '2026-11-01', end_date: '2026-11-05' }],
  entries: [{ date: '2026-11-05', symptoms: ['acne'], events: ['orgasm'], mood: '', flow: '', pain: 0, notes: 'Fixture' }],
  prediction: {
    algorithm_version: 'calendar-v4', status: 'estimated', reason: null, source: 'history',
    cycle_length: 28, period_length: 5, interval_count: 2, variability_days: 0,
    next_period_start: '2026-11-29', next_period_end: '2026-12-03',
    fertile_start: '2026-11-05', fertile_end: '2026-11-15', ovulation_date: '2026-11-11',
    additional_cycles: [{ kind: 'projected', period_start: '2026-12-27', period_end: '2026-12-31', fertile_start: null, fertile_end: null, ovulation_date: null }],
  },
};

test('day details prioritize actual dates and include projected cycles', () => {
  assert.equal(isCalendar(calendar), true);
  assert.deepEqual(dayDetails(calendar, '2026-11-05'), ['Отмеченные месячные']);
  assert.deepEqual(dayDetails(calendar, '2026-11-11'), ['Предполагаемая овуляция', 'Расчётное фертильное окно']);
  assert.deepEqual(dayDetails(calendar, '2026-12-27'), ['Ожидаемые месячные']);
  assert.deepEqual(dayDetails(calendar, '2026-11-18'), []);
});

test('calendar rejects malformed day entries and period estimates', () => {
  const snapshot = structuredClone(calendar);
  snapshot.entries[0].symptoms = [null];
  assert.equal(isCalendar(snapshot), false);
  snapshot.entries = [];
  snapshot.prediction.additional_cycles[0].kind = 'unknown';
  assert.equal(isCalendar(snapshot), false);
});
