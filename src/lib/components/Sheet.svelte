<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from '../i18n.svelte';
  import Icon from './Icon.svelte';

  let { title, onclose, children }: { title: string; onclose: () => void; children: Snippet } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<div class="backdrop" role="presentation" onclick={onclose}></div>
<div class="sheet" role="dialog" aria-modal="true" aria-label={title}>
  <header>
    <h2>{title}</h2>
    <button class="close" onclick={onclose} aria-label={t('details.close')}><Icon name="close" size={20} /></button>
  </header>
  <div class="body">{@render children()}</div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    background: color-mix(in srgb, #000 45%, transparent);
    animation: fade 200ms var(--ease);
  }

  .sheet {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 21;
    max-height: 82dvh;
    overflow-y: auto;
    padding: var(--space-4) var(--space-4) calc(var(--space-5) + env(safe-area-inset-bottom));
    background: var(--surface);
    color: var(--text);
    border-radius: var(--radius) var(--radius) 0 0;
    box-shadow: 0 -8px 32px color-mix(in srgb, #000 28%, transparent);
    animation: rise 260ms var(--ease);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--space-3);
  }

  h2 {
    font-size: var(--fs-lg);
  }

  .close {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border: 0;
    border-radius: 50%;
    background: color-mix(in srgb, var(--text) 8%, transparent);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  @keyframes rise {
    from {
      transform: translateY(100%);
    }
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
