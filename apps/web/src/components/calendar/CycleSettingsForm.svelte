<script lang="ts">
  import { untrack } from 'svelte';
  import { contexts } from '../../lib/calendar';
  import type { CycleSettings } from '../../lib/calendar';
  let { settings, disabled = false, onsave }: { settings: CycleSettings; disabled?: boolean; onsave: (input: CycleSettings) => Promise<void> } = $props();
  const initial = untrack(() => settings);
  let cycleLength = $state<number | undefined>(initial.cycle_length ?? undefined);
  let periodLength = $state(initial.period_length);
  let context = $state<CycleSettings['context']>(initial.context);
  let method = $state<CycleSettings['length_method']>(initial.length_method);
  let busy = $state(false);
  async function submit(event: SubmitEvent) {
    event.preventDefault(); if (busy || disabled) return; busy = true;
    try { await onsave({ cycle_length: cycleLength ?? null, period_length: periodLength, context, length_method: method }); }
    finally { busy = false; }
  }
</script>
<details class="calendar-details">
  <summary>Настройки расчёта</summary>
  <form onsubmit={submit} aria-label="Настройки расчёта цикла">
    <fieldset disabled={disabled || busy}>
      <legend class="visually-hidden">Длительность и режим цикла</legend>
      <div class="field-grid">
        <div class="field"><label for="cycle-length">Привычная длина цикла, дней</label><input id="cycle-length" name="cycle_length" type="number" min="10" max="90" step="1" bind:value={cycleLength} placeholder="Например, 28" required={method === 'manual'} /><span class="detail">От первого дня одних месячных до первого дня следующих. Можно оставить пустым при достаточной истории.</span></div>
        <div class="field"><label for="period-length">Привычная длительность месячных, дней</label><input id="period-length" name="period_length" type="number" min="1" max="14" step="1" bind:value={periodLength} required /></div>
      </div>
      <fieldset class="choice-group">
        <legend>Как выбирать длительность</legend>
        <label><input type="radio" name="length_method" value="auto" bind:group={method} />По истории, при её нехватке — по моему числу</label>
        <label><input type="radio" name="length_method" value="manual" bind:group={method} />Использовать моё число</label>
      </fieldset>
      <div class="field"><label for="cycle-context">Режим цикла</label><select id="cycle-context" name="context" bind:value={context}>{#each contexts as [value, label]}<option {value}>{label}</option>{/each}</select></div>
      <p class="detail">При нерегулярности, беременности, после родов и гормональной контрацепции календарные прогнозы отключены. Эти настройки партнёру не передаются.</p>
      <button class="spaced" type="submit">{busy ? 'Сохраняем…' : 'Сохранить настройки'}</button>
    </fieldset>
  </form>
</details>
