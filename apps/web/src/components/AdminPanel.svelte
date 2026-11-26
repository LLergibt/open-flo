<script lang="ts">
  import PasswordField from "./PasswordField.svelte";
  import OwnerInvitePanel from './OwnerInvitePanel.svelte';
  import { onMount } from 'svelte';
  import { api, errorMessage } from '../lib/api';
  import { roleNames } from '../lib/types';
  import type { Pair, Role, User } from '../lib/types';

  let { user, disabled = false, onselfpasswordchanged }: {
    user: User;
    disabled?: boolean;
    onselfpasswordchanged: () => void;
  } = $props();
  let tab = $state<'users' | 'pairs'>('users');
  let users = $state<User[]>([]);
  let pairs = $state<Pair[]>([]);
  let loaded = $state(false);
  let loading = $state(false);
  let busy = $state('');
  let error = $state('');
  let notice = $state('');
  let username = $state('');
  let displayName = $state('');
  let password = $state('');
  let role = $state<Role>('tracker');
  let trackerId = $state('');
  let partnerId = $state('');
  let resetId = $state<string | null>(null);
  let resetPassword = $state('');
  let confirmUserId = $state<string | null>(null);
  let deleteId = $state<string | null>(null);
  let confirmPairId = $state<string | null>(null);
  const usernamePattern = '[a-z0-9_.\\-]{3,64}';

  const locked = $derived(disabled || loading || busy !== '');
  const pairedIds = $derived(new Set(pairs.flatMap((pair) => [pair.tracker.id, pair.partner.id])));
  const trackers = $derived(users.filter((account) => account.role === 'tracker' && account.is_active && !pairedIds.has(account.id)));
  const partners = $derived(users.filter((account) => account.role === 'partner' && account.is_active && !pairedIds.has(account.id)));

  async function reload() {
    const [nextUsers, nextPairs] = await Promise.all([api.users(), api.pairs()]);
    users = nextUsers;
    pairs = nextPairs;
    loaded = true;
    if (!nextUsers.some((account) => account.id === trackerId && account.is_active && account.role === 'tracker') || nextPairs.some((pair) => pair.tracker.id === trackerId)) trackerId = '';
    if (!nextUsers.some((account) => account.id === partnerId && account.is_active && account.role === 'partner') || nextPairs.some((pair) => pair.partner.id === partnerId)) partnerId = '';
  }

  async function load() {
    if (locked) return;
    loading = true;
    error = '';
    try {
      await reload();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      loading = false;
    }
  }

  async function createUser(event: SubmitEvent) {
    event.preventDefault();
    if (locked) return;
    busy = 'create-user';
    error = '';
    notice = '';
    const submittedPassword = password;
    password = '';
    try {
      const created = await api.createUser({ username, display_name: displayName, password: submittedPassword, role });
      username = '';
      displayName = '';
      notice = `Аккаунт @${created.username} создан.`;
      await reload();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = '';
    }
  }

  async function toggleActive(account: User) {
    if (locked || account.id === user.id) return;
    busy = `active-${account.id}`;
    error = '';
    notice = '';
    try {
      const updated = await api.updateUser(account.id, { is_active: !account.is_active });
      confirmUserId = null;
      notice = updated.is_active ? `Аккаунт @${updated.username} включён.` : `Аккаунт @${updated.username} отключён. Его сессии отозваны.`;
      await reload();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = '';
    }
  }

  async function changePassword(event: SubmitEvent, account: User) {
    event.preventDefault();
    if (locked) return;
    busy = `password-${account.id}`;
    error = '';
    notice = '';
    const submittedPassword = resetPassword;
    resetPassword = '';
    try {
      await api.updateUser(account.id, { password: submittedPassword });
      resetId = null;
      if (account.id === user.id) {
        onselfpasswordchanged();
        return;
      }
      notice = `Пароль @${account.username} изменён. Для входа потребуется новый пароль.`;
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = '';
    }
  }

  async function deleteAccount(account: User) {
    if (locked) return;
    busy = 'delete-' + account.id; error = ''; notice = '';
    try { await api.deleteUser(account.id); deleteId = null; notice = 'Аккаунт удалён.'; await reload(); }
    catch (failure) { error = errorMessage(failure); }
    finally { busy = ''; }
  }

  async function createPair(event: SubmitEvent) {
    event.preventDefault();
    if (locked || !trackerId || !partnerId) return;
    busy = 'create-pair';
    error = '';
    notice = '';
    try {
      const created = await api.createPair(trackerId, partnerId);
      trackerId = '';
      partnerId = '';
      notice = `${created.tracker.display_name} и ${created.partner.display_name}: аккаунты связаны.`;
      await reload();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = '';
    }
  }

  async function unlink(pair: Pair) {
    if (locked) return;
    busy = `pair-${pair.id}`;
    error = '';
    notice = '';
    try {
      await api.unlinkPair(pair.id);
      confirmPairId = null;
      notice = 'Пара разорвана. Последующий доступ через эту связь прекращён.';
      await reload();
    } catch (failure) {
      error = errorMessage(failure);
    } finally {
      busy = '';
    }
  }

  function selectTab(next: 'users' | 'pairs') {
    if (locked) return;
    tab = next;
    error = '';
    notice = '';
    password = '';
    resetPassword = '';
    resetId = null;
    confirmUserId = null;
    confirmPairId = null;
  }

  onMount(() => { void load(); });
