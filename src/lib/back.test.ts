import { describe, expect, test } from 'bun:test';
import { BackStack, EXIT_WINDOW_MS } from './back';
import type { Route } from './types';

function setup(start: Route = 'today') {
  const s = { route: start, clock: 1_000_000 };
  const stack = new BackStack(
    { route: () => s.route, go: (r) => (s.route = r) },
    () => s.clock,
  );
  return { s, stack };
}

describe('back stack', () => {
  test('closes the topmost layer first, then the sub-screen, the tab, then asks to exit', () => {
    const { s, stack } = setup('sources');
    const closed: string[] = [];
    stack.push(() => closed.push('sheet'));
    stack.push(() => closed.push('edit'));
    expect(stack.press()).toBe('handled');
    expect(closed).toEqual(['edit']);
    expect(stack.press()).toBe('handled');
    expect(closed).toEqual(['edit', 'sheet']);
    expect(s.route).toBe('sources');
    expect(stack.press()).toBe('handled');
    expect(s.route).toBe('place');
    expect(stack.press()).toBe('handled');
    expect(s.route).toBe('today');
    expect(stack.press()).toBe('confirm');
    s.clock += EXIT_WINDOW_MS - 1;
    expect(stack.press()).toBe('exit');
  });

  test('the first press at home asks again after the window has passed', () => {
    const { s, stack } = setup();
    expect(stack.press()).toBe('confirm');
    s.clock += EXIT_WINDOW_MS + 1;
    expect(stack.press()).toBe('confirm');
    s.clock += 100;
    expect(stack.press()).toBe('exit');
  });

  test('any handled step resets the exit confirmation', () => {
    const { s, stack } = setup();
    expect(stack.press()).toBe('confirm');
    s.route = 'look';
    expect(stack.press()).toBe('handled');
    expect(stack.press()).toBe('confirm');
  });

  test('an unregistered layer is not closed, and unregistering twice is harmless', () => {
    const { stack } = setup();
    let n = 0;
    const off = stack.push(() => n++);
    off();
    off();
    expect(stack.depth).toBe(0);
    expect(stack.back()).toBe(false);
    expect(n).toBe(0);
  });

  test('back() alone never exits', () => {
    const { stack } = setup();
    expect(stack.back()).toBe(false);
    expect(stack.back()).toBe(false);
  });
});
