<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, ApiError, errorMessage } from '../lib/api';
  import { defaultSettings, dayDetails } from '../lib/calendar';
  import type { CalendarData, CycleSettings, DiaryInput, Period } from '../lib/calendar';
  import { forgetCalendar, recallCalendar, rememberCalendar, roleMatches, unavailable } from '../lib/offline-calendar';
  import type { User } from '../lib/types';
  import { dateTitle, initCalendarDates, localToday } from '../lib/dates';
  import CalendarMonth from './calendar/CalendarMonth.svelte';
  import ForecastSummary from './calendar/ForecastSummary.svelte';
  import CycleSettingsForm from './calendar/CycleSettingsForm.svelte';
  import PeriodEditor from './calendar/PeriodEditor.svelte';
  import DiaryReadOnly from './calendar/DiaryReadOnly.svelte';
  import DiaryEditor from './calendar/DiaryEditor.svelte';
  let { user, disabled = false, offlineData, savedAt, onreconnect }: {
    user: User; disabled?: boolean; offlineData?: CalendarData; savedAt?: number; onreconnect?: () => Promise<void>;
  } = $props();
  let liveData = $state<CalendarData | null>(null);
  let fallbackAt = $state<number>();
  let datesReady = $state(false);
  const data = $derived(datesReady ? offlineData ?? liveData : null);
  const offline = $derived(offlineData !== undefined || fallbackAt !== undefined);
  const copyTime = $derived(savedAt ?? fallbackAt);
  let selected = $state(localToday());
  let month = $state(localToday().slice(0, 7));
  let busy = $state(false);
  const canChange = $derived(!disabled && !offline && !busy);
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
  const partnerCanWrite = $derived(data?.access === 'partner' && Boolean(data.partner_pair_id) && user.role === 'partner');
  const partnerEntry = $derived(data?.partner_entries?.find((entry) => entry.date === selected));
  const diaryEntry = $derived(data?.entries.find((entry) => entry.date === selected));
  async function load() {
    if (offlineData) { await onreconnect?.(); return; }
    const current = ++generation;
    busy = true;
    error = '';
    try {
      await initCalendarDates();
      const next = await api.calendar(user.id);
      if (!mounted || current !== generation) return;
      if (!roleMatches(user, next) || next.viewer_id !== undefined && next.viewer_id !== user.id) {
        void forgetCalendar();
        throw new ApiError('forbidden', 403);
      }
      if (liveData?.partner_pair_id && liveData.partner_pair_id !== next.partner_pair_id) {
        editing = false;
        dayDialog?.close();
      }
      // Persist the unwrapped API response before using it as reactive state.
      void rememberCalendar(user, next);
      liveData = next;
      fallbackAt = undefined;
      ready = true;
      revision += 1;
    } catch (failure) {
      if (!mounted || current !== generation) return;
      const snapshot = unavailable(failure) ? await recallCalendar(user) : null;
      if (!mounted || current !== generation) return;
      if (snapshot) {
        liveData = snapshot.data;
        fallbackAt = snapshot.savedAt;
        editing = false;
      } else {
        if (failure instanceof ApiError && (failure.status === 401 || failure.status === 403)) void forgetCalendar();
        liveData = null;
        fallbackAt = undefined;
        dayDialog?.close();
        error = failure instanceof ApiError && failure.status === 403 && user.role === 'partner'
          ? 'Календарь и дневник появятся после привязки к активному владельцу.' : errorMessage(failure);
      }
    } finally { if (mounted && current === generation) busy = false; }
  }
  async function mutate(operation: () => Promise<void>, message: string) {
    if (!canChange) return false;
    busy = true; error = ''; notice = '';
    try { await operation(); if (!mounted) return false; notice = message; await load(); return true; }
    catch (failure) {
      if (mounted) {
        if (unavailable(failure)) await load();
        if (failure instanceof ApiError && (failure.status === 401 || failure.status === 403)) {
          void forgetCalendar();
          liveData = null;
          dayDialog?.close();
        }
        error = errorMessage(failure);
      }
      return false;
    } finally { if (mounted) busy = false; }
  }
  async function saveSettings(input: CycleSettings) { if (owner) await mutate(() => api.saveCycleSettings(input, user.id), 'Настройки сохранены. Прогноз пересчитан.'); }
  async function savePeriod(input: Pick<Period, 'start_date' | 'end_date'>, id?: string) { return owner && mutate(() => api.savePeriod(input, id, user.id), 'Даты месячных сохранены. Прогноз пересчитан.'); }
  async function deletePeriod(id: string) { return owner && mutate(() => api.deletePeriod(id, user.id), 'Запись месячных удалена.'); }
  async function saveDiary(date: string, input: DiaryInput) { if (owner && await mutate(() => api.saveDiary(date, input, user.id), 'Запись сохранена.')) editing = false; }
  async function deleteDiary(date: string) { if (owner && await mutate(() => api.deleteDiary(date, user.id), 'Запись удалена.')) editing = false; }
  async function savePartnerDiary(date: string, { symptoms, events, mood, pain, notes }: DiaryInput) {
    const pair = data?.partner_pair_id;
    if (partnerCanWrite && pair && await mutate(() => api.savePartnerDiary(pair, date, { symptoms, events, mood, pain, notes }, user.id), 'Запись сохранена.')) editing = false;
  }
  async function deletePartnerDiary(date: string) {
    const pair = data?.partner_pair_id;
    if (partnerCanWrite && pair && await mutate(() => api.deletePartnerDiary(pair, date, user.id), 'Запись удалена.')) editing = false;
  }
  onMount(() => {
    mounted = true;
    void initCalendarDates().then(() => {
      if (!mounted) return;
      datesReady = true;
      if (!offlineData) void load();
    });
    const refresh = () => { if (!offlineData && !document.hidden && !busy) void load(); };
    document.addEventListener('visibilitychange', refresh);
    window.addEventListener('focus', refresh);
    return () => { mounted = false; generation += 1; liveData = null; document.removeEventListener('visibilitychange', refresh); window.removeEventListener('focus', refresh); };
  });