</script>

<section class="window admin-window" aria-labelledby="admin-title" aria-busy={loading || busy !== ''}>
  <div class="titlebar" aria-hidden="true"><span>admin.exe</span><span class="window-controls">─ □ ×</span></div>
  <div class="window-body">
    <p class="eyebrow">УПРАВЛЕНИЕ СЕРВЕРОМ</p>
    <div class="section-heading admin-heading">
      <h2 id="admin-title">Администрирование</h2>
      <button class="secondary small" type="button" onclick={() => void load()} disabled={locked}>{loading ? 'Загрузка…' : 'Обновить'}</button>
    </div>

    <nav class="tabs" aria-label="Разделы администрирования">
      <button class:current={tab === 'users'} aria-current={tab === 'users' ? 'page' : undefined} type="button" onclick={() => selectTab('users')} disabled={locked}>Аккаунты {loaded ? `(${users.length})` : ''}</button>
      <button class:current={tab === 'pairs'} aria-current={tab === 'pairs' ? 'page' : undefined} type="button" onclick={() => selectTab('pairs')} disabled={locked}>Пары {loaded ? `(${pairs.length})` : ''}</button>
    </nav>
    {#if error}<p class="feedback error" role="alert">{error}{loaded ? ' При необходимости обновите список.' : ''}</p>{/if}
    {#if notice}<p class="feedback" role="status">{notice}</p>{/if}
    {#if !loaded}
      <p class="detail" role="status">{loading ? 'Загружаем аккаунты и пары…' : 'Данные пока не загружены. Нажмите «Обновить».'}</p>
    {:else if tab === 'users'}
      <OwnerInvitePanel admin disabled={locked} />
      <section class="section-block" aria-labelledby="create-user-title">
        <h3 id="create-user-title">Новый аккаунт</h3>
        <form onsubmit={createUser} aria-label="Создать аккаунт">
          <fieldset disabled={locked}>
            <div class="field-grid">
              <div class="field"><label for="new-username">Логин</label><input id="new-username" name="new-username" bind:value={username} autocomplete="off" autocapitalize="none" spellcheck="false" required minlength="3" maxlength="64" pattern={usernamePattern} aria-describedby="username-hint" /></div>
              <div class="field"><label for="new-display-name">Отображаемое имя</label><input id="new-display-name" name="display-name" bind:value={displayName} autocomplete="off" required maxlength="80" /></div>
            </div>
            <p class="field-hint" id="username-hint">Логин: 3–64 символа, строчные латинские буквы, цифры, точка, дефис и _.</p>
            <div class="field-grid">
              <PasswordField label="Пароль" id="new-password" name="new-password"  bind:value={password} autocomplete="new-password" required minlength={12} maxlength={128} aria-describedby="password-hint" />
              <div class="field"><label for="new-role">Роль</label><select id="new-role" name="role" bind:value={role}><option value="tracker">Владелец дневника</option><option value="partner">Партнёр</option><option value="admin">Администратор</option></select></div>
            </div>
            <p class="field-hint" id="password-hint">Пароль: 12–128 символов. Передайте его владельцу безопасным способом.</p>
            <button type="submit">{busy === 'create-user' ? 'Создаём…' : 'Создать аккаунт'}</button>
          </fieldset>
        </form>
      </section>

      <section class="section-block" aria-labelledby="accounts-title">
        <h3 id="accounts-title">Все аккаунты</h3>
        <div class="records">
          {#each users as account (account.id)}
            <article class="record-card" aria-label={`Аккаунт ${account.username}`}>
              <div class="record-heading"><h4>{account.display_name}</h4><span class="tag">{roleNames[account.role]}</span></div>
              <p class="detail">@{account.username} · {account.is_active ? 'Активен' : 'Отключён'}{account.id === user.id ? ' · Ваш аккаунт' : ''}</p>
              <div class="actions compact">
                <button class="secondary small" type="button" disabled={locked || account.id === user.id} onclick={() => { confirmUserId = account.id; resetId = null; resetPassword = ''; }} aria-label={`${account.is_active ? 'Отключить' : 'Включить'} аккаунт ${account.username}`}>{account.is_active ? 'Отключить' : 'Включить'}</button>
                <button class="secondary small" type="button" disabled={locked} onclick={() => { resetId = resetId === account.id ? null : account.id; resetPassword = ''; confirmUserId = null; }} aria-expanded={resetId === account.id} aria-label={`Сменить пароль аккаунта ${account.username}`}>Сменить пароль</button>
              </div>
              {#if account.id !== user.id}
                <button class="secondary small spaced" type="button" disabled={locked} onclick={() => { deleteId = account.id; }}>Удалить аккаунт</button>
                {#if deleteId === account.id}
                  <div class="confirmation"><p>Удалить @{account.username} навсегда? Сессии и пара будут удалены; аккаунт партнёра сохранится.</p><div class="actions compact"><button type="button" disabled={locked} onclick={() => void deleteAccount(account)}>Да, удалить навсегда</button><button class="secondary" type="button" disabled={locked} onclick={() => { deleteId = null; }}>Отмена</button></div></div>
                {/if}
              {/if}
              {#if confirmUserId === account.id}
                <div class="confirmation">
                  <p>{account.is_active ? `Отключить @${account.username}? Его действующие сессии будут отозваны.` : `Включить @${account.username}? Пользователь снова сможет войти.`}</p>
                  <div class="actions compact"><button class="small" type="button" disabled={locked} onclick={() => void toggleActive(account)}>{busy === `active-${account.id}` ? 'Сохраняем…' : 'Подтвердить'}</button><button class="secondary small" type="button" disabled={locked} onclick={() => { confirmUserId = null; }}>Отмена</button></div>
                </div>
              {/if}
              {#if resetId === account.id}
                <form class="inline-form" onsubmit={(event) => void changePassword(event, account)} aria-label={`Новый пароль для ${account.username}`}>
                  <fieldset disabled={locked}>
                    <PasswordField label={`Новый пароль для @${account.username}`} id={`password-${account.id}`} bind:value={resetPassword} autocomplete="new-password" required minlength={12} maxlength={128} />
                    <p class="field-hint">12–128 символов. Все сессии этого аккаунта будут завершены.{account.id === user.id ? ' Вам потребуется войти заново.' : ''}</p>
                    <div class="actions compact"><button class="small" type="submit">{busy === `password-${account.id}` ? 'Сохраняем…' : 'Сохранить пароль'}</button><button class="secondary small" type="button" onclick={() => { resetId = null; resetPassword = ''; }}>Отмена</button></div>
                  </fieldset>
                </form>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {:else}
      <section class="section-block" aria-labelledby="create-pair-title">
        <h3 id="create-pair-title">Связать аккаунты</h3>
        <p class="detail">Выберите два активных аккаунта. Каждый участник может состоять только в одной паре.</p>
        <form class="spaced" onsubmit={createPair} aria-label="Связать аккаунты">
          <fieldset disabled={locked}>
            <div class="field"><label for="pair-tracker">Владелец дневника</label><select id="pair-tracker" bind:value={trackerId} required><option value="">Выберите аккаунт</option>{#each trackers as account (account.id)}<option value={account.id}>{account.display_name} (@{account.username})</option>{/each}</select></div>
            <div class="field"><label for="pair-partner">Партнёр</label><select id="pair-partner" bind:value={partnerId} required><option value="">Выберите аккаунт</option>{#each partners as account (account.id)}<option value={account.id}>{account.display_name} (@{account.username})</option>{/each}</select></div>
            {#if !trackers.length || !partners.length}<p class="field-hint">Для новой пары нужны свободные активные аккаунты обеих ролей. Создайте их в разделе «Аккаунты».</p>{/if}
            <button type="submit" disabled={!trackerId || !partnerId}>{busy === 'create-pair' ? 'Связываем…' : 'Связать аккаунты'}</button>
          </fieldset>
        </form>
      </section>
      <section class="section-block" aria-labelledby="pairs-title">
        <h3 id="pairs-title">Активные пары</h3>
        {#if pairs.length === 0}<p class="detail">Активных пар пока нет.</p>{/if}
        <div class="records">
          {#each pairs as pair (pair.id)}
            <article class="record-card" aria-label={`Пара ${pair.tracker.username} и ${pair.partner.username}`}>
              <p class="record-person"><strong>{pair.tracker.display_name}</strong> <span class="tag">Владелец дневника</span></p>
              <p class="detail">@{pair.tracker.username}{pair.tracker.is_active ? '' : ' · Отключён'}</p>
              <p class="pair-divider" aria-hidden="true">↔</p>
              <p class="record-person"><strong>{pair.partner.display_name}</strong> <span class="tag">Партнёр</span></p>
              <p class="detail">@{pair.partner.username}{pair.partner.is_active ? '' : ' · Отключён'}</p>
              {#if confirmPairId === pair.id}
                <div class="confirmation"><p>Разорвать эту пару? Доступ через эту связь прекратится, а записи партнёра в этой паре будут удалены.</p><div class="actions compact"><button class="small" type="button" disabled={locked} onclick={() => void unlink(pair)}>{busy === `pair-${pair.id}` ? 'Разрываем…' : 'Да, разорвать пару'}</button><button class="secondary small" type="button" disabled={locked} onclick={() => { confirmPairId = null; }}>Отмена</button></div></div>
              {:else}
                <button class="secondary small spaced" type="button" disabled={locked} onclick={() => { confirmPairId = pair.id; }}>Разорвать пару</button>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {/if}
  </div>
  <footer class="window-footer"><span>Только администратор</span><span>admin / local</span></footer>
</section>
