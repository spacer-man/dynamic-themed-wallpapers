<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { i18n, t } from '../lib/i18n.svelte';
  import { MAX_RADIUS_KM, MIN_RADIUS_KM, isWide, naturalRadiusKm } from '../lib/place';
  import type { PlaceHit } from '../lib/types';
  import Icon from '../lib/components/Icon.svelte';
  import Toggle from '../lib/components/Toggle.svelte';

  let query = $state('');
  let hits = $state<PlaceHit[]>([]);
  let searching = $state(false);
  let searched = $state(false);
  let seq = 0;

  // Debounced search. `seq` drops answers that arrive after a newer query was typed.
  $effect(() => {
    const q = query.trim();
    if (!q) {
      hits = [];
      searched = false;
      searching = false;
      return;
    }
    searching = true;
    const timer = setTimeout(async () => {
      const mine = ++seq;
      try {
        const res = await api.searchPlaces(q, i18n.lang);
        if (mine !== seq) return;
        hits = res;
        searched = true;
      } catch (e) {
        if (mine === seq) app.fail(e);
      } finally {
        if (mine === seq) searching = false;
      }
    }, 350);
    return () => clearTimeout(timer);
  });

  const place = $derived(app.place);
  const wide = $derived(place ? isWide(place) : false);
  const auto = $derived(place ? naturalRadiusKm(place) : 0);
  const customRadius = $derived(app.settings?.radius_km ?? null);

  function fmtKm(km: number): string {
    return km >= 10 ? String(Math.round(km)) : km.toFixed(1);
  }
</script>

<section class="screen">
  <h1>{t('place.title')}</h1>

  <div class="card current">
    <span class="hint">{t('place.current')}</span>
    {#if place}
      <div class="name">
        <strong>{place.label}</strong>
        <span class="kind">{t(`place.kind.${place.kind}`)}</span>
      </div>
      {#if place.description}<span class="hint">{place.description}</span>{/if}

      {#if wide}
        <p class="hint">{t('place.wide')}</p>
      {:else}
        <Toggle
          checked={customRadius === null}
          label={t('place.radius')}
          hint={customRadius === null ? t('place.radius.auto', { km: fmtKm(Math.min(auto, MAX_RADIUS_KM)) }) : t('place.radius.hint')}
          onchange={(auto) => app.saveSettings({ radius_km: auto ? null : Math.min(naturalRadiusKm(place), MAX_RADIUS_KM) })}
        />
        {#if customRadius !== null}
          <label class="range">
            <input
              type="range"
              min={MIN_RADIUS_KM}
              max={MAX_RADIUS_KM}
              step="0.5"
              value={customRadius}
              onchange={(e) => app.saveSettings({ radius_km: Number(e.currentTarget.value) })}
            />
            <output>{fmtKm(customRadius)} km</output>
          </label>
        {/if}
      {/if}
    {:else}
      <strong>{t('place.none')}</strong>
    {/if}
  </div>

  <label class="search">
    <Icon name="search" size={20} />
    <input
      type="search"
      placeholder={t('place.search')}
      bind:value={query}
      autocomplete="off"
      autocapitalize="off"
      spellcheck="false"
    />
  </label>

  {#if searching}
    <p class="hint">{t('place.searching')}</p>
  {:else if searched && hits.length === 0}
    <p class="hint">{t('place.notfound')}</p>
  {/if}

  {#if hits.length > 0}
    <ul class="hits">
      {#each hits as h (h.qid)}
        <li>
          <button onclick={() => app.selectPlace(h.qid)}>
            <strong>{h.label}</strong>
            {#if h.description}<span class="hint">{h.description}</span>{/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .name {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  strong {
    font-size: var(--fs-lg);
  }

  .kind {
    font-size: var(--fs-xs);
    padding: 2px 10px;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .search {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-4);
    min-height: 52px;
    border-radius: 999px;
    border: 1px solid var(--hairline);
    background: var(--surface);
    color: var(--muted);
  }

  .search:focus-within {
    border-color: var(--accent);
  }

  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    color: var(--text);
  }

  .hits {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border-radius: var(--radius);
    overflow: hidden;
    border: 1px solid var(--hairline);
    background: var(--surface);
  }

  .hits li + li {
    border-top: 1px solid var(--hairline);
  }

  .hits button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    width: 100%;
    padding: var(--space-3) var(--space-4);
    border: 0;
    background: none;
    text-align: left;
    min-height: 56px;
  }

  .hits strong {
    font-size: var(--fs-md);
  }

  .range {
    display: flex;
    align-items: center;
    gap: var(--space-3);
  }

  .range input {
    flex: 1;
    accent-color: var(--accent);
  }

  .range output {
    min-width: 64px;
    text-align: right;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
</style>
