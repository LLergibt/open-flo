<script lang="ts">
  import { onMount } from 'svelte';
  import CalendarWorkspace from './components/CalendarWorkspace.svelte';
  import AdminPanel from './components/AdminPanel.svelte';
  import LoginForm from './components/LoginForm.svelte';
  import MemberPanel from './components/MemberPanel.svelte';
  import DeleteAccount from './components/DeleteAccount.svelte';
  import PasswordForm from './components/PasswordForm.svelte';
  import InviteFlow from './components/InviteFlow.svelte';
  import { consumeInviteFragment } from './lib/invites';
  import { api, ApiError, errorMessage, onSessionExpired } from './lib/api';
  import { forgetCalendar, recallCalendar, unavailable, type CalendarSnapshot } from './lib/offline-calendar';
  import { roleNames } from './lib/types';
  import type { User } from './lib/types';

  let pairRevision = $state(0);
  let user = $state<User | null>(null);
  let pendingInvite = $state<string | null>(null);
  let phase = $state<'loading' | 'ready' | 'error' | 'offline'>('loading');
  let offlineSnapshot = $state<CalendarSnapshot | null>(null);
  let needsSetup = $state(false);
  let restoring = $state(false);
  let loggingOut = $state(false);
  let retryLogout = $state(false);
  let error = $state('');
  let notice = $state('');
  let mounted = false;
  let generation = 0;
  const logoutKey = 'flo-local-logout-pending';

  function rememberPendingLogout(pending: boolean) {
    try {
      if (pending) localStorage.setItem(logoutKey, '1');
      else localStorage.removeItem(logoutKey);
    } catch { /* Browser storage may be unavailable. */ }
  }

  function signedOut(message: string) {
    generation += 1;
    user = null;
    offlineSnapshot = null;
    void forgetCalendar();
    phase = 'ready';
    notice = message;
  }

  async function restore() {
    if (restoring || retryLogout || loggingOut) return;
    restoring = true;
    phase = 'loading';
    error = '';
    const current = ++generation;
    try {
      const setup = await api.setupStatus();
      if (!mounted || current !== generation) return;
      needsSetup = setup;
      if (setup) { user = null; void forgetCalendar(); }
      if (!setup) {
        try {
          const restored = await api.me();
          if (!mounted || current !== generation) return;
          user = restored;
        } catch (failure) {
          if (!(failure instanceof ApiError) || failure.status !== 401) throw failure;
          if (!mounted || current !== generation) return;
          user = null;
          void forgetCalendar();
        }
      }
      if (mounted && current === generation) { offlineSnapshot = null; phase = 'ready'; }
    } catch (failure) {
      if (mounted && current === generation) {
        const snapshot = !retryLogout && unavailable(failure) ? await recallCalendar(user ?? undefined) : null;
        if (!mounted || current !== generation) return;
        offlineSnapshot = snapshot;
        error = errorMessage(failure);
        phase = snapshot ? 'offline' : 'error';
      }
    } finally {
      restoring = false;
    }
  }

  function loggedIn(account: User) {
    generation += 1;
    offlineSnapshot = null;
    void forgetCalendar();
    user = account;
    notice = '';
    retryLogout = false;
    error = '';
    phase = 'ready';
  }

  function inviteAccepted(account: User) {
    pendingInvite = null;
    loggedIn(account);
    notice = 'Приглашение принято.';
  }

  function inviteRegistered(message: string) {
    pendingInvite = null;
    signedOut(message);
  }

  async function logout() {
    if (loggingOut) return;
    loggingOut = true;
    retryLogout = true;
    rememberPendingLogout(true);
    signedOut('');
    await forgetCalendar();
    try {
      await api.logout();
      rememberPendingLogout(false);
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
    const invitation = consumeInviteFragment(window.location, window.history);
    pendingInvite = invitation.token;
    if (invitation.invalid) notice = 'Ссылка приглашения имеет неправильный формат. Попросите владельца создать новую.';
    const unsubscribe = onSessionExpired(() => {
      if (user) signedOut('Сессия завершена. Войдите снова.');
    });
    try { retryLogout = localStorage.getItem(logoutKey) === '1'; } catch { /* Storage may be unavailable. */ }
    if (retryLogout) signedOut('Подтвердите выход после восстановления подключения.');
    else void restore();
    const logoutChanged = (event: StorageEvent) => {
      if (event.key !== logoutKey) return;
      retryLogout = event.newValue === '1';
      if (retryLogout) signedOut('Подтвердите выход после восстановления подключения.');
    };
    window.addEventListener('storage', logoutChanged);
    const reconnect = () => { if (phase === 'offline' || phase === 'error') void restore(); };
    const disconnect = () => { if (phase === 'ready' && user && !loggingOut) void restore(); };
    window.addEventListener('online', reconnect);
    window.addEventListener('offline', disconnect);
    return () => {
      window.removeEventListener('storage', logoutChanged);
      window.removeEventListener('online', reconnect);
      window.removeEventListener('offline', disconnect);
      mounted = false;
      generation += 1;
      unsubscribe();
    };
  });
