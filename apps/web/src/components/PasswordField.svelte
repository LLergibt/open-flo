<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  type Props = Omit<HTMLInputAttributes, 'type' | 'value'> & { id: string; label: string; value?: string };
  let { id, label, value = $bindable(''), name = id, autocomplete = 'current-password', ...attributes }: Props = $props();
  let visible = $state(false);
  // Clearing a submitted/cancelled field also restores its private default.
  $effect(() => { if (!value) visible = false; });
</script>

<div class="field">
  <label for={id}>{label}</label>
  <div class="password-control">
    <input {...attributes} {id} {name} {autocomplete} type={visible ? 'text' : 'password'} bind:value autocapitalize="none" autocorrect="off" spellcheck="false" />
    <button type="button" class="secondary password-toggle" aria-controls={id} aria-pressed={visible} aria-label={`${visible ? 'Скрыть' : 'Показать'} пароль: ${label}`} disabled={attributes.disabled} onclick={() => { visible = !visible; }}>
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true" focusable="false">
        <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6S2 12 2 12Z" />
        <circle cx="12" cy="12" r="3" />
        {#if visible}<path d="m3 3 18 18" />{/if}
      </svg>
    </button>
  </div>
</div>

<style>
  .password-control { position: relative; }
  input { inline-size: 100%; padding-inline-end: 3.5rem; font-size: max(1rem, 1em); }
  .password-toggle { position: absolute; inset-inline-end: 1px; inset-block-start: 1px; inline-size: 3rem; block-size: calc(100% - 2px); min-block-size: 0; display: grid; place-items: center; padding: 0; border: 0; background: transparent; color: #303647; }
  .password-toggle:active:enabled { transform: none; }
  svg { display: block; }
</style>
