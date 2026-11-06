// Static facts about the photo sources the UI needs (names are proper nouns, not translated).

import type { SourceId } from './types';

export interface SourceInfo {
  id: SourceId;
  name: string;
  /** Where the user gets a key; `null` for sources without one. Shown as plain text. */
  keyHost: string | null;
}

export const SOURCES: SourceInfo[] = [
  { id: 'commons', name: 'Wikimedia Commons', keyHost: null },
  { id: 'openverse', name: 'Openverse', keyHost: null },
  { id: 'wallhaven', name: 'Wallhaven', keyHost: null },
  { id: 'unsplash', name: 'Unsplash', keyHost: 'unsplash.com/developers' },
  { id: 'pexels', name: 'Pexels', keyHost: 'pexels.com/api' },
  { id: 'pixabay', name: 'Pixabay', keyHost: 'pixabay.com/api/docs' },
  { id: 'pinterest', name: 'Pinterest', keyHost: null },
];

export function sourceName(id: SourceId): string {
  return SOURCES.find((s) => s.id === id)?.name ?? id;
}