</script>

<svelte:head><title>flo / local · {user ? 'Мой аккаунт' : 'Вход'}</title></svelte:head>

<main>
  <header class="page-header">
    <a class="wordmark" href="/" aria-label="open-source-flo, главная">flo/local</a>
    <span class="build-label">КАЛЕНДАРЬ · 002</span>
  </header>

  {#if notice}<p class="feedback top-notice" class:error={retryLogout} role="status">{notice}</p>{/if}
  {#if retryLogout}<button class="secondary spaced-bottom" type="button" onclick={() => void logout()} disabled={loggingOut}>{loggingOut ? 'Выходим…' : 'Повторить выход'}</button>{/if}

  {#if phase === 'ready' && !pendingInvite && user && user.role !== 'admin'}
    {#key user.id + ':' + pairRevision}<CalendarWorkspace {user} disabled={loggingOut} />{/key}
  {/if}

  {#if phase === 'offline' && offlineSnapshot}
    <CalendarWorkspace user={offlineSnapshot.account} offlineData={offlineSnapshot.data} savedAt={offlineSnapshot.savedAt} onreconnect={restore} />
  {/if}

  <section class="window" aria-labelledby="screen-title">
    <div class="titlebar" aria-hidden="true"><span>{pendingInvite ? 'invite.exe' : user ? 'profile.exe' : 'auth.exe'}</span><span class="window-controls">─ □ ×</span></div>
    <div class="window-body">
      <p class="eyebrow">СВОЙ СЕРВЕР · СВОИ ДАННЫЕ</p>
      {#if phase === 'loading'}
        <h1 id="screen-title">Проверяем сессию</h1>
        <p class="intro" role="status">Подключаемся к вашему серверу…</p>
      {:else if phase === 'offline'}
        <h1 id="screen-title">Локальная копия</h1>
        <button class="secondary small" type="button" onclick={() => void logout()} disabled={loggingOut}>Выйти и удалить копию</button>
      {:else if phase === 'error'}
        <h1 id="screen-title">Сервер недоступен</h1>
        <p class="feedback error" role="alert">{error}</p>
        <button type="button" onclick={() => void restore()} disabled={restoring}>Повторить подключение</button>
      {:else if needsSetup}
        <h1 id="screen-title">Настройка сервера</h1>
        <p class="intro">Первый администратор ещё не создан. Выполните команду на компьютере, где запущен сервер.</p>
        <div class="message-panel setup-panel">
          <p class="panel-label">В КОРНЕ ПРОЕКТА</p>
          <pre class="command"><code>./scripts/create-admin.sh</code></pre>
          <p class="detail">Пароль вводится скрыто в терминале. Аккаунт создаётся локально, без стандартного пароля.</p>
        </div>
        <details class="help"><summary>Команда без вспомогательного скрипта</summary><pre class="command"><code>cargo run --locked -p flo-api -- create-admin --username root --display-name root</code></pre></details>
        <button type="button" onclick={() => void restore()} disabled={restoring}>Проверить настройку</button>
      {:else if pendingInvite}
        <InviteFlow token={pendingInvite} {user} disabled={loggingOut || retryLogout} onlogin={loggedIn} onaccepted={inviteAccepted} onregistered={inviteRegistered} oncancel={() => { pendingInvite = null; }} onlogout={() => void logout()} />
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
          {#key user.id}<MemberPanel {user} disabled={loggingOut} onpairchange={() => { void forgetCalendar(); pairRevision += 1; }} />{/key}
        {/if}
        <PasswordForm disabled={loggingOut} onchanged={() => signedOut('Пароль изменён. Все ваши сессии завершены. Войдите заново с новым паролем.')} />
        <DeleteAccount disabled={loggingOut} onremoved={() => { pendingInvite = null; signedOut('Аккаунт удалён.'); }} />
      {:else}
        <h1 id="screen-title">Вход в аккаунт</h1>
        <p class="intro">Введите логин и пароль, которые выдал администратор вашего сервера.</p>
        <div class="spaced"><LoginForm onlogin={loggedIn} disabled={loggingOut || retryLogout} /></div>
        <details class="help"><summary>Помощь администратору</summary><p class="detail">Первый аккаунт создаётся в терминале на сервере командой ниже. Другие аккаунты создаются после входа в админке.</p><pre class="command"><code>./scripts/create-admin.sh</code></pre></details>
      {/if}
    </div>
    <footer class="window-footer"><span>open-source-flo</span><span>web / local</span></footer>
  </section>

  {#if phase === 'ready' && !pendingInvite && user?.role === 'admin'}
    {#key user.id}<AdminPanel {user} disabled={loggingOut} onselfpasswordchanged={() => signedOut('Пароль изменён. Войдите заново с новым паролем.')} />{/key}
  {/if}
  <p class="footnote">Календарь и личный дневник работают на вашем сервере. Календарные прогнозы приблизительны.</p>
</main>
