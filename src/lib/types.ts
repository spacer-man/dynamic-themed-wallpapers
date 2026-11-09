// Mirrors the serde output of the Rust types in crates/core (snake_case on the wire).

export type Tone = 'any' | 'dark' | 'light';
export type Lang = 'auto' | 'ru' | 'en';
export type CaptionAlign = 'left' | 'center' | 'right';
export type Route = 'today' | 'place' | 'sources' | 'look' | 'schedule' | 'history';

/** Where photos come from. Same strings as `SourceKind` in crates/core. */
export type SourceId = 'commons' | 'openverse' | 'wallhaven' | 'unsplash' | 'pexels' | 'pixabay' | 'pinterest';

export interface Settings {
  tone: Tone;
  follow_system_theme: boolean;
  radius_km: number | null;
  /** "HH:MM", local time. */
  change_time: string;
  /** The automatic daily change runs at all. */
  schedule_enabled: boolean;
  wifi_only: boolean;
  cache_mb: number;
  /** The single source photos come from (sources are never mixed). */
  source: SourceId;
  lang: Lang;
  /** Draw the place name and credit on the wallpaper. */
  caption_enabled: boolean;
  /** Vertical centre of the caption, percent of the screen height (3 top … 97 bottom). */
  caption_y: number;
  caption_align: CaptionAlign;
}

export interface GeoPoint {
  lat: number;
  lon: number;
}

export type PlaceKind = 'country' | 'region' | 'city' | 'district' | 'landmark' | 'natural' | 'other';

export interface PlaceHit {
  qid: string;
  label: string;
  description: string | null;
}

export interface Place extends PlaceHit {
  coords: GeoPoint | null;
  kind: PlaceKind;
  area_km2: number | null;
  commons_category: string | null;
}

/** A saved thing to show: a place, free text, or a Pinterest board (served by Pinterest only). */
export type Subject = { id: string } & (
  | { kind: 'place'; place: Place }
  | { kind: 'query'; text: string }
  | { kind: 'board'; url: string; name: string }
);

export const subjectName = (s: Subject): string =>
  s.kind === 'place' ? s.place.label : s.kind === 'query' ? s.text : s.name;

export interface Subjects {
  items: Subject[];
  /** Id of the active subject. */
  active: string | null;
}

export type SourceState = 'available' | 'needs_key' | 'unreachable' | 'unsupported';

export interface SourceStatus {
  source: SourceId;
  state: SourceState;
  needs_key: boolean;
  has_key: boolean;
  active: boolean;
  /** Last error, when the last request failed. */
  error: string | null;
  /** Unix seconds of the last request. */
  checked_at: number | null;
}

export type Rgb = [number, number, number];

export interface Swatch {
  rgb: Rgb;
  weight: number;
}

export interface License {
  short_name: string;
  url: string | null;
  attribution_required: boolean;
}

export interface Candidate {
  source: SourceId;
  id: string;
  title: string;
  description: string | null;
  page_url: string;
  image_url: string;
  preview_url: string;
  width: number;
  height: number;
  author: string;
  author_url: string | null;
  license: License;
  coords: GeoPoint | null;
  place_label: string;
}

export interface HistoryEntry {
  id: number;
  shown_at: number;
  candidate: Candidate;
  palette: Swatch[];
  /** true liked, false skipped, null no feedback yet. */
  liked: boolean | null;
}

export interface Theme {
  dark: boolean;
  background: string;
  surface: string;
  text: string;
  text_muted: string;
  accent: string;
  on_accent: string;
}

export interface Wallpaper {
  entry: HistoryEntry;
  theme: Theme;
  /** data: URL of the rendered JPEG. */
  image: string | null;
}

export interface CacheInfo {
  bytes: number;
  budget_bytes: number;
  ready: number;
}

/** What the last daily run did and when the next is planned (epoch milliseconds, 0 = never). */
export interface ScheduleStatus {
  last_at: number;
  /** "queued", "changed", "retry" or "failed"; empty before the first run. */
  outcome: string;
  message: string;
  next_at: number;
}

export interface ScreenSize {
  width: number;
  height: number;
}

export type ErrorKind = 'network' | 'refused' | 'not_found' | 'config' | 'parse' | 'image' | 'storage' | 'internal';

export class AppError extends Error {
  constructor(
    public kind: ErrorKind,
    message: string,
  ) {
    super(message);
  }
}
