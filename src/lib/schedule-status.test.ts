import { describe, expect, test } from 'bun:test';
import { describeSchedule, OVERDUE_MS } from './schedule-status';
import { missingKeys } from './dict';

const NOW = 1_800_000_000_000;

describe('describeSchedule', () => {
  test('nothing recorded yet', () => {
    expect(describeSchedule({ last_at: 0, outcome: '', message: '', next_at: 0 }, NOW)).toEqual({
      last: null,
      next: null,
      overdue: false,
    });
  });

  test('last run and a next time in the future', () => {
    const r = describeSchedule({ last_at: NOW - 1000, outcome: 'failed', message: 'no route', next_at: NOW + 5000 }, NOW);
    expect(r.last).toEqual({ at: NOW - 1000, outcome: 'failed', message: 'no route' });
    expect(r.next).toBe(NOW + 5000);
    expect(r.overdue).toBe(false);
  });

  test('a planned time long past means the alarm did not fire', () => {
    const s = { last_at: 0, outcome: '', message: '', next_at: NOW - OVERDUE_MS - 1 };
    expect(describeSchedule(s, NOW).overdue).toBe(true);
    expect(describeSchedule({ ...s, next_at: NOW - OVERDUE_MS + 1000 }, NOW).overdue).toBe(false);
  });

  test('an unknown outcome is not shown', () => {
    expect(describeSchedule({ last_at: NOW, outcome: 'weird', message: '', next_at: 0 }, NOW).last).toBeNull();
  });

  test('every outcome has text in both languages', () => {
    expect(missingKeys()).toEqual({ en: [], ru: [] });
  });
});