</script>

<section class="window calendar-window" aria-labelledby="calendar-title" aria-busy={busy}>
  <div class="titlebar" aria-hidden="true"><span>calendar.exe</span><span class="window-controls">─ □ ×</span></div>
  <div class="window-body calendar-body">
    <div class="calendar-heading"><div><p class="eyebrow">КАЛЕНДАРЬ ЦИКЛА</p><h1 id="calendar-title">{user.role === 'partner' ? 'Календарь партнёра' : 'Мой календарь'}</h1></div><button class="secondary small" type="button" disabled={busy || disabled} onclick={() => void load()}>Обновить</button></div>
    {#if error}<p class="feedback error" role="alert">{error} Календарь скрыт, если доступ не удалось подтвердить.</p>{/if}
    {#if offline}<p class="feedback" role="status">Локальная копия · только просмотр{copyTime ? ' · сохранена ' + new Date(copyTime).toLocaleString('ru-RU') : ''}.</p>{/if}
    {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
    {#if !ready && busy}<p class="detail" role="status">Загружаем календарь…</p>{/if}
    {#if data}
      {#if data.access === 'partner'}<p class="detail partner-banner">Календарь и дневник · {data.owner_display_name}</p>{/if}
      <CalendarMonth {data} bind:month bind:selected ondayselect={openDay} disabled={busy || disabled} />
      <div class="calendar-selected"><button type="button" class="secondary small" onclick={() => openDay(selected)}>{dateTitle(selected)} · подробности</button><button class="secondary small" type="button" disabled={busy || disabled} onclick={() => { month = data!.as_of.slice(0, 7); openDay(data!.as_of); }}>Сегодня</button></div>
      <ForecastSummary prediction={data.prediction} />
      {#if owner && !offline}
        {#key revision}<PeriodEditor periods={data.periods} today={data.as_of} {selected} disabled={disabled || busy} onsave={savePeriod} ondelete={deletePeriod} /><CycleSettingsForm settings={data.settings ?? defaultSettings} disabled={disabled || busy} onsave={saveSettings} />{/key}
      {/if}
    {/if}
  </div>
  <footer class="window-footer"><span>Даты хранятся на вашем сервере</span><span>{offline ? 'только просмотр' : data?.access === 'partner' ? 'календарь партнёра' : 'личный календарь'}</span></footer>
</section>

<dialog bind:this={dayDialog} class="day-dialog" aria-labelledby="day-dialog-title" onclose={() => { dayOpen = false; editing = false; }}>
  {#if dayOpen && data}
    <header class="day-dialog-heading">
      <h2 id="day-dialog-title">Подробности дня</h2>
      <button type="button" class="secondary small" onclick={() => dayDialog.close()}>Закрыть</button>
    </header>
    {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
    {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
    <p class="detail">{dateTitle(selected)}{dayDetails(data, selected).map((detail) => ' · ' + detail).join('')}</p>
    {#if editing && !offline}
      <button type="button" class="secondary small" disabled={busy} onclick={() => { editing = false; }}>Назад к подробностям</button>
      {#if owner}
        {#key selected + ':' + revision}<DiaryEditor date={selected} entry={diaryEntry} today={data.as_of} disabled={!canChange} onsave={saveDiary} ondelete={deleteDiary} />{/key}
      {:else if partnerCanWrite}
        {#key selected}<DiaryEditor date={selected} entry={partnerEntry} today={data.as_of} partner disabled={!canChange} onsave={savePartnerDiary} ondelete={deletePartnerDiary} />{/key}
      {/if}
    {:else}
      <DiaryReadOnly entry={diaryEntry} />
      {#if data.partner_diary_enabled || partnerEntry}
        <DiaryReadOnly entry={partnerEntry} partner />
      {/if}
      {#if !offline && (owner || partnerCanWrite) && selected <= data.as_of}
        <button type="button" disabled={!canChange} onclick={() => { editing = true; }}>
          {(owner ? diaryEntry : partnerEntry) ? 'Редактировать свою запись' : 'Добавить свою запись'}
        </button>
      {/if}
    {/if}
  {/if}
</dialog>
