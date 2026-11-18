<script lang="ts">
  import { diaryCategories, flows, moods } from '../../lib/calendar';
  import type { DiaryEntry } from '../../lib/calendar';
  import { dateTitle } from '../../lib/dates';

  let { date, entry, owner = false }: { date: string; entry?: DiaryEntry; owner?: boolean } = $props();

  const painLabels = ['Не отмечена', 'Слабая', 'Умеренная', 'Сильная'];
  function labels(options: readonly (readonly [string, string])[], values: string[]): string {
    return options.filter(([id]) => values.includes(id)).map(([, label]) => label).join(', ');
  }
</script>

<section class="diary-editor" aria-labelledby="shared-diary-title">
  <p class="eyebrow">{owner ? "ДНЕВНИК" : "ДНЕВНИК · ПРОСМОТР"}</p>
  <h2 id="shared-diary-title">{dateTitle(date)}</h2>
  {#if entry}
    <dl>
      <div><dt>Боль</dt><dd>{painLabels[entry.pain] ?? 'Не отмечена'}</dd></div>
      <div><dt>Интенсивность кровотечения</dt><dd>{labels(flows, [entry.flow])}</dd></div>
      <div><dt>Настроение</dt><dd>{labels(moods, [entry.mood])}</dd></div>
      {#each diaryCategories as category (category.title)}
        {@const selected = labels(category.options, entry[category.field])}
        {#if selected}<div><dt>{category.title}</dt><dd>{selected}</dd></div>{/if}
      {/each}
      {#if entry.notes}<div><dt>Заметка</dt><dd class="notes">{entry.notes}</dd></div>{/if}
    </dl>
  {:else}
    <p class="detail">За этот день записей нет.</p>
  {/if}
</section>

<style>
  dl { display: grid; gap: 12px; }
  dt { font-weight: 650; }
  dd { margin: 4px 0 0; overflow-wrap: anywhere; }
  .notes { white-space: pre-wrap; }
</style>
