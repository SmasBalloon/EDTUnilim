/**
 * Les classes Tailwind des éléments qui reviennent d'une page à l'autre.
 *
 * Une variante ne redéclare jamais une propriété de sa base : deux utilitaires
 * qui se disputent la même propriété l'emportent dans l'ordre de la feuille
 * générée, pas dans celui de l'attribut `class`.
 */

/** Assemble des classes, en laissant tomber celles qui ne s'appliquent pas. */
export const cx = (...parts: (string | false | null | undefined)[]): string =>
  parts.filter(Boolean).join(' ');

/*
 * Le retour se donne à l'appui, pas au relâchement : une commande qui attend
 * le clic pour réagir passe pour morte. L'enfoncement est instantané, le
 * retour à la taille normale prend le temps d'être vu.
 */
const transition =
  'transition-[scale,background-color,border-color,color,box-shadow] duration-(--settle) ease-out active:duration-(--press)';
export const press = `${transition} active:not-disabled:scale-97`;
/** Les grandes surfaces s'enfoncent moins : le même facteur y paraîtrait mou. */
export const pressLarge = `${transition} active:scale-[0.985]`;

// --- Boutons -----------------------------------------------------------------

const control = `${press} inline-flex items-center justify-center gap-1.5 border cursor-pointer disabled:cursor-default disabled:opacity-40 [&_svg]:flex-none`;
/* Au doigt, les commandes gagnent la hauteur qu'il faut pour être visées. */
const sized = 'min-h-9 min-w-9 px-2.5 rounded-control text-[0.8125rem] pointer-coarse:min-h-10';

export const button = {
  plain: `${control} ${sized} border-line bg-surface text-text hover:not-disabled:bg-elevated`,
  primary: `${control} ${sized} border-transparent bg-accent font-semibold text-on-accent hover:not-disabled:bg-accent-deep`,
  /* Couleur de la marque Discord, reconnaissable d'un coup d'œil. */
  discord: `${control} ${sized} border-transparent bg-[#5865f2] font-semibold text-white hover:not-disabled:bg-[#4752c4]`,
  /** Bouton rond : navigation entre les semaines, petites actions. */
  round: `${control} size-9 rounded-full border-accent/65 bg-accent/18 text-accent-ink hover:not-disabled:bg-accent hover:not-disabled:text-on-accent pointer-coarse:size-10`,
};

// --- Choix -------------------------------------------------------------------

export const chip = cx(
  press,
  'rounded-full border px-3 py-[0.3rem] text-[0.8125rem] cursor-pointer pointer-coarse:min-h-10',
  'border-line bg-surface text-text hover:not-disabled:border-accent-line',
  'disabled:cursor-default disabled:opacity-40',
  'aria-pressed:border-accent/60 aria-pressed:bg-accent-soft aria-pressed:font-semibold aria-pressed:text-accent-ink',
);

export const segment = 'inline-flex gap-0.5 rounded-control border border-line bg-elevated p-0.5';

export const segmentOption = cx(
  press,
  'rounded-lg px-[0.7rem] py-[0.3rem] text-[0.8125rem] text-muted cursor-pointer pointer-coarse:min-h-10',
  'aria-pressed:bg-surface aria-pressed:font-semibold aria-pressed:text-text',
);

/** Option en pleine largeur, avec son explication. */
export const option = cx(
  pressLarge,
  'w-full rounded-control border border-line bg-surface p-3 text-start text-text cursor-pointer',
  'aria-pressed:border-accent aria-pressed:bg-elevated',
);
export const optionTitle = 'block font-medium';
export const optionHint = 'mt-0.5 block text-xs text-muted';

/** Ce que l'on tape : sur fond creusé, chiffres alignés. */
export const input = (options: { block?: boolean; invalid?: boolean } = {}): string =>
  cx(
    'min-h-[2.125rem] rounded-control border bg-elevated px-2 py-1 text-text tabular-nums pointer-coarse:min-h-10',
    options.invalid ? 'border-accent' : 'border-line',
    options.block ? 'block w-full text-[0.9375rem]' : 'text-sm',
  );

// --- Sections de formulaire --------------------------------------------------

