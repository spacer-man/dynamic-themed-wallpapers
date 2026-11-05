<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { rgbToHex } from '../lib/color';
  import { t } from '../lib/i18n.svelte';
  import type { CaptionAlign, Lang, Tone } from '../lib/types';
  import CaptionPreview from '../lib/components/CaptionPreview.svelte';
  import Segmented from '../lib/components/Segmented.svelte';
  import Toggle from '../lib/components/Toggle.svelte';

  const s = $derived(app.settings);
  const palette = $derived(app.wallpaper?.entry.palette ?? []);

  const tones = $derived<{ value: Tone; label: string }[]>([
    { value: 'any', label: t('look.tone.any') },
    { value: 'dark', label: t('look.tone.dark') },
    { value: 'light', label: t('look.tone.light') },
  ]);
  const aligns = $derived<{ value: CaptionAlign; label: string }[]>([
    { value: 'left', label: t('look.align.left') },
    { value: 'center', label: t('look.align.center') },
    { value: 'right', label: t('look.align.right') },
  ]);

  // While the slider is dragged only the preview moves; the wallpaper is redone on release.
  let draftY = $state<number | null>(null);
  const shownY = $derived(draftY ?? s?.caption_y ?? 94);

  const langs = $derived<{ value: Lang; label: string }[]>([
    { value: 'auto', label: t('look.lang.auto') },
    { value: 'ru', label: 'Русский' },
    { value: 'en', label: 'English' },
  ]);
</script>

<section class="screen">
  <h1>{t('tab.look')}</h1>

  {#if s}
    <div class="card">
      <span class="label">{t('look.tone')}</span>
      <Segmented
        options={tones}
        value={s.tone}
        disabled={s.follow_system_theme}
        onchange={(tone) => app.saveSettings({ tone })}
      />
      <span class="hint">{t('look.tone.hint')}</span>
    </div>

    <div class="card">
      <Toggle
        checked={s.follow_system_theme}
        label={t('look.follow')}
        hint={t('look.follow.hint')}
        onchange={(v) => app.saveSettings({ follow_system_theme: v })}
      />
    </div>

    {#if palette.length > 0}
      <div class="card">
        <span class="label">{t('look.palette')}</span>
        <div class="palette">
          {#each palette as sw (sw.rgb.join(','))}
            <div class="swatch" style:flex={Math.max(sw.weight, 0.08)} style:background={rgbToHex(sw.rgb)} title={rgbToHex(sw.rgb)}></div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="card">
      <span class="label">{t('look.caption')}</span>
      <Toggle
        checked={s.caption_enabled}
        label={t('look.caption.show')}
        hint={t('look.caption.show.hint')}
        onchange={(v) => app.saveCaption({ caption_enabled: v })}
      />
      {#if s.caption_enabled}
        <div class="caption-editor">
          <CaptionPreview image={app.wallpaper?.image ?? null} y={shownY} align={s.caption_align} enabled />
          <div class="controls">
            <label class="slider">
              <span class="label">{t('look.caption.position')}</span>
              <input
                type="range"
                min="3"
                max="97"
                step="1"
                value={shownY}
                aria-label={t('look.caption.position')}
                oninput={(e) => (draftY = e.currentTarget.valueAsNumber)}
                onchange={async (e) => {
                  const y = e.currentTarget.valueAsNumber;
                  await app.saveCaption({ caption_y: y });
                  draftY = null;
                }}
              />
              <span class="ends"><span>{t('look.caption.top')}</span><span>{t('look.caption.bottom')}</span></span>
              <span class="hint">{t('look.caption.position.hint')}</span>
            </label>
          </div>
        </div>
        <div class="align">
          <span class="label">{t('look.align')}</span>
          <Segmented
            options={aligns}
            value={s.caption_align}
            onchange={(caption_align) => app.saveCaption({ caption_align })}
          />
        </div>
      {/if}
    </div>

    <div class="card">
      <span class="label">{t('look.language')}</span>
      <Segmented options={langs} value={s.lang} onchange={(lang) => app.saveSettings({ lang })} />
    </div>
  {/if}
</section>

<style>
  .palette {
    display: flex;
    height: 48px;
    gap: 3px;
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .caption-editor {
    display: flex;
    gap: var(--space-4);
    align-items: flex-start;
  }

  .controls {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .slider {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .slider input {
    width: 100%;
    min-height: 44px;
    accent-color: var(--accent);
  }

  .ends {
    display: flex;
    justify-content: space-between;
    font-size: var(--fs-xs);
    color: var(--muted);
  }

  .align {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .swatch {
    min-width: 12px;
    border-radius: 6px;
  }
</style>
