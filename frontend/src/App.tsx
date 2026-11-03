import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  on,
  onCleanup,
  onMount,
} from 'solid-js';
import { Download } from 'lucide-solid';

import { ApiError, clearCache, fetchWeek, fetchYears, type Course } from './lib/api';
import { diff, recall, remember, slotKey, weekKey, type Change } from './lib/changes';
import { loadHomework, type Draft } from './lib/homework';
import { groupBases, groupLabel, weekDays } from './lib/groups';
import { downloadIcs } from './lib/ics';
import { clock, greeting, periodLabel, resolveShownWeek } from './lib/time';
import { settings, update, type ViewMode } from './lib/settings';
import { button, segment } from './lib/ui';
import { AccountPage } from './components/AccountPage';
import { AnalysisPage } from './components/AnalysisPage';
import { PdfsPage } from './components/PdfsPage';
import { ChangesBar } from './components/ChangesBar';
import { ComparePage } from './components/ComparePage';
import { CourseModal } from './components/CourseModal';
import { HomeworkSheet } from './components/HomeworkSheet';
import { RoomsPage } from './components/RoomsPage';
import { Empty, Foot, PageHead, Rail, type Page } from './components/Shell';
import { TeachersPage } from './components/TeachersPage';
import { TodoPage } from './components/TodoPage';
import { SettingsSheet } from './components/SettingsSheet';
import { Setup } from './components/Setup';
import { StatusCard } from './components/StatusCard';
import { WeekBar } from './components/WeekBar';
import { WeekView } from './components/WeekView';

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
  if (location.hash === '#pdf') return 'pdfs';
  return location.hash.startsWith('#compte') ? 'account' : 'timetable';
}

/** Les deux façons de lire la semaine. */
const VIEWS: { value: ViewMode; label: string }[] = [
  { value: 'grid', label: 'Grille' },
  { value: 'list', label: 'Liste' },
];

/** Les quatre natures de la légende, dans l'ordre de la maquette. */
const LEGEND: [string, string][] = [
  ['t-sae', 'SAÉ'],
  ['t-cm', 'CM'],
  ['t-td', 'TD'],
  ['t-tp', 'TP'],
];

