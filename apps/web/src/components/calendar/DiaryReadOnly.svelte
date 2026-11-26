<script lang="ts">
  import { diaryCategoriesFor, flows, moods } from '../../lib/calendar';
  import type { DiaryEntry, PartnerDiaryEntry } from '../../lib/calendar';

  let { entry, partner = false }: { entry?: DiaryEntry | PartnerDiaryEntry; partner?: boolean } = $props();
  const id = $props.id();
  const categories = $derived(diaryCategoriesFor(partner));

  const painLabels = ['Не отмечена', 'Слабая', 'Умеренная', 'Сильная'];
  function labels(options: readonly (readonly [string, string])[], values: string[]): string {
    return options.filter(([id]) => values.includes(id)).map(([, label]) => label).join(', ');
  }
</script>

<article class="diary-editor" class:diary-owner={!partner} class:diary-partner={partner} aria-labelledby={id}>
  <h3 {id}>{partner ? 'День партнёра' : 'День владельца дневника'}</h3>
  {#if entry}
    <dl>
      <div><dt>Боль</dt><dd>{painLabels[entry.pain ?? 0] ?? 'Не отмечена'}</dd></div>
      {#if !partner && "flow" in entry}<div><dt>Интенсивность кровотечения</dt><dd>{labels(flows, [entry.flow])}</dd></div>{/if}
      <div><dt>Настроение</dt><dd>{labels(moods, [entry.mood])}</dd></div>
      {#each categories as category (category.title)}
        {@const selected = labels(category.options, entry[category.field] ?? [])}
        {#if selected}<div><dt>{category.title}</dt><dd>{selected}</dd></div>{/if}
      {/each}
      {#if entry.notes}<div><dt>Заметка</dt><dd class="notes">{entry.notes}</dd></div>{/if}
    </dl>
  {:else}
    <p class="detail">За этот день записей нет.</p>
  {/if}
</article>

<style>
  dl { display: grid; gap: 12px; }
  dt { font-weight: 650; }
  dd { margin: 4px 0 0; overflow-wrap: anywhere; }
  .notes { white-space: pre-wrap; }
</style>
