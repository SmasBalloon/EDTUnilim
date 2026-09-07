import { Show, createEffect, createMemo, createResource, createSignal, on, onCleanup, onMount } from 'solid-js';

import { ApiError, fetchWeek, fetchYears, type Course } from './lib/api';
import { groupBases, groupLabel, weekDays } from './lib/groups';
import { downloadIcs } from './lib/ics';
import { clock, greeting, humanDuration, periodLabel, resolveCurrentWeek } from './lib/time';
import { settings, update } from './lib/settings';
import { CourseModal } from './components/CourseModal';
import { SettingsSheet } from './components/SettingsSheet';
import { Setup } from './components/Setup';
import { StatusCard } from './components/StatusCard';
import { WeekView } from './components/WeekView';

export function App() {
  const [years] = createResource(fetchYears);
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
  const [data] = createResource(
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
  const [todayWeek] = createResource(
    () => {
      const { year, group } = settings();
      const current = currentWeek();
      return year !== null && group !== null && current !== undefined
        ? ({ year, group, week: current } as const)
        : undefined;
    },
    ({ year, group, week: target }) => fetchWeek(year, target, group),
  );

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
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setSelected(null);
        setShowSettings(false);
      }
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
          when={settings().year !== null && settings().group !== null}
          fallback={<Setup years={loaded()} />}
        >
          <div class="app">
            {/* Bandeau : qui on est, et les actions de l'application. */}
            <header class="banner">
              <div>
                <h1 class="banner__greeting">{greeting(clock())}</h1>
                <p class="banner__identity">
                  {`Vous êtes en ${groupLabel(settings().group ?? '')}.`}
                </p>
              </div>

              <div class="banner__actions">
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
                  .ics
                </button>
                <button
                  type="button"
                  class="button button--on-accent"
                  title="Réglages"
                  onClick={() => setShowSettings(true)}
                >
                  ⚙
                </button>
              </div>
            </header>

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
                    ‹
                  </button>
                  <button
                    type="button"
                    class="button button--round"
                    onClick={() => step(1)}
                    disabled={weeks().indexOf(week() ?? -1) >= weeks().length - 1}
                    aria-label="Semaine suivante"
                  >
                    ›
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

            <CourseModal course={selected()} onClose={() => setSelected(null)} />

            <SettingsSheet
              open={showSettings()}
              bases={bases()}
              years={loaded().map((item) => item.year)}
              onClose={() => setShowSettings(false)}
            />
          </div>
        </Show>
      )}
    </Show>
  );
}
