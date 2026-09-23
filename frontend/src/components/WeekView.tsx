import { For, Show, createMemo, createSignal, type Component, type ParentComponent } from 'solid-js';

import type { Course, Week } from '../lib/api';
import { gridBounds, packDay } from '../lib/layout';
import { useCompact } from '../lib/media';
import type { ViewMode } from '../lib/settings';
import { clock, dayIndex, dayLabel, isRunning, nowMinutes, shortDate, toMinutes, todayIso } from '../lib/time';
import { cx, gridCorner, gridHead, hourMark, nowLine } from '../lib/ui';
import { CourseBlock } from './CourseBlock';
import { DayCard } from './DayCard';
import { DayTabs } from './DayTabs';
import { Pager } from './Pager';

interface Props {
  week: Week;
  /** Colonnes à afficher, jours vides pour le groupe choisi compris. */
  days: { name: string; date: string }[];
  view: ViewMode;
  /** Ouvre le détail d'un cours. */
  onSelect: (course: Course) => void;
  /** Remplace la ligne de détail d'un cours : voir `CourseBlock`. */
  describe?: (course: Course) => string;
  /** Dit d'un cours qu'il a bougé depuis la dernière visite. */
  changed?: (course: Course) => string | undefined;
}

/** Hauteur d'une heure de grille, en pixels. */
const HOUR_HEIGHT = 57;
/** Marge haute et basse, pour que les étiquettes extrêmes tiennent. */
const EDGE_PAD = 5;

function coursesByDay(week: Week): Map<string, Course[]> {
  const byDay = new Map<string, Course[]>();
  for (const course of week.courses) {
    byDay.set(course.date, [...(byDay.get(course.date) ?? []), course]);
  }
  return byDay;
}

export const WeekView: Component<Props> = (props) => {
  const byDay = createMemo(() => coursesByDay(props.week));
  const compact = useCompact();

  // Sur écran étroit, la semaine se feuillette : un jour par page. On retient
  // le jour ouvert, pas son rang : la page est alors juste dès le premier
  // rendu, sans correction après coup — donc sans saut ni glissement parasite.
  const [chosen, setChosen] = createSignal<string | null>(null);

  const index = () => dayIndex(props.days, chosen());
  const selected = () => props.days[index()]?.date ?? null;
  const open = (position: number) => setChosen(props.days[position]?.date ?? null);

  const day = (only: { name: string; date: string }[]) => (
    <Show
      when={props.view === 'grid'}
      fallback={<DayStrip {...props} days={only} byDay={byDay()} />}
    >
      <TimeGrid {...props} days={only} byDay={byDay()} />
    </Show>
  );

  return (
    <WeekFrame single={compact()}>
      <Show when={compact()}>
        <DayTabs
          days={props.days}
          selected={selected() ?? ''}
          onSelect={setChosen}
        />
      </Show>

      <Show when={compact()} fallback={day(props.days)}>
        <Pager
          items={props.days}
          index={index()}
          onIndex={open}
          enabled={compact()}
        >
          {(only) => day([only])}
        </Pager>
      </Show>
    </WeekFrame>
  );
};

/**
 * Le cadre d'une semaine. Quand un seul jour tient à l'écran (`single`), les
 * grilles et les colonnes qu'il contient prennent toute la largeur, par les
 * variantes `group-data-single/week:`.
 *
 * En jour unique, le geste horizontal est celui de l'application : on interdit
 * le panoramique latéral au navigateur, sinon il le prend pour un retour en
 * arrière et quitte la page avant même que le code ne voie le toucher.
 */
export const WeekFrame: ParentComponent<{ single: boolean }> = (props) => (
  <div class={cx('group/week', props.single && 'touch-pan-y')} data-single={props.single ? '' : undefined}>
    {props.children}
  </div>
);

type ViewProps = Props & { byDay: Map<string, Course[]> };

/** Les jours à la suite, chacun dans sa carte. */
const DayStrip: Component<ViewProps> = (props) => (
  <div class="flex items-start gap-gap px-edge pb-6">
    <For each={props.days}>
      {(day) => (
        <div class="min-w-60 flex-[1_0_15rem] snap-start group-data-single/week:min-w-0 group-data-single/week:basis-full">
          <DayCard
            date={day.date}
            courses={props.byDay.get(day.date) ?? []}
            describe={props.describe}
            changed={props.changed}
            onSelect={props.onSelect}
          />
        </div>
      )}
    </For>
  </div>
);

