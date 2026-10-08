import {
  For,
  Show,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  on,
  onCleanup,
  onMount,
  type Component,
  type JSX,
} from 'solid-js';
import {
  CalendarDays,
  Columns2,
  DoorOpen,
  Download,
  GraduationCap,
  LayoutGrid,
  ListChecks,
  List,
  ScanSearch,
  Settings,
  UserRound,
} from 'lucide-solid';

import { ApiError, clearCache, fetchWeek, fetchYears, type Course } from './lib/api';
import { diff, recall, remember, slotKey, weekKey, type Change } from './lib/changes';
import { loadHomework, type Draft } from './lib/homework';
import { groupBases, groupLabel, weekDays } from './lib/groups';
import { downloadIcs } from './lib/ics';
import { clock, greeting, periodLabel, resolveShownWeek } from './lib/time';
import { settings, update, type ViewMode } from './lib/settings';
import { centered, cx, note, press, summary as headCards } from './lib/ui';
import { AccountPage } from './components/AccountPage';
import { AnalysisPage } from './components/AnalysisPage';
import { ChangesBar } from './components/ChangesBar';
import { ComparePage } from './components/ComparePage';
import { CourseModal } from './components/CourseModal';
import { Footer } from './components/Footer';
import { HomeworkSheet } from './components/HomeworkSheet';
import { RoomsPage } from './components/RoomsPage';
import { TeachersPage } from './components/TeachersPage';
import { TodoPage } from './components/TodoPage';
import { SettingsSheet } from './components/SettingsSheet';
import { Setup } from './components/Setup';
import { StatusCard } from './components/StatusCard';
import { WeekBar } from './components/WeekBar';
import { WeekView } from './components/WeekView';

type Page = 'timetable' | 'compare' | 'teachers' | 'rooms' | 'analysis' | 'todo' | 'account';

const HASHES: Record<Page, string> = {
  timetable: '',
  compare: 'comparer',
  teachers: 'enseignants',
  rooms: 'salles',
  analysis: 'analyse',
  todo: 'devoirs',
  account: 'compte',
};

/**
 * La page suit l'ancre de l'URL : `#salles` se partage et revient avec
 * « Précédent ». `#compte` peut porter un motif d'échec de connexion.
 */
function pageFromHash(): Page {
  if (location.hash === '#salles') return 'rooms';
  if (location.hash === '#comparer') return 'compare';
  if (location.hash === '#enseignants') return 'teachers';
  if (location.hash === '#analyse') return 'analysis';
  if (location.hash === '#devoirs') return 'todo';
  return location.hash.startsWith('#compte') ? 'account' : 'timetable';
}

const TITLES: Record<Exclude<Page, 'timetable'>, [string, string]> = {
  compare: ['Comparer', 'Deux classes côte à côte, heure par heure.'],
  teachers: ['Enseignants', 'La semaine d\u2019un enseignant, les trois années réunies.'],
  rooms: ['Salles', 'Libres ou occupées, maintenant ou à l’heure choisie.'],
  analysis: ['Analyse', 'Les salles réservées deux fois dans les PDF que tu déposes.'],
  todo: ['Devoirs', 'Ce qu’il y a à faire, pour toi ou pour ta classe.'],
  account: ['Compte', 'Ta classe et tes messages privés sur Discord.'],
};

/**
 * Une commande du bandeau.
 *
 * Toutes ont le même gabarit et la même matière ; seule l'icône change. Le
 * libellé n'est pas affiché mais reste dit, à la souris comme au lecteur
 * d'écran : à cette taille, sept libellés ne tiendraient pas sur une ligne.
 *
 * Posée sur le rouge, c'est une matière claire sur une matière plus lourde —
 * jamais l'inverse, la lisibilité s'y perdrait. Dans un bloc, elle ne porte
 * plus son propre fond : c'est le bloc qui le donne, et seule celle qui est
 * active s'en détache.
 */
