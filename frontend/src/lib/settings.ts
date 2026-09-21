//! Réglages de l'utilisateur, mémorisés dans le navigateur.

import { createEffect, createSignal } from 'solid-js';

import { toHalf } from './groups';

export type ViewMode = 'grid' | 'list';
export type Theme = 'system' | 'light' | 'dark';
/** Couleur d'accent, libre, écrite en hexadécimal : `#e5303a`. */
export type Accent = string;

/** Le rouge d'origine, celui du département. */
export const DEFAULT_ACCENT = '#e5303a';

/** Ramène une saisie à un `#rrggbb` minuscule, ou `null` si ce n'en est pas une. */
export function toAccent(value: string): Accent | null {
  const hex = value.trim().replace(/^#?/, '#').toLowerCase();
  if (/^#[0-9a-f]{3}$/.test(hex)) {
    return `#${hex[1]}${hex[1]}${hex[2]}${hex[2]}${hex[3]}${hex[3]}`;
  }
  return /^#[0-9a-f]{6}$/.test(hex) ? hex : null;
}

/**
 * Noir ou blanc sur la couleur donnée, selon celle des deux qui s'y lit le
 * mieux : un jaune pâle réclame de l'encre sombre là où un bleu profond veut
 * du blanc. Luminance relative de sRGB, seuil à mi-chemin.
 */
function inkOn(hex: string): string {
  const channel = (from: number) => {
    const c = parseInt(hex.slice(from, from + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  return luminance > 0.45 ? '#14171c' : '#ffffff';
}

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
  /** Couleur d'accent, celle du bandeau en haut de l'écran. */
  accent: Accent;
  /**
   * Les deux classes mises côte à côte sur la page de comparaison. À gauche,
   * `null` veut dire « la mienne » : la comparaison suit alors le groupe suivi.
   */
  compare: { left: Side | null; right: Side | null };
  /** Enseignant suivi sur sa page, par son identifiant : `dubreuil-l`. */
  teacher: string | null;
}

const KEY = 'edt-unilim/settings';

const DEFAULTS: Settings = {
  year: null,
  group: null,
  view: 'list',
  theme: 'dark',
  accent: DEFAULT_ACCENT,
  compare: { left: null, right: null },
  teacher: null,
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
    const isDark = theme === 'dark' || (theme === 'system' && media.matches);
    const root = document.documentElement;
    root.classList.toggle('dark', isDark);
    // Posé en style direct sur <html>, l'accent choisi l'emporte sur les
    // jetons de la feuille de style, thème clair comme sombre.
    const color = toAccent(settings().accent) ?? DEFAULT_ACCENT;
    root.style.setProperty('--color-accent', color);
    root.style.setProperty('--color-on-accent', inkOn(color));
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute('content', isDark ? '#0f1117' : '#f6f7f9');
  };
  createEffect(apply);
  media.addEventListener('change', apply);
}

export function update(patch: Partial<Settings>): void {
  setSettings((current) => ({ ...current, ...patch }));
}

export { settings };
