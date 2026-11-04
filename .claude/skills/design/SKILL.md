---
name: design
description: Design rules for this project's UI (photo-forward, palette-driven theme, spacing/type scales, contrast floors). Use before creating or restyling any Svelte component or CSS in src/.
---

Follow `docs/design.md` — it is the source of truth. Short version:

- Photo-forward: the wallpaper fills the Today screen; the rest of the UI is quiet and takes its colours from the
  photo's palette through the CSS variables `--bg --surface --text --muted --accent --on-accent`.
- Use only the tokens from `src/app.css` (spacing 4/8/12/16/24/32, type 12/14/16/20/28, radii 20/12/pill). No new
  magic numbers, no hard-coded colours (except white/black ink over photos).
- Contrast floors: text ≥ 7:1, muted text and accent ≥ 4.5:1, on-accent ≥ 4.5:1; caption over a photo ≥ 4.5:1 via
  `captionTreatment`. Never "fix" contrast with text shadows or outlines.
- Touch targets ≥ 44 px; every icon-only button needs an `aria-label`; respect `prefers-reduced-motion`.
- All user-visible text goes through `t('key')`; add the key to BOTH dictionaries in `src/lib/dict.ts`
  (a unit test fails otherwise). Photo metadata (titles, authors) is untrusted: render as text, never `{@html}`.
- After UI changes run the `/ui-check` skill.
