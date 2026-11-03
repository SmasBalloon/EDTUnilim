import { For, Show, createSignal, type Component, type JSX, type ParentComponent } from 'solid-js';
import {
  CalendarDays,
  Columns2,
  DoorOpen,
  Ellipsis,
  GraduationCap,
  ListChecks,
  ScanSearch,
  Settings,
  UserRound,
} from 'lucide-solid';

import { cx } from '../lib/ui';
import { Sheet, SheetHeader } from './Sheet';

export type Page = 'timetable' | 'compare' | 'teachers' | 'rooms' | 'analysis' | 'todo' | 'account' | 'pdfs';

export const HASHES: Record<Page, string> = {
  timetable: '',
  compare: 'comparer',
  teachers: 'enseignants',
  rooms: 'salles',
  analysis: 'analyse',
  todo: 'devoirs',
  account: 'compte',
  pdfs: 'pdf',
};

/** Icône de menu : trait de 22 px, comme dans la maquette (COMPONENTS.md). */
const icon = (Icon: typeof CalendarDays) => () => <Icon size={22} strokeWidth={1.7} aria-hidden="true" />;

interface Entry {
  page: Page;
  label: string;
  /** Rangée dans « Plus » sur téléphone (INTEGRATION.md § 7). */
  extra?: boolean;
  icon: () => JSX.Element;
}

const MAIN: Entry[] = [
  { page: 'timetable', label: 'Semaine', icon: icon(CalendarDays) },
  { page: 'compare', label: 'Comparer', extra: true, icon: icon(Columns2) },
  { page: 'teachers', label: 'Enseignants', extra: true, icon: icon(GraduationCap) },
  { page: 'rooms', label: 'Salles', icon: icon(DoorOpen) },
  { page: 'analysis', label: 'Analyse', extra: true, icon: icon(ScanSearch) },
  { page: 'todo', label: 'Devoirs', icon: icon(ListChecks) },
];

const ACCOUNT: Entry = { page: 'account', label: 'Compte', extra: true, icon: icon(UserRound) };

const href = (page: Page) => `#${HASHES[page]}`;

/**
 * Le menu vertical de la refonte. Sur téléphone, il devient la barre
 * d'onglets du bas : Semaine, Salles, Devoirs, et « Plus » pour le reste.
 */
export const Rail: Component<{ page: Page; onSettings: () => void }> = (props) => {
  const [more, setMore] = createSignal(false);

  const item = (entry: Entry) => (
    <a
      class={cx('ni', props.page === entry.page && 'on', entry.extra && 'ni-extra')}
      href={href(entry.page)}
      aria-current={props.page === entry.page ? 'page' : undefined}
    >
      {entry.icon()}
      {entry.label}
    </a>
  );

  /** « Plus » est actif quand la page ouverte y est rangée. */
  const inMore = () => [...MAIN, ACCOUNT].some((entry) => entry.extra && entry.page === props.page) || props.page === 'pdfs';

  return (
    <>
      <nav class="rail" aria-label="Navigation">
        <a class="logo" href="#" aria-label="EDT — ma semaine">EDT</a>
        <For each={MAIN}>{item}</For>
        <div class="sp" />
        {item(ACCOUNT)}
        <button type="button" class="ni ni-extra" onClick={props.onSettings}>
          {icon(Settings)()}
          Réglages
        </button>
        <button
          type="button"
          class={cx('ni ni-more', inMore() && 'on')}
          aria-haspopup="dialog"
          onClick={() => setMore(true)}
        >
          {icon(Ellipsis)()}
          Plus
        </button>
      </nav>

      <Sheet open={more()} onClose={() => setMore(false)} label="Plus">
        <SheetHeader title="Plus" onClose={() => setMore(false)} />
        <div class="grid gap-2">
          <For each={[...MAIN, ACCOUNT].filter((entry) => entry.extra)}>
            {(entry) => (
              <a
                class={cx('btn', props.page === entry.page && 'pri')}
                href={href(entry.page)}
                onClick={() => setMore(false)}
              >
                {entry.icon()}
                {entry.label}
              </a>
            )}
          </For>
          <button
            type="button"
            class="btn"
            onClick={() => {
              setMore(false);
              props.onSettings();
            }}
          >
            {icon(Settings)()}
            Réglages
          </button>
        </div>
      </Sheet>
    </>
  );
};

/** L'en-tête d'une page : titre et sous-titre à gauche, commandes à droite. */
export const PageHead: Component<{ title: string; sub?: string; children?: JSX.Element }> = (props) => (
  <div class="top">
    <div class="min-w-0">
      <h1>{props.title}</h1>
      <Show when={props.sub}>
        <p class="sub">{props.sub}</p>
      </Show>
    </div>
    <Show when={props.children}>
      <div class="row">{props.children}</div>
    </Show>
  </div>
);

/** État vide, chargement ou absence : bordure en pointillés (`.empty`). */
export const Empty: ParentComponent<{ title?: string }> = (props) => (
  <div class="empty" role="status">
    <Show when={props.title}>
      <h3>{props.title}</h3>
    </Show>
    <Show when={props.children}>
      <p>{props.children}</p>
    </Show>
  </div>
);

/** Pied de page discret : source des données, lien PDF. */
export const Foot: ParentComponent = (props) => <div class="foot gap-4 flex-wrap">{props.children}</div>;
