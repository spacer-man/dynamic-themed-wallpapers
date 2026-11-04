// Mirrors `Place::natural_radius_km` / `default_scope` in crates/core/src/place.rs, so the UI can
// show what "automatic" means. If the Rust side changes, change this too (a test pins the numbers).

import type { Place, PlaceKind } from './types';

export const MIN_RADIUS_KM = 0.5;
export const MAX_RADIUS_KM = 40;

const DEFAULT_KM: Record<PlaceKind, number> = {
  country: 500,
  region: 80,
  city: 10,
  district: 2,
  landmark: 1,
  natural: 6,
  other: 5,
};

export function naturalRadiusKm(p: Pick<Place, 'kind' | 'area_km2'>): number {
  const fromArea = p.area_km2 && p.area_km2 > 0 ? Math.sqrt(p.area_km2 / Math.PI) : null;
  return Math.max(fromArea ?? DEFAULT_KM[p.kind], MIN_RADIUS_KM);
}

/** Large places are sampled from notable sub-places instead of one big circle. */
export function isWide(p: Pick<Place, 'kind' | 'area_km2' | 'coords'>): boolean {
  return p.kind === 'country' || p.coords === null || naturalRadiusKm(p) > MAX_RADIUS_KM;
}
