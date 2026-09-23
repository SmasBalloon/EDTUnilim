import { For, Show, createMemo, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { gridBounds, packDay } from '../lib/layout';
import { clock, humanDuration, isRunning, nowMinutes, shortDate, toMinutes, todayIso } from '../lib/time';
import { cx, gridCorner, gridHead, hourMark, nowLine } from '../lib/ui';
import { CourseBlock } from './CourseBlock';

/** Un côté de la comparaison : son nom et ses cours de la semaine. */
export interface Side {
  label: string;
  courses: Course[];
}

interface Props {
  days: { name: string; date: string }[];
  left: Side;
  right: Side;
  onSelect: (course: Course) => void;
}

/** Hauteur d'une heure de grille, en pixels. */
const HOUR_HEIGHT = 57;
/** Marge haute et basse, pour que les étiquettes extrêmes tiennent. */
const EDGE_PAD = 5;
/** En deçà, un trou commun n'est pas un moment : c'est un interstice. */
const WORTH_NOTING = 30;

/**
 * Ce qui identifie un même cours d'un côté et de l'autre : le créneau et ce
 * qu'on y fait. Deux groupes réunis en amphi ont le même, deux TP séparés non.
 */
function signature(course: Course): string {
  const what = course.resource?.code ?? course.title ?? course.kind;
  return `${course.date} ${course.start} ${course.end} ${what}`;
}

function byDay(courses: Course[]): Map<string, Course[]> {
  const map = new Map<string, Course[]>();
  for (const course of courses) map.set(course.date, [...(map.get(course.date) ?? []), course]);
  return map;
}

/** Les intervalles occupés, fusionnés : `[[480, 600], [630, 720]]`. */
function merge(courses: Course[]): [number, number][] {
  const spans = courses
    .map((course) => [toMinutes(course.start), toMinutes(course.end)] as [number, number])
    .sort((a, b) => a[0] - b[0]);

  const merged: [number, number][] = [];
  for (const span of spans) {
    const last = merged[merged.length - 1];
    if (last && span[0] <= last[1]) last[1] = Math.max(last[1], span[1]);
    else merged.push([...span]);
  }
  return merged;
}

/**
 * Les moments où ni l'une ni l'autre des deux classes n'a cours, entre le
 * premier et le dernier cours de la journée. Avant et après, tout le monde est
 * libre : ça ne s'affiche pas.
 */
function sharedGaps(left: Course[], right: Course[]): [number, number][] {
  const busy = merge([...left, ...right]);
  const gaps: [number, number][] = [];
  for (let index = 1; index < busy.length; index += 1) {
    const from = busy[index - 1]?.[1] ?? 0;
    const to = busy[index]?.[0] ?? 0;
    if (to - from >= WORTH_NOTING) gaps.push([from, to]);
  }
  return gaps;
}

/**
 * Deux emplois du temps sur la même échelle horaire, colonne contre colonne.
 *
 * Ce que la page doit donner d'un coup d'œil : ce qu'on suit ensemble (les
 * créneaux communs sont cerclés et liés), et quand on est libres tous les deux
 * (les bandes qui traversent les deux colonnes).
 */
export const CompareGrid: Component<Props> = (props) => {
  const today = createMemo(() => todayIso(clock()));
  const minutes = createMemo(() => nowMinutes(clock()));

  const left = createMemo(() => byDay(props.left.courses));
  const right = createMemo(() => byDay(props.right.courses));

  /** Les créneaux que les deux classes partagent, tous jours confondus. */
  const together = createMemo(() => {
    const seen = new Set(props.left.courses.map(signature));
    return new Set(props.right.courses.filter((course) => seen.has(signature(course))).map(signature));
  });

  const bounds = createMemo(() => gridBounds([...props.left.courses, ...props.right.courses]));
  const height = createMemo(() => ((bounds().end - bounds().start) / 60) * HOUR_HEIGHT + EDGE_PAD * 2);

  const hours = createMemo(() => {
    const list: number[] = [];
    for (let m = Math.ceil(bounds().start / 60) * 60; m <= bounds().end; m += 60) list.push(m);
    return list;
  });

  /** Ordonnée, en pixels, d'un instant de la journée. */
  const offset = (m: number) => ((m - bounds().start) / 60) * HOUR_HEIGHT + EDGE_PAD;

  const sides = () => [
    { side: props.left, courses: left() },
    { side: props.right, courses: right() },
  ];

  return (
    // Une colonne par jour, coupée en deux : la classe de gauche, celle de
    // droite. Elles se resserrent pour que la semaine entière tienne à l'écran
    // sur un ordinateur ; un seul jour à l'écran, elles se partagent la largeur.
    <div
      class={cx(
        'grid w-full [--column-min:clamp(5.5rem,26vw,6.25rem)] [--gutter:clamp(2.25rem,7vw,3.25rem)]',
        'grid-cols-[var(--gutter)_repeat(var(--day-count),minmax(0,1fr))]',
        'min-w-[calc(var(--gutter)+var(--day-count)*2*var(--column-min))]',
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
            <div class="pt-2">
              <div class="text-2xs font-semibold uppercase tracking-[0.04em]">{day.name.toLowerCase()}</div>
              <div class="text-2xs text-muted">{shortDate(day.date)}</div>
              {/* Les noms des deux classes tiennent au-dessus de leur colonne : on
                  sait à qui appartient chaque cours sans remonter plus haut. */}
              <div class="mt-1 flex">
                <span class="min-w-0 flex-1 truncate py-0.5 px-1 text-2xs font-semibold text-muted">
                  {props.left.label}
                </span>
                <span class="min-w-0 flex-1 truncate py-0.5 px-1 text-2xs font-semibold text-muted">
                  {props.right.label}
                </span>
              </div>
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
          <div class="relative flex h-(--body-height) border-l border-line">
            {/* Les moments libres des deux côtés passent derrière les cours. */}
            <For each={sharedGaps(left().get(day.date) ?? [], right().get(day.date) ?? [])}>
              {(gap) => (
                <div
                  class="kind-sae pointer-events-none absolute inset-x-0 z-[1] grid place-items-center border-y border-dashed border-kind/45 bg-kind/12 text-2xs font-semibold text-kind-ink"
                  style={{ top: `${offset(gap[0])}px`, height: `${offset(gap[1]) - offset(gap[0])}px` }}
                >
                  <Show when={gap[1] - gap[0] >= 45}>
                    <span>{`libres ${humanDuration(gap[1] - gap[0])}`}</span>
                  </Show>
                </div>
              )}
            </For>

            <For each={sides()}>
              {({ courses }) => (
                // La séparation entre deux classes est plus discrète qu'entre deux jours.
                <div
                  class={cx(
                    'hour-lines relative min-w-0 flex-1 [&+&]:border-l [&+&]:border-dashed [&+&]:border-line/75',
                    day.date === today() && 'bg-accent/7',
                  )}
                >
                  <For each={packDay(courses.get(day.date) ?? [])}>
                    {(placed) => {
                      const slot =
                        ((toMinutes(placed.course.end) - toMinutes(placed.course.start)) / 60) *
                        HOUR_HEIGHT;
                      return (
                        <CourseBlock
                          course={placed.course}
                          live={isRunning(placed.course, today(), minutes())}
                          dense={slot < 62}
                          together={together().has(signature(placed.course))}
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

                  <Show
                    when={day.date === today() && minutes() >= bounds().start && minutes() <= bounds().end}
                  >
                    <div class={nowLine} style={{ top: `${offset(minutes())}px` }} />
                  </Show>
                </div>
              )}
            </For>
          </div>
        )}
      </For>
    </div>
  );
};
