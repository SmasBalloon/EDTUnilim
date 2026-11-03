import { For, Show, createMemo, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { blockStyle, gridScale, isShort, packDay } from '../lib/layout';
import { clock, humanDuration, isRunning, nowMinutes, toMinutes, todayIso } from '../lib/time';
import { CourseBlock } from './CourseBlock';
import { Hours, NowLine } from './WeekView';

/** Un côté de la comparaison : son nom et ses cours de la semaine. */
export interface Side {
  label: string;
  courses: Course[];
}

interface Props {
  /** Le jour comparé : la maquette en montre un à la fois. */
  date: string;
  left: Side;
  right: Side;
  onSelect: (course: Course) => void;
}

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
 * Deux emplois du temps d'un même jour, colonne contre colonne, sur la même
 * échelle horaire. Ce que la page doit donner d'un coup d'œil : ce qu'on suit
 * ensemble (contour rouge, `cmp-both`) et quand on est libres tous les deux
 * (bande verte qui traverse les deux colonnes, `cmp-free`).
 */
export const CompareGrid: Component<Props> = (props) => {
  const today = createMemo(() => todayIso(clock()));
  const minutes = createMemo(() => nowMinutes(clock()));

  const ofDay = (courses: Course[]) => courses.filter((course) => course.date === props.date);
  const left = createMemo(() => ofDay(props.left.courses));
  const right = createMemo(() => ofDay(props.right.courses));

  /** Les créneaux que les deux classes partagent ce jour-là. */
  const together = createMemo(() => {
    const seen = new Set(left().map(signature));
    return new Set(right().filter((course) => seen.has(signature(course))).map(signature));
  });

  /** L'échelle couvre la semaine entière des deux côtés : elle ne saute pas d'un jour à l'autre. */
  const scale = createMemo(() => gridScale([...props.left.courses, ...props.right.courses]));
  const gaps = createMemo(() => sharedGaps(left(), right()));

  const sides = () => [
    { side: props.left, courses: left() },
    { side: props.right, courses: right() },
  ];

  return (
    <div class="card week-grid">
      <div class="grid grid-cols-[56px_minmax(0,1fr)_minmax(0,1fr)]">
        <div />
        <For each={sides()}>
          {({ side }) => (
            <div class="dh">
              <b class="truncate">{side.label}</b>
            </div>
          )}
        </For>

        <Hours hours={scale().hours} />

        <For each={sides()}>
          {({ courses }) => (
            <div class="sched" style={{ height: `${scale().height}px` }}>
              {/* Les moments libres des deux côtés passent derrière les cours. */}
              <For each={gaps()}>
                {(gap) => (
                  <div
                    class="cmp-free"
                    style={{
                      top: `${scale().offset(gap[0])}px`,
                      height: `${scale().offset(gap[1]) - scale().offset(gap[0])}px`,
                    }}
                  >
                    <Show when={gap[1] - gap[0] >= 45}>
                      <span>{`libres ${humanDuration(gap[1] - gap[0])}`}</span>
                    </Show>
                  </div>
                )}
              </For>

              <For each={packDay(courses)}>
                {(placed) => (
                  <CourseBlock
                    course={placed.course}
                    live={isRunning(placed.course, today(), minutes())}
                    dense={isShort(placed.course)}
                    together={together().has(signature(placed.course))}
                    onSelect={() => props.onSelect(placed.course)}
                    style={blockStyle(placed, scale())}
                  />
                )}
              </For>

              <Show when={props.date === today() && minutes() >= scale().start && minutes() <= scale().end}>
                <NowLine top={scale().offset(minutes())} />
              </Show>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};
