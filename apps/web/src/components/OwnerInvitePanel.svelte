<script lang="ts">
  import { api, errorMessage } from '../lib/api';
  import { expiryLabel, inviteLink } from '../lib/invites';
  import type { Invite } from '../lib/types';

  let { disabled = false }: { disabled?: boolean } = $props();
  let invitation = $state<Invite | null>(null);
  let link = $state('');
  let busy = $state(false);
  let error = $state('');
  let notice = $state('');
  let linkInput = $state<HTMLInputElement>();

  async function create() {
    if (busy || disabled) return;
    busy = true;
    error = '';
    notice = '';
    invitation = null;
    link = '';
    try {
      const issued = await api.createInvite();
      link = inviteLink(issued, window.location.origin);
      invitation = issued;
      notice = 'Приглашение создано. Предыдущая неиспользованная ссылка отозвана.';
    } catch (failure) {
      error = errorMessage(failure);
    } finally { busy = false; }
  }

  async function revoke() {
    if (busy || disabled) return;
    busy = true;
    error = '';
    notice = '';
    try {
      await api.revokeInvite();
      invitation = null;
      link = '';
      notice = 'Неиспользованные приглашения отозваны.';
    } catch (failure) {
      error = errorMessage(failure);
    } finally { busy = false; }
  }

  async function copy() {
    if (busy || disabled || !link) return;
    error = '';
    notice = '';
    try {
      if (!navigator.clipboard?.writeText) throw new Error('clipboard_unavailable');
      await navigator.clipboard.writeText(link);
      notice = 'Ссылка скопирована.';
    } catch {
      linkInput?.focus();
      linkInput?.select();
      notice = 'Автоматическое копирование недоступно. Скопируйте выделенную ссылку из поля.';
    }
  }
</script>

<section class="section-block" aria-labelledby="invite-owner-title" aria-busy={busy}>
  <h2 id="invite-owner-title">Пригласить партнёра</h2>
  <p class="detail">Одноразовая ссылка действует 24 часа. Передайте её тому, с кем хотите связать аккаунт. Партнёр сможет создать аккаунт по приглашению или войти в существующий.</p>
  {#if error}<p class="feedback error spaced" role="alert">{error}</p>{/if}
  {#if notice}<p class="feedback spaced" role="status">{notice}</p>{/if}
  {#if invitation && link}
    <div class="field spaced"><label for="owner-invite-link">Одноразовая ссылка</label><input id="owner-invite-link" bind:this={linkInput} value={link} readonly autocomplete="off" spellcheck="false" /></div>
    <p class="field-hint">Действует до {expiryLabel(invitation.expires_at)}. После обновления страницы ссылка не сохраняется; при необходимости создайте новую.</p>
    <button class="secondary small" type="button" onclick={() => void copy()} disabled={busy || disabled}>Скопировать ссылку</button>
  {/if}
  <div class="actions compact">
    <button type="button" onclick={() => void create()} disabled={busy || disabled}>{busy ? 'Выполняем…' : invitation ? 'Создать новую ссылку' : 'Создать приглашение'}</button>
    <button class="secondary" type="button" onclick={() => void revoke()} disabled={busy || disabled}>Отозвать приглашения</button>
  </div>
</section>
