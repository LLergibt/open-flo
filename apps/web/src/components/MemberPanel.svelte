<script lang="ts">
  import { onMount } from 'svelte';
  import OwnerInvitePanel from './OwnerInvitePanel.svelte';
  import { api, errorMessage } from '../lib/api';
  import type { Pair, User } from '../lib/types';
  import { roleNames } from '../lib/types';

  let { user, disabled = false }: { user: User; disabled?: boolean } = $props();
  let pair = $state<Pair | null>(null);
  let loaded = $state(false);
  let busy = $state(false);
  let confirming = $state(false);
  let error = $state('');
  let notice = $state('');
  const other = $derived(pair ? (pair.tracker.id === user.id ? pair.partner : pair.tracker) : null);

  async function load() {
    if (busy || disabled) return;
    busy = true;
    error = '';
    try {
      pair = await api.myPair();
      loaded = true;
      confirming = false;
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = false;
    }
  }

  async function unlink() {
    if (busy || disabled || !pair) return;
    busy = true;
    error = '';
    notice = '';
    try {
      await api.leavePair();
      pair = null;
      confirming = false;
      notice = 'Связь прекращена. Новые данные больше не доступны через эту пару.';
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = false;
    }
  }

  onMount(() => { void load(); });
</script>

<section class="section-block" aria-labelledby="pair-title" aria-busy={busy}>
  <div class="section-heading">
    <h2 id="pair-title">Связанный аккаунт</h2>
    <button class="secondary small" type="button" onclick={() => void load()} disabled={busy || disabled} aria-label="Обновить информацию о паре">{busy ? 'Загрузка…' : 'Обновить'}</button>
  </div>
  {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
  {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
  {#if !loaded}
    <p class="detail" role="status">{busy ? 'Получаем информацию о паре…' : 'Информация о паре пока не загружена.'}</p>
  {:else if pair && other}
    <div class="record-card">
      <div class="record-heading">
        <h3>{other.display_name}</h3>
        <span class="tag">{roleNames[other.role]}</span>
      </div>
      <p class="detail">@{other.username} · {other.is_active ? 'Аккаунт активен' : 'Аккаунт отключён'}</p>
      <p class="detail">Связь активна. Дневник и просмотр симптомов появятся на следующем этапе.</p>
      {#if confirming}
        <div class="confirmation">
          <p>Прекратить связь с {other.display_name}? Новые данные больше не будут доступны партнёру.</p>
          <p class="detail">Уже сохранённые вне приложения копии отозвать нельзя.</p>
          <div class="actions compact">
            <button type="button" onclick={() => void unlink()} disabled={busy || disabled}>{busy ? 'Прекращаем…' : 'Да, прекратить связь'}</button>
            <button class="secondary" type="button" onclick={() => { confirming = false; }} disabled={busy || disabled}>Отмена</button>
          </div>
        </div>
      {:else}
        <button class="secondary small spaced" type="button" onclick={() => { confirming = true; }} disabled={busy || disabled}>Прекратить связь</button>
      {/if}
    </div>
  {:else}
    <div class="record-card empty-state">
      <h3>Активной пары нет</h3>
      <p class="detail">{user.role === 'tracker' ? 'Пригласите партнёра по одноразовой ссылке или попросите администратора связать аккаунты.' : 'Откройте ссылку от владельца дневника или попросите администратора связать аккаунты.'}</p>
    </div>
  {/if}
</section>

{#if loaded && !pair && user.role === 'tracker'}
  <OwnerInvitePanel disabled={disabled || busy} />
{/if}
