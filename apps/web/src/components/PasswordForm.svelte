<script lang="ts">
  import PasswordField from "./PasswordField.svelte";
  import { api, ApiError, errorMessage } from '../lib/api';

  let { disabled = false, onchanged }: { disabled?: boolean; onchanged: () => void } = $props();
  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmation = $state('');
  let busy = $state(false);
  let error = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || disabled) return;
    error = '';
    if (newPassword !== confirmation) { error = 'Новые пароли не совпадают.'; return; }
    if (newPassword === currentPassword) { error = 'Новый пароль должен отличаться от текущего.'; return; }
    busy = true;
    let current = currentPassword;
    let next = newPassword;
    currentPassword = '';
    newPassword = '';
    confirmation = '';
    try {
      await api.changePassword(current, next);
      onchanged();
    } catch (failure) {
      error = failure instanceof ApiError && failure.code === 'invalid_credentials' ? 'Неверный текущий пароль.' : errorMessage(failure);
    } finally {
      current = '';
      next = '';
      busy = false;
    }
  }
</script>

<section class="section-block" aria-labelledby="password-title">
  <h2 id="password-title">Свой пароль</h2>
  <details class="help">
    <summary>Сменить свой пароль</summary>
    <p class="detail">После смены пароля все ваши сессии будут завершены. Войдите заново с новым паролем.</p>
    <form class="spaced" onsubmit={submit} aria-label="Сменить свой пароль" aria-busy={busy}>
      <fieldset disabled={busy || disabled}>
        <PasswordField label="Текущий пароль" id="current-password" name="current-password"  bind:value={currentPassword} autocomplete="current-password" required maxlength={128} />
        <PasswordField label="Новый пароль" id="self-new-password" name="self-new-password"  bind:value={newPassword} autocomplete="new-password" required minlength={12} maxlength={128} />
        <PasswordField label="Повторите новый пароль" id="self-confirm-password" name="self-confirm-password"  bind:value={confirmation} autocomplete="new-password" required minlength={12} maxlength={128} />
        <p class="field-hint">12–128 символов. Поля очищаются при отправке.</p>
        {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
        <button type="submit">{busy ? 'Сохраняем…' : 'Сохранить свой пароль'}</button>
      </fieldset>
    </form>
  </details>
</section>
