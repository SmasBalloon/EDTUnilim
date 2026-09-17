import { Show, createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount } from 'solid-js';
import { CalendarDays, ChevronLeft, ChevronRight, DoorOpen, Download, Settings } from 'lucide-solid';

import { ApiError, clearCache, fetchWeek, fetchYears, type Course } from './lib/api';
import { groupBases, groupLabel, weekDays } from './lib/groups';
import { downloadIcs } from './lib/ics';
import { clock, greeting, humanDuration, periodLabel, resolveCurrentWeek } from './lib/time';
import { settings, update } from './lib/settings';
import { CourseModal } from './components/CourseModal';
import { RoomsPage } from './components/RoomsPage';
import { SettingsSheet } from './components/SettingsSheet';
import { Setup } from './components/Setup';
import { StatusCard } from './components/StatusCard';
import { WeekView } from './components/WeekView';

type Page = 'timetable' | 'rooms';

/** La page suit l'ancre de l'URL : `#salles` se partage et revient avec « Précédent ». */
function pageFromHash(): Page {
  return location.hash === '#salles' ? 'rooms' : 'timetable';
}

export function App() {
  const [page, setPage] = createSignal<Page>(pageFromHash());
  const goTo = (next: Page) => {
    location.hash = next === 'rooms' ? 'salles' : '';
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
      if (page() !== 'timetable') return;
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
          when={page() === 'rooms' || (settings().year !== null && settings().group !== null)}
          fallback={<Setup years={loaded()} />}
        >
          <div class="app">
            {/* Bandeau : qui on est, et les actions de l'application. */}
            <header class="banner">
              <div>
                <h1 class="banner__greeting">
                  {page() === 'rooms' ? 'Salles' : greeting(clock())}
                </h1>
                <p class="banner__identity">
                  {page() === 'rooms'
                    ? 'Libres ou occupées, maintenant ou à l’heure choisie.'
                    : `Vous êtes en ${groupLabel(settings().group ?? '')}.`}
                </p>
              </div>

              <div class="banner__actions">
                <Show
                  when={page() === 'timetable'}
                  fallback={
                    <button
                      type="button"
                      class="button button--on-accent"
                      title="Emploi du temps"
                      aria-label="Emploi du temps"
                      onClick={() => goTo('timetable')}
                    >
                      <CalendarDays size={16} aria-hidden="true" />
                      <span class="button__label">Emploi du temps</span>
                    </button>
                  }
                >
                  <button
                    type="button"
                    class="button button--on-accent"
                    aria-pressed={settings().view === 'grid'}
                    onClick={() => update({ view: 'grid' })}
                  >
                    Grille
                  </button>
                  <button
                    type="button"
                    class="button button--on-accent"
                    aria-pressed={settings().view === 'list'}
                    onClick={() => update({ view: 'list' })}
                  >
                    Liste
                  </button>
                  <button
                    type="button"
                    class="button button--on-accent"
                    title="Exporter la semaine (.ics)"
                    disabled={!data()}
                    onClick={() => {
                      const loadedWeek = data();
                      if (loadedWeek) {
                        downloadIcs(loadedWeek.filtered, loadedWeek.filtered.courses, settings().group);
                      }
                    }}
                  >
                    <Download size={16} aria-hidden="true" />
                    .ics
                  </button>
                  <button
                    type="button"
                    class="button button--on-accent"
                    title="Salles libres"
                    aria-label="Salles libres"
                    onClick={() => goTo('rooms')}
                  >
                    <DoorOpen size={16} aria-hidden="true" />
                    <span class="button__label">Salles</span>
                  </button>
                </Show>
                <button
                  type="button"
                  class="button button--on-accent"
                  title="Réglages"
                  aria-label="Réglages"
                  onClick={() => setShowSettings(true)}
                >
                  <Settings size={16} aria-hidden="true" />
                </button>
              </div>
            </header>

            <Show
              when={page() === 'timetable'}
              fallback={<RoomsPage years={loaded()} onSelect={setSelected} />}
            >
              {/* Les deux cartes de tête chevauchent le bas du bandeau. */}
              <div class="summary">
                <Show when={todayWeek()} fallback={<div />}>
                  {(current) => <StatusCard courses={current().courses} />}
                </Show>

                <div class="card week-summary">
                  <div class="week-summary__info">
                    <span class="week-summary__number">{`Semaine ${week() ?? '—'}`}</span>
                    <Show when={data()}>
                      {(loadedWeek) => (
                        <span class="week-summary__period">{periodLabel(loadedWeek().filtered)}</span>
                      )}
                    </Show>
                    <Show when={workload() > 0}>
                      <span class="week-summary__workload">{humanDuration(workload())}</span>
                    </Show>
                  </div>

                  <div class="week-summary__nav">
                    <Show when={currentWeek() !== undefined && week() !== currentWeek()}>
                      <button
                        type="button"
                        class="button"
                        onClick={() => {
                          setWeek(currentWeek() ?? null);
                          setSelected(null);
                        }}
                      >
                        Aujourd'hui
                      </button>
                    </Show>
                    <button
                      type="button"
                      class="button button--round"
                      onClick={() => step(-1)}
                      disabled={weeks().indexOf(week() ?? -1) <= 0}
                      aria-label="Semaine précédente"
                    >
                      <ChevronLeft size={18} aria-hidden="true" />
                    </button>
                    <button
                      type="button"
                      class="button button--round"
                      onClick={() => step(1)}
                      disabled={weeks().indexOf(week() ?? -1) >= weeks().length - 1}
                      aria-label="Semaine suivante"
                    >
                      <ChevronRight size={18} aria-hidden="true" />
                    </button>
                  </div>
                </div>
              </div>

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
