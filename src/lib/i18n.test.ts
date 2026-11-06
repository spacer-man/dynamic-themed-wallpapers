import { expect, test } from 'bun:test';
import { detectLang, missingKeys, translate } from './dict';

test('both languages have exactly the same keys', () => {
  expect(missingKeys()).toEqual({ en: [], ru: [] });
});

test('interpolation and fallback', () => {
  expect(translate('en', 'today.first.text', { place: 'Kyoto' })).toBe('Get the first wallpaper: Kyoto.');
  expect(translate('en', 'no.such.key')).toBe('no.such.key');
  expect(translate('ru', 'today.first.text', { place: 'Киото' })).toBe('Подберём первые обои: Киото.');
  // Missing params stay visible instead of printing "undefined".
  expect(translate('ru', 'place.radius.auto')).toContain('{km}');
});

test('language detection', () => {
  expect(detectLang('ru-RU')).toBe('ru');
  expect(detectLang('en-US')).toBe('en');
  expect(detectLang('de')).toBe('en');
});