const Action: Component<{
  label: string;
  /** Rangée dans un bloc de commandes (`group`). */
  grouped?: boolean;
  /** Page en cours : « vous êtes ici », pour qui écoute la page. */
  current?: boolean;
  /** Choix retenu, pour une commande à deux états. */
  pressed?: boolean;
  disabled?: boolean;
  onClick: () => void;
  children: JSX.Element;
}> = (props) => (
  <button
    type="button"
    class={cx(
      press,
      'inline-flex min-h-9 min-w-9 items-center justify-center rounded-control border px-2.5 font-semibold text-on-accent',
      'cursor-pointer disabled:cursor-default disabled:opacity-40 pointer-coarse:min-h-10',
      // Sur téléphone, chaque pixel compte entre le salut et les commandes.
      'max-[34rem]:min-w-8 max-[34rem]:px-1.5',
      props.grouped
        ? 'border-transparent hover:not-disabled:bg-white/20'
        : 'border-white/22 bg-white/16 backdrop-blur-md hover:not-disabled:bg-white/30 reduce-transparency:backdrop-filter-none',
      // La page qu'on regarde, ou l'affichage choisi.
      'aria-[current=page]:bg-on-accent aria-[current=page]:text-accent aria-pressed:bg-on-accent aria-pressed:text-accent',
    )}
    title={props.label}
    aria-label={props.label}
    aria-current={props.current ? 'page' : undefined}
    aria-pressed={props.pressed}
    disabled={props.disabled}
    onClick={props.onClick}
  >
    {props.children}
  </button>
);

/** Les pages, dans l'ordre où elles se suivent dans le bandeau. */
const NAV: { page: Page; label: string; icon: () => JSX.Element }[] = [
  { page: 'timetable', label: 'Emploi du temps', icon: () => <CalendarDays size={16} aria-hidden="true" /> },
  { page: 'compare', label: 'Comparer deux classes', icon: () => <Columns2 size={16} aria-hidden="true" /> },
  { page: 'teachers', label: 'Emploi du temps d’un enseignant', icon: () => <GraduationCap size={16} aria-hidden="true" /> },
  { page: 'rooms', label: 'Salles libres', icon: () => <DoorOpen size={16} aria-hidden="true" /> },
  { page: 'analysis', label: 'Analyser des PDF', icon: () => <ScanSearch size={16} aria-hidden="true" /> },
  { page: 'todo', label: 'Devoirs à faire', icon: () => <ListChecks size={16} aria-hidden="true" /> },
  { page: 'account', label: 'Compte Discord', icon: () => <UserRound size={16} aria-hidden="true" /> },
];

/*
 * Les commandes se lisent en blocs : l'affichage de la semaine, les pages,
 * puis les réglages. Ce qui va ensemble tient dans la même matière claire, ce
 * qui est isolé flotte seul.
 */
const group =
  'flex items-center gap-0.5 rounded-[calc(var(--radius-control)+0.1875rem)] bg-white/14 p-[0.1875rem] backdrop-blur-md max-[34rem]:gap-0 max-[34rem]:p-0.5';

/** Les deux façons de lire la semaine. */
const VIEWS: { value: ViewMode; label: string; icon: () => JSX.Element }[] = [
  { value: 'grid', label: 'Grille horaire', icon: () => <LayoutGrid size={16} aria-hidden="true" /> },
  { value: 'list', label: 'Liste par jour', icon: () => <List size={16} aria-hidden="true" /> },
];

