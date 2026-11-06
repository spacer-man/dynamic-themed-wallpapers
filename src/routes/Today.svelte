<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { captionTreatment, type Ink } from '../lib/color';
  import { t } from '../lib/i18n.svelte';
  import type { Rgb } from '../lib/types';
  import Icon from '../lib/components/Icon.svelte';
  import PhotoDetails from '../lib/components/PhotoDetails.svelte';

  let showDetails = $state(false);
  /** Caption colours: ink (text) and the scrim behind it; recomputed for every photo. */
  let scrim = $state(0.75);
  let ink = $state<Ink>('light');

  const w = $derived(app.wallpaper);
  const c = $derived(w?.entry.candidate);

  /**
   * Caption contrast: sample the photo where the text sits, find how bright its brightest 5%
   * is, and make the scrim just dark enough for white text to reach 4.5:1 there.
   */
  function measure(img: HTMLImageElement) {
    try {
      const cw = 48;
      const ch = Math.max(8, Math.round((cw * img.naturalHeight) / img.naturalWidth));
      const canvas = document.createElement('canvas');
      canvas.width = cw;
      canvas.height = ch;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      if (!ctx) return;
      ctx.drawImage(img, 0, 0, cw, ch);
      const top = Math.floor(ch * 0.62); // bottom 38% — slightly more than the text block
      const data = ctx.getImageData(0, top, cw, ch - top).data;
      const px: Rgb[] = [];
      for (let i = 0; i < data.length; i += 4) px.push([data[i]!, data[i + 1]!, data[i + 2]!]);
      const tr = captionTreatment(px);
      scrim = tr.alpha;
      ink = tr.ink;
    } catch {
      scrim = 0.8; // unreadable pixels: be safe
      ink = 'light';
    }
  }
</script>

<section class="today" class:ink-dark={ink === 'dark'} style:--scrim={scrim}>
  {#if w?.image}
    {#key w.entry.candidate.id}
      <img
        class="photo"
        src={w.image}
        alt={c?.title ?? ''}
        onload={(e) => measure(e.currentTarget as HTMLImageElement)}
      />
    {/key}
    <div class="scrim" aria-hidden="true"></div>

    <div class="meta">
      <div class="caption">
        <h1>{c?.place_label}</h1>
        <p class="title">{c?.title}</p>
        <p class="credit">
          {c?.author} · {c?.license.short_name}
        </p>
      </div>

      <div class="actions">
        <button class="act" onclick={() => app.skip()} disabled={app.busy} aria-label={t('today.skip')}>
          <Icon name="close" />
          <span>{t('today.skip')}</span>
        </button>
        <button
          class="act"
          class:liked={w.entry.liked === true}
          onclick={() => app.like()}
          disabled={app.busy}
          aria-label={t('today.like')}
          aria-pressed={w.entry.liked === true}
        >
          <Icon name="heart" filled={w.entry.liked === true} />
          <span>{t('today.like')}</span>
        </button>
        <button class="act main" onclick={() => app.changeNow()} disabled={app.busy} aria-label={t('today.change')}>
          <Icon name="refresh" />
          <span>{t('today.change')}</span>
        </button>
        <button class="act" onclick={() => (showDetails = true)} aria-label={t('today.details')}>
          <Icon name="info" />
          <span>{t('today.details')}</span>
        </button>
      </div>
    </div>
  {:else}
    <div class="empty">
      {#if !app.activeSubject}
        <div class="card">
          <h1>{t('today.empty.title')}</h1>
          <p class="hint">{t('today.empty.text')}</p>
          <button class="btn primary" onclick={() => app.go('place')}>{t('today.empty.cta')}</button>
        </div>
      {:else}
        <div class="card">
          <h1>{t('today.first.title')}</h1>
          <p class="hint">{t('today.first.text', { place: app.activeLabel })}</p>
          <button class="btn primary" onclick={() => app.changeNow()} disabled={app.busy}>
            {t('today.first.cta')}
          </button>
        </div>
      {/if}
    </div>
  {/if}

  {#if app.busy}
    <div class="busy" role="status">
      <span class="spinner" aria-hidden="true"></span>
      <span>{t('today.loading')}</span>
    </div>
  {/if}
</section>

{#if showDetails && w}
  <PhotoDetails entry={w.entry} onclose={() => (showDetails = false)} />
{/if}

<style>
  .today {
    position: relative;
    height: 100%;
    overflow: hidden;
    background: var(--bg);
    /* Caption palette: white on a dark scrim by default, flipped by .ink-dark. */
    --ink: #fff;
    --scrim-rgb: 0 0 0;
    --glass-bg: rgb(255 255 255 / 0.14);
    --glass-border: rgb(255 255 255 / 0.28);
  }

  .today.ink-dark {
    --ink: #111;
    --scrim-rgb: 255 255 255;
    --glass-bg: rgb(0 0 0 / 0.07);
    --glass-border: rgb(0 0 0 / 0.2);
  }

  .photo {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    animation: reveal 600ms var(--ease);
  }

  .scrim {
    position: absolute;
    inset: auto 0 0 0;
    height: 62%;
    background: linear-gradient(
      to top,
      rgb(var(--scrim-rgb) / var(--scrim)) 0%,
      rgb(var(--scrim-rgb) / var(--scrim)) 52%,
      rgb(var(--scrim-rgb) / 0) 100%
    );
    pointer-events: none;
  }

  .meta {
    position: absolute;
    inset: auto 0 0 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding: 0 var(--space-4) calc(var(--tabbar-h) + env(safe-area-inset-bottom) + var(--space-4));
    color: var(--ink);
  }

  .caption {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  h1 {
    font-size: var(--fs-xl);
    font-weight: 700;
    letter-spacing: -0.02em;
    line-height: 1.15;
  }

  .title {
    font-size: var(--fs-md);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .credit {
    font-size: var(--fs-xs);
    opacity: 0.92;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--space-2);
  }

  .act {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: var(--space-3) 2px;
    border: 1px solid var(--glass-border);
    border-radius: var(--radius-sm);
    background: var(--glass-bg);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    color: var(--ink);
    font-size: 11px;
    font-weight: 600;
    transition: transform 120ms var(--ease), background-color 200ms;
  }

  .act span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .act:active {
    transform: scale(0.96);
  }

  .act:disabled {
    opacity: 0.5;
  }

  .act.main {
    background: var(--accent);
    color: var(--on-accent);
    border-color: transparent;
  }

  .act.liked {
    background: #fff;
    color: #b3123f;
  }

  .empty {
    height: 100%;
    display: grid;
    place-items: center;
    padding: var(--space-4) var(--space-4) calc(var(--tabbar-h) + var(--space-4));
  }

  .empty .card {
    width: 100%;
    max-width: 420px;
    gap: var(--space-4);
    padding: var(--space-5);
  }

  .busy {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-4);
    background: color-mix(in srgb, #000 50%, transparent);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    color: #fff;
    font-weight: 600;
    animation: fade 200ms;
  }

  .spinner {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 3px solid rgb(255 255 255 / 0.3);
    border-top-color: #fff;
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  @keyframes reveal {
    from {
      opacity: 0;
      transform: scale(1.03);
    }
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
</style>
