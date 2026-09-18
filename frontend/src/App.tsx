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
  List,
  Settings,
  UserRound,
} from 'lucide-solid';

import { ApiError, clearCache, fetchWeek, fetchYears, type Course } from './lib/api';
import { diff, recall, remember, slotKey, weekKey, type Change } from './lib/changes';
import { groupBases, groupLabel, weekDays } from './lib/groups';
import { downloadIcs } from './lib/ics';
import { clock, greeting, periodLabel, resolveCurrentWeek } from './lib/time';
import { settings, update, type ViewMode } from './lib/settings';
import { AccountPage } from './components/AccountPage';
import { ChangesBar } from './components/ChangesBar';
import { ComparePage } from './components/ComparePage';
import { CourseModal } from './components/CourseModal';
import { RoomsPage } from './components/RoomsPage';
import { TeachersPage } from './components/TeachersPage';
import { SettingsSheet } from './components/SettingsSheet';
import { Setup } from './components/Setup';
import { StatusCard } from './components/StatusCard';
import { WeekBar } from './components/WeekBar';
import { WeekView } from './components/WeekView';

type Page = 'timetable' | 'compare' | 'teachers' | 'rooms' | 'account';

const HASHES: Record<Page, string> = {
  timetable: '',
  compare: 'comparer',
  teachers: 'enseignants',
  rooms: 'salles',
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
  return location.hash.startsWith('#compte') ? 'account' : 'timetable';
}

const TITLES: Record<Exclude<Page, 'timetable'>, [string, string]> = {
  compare: ['Comparer', 'Deux classes côte à côte, heure par heure.'],
  teachers: ['Enseignants', 'La semaine d\u2019un enseignant, les trois années réunies.'],
  rooms: ['Salles', 'Libres ou occupées, maintenant ou à l’heure choisie.'],
  account: ['Compte', 'Ta classe et tes messages privés sur Discord.'],
};

/**
 * Une commande du bandeau.
 *
 * Toutes ont le même gabarit et la même matière ; seule l'icône change. Le
 * libellé n'est pas affiché mais reste dit, à la souris comme au lecteur
 * d'écran : à cette taille, sept libellés ne tiendraient pas sur une ligne.
 */
const Action: Component<{
  label: string;
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
    class="button button--on-accent"
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
  { page: 'account', label: 'Compte Discord', icon: () => <UserRound size={16} aria-hidden="true" /> },
];

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

  const summary = createMemo(() => years()?.find((item) => item.year === settings().year));
  const weeks = () => summary()?.weeks ?? [];

  /** Semaine contenant aujourd'hui, déduite des périodes publiées. */
  const [currentWeek] = createResource(
    () => {
      const year = settings().year;
      const found = summary();
      return year !== null && found ? ({ year, summary: found } as const) : undefined;
    },
    ({ year, summary: found }) => resolveCurrentWeek(year, found),
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
      if (page() === 'rooms' || page() === 'account') return;
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
        <main class="setup">
          <Show
            when={!years.error}
            fallback={<p class="setup__note">Service injoignable. Réessaie plus tard.</p>}
          >
            <p class="setup__note">Chargement…</p>
          </Show>
        </main>
      }
    >
      {(loaded) => (
        <Show
          when={page() !== 'timetable' || (settings().year !== null && settings().group !== null)}
          fallback={<Setup years={loaded()} />}
        >
          <div class="app">
            {/* Bandeau : qui on est, et les actions de l'application. */}
            <header class="banner">
              <div class="banner__who">
                <h1 class="banner__greeting">
                  {page() === 'timetable' ? greeting(clock()) : TITLES[page() as 'rooms'][0]}
                </h1>
                <p class="banner__identity">
                  {page() === 'timetable'
                    ? `Vous êtes en ${groupLabel(settings().group ?? '')}.`
                    : TITLES[page() as 'rooms'][1]}
                </p>
              </div>

              <div class="banner__actions">
                {/* L'affichage de la semaine : deux états d'une même chose,
                    donc un sélecteur — et il ne paraît que là où il agit. */}
                <Show when={page() === 'timetable' || page() === 'teachers'}>
                  <div class="banner__group banner__views" role="group" aria-label="Affichage de la semaine">
                    <For each={VIEWS}>
                      {(item) => (
                        <Action
                          label={item.label}
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
                <nav class="banner__group" aria-label="Pages">
                  <For each={NAV}>
                    {(item) => (
                      <Action
                        label={item.label}
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
              <div class="summary">
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

              <main class="app__timetable">
                <Show
                  when={data()}
                  fallback={
                    <div class="centered">
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
                        <div class="centered">
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

              <footer class="app__footer">
                <Show when={data()}>
                  {(loadedWeek) => (
                    <>
                      <span>{`${loadedWeek().filtered.count} cours · lus dans le PDF du département`}</span>
                      <a
                        href={loadedWeek().filtered.source.url}
                        target="_blank"
                        rel="noreferrer"
                      >
                        PDF d'origine
                      </a>
                    </>
                  )}
                </Show>
              </footer>
            </Show>

            <CourseModal course={selected()} onClose={() => setSelected(null)} />

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
