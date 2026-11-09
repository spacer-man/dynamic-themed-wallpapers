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
  SourceId,
  SourceState,
  SourceStatus,
  Subject,
  Subjects,
  Swatch,
  Theme,
  Wallpaper,
} from './types';
import { subjectName } from './types';

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

/** What the mock remembers per source: a key and the outcome of the last request. */
interface MockSource {
  key: string | null;
  error: string | null;
  checked_at: number | null;
}

interface MockDb {
  settings: Settings;
  subjects: Subjects;
  sources: Partial<Record<SourceId, MockSource>>;
  history: { id: number; shown_at: number; scene: string; liked: boolean | null }[];
  nextId: number;
  /** The source chosen before a Pinterest board took over; restored when the board is left. */
  before_board?: SourceId;
}

const DEFAULT_SETTINGS: Settings = {
  tone: 'any',
  follow_system_theme: false,
  radius_km: null,
  change_time: '08:00',
  schedule_enabled: true,
  wifi_only: true,
  cache_mb: 150,
  source: 'commons',
  lang: 'auto',
  caption_enabled: true,
  caption_y: 94,
  caption_align: 'left',
};

const KEY = 'dtw-mock-db';

const EMPTY_SUBJECTS = (): Subjects => ({ items: [], active: null });

const NEEDS_KEY: SourceId[] = ['unsplash', 'pexels', 'pixabay'];

/** Databases saved by earlier versions had `place` and `unsplash_enabled`; bring them forward. */
export function migrateDb(raw: Record<string, unknown>): MockDb {
  const old = raw as Partial<MockDb> & { place?: Place | null; settings?: Record<string, unknown> };
  const settings = { ...DEFAULT_SETTINGS, ...(old.settings as Partial<Settings>) } as Settings & {
    unsplash_enabled?: boolean;
  };
  if (settings.unsplash_enabled && !(old.settings && 'source' in old.settings)) settings.source = 'unsplash';
  delete settings.unsplash_enabled;
  let subjects = old.subjects;
  if (!subjects) {
    subjects = EMPTY_SUBJECTS();
    if (old.place) {
      subjects.items.push({ id: 's1', kind: 'place', place: old.place });
      subjects.active = 's1';
    }
  }
  return {
    settings,
    subjects,
    sources: old.sources ?? {},
    history: old.history ?? [],
    nextId: old.nextId ?? 1,
    before_board: old.before_board,
  };
}

function load(): MockDb {
  try {
    const raw = typeof localStorage !== 'undefined' ? localStorage.getItem(KEY) : null;
    if (raw) return migrateDb(JSON.parse(raw) as Record<string, unknown>);
  } catch {
    /* fall through to defaults */
  }
  return fresh();
}

function fresh(): MockDb {
  return { settings: { ...DEFAULT_SETTINGS }, subjects: EMPTY_SUBJECTS(), sources: {}, history: [], nextId: 1 };
}

let db = load();

