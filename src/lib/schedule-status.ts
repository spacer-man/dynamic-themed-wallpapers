// Pure helpers for the "last change / next change" lines of the Schedule screen (unit-testable under bun).

import type { ScheduleStatus } from './types';

export const OUTCOMES = ['queued', 'changed', 'retry', 'failed'] as const;
export type Outcome = (typeof OUTCOMES)[number];

/** A planned time this far in the past means the phone never started the alarm. */
export const OVERDUE_MS = 10 * 60 * 1000;

export interface ScheduleInfo {
  last: { at: number; outcome: Outcome; message: string } | null;
  next: number | null;
  overdue: boolean;
}

export function describeSchedule(s: ScheduleStatus, now: number): ScheduleInfo {
  const known = (OUTCOMES as readonly string[]).includes(s.outcome);
  const last = s.last_at > 0 && known ? { at: s.last_at, outcome: s.outcome as Outcome, message: s.message } : null;
  const next = s.next_at > 0 ? s.next_at : null;
  return { last, next, overdue: next !== null && now - next > OVERDUE_MS };
}
