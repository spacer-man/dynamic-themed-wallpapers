// Application state. One instance, shared by all screens.

import { api, inTauri } from './api';
import { paletteIsDark } from './color';
import { i18n } from './i18n.svelte';
import { AppError } from './types';
import type {
  CacheInfo,
  HistoryEntry,
  Place,
  Route,
  Settings,
  SourceId,
  SourceStatus,
  Subject,
  Subjects,
  Theme,
  Wallpaper,
} from './types';

const HISTORY_PAGE = 60;
const ROUTES: Route[] = ['today', 'place', 'sources', 'look', 'schedule', 'history'];
// The Android system recreates the WebView when the wallpaper changes (and in a few other cases).
// Remembering where the user was and the last theme makes that invisible.
const ROUTE_KEY = 'dtw-route';
const THEME_KEY = 'dtw-theme';

class App {
  route = $state<Route>('today');
  settings = $state<Settings | null>(null);
  subjects = $state<Subjects>({ items: [], active: null });
  sources = $state<SourceStatus[]>([]);
  /** Sources being checked right now. */
  checking = $state<SourceId[]>([]);
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

  /** The subject photos are searched for, if one is selected. */
  get activeSubject(): Subject | null {
    return this.subjects.items.find((s) => s.id === this.subjects.active) ?? null;
  }

  /** Short name of the active subject: the place label or the search text. */
  get activeLabel(): string {
    const s = this.activeSubject;
    return s ? (s.kind === 'place' ? s.place.label : s.text) : '';
  }

  /** The active subject's place, when it is one. */
  get place(): Place | null {
    const s = this.activeSubject;
    return s?.kind === 'place' ? s.place : null;
  }

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
      this.subjects = await api.subjects();
      this.sources = await api.sourceStatuses();
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
    if (route === 'sources' || route === 'place') void this.loadSources();
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

  /** Set a photo from the history as the wallpaper again. Returns whether it worked. */
  async applyFromHistory(historyId: number): Promise<boolean> {
    if (this.busy) return false;
    this.busy = true;
    this.error = null;
    try {
      const w = await api.applyFromHistory(historyId, this.systemDark);
      if (w) {
        this.wallpaper = w;
        this.theme = w.theme;
        if (w.theme.dark !== this.dark) await this.retheme();
        await this.loadHistory();
      }
      return true;
    } catch (e) {
      this.fail(e);
      return false;
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
      await api.selectPlace(qid, i18n.lang);
      this.subjects = await api.subjects();
      this.settings = await api.getSettings();
      void this.loadSources();
      this.route = 'today';
    } catch (e) {
      this.fail(e);
    }
  }

  /** Run a subject call and keep the new list; the queue changed, so statuses may have too. */
  private async subjectCall(call: () => Promise<Subjects>): Promise<boolean> {
    try {
      this.subjects = await call();
      void this.loadSources();
      return true;
    } catch (e) {
      this.fail(e);
      return false;
    }
  }

  addQuery(text: string) {
    return this.subjectCall(() => api.addQuerySubject(text));
  }

  updateQuery(id: string, text: string) {
    return this.subjectCall(() => api.updateQuerySubject(id, text));
  }

  selectSubject(id: string) {
    return this.subjectCall(() => api.selectSubject(id));
  }

  deleteSubject(id: string) {
    return this.subjectCall(() => api.deleteSubject(id));
  }

  async loadSources() {
    try {
      this.sources = await api.sourceStatuses();
    } catch (e) {
      this.fail(e);
    }
  }

  /** One lightweight request to the source; its status is updated in place. */
  async checkSource(id: SourceId) {
    if (this.checking.includes(id)) return;
    this.checking = [...this.checking, id];
    try {
      const st = await api.checkSource(id);
      this.sources = this.sources.map((s) => (s.source === id ? st : s));
    } catch (e) {
      this.fail(e);
    } finally {
      this.checking = this.checking.filter((x) => x !== id);
    }
  }

  /** Refresh button: check every source that can be checked, one after another (be gentle). */
  async checkAll() {
    for (const s of this.sources) {
      if (!s.needs_key || s.has_key) await this.checkSource(s.source);
    }
  }

  /** Choose the source photos come from. */
  async selectSource(id: SourceId) {
    await this.saveSettings({ source: id });
    await this.loadSources();
  }

  /** Save or (with `null`) remove a key. Returns false when the key looks malformed. */
  async setSourceKey(id: SourceId, key: string | null): Promise<boolean> {
    try {
      await api.setSourceKey(id, key);
      await this.loadSources();
      return true;
    } catch (e) {
      if (e instanceof AppError && e.kind === 'config') return false;
      this.fail(e);
      return false;
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
