<script lang="ts">
  import PasswordField from "./PasswordField.svelte";
  import { api, errorMessage } from '../lib/api';
  import type { User } from '../lib/types';

  let { onlogin, disabled = false }: { onlogin: (user: User) => void; disabled?: boolean } = $props();
  let username = $state('');
  let password = $state('');
  let submitting = $state(false);
  let error = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (submitting || disabled) return;
    submitting = true;
    error = '';
    const submittedPassword = password;
    password = '';
    try {
      onlogin(await api.login(username, submittedPassword));
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      submitting = false;
    }
  }
</script>

<form onsubmit={submit} aria-label="Вход в аккаунт" aria-busy={submitting}>
  <fieldset disabled={submitting || disabled}>
    <div class="field">
      <label for="login-username">Логин</label>
      <input id="login-username" name="username" bind:value={username} autocomplete="username" autocapitalize="none" spellcheck="false" required maxlength="64" placeholder="your.login" />
    </div>
    <PasswordField label="Пароль" id="login-password" name="password"  bind:value={password} autocomplete="current-password" required maxlength={128} />
    {#if error}<p class="feedback error" role="alert">{error}</p>{/if}
    <div class="actions">
      <button type="submit">{submitting ? 'Входим…' : 'Войти'}</button>
      <span class="status" role="status">{submitting ? 'Проверяем аккаунт' : 'Аккаунт создаёт администратор'}</span>
    </div>
  </fieldset>
</form>
