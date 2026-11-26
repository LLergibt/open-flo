import catalog from './diary-catalog.json' with { type: 'json' };

export interface Period { id: string; start_date: string; end_date: string | null }
export interface CycleSettings {
  cycle_length: number | null; period_length: number;
  context: 'regular' | 'irregular' | 'pregnancy' | 'postpartum' | 'hormonal_contraception' | 'paused';
  length_method: 'auto' | 'manual';
}
export interface CycleEstimate {
  kind: 'retrospective' | 'projected';
  period_start: string | null;
  period_end: string | null;
  ovulation_date: string | null;
  fertile_start: string | null;
  fertile_end: string | null;
}

export interface Prediction {
  algorithm_version: string; status: 'estimated' | 'insufficient_data' | 'paused' | 'irregular' | 'overdue';
  reason: string | null; source: string | null; cycle_length: number | null; period_length: number;
  next_period_start: string | null; next_period_end: string | null;
  ovulation_date: string | null; fertile_start: string | null; fertile_end: string | null;
  interval_count: number; variability_days: number | null;
  additional_cycles?: CycleEstimate[];
}
export interface CalendarData {
  access: 'owner' | 'partner'; owner_display_name: string; as_of: string; viewer_id?: string;
  periods: Period[]; entries: DiaryEntry[]; partner_entries?: PartnerDiaryEntry[]; partner_diary_enabled?: boolean; partner_pair_id?: string | null; prediction: Prediction; settings: CycleSettings | null;
}
export interface DiaryEntry {
  date: string; symptoms: string[]; events: string[]; mood: string; flow: string; pain: number; notes: string;
}
export type DiaryInput = Omit<DiaryEntry, 'date'>;
export type PartnerDiaryInput = Omit<DiaryInput, 'flow'>;
// Previously saved offline entries may have only mood and notes.
export type PartnerDiaryEntry = Pick<PartnerDiaryInput, 'mood' | 'notes'> &
  Partial<Pick<PartnerDiaryInput, 'symptoms' | 'events' | 'pain'>> & { date: string };
type DiaryCategory = {
  title: string;
  field: 'symptoms' | 'events';
  options: [string, string][];
  owner_only?: boolean;
  owner_only_options?: string[];
};
export const diaryCategories = catalog as DiaryCategory[];
const partnerDiaryCategories = diaryCategories.filter((category) => !category.owner_only)
  .map((category) => ({ ...category, options: category.options.filter(([id]) => !category.owner_only_options?.includes(id)) }));
