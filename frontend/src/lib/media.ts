//! Points de rupture observés depuis le code.

import { createSignal, onCleanup } from 'solid-js';

/** Une requête média lue comme un signal. */
export function useMedia(query: string): () => boolean {
  const media = window.matchMedia(query);
  const [matches, setMatches] = createSignal(media.matches);

  const update = (event: MediaQueryListEvent) => setMatches(event.matches);
  media.addEventListener('change', update);
  onCleanup(() => media.removeEventListener('change', update));

  return matches;
}

/** Vrai sur les écrans étroits, où la semaine se lit un jour à la fois. */
export function useCompact(): () => boolean {
  return useMedia('(max-width: 47.99rem)');
}

/** Vrai quand la feuille monte du bas plutôt que de se poser au centre. */
export function useBottomSheet(): () => boolean {
  return useMedia('(max-width: 39.99rem)');
}
