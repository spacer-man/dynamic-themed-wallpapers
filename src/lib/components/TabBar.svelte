<script lang="ts">
  import { app } from '../app.svelte';
  import { t } from '../i18n.svelte';
  import type { Route } from '../types';
  import Icon from './Icon.svelte';

  const tabs: { route: Route; icon: 'today' | 'place' | 'look' | 'schedule' | 'history' }[] = [
    { route: 'today', icon: 'today' },
    { route: 'place', icon: 'place' },
    { route: 'look', icon: 'look' },
    { route: 'schedule', icon: 'schedule' },
    { route: 'history', icon: 'history' },
  ];
</script>

<nav class="tabbar" aria-label="Main">
  {#each tabs as tab (tab.route)}
    <button
      class="tab"
      class:active={app.route === tab.route}
      aria-current={app.route === tab.route ? 'page' : undefined}
      onclick={() => app.go(tab.route)}
    >
      <Icon name={tab.icon} size={22} />
      <span>{t(`tab.${tab.route}`)}</span>
    </button>
  {/each}
</nav>

<style>
  .tabbar {
    position: fixed;
    inset: auto 0 0 0;
    z-index: 10;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    height: calc(var(--tabbar-h) + env(safe-area-inset-bottom));
    padding-bottom: env(safe-area-inset-bottom);
    background: var(--glass);
    backdrop-filter: blur(18px) saturate(1.4);
    -webkit-backdrop-filter: blur(18px) saturate(1.4);
    border-top: 1px solid var(--hairline);
  }

  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    transition: color 200ms;
  }

  .tab.active {
    color: var(--accent);
  }

  .tab span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding: 0 2px;
  }
</style>
