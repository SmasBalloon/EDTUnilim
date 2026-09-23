import { For, Show, createMemo, type Component } from 'solid-js';
import { Hourglass } from 'lucide-solid';

import type { Course } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { homework, onCourse } from '../lib/homework';
import { clock, humanDuration, isRunning, nowMinutes, relativeDayLabel, toMinutes, todayIso } from '../lib/time';
import { badge, card, cx, pressLarge } from '../lib/ui';

interface Props {
  date: string;
  courses: Course[];
  onSelect: (course: Course) => void;
  /** Remplace « intitulé avec enseignant » quand celui-ci n'apprend rien. */
  describe?: (course: Course) => string;
  /** Dit d'un cours qu'il a bougé depuis la dernière visite. */
  changed?: (course: Course) => string | undefined;
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
      <div class={cx(card, 'relative mt-3 py-5', isToday() && 'border-accent!')}>
        {/* L'étiquette du jour est posée à cheval sur le bord supérieur. */}
        <div class="absolute inset-x-0 -top-3.5 flex justify-center">
          <span class={badge.accent}>{relativeDayLabel(props.date, clock())}</span>
        </div>

        <Show
          when={props.courses.length > 0}
          fallback={<p class="px-4 py-2 text-center text-[0.8125rem] text-muted">Rien ce jour-là.</p>}
        >
          <For each={rows()}>
            {(row) => (
              <Show when={row.kind === 'course' ? row.course : null} fallback={<Marker row={row} />}>
                {(course) => (
                  <CourseRow
                    course={course()}
                    describe={props.describe}
                    changed={props.changed?.(course())}
                    onSelect={() => props.onSelect(course())}
                  />
                )}
              </Show>
            )}
          </For>
        </Show>
      </div>

      <Show when={props.courses.length > 0}>
        <p class="pt-2 text-center text-2xs text-muted">{`${props.courses.length} cours · ${humanDuration(total())}`}</p>
      </Show>
    </>
  );
};

/** Séparateur horaire, ou annonce d'un trou dans la journée. */
const Marker: Component<{ row: Row }> = (props) => (
  <Show
    when={props.row.kind === 'time' ? props.row.at : null}
    fallback={
      <p class="flex items-center justify-center gap-2 p-3 text-[0.8125rem] text-muted">
        <Hourglass size={14} aria-hidden="true" />
        {props.row.kind === 'gap' ? `${humanDuration(props.row.minutes)} de vide` : ''}
      </p>
    }
  >
    {/* Heure de début, prolongée par un filet jusqu'au bord de la carte. */}
    {(at) => (
      <p class="flex items-center gap-3 px-3.5 py-2 text-[0.8125rem] text-muted tabular-nums after:h-px after:flex-1 after:bg-line">
        {at()}
      </p>
    )}
  </Show>
);

/** Une ligne de cours ; le détail s'ouvre en modal. */
const CourseRow: Component<{
  course: Course;
  describe?: (course: Course) => string;
  changed?: string;
  onSelect: () => void;
}> = (props) => {
  const live = () => isRunning(props.course, todayIso(clock()), nowMinutes(clock()));
  /** Un devoir rattaché à ce créneau précis. */
  const noted = () => homework().some((item) => onCourse(item, props.course));

  /** « Anglais avec JP », ou seulement l'un des deux quand l'autre manque. */
  const subject = () => {
    if (props.describe) return props.describe(props.course);
    const name = props.course.resource?.name ?? props.course.title;
    const teacher = props.course.teacher ? `avec ${props.course.teacher}` : '';
    return [name, teacher].filter(Boolean).join(' ');
  };

  return (
    <button
      type="button"
      class={cx(
        pressLarge,
        `kind-${props.course.kind}`,
        'block w-full cursor-pointer border-s-3 border-transparent px-3.5 py-[0.6rem] text-start',
        'hover:border-kind/75 hover:bg-kind/9',
      )}
      onClick={props.onSelect}
    >
      <span class="flex items-center gap-2">
        <span class="text-base font-medium">
          {props.course.resource?.code ?? props.course.title ?? 'Cours'}
        </span>
        <Show when={live()}>
          <span class={badge.accent}>en cours</span>
        </Show>
        <Show when={props.changed}>
          {(change) => (
            <span class={badge.change}>
              {change() === 'added' ? 'ajouté' : 'modifié'}
            </span>
          )}
        </Show>
        <span class="ml-auto flex items-center gap-1.5">
          <Show when={noted()}>
            <span class="mr-1 font-bold text-accent" title="Un devoir est rattaché à ce cours">✎</span>
          </Show>
          <span class={badge.kind}>{KIND_LABEL[props.course.kind]}</span>
          <Show when={props.course.room}>
            {(room) => <span class={badge.accent}>{room()}</span>}
          </Show>
        </span>
      </span>

      <Show when={subject()}>{(text) => <span class="mt-1 block text-sm text-muted">{text()}</span>}</Show>
    </button>
  );
};
