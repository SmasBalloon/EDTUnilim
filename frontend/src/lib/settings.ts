//! Réglages de l'utilisateur, mémorisés dans le navigateur.

import { createEffect, createSignal } from 'solid-js';

import { toHalf } from './groups';

export type ViewMode = 'grid' | 'list';
export type Theme = 'system' | 'light' | 'dark';
/** Couleur d'accent, libre, écrite en hexadécimal : `#ff5252`. */
export type Accent = string;

/**
 * Le rouge de la refonte. Tant qu'il est retenu, ce sont les jetons de
 * `tokens.css` qui décident — `#FF5252` en sombre, `#D9303A` en clair.
 */
export const DEFAULT_ACCENT = '#ff5252';

/** Rouge par défaut des versions précédentes : il redevient celui du thème. */
const LEGACY_ACCENTS = ['#e5303a', '#ff4b4b'];

/** Ramène une saisie à un `#rrggbb` minuscule, ou `null` si ce n'en est pas une. */
export function toAccent(value: string): Accent | null {
  const hex = value.trim().replace(/^#?/, '#').toLowerCase();
  if (/^#[0-9a-f]{3}$/.test(hex)) {
    return `#${hex[1]}${hex[1]}${hex[2]}${hex[2]}${hex[3]}${hex[3]}`;
  }
  return /^#[0-9a-f]{6}$/.test(hex) ? hex : null;
}

/** Luminance relative d'un `#rrggbb` (WCAG). */
function luminance(hex: string): number {
  const channel = (from: number) => {
    const c = parseInt(hex.slice(from, from + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

const contrast = (a: number, b: number) => (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);

/** Encre sombre de la maquette (`--edt-on-accent`), ou blanc. */
const DARK_INK = '#1a0708';

/**
 * L'encre posée sur l'accent : celle des deux qui contraste le plus
 * (INTEGRATION.md § 6). Un rouge vif veut l'encre sombre, un bleu profond du
 * blanc.
 */
function inkOn(hex: string): string {
  const own = luminance(hex);
  return contrast(own, luminance(DARK_INK)) >= contrast(own, 1) ? DARK_INK : '#ffffff';
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
    if (LEGACY_ACCENTS.includes(saved.accent.toLowerCase())) saved.accent = DEFAULT_ACCENT;
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
  const media = window.matchMedia('(prefers-color-scheme: light)');
  const apply = () => {
    const theme = settings().theme;
    const isLight = theme === 'light' || (theme === 'system' && media.matches);
    const root = document.documentElement;
    root.dataset.theme = isLight ? 'light' : 'dark';

    // L'accent par défaut est celui des jetons, propre à chaque thème. Une
    // couleur choisie s'écrit sur <html> et l'emporte ; ses dérivés (texte
    // accent, « aujourd'hui », contour) la suivent, éclaircis en sombre et
    // assombris en clair pour rester lisibles sur le fond.
    const color = toAccent(settings().accent) ?? DEFAULT_ACCENT;
    const derived = ['--edt-accent', '--edt-on-accent', '--edt-accent-text', '--edt-accent-hot', '--edt-accent-ring'];
    if (color === DEFAULT_ACCENT) {
      for (const name of derived) root.style.removeProperty(name);
    } else {
      const toward = isLight ? 'black' : 'white';
      root.style.setProperty('--edt-accent', color);
      root.style.setProperty('--edt-on-accent', inkOn(color));
      root.style.setProperty('--edt-accent-text', `color-mix(in oklab, ${color} ${isLight ? 70 : 55}%, ${toward})`);
      root.style.setProperty('--edt-accent-hot', `color-mix(in oklab, ${color} ${isLight ? 82 : 70}%, ${toward})`);
      root.style.setProperty('--edt-accent-ring', `color-mix(in srgb, ${color} 75%, transparent)`);
    }

    // `theme-color` ne lit pas les variables CSS : il reprend l'accent, relu
    // sur les jetons appliqués.
    const accent = getComputedStyle(root).getPropertyValue('--edt-accent').trim();
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', accent || '#ff5252');
  };
  createEffect(apply);
  media.addEventListener('change', apply);
}

export function update(patch: Partial<Settings>): void {
  setSettings((current) => ({ ...current, ...patch }));
}

export { settings };
