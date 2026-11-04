import { expect, test } from 'bun:test';
import { isWide, naturalRadiusKm } from './place';

test('radius from area matches the Rust side (Kyoto ≈ 16.2 km)', () => {
  expect(naturalRadiusKm({ kind: 'city', area_km2: 827.83 })).toBeCloseTo(16.23, 1);
});

test('per-kind defaults when the area is unknown', () => {
  expect(naturalRadiusKm({ kind: 'landmark', area_km2: null })).toBe(1);
  expect(naturalRadiusKm({ kind: 'natural', area_km2: null })).toBe(6);
  expect(naturalRadiusKm({ kind: 'district', area_km2: 0 })).toBe(2);
});

test('wide places', () => {
  const here = { lat: 1, lon: 1 };
  expect(isWide({ kind: 'country', area_km2: 377975, coords: here })).toBe(true);
  expect(isWide({ kind: 'region', area_km2: 10_000, coords: here })).toBe(true); // 56 km
  expect(isWide({ kind: 'city', area_km2: 827, coords: here })).toBe(false);
  expect(isWide({ kind: 'city', area_km2: 827, coords: null })).toBe(true);
});
