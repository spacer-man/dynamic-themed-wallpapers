<script lang="ts">
  import { app } from '../app.svelte';
  import { t } from '../i18n.svelte';
  import { sourceName } from '../sources';
</script>

{#if app.error}
  <div class="banner" class:wide={app.fallbackOptions.length > 0} role="alert">
    <span>{t(`error.${app.error.kind}`)}</span>
    {#if app.fallbackOptions.length > 0}
      <!-- One-time substitutes: the chosen source stays as it is. -->
      <div class="options">
        {#each app.fallbackOptions as o (o.source)}
          <button onclick={() => app.changeNow(o.source)} disabled={app.busy}>
            {t('error.try', { source: sourceName(o.source) })}
          </button>
        {/each}
      </div>
    {/if}
    <button onclick={() => (app.error = null)}>{t('error.dismiss')}</button>
  </div>
{/if}

<style>
  .banner {
    position: fixed;
    z-index: 30;
    inset: calc(var(--space-3) + env(safe-area-inset-top)) var(--space-3) auto var(--space-3);
    display: flex;
    flex-wrap: wrap;
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

  .wide > span {
    flex: 1 1 100%;
  }

  .options {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  button {
    flex: none;
    min-height: 44px;
    border: 1px solid color-mix(in srgb, #ffe9ea 50%, transparent);
    border-radius: 999px;
    padding: 4px 12px;
    background: none;
    color: inherit;
    font-size: var(--fs-xs);
  }

  button:disabled {
    opacity: 0.5;
  }

  @keyframes drop {
    from {
      transform: translateY(-16px);
      opacity: 0;
    }
  }
</style>
