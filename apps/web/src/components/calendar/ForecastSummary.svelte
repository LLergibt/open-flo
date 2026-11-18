<script lang="ts">
  import type { Prediction } from '../../lib/calendar';
  import { dateTitle } from '../../lib/dates';
  let { prediction: p }: { prediction: Prediction } = $props();
  const statuses: Record<string, string> = {
    estimated: 'Календарный прогноз', insufficient_data: 'Нужно больше данных', paused: 'Прогноз приостановлен',
    irregular: 'Приблизительный прогноз месячных', overdue: 'Ожидаемая дата уже наступила',
  };
</script>
<div class="forecast-summary" aria-live="polite">
  <p class="panel-label">{statuses[p.status] ?? 'Прогноз недоступен'}</p>
  {#if p.next_period_start}<p class="forecast-main">{p.status === 'overdue' ? 'Ожидались с ' : 'Следующие месячные: '}{dateTitle(p.next_period_start)}{#if p.next_period_end} — {dateTitle(p.next_period_end)}{/if}</p>
    <p class="detail">Примерная длительность месячных: {p.period_length} дн.</p>{/if}
  {#if p.ovulation_date}<p class="detail">Предполагаемая овуляция: {dateTitle(p.ovulation_date)}.</p>{/if}
  {#if p.fertile_start && p.fertile_end}<p class="detail">Предполагаемое фертильное окно: {dateTitle(p.fertile_start)} — {dateTitle(p.fertile_end)}.</p>{/if}
  {#if p.cycle_length}<p class="detail">Расчётная длина цикла: {p.cycle_length} дн. · {p.source === 'history' ? 'по отмеченной истории' : p.source === 'baseline' ? 'стартовая оценка' : 'по введённой длительности'}.</p>{/if}
  {#if p.status === 'insufficient_data'}<p class="detail">Добавьте дату начала последних месячных в истории.</p>{/if}
  {#if p.source === 'baseline'}<p class="detail">Пока использованы условные 28 дней. Введите свою длину цикла или добавьте предыдущие месячные для уточнения.</p>{/if}
  {#if p.status === 'paused'}<p class="detail">Календарные оценки отключены для выбранного режима. Можно продолжать отмечать факты.</p>{/if}
  {#if p.status === 'irregular'}<p class="detail">Данных для оценки овуляции и фертильного окна сейчас недостаточно.</p>{/if}
  {#if p.status === 'overdue'}<p class="detail">Если месячные начались, отметьте фактическую дату. Приложение не переносит прогноз автоматически через пропущенный цикл.</p>{/if}
  {#if p.additional_cycles?.some((cycle) => cycle.kind === 'projected')}<p class="detail">В календаре есть прогноз на три цикла вперёд. Чем дальше дата, тем выше неопределённость.</p>{/if}
  <p class="detail forecast-caution">Красные отметки имеют приоритет при пересечении дат. Запас после предполагаемой овуляции учитывает возможный сдвиг даты. Календарь не подходит для контрацепции.</p>
</div>
