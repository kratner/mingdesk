// Which List groups are folded (a display, a desktop, Minimized, an app), kept per viewer. A convenience only:
// without storage everything starts open.

import { useCallback, useState } from 'react';

const KEY = 'mingdesk.folded';

function load(): Set<string> {
  try {
    return new Set(JSON.parse(localStorage.getItem(KEY) ?? '[]') as string[]);
  } catch {
    return new Set();
  }
}

export interface Folds {
  isFolded: (id: string) => boolean;
  toggle: (id: string) => void;
}

/** While a filter is typed, nothing is folded, so every match shows. */
export function useFolds(filtering: boolean): Folds {
  const [folded, setFolded] = useState(load);
  const toggle = useCallback((id: string) => {
    setFolded((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      try {
        localStorage.setItem(KEY, JSON.stringify([...next]));
      } catch {
        /* not kept; fine */
      }
      return next;
    });
  }, []);
  return { isFolded: (id) => !filtering && folded.has(id), toggle };
}
