<script lang="ts">
  import { onMount } from 'svelte';

  type Message = { message: string; source: 'sqlite' };
  type State =
    | { status: 'loading' }
    | { status: 'success'; value: Message }
    | { status: 'error'; explanation: string };

  let state = $state<State>({ status: 'loading' });
  let controller: AbortController | undefined;

  function isMessage(value: unknown): value is Message {
    if (typeof value !== 'object' || value === null) return false;
    const message = value as Record<string, unknown>;
    return typeof message.message === 'string' && message.message.trim().length > 0 && message.source === 'sqlite';
  }

  async function loadMessage() {
    controller?.abort();
    const request = new AbortController();
    controller = request;
    state = { status: 'loading' };
    let timedOut = false;
    const timeout = window.setTimeout(() => {
      timedOut = true;
      request.abort();
    }, 10_000);

    try {
      const response = await fetch('/api/message', {
        signal: request.signal,
        cache: 'no-store',
        headers: { Accept: 'application/json' },
      });
      if (!response.ok) throw new Error(`Сервер вернул HTTP ${response.status}.`);
      const value: unknown = await response.json();
      if (!isMessage(value)) throw new Error('Ответ сервера не соответствует формату сообщения.');
      if (!request.signal.aborted) state = { status: 'success', value };
    } catch (error) {
      if (request.signal.aborted && !timedOut) return;
      state = {
        status: 'error',
        explanation: timedOut
          ? 'Сервер не ответил за 10 секунд.'
          : error instanceof TypeError
            ? 'Не удалось связаться с API. Проверьте, что сервер запущен.'
            : error instanceof Error
              ? error.message
              : 'Не удалось получить сообщение.',
      };
    } finally {
      window.clearTimeout(timeout);
    }
  }

  onMount(() => {
    void loadMessage();
    return () => controller?.abort();
  });
</script>

<main>
  <header class="page-header">
    <a class="wordmark" href="/" aria-label="open-source-flo, главная">flo<span aria-hidden="true"> / </span>local</a>
    <span class="build-label">КАРКАС · 001</span>
  </header>

  <section class="window" aria-labelledby="screen-title">
    <div class="titlebar" aria-hidden="true">
      <span>connection.exe</span>
      <span class="window-controls">─ □ ×</span>
    </div>

    <div class="window-body">
      <p class="eyebrow">СВОЙ СЕРВЕР · СВОИ ДАННЫЕ</p>
      <h1 id="screen-title">Проверка подключения</h1>
      <p class="intro">Эта страница получает тестовое сообщение из базы данных через API.</p>

      <ol class="connection-path" aria-label="Путь получения данных">
        <li>SQLite</li>
        <li><span aria-hidden="true">→</span> API</li>
        <li><span aria-hidden="true">→</span> Приложение</li>
      </ol>

      <div class="message-panel" aria-busy={state.status === 'loading'}>
        <p class="panel-label">СООБЩЕНИЕ ИЗ БАЗЫ</p>
        <div aria-live="polite" aria-atomic="true">
          {#if state.status === 'loading'}
            <p class="message loading">Получаем сообщение…</p>
            <p class="detail">Подключаемся к серверу.</p>
          {:else if state.status === 'success'}
            <p class="message" data-testid="database-message">{state.value.message}</p>
            <p class="detail">Источник: {state.value.source} · GET /api/message</p>
          {:else}
            <p class="message">Соединение не установлено</p>
            <p class="detail error" role="alert">{state.explanation}</p>
          {/if}
        </div>
      </div>

      <div class="actions">
        <button type="button" onclick={() => void loadMessage()} disabled={state.status === 'loading'}>
          {state.status === 'loading' ? 'Загрузка…' : state.status === 'error' ? 'Повторить подключение' : 'Получить снова'}
        </button>
        <span class="status" role="status">
          <span class="status-mark" aria-hidden="true">{state.status === 'success' ? '●' : state.status === 'error' ? '×' : '○'}</span>
          {state.status === 'success' ? 'Подключено' : state.status === 'error' ? 'Нет соединения' : 'Ожидание'}
        </span>
      </div>
    </div>

    <footer class="window-footer">
      <span>open-source-flo</span>
      <span>web / dev</span>
    </footer>
  </section>

  <p class="footnote">Тестовая сборка. Здесь используются только демонстрационные данные.</p>
</main>
