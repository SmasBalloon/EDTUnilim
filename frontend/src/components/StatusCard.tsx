import { Show, createMemo, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { clock, isRunning, nowMinutes, relativeDayLabel, toMinutes, todayIso } from '../lib/time';
import { card, cx } from '../lib/ui';

interface State {
  /** Le cours en train de se dérouler, s'il y en a un. */
  running: Course | null;
  /** Le prochain cours à venir, où qu'il soit dans la semaine. */
  next: Course | null;
}

const Accent: Component<{ children: string }> = (props) => (
  <span class="font-medium text-accent">{props.children}</span>
);

/** Quand a lieu le prochain cours : « à 14:00 », « demain à 08:00 ». */
function when(course: Course, today: string): string {
  if (course.date === today) return `à ${course.start}`;
  const [position] = relativeDayLabel(course.date, clock()).split(' : ');
  return `${(position ?? '').toLowerCase()} à ${course.start}`;
}

/** « CM R1.11 en AC avec Brunel M. » */
function summarise(course: Course): string {
  return [
    KIND_LABEL[course.kind],
    course.resource?.code,
    course.room && `en ${course.room}`,
    course.teacher && `avec ${course.teacher}`,
  ]
    .filter(Boolean)
    .join(' ');
}

/**
 * L'état de la journée, tel qu'on le regarde en arrivant : ce qui se passe
 * maintenant, puis ce qui vient ensuite. Toujours calculé sur la semaine
 * courante, même quand l'écran affiche une autre semaine.
 */
export const StatusCard: Component<{ courses: Course[] }> = (props) => {
  const state = createMemo<State>(() => {
    const today = todayIso(clock());
    const minutes = nowMinutes(clock());

    return {
      running: props.courses.find((course) => isRunning(course, today, minutes)) ?? null,
      next:
        props.courses
          .filter(
            (course) => course.date > today || (course.date === today && toMinutes(course.start) > minutes),
          )
          .sort((a, b) => a.date.localeCompare(b.date) || toMinutes(a.start) - toMinutes(b.start))[0] ?? null,
    };
  });

  return (
    <div class={`${card} px-3.5 py-2.5 text-sm`}>
      <p class="flex items-center gap-2 font-medium">
        <span
          class={cx('size-2 shrink-0 rounded-full', state().running ? 'bg-accent' : 'bg-muted')}
          aria-hidden="true"
        />
        <Show
          when={state().running}
          fallback={
            <Show
              when={state().next?.date === todayIso(clock())}
              fallback={<span>Journée terminée</span>}
            >
              <span>Pause</span>
            </Show>
          }
        >
          {(course) => (
            <span>
              En cours jusqu'à <Accent>{course().end}</Accent>
            </span>
          )}
        </Show>
      </p>

      <Show
        when={state().next}
        fallback={<p class="mt-1 text-[0.8125rem] text-muted">Plus rien cette semaine.</p>}
      >
        {(course) => (
          <p class="mt-1 text-[0.8125rem] text-muted">
            Prochain <Accent>{when(course(), todayIso(clock()))}</Accent> · {summarise(course())}
          </p>
        )}
      </Show>
    </div>
  );
};
