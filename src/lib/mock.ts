// In-memory stand-in for the Rust backend, used when the UI runs in a plain browser.
// Photos are generated SVG scenes labelled as samples. Real logic lives in crates/core.

import { blend, contrast, ensureContrast, hexToRgb, rgbToHex, WHITE, BLACK } from './color';
import type {
  CacheInfo,
  Candidate,
  ErrorKind,
  HistoryEntry,
  Place,
  PlaceHit,
  Rgb,
  Settings,
  Swatch,
  Theme,
  Wallpaper,
} from './types';

interface Scene {
  id: string;
  title: string;
  dark: boolean;
  sky: [string, string];
  sun: string;
  ridges: [string, string, string];
}

const SCENES: Scene[] = [
  { id: 's1', title: 'Dusk over the ridge', dark: true, sky: ['#1b1740', '#e0745a'], sun: '#ffd9a0', ridges: ['#4a2f6b', '#2c1d4a', '#150f2b'] },
  { id: 's2', title: 'Midnight bamboo', dark: true, sky: ['#04120f', '#1c4a3c'], sun: '#bfe8d2', ridges: ['#0f3a2c', '#0a2a20', '#04140f'] },
  { id: 's3', title: 'Neon street at night', dark: true, sky: ['#0a0d2e', '#b02a78'], sun: '#ffd0f0', ridges: ['#2a1450', '#1b0e38', '#0c0820'] },
  { id: 's4', title: 'Misty morning', dark: false, sky: ['#dfe8ee', '#f6efe6'], sun: '#fffaf0', ridges: ['#c3d0d8', '#aebdc7', '#93a5b1'] },
  { id: 's5', title: 'Spring blossom', dark: false, sky: ['#fde8ee', '#fff6f0'], sun: '#ffffff', ridges: ['#f3b9c8', '#e69ab1', '#cf7a97'] },
  { id: 's6', title: 'Sunlit shore', dark: false, sky: ['#bfe9f2', '#fdf3d6'], sun: '#fff7d6', ridges: ['#7fc6cf', '#e7d3a3', '#cdb27a'] },
];

