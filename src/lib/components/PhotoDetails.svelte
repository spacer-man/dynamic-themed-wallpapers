<script lang="ts">
  import { app } from '../app.svelte';
  import { t } from '../i18n.svelte';
  import type { HistoryEntry } from '../types';
  import Sheet from './Sheet.svelte';

  // `onapply` (History only) adds a "Set as wallpaper" button.
  let {
    entry,
    onclose,
    onapply,
  }: { entry: HistoryEntry; onclose: () => void; onapply?: () => void } = $props();
  const c = $derived(entry.candidate);
</script>

<Sheet title={t('details.title')} {onclose}>
  <h3 class="title">{c.title}</h3>
  <dl>
    <dt>{t('details.place')}</dt>
    <dd>{c.place_label}</dd>
    <dt>{t('details.author')}</dt>
    <dd>{c.author}</dd>
    <dt>{t('details.license')}</dt>
    <dd>{c.license.short_name}</dd>
    <dt>{t('details.source')}</dt>
    <!-- Photo metadata is untrusted: rendered as text only, link target is a plain https URL. -->
    <dd><a href={c.page_url} target="_blank" rel="noopener noreferrer">{c.page_url}</a></dd>
  </dl>
  {#if c.description}<p class="hint">{c.description}</p>{/if}
  {#if onapply}
    <button class="btn primary apply" onclick={onapply} disabled={app.busy} aria-busy={app.busy}>
      {app.busy ? t('details.applying') : t('details.apply')}
    </button>
  {/if}
</Sheet>

<style>
  .title {
    font-size: var(--fs-md);
    overflow-wrap: anywhere;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--space-2) var(--space-4);
    margin: 0;
    font-size: var(--fs-sm);
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  a {
    color: var(--accent);
  }

  .apply {
    width: 100%;
    margin-top: var(--space-4);
  }
</style>
