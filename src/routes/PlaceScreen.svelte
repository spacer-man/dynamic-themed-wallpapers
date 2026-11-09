<script lang="ts">
  import { api } from '../lib/api';
  import { app } from '../lib/app.svelte';
  import { i18n, t } from '../lib/i18n.svelte';
  import { MAX_RADIUS_KM, MIN_RADIUS_KM, isWide, naturalRadiusKm } from '../lib/place';
  import type { PlaceHit } from '../lib/types';
  import { sourceName } from '../lib/sources';
  import { subjectName, type Subject } from '../lib/types';
  import Icon from '../lib/components/Icon.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import StatusChip from '../lib/components/StatusChip.svelte';
  import Toggle from '../lib/components/Toggle.svelte';

  let mode = $state<'place' | 'query' | 'board'>('place');
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

  // Free-text subjects: add a new one, or edit a saved one.
  let text = $state('');
  let editingId = $state<string | null>(null);
  let textInvalid = $state(false);

  const nameOf = (s: Subject) => subjectName(s);

  // Pinterest boards: paste an address; the source switches to Pinterest by itself.
  let boardUrl = $state('');
  let addingBoard = $state(false);
  let boardInvalid = $state(false);

  async function submitBoard() {
    const v = boardUrl.trim();
    if (!v) return;
    addingBoard = true;
    boardInvalid = false;
    app.boardError = null;
    const ok = await app.addBoard(v);
    addingBoard = false;
    if (ok) boardUrl = '';
    else boardInvalid = app.boardError !== null;
  }

  async function submitText() {
    const v = text.trim();
    if ([...v].length < 2 || [...v].length > 80) {
      textInvalid = true;
      return;
    }
    textInvalid = false;
    const ok = editingId ? await app.updateQuery(editingId, v) : await app.addQuery(v);
    if (ok) cancelEdit();
  }

  function startEdit(s: Subject) {
    if (s.kind !== 'query') return;
    mode = 'query';
    editingId = s.id;
    text = s.text;
    textInvalid = false;
  }

  // While a saved subject is being edited, back cancels the edit.
  $effect(() => {
    if (!editingId) return;
    return app.pushLayer(cancelEdit);
  });

  function cancelEdit() {
    editingId = null;
    text = '';
    textInvalid = false;
  }

  const place = $derived(app.place);
  const activeSource = $derived(app.sources.find((x) => x.active));
  const wide = $derived(place ? isWide(place) : false);
  const auto = $derived(place ? naturalRadiusKm(place) : 0);
  const customRadius = $derived(app.settings?.radius_km ?? null);

  function fmtKm(km: number): string {
    return km >= 10 ? String(Math.round(km)) : km.toFixed(1);
  }
</script>

