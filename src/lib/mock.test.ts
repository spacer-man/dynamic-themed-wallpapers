import { describe, expect, test } from 'bun:test';
import { contrast, hexToRgb } from './color';
import { MOCK_SCENES, mockCall, mockTheme, resetMock, scenePalette } from './mock';
import type { Wallpaper } from './types';

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
