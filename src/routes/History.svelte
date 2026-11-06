<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { t } from '../lib/i18n.svelte';
  import type { HistoryEntry } from '../lib/types';
  import Icon from '../lib/components/Icon.svelte';
  import PhotoDetails from '../lib/components/PhotoDetails.svelte';

  let selected = $state<HistoryEntry | null>(null);

  const fmtDate = (ts: number) =>
    new Date(ts * 1000).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
</script>

<section class="screen">
  <h1>{t('history.title')}</h1>

  {#if app.history.length === 0}
    <p class="hint">{t('history.empty')}</p>
  {:else}
    <ul class="grid">
      {#each app.history as e (e.id)}
        <li>
          <button onclick={() => (selected = e)} aria-label={`${e.candidate.title}, ${fmtDate(e.shown_at)}`}>
            <img src={e.candidate.preview_url} alt="" loading="lazy" decoding="async" />
            <span class="date">{fmtDate(e.shown_at)}</span>
            {#if e.liked !== null}
              <span class="mark" class:liked={e.liked} title={e.liked ? t('history.liked') : t('history.skipped')}>
                <Icon name={e.liked ? 'heart' : 'close'} size={14} filled={e.liked} />
              </span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if selected}
  <PhotoDetails
    entry={selected}
    onclose={() => (selected = null)}
    onapply={async () => {
      if (await app.applyFromHistory(selected!.id)) selected = null;
    }}
  />
{/if}

<style>
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-2);
  }

  button {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 9 / 16;
    padding: 0;
    border: 1px solid var(--hairline);
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--surface);
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .date {
    position: absolute;
    left: 6px;
    bottom: 6px;
    padding: 1px 7px;
    border-radius: 999px;
    background: rgb(0 0 0 / 0.62);
    color: #fff;
    font-size: 11px;
    font-weight: 600;
  }

  .mark {
    position: absolute;
    top: 6px;
    right: 6px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgb(0 0 0 / 0.62);
    color: #fff;
  }

  .mark.liked {
    background: #fff;
    color: #b3123f;
  }
</style>
