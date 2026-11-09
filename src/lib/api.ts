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
  ScheduleStatus,
  ScreenSize,
  Settings,
  SourceId,
  SourceStatus,
  Subjects,
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
  searchPlaces: (query: string, lang: string) => call<PlaceHit[]>('search_places', { query, lang }),
  selectPlace: (qid: string, lang: string) => call<Place>('select_place', { qid, lang }),
  subjects: () => call<Subjects>('subjects'),
  addQuerySubject: (text: string) => call<Subjects>('add_query_subject', { text }),
  /** A Pinterest board address as pasted by the user; also switches the source to Pinterest. */
  addBoardSubject: (url: string) => call<Subjects>('add_board_subject', { url }),
  updateQuerySubject: (id: string, text: string) => call<Subjects>('update_query_subject', { id, text }),
  selectSubject: (id: string) => call<Subjects>('select_subject', { id }),
  deleteSubject: (id: string) => call<Subjects>('delete_subject', { id }),
  sourceStatuses: () => call<SourceStatus[]>('source_statuses'),
  checkSource: (source: SourceId) => call<SourceStatus>('check_source', { source }),
  /** `null` removes the key. The key is never read back. */
  setSourceKey: (source: SourceId, key: string | null) => call<void>('set_source_key', { source, key }),
  currentWallpaper: (dark: boolean) => call<Wallpaper | null>('current_wallpaper', { dark }),
  /** `source` is a one-time substitute for the chosen source; it is never saved. */
  changeNow: (systemDark: boolean, source?: SourceId) =>
    call<Wallpaper | null>('change_now', { systemDark, screen: screenSize(), source: source ?? null }),
  /** The screen a notification asked for (once), e.g. "sources". Always null outside Android. */
  launchRoute: () => call<string | null>('launch_route'),
  notificationsEnabled: () => call<boolean>('notifications_enabled'),
  /** Shows the system permission dialog on Android 13+; resolves with the state before the answer. */
  requestNotifications: () => call<boolean>('request_notifications'),
  /** Last daily run and next planned time. Empty outside Android. */
  scheduleStatus: () => call<ScheduleStatus>('schedule_status'),
  /** Opens the system screen where autostart / battery limits of the app can be lifted. */
  openBatterySettings: () => call<void>('open_battery_settings'),
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
