import {
  For,
  Show,
  createEffect,
  createMemo,
  createResource,
  createSignal,
  on,
  type Component,
} from 'solid-js';
import { ArrowLeftRight, Plus, X } from 'lucide-solid';

import { ApiError, fetchWeek, type Course, type Week, type YearSummary } from '../lib/api';
import { groupBases, groupLabel, weekDays } from '../lib/groups';
import { settings, update, type Side } from '../lib/settings';
import { DAY_ORDER, dayIndex, periodLabel } from '../lib/time';
import { useCompact } from '../lib/media';
import { CompareGrid } from './CompareGrid';
import { DayTabs } from './DayTabs';
import { GroupPicker } from './GroupPicker';
import { Pager } from './Pager';
import { Sheet } from './Sheet';
import { WeekBar } from './WeekBar';

interface Props {
  years: YearSummary[];
  week: number | null;
  current: number | undefined;
  onWeek: (week: number) => void;
  onSelect: (course: Course) => void;
}

type Which = 'left' | 'right';

/** Ce qu'une classe a donné pour la semaine demandée. */
interface Loaded {
  /** La semaine entière, tous groupes : elle donne les jours et les groupes. */
  structure: Week;
  /** La semaine filtrée sur le groupe : ce qui s'affiche. */
  filtered: Week;
}

function label(side: Side | null): string {
  return side === null ? 'Choisir' : groupLabel(side.group);
}

/**
 * Deux classes côte à côte, sur la même échelle horaire.
 *
 * La classe de gauche est la sienne par défaut, celle de droite se choisit ;
 * l'une et l'autre peuvent venir de n'importe quelle année, chacune étant lue
 * séparément — une semaine non publiée d'un côté ne prive pas de l'autre.
 */
export const ComparePage: Component<Props> = (props) => {
  const compact = useCompact();
  const [editing, setEditing] = createSignal<Which | null>(null);

  /** À gauche, `null` veut dire « ma classe ». */
  const side = (which: Which): Side | null => {
    const saved = settings().compare[which];
    if (saved) return saved;
    const { year, group } = settings();
    return which === 'left' && year !== null && group !== null ? { year, group } : null;
  };

  const choose = (which: Which, chosen: Side) => {
    update({ compare: { ...settings().compare, [which]: chosen } });
  };

  const swap = () => {
    update({ compare: { left: side('right'), right: side('left') } });
  };

  const load = (which: Which) =>
    createResource(
      () => {
        const chosen = side(which);
        const week = props.week;
        return chosen && week !== null ? ({ ...chosen, week } as const) : undefined;
      },
      async ({ year, group, week }): Promise<Loaded> => {
        const [structure, filtered] = await Promise.all([
          fetchWeek(year, week),
          fetchWeek(year, week, group),
        ]);
        return { structure, filtered };
      },
    );

  const [left] = load('left');
  const [right] = load('right');

  /** Ce qui manque à l'une ou l'autre des classes, dit en une phrase. */
  const troubles = createMemo(() => {
    const said: string[] = [];
    for (const [which, data, failure] of [
      ['left', left(), left.error] as const,
      ['right', right(), right.error] as const,
    ]) {
      const chosen = side(which);
      if (chosen === null) continue;
      const who = `${groupLabel(chosen.group)} (BUT ${chosen.year})`;
      if (failure instanceof ApiError) {
        said.push(`${who} : ${failure.isMissing ? 'semaine non publiée' : failure.message}.`);
      } else if (failure) {
        said.push(`${who} : emploi du temps indisponible.`);
      } else if (data && data.filtered.courses.length === 0) {
        said.push(`${who} : aucun cours cette semaine.`);
      }
    }
    return said;
  });

  /** Les jours occupés par l'une ou l'autre des deux classes. */
  const days = createMemo(() => {
    const seen = new Map<string, { name: string; date: string }>();
    for (const data of [left(), right()]) {
      if (!data) continue;
      for (const day of weekDays(data.structure)) seen.set(day.date, day);
    }
    return [...seen.values()].sort(
      (a, b) => DAY_ORDER.indexOf(a.name) - DAY_ORDER.indexOf(b.name),
    );
  });

  const sides = createMemo(() => ({
    left: { label: label(side('left')), courses: left()?.filtered.courses ?? [] },
    right: { label: label(side('right')), courses: right()?.filtered.courses ?? [] },
  }));

  const period = createMemo(() => {
    const data = left() ?? right();
    return data ? periodLabel(data.filtered) : undefined;
  });

  // Sur écran étroit, la semaine se feuillette : un jour par page. On retient
  // le jour ouvert plutôt que son rang, pour que la page soit juste dès le
  // premier rendu — sinon elle part de lundi et glisse jusqu'à aujourd'hui.
  const [chosen, setChosen] = createSignal<string | null>(null);
  const index = () => dayIndex(days(), chosen());

  const grid = (only: { name: string; date: string }[]) => (
    <CompareGrid
      days={only}
      left={sides().left}
      right={sides().right}
      onSelect={props.onSelect}
    />
  );

  return (
    <div class="compare">
      <div class="summary">
        <div class="card compare__sides">
          <For each={['left', 'right'] as Which[]}>
            {(which, position) => (
              <>
                <Show when={position() === 1}>
                  <button
                    type="button"
                    class="button button--round"
                    onClick={swap}
                    aria-label="Échanger les deux classes"
                    title="Échanger les deux classes"
                  >
                    <ArrowLeftRight size={16} aria-hidden="true" />
                  </button>
                </Show>
                <button
                  type="button"
                  class="compare__pick"
                  onClick={() => setEditing(which)}
                  aria-label={`Changer la classe ${position() === 0 ? 'de gauche' : 'de droite'}`}
                >
                  <Show
                    when={side(which)}
                    fallback={
                      <>
                        <Plus size={16} aria-hidden="true" />
                        <span class="compare__pick-name">Choisir une classe</span>
                      </>
                    }
                  >
                    {(chosen) => (
                      <>
                        <span class="compare__pick-name">{groupLabel(chosen().group)}</span>
                        <span class="compare__pick-year">{`BUT ${chosen().year}`}</span>
                      </>
                    )}
                  </Show>
                </button>
              </>
            )}
          </For>
        </div>

        <WeekBar
          week={props.week}
          weeks={props.years.find((item) => item.year === side('left')?.year)?.weeks ?? []}
          current={props.current}
          period={period()}
          onWeek={props.onWeek}
        />
      </div>

      <main class="app__timetable">
        <Show
          when={side('right')}
          fallback={
            <div class="centered">
              <p>Choisis une seconde classe pour la mettre en regard de la tienne.</p>
            </div>
          }
        >
          <Show when={days().length > 0} fallback={<div class="centered"><p>Chargement…</p></div>}>
            <For each={troubles()}>
              {(said) => <p class="notice compare__notice">{said}</p>}
            </For>

            <div class="week-view" classList={{ 'week-view--single': compact() }}>
              <Show when={compact()}>
                <DayTabs
                  days={days()}
                  selected={days()[index()]?.date ?? ''}
                  onSelect={setChosen}
                />
              </Show>

              <Show when={compact()} fallback={grid(days())}>
                <Pager
                  items={days()}
                  index={index()}
                  onIndex={(position) => setChosen(days()[position]?.date ?? null)}
                  enabled={compact()}
                >
                  {(day) => grid([day])}
                </Pager>
              </Show>
            </div>
          </Show>
        </Show>
      </main>

      <footer class="app__footer">
        <span class="compare__legend">
          <span class="compare__legend-mark compare__legend-mark--together" aria-hidden="true" />
          ensemble
        </span>
        <span class="compare__legend">
          <span class="compare__legend-mark compare__legend-mark--free" aria-hidden="true" />
          libres tous les deux
        </span>
      </footer>

      <Editor
        which={editing()}
        years={props.years}
        week={props.week}
        initial={editing() === null ? null : side(editing() as Which)}
        onChoose={choose}
        onClose={() => setEditing(null)}
      />
    </div>
  );
};

