//! Réglages de l'utilisateur, mémorisés dans le navigateur.

import { createEffect, createSignal } from 'solid-js';

import { toHalf } from './groups';

export type ViewMode = 'grid' | 'list';
export type Theme = 'system' | 'light' | 'dark';
export type Accent = (typeof ACCENTS)[number]['value'];

/**
 * Couleurs proposées pour le bandeau et tout ce qui porte l'accent. Chaque
 * teinte est donnée deux fois : posée sur un fond clair elle doit être assez
 * sombre pour porter du blanc, sur un fond sombre assez vive pour se détacher.
 */
export const ACCENTS = [
  { value: 'rouge', label: 'Rouge', light: '#e5303a', dark: '#ff4b4b' },
  { value: 'orange', label: 'Orange', light: '#d1670f', dark: '#ff9f52' },
  { value: 'ambre', label: 'Ambre', light: '#a8730a', dark: '#ffd75e' },
  { value: 'vert', label: 'Vert', light: '#14804a', dark: '#3fce87' },
  { value: 'turquoise', label: 'Turquoise', light: '#0d7d84', dark: '#3ed2d2' },
  { value: 'bleu', label: 'Bleu', light: '#2563c9', dark: '#6fb0ff' },
  { value: 'violet', label: 'Violet', light: '#6d3fd4', dark: '#b39bff' },
  { value: 'rose', label: 'Rose', light: '#c81e78', dark: '#ff7ab8' },
] as const;

const DEFAULT_ACCENT: Accent = 'rouge';

function accentOf(value: Accent): (typeof ACCENTS)[number] {
  return ACCENTS.find((accent) => accent.value === value) ?? ACCENTS[0];
}

/**
 * Noir ou blanc sur la couleur donnée, selon celle des deux qui s'y lit le
 * mieux : une ambre claire réclame de l'encre sombre là où un bleu profond
 * veut du blanc. Luminance relative de sRGB, seuil à mi-chemin.
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

/** Vrai quand l'application est en sombre, thème système compris. */
const [dark, setDark] = createSignal(false);

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
    setDark(isDark);
    const root = document.documentElement;
    root.classList.toggle('dark', isDark);
    // Posé en style direct sur <html>, l'accent choisi l'emporte sur les
    // jetons de la feuille de style, thème clair comme sombre.
    const accent = accentOf(settings().accent);
    const color = isDark ? accent.dark : accent.light;
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

export { dark, settings };
