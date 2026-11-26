<script lang="ts">
  import type { CalendarData } from '../../lib/calendar';
  import { dateTitle, inRange, monthDays, monthTitle, shiftMonth } from '../../lib/dates';
  let { data, month = $bindable(), selected = $bindable(), ondayselect, disabled = false }: { data: CalendarData; month: string; selected: string; ondayselect: (day: string) => void; disabled?: boolean } = $props();
  const loggedDates = $derived(new Set(data.entries.map((entry) => entry.date)));
  const partnerDates = $derived(new Set(data.partner_entries?.map((entry) => entry.date)));
  const days = $derived(monthDays(month));
  const cycles = $derived(data.prediction.additional_cycles ?? []);

  function dayState(day: string) {
    const actual = data.periods.some((p) => inRange(day, p.start_date, p.end_date ?? data.as_of));
    const prediction = data.prediction;
    const forecast = !actual && (
      prediction.status !== 'overdue' && inRange(day, prediction.next_period_start, prediction.next_period_end)
      || cycles.some((cycle) => inRange(day, cycle.period_start, cycle.period_end))
    );
    const showsFertility = !actual && !forecast;
    const retrospective = showsFertility && cycles.some((cycle) => cycle.kind === 'retrospective'
      && inRange(day, cycle.fertile_start, cycle.fertile_end));
    const fertile = showsFertility && (inRange(day, prediction.fertile_start, prediction.fertile_end)
      || cycles.some((cycle) => inRange(day, cycle.fertile_start, cycle.fertile_end)));
    const ovulation = showsFertility && (day === prediction.ovulation_date
      || cycles.some((cycle) => day === cycle.ovulation_date));
    const label = [
      dateTitle(day),
      day === data.as_of ? 'сегодня' : '',
      actual ? 'отмеченные месячные' : forecast ? 'прогноз месячных' : '',
      retrospective ? 'оценка фертильного окна прошлого цикла' : fertile ? 'предполагаемое фертильное окно' : '',
      ovulation ? 'предполагаемая овуляция' : '',
      loggedDates.has(day) ? 'есть запись владельца дневника' : '',
      partnerDates.has(day) ? 'есть запись партнёра' : '',
    ].filter(Boolean).join(', ');
    return { actual, forecast, fertile, ovulation, retrospective, label };
  }
</script>

<div class="calendar-toolbar">
  <button type="button" class="secondary month-arrow" aria-label="Предыдущий месяц" disabled={disabled || month <= '2000-01'} onclick={() => { month = shiftMonth(month, -1); }}>←</button>
  <h2 id="calendar-month" aria-live="polite">{monthTitle(month)}</h2>
  <button type="button" class="secondary month-arrow" aria-label="Следующий месяц" disabled={disabled || month >= '2100-12'} onclick={() => { month = shiftMonth(month, 1); }}>→</button>
</div>
<div class="calendar-weekdays" aria-hidden="true">{#each ['Пн','Вт','Ср','Чт','Пт','Сб','Вс'] as day}<span>{day}</span>{/each}</div>
<div class="calendar-days" role="group" aria-labelledby="calendar-month">
  {#each days as day (day)}
    {@const state = dayState(day)}
    <button type="button" class="calendar-day" class:outside={day.slice(0, 7) !== month}
      class:actual={state.actual} class:forecast={state.forecast} class:fertile={state.fertile}
      class:ovulation={state.ovulation} class:selected={selected === day}
      aria-label={state.label} aria-pressed={selected === day} aria-current={day === data.as_of ? 'date' : undefined}
      disabled={disabled || day < '2000-01-01' || day > '2100-12-31'}
      onclick={() => { selected = day; month = day.slice(0, 7); ondayselect(day); }}>
      <span>{Number(day.slice(-2))}{#if loggedDates.has(day)}<sup class="owner-mark" aria-hidden="true">●</sup>{/if}{#if partnerDates.has(day)}<sup class="partner-mark" aria-hidden="true">◆</sup>{/if}</span>
      {#if state.actual || state.forecast || state.fertile}
        <span class="day-mark" aria-hidden="true">
          {state.actual ? '●' : state.forecast ? '┄' : ''}{state.ovulation ? (state.retrospective ? '◌' : '○') : state.fertile ? (state.retrospective ? '≈' : '~') : ''}
        </span>
      {/if}
    </button>
  {/each}
</div>
<div class="calendar-legend" aria-label="Обозначения календаря">
  <span><i class="legend-actual" aria-hidden="true"></i>Месячные · факт</span>
  <span><i class="legend-forecast" aria-hidden="true"></i>Месячные · прогноз</span>
  <span><i class="legend-fertile" aria-hidden="true"></i>Фертильное окно · оценка</span>
  <span>○ Овуляция · оценка</span>
  <span>≈ / ◌ Оценки прошлых циклов</span>
  <span><b class="owner-mark" aria-hidden="true">●</b> Запись владельца</span>
  {#if data.partner_diary_enabled}<span><b class="partner-mark" aria-hidden="true">◆</b> Запись партнёра</span>{/if}
</div>
