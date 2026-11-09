<script lang="ts">
  import { onMount } from 'svelte';
  import { app, applyTheme } from './lib/app.svelte';
  import { inTauri } from './lib/api';
  import ErrorBanner from './lib/components/ErrorBanner.svelte';
  import TabBar from './lib/components/TabBar.svelte';
  import Toast from './lib/components/Toast.svelte';
  import History from './routes/History.svelte';
  import Look from './routes/Look.svelte';
  import PlaceScreen from './routes/PlaceScreen.svelte';
  import Schedule from './routes/Schedule.svelte';
  import Sources from './routes/Sources.svelte';
  import Today from './routes/Today.svelte';

  onMount(() => {
    // Android: MainActivity's back callback calls this through evaluateJavascript; 'exit' makes it
    // finish the activity. Anything else (or this function missing) is handled by Kotlin safely.
    (window as unknown as { __dtwBack?: () => string }).__dtwBack = () => (app.pressBack() === 'exit' ? 'exit' : 'handled');
    return () => {
      delete (window as unknown as { __dtwBack?: () => string }).__dtwBack;
    };
  });
  onMount(() => app.init());

  // Browser mock: Escape stands in for the back gesture.
  function onKey(e: KeyboardEvent) {
    if (!inTauri && e.key === 'Escape') app.pressBack();
  }

  // Paint the palette-derived colours whenever the theme or its light/dark variant changes.
  $effect(() => applyTheme(app.theme, app.dark));

  // The variant can flip without a new photo (system theme change): refetch the colours.
  $effect(() => {
    if (app.ready && app.wallpaper && app.theme && app.theme.dark !== app.dark) void app.retheme();
  });
</script>

<svelte:window onkeydown={onKey} />

{#if app.ready}
  <main>
    {#if app.route === 'today'}<Today />
    {:else if app.route === 'place'}<PlaceScreen />
    {:else if app.route === 'sources'}<Sources />
    {:else if app.route === 'look'}<Look />
    {:else if app.route === 'schedule'}<Schedule />
    {:else}<History />{/if}
  </main>
  <TabBar />
  <ErrorBanner />
  <Toast />
{/if}

<style>
  main {
    height: 100%;
  }
</style>