// Dev/screenshot helper: ?scene=s3&liked=1 starts with that wallpaper already shown (not persisted).
{
  const q = typeof location !== 'undefined' ? new URLSearchParams(location.search) : null;
  const scene = q?.get('scene');
  if (q && scene && SCENES.some((s) => s.id === scene)) {
    db = { ...fresh(), subjects: { items: [{ id: 's1', kind: 'place', place: { ...PLACES[1]! } }], active: 's1' }, nextId: 2 };
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

function activeSubject(): Subject | null {
  return db.subjects.items.find((x) => x.id === db.subjects.active) ?? null;
}

const subjectLabel = (x: Subject | null): string => (x ? subjectName(x) : 'Sample');

/** Same rule as core: boards need Pinterest, and Pinterest serves nothing but boards. */
const supports = (id: SourceId, kind: Subject['kind']): boolean => (kind === 'board') === (id === 'pinterest');

/** Pinterest board address → [user, board], or null. Mirrors parse_board_url in core. */
export function parseBoard(input: string): { user: string; board: string; url: string } | null {
  const m = /^(?:https?:\/\/)?(?:[a-z]{2}\.|www\.)?pinterest\.(?:[a-z]{2,3}|co\.uk|com\.au)\/([^/?#\s]+)\/([^/?#\s]+)/i.exec(input.trim());
  if (!m) return null;
  const user = m[1]!;
  const board = m[2]!.replace(/\.rss$/, '');
  if (['pin', 'search', 'ideas', 'today', 'explore', 'settings', 'business', 'login'].includes(user.toLowerCase())) return null;
  return { user, board, url: `https://www.pinterest.com/${user}/${board}.rss` };
}

/** Keep the source in line with the active subject (core: Engine::sync_source). */
function syncSource() {
  const a = activeSubject();
  if (a?.kind === 'board') {
    if (db.settings.source !== 'pinterest') {
      db.before_board = db.settings.source;
      db.settings.source = 'pinterest';
    }
  } else if (db.settings.source === 'pinterest') {
    db.settings.source = db.before_board && db.before_board !== 'pinterest' ? db.before_board : 'commons';
  }
}

function candidate(s: Scene, label: string, coords: Place['coords']): Candidate {
  return {
    source: db.settings.source,
    id: `mock:${s.id}`,
    title: s.title,
    description: null,
    page_url: 'https://commons.wikimedia.org/wiki/Special:Random',
    image_url: sceneImage(s),
    preview_url: sceneImage(s, 270, 600),
    width: 4000,
    height: 6000,
    author: db.settings.source === 'pinterest' ? 'Pinterest' : 'Sample Author',
    author_url: null,
    license: { short_name: 'CC BY-SA 4.0 (sample)', url: 'https://creativecommons.org/licenses/by-sa/4.0', attribution_required: true },
    coords,
    place_label: db.settings.source === 'pinterest' ? `Pinterest: ${label}` : label,
  };
}

const activePlace = (): Place | null => {
  const a = activeSubject();
  return a?.kind === 'place' ? a.place : null;
};

function toEntry(h: MockDb['history'][number]): HistoryEntry {
  const s = SCENES.find((x) => x.id === h.scene)!;
  return {
    id: h.id,
    shown_at: h.shown_at,
    candidate: candidate(s, subjectLabel(activeSubject()), activePlace()?.coords ?? null),
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

function sourceStatus(id: SourceId): SourceStatus {
  const m = db.sources[id];
  const needsKey = NEEDS_KEY.includes(id);
  const hasKey = !!m?.key;
  const subject = activeSubject();
  const unsupported = subject ? !supports(id, subject.kind) : false;
  const state: SourceState = unsupported
    ? 'unsupported'
    : needsKey && !hasKey
      ? 'needs_key'
      : m?.error
        ? 'unreachable'
        : 'available';
  return {
    source: id,
    state,
    needs_key: needsKey,
    has_key: hasKey,
    active: db.settings.source === id,
    error: m?.error ?? null,
    checked_at: m?.checked_at ?? null,
  };
}

const SOURCE_IDS: SourceId[] = ['commons', 'openverse', 'wallhaven', 'unsplash', 'pexels', 'pixabay', 'pinterest'];

function normalizeQuery(text: unknown): string {
  const q = String(text).replace(/\s+/g, ' ').trim();
  if ([...q].length < 2 || [...q].length > 80) fail('config', 'a search text needs 2–80 characters');
  return q;
}

export async function mockCall(cmd: string, a: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case 'reapply_wallpaper':
      return true;
    case 'get_settings':
      return db.settings;
    case 'save_settings': {
      const next = { ...DEFAULT_SETTINGS, ...(a.settings as Settings) };
      if (!SOURCE_IDS.includes(next.source)) next.source = 'commons';
      // A stale screen must not undo the source a board needs.
      const subject = activeSubject();
      if (subject && !supports(next.source, subject.kind)) next.source = db.settings.source;
      db.settings = next;
      save();
      return db.settings;
    }
    case 'selected_place':
      return activePlace();
    case 'subjects':
      return db.subjects;
    case 'add_query_subject': {
      const text = normalizeQuery(a.text);
      const same = db.subjects.items.find((x) => x.kind === 'query' && x.text.toLowerCase() === text.toLowerCase());
      if (same) db.subjects.active = same.id;
      else {
        const id = `s${Math.max(0, ...db.subjects.items.map((x) => Number(x.id.slice(1)) || 0)) + 1}`;
        db.subjects.items.push({ id, kind: 'query', text });
        db.subjects.active = id;
      }
      syncSource();
      save();
      return db.subjects;
    }
    case 'add_board_subject': {
      // The mock can't follow redirects: a short pin.it link stands for a sample board.
      const input = /^(?:https?:\/\/)?pin\.it\//i.test(String(a.url).trim()) ? 'pinterest.com/sample/misty-forests' : String(a.url);
      const b = parseBoard(input) ?? fail('config', 'not a Pinterest board address');
      await sleep(350);
      const same = db.subjects.items.find((x) => x.kind === 'board' && x.url === b.url);
      if (same) db.subjects.active = same.id;
      else {
        const id = `s${Math.max(0, ...db.subjects.items.map((x) => Number(x.id.slice(1)) || 0)) + 1}`;
        db.subjects.items.push({ id, kind: 'board', url: b.url, name: decodeURIComponent(b.board).replace(/[-_]+/g, ' ') });
        db.subjects.active = id;
      }
      syncSource();
      save();
      return db.subjects;
    }
    case 'update_query_subject': {
      const text = normalizeQuery(a.text);
      const x = db.subjects.items.find((i) => i.id === a.id) ?? fail('not_found', 'no saved subject');
      if (x.kind !== 'query') fail('config', 'only a search text can be edited');
      x.text = text;
      syncSource();
      save();
      return db.subjects;
    }
    case 'select_subject': {
      if (!db.subjects.items.some((i) => i.id === a.id)) fail('not_found', 'no saved subject');
      db.subjects.active = String(a.id);
      syncSource();
      save();
      return db.subjects;
    }
    case 'delete_subject': {
      if (!db.subjects.items.some((i) => i.id === a.id)) fail('not_found', 'no saved subject');
      db.subjects.items = db.subjects.items.filter((i) => i.id !== a.id);
      if (db.subjects.active === a.id) db.subjects.active = db.subjects.items[0]?.id ?? null;
      syncSource();
      save();
      return db.subjects;
    }
    case 'source_statuses':
      return SOURCE_IDS.map(sourceStatus);
    case 'check_source': {
      const id = a.source as SourceId;
      if (!SOURCE_IDS.includes(id)) fail('config', 'unknown source');
      const m = (db.sources[id] ??= { key: null, error: null, checked_at: null });
      if (NEEDS_KEY.includes(id) && !m.key) return sourceStatus(id);
      await sleep(400);
      // Demo: a key that starts with "bad" is rejected, so the "unreachable" state can be seen.
      m.error = m.key?.startsWith('bad') ? 'HTTP 401 from the service' : null;
      m.checked_at = Math.floor(Date.now() / 1000);
      save();
      return sourceStatus(id);
    }
    case 'set_source_key': {
      const id = a.source as SourceId;
      if (!NEEDS_KEY.includes(id)) fail('config', 'this source does not use a key');
      const m = (db.sources[id] ??= { key: null, error: null, checked_at: null });
      const key = a.key == null ? '' : String(a.key).trim();
      if (key && !/^[A-Za-z0-9_-]{16,128}$/.test(key)) fail('config', 'API key looks malformed');
      m.key = key || null;
      m.error = null;
      m.checked_at = null;
      save();
      return null;
    }
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
      const place = { ...p, label: a.lang === 'ru' ? (RU[p.label] ?? p.label) : p.label };
      const same = db.subjects.items.find((x) => x.kind === 'place' && x.place.qid === p.qid);
      if (same && same.kind === 'place') {
        same.place = place;
        db.subjects.active = same.id;
      } else {
        const id = `s${Math.max(0, ...db.subjects.items.map((x) => Number(x.id.slice(1)) || 0)) + 1}`;
        db.subjects.items.push({ id, kind: 'place', place });
        db.subjects.active = id;
      }
      syncSource();
      save();
      return place;
    }
    case 'launch_route':
      return null;
    case 'notifications_enabled':
    case 'request_notifications':
      return true;
    case 'schedule_status':
      return { last_at: 0, outcome: '', message: '', next_at: 0 };
    case 'open_battery_settings':
      return null;
    case 'current_wallpaper':
      return wallpaper(Boolean(a.dark));
    case 'change_now': {
      if (!activeSubject()) fail('config', 'nothing to search for is selected yet');
      // `source` is a one-time substitute chosen after a failure; it is never saved.
      const used = (a.source as SourceId | null) ?? db.settings.source;
      if (!SOURCE_IDS.includes(used)) fail('config', 'unknown source');
      const st = sourceStatus(used);
      if (st.state === 'needs_key') fail('config', 'this source needs an API key');
      if (st.state === 'unsupported') fail('config', 'this source cannot serve this subject');
      await sleep(900);
      // Demo: a source whose last check failed keeps failing until a check succeeds.
      if (!a.source && st.state === 'unreachable') fail('network', st.error ?? 'the source is not reachable');
      const tone = db.settings.follow_system_theme ? (a.systemDark ? 'dark' : 'light') : db.settings.tone;
      const shown = new Set(db.history.map((h) => h.scene));
      const next =
        SCENES.find((s) => !shown.has(s.id) && (tone === 'any' || (tone === 'dark') === s.dark)) ??
        fail('not_found', 'no suitable photo');
      db.history.push({ id: db.nextId++, shown_at: Math.floor(Date.now() / 1000), scene: next.id, liked: null });
      save();
      return wallpaper(Boolean(a.systemDark));
    }
    case 'apply_from_history': {
      const h =
        db.history.find((x) => x.id === Number(a.historyId)) ??
        fail('not_found', 'that photo is no longer in the history');
      await sleep(900);
      db.history.push({ id: db.nextId++, shown_at: Math.floor(Date.now() / 1000), scene: h.scene, liked: h.liked });
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
  db = fresh();
}

export const MOCK_SCENES = SCENES;
export { palette as scenePalette };
