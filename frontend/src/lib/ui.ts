/**
 * Les classes des éléments qui reviennent d'une page à l'autre.
 *
 * Elles renvoient aux composants de la refonte (`styles/components.css` et
 * `styles/screens.css`) : `.btn`, `.opt`, `.seg`, `.tag`, `.card`… Tailwind ne
 * sert plus qu'à la mise en page autour.
 */

import type { Kind } from './api';

/** Assemble des classes, en laissant tomber celles qui ne s'appliquent pas. */
export const cx = (...parts: (string | false | null | undefined)[]): string =>
  parts.filter(Boolean).join(' ');

// --- Boutons -----------------------------------------------------------------

export const button = {
  plain: 'btn',
  primary: 'btn pri',
  discord: 'btn discord',
  /** Bouton carré à icône : navigation entre les semaines, petites actions. */
  icon: 'btn icon',
  /** Plus discret, dans une ligne de liste. */
  small: 'btn sm',
  smallIcon: 'btn icon sm',
};

// --- Choix -------------------------------------------------------------------

/** Puce de choix : année, groupe, sous-groupe. Sélection par `aria-pressed`. */
export const chip = 'opt';
export const chipSmall = 'opt sm';

/** Sélecteur à segments : les boutons enfants se marquent par `aria-pressed`. */
export const segment = 'seg';

/** Option en pleine largeur, avec son explication (`<b>` puis `<span>`). */
export const option = 'choice';

/** Champ de saisie, à l'allure du `.field` de la maquette. */
export const input = 'input';

// --- Sections de formulaire --------------------------------------------------

export const label = 'lbl';
export const fieldLabel = 'lbl';
export const fieldRow = 'row gap-2.5';

// --- Surfaces ----------------------------------------------------------------

export const card = 'card';

export const notice = (error = false): string => cx('notice', error && 'err');

export const note = 'text-sm text-muted';
export const link = 'lnk';

/** Titre de section d'une page : « Libres sur toute la plage ». */
export const sectionTitle = 'font-display text-[22px] font-semibold leading-tight';

// --- Natures de cours ----------------------------------------------------------

/** Suffixe des classes `k-*` (carte) et `t-*` (tag) de la maquette. */
export const KIND_KEY: Record<Kind, string> = {
  amphi: 'cm',
  td: 'td',
  tp: 'tp',
  sae: 'sae',
  controle: 'ctl',
};

/** Couleur de carte d'un cours : `k-cm`. */
export const kindCard = (kind: Kind): string => `k-${KIND_KEY[kind]}`;
/** Tag d'un cours : `tag t-cm`. */
export const kindTag = (kind: Kind): string => `tag t-${KIND_KEY[kind]}`;

/** Liseré d'un créneau, dans la couleur de sa nature (classe `kind-*`). */
export const kindEdge = 'border-s-3 border-s-(--kind-line)';
