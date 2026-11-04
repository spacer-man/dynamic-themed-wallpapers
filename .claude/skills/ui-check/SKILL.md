---
name: ui-check
description: Verify the UI visually — start the dev server, screenshot every screen (light and dark, several wallpapers), and report deviations from docs/design.md. Use after any change in src/.
---

1. Static checks first (they are fast):
   ```sh
   ./node_modules/.bin/svelte-check --tsconfig ./tsconfig.json
   bun test
   ```
   (`bun run <script>` can fail in sandboxes that cannot read the parent directory — call the binaries directly.)

2. Start the dev server in the background: `./node_modules/.bin/vite --port 1420 --strictPort`.
   In a plain browser the app uses the in-memory mock (`src/lib/mock.ts`). URL helpers:
   `?scene=s1…s6` starts with that sample wallpaper shown (`s1–s3` dark, `s4–s6` light), `&liked=1`,
   `&route=today|place|look|schedule|history`.

3. Screenshot with system Chromium (no Playwright needed). The profile directory must be writable:
   ```sh
   chromium --headless=new --no-sandbox --disable-gpu --hide-scrollbars \
     --user-data-dir=<writable dir> --window-size=412,892 --virtual-time-budget=4000 \
     --screenshot=<out>.png "http://localhost:1420/?scene=s4&route=look"
   ```
   Dark system theme: add `--blink-settings=preferredColorScheme=1`. Look at the PNGs with the Read tool.

4. Check for: horizontal scroll, text clipped or overlapping, contrast problems (compare with the floors in
   `docs/design.md`), mismatched light/dark colours, empty states (`/` with no seed), long place names.
5. Report a list of deviations; fix them; repeat.
