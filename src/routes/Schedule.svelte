<script lang="ts">
  import { app } from '../lib/app.svelte';
  import { t, i18n } from '../lib/i18n.svelte';
  import Toggle from '../lib/components/Toggle.svelte';
  import { api } from '../lib/api';
  import { describeSchedule } from '../lib/schedule-status';
  import type { ScheduleStatus } from '../lib/types';

  const s = $derived(app.settings);
  const cache = $derived(app.cache);
  let confirmForget = $state(false);
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  let status = $state<ScheduleStatus | null>(null);
  const info = $derived(status ? describeSchedule(status, Date.now()) : null);

  $effect(() => {
    void s?.change_time; // look again after the schedule is changed
    void s?.schedule_enabled;
    api
      .scheduleStatus()
      .then((v) => (status = v))
      .catch(() => (status = null));
  });

  const when = (ms: number) => new Date(ms).toLocaleString(i18n.lang, { dateStyle: 'short', timeStyle: 'short' });

  const mb = (bytes: number) => `${Math.round(bytes / 1024 / 1024)} MB`;
  const usedPct = $derived(cache && cache.budget_bytes > 0 ? Math.min(100, (cache.bytes / cache.budget_bytes) * 100) : 0);

  // Two taps to erase: the first arms the button for 4 seconds.
  function forget() {
    if (!confirmForget) {
      confirmForget = true;
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmForget = false), 4000);
      return;
    }
    clearTimeout(confirmTimer);
    confirmForget = false;
    void app.forgetAll();
  }
</script>

<section class="screen">
  <h1>{t('tab.schedule')}</h1>

  {#if s}
    <div class="card">
      <Toggle
        checked={s.schedule_enabled}
        label={t('schedule.enabled')}
        hint={t('schedule.enabled.hint')}
        onchange={(v) => app.saveSettings({ schedule_enabled: v })}
      />
      <div class="rows" class:off={!s.schedule_enabled} inert={!s.schedule_enabled}>
      <label class="row">
        <span class="label">{t('schedule.time')}</span>
        <input
          type="time"
          value={s.change_time}
          onchange={(e) => e.currentTarget.value && app.saveSettings({ change_time: e.currentTarget.value })}
        />
      </label>
      <Toggle
        checked={s.wifi_only}
        label={t('schedule.wifi')}
        hint={t('schedule.wifi.hint')}
        onchange={(v) => app.saveSettings({ wifi_only: v })}
      />
      <span class="hint">{t('schedule.android')}</span>
      {#if info}
        {#if info.last}
          <span class="hint">
            {t('schedule.status.last', { when: when(info.last.at), result: t(`schedule.status.${info.last.outcome}`) })}
          </span>
          {#if info.last.message}<span class="hint">{info.last.message}</span>{/if}
        {:else}
          <span class="hint">{t('schedule.status.never')}</span>
        {/if}
        {#if info.next}
          <span class="hint">{t(info.overdue ? 'schedule.status.overdue' : 'schedule.status.next', { when: when(info.next) })}</span>
        {/if}
      {/if}
      </div>
      {#if app.notifications}
        <span class="hint">{t('schedule.notify.on')}</span>
      {:else}
        <span class="hint">{t('schedule.notify.off')}</span>
        <button class="btn" onclick={() => app.askNotifications()}>{t('schedule.notify.allow')}</button>
      {/if}
    </div>

    <div class="card">
      <span class="hint">{t('schedule.battery.hint')}</span>
      <button class="btn" onclick={() => void api.openBatterySettings().catch(() => {})}>{t('schedule.battery.open')}</button>
    </div>

    <div class="card">
      <label class="stack">
        <span class="row">
          <span class="label">{t('schedule.cache')}</span>
          <output>{s.cache_mb} MB</output>
        </span>
        <input
          type="range"
          min="50"
          max="1000"
          step="50"
          value={s.cache_mb}
          onchange={(e) => app.saveSettings({ cache_mb: Number(e.currentTarget.value) })}
        />
      </label>
      {#if cache}
        <div class="bar" aria-hidden="true"><div style:width="{usedPct}%"></div></div>
        <span class="hint">
          {t('schedule.cache.used', { used: mb(cache.bytes), budget: mb(cache.budget_bytes), ready: cache.ready })}
        </span>
      {/if}
      <button class="btn" onclick={() => app.clearCache()}>{t('schedule.clear')}</button>
    </div>

    <div class="card">
      <button class="btn danger" onclick={forget}>
        {confirmForget ? t('schedule.forget.confirm') : t('schedule.forget')}
      </button>
      <span class="hint">{t('schedule.forget.hint')}</span>
    </div>
  {/if}
</section>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    min-height: 44px;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .rows.off {
    opacity: 0.45;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  input[type='time'] {
    min-height: 44px;
    padding: 0 var(--space-3);
    border-radius: var(--radius-sm);
    border: 1px solid var(--hairline);
    background: var(--bg);
  }

  input[type='range'] {
    accent-color: var(--accent);
  }

  output {
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .bar {
    height: 8px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text) 10%, transparent);
    overflow: hidden;
  }

  .bar div {
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width 300ms var(--ease);
  }

  .btn.danger {
    white-space: normal;
    text-align: center;
  }
</style>
