// Application state. One instance, shared by all screens.

import { api, inTauri } from './api';
import { paletteIsDark } from './color';
import { i18n } from './i18n.svelte';
import { AppError } from './types';
import type { CacheInfo, HistoryEntry, Place, Route, Settings, Theme, Wallpaper } from './types';

const HISTORY_PAGE = 60;
const ROUTES: Route[] = ['today', 'place', 'look', 'schedule', 'history'];
// The Android system recreates the WebView when the wallpaper changes (and in a few other cases).
// Remembering where the user was and the last theme makes that invisible.
const ROUTE_KEY = 'dtw-route';
const THEME_KEY = 'dtw-theme';

class App {
  route = $state<Route>('today');
  settings = $state<Settings | null>(null);
  place = $state<Place | null>(null);
  wallpaper = $state<Wallpaper | null>(null);
  history = $state<HistoryEntry[]>([]);
  cache = $state<CacheInfo | null>(null);
  /** A wallpaper is being picked (network + image work), can take several seconds. */
  busy = $state(false);
  error = $state<AppError | null>(null);
  systemDark = $state(false);
  ready = $state(false);
  /** Theme for the current photo; recomputed when the dark/light variant flips. */
  theme = $state<Theme | null>(null);

  /** Dark UI? Follows the system if asked to, otherwise the photo's own tone. */
  get dark(): boolean {
    if (this.settings?.follow_system_theme || !this.wallpaper) return this.systemDark;
    return paletteIsDark(this.wallpaper.entry.palette);
  }

  async init() {
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    this.systemDark = mq.matches;
    // Browser-only helper for design work: ?route=look
    const r = !inTauri ? new URLSearchParams(location.search).get('route') : null;
    const saved = inTauri ? loadRoute() : null;
    if (saved) this.go(saved);
    if (r && (ROUTES as string[]).includes(r)) this.go(r as Route);
    mq.addEventListener('change', (e) => (this.systemDark = e.matches));
    try {
      this.settings = await api.getSettings();
      i18n.choice = this.settings.lang;
      this.place = await api.selectedPlace();
      this.wallpaper = await api.currentWallpaper(this.systemDark);
      this.theme = this.wallpaper?.theme ?? null;
      // The first guess used the system variant; fix it before anything is shown (no flash).
      if (this.theme && this.theme.dark !== this.dark) await this.retheme();
    } catch (e) {
      this.fail(e);
    } finally {
      this.ready = true;
    }
  }

  fail(e: unknown) {
    this.error = e instanceof AppError ? e : new AppError('internal', String(e));
  }

  go(route: Route) {
    this.route = route;
    try {
      localStorage.setItem(ROUTE_KEY, route);
    } catch {
      // storage unavailable: just not remembered
    }
    if (route === 'history') void this.loadHistory();
    if (route === 'schedule') void this.loadCache();
  }

  /** Re-theme after the dark/light variant changed (cheap: palette in, colours out). */
  async retheme() {
    const w = this.wallpaper;
    if (!w) return;
    try {
      this.theme = await api.themeFor(w.entry.palette, this.dark);
    } catch (e) {
      this.fail(e);
    }
  }

  async changeNow() {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    try {
      const w = await api.changeNow(this.systemDark);
      if (w) {
        this.wallpaper = w;
        this.theme = w.theme;
        // The theme variant may differ now that the photo (and thus `dark`) changed.
        if (w.theme.dark !== this.dark) await this.retheme();
      }
    } catch (e) {
      this.fail(e);
    } finally {
      this.busy = false;
    }
  }

  async like() {
    await this.rate(true);
  }

  /** "Not for me": remember it, then move on to the next photo. */
  async skip() {
    if (await this.rate(false)) await this.changeNow();
  }

  private async rate(liked: boolean): Promise<boolean> {
    const w = this.wallpaper;
    if (!w) return false;
    try {
      await api.feedback(w.entry.candidate.id, liked);
      w.entry.liked = liked;
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  async saveSettings(patch: Partial<Settings>) {
    if (!this.settings) return;
    const before = this.settings;
    this.settings = { ...before, ...patch };
    try {
      this.settings = await api.saveSettings(this.settings);
      if (patch.lang) i18n.choice = this.settings.lang;
    } catch (e) {
      this.settings = before;
      this.fail(e);
    }
  }

  /** Save a caption setting and put the current wallpaper on the screen again with it. */
  async saveCaption(patch: Partial<Pick<Settings, 'caption_enabled' | 'caption_y' | 'caption_align'>>) {
    await this.saveSettings(patch);
    try {
      await api.reapplyWallpaper();
    } catch (e) {
      this.fail(e);
    }
  }

  async selectPlace(qid: string) {
    try {
      this.place = await api.selectPlace(qid, i18n.lang);
      this.settings = await api.getSettings();
      this.route = 'today';
    } catch (e) {
      this.fail(e);
    }
  }

  async loadHistory() {
    try {
      this.history = await api.history(HISTORY_PAGE, 0);
    } catch (e) {
      this.fail(e);
    }
  }

  async loadCache() {
    try {
      this.cache = await api.cacheInfo();
    } catch (e) {
      this.fail(e);
    }
  }

  async clearCache() {
    try {
      await api.clearCache();
      await this.loadCache();
    } catch (e) {
      this.fail(e);
    }
  }

  async forgetAll() {
    try {
      await api.forgetAll();
      this.history = [];
      this.wallpaper = null;
      this.theme = null;
      await this.loadCache();
    } catch (e) {
      this.fail(e);
    }
  }
}

export const app = new App();

/** Write the theme into CSS custom properties; `null` restores the stylesheet defaults. */
export function applyTheme(theme: Theme | null, dark: boolean) {
  const root = document.documentElement;
  root.style.colorScheme = dark ? 'dark' : 'light';
  const vars: Record<string, string | undefined> = {
    '--bg': theme?.background,
    '--surface': theme?.surface,
    '--text': theme?.text,
    '--muted': theme?.text_muted,
    '--accent': theme?.accent,
    '--on-accent': theme?.on_accent,
  };
  for (const [k, v] of Object.entries(vars)) {
    if (v) root.style.setProperty(k, v);
    else root.style.removeProperty(k);
  }
  root.dataset.theme = dark ? 'dark' : 'light';
  if (theme) {
    try {
      localStorage.setItem(THEME_KEY, JSON.stringify({ theme, dark }));
    } catch {
      // not remembered
    }
  }
}

function loadRoute(): Route | null {
  try {
    const r = localStorage.getItem(ROUTE_KEY);
    return r && (ROUTES as string[]).includes(r) ? (r as Route) : null;
  } catch {
    return null;
  }
}

/** Paint the last known theme before the first render, so a reload does not flash default colours. */
export function restoreTheme() {
  try {
    const raw = localStorage.getItem(THEME_KEY);
    if (!raw) return;
    const { theme, dark } = JSON.parse(raw) as { theme: Theme; dark: boolean };
    applyTheme(theme, dark);
  } catch {
    // nothing remembered or unreadable: the stylesheet defaults apply
  }
}
