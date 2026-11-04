// Colour maths for contrast checks. WCAG 2.x relative luminance and contrast ratio.

import type { Rgb } from './types';

export function hexToRgb(hex: string): Rgb {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) throw new Error(`bad colour: ${hex}`);
  const v = parseInt(m[1]!, 16);
  return [(v >> 16) & 255, (v >> 8) & 255, v & 255];
}

export function rgbToHex(c: Rgb): string {
  return '#' + c.map((x) => Math.round(Math.min(255, Math.max(0, x))).toString(16).padStart(2, '0')).join('');
}

function toLinear(c8: number): number {
  const c = c8 / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

export function luminance(c: Rgb): number {
  return 0.2126 * toLinear(c[0]) + 0.7152 * toLinear(c[1]) + 0.0722 * toLinear(c[2]);
}

export function contrast(a: Rgb, b: Rgb): number {
  const [la, lb] = [luminance(a), luminance(b)];
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** `top` over `bottom` with `alpha` opacity (sRGB space, like CSS does). */
export function blend(top: Rgb, bottom: Rgb, alpha: number): Rgb {
  return [0, 1, 2].map((i) => top[i]! * alpha + bottom[i]! * (1 - alpha)) as Rgb;
}

export const WHITE: Rgb = [255, 255, 255];
export const BLACK: Rgb = [0, 0, 0];
export const MIN_TEXT_CONTRAST = 4.5;

export type Ink = 'light' | 'dark';

/**
 * Weakest scrim opacity that keeps text of the given ink ≥ 4.5:1 on `bg`.
 * Light ink (white text) gets a black scrim; dark ink (black text) a white one.
 */
export function scrimAlphaFor(bg: Rgb, min = MIN_TEXT_CONTRAST, ink: Ink = 'light'): number {
  const [text, scrim] = ink === 'light' ? [WHITE, BLACK] : [BLACK, WHITE];
  for (let a = 0.2; a <= 0.9001; a += 0.02) {
    if (contrast(text, blend(scrim, bg, a)) >= min) return Math.round(a * 100) / 100;
  }
  return 0.9;
}

/**
 * How to draw the caption over a photo region: white text on a dark scrim, or black text on a
 * light scrim — whichever needs the weaker scrim (keeps the photo as clean as possible).
 * White text must survive the brightest 5% of the region, black text the darkest 5%.
 */
export function captionTreatment(pixels: Rgb[]): { ink: Ink; alpha: number } {
  const light = scrimAlphaFor(quantileColor(pixels, 0.95), MIN_TEXT_CONTRAST, 'light');
  const dark = scrimAlphaFor(quantileColor(pixels, 0.05), MIN_TEXT_CONTRAST, 'dark');
  return dark < light ? { ink: 'dark', alpha: dark } : { ink: 'light', alpha: light };
}

/**
 * Colour whose luminance is the `q`-quantile (0..1) of the given pixels — "how bright is the
 * brightest part under the text", robust against a single hot pixel.
 */
export function quantileColor(pixels: Rgb[], q = 0.95): Rgb {
  if (pixels.length === 0) return BLACK;
  const sorted = [...pixels].sort((a, b) => luminance(a) - luminance(b));
  return sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))]!;
}

/** Move `fg` towards white/black until it has `min` contrast with `bg` (used by the mock theme). */
export function ensureContrast(fg: Rgb, bg: Rgb, min: number): Rgb {
  if (contrast(fg, bg) >= min) return fg;
  const target = luminance(bg) < 0.4 ? WHITE : BLACK;
  for (let t = 0.05; t <= 1.0001; t += 0.05) {
    const c = blend(target, fg, t);
    if (contrast(c, bg) >= min) return c;
  }
  return target;
}

/** Is this palette, weighted by area, dark overall? (decides the UI variant when not following the system) */
export function paletteIsDark(p: { rgb: Rgb; weight: number }[]): boolean {
  const total = p.reduce((s, x) => s + x.weight, 0);
  if (total <= 0) return false;
  // Mean of perceptual lightness (luminance^(1/2.2)), so one bright sun does not outweigh a dark sky.
  const mean = p.reduce((s, x) => s + Math.pow(luminance(x.rgb), 1 / 2.2) * x.weight, 0) / total;
  return mean < 0.5;
}