export function App() {
  const [page, setPage] = createSignal<Page>(pageFromHash());
  const goTo = (next: Page) => {
    location.hash = HASHES[next];
  };
  const [years, { refetch: refetchYears }] = createResource(fetchYears);
  const [week, setWeek] = createSignal<number | null>(null);
  const [selected, setSelected] = createSignal<Course | null>(null);
  const [showSettings, setShowSettings] = createSignal(false);
  /** Le devoir en cours de rédaction, préparé par le cours d'où il vient. */
  const [noting, setNoting] = createSignal<Partial<Draft> | null>(null);

  const summary = createMemo(() => years()?.find((item) => item.year === settings().year));
  const weeks = () => summary()?.weeks ?? [];

  /**
   * Semaine contenant aujourd'hui, déduite des périodes publiées — ou déjà la
   * suivante le week-end, quand il n'y a plus cours avant lundi.
   */
  const [currentWeek] = createResource(
    () => {
      const { year, group } = settings();
      const found = summary();
      return year !== null && found ? ({ year, group, summary: found } as const) : undefined;
    },
    ({ year, group, summary: found }) => resolveShownWeek(year, found, group),
  );

  // La semaine affichée suit la semaine courante tant que rien n'a été choisi,
  // et repart à zéro quand on change d'année.
  createEffect(() => {
    const current = currentWeek();
    if (current !== undefined && week() === null) setWeek(current);
  });
  createEffect(on(() => settings().year, () => setWeek(null), { defer: true }));

  /**
   * Deux lectures : la semaine filtrée sur le groupe (ce qu'on affiche) et la
   * semaine complète (qui donne les jours occupés et la liste des groupes).
   */
  const [data, { refetch: refetchData }] = createResource(
    () => {
      const { year, group } = settings();
      const target = week();
      return year !== null && group !== null && target !== null
        ? ({ year, group, week: target } as const)
        : undefined;
    },
    async ({ year, group, week: target }) => {
      const [structure, filtered] = await Promise.all([
        fetchWeek(year, target),
        fetchWeek(year, target, group),
      ]);
      return { structure, filtered };
    },
  );

  /** Semaine courante filtrée : elle alimente la carte d'état. */
  const [todayWeek, { refetch: refetchTodayWeek }] = createResource(
    () => {
      const { year, group } = settings();
      const current = currentWeek();
      return year !== null && group !== null && current !== undefined
        ? ({ year, group, week: current } as const)
        : undefined;
    },
    ({ year, group, week: target }) => fetchWeek(year, target, group),
  );

  /**
   * Ce qui a bougé dans la semaine affichée depuis la dernière fois qu'on l'a
   * regardée. La première visite ne signale rien : elle ne fait que retenir.
   */
  const [changes, setChanges] = createSignal<Change[]>([]);

  /** La semaine affichée, telle qu'on peut la mémoriser. */
  const snapshot = () => {
    const loaded = data();
    const { year, group } = settings();
    const target = week();
    return loaded && year !== null && group !== null && target !== null
      ? ({ key: weekKey(year, target, group), courses: loaded.filtered.courses } as const)
      : null;
  };

  createEffect(() => {
    const current = snapshot();
    if (!current) {
      setChanges([]);
      return;
    }
    const seen = recall(current.key);
    if (!seen) {
      remember(current.key, current.courses);
      setChanges([]);
      return;
    }
    setChanges(diff(seen.courses, current.courses));
  });

  /** « J'ai vu » : la version affichée devient la référence. */
  const markSeen = () => {
    const current = snapshot();
    if (current) remember(current.key, current.courses);
    setChanges([]);
  };

  /** Les créneaux à signaler sur la grille, par créneau. */
  const marks = createMemo(() => {
    const map = new Map<string, string>();
    for (const change of changes()) {
      if (change.kind !== 'removed') map.set(slotKey(change.course), change.kind);
    }
    return map;
  });

  /** Relit tout ce qui est affiché, sans passer par les caches. */
  const reload = () => {
    clearCache();
    void refetchYears();
    void refetchData();
    void refetchTodayWeek();
  };

  const bases = createMemo(() => {
    const loaded = data();
    return loaded ? groupBases(loaded.structure) : [];
  });

  /** `14 – 19 sept.` pour la semaine affichée. */
  const period = createMemo(() => {
    const loaded = data();
    return loaded ? periodLabel(loaded.filtered) : undefined;
  });

  /** Volume horaire de la semaine affichée. */
  const workload = createMemo(() => {
    const loaded = data();
    return loaded
      ? loaded.filtered.courses.reduce((sum, course) => sum + course.duration_minutes, 0)
      : 0;
  });

  const step = (delta: number) => {
    const list = weeks();
    const next = list[list.indexOf(week() ?? -1) + delta];
    if (next !== undefined) {
      setWeek(next);
      setSelected(null);
    }
  };

  onMount(() => void loadHomework());

  onMount(() => {
    const onHash = () => {
      setPage(pageFromHash());
      setSelected(null);
    };
    window.addEventListener('hashchange', onHash);
    onCleanup(() => window.removeEventListener('hashchange', onHash));
  });

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setSelected(null);
        setShowSettings(false);
      }
      // Les flèches changent de semaine, sur l'emploi du temps comme sur la
      // comparaison : les deux pages en affichent une.
      if (page() === 'rooms' || page() === 'analysis' || page() === 'account') return;
      if (event.key === 'ArrowLeft') step(-1);
      if (event.key === 'ArrowRight') step(1);
    };
    window.addEventListener('keydown', onKey);
    onCleanup(() => window.removeEventListener('keydown', onKey));
  });

  const error = () => data.error as unknown;

  return (
    <Show
      when={years()}
      fallback={
        <main class="grid min-h-dvh place-items-center p-5">
          <Show
            when={!years.error}
            fallback={<p class={note}>Service injoignable. Réessaie plus tard.</p>}
          >
            <p class={note}>Chargement…</p>
          </Show>
        </main>
      }
    >
      {(loaded) => (
        <Show
          when={page() !== 'timetable' || (settings().year !== null && settings().group !== null)}
          fallback={<Setup years={loaded()} />}
        >
          <div class="flex h-dvh flex-col overflow-x-hidden">
            {/* Bandeau : qui on est, et les actions de l'application. Il laisse
                dépasser les cartes de tête sur sa hauteur, et se pose sur le
                contenu par une ombre, sans filet tranchant. */}
            <header
              class={cx(
                'bg-banner flex shrink-0 items-center gap-3 px-edge text-on-accent',
                'pt-[max(clamp(0.625rem,2.5vw,1rem),env(safe-area-inset-top))] pb-[calc(var(--spacing-overlap)+clamp(0.625rem,2vw,1rem))]',
                'shadow-[0_10px_30px_-22px_rgb(0_0_0/0.8)]',
              )}
            >
              {/* Le salut cède la place aux commandes plutôt que de les pousser dehors. */}
              <div class="min-w-0">
                <h1 class="truncate text-[clamp(1.125rem,4.5vw,1.375rem)] font-semibold">
                  {page() === 'timetable' ? greeting(clock()) : TITLES[page() as 'rooms'][0]}
                </h1>
                {/* Sur une surface colorée, un gris pâle disparaît : le
                    sous-titre garde du contraste plutôt que de s'effacer. */}
                <p class="mt-0.5 truncate text-[0.8125rem] tracking-[0.01em] text-white/88">
                  {page() === 'timetable'
                    ? `Vous êtes en ${groupLabel(settings().group ?? '')}.`
                    : TITLES[page() as 'rooms'][1]}
                </p>
              </div>

              <div class="ml-auto flex shrink-0 items-center gap-1.5 max-[34rem]:gap-1">
                {/* L'affichage de la semaine : deux états d'une même chose,
                    donc un sélecteur — et il ne paraît que là où il agit. Sur
                    téléphone, il s'efface : il reste à portée dans les réglages. */}
                <Show when={page() === 'timetable' || page() === 'teachers'}>
                  <div class={`${group} max-[34rem]:hidden`} role="group" aria-label="Affichage de la semaine">
                    <For each={VIEWS}>
                      {(item) => (
                        <Action
                          label={item.label}
                          grouped
                          pressed={settings().view === item.value}
                          onClick={() => update({ view: item.value })}
                        >
                          {item.icon()}
                        </Action>
                      )}
                    </For>
                  </div>
                </Show>

                <Show when={page() === 'timetable'}>
                  <Action
                    label="Exporter la semaine (.ics)"
                    disabled={!data()}
                    onClick={() => {
                      const loadedWeek = data();
                      if (loadedWeek) {
                        downloadIcs(loadedWeek.filtered, loadedWeek.filtered.courses, settings().group);
                      }
                    }}
                  >
                    <Download size={16} aria-hidden="true" />
                  </Action>
                </Show>

                {/* Les pages sont toutes là, toujours dans le même ordre, et
                    celle qu'on regarde est marquée : on sait où l'on est et
                    ce qu'il y a à côté, sans que le bandeau se réarrange. */}
                <nav class={group} aria-label="Pages">
                  <For each={NAV}>
                    {(item) => (
                      <Action
                        label={item.label}
                        grouped
                        current={page() === item.page}
                        onClick={() => goTo(item.page)}
                      >
                        {item.icon()}
                      </Action>
                    )}
                  </For>
                </nav>

                <Action label="Réglages" onClick={() => setShowSettings(true)}>
                  <Settings size={16} aria-hidden="true" />
                </Action>
              </div>
            </header>

            <Show
              when={page() === 'timetable'}
              fallback={
                page() === 'rooms' ? (
                  <RoomsPage years={loaded()} onSelect={setSelected} />
                ) : page() === 'analysis' ? (
                  <AnalysisPage />
                ) : page() === 'todo' ? (
                  <TodoPage />
                ) : page() === 'teachers' ? (
                  <TeachersPage
                    years={loaded()}
                    week={week()}
                    current={currentWeek()}
                    onWeek={(next) => {
                      setWeek(next);
                      setSelected(null);
                    }}
                    onSelect={setSelected}
                  />
                ) : page() === 'compare' ? (
                  <ComparePage
                    years={loaded()}
                    week={week()}
                    current={currentWeek()}
                    onWeek={(next) => {
                      setWeek(next);
                      setSelected(null);
                    }}
                    onSelect={setSelected}
                  />
                ) : (
                  <AccountPage years={loaded()} />
                )
              }
            >
              {/* Les deux cartes de tête chevauchent le bas du bandeau. */}
              <div class={headCards}>
                <Show when={todayWeek()} fallback={<div />}>
                  {(current) => <StatusCard courses={current().courses} />}
                </Show>

                <WeekBar
                  week={week()}
                  weeks={weeks()}
                  current={currentWeek()}
                  period={period()}
                  workload={workload()}
                  onWeek={(next) => {
                    setWeek(next);
                    setSelected(null);
                  }}
                />
              </div>

              <Show when={changes().length > 0}>
                <ChangesBar changes={changes()} onSeen={markSeen} />
              </Show>

              <main class="timetable">
                <Show
                  when={data()}
                  fallback={
                    <div class={centered}>
                      <Show when={error()} fallback={<p>Chargement de la semaine…</p>}>
                        <p>
                          {error() instanceof ApiError && (error() as ApiError).isMissing
                            ? `Aucun emploi du temps publié pour la semaine ${week()}.`
                            : `Emploi du temps indisponible : ${(error() as Error).message}`}
                        </p>
                      </Show>
                    </div>
                  }
                >
                  {(loadedWeek) => (
                    <Show
                      when={loadedWeek().filtered.courses.length > 0}
                      fallback={
                        <div class={centered}>
                          <p>{`Aucun cours pour ${groupLabel(settings().group ?? '')} cette semaine.`}</p>
                        </div>
                      }
                    >
                      <WeekView
                        week={loadedWeek().filtered}
                        days={weekDays(loadedWeek().structure)}
                        view={settings().view}
                        changed={(course) => marks().get(slotKey(course))}
                        onSelect={setSelected}
                      />
                    </Show>
                  )}
                </Show>
              </main>

              <Footer>
                <Show when={data()}>
                  {(loadedWeek) => (
                    <>
                      <span>{`${loadedWeek().filtered.count} cours · lus dans le PDF du département`}</span>
                      <a
                        class="ml-auto whitespace-nowrap underline"
                        href={loadedWeek().filtered.source.url}
                        target="_blank"
                        rel="noreferrer"
                      >
                        PDF d'origine
                      </a>
                    </>
                  )}
                </Show>
              </Footer>
            </Show>

            <CourseModal
              course={selected()}
              slot={
                page() === 'timetable' && settings().year !== null && week() !== null
                  ? { year: settings().year ?? 0, week: week() ?? 0 }
                  : undefined
              }
              onNote={(draft) => {
                // Une feuille à la fois : le détail du cours s'efface derrière
                // le formulaire qu'il vient d'ouvrir.
                setSelected(null);
                setNoting(draft);
              }}
              onClose={() => setSelected(null)}
            />

            <HomeworkSheet
              open={noting() !== null}
              initial={noting()}
              onClose={() => setNoting(null)}
            />

            <SettingsSheet
              open={showSettings()}
              bases={bases()}
              years={loaded().map((item) => item.year)}
              reloading={years.loading || data.loading || todayWeek.loading}
              onReload={reload}
              onClose={() => setShowSettings(false)}
            />
          </div>
        </Show>
      )}
    </Show>
  );
}
