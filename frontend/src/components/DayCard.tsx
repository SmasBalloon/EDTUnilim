import { For, Show, createMemo, type Component } from 'solid-js';
import { Hourglass } from 'lucide-solid';

import type { Course } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { clock, humanDuration, isRunning, nowMinutes, relativeDayLabel, toMinutes, todayIso } from '../lib/time';

interface Props {
  date: string;
  courses: Course[];
  onSelect: (course: Course) => void;
}

/** Une journée se lit comme une suite d'heures, de cours et de trous. */
type Row =
  | { kind: 'time'; at: string }
  | { kind: 'gap'; minutes: number }
  | { kind: 'course'; course: Course };

function buildRows(courses: Course[]): Row[] {
  const sorted = [...courses].sort((a, b) => toMinutes(a.start) - toMinutes(b.start));
  const rows: Row[] = [];

  sorted.forEach((course, index) => {
    const previous = sorted[index - 1];
    if (previous) {
      const gap = toMinutes(course.start) - toMinutes(previous.end);
      // Un trou se lit comme une parenthèse : l'heure où l'on sort, le temps
      // libre, puis l'heure où l'on reprend.
      if (gap > 0) {
        rows.push({ kind: 'time', at: previous.end });
        rows.push({ kind: 'gap', minutes: gap });
      }
    }
    rows.push({ kind: 'time', at: course.start });
    rows.push({ kind: 'course', course });
  });

  const last = sorted[sorted.length - 1];
  if (last) rows.push({ kind: 'time', at: last.end });
  return rows;
}

/** Une journée en mode liste, dans sa carte. */
export const DayCard: Component<Props> = (props) => {
  const rows = createMemo(() => buildRows(props.courses));
  const total = createMemo(() => props.courses.reduce((sum, c) => sum + c.duration_minutes, 0));
  const isToday = () => props.date === todayIso(clock());

  return (
    <>
      <div class="card day-card" classList={{ 'day-card--today': isToday() }}>
        <div class="day-card__label">
          <span class="badge badge--accent">{relativeDayLabel(props.date, clock())}</span>
        </div>

        <Show when={props.courses.length > 0} fallback={<p class="day-card__empty">Rien ce jour-là.</p>}>
          <For each={rows()}>
            {(row) => (
              <Show when={row.kind === 'course' ? row.course : null} fallback={<Marker row={row} />}>
                {(course) => <CourseRow course={course()} onSelect={() => props.onSelect(course())} />}
              </Show>
            )}
          </For>
        </Show>
      </div>

      <Show when={props.courses.length > 0}>
        <p class="day-card__total">{`${props.courses.length} cours · ${humanDuration(total())}`}</p>
      </Show>
    </>
  );
};

/** Séparateur horaire, ou annonce d'un trou dans la journée. */
const Marker: Component<{ row: Row }> = (props) => (
  <Show
    when={props.row.kind === 'time' ? props.row.at : null}
    fallback={
      <p class="gap-note">
        <Hourglass size={14} aria-hidden="true" />
        {props.row.kind === 'gap' ? `${humanDuration(props.row.minutes)} de vide` : ''}
      </p>
    }
  >
    {(at) => <p class="time-rule">{at()}</p>}
  </Show>
);

/** Une ligne de cours ; le détail s'ouvre en modal. */
const CourseRow: Component<{ course: Course; onSelect: () => void }> = (props) => {
  const live = () => isRunning(props.course, todayIso(clock()), nowMinutes(clock()));

  /** « Anglais avec JP », ou seulement l'un des deux quand l'autre manque. */
  const subject = () => {
    const name = props.course.resource?.name ?? props.course.title;
    const teacher = props.course.teacher ? `avec ${props.course.teacher}` : '';
    return [name, teacher].filter(Boolean).join(' ');
  };

  return (
    <button
      type="button"
      class={`course-row kind-${props.course.kind}`}
      onClick={props.onSelect}
    >
      <span class="course-row__head">
        <span class="course-row__code">
          {props.course.resource?.code ?? props.course.title ?? 'Cours'}
        </span>
        <Show when={live()}>
          <span class="badge badge--accent">en cours</span>
        </Show>
        <span class="course-row__badges">
          <span class="badge badge--kind">{KIND_LABEL[props.course.kind]}</span>
          <Show when={props.course.room}>
            {(room) => <span class="badge badge--accent">{room()}</span>}
          </Show>
        </span>
      </span>

      <Show when={subject()}>{(text) => <span class="course-row__subject">{text()}</span>}</Show>
    </button>
  );
};
