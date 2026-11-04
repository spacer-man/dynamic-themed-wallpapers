<script lang="ts">
  import { app } from '../app.svelte';
  import { t } from '../i18n.svelte';
</script>

{#if app.error}
  <div class="banner" role="alert">
    <span>{t(`error.${app.error.kind}`)}</span>
    <button onclick={() => (app.error = null)}>{t('error.dismiss')}</button>
  </div>
{/if}

<style>
  .banner {
    position: fixed;
    z-index: 30;
    inset: calc(var(--space-3) + env(safe-area-inset-top)) var(--space-3) auto var(--space-3);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-sm);
    background: #3a0f12;
    color: #ffe9ea;
    font-size: var(--fs-sm);
    box-shadow: 0 6px 24px color-mix(in srgb, #000 35%, transparent);
    animation: drop 240ms var(--ease);
  }

  button {
    flex: none;
    border: 1px solid color-mix(in srgb, #ffe9ea 50%, transparent);
    border-radius: 999px;
    padding: 4px 12px;
    background: none;
    color: inherit;
    font-size: var(--fs-xs);
  }

  @keyframes drop {
    from {
      transform: translateY(-16px);
      opacity: 0;
    }
  }
</style>