function sceneImage(s: Scene, w = 540, h = 1200): string {
  const ridge = (y: number, amp: number, phase: number, color: string) => {
    let d = `M0 ${h}`;
    for (let x = 0; x <= w; x += 30) {
      d += ` L${x} ${(y + amp * Math.sin(x / 90 + phase) + amp * 0.5 * Math.sin(x / 37 + phase * 2)).toFixed(1)}`;
    }
    return `<path d="${d} L${w} ${h} Z" fill="${color}"/>`;
  };
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">` +
    `<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${s.sky[0]}"/><stop offset="0.8" stop-color="${s.sky[1]}"/></linearGradient>` +
    `<radialGradient id="r"><stop offset="0" stop-color="${s.sun}" stop-opacity="0.9"/><stop offset="1" stop-color="${s.sun}" stop-opacity="0"/></radialGradient></defs>` +
    `<rect width="${w}" height="${h}" fill="url(#g)"/>` +
    `<circle cx="${w * 0.66}" cy="${h * 0.4}" r="${w * 0.4}" fill="url(#r)"/>` +
    `<circle cx="${w * 0.66}" cy="${h * 0.4}" r="${w * 0.09}" fill="${s.sun}"/>` +
    ridge(h * 0.66, 38, 0.4, s.ridges[0]) +
    ridge(h * 0.76, 30, 2.0, s.ridges[1]) +
    ridge(h * 0.88, 22, 4.1, s.ridges[2]) +
    `</svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

function palette(s: Scene): Swatch[] {
  const rgb = (h: string) => hexToRgb(h);
  return [
    { rgb: rgb(s.sky[1]), weight: 0.4 },
    { rgb: rgb(s.sky[0]), weight: 0.25 },
    { rgb: rgb(s.ridges[1]), weight: 0.2 },
    { rgb: rgb(s.sun), weight: 0.1 },
    { rgb: rgb(s.ridges[2]), weight: 0.05 },
  ];
}

/** Simplified stand-in for core/theme.rs; the test below checks it keeps the contrast promises. */
export function mockTheme(p: Swatch[], dark: boolean): Theme {
  const base: Rgb = p[0]?.rgb ?? [60, 60, 80];
  const bg = blend(dark ? BLACK : WHITE, base, dark ? 0.82 : 0.9);
  const surface = blend(dark ? WHITE : BLACK, bg, dark ? 0.08 : 0.05);
  const accentSrc =
    [...p].sort((a, b) => chroma(b.rgb) * Math.sqrt(b.weight) - chroma(a.rgb) * Math.sqrt(a.weight))[0]?.rgb ?? base;
  const accent = ensureContrast(accentSrc, bg, 4.5);
  return {
    dark,
    background: rgbToHex(bg),
    surface: rgbToHex(surface),
    text: rgbToHex(ensureContrast(dark ? [245, 245, 250] : [24, 24, 32], bg, 7)),
    text_muted: rgbToHex(ensureContrast(dark ? [180, 180, 195] : [90, 90, 105], bg, 4.5)),
    accent: rgbToHex(accent),
    on_accent: contrast(WHITE, accent) >= contrast(BLACK, accent) ? '#ffffff' : '#000000',
  };
}

const chroma = (c: Rgb) => Math.max(...c) - Math.min(...c);

const PLACES: Place[] = [
  { qid: 'Q17', label: 'Japan', description: 'island country in East Asia', coords: { lat: 36.2, lon: 138.25 }, kind: 'country', area_km2: 377975, commons_category: 'Japan' },
  { qid: 'Q34600', label: 'Kyoto', description: 'city in Kyoto Prefecture, Japan', coords: { lat: 35.0116, lon: 135.7681 }, kind: 'city', area_km2: 827.8, commons_category: 'Kyoto' },
  { qid: 'Q39231', label: 'Mount Fuji', description: 'highest mountain in Japan', coords: { lat: 35.3606, lon: 138.7274 }, kind: 'natural', area_km2: null, commons_category: 'Mount Fuji' },
  { qid: 'Q1490', label: 'Tokyo', description: 'capital city of Japan', coords: { lat: 35.6895, lon: 139.6917 }, kind: 'city', area_km2: 2194, commons_category: 'Tokyo' },
  { qid: 'Q202197', label: 'Shibuya', description: 'special ward of Tokyo', coords: { lat: 35.6617, lon: 139.7041 }, kind: 'district', area_km2: 15.1, commons_category: 'Shibuya' },
  { qid: 'Q261932', label: 'Hōryū-ji', description: 'Buddhist temple in Nara Prefecture', coords: { lat: 34.6144, lon: 135.7342 }, kind: 'landmark', area_km2: null, commons_category: 'Hōryū-ji' },
];

const RU: Record<string, string> = { Japan: 'Япония', Kyoto: 'Киото', 'Mount Fuji': 'Фудзияма', Tokyo: 'Токио', Shibuya: 'Сибуя', 'Hōryū-ji': 'Хорю-дзи' };

interface MockDb {
  settings: Settings;
  place: Place | null;
  history: { id: number; shown_at: number; scene: string; liked: boolean | null }[];
  nextId: number;
}

const DEFAULT_SETTINGS: Settings = {
  tone: 'any',
  follow_system_theme: false,
  radius_km: null,
  change_time: '08:00',
  wifi_only: true,
  cache_mb: 150,
  unsplash_enabled: false,
  lang: 'auto',
};

const KEY = 'dtw-mock-db';

function load(): MockDb {
  try {
    const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(KEY) : null;
    if (raw) return JSON.parse(raw) as MockDb;
  } catch {
    /* fall through to defaults */
  }
  return { settings: { ...DEFAULT_SETTINGS }, place: null, history: [], nextId: 1 };
}

let db = load();

// Dev/screenshot helper: ?scene=s3&liked=1 starts with that wallpaper already shown (not persisted).
{
  const q = typeof location !== 'undefined' ? new URLSearchParams(location.search) : null;
  const scene = q?.get('scene');
  if (q && scene && SCENES.some((s) => s.id === scene)) {
    db = { settings: { ...DEFAULT_SETTINGS }, place: { ...PLACES[1]! }, nextId: 2, history: [] };
    db.history = [{ id: 1, shown_at: Math.floor(Date.now() / 1000), scene, liked: q.get('liked') === '1' ? true : null }];
    for (const [i, id] of ['s4', 's2', 's5', 's1'].entries()) {
      if (id !== scene) db.history.unshift({ id: 10 + i, shown_at: Math.floor(Date.now() / 1000) - 86400 * (i + 1), scene: id, liked: i % 2 === 0 ? true : false });
    }
  }
}
const save = () => {
  try {
    localStorage.setItem(KEY, JSON.stringify(db));
  } catch {
    /* private mode */
  }
};

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

function fail(kind: ErrorKind, message: string): never {
  throw { kind, message };
}

function candidate(s: Scene, place: Place): Candidate {
  return {
    source: 'commons',
    id: `mock:${s.id}`,
    title: s.title,
    description: null,
    page_url: 'https://commons.wikimedia.org/wiki/Special:Random',
    image_url: sceneImage(s),
    preview_url: sceneImage(s, 270, 600),
    width: 4000,
    height: 6000,
    author: 'Sample Author',
    author_url: null,
    license: { short_name: 'CC BY-SA 4.0 (sample)', url: 'https://creativecommons.org/licenses/by-sa/4.0', attribution_required: true },
    coords: place.coords,
    place_label: place.label,
  };
}

function toEntry(h: MockDb['history'][number]): HistoryEntry {
  const s = SCENES.find((x) => x.id === h.scene)!;
  return {
    id: h.id,
    shown_at: h.shown_at,
    candidate: candidate(s, db.place ?? PLACES[0]!),
    palette: palette(s),
    liked: h.liked,
  };
}

function wallpaper(dark: boolean): Wallpaper | null {
  const last = db.history[db.history.length - 1];
  if (!last) return null;
  const entry = toEntry(last);
  const s = SCENES.find((x) => x.id === last.scene)!;
  return { entry, theme: mockTheme(entry.palette, dark), image: sceneImage(s) };
}

export async function mockCall(cmd: string, a: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case 'get_settings':
      return db.settings;
    case 'save_settings': {
      db.settings = { ...DEFAULT_SETTINGS, ...(a.settings as Settings) };
      save();
      return db.settings;
    }
    case 'selected_place':
      return db.place;
    case 'search_places': {
      await sleep(350);
      const q = String(a.query).trim().toLowerCase();
      if (!q) return [];
      const ru = a.lang === 'ru';
      return PLACES.filter((p) => p.label.toLowerCase().includes(q) || (RU[p.label] ?? '').toLowerCase().includes(q)).map(
        (p): PlaceHit => ({ qid: p.qid, label: ru ? (RU[p.label] ?? p.label) : p.label, description: p.description }),
      );
    }
    case 'select_place': {
      await sleep(250);
      const p = PLACES.find((x) => x.qid === a.qid) ?? fail('not_found', 'unknown place');
      db.place = { ...p, label: a.lang === 'ru' ? (RU[p.label] ?? p.label) : p.label };
      save();
      return db.place;
    }
    case 'current_wallpaper':
      return wallpaper(Boolean(a.dark));
    case 'change_now': {
      if (!db.place) fail('config', 'no place selected yet');
      await sleep(900);
      const tone = db.settings.follow_system_theme ? (a.systemDark ? 'dark' : 'light') : db.settings.tone;
      const shown = new Set(db.history.map((h) => h.scene));
      const next =
        SCENES.find((s) => !shown.has(s.id) && (tone === 'any' || (tone === 'dark') === s.dark)) ??
        fail('not_found', 'no suitable photo');
      db.history.push({ id: db.nextId++, shown_at: Math.floor(Date.now() / 1000), scene: next.id, liked: null });
      save();
      return wallpaper(Boolean(a.systemDark));
    }
    case 'feedback': {
      const id = String(a.candidateId).replace('mock:', '');
      const h = [...db.history].reverse().find((x) => x.scene === id);
      if (h) h.liked = Boolean(a.liked);
      save();
      return null;
    }
    case 'history': {
      const all = [...db.history].reverse().map(toEntry);
      return all.slice(Number(a.offset), Number(a.offset) + Number(a.limit));
    }
    case 'cache_info':
      return { bytes: 41_000_000, budget_bytes: db.settings.cache_mb * 1024 * 1024, ready: 3 } satisfies CacheInfo;
    case 'clear_cache':
      return null;
    case 'forget_all':
      db.history = [];
      save();
      return null;
    case 'theme_for':
      return mockTheme(a.palette as Swatch[], Boolean(a.dark));
    default:
      return fail('internal', `mock: unknown command ${cmd}`);
  }
}

/** Test hook: start from a clean database. */
export function resetMock() {
  db = { settings: { ...DEFAULT_SETTINGS }, place: null, history: [], nextId: 1 };
}

export const MOCK_SCENES = SCENES;
export { palette as scenePalette };
