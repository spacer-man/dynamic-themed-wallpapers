<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { rgbToHex } from '../lib/color';
  import { t } from '../lib/i18n.svelte';
  import type { Lang, Tone } from '../lib/types';
  import Segmented from '../lib/components/Segmented.svelte';
  import Toggle from '../lib/components/Toggle.svelte';

  const s = $derived(app.settings);
  const palette = $derived(app.wallpaper?.entry.palette ?? []);

  const tones = $derived<{ value: Tone; label: string }[]>([
    { value: 'any', label: t('look.tone.any') },
    { value: 'dark', label: t('look.tone.dark') },
    { value: 'light', label: t('look.tone.light') },
  ]);
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

  .swatch {
    min-width: 12px;
    border-radius: 6px;
  }
</style>
