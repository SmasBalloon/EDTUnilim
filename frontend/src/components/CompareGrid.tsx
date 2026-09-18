import { For, Show, createMemo, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { gridBounds, packDay } from '../lib/layout';
import { clock, humanDuration, isRunning, nowMinutes, shortDate, toMinutes, todayIso } from '../lib/time';
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
    <div
      class="compare-grid"
      style={{
        '--day-count': String(props.days.length),
        '--hour-height': `${HOUR_HEIGHT}px`,
        '--body-height': `${height()}px`,
        '--body-pad': `${EDGE_PAD}px`,
      }}
    >
      <div class="week-grid__corner" />
      <For each={props.days}>
        {(day) => (
          <div class="compare-grid__head" classList={{ 'week-grid__day--today': day.date === today() }}>
            <div class="week-grid__day-name">{day.name.toLowerCase()}</div>
            <div class="week-grid__day-date">{shortDate(day.date)}</div>
            <div class="compare-grid__names">
              <span class="compare-grid__name">{props.left.label}</span>
              <span class="compare-grid__name">{props.right.label}</span>
            </div>
          </div>
        )}
      </For>

      <div class="week-grid__gutter">
        <div class="week-grid__hours">
          <For each={hours()}>
            {(hour) => (
              <span class="week-grid__hour" style={{ top: `${offset(hour)}px` }}>
                {`${Math.floor(hour / 60)}h`}
              </span>
            )}
          </For>
        </div>
      </div>

      <For each={props.days}>
        {(day) => (
          <div class="compare-grid__body">
            {/* Les moments libres des deux côtés passent derrière les cours. */}
            <For each={sharedGaps(left().get(day.date) ?? [], right().get(day.date) ?? [])}>
              {(gap) => (
                <div
                  class="compare-grid__free"
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
                <div
                  class="compare-grid__side"
                  classList={{ 'compare-grid__side--today': day.date === today() }}
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
                    <div class="week-grid__now" style={{ top: `${offset(minutes())}px` }} />
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
