<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, ApiError, errorMessage } from '../lib/api';
  import { defaultSettings, dayDetails } from '../lib/calendar';
  import type { CalendarData, CycleSettings, DiaryInput, Period } from '../lib/calendar';
  import type { User } from '../lib/types';
  import { dateTitle, initCalendarDates, localToday } from '../lib/dates';
  import CalendarMonth from './calendar/CalendarMonth.svelte';
  import ForecastSummary from './calendar/ForecastSummary.svelte';
  import CycleSettingsForm from './calendar/CycleSettingsForm.svelte';
  import PeriodEditor from './calendar/PeriodEditor.svelte';
  import DiaryReadOnly from './calendar/DiaryReadOnly.svelte';
  import DiaryEditor from './calendar/DiaryEditor.svelte';
  let { user, disabled = false }: { user: User; disabled?: boolean } = $props();
  let data = $state<CalendarData | null>(null);
  let selected = $state(localToday());
  let month = $state(localToday().slice(0, 7));
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');
  let dayDialog: HTMLDialogElement;
  let dayOpen = $state(false);
  let editing = $state(false);
  async function openDay(day: string) {
    selected = day;
    editing = false;
    dayOpen = true;
    await tick();
    if (mounted && dayOpen && data && !dayDialog.open) dayDialog.showModal();
  }
  let ready = $state(false);
  let revision = $state(0);
  let mounted = false;
  let generation = 0;
  const owner = $derived(data?.access === 'owner' && user.role === 'tracker');
  const diaryEntry = $derived(data?.entries.find((entry) => entry.date === selected));
  async function load() {
    const current = ++generation; busy = true; error = '';
    if (user.role === 'partner') { dayDialog?.close(); data = null; }
    try {
      await initCalendarDates();
      const next = await api.calendar();
      if (!mounted || current !== generation) return;
      data = next; ready = true; revision += 1;
    } catch (failure) {
      if (mounted && current === generation) { data = null; dayDialog?.close(); error = failure instanceof ApiError && failure.status === 403 && user.role === 'partner' ? 'Календарь и дневник появятся после привязки к активному владельцу.' : errorMessage(failure); }
    } finally { if (mounted && current === generation) busy = false; }
  }
  async function mutate(operation: () => Promise<void>, message: string) {
    if (busy || disabled || !owner) return false;
    busy = true; error = ''; notice = '';
    try { await operation(); if (!mounted) return false; notice = message; await load(); return true; }
    catch (failure) { if (mounted) { error = errorMessage(failure); if (failure instanceof Error && 'status' in failure && (failure.status === 401 || failure.status === 403)) { data = null; dayDialog?.close(); } } return false; }
    finally { if (mounted) busy = false; }
  }
  async function saveSettings(input: CycleSettings) { await mutate(() => api.saveCycleSettings(input), 'Настройки сохранены. Прогноз пересчитан.'); }
  async function savePeriod(input: Pick<Period, 'start_date' | 'end_date'>, id?: string) { return mutate(() => api.savePeriod(input, id), 'Даты месячных сохранены. Прогноз пересчитан.'); }
  async function deletePeriod(id: string) { return mutate(() => api.deletePeriod(id), 'Запись месячных удалена.'); }
  async function saveDiary(date: string, input: DiaryInput) { if (await mutate(() => api.saveDiary(date, input), 'Запись сохранена.')) editing = false; }
  async function deleteDiary(date: string) { if (await mutate(() => api.deleteDiary(date), 'Запись удалена.')) editing = false; }
  onMount(() => {
    mounted = true; void load();
    const refresh = () => { if (!document.hidden && !busy) void load(); };
    document.addEventListener('visibilitychange', refresh);
    window.addEventListener('focus', refresh);
    return () => { mounted = false; generation += 1; data = null; document.removeEventListener('visibilitychange', refresh); window.removeEventListener('focus', refresh); };
  });
</script>

<section class="window calendar-window" aria-labelledby="calendar-title" aria-busy={busy}>
  <div class="titlebar" aria-hidden="true"><span>calendar.exe</span><span class="window-controls">─ □ ×</span></div>
  <div class="window-body calendar-body">
    <div class="calendar-heading"><div><p class="eyebrow">КАЛЕНДАРЬ ЦИКЛА</p><h1 id="calendar-title">{user.role === 'partner' ? 'Календарь партнёра' : 'Мой календарь'}</h1></div><button class="secondary small" type="button" disabled={busy || disabled} onclick={() => void load()}>Обновить</button></div>
    {#if error}<p class="feedback error" role="alert">{error} Календарь скрыт, если доступ не удалось подтвердить.</p>{/if}
    {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
    {#if !ready && busy}<p class="detail" role="status">Загружаем календарь…</p>{/if}
    {#if data}
      {#if data.access === 'partner'}<p class="detail partner-banner">Календарь и дневник · {data.owner_display_name}. Только просмотр.</p>{/if}
      <CalendarMonth {data} bind:month bind:selected ondayselect={openDay} disabled={busy || disabled} />
      <div class="calendar-selected"><button type="button" class="secondary small" onclick={() => openDay(selected)}>{dateTitle(selected)} · подробности</button><button class="secondary small" type="button" disabled={busy || disabled} onclick={() => { month = data!.as_of.slice(0, 7); openDay(data!.as_of); }}>Сегодня</button></div>
      <ForecastSummary prediction={data.prediction} />
      {#if owner}
        {#key revision}<PeriodEditor periods={data.periods} today={data.as_of} {selected} disabled={disabled || busy} onsave={savePeriod} ondelete={deletePeriod} /><CycleSettingsForm settings={data.settings ?? defaultSettings} disabled={disabled || busy} onsave={saveSettings} />{/key}
      {/if}
    {/if}
  </div>
  <footer class="window-footer"><span>Даты хранятся на вашем сервере</span><span>{data?.access === 'partner' ? 'только просмотр' : 'личный календарь'}</span></footer>
</section>

<dialog bind:this={dayDialog} class="day-dialog" aria-labelledby="day-dialog-title" onclose={() => { dayOpen = false; editing = false; }}>
  {#if dayOpen && data}
    <header class="day-dialog-heading">
      <h2 id="day-dialog-title">Подробности дня</h2>
      <button type="button" class="secondary small" onclick={() => dayDialog.close()}>Закрыть</button>
    </header>
    {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
    {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
    <p class="detail">{dayDetails(data, selected).join(' · ')}</p>
    {#if editing && owner}
      <button type="button" class="secondary small" disabled={busy} onclick={() => { editing = false; }}>Назад к подробностям</button>
      {#key selected + ':' + revision}<DiaryEditor date={selected} entry={diaryEntry} today={data.as_of} disabled={disabled || busy} onsave={saveDiary} ondelete={deleteDiary} />{/key}
    {:else}
      <DiaryReadOnly date={selected} entry={diaryEntry} {owner} />
      {#if owner && selected <= data.as_of}<button type="button" disabled={disabled || busy} onclick={() => { editing = true; }}>{diaryEntry ? 'Редактировать запись' : 'Добавить запись'}</button>{/if}
    {/if}
  {/if}
</dialog>
