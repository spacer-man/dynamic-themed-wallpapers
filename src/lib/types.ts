// Mirrors the serde output of the Rust types in crates/core (snake_case on the wire).

export type Tone = 'any' | 'dark' | 'light';
export type Lang = 'auto' | 'ru' | 'en';
export type Route = 'today' | 'place' | 'look' | 'schedule' | 'history';

export interface Settings {
  tone: Tone;
  follow_system_theme: boolean;
  radius_km: number | null;
  /** "HH:MM", local time. */
  change_time: string;
  wifi_only: boolean;
  cache_mb: number;
  unsplash_enabled: boolean;
  lang: Lang;
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
  source: 'commons' | 'unsplash';
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

export interface ScreenSize {
  width: number;
  height: number;
}

export type ErrorKind = 'network' | 'not_found' | 'config' | 'parse' | 'image' | 'storage' | 'internal';

export class AppError extends Error {
  constructor(
    public kind: ErrorKind,
    message: string,
  ) {
    super(message);
  }
}
