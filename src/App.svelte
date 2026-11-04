<script lang="ts">
  import { onMount } from 'svelte';
  import { app, applyTheme } from './lib/app.svelte';
  import ErrorBanner from './lib/components/ErrorBanner.svelte';
  import TabBar from './lib/components/TabBar.svelte';
  import History from './routes/History.svelte';
  import Look from './routes/Look.svelte';
  import PlaceScreen from './routes/PlaceScreen.svelte';
  import Schedule from './routes/Schedule.svelte';
  import Today from './routes/Today.svelte';

  onMount(() => app.init());

  // Paint the palette-derived colours whenever the theme or its light/dark variant changes.
  $effect(() => applyTheme(app.theme, app.dark));

  // The variant can flip without a new photo (system theme change): refetch the colours.
  $effect(() => {
    if (app.ready && app.wallpaper && app.theme && app.theme.dark !== app.dark) void app.retheme();
  });
</script>

{#if app.ready}
  <main>
    {#if app.route === 'today'}<Today />
    {:else if app.route === 'place'}<PlaceScreen />
    {:else if app.route === 'look'}<Look />
    {:else if app.route === 'schedule'}<Schedule />
    {:else}<History />{/if}
  </main>
  <TabBar />
  <ErrorBanner />
{/if}

<style>
  main {
    height: 100%;
  }
</style>
