<script lang="ts">
  import { app } from '../app.svelte';
</script>

{#if app.toast}
  <div class="toast" role="status">{app.toast}</div>
{/if}

<style>
  .toast {
    position: fixed;
    z-index: 40;
    left: 50%;
    bottom: calc(var(--tabbar-h) + var(--space-4) + env(safe-area-inset-bottom));
    transform: translateX(-50%);
    max-width: calc(100% - 2 * var(--space-4));
    padding: var(--space-2) var(--space-4);
    border-radius: 999px;
    background: color-mix(in srgb, #000 78%, transparent);
    color: #fff;
    font-size: var(--fs-sm);
    text-align: center;
    pointer-events: none;
    box-shadow: 0 6px 24px color-mix(in srgb, #000 35%, transparent);
    animation: rise 200ms var(--ease);
  }

  @keyframes rise {
    from {
      transform: translate(-50%, 8px);
      opacity: 0;
    }
  }
</style>
