<script lang="ts">
  import { t } from '../i18n.svelte';
  import type { SourceState } from '../types';

  let { state }: { state: SourceState } = $props();
</script>

<!-- One accent for "all good"; every other state is neutral text, so colour never carries the meaning alone. -->
<span class="chip" class:ok={state === 'available'}>{t(`sources.state.${state}`)}</span>

<style>
  .chip {
    flex: none;
    font-size: var(--fs-xs);
    font-weight: 600;
    padding: 2px 10px;
    border-radius: 999px;
    border: 1px solid var(--hairline);
    color: var(--muted);
  }

  .chip.ok {
    border-color: transparent;
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
