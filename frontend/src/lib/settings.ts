//! Réglages de l'utilisateur, mémorisés dans le navigateur.

import { createEffect, createSignal } from 'solid-js';

import { toHalf } from './groups';

export type ViewMode = 'grid' | 'list';
export type Theme = 'system' | 'light' | 'dark';

/** Une classe désignée : son année et son demi-groupe. */
export interface Side {
  year: number;
  group: string;
}

export interface Settings {
  /** Année suivie (1, 2 ou 3), `null` tant que le choix n'a pas été fait. */
  year: number | null;
  /** Demi-groupe suivi : `G1a`. */
  group: string | null;
  /** Grille horaire ou liste des cours par jour. */
  view: ViewMode;
  theme: Theme;
  /**
   * Les deux classes mises côte à côte sur la page de comparaison. À gauche,
   * `null` veut dire « la mienne » : la comparaison suit alors le groupe suivi.
   */
  compare: { left: Side | null; right: Side | null };
}

const KEY = 'edt-unilim/settings';

const DEFAULTS: Settings = {
  year: null,
  group: null,
  view: 'list',
  theme: 'dark',
  compare: { left: null, right: null },
};

function load(): Settings {
  try {
    const stored = localStorage.getItem(KEY);
    if (!stored) return DEFAULTS;
    const saved = { ...DEFAULTS, ...(JSON.parse(stored) as Partial<Settings>) };
    if (typeof saved.compare !== 'object' || saved.compare === null) saved.compare = DEFAULTS.compare;
    // Un groupe entier enregistré par une version précédente devient sa
    // première moitié : l'application ne raisonne plus qu'en demi-groupes.
    return { ...saved, group: saved.group === null ? null : toHalf(saved.group) };
  } catch {
    return DEFAULTS;
  }
}

const [settings, setSettings] = createSignal<Settings>(load());

createEffect(() => {
  try {
    localStorage.setItem(KEY, JSON.stringify(settings()));
  } catch {
    // Navigation privée ou stockage refusé : les réglages ne survivront pas
    // à la session, ce qui reste préférable à un plantage.
  }
});

/** Applique le thème choisi, en suivant le système quand c'est demandé. */
export function watchTheme(): void {
  const media = window.matchMedia('(prefers-color-scheme: dark)');
  const apply = () => {
    const theme = settings().theme;
    const dark = theme === 'dark' || (theme === 'system' && media.matches);
    document.documentElement.classList.toggle('dark', dark);
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute('content', dark ? '#0f1117' : '#f6f7f9');
  };
  createEffect(apply);
  media.addEventListener('change', apply);
}

export function update(patch: Partial<Settings>): void {
  setSettings((current) => ({ ...current, ...patch }));
}

export { settings };
