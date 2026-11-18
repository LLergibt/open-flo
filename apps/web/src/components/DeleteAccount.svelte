<script lang="ts">
  import PasswordField from "./PasswordField.svelte";
  import { api, ApiError, errorMessage } from '../lib/api';
  let { onremoved, disabled = false }: { onremoved: () => void; disabled?: boolean } = $props();
  let password = $state('');
  let busy = $state(false);
  let error = $state('');
  async function remove(event: SubmitEvent) {
    event.preventDefault();
    if (busy || disabled) return;
    busy = true; error = '';
    const value = password; password = '';
    try { await api.deleteSelf(value); onremoved(); }
    catch (failure) { error = failure instanceof ApiError && failure.code === 'conflict' ? 'Нельзя удалить последнего активного администратора. Сначала создайте другого.' : failure instanceof ApiError && failure.code === 'invalid_credentials' ? 'Неверный текущий пароль.' : errorMessage(failure); }
    finally { busy = false; }
  }
</script>
<section class="section-block">
  <details class="help"><summary>Удалить мой аккаунт</summary>
    <p class="detail">Аккаунт будет удалён без возможности отмены. Сессии, приглашения и связь с партнёром будут удалены. Аккаунт партнёра сохранится.</p>
    <form class="spaced" onsubmit={remove}><fieldset disabled={busy || disabled}>
      <PasswordField label="Текущий пароль" id="delete-password"  autocomplete="current-password" bind:value={password} required maxlength={128} />
      <label class="checkbox-field"><input type="checkbox" required /><span>Подтверждаю удаление моего аккаунта.</span></label>
      {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
      <button type="submit">Удалить аккаунт навсегда</button>
    </fieldset></form>
  </details>
</section>
