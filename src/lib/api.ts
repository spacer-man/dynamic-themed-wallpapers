// The only module that talks to the backend. Inside Tauri it calls the Rust commands; in a plain
// browser (bun run dev, screenshots, design work) it falls back to an in-memory mock.

import { invoke } from '@tauri-apps/api/core';
import { AppError } from './types';
import type {
  CacheInfo,
  ErrorKind,
  HistoryEntry,
  Place,
  PlaceHit,
  ScreenSize,
  Settings,
  Swatch,
  Theme,
  Wallpaper,
} from './types';

export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

function toAppError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'kind' in e && 'message' in e) {
    const { kind, message } = e as { kind: ErrorKind; message: string };
    return new AppError(kind, message);
  }
  return new AppError('internal', String(e));
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    if (inTauri) return await invoke<T>(cmd, args);
    const { mockCall } = await import('./mock');
    return (await mockCall(cmd, args ?? {})) as T;
  } catch (e) {
    throw toAppError(e);
  }
}

/** Portrait size of this device's screen in physical pixels (desktop: a monitor, rotated). */
export function screenSize(): ScreenSize {
  const dpr = window.devicePixelRatio || 1;
  const w = Math.round(screen.width * dpr);
  const h = Math.round(screen.height * dpr);
  return { width: Math.min(w, h), height: Math.max(w, h) };
}

export const api = {
  getSettings: () => call<Settings>('get_settings'),
  saveSettings: (settings: Settings) => call<Settings>('save_settings', { settings }),
  selectedPlace: () => call<Place | null>('selected_place'),
  searchPlaces: (query: string, lang: string) => call<PlaceHit[]>('search_places', { query, lang }),
  selectPlace: (qid: string, lang: string) => call<Place>('select_place', { qid, lang }),
  currentWallpaper: (dark: boolean) => call<Wallpaper | null>('current_wallpaper', { dark }),
  changeNow: (systemDark: boolean) =>
    call<Wallpaper | null>('change_now', { systemDark, screen: screenSize() }),
  applyFromHistory: (historyId: number, systemDark: boolean) =>
    call<Wallpaper | null>('apply_from_history', { historyId, systemDark, screen: screenSize() }),
  reapplyWallpaper: () => call<boolean>('reapply_wallpaper'),
  feedback: (candidateId: string, liked: boolean) => call<void>('feedback', { candidateId, liked }),
  history: (limit: number, offset: number) => call<HistoryEntry[]>('history', { limit, offset }),
  cacheInfo: () => call<CacheInfo>('cache_info'),
  clearCache: () => call<void>('clear_cache'),
  forgetAll: () => call<void>('forget_all'),
  themeFor: (palette: Swatch[], dark: boolean) => call<Theme>('theme_for', { palette, dark }),
};
