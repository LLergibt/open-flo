<script lang="ts">
  import { onMount } from 'svelte';
  import LoginForm from './LoginForm.svelte';
  import { api, errorMessage } from '../lib/api';
  import { expiryLabel } from '../lib/invites';
  import { roleNames } from '../lib/types';
  import type { InvitePreview, User } from '../lib/types';

  let { token, user, disabled = false, onlogin, onaccepted, onregistered, oncancel, onlogout }: {
    token: string;
    user: User | null;
    disabled?: boolean;
    onlogin: (account: User) => void;
    onaccepted: (account: User) => void;
    onregistered: (message: string) => void;
    oncancel: () => void;
    onlogout: () => void;
  } = $props();
  let preview = $state<InvitePreview | null>(null);
  let loading = $state(false);
  let busy = $state(false);
  let mode = $state<'signup' | 'login'>('signup');
  let error = $state('');
  let username = $state('');
  let displayName = $state('');
  let password = $state('');
  let confirmation = $state('');
  let consent = $state(false);
  const usernamePattern = '[a-z0-9_.\\-]{3,64}';
  const locked = $derived(disabled || loading || busy);

  async function load() {
    if (locked) return;
    loading = true;
    error = '';
    preview = null;
    try { preview = await api.previewInvite(token); } catch (failure) { error = errorMessage(failure); }
    finally { loading = false; }
  }

  async function register(event: SubmitEvent) {
    event.preventDefault();
    if (locked || !preview || !consent) return;
    error = '';
    if (password !== confirmation) { error = 'Пароли не совпадают.'; return; }
    busy = true;
    let submittedPassword = password;
    password = '';
    confirmation = '';
    let registered = false;
    try {
      const result = await api.registerInvite({ token, username, display_name: displayName, password: submittedPassword });
      registered = true;
      const account = await api.login(result.user.username, submittedPassword);
      onaccepted(account);
    } catch (failure) {
      if (registered) {
        onregistered('Аккаунт создан, приглашение принято. Войдите с выбранными логином и паролем. ' + errorMessage(failure));
      } else {
        error = errorMessage(failure);
      }
    } finally {
      submittedPassword = '';
      busy = false;
    }
  }

  async function accept() {
    if (locked || !preview || !user || user.role !== 'partner' || !consent) return;
    busy = true;
    error = '';
    try {
      await api.acceptInvite(token);
      onaccepted(user);
    } catch (failure) { error = errorMessage(failure); }
    finally { busy = false; }
  }

  function selectMode(next: 'signup' | 'login') {
    if (locked) return;
    mode = next;
    password = '';
    confirmation = '';
    consent = false;
    error = '';
  }

  onMount(() => { void load(); });
</script>

<h1 id="screen-title">Приглашение партнёра</h1>
{#if error}<p class="feedback error" role="alert">{error}</p>{/if}
{#if loading}
  <p class="intro" role="status">Проверяем приглашение…</p>
{:else if !preview}
  <p class="intro">Приглашение пока не подтверждено. Ссылка могла истечь, быть отозвана или уже использована.</p>
  <div class="actions"><button type="button" onclick={() => void load()} disabled={locked}>Проверить приглашение</button><button class="secondary" type="button" onclick={oncancel} disabled={locked}>Закрыть приглашение</button></div>
{:else}
  <div class="message-panel invite-preview">
    <p class="panel-label">ВАС ПРИГЛАШАЕТ</p>
    <p class="message">{preview.tracker_display_name}</p>
    <p class="detail">Действует до {expiryLabel(preview.expires_at)}.</p>
  </div>
  <p class="detail">Вы станете партнёром владельца дневника и сможете просматривать доступные вам записи после появления дневника. Создание пары требует вашего подтверждения.</p>
  {#if user}
    <p class="feedback spaced">Вы вошли как {user.display_name} (@{user.username}) · {roleNames[user.role]}.</p>
    {#if user.role === 'partner'}
      <label class="checkbox-field"><input type="checkbox" bind:checked={consent} disabled={locked} /><span>Я хочу связать этот аккаунт с {preview.tracker_display_name}.</span></label>
      <div class="actions"><button type="button" onclick={() => void accept()} disabled={locked || !consent}>{busy ? 'Принимаем…' : 'Подтвердить приглашение'}</button><button class="secondary" type="button" onclick={onlogout} disabled={locked}>Войти в другой аккаунт</button></div>
    {:else}
      <p class="detail">Принять приглашение может только аккаунт с ролью «Партнёр». Выйдите и войдите в подходящий аккаунт или создайте новый по этой ссылке.</p>
      <button class="secondary spaced" type="button" onclick={onlogout} disabled={locked}>Выйти и продолжить приглашение</button>
    {/if}
  {:else}
    <nav class="tabs" aria-label="Способ принять приглашение"><button type="button" class:current={mode === 'signup'} onclick={() => selectMode('signup')} disabled={locked}>Создать аккаунт</button><button type="button" class:current={mode === 'login'} onclick={() => selectMode('login')} disabled={locked}>У меня есть аккаунт</button></nav>
    {#if mode === 'login'}
      <p class="detail">Войдите в аккаунт партнёра. После входа отдельно подтвердите принятие приглашения.</p>
      <div class="spaced"><LoginForm {onlogin} disabled={locked} /></div>
    {:else}
      <form class="spaced" onsubmit={register} aria-label="Регистрация партнёра по приглашению" aria-busy={busy}>
        <fieldset disabled={locked}>
          <div class="field"><label for="invite-username">Логин</label><input id="invite-username" bind:value={username} autocomplete="username" autocapitalize="none" spellcheck="false" required minlength="3" maxlength="64" pattern={usernamePattern} /></div>
          <div class="field"><label for="invite-display-name">Отображаемое имя</label><input id="invite-display-name" bind:value={displayName} autocomplete="name" required maxlength="80" /></div>
          <div class="field"><label for="invite-password">Пароль</label><input id="invite-password" type="password" bind:value={password} autocomplete="new-password" required minlength="12" maxlength="128" /></div>
          <div class="field"><label for="invite-confirm-password">Повторите пароль</label><input id="invite-confirm-password" type="password" bind:value={confirmation} autocomplete="new-password" required minlength="12" maxlength="128" /></div>
          <p class="field-hint">Роль нового аккаунта — «Партнёр». Пароль: 12–128 символов.</p>
          <label class="checkbox-field"><input type="checkbox" bind:checked={consent} required /><span>Я хочу создать аккаунт партнёра и связать его с {preview.tracker_display_name}.</span></label>
          <button type="submit" disabled={!consent}>{busy ? 'Создаём аккаунт…' : 'Создать аккаунт и принять приглашение'}</button>
        </fieldset>
      </form>
    {/if}
  {/if}
  <button class="secondary small spaced" type="button" onclick={oncancel} disabled={locked}>Отменить приглашение</button>
{/if}
