import { describe, expect, test } from 'bun:test';
import {
  BLACK,
  WHITE,
  blend,
  contrast,
  ensureContrast,
  hexToRgb,
  quantileColor,
  rgbToHex,
  scrimAlphaFor,
  paletteIsDark,
  captionTreatment,
  MIN_TEXT_CONTRAST,
} from './color';

describe('contrast', () => {
  test('WCAG reference values', () => {
    expect(contrast(BLACK, WHITE)).toBeCloseTo(21, 1);
    expect(contrast([119, 119, 119], WHITE)).toBeCloseTo(4.48, 1);
    expect(contrast(WHITE, WHITE)).toBe(1);
  });

  test('hex round trip', () => {
    expect(rgbToHex(hexToRgb('#1a2b3c'))).toBe('#1a2b3c');
    expect(() => hexToRgb('nope')).toThrow();
  });
});

describe('caption scrim', () => {
  test('white text stays readable over any background once the scrim is applied', () => {
    // Sweep the whole grey ramp plus saturated colours, including the worst case: pure white.
    const backgrounds: [number, number, number][] = [
      WHITE,
      [255, 255, 0],
      [240, 240, 250],
      [200, 120, 40],
      [128, 128, 128],
      [10, 10, 10],
      ...Array.from({ length: 16 }, (_, i): [number, number, number] => [i * 17, i * 17, i * 17]),
    ];
    for (const bg of backgrounds) {
      const a = scrimAlphaFor(bg);
      expect(a).toBeGreaterThanOrEqual(0.2);
      expect(a).toBeLessThanOrEqual(0.9);
      expect(contrast(WHITE, blend(BLACK, bg, a))).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
    }
  });

  test('darker photos need a lighter scrim', () => {
    expect(scrimAlphaFor([20, 20, 20])).toBeLessThan(scrimAlphaFor([220, 220, 220]));
  });

  test('quantile ignores a single hot pixel', () => {
    const px = Array.from({ length: 100 }, () => [40, 40, 40] as [number, number, number]);
    px[3] = [255, 255, 255];
    expect(quantileColor(px, 0.95)).toEqual([40, 40, 40]);
    expect(quantileColor(px, 1)).toEqual(WHITE);
    expect(quantileColor([])).toEqual(BLACK);
  });
});

describe('ensureContrast', () => {
  test('fixes weak pairs and leaves good ones alone', () => {
    const bg: [number, number, number] = [30, 30, 40];
    expect(ensureContrast([220, 220, 230], bg, 7)).toEqual([220, 220, 230]);
    const fixed = ensureContrast([60, 60, 80], bg, 4.5);
    expect(contrast(fixed, bg)).toBeGreaterThanOrEqual(4.5);
  });
});

describe('paletteIsDark', () => {
  test("weights by area", () => {
    expect(paletteIsDark([{ rgb: [20, 20, 40], weight: 0.9 }, { rgb: [255, 240, 200], weight: 0.1 }])).toBe(true);
    expect(paletteIsDark([{ rgb: [240, 240, 235], weight: 0.8 }, { rgb: [30, 30, 30], weight: 0.2 }])).toBe(false);
    expect(paletteIsDark([])).toBe(false);
  });
});

describe('captionTreatment', () => {
  const flat = (c: [number, number, number], n = 100) => Array.from({ length: n }, () => c);
  test("dark photo → white text, light photo → dark text", () => {
    expect(captionTreatment(flat([25, 25, 35])).ink).toBe('light');
    expect(captionTreatment(flat([235, 238, 240])).ink).toBe('dark');
  });
  test("the chosen combination always meets the contrast floor", () => {
    for (let v = 0; v <= 255; v += 15) {
      for (const tint of [0, 60, -60]) {
        const c: [number, number, number] = [v, Math.max(0, Math.min(255, v + tint)), v];
        const { ink, alpha } = captionTreatment([...flat(c, 90), ...flat([0, 0, 0], 5), ...flat([255, 255, 255], 5)]);
        const text = ink === "light" ? WHITE : BLACK;
        const scrim = ink === "light" ? BLACK : WHITE;
        // Check against the worst pixel the treatment promises to cover (p95 for light ink, p05 for dark).
        const worst = ink === "light" ? quantileColor([...flat(c, 90), ...flat([0, 0, 0], 5), ...flat([255, 255, 255], 5)], 0.95) : quantileColor([...flat(c, 90), ...flat([0, 0, 0], 5), ...flat([255, 255, 255], 5)], 0.05);
        expect(contrast(text, blend(scrim, worst, alpha))).toBeGreaterThanOrEqual(MIN_TEXT_CONTRAST);
      }
    }
  });
  test("mid-grey picks the weaker scrim", () => {
    const { ink, alpha } = captionTreatment(flat([128, 128, 128]));
    const other = scrimAlphaFor([128, 128, 128], 4.5, ink === "light" ? "dark" : "light");
    expect(alpha).toBeLessThanOrEqual(other);
  });
});