interface EditorProps {
  which: Which | null;
  years: YearSummary[];
  week: number | null;
  initial: Side | null;
  onChoose: (which: Which, side: Side) => void;
  onClose: () => void;
}

/** Le choix d'une classe : son année, puis son groupe. */
const Editor: Component<EditorProps> = (props) => {
  const [year, setYear] = createSignal<number | null>(null);

  // À l'ouverture, l'année part de celle de la classe qu'on remplace.
  createEffect(
    on(
      () => props.which,
      (which) => {
        if (which !== null) setYear(props.initial?.year ?? settings().year ?? props.years[0]?.year ?? null);
      },
    ),
  );

  /** Les groupes se lisent dans la semaine, l'API n'en publie pas la liste. */
  const [structure] = createResource(
    () => {
      const chosen = year();
      const week = props.week;
      return chosen !== null && week !== null ? ({ year: chosen, week } as const) : undefined;
    },
    ({ year: chosen, week }) => fetchWeek(chosen, week),
  );

  return (
    <Sheet open={props.which !== null} onClose={props.onClose} label="Choisir une classe">
      <header class="sheet__header">
        <h2 class="sheet__title">
          {props.which === 'right' ? 'Classe de droite' : 'Classe de gauche'}
        </h2>
        <button type="button" class="button" onClick={props.onClose} aria-label="Fermer">
          <X size={16} aria-hidden="true" />
        </button>
      </header>

      <section class="field">
        <h3 class="field__label">Année</h3>
        <div class="field__row">
          <For each={props.years}>
            {(item) => (
              <button
                type="button"
                class="chip"
                aria-pressed={year() === item.year}
                onClick={() => setYear(item.year)}
              >
                {`BUT ${item.year}`}
              </button>
            )}
          </For>
        </div>
      </section>

      <section class="field">
        <Show
          when={structure()}
          fallback={<p class="setup__note">{structure.error ? 'Semaine non publiée pour cette année.' : 'Lecture des groupes…'}</p>}
        >
          {(week) => (
            <GroupPicker
              bases={groupBases(week())}
              value={props.initial?.year === year() ? (props.initial?.group ?? null) : null}
              onSelect={(group) => {
                const chosen = year();
                const which = props.which;
                if (chosen === null || which === null) return;
                props.onChoose(which, { year: chosen, group });
                props.onClose();
              }}
            />
          )}
        </Show>
      </section>
    </Sheet>
  );
};
