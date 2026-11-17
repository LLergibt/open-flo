<script lang="ts">
  import { onMount } from 'svelte';
  import AdminPanel from './components/AdminPanel.svelte';
  import LoginForm from './components/LoginForm.svelte';
  import MemberPanel from './components/MemberPanel.svelte';
  import { api, ApiError, errorMessage, onSessionExpired } from './lib/api';
  import { roleNames } from './lib/types';
  import type { User } from './lib/types';

  let user = $state<User | null>(null);
  let phase = $state<'loading' | 'ready' | 'error'>('loading');
  let needsSetup = $state(false);
  let restoring = $state(false);
  let loggingOut = $state(false);
  let retryLogout = $state(false);
  let error = $state('');
  let notice = $state('');
  let mounted = false;
  let generation = 0;

  function signedOut(message: string) {
    generation += 1;
    user = null;
    phase = 'ready';
    notice = message;
  }

  async function restore() {
    if (restoring) return;
    restoring = true;
    phase = 'loading';
    error = '';
    const current = ++generation;
    try {
      const setup = await api.setupStatus();
      if (!mounted || current !== generation) return;
      needsSetup = setup;
      if (!setup) {
        try {
          const restored = await api.me();
          if (!mounted || current !== generation) return;
          user = restored;
        } catch (failure) {
          if (!(failure instanceof ApiError) || failure.status !== 401) throw failure;
          if (!mounted || current !== generation) return;
          user = null;
        }
      }
      if (mounted && current === generation) phase = 'ready';
    } catch (failure) {
      if (mounted && current === generation) {
        error = errorMessage(failure);
        phase = 'error';
      }
    } finally {
      restoring = false;
    }
  }

  function loggedIn(account: User) {
    generation += 1;
    user = account;
    notice = '';
    retryLogout = false;
    error = '';
    phase = 'ready';
  }

  async function logout() {
    if (loggingOut) return;
    loggingOut = true;
    notice = '';
    try {
      await api.logout();
      retryLogout = false;
      signedOut('Вы вышли из аккаунта.');
    } catch (failure) {
      retryLogout = true;
      signedOut(`Аккаунт скрыт на этом экране, но выход на сервере не подтверждён. ${errorMessage(failure)} Повторите выход перед сменой пользователя.`);
    } finally {
      loggingOut = false;
    }
  }

  onMount(() => {
    mounted = true;
    const unsubscribe = onSessionExpired(() => {
      if (user) signedOut('Сессия завершена. Войдите снова.');
    });
    void restore();
    return () => {
      mounted = false;
      generation += 1;
      unsubscribe();
    };
  });
</script>

<svelte:head><title>flo / local · {user ? 'Мой аккаунт' : 'Вход'}</title></svelte:head>

<main>
  <header class="page-header">
    <a class="wordmark" href="/" aria-label="open-source-flo, главная">flo<span aria-hidden="true"> / </span>local</a>
    <span class="build-label">АВТОРИЗАЦИЯ · 001</span>
  </header>

  {#if notice}<p class="feedback top-notice" class:error={retryLogout} role="status">{notice}</p>{/if}
  {#if retryLogout}<button class="secondary spaced-bottom" type="button" onclick={() => void logout()} disabled={loggingOut}>{loggingOut ? 'Выходим…' : 'Повторить выход'}</button>{/if}

  <section class="window" aria-labelledby="screen-title">
    <div class="titlebar" aria-hidden="true"><span>{user ? 'profile.exe' : 'auth.exe'}</span><span class="window-controls">─ □ ×</span></div>
    <div class="window-body">
      <p class="eyebrow">СВОЙ СЕРВЕР · СВОИ ДАННЫЕ</p>
      {#if phase === 'loading'}
        <h1 id="screen-title">Проверяем сессию</h1>
        <p class="intro" role="status">Подключаемся к вашему серверу…</p>
      {:else if phase === 'error'}
        <h1 id="screen-title">Сервер недоступен</h1>
        <p class="feedback error" role="alert">{error}</p>
        <button type="button" onclick={() => void restore()} disabled={restoring}>Повторить подключение</button>
      {:else if needsSetup}
        <h1 id="screen-title">Настройка сервера</h1>
        <p class="intro">Первый администратор ещё не создан. Выполните команду на компьютере, где запущен сервер.</p>
        <div class="message-panel setup-panel">
          <p class="panel-label">В КОРНЕ ПРОЕКТА</p>
          <pre class="command"><code>./scripts/create-admin.sh admin</code></pre>
          <p class="detail">Пароль вводится скрыто в терминале. Аккаунт создаётся локально, без стандартного пароля.</p>
        </div>
        <details class="help"><summary>Команда без вспомогательного скрипта</summary><pre class="command"><code>cargo run --locked -p flo-api -- create-admin --username admin --display-name 'Администратор'</code></pre></details>
        <button type="button" onclick={() => void restore()} disabled={restoring}>Проверить настройку</button>
      {:else if user}
        <div class="profile-heading">
          <h1 id="screen-title">Мой аккаунт</h1>
          <button class="secondary small" type="button" onclick={() => void logout()} disabled={loggingOut}>{loggingOut ? 'Выходим…' : 'Выйти'}</button>
        </div>
        <div class="profile-data">
          <p class="profile-name">{user.display_name}</p>
          <p class="detail">@{user.username}</p>
          <span class="tag">{roleNames[user.role]}</span>
        </div>
        {#if user.role === 'admin'}
          <p class="detail">Создавайте аккаунты и связывайте владельца дневника с партнёром в панели ниже.</p>
        {:else}
          {#key user.id}<MemberPanel {user} disabled={loggingOut} />{/key}
        {/if}
      {:else}
        <h1 id="screen-title">Вход в аккаунт</h1>
        <p class="intro">Введите логин и пароль, которые выдал администратор вашего сервера.</p>
        <div class="spaced"><LoginForm onlogin={loggedIn} disabled={loggingOut || retryLogout} /></div>
        <details class="help"><summary>Помощь администратору</summary><p class="detail">Первый аккаунт создаётся в терминале на сервере командой ниже. Другие аккаунты создаются после входа в админке.</p><pre class="command"><code>./scripts/create-admin.sh admin</code></pre></details>
      {/if}
    </div>
    <footer class="window-footer"><span>open-source-flo</span><span>web / local</span></footer>
  </section>

  {#if phase === 'ready' && user?.role === 'admin'}
    {#key user.id}<AdminPanel {user} disabled={loggingOut} onselfpasswordchanged={() => signedOut('Пароль изменён. Войдите заново с новым паролем.')} />{/key}
  {/if}
  <p class="footnote">Аккаунты и пары работают на вашем сервере. Дневник и расчёты цикла появятся на следующем этапе.</p>
</main>