export function App() {
  const [page, setPage] = createSignal<Page>(pageFromHash());
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
      window.scrollTo({ top: 0 });
    };
    window.addEventListener('hashchange', onHash);
    onCleanup(() => window.removeEventListener('hashchange', onHash));
  });

  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      // Les flèches changent de semaine, sur les pages qui en affichent une ;
      // jamais pendant une saisie ni sous une boîte de dialogue.
      if (page() !== 'timetable' && page() !== 'compare' && page() !== 'teachers') return;
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, select, textarea, [role="dialog"], [role="tablist"]')) return;
      if (event.key === 'ArrowLeft') step(-1);
      if (event.key === 'ArrowRight') step(1);
    };
    window.addEventListener('keydown', onKey);
    onCleanup(() => window.removeEventListener('keydown', onKey));
  });

  const error = () => data.error as unknown;
  const weekRoute = () => ({ week: week(), current: currentWeek(), onWeek: (next: number) => {
    setWeek(next);
    setSelected(null);
  } });

  return (
    <Show
      when={years()}
      fallback={
        <div class="grid min-h-dvh place-items-center p-5">
          <Show
            when={!years.error}
            fallback={<Empty title="Service injoignable">Réessaie plus tard.</Empty>}
          >
            <Empty>Chargement…</Empty>
          </Show>
        </div>
      }
    >
      {(loaded) => (
        <Show
          when={page() !== 'timetable' || (settings().year !== null && settings().group !== null)}
          fallback={<Setup years={loaded()} />}
        >
          <div class="app">
            <Rail page={page()} onSettings={() => setShowSettings(true)} />

            <main class="main" id="contenu">
              <Switch>
                <Match when={page() === 'rooms'}>
                  <RoomsPage years={loaded()} onSelect={setSelected} />
                </Match>
                <Match when={page() === 'analysis'}>
                  <AnalysisPage />
                </Match>
                <Match when={page() === 'pdfs'}>
                  <PdfsPage />
                </Match>
                <Match when={page() === 'todo'}>
                  <TodoPage />
                </Match>
                <Match when={page() === 'account'}>
                  <AccountPage years={loaded()} />
                </Match>
                <Match when={page() === 'teachers'}>
                  <TeachersPage years={loaded()} {...weekRoute()} onSelect={setSelected} />
                </Match>
                <Match when={page() === 'compare'}>
                  <ComparePage years={loaded()} {...weekRoute()} onSelect={setSelected} />
                </Match>
                <Match when={page() === 'timetable'}>
                  <PageHead
                    title={greeting(clock())}
                    sub={`Vous êtes en ${groupLabel(settings().group ?? '')} · BUT ${settings().year ?? ''}`}
                  >
                    <div class={segment} role="group" aria-label="Affichage de la semaine">
                      <For each={VIEWS}>
                        {(item) => (
                          <button
                            type="button"
                            aria-pressed={settings().view === item.value}
                            onClick={() => update({ view: item.value })}
                          >
                            {item.label}
                          </button>
                        )}
                      </For>
                    </div>
                    <button
                      type="button"
                      class={button.plain}
                      disabled={!data()}
                      title="Exporter la semaine (.ics)"
                      onClick={() => {
                        const loadedWeek = data();
                        if (loadedWeek) {
                          downloadIcs(loadedWeek.filtered, loadedWeek.filtered.courses, settings().group);
                        }
                      }}
                    >
                      <Download size={20} aria-hidden="true" />
                      <span class="max-[520px]:sr-only">Exporter .ics</span>
                    </button>
                    <WeekBar
                      week={week()}
                      weeks={weeks()}
                      current={currentWeek()}
                      period={period()}
                      workload={workload()}
                      onWeek={weekRoute().onWeek}
                    />
                  </PageHead>

                  <Show when={changes().length > 0}>
                    <ChangesBar changes={changes()} onSeen={markSeen} />
                  </Show>

                  <div class="row items-stretch">
                    <Show when={todayWeek()}>{(current) => <StatusCard courses={current().courses} />}</Show>
                    <Show when={data()}>
                      {(loadedWeek) => (
                        <div class="card week-strip flex-[2_1_420px]">
                          <b>{`${loadedWeek().filtered.count} cours cette semaine`}</b>
                          <span class="row gap-2">
                            <For each={LEGEND}>{([tone, name]) => <span class={`tag ${tone}`}>{name}</span>}</For>
                          </span>
                          <span class="src max-[640px]:hidden">Lus dans le PDF du département</span>
                        </div>
                      )}
                    </Show>
                  </div>

                  <Show
                    when={data()}
                    fallback={
                      <Show when={error()} fallback={<Empty>Chargement de la semaine…</Empty>}>
                        {error() instanceof ApiError && (error() as ApiError).isMissing ? (
                          <Empty title="Semaine non publiée">
                            {`Aucun emploi du temps publié pour la semaine ${week()}.`}
                          </Empty>
                        ) : (
                          <Empty title="Emploi du temps indisponible">{(error() as Error).message}</Empty>
                        )}
                      </Show>
                    }
                  >
                    {(loadedWeek) => (
                      <Show
                        when={loadedWeek().filtered.courses.length > 0}
                        fallback={
                          <Empty title="Aucun cours">
                            {`Rien pour ${groupLabel(settings().group ?? '')} cette semaine.`}
                          </Empty>
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

                  <Show when={data()}>
                    {(loadedWeek) => (
                      <Foot>
                        <span>{`${loadedWeek().filtered.count} cours · lus dans le PDF du département`}</span>
                        <a class="lnk" href={loadedWeek().filtered.source.url} target="_blank" rel="noreferrer">
                          PDF d'origine
                        </a>
                      </Foot>
                    )}
                  </Show>
                </Match>
              </Switch>
            </main>

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
