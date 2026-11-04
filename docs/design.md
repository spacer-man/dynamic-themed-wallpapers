# Design system

Direction: **photo-forward**. The wallpaper is the hero; the UI is quiet and takes its colours from the photo.

## Principles

1. The photo of the day fills the Today screen edge to edge. No frames, no cards on top of it.
2. Everything else is derived from the photo's palette: background, surface, text, accent. Nothing is hard-coded
   except the neutral defaults in `src/app.css`.
3. Readability beats style. Text always meets WCAG: body text ≥ 7:1, muted text and accent ≥ 4.5:1, text on the
   accent colour ≥ 4.5:1 (enforced in `crates/core/src/theme.rs`, tested).
4. Calm: one accent colour, no gradients except the caption scrim, motion only to explain (reveal, sheet rise).

## Tokens (`src/app.css`)

| Token | Values |
|---|---|
| Spacing | 4 · 8 · 12 · 16 · 24 · 32 px (`--space-1…6`) |
| Type | 12 · 14 · 16 · 20 · 28 px (`--fs-xs…xl`), system UI font with Noto CJK fallback |
| Radius | 20 px cards (`--radius`), 12 px controls (`--radius-sm`), pills 999 px |
| Colour | `--bg --surface --text --muted --accent --on-accent`, set from the palette; `--hairline`, `--glass`, `--accent-soft` are mixed from them |
| Touch target | ≥ 44 px high |

## Palette → theme

The wallpaper's k-means palette (5 swatches, OKLab) feeds `theme::from_palette(palette, dark)`:

- background / surface: the dominant hue at fixed lightness (dark 0.18 / 0.24, light 0.97 / 0.93), chroma reduced;
- text: dominant hue, near-white or near-black, pushed until contrast ≥ 7:1;
- accent: the swatch with the most chroma × √area, pushed until contrast ≥ 4.5:1 with the background;
- the dark/light variant follows the system when "follow system theme" is on, otherwise the photo's own tone.

## Caption on the photo

The text over the photo is checked per photo (`captionTreatment` in `src/lib/color.ts`): the bottom ~38% of the
image is sampled; white text must survive the brightest 5% of that region, black text the darkest 5%. Whichever
needs the weaker scrim (black for white text, white for black text) wins, at the lowest opacity that reaches 4.5:1.

## Glass

Used only over the photo (action buttons) and for the tab bar: translucent fill + 14–18 px blur + 1 px hairline.
Not on content screens.

## Don'ts

- No drop shadows on text, no outlines around letters — fix contrast with the scrim instead.
- No more than one accent colour on a screen.
- No hard-coded colours in components (exceptions: white/black ink over photos, the error banner).
- No icon-only controls without an `aria-label`.