export const field = '[&+&]:mt-5';
export const label = 'text-xs font-semibold uppercase tracking-[0.04em] text-muted';
export const fieldLabel = `mb-2 ${label}`;
export const fieldRow = 'flex flex-wrap gap-2';
export const fieldStack = 'grid gap-2';

// --- Surfaces ----------------------------------------------------------------

/* En sombre, un bord clair en haut de carte : la lumière qui accroche l'arête. */
export const card = 'rounded-card border border-line bg-surface shadow-card dark:border-line/80 dark:border-t-edge';

export const notice = (error = false): string =>
  cx(
    'rounded-control border px-3 py-2.5 text-sm',
    error ? 'border-accent/50 bg-accent-wash' : 'border-line bg-elevated',
  );

// --- Pastilles ---------------------------------------------------------------

const badgeBase = 'inline-flex items-center whitespace-nowrap rounded-full px-[0.55rem] py-[0.1rem] text-xs font-semibold leading-[1.4]';

export const badge = {
  /** Pleine : salle, jour, mention « en cours ». */
  accent: `${badgeBase} bg-accent text-on-accent`,
  /** Aux couleurs du cours qui la porte (classe `kind-*` d'un parent). */
  kind: `${badgeBase} border border-kind/60 bg-kind/16 text-kind-ink`,
  change: `${badgeBase} border border-accent/55 text-accent`,
};

/** Compteur discret, à côté d'un titre. */
export const pill = 'rounded-full bg-elevated px-[0.45rem] py-[0.05rem] text-xs text-muted tabular-nums';

// --- Mise en page ------------------------------------------------------------

/** Écran centré : chargement, message d'absence. */
export const centered = 'grid h-full place-items-center p-6 text-center text-sm text-muted';

/** Les cartes de tête, qui chevauchent le bas du bandeau. */
export const summary = cx(
  'relative z-10 -mt-overlap grid shrink-0 gap-gap px-edge',
  'min-[60rem]:grid-cols-[minmax(0,1fr)_auto] min-[60rem]:items-start',
);

/** Une page, sous le bandeau : cartes de tête, zone qui défile, pied. */
export const page = 'flex min-h-0 flex-1 flex-col';

export const note = 'text-sm text-muted';
export const link = 'text-text underline';

/*
 * Un choix posé dans une carte de tête : la classe comparée, l'enseignant.
 * La disposition (qui s'étire, où va le texte) revient à l'appelant.
 */
export const pick = cx(
  'flex min-h-10 items-baseline rounded-control border border-transparent bg-elevated px-2 py-1 text-text cursor-pointer',
  'transition-[scale,border-color] duration-(--settle) ease-out active:scale-97 active:duration-(--press)',
  'hover:border-accent-line',
);
export const pickName = 'truncate text-base font-semibold tracking-[-0.01em]';

/** Section titrée d'une page : « Libres 12 », « À faire 3 ». */
export const sectionTitle = 'mb-2 flex items-center gap-2 text-[0.9375rem] font-semibold';

// --- Grilles horaires ----------------------------------------------------------

/** Liseré d'un créneau, dans la couleur de sa nature (classe `kind-*`). */
export const kindEdge = 'border-s-3 border-s-[color-mix(in_oklab,var(--color-kind)_var(--kind-edge),transparent)]';

/**
 * En-tête collant d'une grille, dans la zone `timetable`. Le coin se pose
 * au-dessus des jours (z-30) quand la grille défile dans les deux sens.
 */
export const gridHead = 'sticky-head z-20 border-b border-line bg-bg';
export const gridCorner = 'sticky-head left-0 z-30 border-b border-line bg-bg';

/** Repère de l'heure courante, sous le bandeau des jours (z-20). */
export const nowLine = cx(
  'pointer-events-none absolute inset-x-0 z-[5] h-px bg-accent',
  'before:absolute before:-left-1 before:-top-[0.1875rem] before:size-[0.4375rem] before:rounded-full before:bg-accent',
);

/** Graduation de la colonne des heures. */
export const hourMark = 'absolute right-2 -translate-y-1/2 text-2xs text-muted tabular-nums';