<section class="screen">
  <h1>{t('place.title')}</h1>

  <div class="card">
    <span class="label">{t('subject.saved')}</span>
    {#if app.subjects.items.length === 0}
      <p class="hint">{t('subject.saved.empty')}</p>
    {:else}
      <ul class="saved">
        {#each app.subjects.items as s (s.id)}
          {@const on = s.id === app.subjects.active}
          <li class:on>
            <button
              class="pick"
              aria-pressed={on}
              aria-label={t('subject.use', { name: nameOf(s) })}
              onclick={() => app.selectSubject(s.id)}
            >
              <span class="check">{#if on}<Icon name="check" size={20} />{/if}</span>
              <span class="what">
                <strong>{nameOf(s)}</strong>
                <span class="kind">{t(`subject.kind.${s.kind}`)}</span>
              </span>
            </button>
            {#if s.kind === 'query'}
              <button class="icon" aria-label={t('subject.edit', { name: nameOf(s) })} onclick={() => startEdit(s)}>
                <Icon name="edit" size={20} />
              </button>
            {/if}
            <button class="icon" aria-label={t('subject.delete', { name: nameOf(s) })} onclick={() => app.deleteSubject(s.id)}>
              <Icon name="trash" size={20} />
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  {#if place}
    <div class="card current">
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
    </div>
  {/if}

  <div class="card">
    <span class="label">{t('subject.add')}</span>
    <Segmented
      options={[
        { value: 'place', label: t('subject.mode.place') },
        { value: 'query', label: t('subject.mode.query') },
        { value: 'board', label: t('subject.mode.board') },
      ]}
      value={mode}
      onchange={(v) => (mode = v)}
    />

    {#if mode === 'place'}
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
    {:else if mode === 'board'}
      <form
        class="textform"
        onsubmit={(e) => {
          e.preventDefault();
          void submitBoard();
        }}
      >
        <input
          type="url"
          inputmode="url"
          placeholder={t('subject.board.placeholder')}
          bind:value={boardUrl}
          maxlength="300"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          aria-label={t('subject.mode.board')}
          aria-invalid={boardInvalid}
        />
        <div class="actions">
          <button class="btn primary" type="submit" disabled={addingBoard || !boardUrl.trim()}>
            {addingBoard ? t('subject.board.adding') : t('subject.board.add')}
          </button>
        </div>
      </form>
      <span class="hint">{boardInvalid ? t('subject.board.invalid') : t('subject.board.hint')}</span>
      <span class="hint">{t('subject.board.locked')}</span>
    {:else}
      <form
        class="textform"
        onsubmit={(e) => {
          e.preventDefault();
          void submitText();
        }}
      >
        <input
          type="text"
          placeholder={t('subject.query.placeholder')}
          bind:value={text}
          maxlength="80"
          autocomplete="off"
          spellcheck="false"
          aria-invalid={textInvalid}
        />
        <div class="actions">
          <button class="btn primary" type="submit">
            {editingId ? t('subject.query.update') : t('subject.query.add')}
          </button>
          {#if editingId}
            <button class="btn" type="button" onclick={cancelEdit}>{t('subject.query.cancel')}</button>
          {/if}
        </div>
      </form>
      <span class="hint">{textInvalid ? t('subject.query.invalid') : t('subject.query.hint')}</span>
    {/if}
  </div>

  <div class="card">
    <span class="label">{t('source.current')}</span>
    {#if activeSource}
      <div class="name">
        <strong>{sourceName(activeSource.source)}</strong>
        <StatusChip state={activeSource.state} />
      </div>
    {/if}
    {#if app.activeSubject?.kind === 'board'}
      <span class="hint">{t('sources.locked.board')}</span>
    {/if}
    <button class="btn" onclick={() => app.go('sources')}>{t('source.change')}</button>
  </div>
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
    color: var(--text);
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
    background: var(--bg);
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
    text-overflow: ellipsis;
  }

  .saved {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .saved li {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    border-radius: var(--radius-sm);
  }

  .saved li.on {
    background: var(--accent-soft);
  }

  .pick {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: 48px;
    padding: var(--space-2);
    border: 0;
    background: none;
    text-align: left;
  }

  .check {
    flex: none;
    width: 20px;
    color: var(--accent);
    display: inline-flex;
  }

  .what {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    flex-wrap: wrap;
    min-width: 0;
  }

  .what strong {
    font-size: var(--fs-md);
    overflow-wrap: anywhere;
  }

  .icon {
    flex: none;
    width: 44px;
    height: 44px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 50%;
    background: none;
    color: var(--muted);
  }

  .textform {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .textform input {
    min-height: 48px;
    padding: 0 var(--space-4);
    border-radius: 999px;
    border: 1px solid var(--hairline);
    background: var(--bg);
    color: var(--text);
  }

  .textform input:focus {
    outline: 0;
    border-color: var(--accent);
  }

  .textform input[aria-invalid='true'] {
    border-color: var(--text);
  }

  .actions {
    display: flex;
    gap: var(--space-3);
    flex-wrap: wrap;
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
