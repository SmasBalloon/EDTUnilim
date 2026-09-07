//! Points de rupture observés depuis le code.

import { createSignal, onCleanup } from 'solid-js';

/** Vrai sur les écrans étroits, où la semaine se lit un jour à la fois. */
export function useCompact(): () => boolean {
  const media = window.matchMedia('(max-width: 47.99rem)');
  const [compact, setCompact] = createSignal(media.matches);

  const update = (event: MediaQueryListEvent) => setCompact(event.matches);
  media.addEventListener('change', update);
  onCleanup(() => media.removeEventListener('change', update));

  return compact;
}
