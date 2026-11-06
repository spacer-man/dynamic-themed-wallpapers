<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { i18n, t } from '../lib/i18n.svelte';
  import { SOURCES } from '../lib/sources';
  import type { SourceId, SourceStatus } from '../lib/types';
  import StatusChip from '../lib/components/StatusChip.svelte';

  // What the user typed per source. Never kept longer than needed and never shown again.
  let drafts = $state<Partial<Record<SourceId, string>>>({});
  let invalid = $state<Partial<Record<SourceId, boolean>>>({});

  const status = (id: SourceId): SourceStatus | undefined => app.sources.find((s) => s.source === id);
  const busy = $derived(app.checking.length > 0);

  function when(ts: number): string {
    const lang = i18n.lang;
    return new Date(ts * 1000).toLocaleString(lang, { dateStyle: 'short', timeStyle: 'short' });
  }

  async function saveKey(id: SourceId) {
    const key = (drafts[id] ?? '').trim();
    if (!key) return;
    const ok = await app.setSourceKey(id, key);
    invalid = { ...invalid, [id]: !ok };
    if (ok) {
      drafts = { ...drafts, [id]: '' };
      void app.checkSource(id);
    }
  }

  async function removeKey(id: SourceId) {
    invalid = { ...invalid, [id]: false };
    await app.setSourceKey(id, null);
  }
</script>

<section class="screen">
  <div class="head">
    <button class="btn" onclick={() => app.go('place')}>{t('sources.back')}</button>
    <button class="btn" onclick={() => app.checkAll()} disabled={busy}>
      {busy ? t('sources.checking') : t('sources.refresh')}
    </button>
  </div>
  <h1>{t('sources.title')}</h1>
  <p class="hint">{t('sources.intro')}</p>

  {#each SOURCES as info (info.id)}
    {@const st = status(info.id)}
    {#if st}
      <div class="card" class:active={st.active}>
        <div class="top">
          <strong>{info.name}</strong>
          <StatusChip state={st.state} />
        </div>
        <p class="hint">{t(`sources.${info.id}`)}</p>

        <p class="hint" role="status">
          {#if app.checking.includes(info.id)}
            {t('sources.checking')}
          {:else if st.error}
            {st.error} · {st.checked_at ? t('sources.checked', { time: when(st.checked_at) }) : ''}
          {:else if st.checked_at}
            {t('sources.checked', { time: when(st.checked_at) })}
          {:else}
            {t('sources.never')}
          {/if}
        </p>

        {#if info.keyHost}
          <div class="key">
            <span class="label">{t('sources.key')}</span>
            {#if st.has_key}
              <span class="hint">{t('sources.key.saved')}</span>
              <button class="btn" onclick={() => removeKey(info.id)}>{t('sources.key.remove')}</button>
            {:else}
              <form
                onsubmit={(e) => {
                  e.preventDefault();
                  void saveKey(info.id);
                }}
              >
                <input
                  type="password"
                  autocomplete="off"
                  autocapitalize="off"
                  spellcheck="false"
                  placeholder={t('sources.key.placeholder')}
                  aria-label={`${info.name}: ${t('sources.key')}`}
                  aria-invalid={invalid[info.id] ? true : undefined}
                  bind:value={drafts[info.id]}
                />
                <button class="btn" type="submit" disabled={!(drafts[info.id] ?? '').trim()}>
                  {t('sources.key.save')}
                </button>
              </form>
              {#if invalid[info.id]}<span class="hint">{t('sources.key.invalid')}</span>{/if}
              <span class="hint">{t('sources.key.get', { host: info.keyHost })}</span>
            {/if}
            <span class="hint">{t('sources.key.private')}</span>
          </div>
        {/if}

        <div class="actions">
          {#if st.active}
            <span class="inuse">{t('sources.inuse')}</span>
          {:else}
            <button
              class="btn primary"
              onclick={() => app.selectSource(info.id)}
              disabled={st.state === 'needs_key' || st.state === 'unsupported'}
            >
              {t('sources.use')}
            </button>
          {/if}
          {#if !st.needs_key || st.has_key}
            <button class="btn" onclick={() => app.checkSource(info.id)} disabled={app.checking.includes(info.id)}>
              {t('sources.check')}
            </button>
          {/if}
        </div>
      </div>
    {/if}
  {/each}
</section>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .card.active {
    border-color: var(--accent);
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .top strong {
    font-size: var(--fs-md);
  }

  .key {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .key form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .key input {
    min-height: 48px;
    padding: 0 var(--space-4);
    border-radius: 999px;
    border: 1px solid var(--hairline);
    background: var(--bg);
    color: var(--text);
  }

  .key input:focus {
    outline: 0;
    border-color: var(--accent);
  }

  .key input[aria-invalid='true'] {
    border-color: var(--text);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .inuse {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--accent);
  }
</style>
