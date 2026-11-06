import { describe, expect, test } from 'bun:test';
import { contrast, hexToRgb } from './color';
import { MOCK_SCENES, migrateDb, mockCall, mockTheme, resetMock, scenePalette } from './mock';
import type { SourceStatus, Subjects, Wallpaper } from './types';

describe('mock theme keeps the contrast promises of core/theme.rs', () => {
  for (const scene of MOCK_SCENES) {
    for (const dark of [true, false]) {
      test(`${scene.id} ${dark ? 'dark' : 'light'}`, () => {
        const t = mockTheme(scenePalette(scene), dark);
        const bg = hexToRgb(t.background);
        expect(contrast(hexToRgb(t.text), bg)).toBeGreaterThanOrEqual(7);
        expect(contrast(hexToRgb(t.text_muted), bg)).toBeGreaterThanOrEqual(4.5);
        expect(contrast(hexToRgb(t.accent), bg)).toBeGreaterThanOrEqual(4.5);
        expect(contrast(hexToRgb(t.on_accent), hexToRgb(t.accent))).toBeGreaterThanOrEqual(4.5);
      });
    }
  }
});

describe('mock backend behaves like the real one', () => {
  test('needs a place, honours tone, never repeats, then runs out', async () => {
    resetMock();
    await expect(mockCall('change_now', { systemDark: false })).rejects.toMatchObject({ kind: 'config' });
    await mockCall('select_place', { qid: 'Q34600', lang: 'en' });

    const s = (await mockCall('get_settings', {})) as Record<string, unknown>;
    await mockCall('save_settings', { settings: { ...s, tone: 'dark' } });
    const seen = new Set<string>();
    for (let i = 0; i < 3; i++) {
      const w = (await mockCall('change_now', { systemDark: false })) as Wallpaper;
      expect(w.theme).toBeDefined();
      expect(seen.has(w.entry.candidate.id)).toBe(false);
      seen.add(w.entry.candidate.id);
      // dark scenes only
      expect(w.entry.palette.length).toBeGreaterThan(0);
    }
    await expect(mockCall('change_now', { systemDark: false })).rejects.toMatchObject({ kind: 'not_found' });
  }, 15000);
});

describe('mock apply_from_history', () => {
  test('sets an older photo as current, as a new history row; unknown id is not_found', async () => {
    resetMock();
    await mockCall('select_place', { qid: 'Q34600', lang: 'en' });
    const first = (await mockCall('change_now', { systemDark: false })) as Wallpaper;
    await mockCall('change_now', { systemDark: false });
    const hist = (await mockCall('history', { limit: 10, offset: 0 })) as { id: number }[];
    const oldest = hist[hist.length - 1]!;
    const w = (await mockCall('apply_from_history', { historyId: oldest.id, systemDark: false })) as Wallpaper;
    expect(w.entry.candidate.id).toBe(first.entry.candidate.id);
    expect(((await mockCall('history', { limit: 10, offset: 0 })) as unknown[]).length).toBe(3);
    await expect(mockCall('apply_from_history', { historyId: 999, systemDark: false })).rejects.toMatchObject({
      kind: 'not_found',
    });
  }, 15000);
});

describe('mock subjects and sources', () => {
  test('several subjects, one active; queries can be edited and deleted', async () => {
    resetMock();
    await mockCall('select_place', { qid: 'Q34600', lang: 'en' });
    let subs = (await mockCall('add_query_subject', { text: '  foggy   forest ' })) as Subjects;
    expect(subs.items.map((i) => i.kind)).toEqual(['place', 'query']);
    const q = subs.items[1]!;
    expect(subs.active).toBe(q.id);
    await expect(mockCall('add_query_subject', { text: 'x' })).rejects.toMatchObject({ kind: 'config' });
    // The same text only selects it.
    subs = (await mockCall('add_query_subject', { text: 'Foggy forest' })) as Subjects;
    expect(subs.items.length).toBe(2);
    subs = (await mockCall('update_query_subject', { id: q.id, text: 'misty lake' })) as Subjects;
    expect(subs.items[1]).toMatchObject({ kind: 'query', text: 'misty lake' });
    await expect(mockCall('update_query_subject', { id: subs.items[0]!.id, text: 'no' })).rejects.toMatchObject({
      kind: 'config',
    });
    subs = (await mockCall('select_subject', { id: subs.items[0]!.id })) as Subjects;
    expect(subs.active).toBe(subs.items[0]!.id);
    subs = (await mockCall('delete_subject', { id: subs.items[0]!.id })) as Subjects;
    expect(subs.active).toBe(subs.items[0]!.id);
    expect(subs.items.length).toBe(1);
  });

  test('key sources need a key; statuses and checks', async () => {
    resetMock();
    await mockCall('add_query_subject', { text: 'fog' });
    const statuses = (await mockCall('source_statuses', {})) as SourceStatus[];
    expect(statuses.map((s) => s.source)).toEqual(['commons', 'openverse', 'wallhaven', 'unsplash', 'pexels', 'pixabay']);
    expect(statuses.find((s) => s.source === 'pexels')!.state).toBe('needs_key');
    expect(statuses.find((s) => s.source === 'commons')!.active).toBe(true);

    const s = (await mockCall('get_settings', {})) as Record<string, unknown>;
    await mockCall('save_settings', { settings: { ...s, source: 'pexels' } });
    await expect(mockCall('change_now', { systemDark: false })).rejects.toMatchObject({ kind: 'config' });

    await expect(mockCall('set_source_key', { source: 'pexels', key: 'short' })).rejects.toMatchObject({ kind: 'config' });
    await expect(mockCall('set_source_key', { source: 'commons', key: 'x'.repeat(30) })).rejects.toMatchObject({
      kind: 'config',
    });
    await mockCall('set_source_key', { source: 'pexels', key: 'bad_key_that_is_long_enough' });
    const bad = (await mockCall('check_source', { source: 'pexels' })) as SourceStatus;
    expect(bad).toMatchObject({ state: 'unreachable', has_key: true, active: true });
    expect(bad.error).toContain('401');
    expect(bad.checked_at).not.toBeNull();
    await mockCall('set_source_key', { source: 'pexels', key: 'good_key_that_is_long_enough' });
    const ok = (await mockCall('check_source', { source: 'pexels' })) as SourceStatus;
    expect(ok.state).toBe('available');
    // Removing the key brings back "needs key".
    await mockCall('set_source_key', { source: 'pexels', key: null });
    const gone = (await mockCall('check_source', { source: 'pexels' })) as SourceStatus;
    expect(gone.state).toBe('needs_key');
  });

  test('a database saved by an older version migrates', () => {
    const db = migrateDb({
      settings: { tone: 'dark', unsplash_enabled: true },
      place: { qid: 'Q1', label: 'Kyoto' },
      history: [],
      nextId: 3,
    });
    expect(db.settings.source).toBe('unsplash');
    expect('unsplash_enabled' in db.settings).toBe(false);
    expect(db.subjects.active).toBe('s1');
    expect(db.subjects.items[0]).toMatchObject({ kind: 'place' });
    expect(migrateDb({ settings: { unsplash_enabled: false } }).settings.source).toBe('commons');
  });
});