export function diaryCategoriesFor(partner: boolean) { return partner ? partnerDiaryCategories : diaryCategories; }
export const moods = [['', 'Не отмечено'], ['calm', 'Спокойствие'], ['happy', 'Радость'], ['sad', 'Грусть'], ['anxious', 'Тревога'], ['irritable', 'Раздражительность'], ['energetic', 'Энергичность']] as const;
export const flows = [['', 'Не отмечено'], ['spotting', 'Мажущие выделения'], ['light', 'Слабые'], ['medium', 'Умеренные'], ['heavy', 'Обильные']] as const;
export const contexts = [['regular', 'Регулярный цикл'], ['irregular', 'Нерегулярный цикл'], ['pregnancy', 'Беременность'], ['postpartum', 'После родов'], ['hormonal_contraception', 'Гормональная контрацепция'], ['paused', 'Приостановить прогноз']] as const;
export const defaultSettings: CycleSettings = { cycle_length: null, period_length: 5, context: 'regular', length_method: 'auto' };
export function emptyDiary(): DiaryInput { return { symptoms: [], events: [], mood: '', flow: '', pain: 0, notes: '' }; }
function strings(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string');
}
function record(value: unknown): value is Record<string, unknown> { return typeof value === 'object' && value !== null; }
const date = (v: unknown): v is string => typeof v === 'string' && /^20\d{2}-\d{2}-\d{2}$/.test(v) || typeof v === 'string' && /^2100-\d{2}-\d{2}$/.test(v);
const nullableDate = (v: unknown) => v === null || date(v);
export function isPeriod(v: unknown): v is Period { return record(v) && typeof v.id === 'string' && date(v.start_date) && nullableDate(v.end_date); }
export function isSettings(v: unknown): v is CycleSettings {
  return record(v) && (v.cycle_length === null || typeof v.cycle_length === 'number') && typeof v.period_length === 'number'
    && contexts.some(([id]) => id === v.context) && ['auto', 'manual'].includes(String(v.length_method));
}
function isCycleEstimate(value: unknown): value is CycleEstimate {
  return record(value) && (value.kind === 'retrospective' || value.kind === 'projected')
    && [value.period_start, value.period_end, value.ovulation_date, value.fertile_start, value.fertile_end].every(nullableDate);
}
export function isCalendar(v: unknown): v is CalendarData {
  if (!record(v) || !['owner', 'partner'].includes(String(v.access)) || typeof v.owner_display_name !== 'string' || !date(v.as_of)
    || !Array.isArray(v.periods) || !v.periods.every(isPeriod) || !Array.isArray(v.entries) || !v.entries.every(isDiary) || !(v.settings === null || isSettings(v.settings)) || !record(v.prediction)) return false;
  if (v.partner_entries !== undefined && (!Array.isArray(v.partner_entries) || !v.partner_entries.every(
    (entry) => record(entry) && date(entry.date) && typeof entry.mood === 'string' && typeof entry.notes === 'string'
      && (entry.symptoms === undefined || strings(entry.symptoms)) && (entry.events === undefined || strings(entry.events))
      && (entry.pain === undefined || Number.isInteger(entry.pain) && Number(entry.pain) >= 0 && Number(entry.pain) <= 3)
  ))) return false;
  if (v.partner_diary_enabled !== undefined && typeof v.partner_diary_enabled !== 'boolean') return false;
  if (v.viewer_id !== undefined && typeof v.viewer_id !== 'string') return false;
  if (v.partner_pair_id !== undefined && v.partner_pair_id !== null && typeof v.partner_pair_id !== 'string') return false;
  const p = v.prediction;
  return typeof p.algorithm_version === 'string' && ['estimated', 'insufficient_data', 'paused', 'irregular', 'overdue'].includes(String(p.status))
    && (p.reason === null || typeof p.reason === 'string') && (p.source === null || typeof p.source === 'string')
    && (p.cycle_length === null || typeof p.cycle_length === 'number') && typeof p.period_length === 'number'
    && [p.next_period_start, p.next_period_end, p.ovulation_date, p.fertile_start, p.fertile_end].every(nullableDate)
    && (p.additional_cycles === undefined || Array.isArray(p.additional_cycles) && p.additional_cycles.every(isCycleEstimate))
    && typeof p.interval_count === 'number' && (p.variability_days === null || typeof p.variability_days === 'number');
}
export function isDiary(v: unknown): v is DiaryEntry {
  return record(v) && date(v.date) && strings(v.symptoms)
    && strings(v.events) && typeof v.mood === 'string' && typeof v.flow === 'string'
    && typeof v.pain === 'number' && typeof v.notes === 'string';
}

export function dayDetails(data: CalendarData, date: string): string[] {
  const within = (start: string | null, end: string | null) => start !== null && end !== null && start <= date && date <= end;
  const p = data.prediction;
  const cycles = p.additional_cycles ?? [];
  if (data.periods.some(period => within(period.start_date, period.end_date ?? data.as_of))) return ['Отмеченные месячные'];
  if (p.status !== 'overdue' && within(p.next_period_start, p.next_period_end)
      || cycles.some(cycle => within(cycle.period_start, cycle.period_end))) return ['Ожидаемые месячные'];
  if (p.ovulation_date === date || cycles.some(cycle => cycle.ovulation_date === date)) return ['Предполагаемая овуляция', 'Расчётное фертильное окно'];
  if (within(p.fertile_start, p.fertile_end) || cycles.some(cycle => within(cycle.fertile_start, cycle.fertile_end))) return ['Расчётное фертильное окно'];
  return [];
}