/** La semaine sur une échelle horaire commune. */
const TimeGrid: Component<ViewProps> = (props) => {
  const today = createMemo(() => todayIso(clock()));
  const minutes = createMemo(() => nowMinutes(clock()));

  const bounds = createMemo(() => gridBounds(props.week.courses));
  const height = createMemo(() => ((bounds().end - bounds().start) / 60) * HOUR_HEIGHT + EDGE_PAD * 2);

  const hours = createMemo(() => {
    const list: number[] = [];
    for (let m = Math.ceil(bounds().start / 60) * 60; m <= bounds().end; m += 60) list.push(m);
    return list;
  });

  /** Ordonnée, en pixels, d'un instant de la journée. */
  const offset = (m: number) => ((m - bounds().start) / 60) * HOUR_HEIGHT + EDGE_PAD;

  return (
    // Une colonne par jour, plus la colonne des heures. Les colonnes se
    // partagent la largeur sans jamais passer sous `--column-min` : la semaine
    // reste entière, quitte à défiler horizontalement sur téléphone.
    <div
      class={cx(
        'grid w-full [--column-min:clamp(9rem,40vw,11rem)] [--gutter:clamp(2.25rem,7vw,3.25rem)]',
        'grid-cols-[var(--gutter)_repeat(var(--day-count),minmax(0,1fr))]',
        'min-w-[calc(var(--gutter)+var(--day-count)*var(--column-min))]',
        'group-data-single/week:min-w-full',
      )}
      style={{
        '--day-count': String(props.days.length),
        '--hour-height': `${HOUR_HEIGHT}px`,
        '--body-height': `${height()}px`,
        '--body-pad': `${EDGE_PAD}px`,
      }}
    >
      <div class={gridCorner} />
      <For each={props.days}>
        {(day) => (
          <div
            class={cx(
              gridHead,
              'text-center',
              day.date === today() && 'text-accent shadow-[inset_0_-2px_0_var(--color-accent)]',
            )}
          >
            <div class="p-2">
              <div class="text-2xs font-semibold uppercase tracking-[0.04em]">{dayLabel(day.date)}</div>
              <div class="text-2xs text-muted">{shortDate(day.date)}</div>
            </div>
          </div>
        )}
      </For>

      <div class="sticky left-0 z-10 bg-bg">
        <div class="relative h-(--body-height)">
          <For each={hours()}>
            {(hour) => (
              <span
                class={hourMark}
                style={{ top: `${offset(hour)}px` }}
              >
                {`${Math.floor(hour / 60)}h`}
              </span>
            )}
          </For>
        </div>
      </div>

      <For each={props.days}>
        {(day) => (
          <div
            class={cx(
              'hour-lines relative h-(--body-height) border-l border-line',
              day.date === today() && 'bg-accent/7',
            )}
          >
            <For each={packDay(props.byDay.get(day.date) ?? [])}>
              {(placed) => {
                const slot =
                  ((toMinutes(placed.course.end) - toMinutes(placed.course.start)) / 60) * HOUR_HEIGHT;
                return (
                  <CourseBlock
                    course={placed.course}
                    live={isRunning(placed.course, today(), minutes())}
                    dense={slot < 62}
                    describe={props.describe}
                    changed={props.changed?.(placed.course)}
                    onSelect={() => props.onSelect(placed.course)}
                    style={{
                      top: `${offset(toMinutes(placed.course.start)) + 1}px`,
                      height: `${slot - 3}px`,
                      left: `calc(${(placed.lane / placed.lanes) * 100}% + 2px)`,
                      width: `calc(${100 / placed.lanes}% - 4px)`,
                    }}
                  />
                );
              }}
            </For>

            <Show when={day.date === today() && minutes() >= bounds().start && minutes() <= bounds().end}>
              <div class={nowLine} style={{ top: `${offset(minutes())}px` }} />
            </Show>
          </div>
        )}
      </For>
    </div>
  );
};
