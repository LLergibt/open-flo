<script lang="ts">
  import { untrack } from 'svelte';
  import { dateTitle } from '../../lib/dates';
  import { emptyDiary, diaryCategoriesFor, moods, flows } from '../../lib/calendar';
  import type { DiaryEntry, DiaryInput, PartnerDiaryEntry } from '../../lib/calendar';
  let { date, entry, today, partner = false, disabled = false, onsave, ondelete }: { date: string; entry?: DiaryEntry | PartnerDiaryEntry; today: string; partner?: boolean; disabled?: boolean; onsave: (date: string, input: DiaryInput) => Promise<void>; ondelete: (date: string) => Promise<void> } = $props();
  const initial = untrack(() => ({ ...emptyDiary(), ...entry }));
  const categories = $derived(diaryCategoriesFor(partner));
  let symptoms = $state<string[]>([...initial.symptoms]);
  let events = $state<string[]>([...initial.events]);
  let mood = $state(initial.mood);
  let flow = $state(initial.flow);
  let pain = $state(initial.pain);
  let notes = $state(initial.notes);
  let busy = $state(false);
  let confirming = $state(false);
  const empty = $derived(!symptoms.length && !events.length && !mood && pain === 0 && !notes.trim());
  const locked = $derived(disabled || busy || date > today);
  function toggle(field: 'symptoms' | 'events', value: string, checked: boolean) {
    const current = field === 'symptoms' ? symptoms : events;
    const next = checked ? [...new Set([...current, value])] : current.filter((item) => item !== value);
    if (field === 'symptoms') symptoms = next;
    else events = next;
  }
  async function save(event: SubmitEvent) { event.preventDefault(); if (locked || partner && empty) return; busy = true; try { await onsave(date, { symptoms, events, mood, flow, pain, notes }); } finally { busy = false; } }
  async function remove() { if (locked) return; busy = true; try { await ondelete(date); } finally { busy = false; } }
</script>
<section class="diary-editor" class:diary-partner={partner} class:diary-owner={!partner} aria-labelledby="diary-title">
  <p class="eyebrow">{partner ? 'ДНЕВНИК ПАРТНЁРА' : 'ДНЕВНИК ВЛАДЕЛЬЦА'}</p>
  <h2 id="diary-title">{dateTitle(date)}</h2>
  <p class="detail">{partner ? 'Владелец дневника увидит твою запись.' : 'Связанный партнёр увидит твою запись.'}</p>
  {#if date > today}<p class="feedback spaced">Будущий день. Записи можно вести за сегодня и прошлые даты.</p>{/if}
  <form onsubmit={save} aria-label="Запись личного дневника" class="spaced">
    <fieldset disabled={locked}>
      <legend class="visually-hidden">Отметки дня</legend>

      <div class="field-grid">
        <div class="field"><label for="day-pain">Боль</label><select id="day-pain" name="pain" bind:value={pain}><option value={0}>Не отмечена</option><option value={1}>Слабая</option><option value={2}>Умеренная</option><option value={3}>Сильная</option></select></div>
        {#if !partner}<div class="field"><label for="day-flow">Интенсивность кровотечения</label><select id="day-flow" name="flow" bind:value={flow}>{#each flows as [value, label]}<option {value}>{label}</option>{/each}</select></div>{/if}
      </div>
      {#if !partner}<p class="field-hint">Чтобы запустить расчёт, внесите дату начала в «Месячные и история дат».</p>{/if}
      <div class="field"><label for="day-mood">Настроение</label><select id="day-mood" name="mood" bind:value={mood}>{#each moods as [value, label]}<option {value}>{label}</option>{/each}</select></div>
      {#each categories as category (category.title)}
        <details class="calendar-details diary-category">
          <summary>{category.title}{#if category.options.some(([value]) => (category.field === 'symptoms' ? symptoms : events).includes(value))}<span aria-label="Есть отметки"> · </span>{/if}</summary>
          <fieldset class="choice-group">
            <legend class="visually-hidden">{category.title}</legend>
            <div class="choice-grid">
              {#each category.options as [value, label] (value)}
                <label>
                  <input type="checkbox" name={category.field} {value}
                    checked={(category.field === 'symptoms' ? symptoms : events).includes(value)}
                    onchange={(event) => toggle(category.field, value, event.currentTarget.checked)} />
                  {label}
                </label>
              {/each}
            </div>
          </fieldset>
        </details>
      {/each}
      <div class="field"><label for="day-notes">Личная заметка</label><textarea id="day-notes" name="notes" bind:value={notes} maxlength="2000" rows="4" placeholder="Что хочется сохранить об этом дне"></textarea></div>
      <button type="submit" disabled={partner && empty}>{busy ? 'Сохраняем…' : entry ? 'Сохранить изменения' : 'Сохранить день'}</button>
      {#if entry}
        {#if confirming}<div class="confirmation"><p>Удалить личную запись за этот день?</p><button type="button" onclick={() => void remove()}>Да, удалить запись</button><button class="secondary" type="button" onclick={() => { confirming = false; }}>Отмена</button></div>
        {:else}<button class="secondary spaced" type="button" onclick={() => { confirming = true; }}>Удалить запись дня</button>{/if}
      {/if}
    </fieldset>
  </form>
</section>
