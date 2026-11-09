// The Android back stack, kept free of Svelte so it can be unit-tested.
//
// Order of what "back" does: close the topmost open layer (sheet, edit mode) → leave a sub-screen
// for its parent → leave a non-home tab for the home tab → on home, ask for a second press
// within EXIT_WINDOW_MS before the app really closes. The error banner is not a layer.

import type { Route } from './types';

export const HOME: Route = 'today';
/** Second back press within this time on the home screen exits. */
export const EXIT_WINDOW_MS = 2000;

/** Sub-screens and the screen they return to (everything else returns to HOME). */
const PARENT: Partial<Record<Route, Route>> = { sources: 'place' };

export type BackResult = 'handled' | 'confirm' | 'exit';

export class BackStack {
  private layers: Array<{ close: () => void }> = [];
  private lastHomePress = Number.NEGATIVE_INFINITY;

  constructor(
    private nav: { route: () => Route; go: (r: Route) => void },
    private now: () => number = Date.now,
  ) {}

  /** Register an open layer; the returned function removes it (safe to call twice). */
  push(close: () => void): () => void {
    const layer = { close };
    this.layers.push(layer);
    return () => {
      this.layers = this.layers.filter((l) => l !== layer);
    };
  }

  get depth(): number {
    return this.layers.length;
  }

  /** Step back inside the app. Returns false when already at the home screen with nothing open. */
  back(): boolean {
    const top = this.layers.pop();
    if (top) {
      top.close();
      return true;
    }
    const route = this.nav.route();
    const parent = PARENT[route];
    if (parent) {
      this.nav.go(parent);
      return true;
    }
    if (route !== HOME) {
      this.nav.go(HOME);
      return true;
    }
    return false;
  }

  /** A back press from the system: step back, or 'confirm' (first press at home) / 'exit' (second). */
  press(): BackResult {
    if (this.back()) {
      this.lastHomePress = Number.NEGATIVE_INFINITY;
      return 'handled';
    }
    const t = this.now();
    if (t - this.lastHomePress <= EXIT_WINDOW_MS) {
      this.lastHomePress = Number.NEGATIVE_INFINITY;
      return 'exit';
    }
    this.lastHomePress = t;
    return 'confirm';
  }
}
