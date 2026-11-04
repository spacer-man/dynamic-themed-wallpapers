// Reactive wrapper around the pure functions in dict.ts.

import { detectLang, translate, type Language } from './dict';
import type { Lang } from './types';

class I18n {
  /** User choice; 'auto' follows the device language. */
  choice = $state<Lang>('auto');

  get lang(): Language {
    return this.choice === 'auto'
      ? detectLang(typeof navigator !== 'undefined' ? navigator.language : 'en')
      : this.choice;
  }
}

export const i18n = new I18n();

/** Reads `i18n.choice`, so anything calling it in a template re-renders on language change. */
export function t(key: string, params: Record<string, string | number> = {}): string {
  return translate(i18n.lang, key, params);
}
