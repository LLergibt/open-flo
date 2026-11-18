<script lang="ts">
  import { untrack } from 'svelte';
  import { dateTitle } from '../../lib/dates';
  import type { Period } from '../../lib/calendar';
  let { periods, today, selected, disabled = false, onsave, ondelete }: {
    periods: Period[]; today: string; selected: string; disabled?: boolean;
    onsave: (input: { start_date: string; end_date: string | null }, id?: string) => Promise<boolean>;
    ondelete: (id: string) => Promise<boolean>;
  } = $props();
  let editingId = $state<string | undefined>();
  let start = $state(untrack(() => selected <= today ? selected : today));
  let end = $state('');
  let ongoing = $state(true);
  let busy = $state(false);
  let error = $state('');
  let deleting = $state<string | null>(null);
  function edit(period: Period) { if (busy || disabled) return; editingId = period.id; start = period.start_date; end = period.end_date ?? ''; ongoing = period.end_date === null; error = ''; deleting = null; }
  function reset() { editingId = undefined; start = selected <= today ? selected : today; end = ''; ongoing = true; error = ''; deleting = null; }
  async function submit(event: SubmitEvent) {
    event.preventDefault(); if (busy || disabled) return;
    if (!ongoing && end < start) { error = 'Окончание не может быть раньше начала.'; return; }
    busy = true; error = '';
    try { if (await onsave({ start_date: start, end_date: ongoing ? null : end }, editingId)) reset(); } finally { busy = false; }
  }
  async function remove(id: string) { busy = true; try { if (await ondelete(id)) reset(); } finally { busy = false; } }
  const sorted = $derived([...periods].sort((a, b) => b.start_date.localeCompare(a.start_date)));
</script>
<details class="calendar-details">
  <summary>Месячные и история дат</summary>
  <p class="detail">Отметьте последние месячные или исправьте любую запись. Изменение начала сразу пересчитает прогноз. Записи дневника остаются на своих календарных датах.</p>
  <form onsubmit={submit} aria-label={editingId ? 'Исправить даты месячных' : 'Добавить месячные'} class="spaced">
    <fieldset disabled={disabled || busy}>
      <legend>{editingId ? 'Редактирование дат' : 'Новая запись месячных'}</legend>
      <div class="field-grid">
        <div class="field"><label for="period-start">Дата начала</label><input id="period-start" name="start_date" type="date" bind:value={start} min="2000-01-01" max={today} required /></div>
        <div class="field"><label for="period-end">Дата окончания</label><input id="period-end" name="end_date" type="date" bind:value={end} min={start || '2000-01-01'} max={today} required={!ongoing} disabled={ongoing || disabled || busy} /></div>
      </div>
      <label class="checkbox-field"><input type="checkbox" bind:checked={ongoing} /><span>Месячные ещё идут</span></label>
      {#if !editingId && selected <= today && start !== selected}<button class="secondary small" type="button" onclick={() => { start = selected; }}>Взять дату из календаря</button>{/if}
      {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
      <div class="actions compact"><button type="submit">{busy ? 'Сохраняем…' : editingId ? 'Сохранить исправление' : 'Добавить месячные'}</button>{#if editingId}<button class="secondary" type="button" onclick={reset}>Отменить редактирование</button>{/if}</div>
    </fieldset>
  </form>
  <div class="period-history spaced" aria-label="История месячных">
    {#if !sorted.length}<p class="detail">Пока нет отмеченных месячных.</p>{/if}
    {#each sorted as period (period.id)}
      <article class="period-row">
        <div><strong>{dateTitle(period.start_date)}</strong><p class="detail">{period.end_date ? 'по ' + dateTitle(period.end_date) : 'Продолжаются'}</p></div>
        <button class="secondary small" type="button" disabled={busy || disabled} onclick={() => edit(period)}>Изменить даты</button>
        {#if deleting === period.id}<div class="confirmation"><p>Удалить эту ошибочную запись? Прогноз будет пересчитан.</p><button type="button" disabled={busy || disabled} onclick={() => void remove(period.id)}>Да, удалить запись</button><button class="secondary" type="button" disabled={busy || disabled} onclick={() => { deleting = null; }}>Отмена</button></div>
        {:else}<button class="secondary small" type="button" disabled={busy || disabled} onclick={() => { deleting = period.id; }}>Удалить запись</button>{/if}
      </article>
    {/each}
  </div>
</details>
