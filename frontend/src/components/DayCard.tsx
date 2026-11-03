import { For, Match, Show, Switch, createMemo, type Component } from 'solid-js';

import type { Course } from '../lib/api';
import { KIND_LABEL } from '../lib/layout';
import { homework, onCourse } from '../lib/homework';
import { clock, dayName, dayStamp, humanDuration, isRunning, nowMinutes, shortDate, toMinutes, todayIso } from '../lib/time';
import { cx, kindCard, kindTag } from '../lib/ui';
import type { Day } from './WeekView';

interface Props {
  day: Day;
  courses: Course[];
  onSelect: (course: Course) => void;
  /** Remplace l'enseignant quand il n'apprend rien (page d'un enseignant). */
  describe?: (course: Course) => string;
  /** Dit d'un cours qu'il a bougé depuis la dernière visite. */
  changed?: (course: Course) => string | undefined;
  /** Un seul jour à l'écran, au format téléphone (`mobile-semaine.html`). */
  mobile?: boolean;
}

/** Une journée se lit comme une suite de cours et de trous. */
type Row =
  | { kind: 'gap'; minutes: number }
  | { kind: 'part'; label: string }
  | { kind: 'course'; course: Course };

/** Midi : la frontière entre le matin et l'après-midi. */
const NOON = 12 * 60;

function buildRows(courses: Course[], split: boolean): Row[] {
  const sorted = [...courses].sort((a, b) => toMinutes(a.start) - toMinutes(b.start));
  const rows: Row[] = [];
  // Le matin et l'après-midi ne se nomment que s'il y a les deux.
  const both =
    split &&
    sorted.some((course) => toMinutes(course.start) < NOON) &&
    sorted.some((course) => toMinutes(course.start) >= NOON);

  sorted.forEach((course, index) => {
    const previous = sorted[index - 1];
    if (both && (index === 0 || (previous && toMinutes(previous.start) < NOON && toMinutes(course.start) >= NOON))) {
      if (previous) {
        const gap = toMinutes(course.start) - toMinutes(previous.end);
        if (gap > 0) rows.push({ kind: 'gap', minutes: gap });
      }
      rows.push({ kind: 'part', label: toMinutes(course.start) < NOON ? 'Matin' : 'Après-midi' });
    } else if (previous) {
      const gap = toMinutes(course.start) - toMinutes(previous.end);
      if (gap > 0) rows.push({ kind: 'gap', minutes: gap });
    }
    rows.push({ kind: 'course', course });
  });
  return rows;
}

/** Une journée en mode liste. */
export const DayCard: Component<Props> = (props) => {
  const rows = createMemo(() => buildRows(props.courses, props.mobile === true));
  const total = createMemo(() => props.courses.reduce((sum, c) => sum + c.duration_minutes, 0));
  const isToday = () => props.day.date === todayIso(clock());
  const summary = () =>
    props.courses.length > 0 ? `${props.courses.length} cours · ${humanDuration(total())}` : 'Aucun cours';

  const content = () => (
    <For each={rows()}>
      {(row) => (
        <Switch>
          <Match when={row.kind === 'course' && row.course}>
            {(course) => (
              <CourseRow
                course={course()}
                mobile={props.mobile === true}
                describe={props.describe}
                changed={props.changed?.(course())}
                onSelect={() => props.onSelect(course())}
              />
            )}
          </Match>
          <Match when={row.kind === 'gap' && row}>
            {(gap) => (
              <div class="week-gap">
                <span>{`${humanDuration(gap().minutes)} de vide`}</span>
                <i />
              </div>
            )}
          </Match>
          <Match when={row.kind === 'part' && row}>{(part) => <div class="week-part">{part().label}</div>}</Match>
        </Switch>
      )}
    </For>
  );

  return (
    <Show
      when={props.mobile}
      fallback={
        <section class={cx('card week-day', isToday() && 'today')}>
          <div class="week-dt">
            <span>{dayName(props.day.name)}</span>
            <h2>{shortDate(props.day.date)}</h2>
            <small>{summary()}</small>
          </div>
          <Show when={props.courses.length > 0} fallback={<p class="week-none">Rien ce jour-là.</p>}>
            {content()}
          </Show>
        </section>
      }
    >
      <section class="week-mobile">
        <div class="week-mobile-head">
          <b class={cx(isToday() && 'today')}>{`${dayName(props.day.name)} ${dayStamp(props.day.date)}`}</b>
          <span>{summary()}</span>
        </div>
        <Show
          when={props.courses.length > 0}
          fallback={
            <div class="empty">
              <p>Rien ce jour-là.</p>
            </div>
          }
        >
          {content()}
        </Show>
      </section>
    </Show>
  );
};

/** Une carte de cours de la liste ; le détail s'ouvre en modal. */
const CourseRow: Component<{
  course: Course;
  mobile: boolean;
  describe?: (course: Course) => string;
  changed?: string;
  onSelect: () => void;
}> = (props) => {
  const live = () => isRunning(props.course, todayIso(clock()), nowMinutes(clock()));
  /** Un devoir rattaché à ce créneau précis. */
  const noted = () => homework().some((item) => onCourse(item, props.course));
  const who = () => (props.describe ? props.describe(props.course) : props.course.teacher);
  const code = () => props.course.resource?.code ?? props.course.title ?? 'Cours';

  const badges = () => (
    <>
      <Show when={live()}>
        <span class="tag t-room">en cours</span>
      </Show>
      <Show when={props.changed}>
        {(change) => <span class="tag t-room">{change() === 'added' ? 'ajouté' : 'modifié'}</span>}
      </Show>
      <span class={kindTag(props.course.kind)}>{KIND_LABEL[props.course.kind]}</span>
    </>
  );

  const hw = () => (
    <Show when={noted()}>
      <span class="mark-hw" title="Un devoir est rattaché à ce cours">✎</span>
    </Show>
  );

  return (
    <Show
      when={props.mobile}
      fallback={
        <button
          type="button"
          class={cx('week-crs', kindCard(props.course.kind), live() && 'is-live', props.changed && 'is-changed')}
          onClick={props.onSelect}
        >
          <div class="tm">
            {`${props.course.start} – ${props.course.end}`}
            <span class="flex flex-wrap justify-end gap-1">{badges()}</span>
          </div>
          <b>{code()}</b>
          <Show when={props.course.resource?.name}>{(name) => <small>{name()}</small>}</Show>
          <Show when={[props.course.room, who()].filter(Boolean).join(' · ')}>
            {(meta) => (
              <small class="m">
                {hw()}
                {meta()}
              </small>
            )}
          </Show>
        </button>
      }
    >
      <button
        type="button"
        class={cx('week-mc', kindCard(props.course.kind), live() && 'is-live', props.changed && 'is-changed')}
        onClick={props.onSelect}
      >
        <div class="t">
          {props.course.start}
          <small>{`→ ${props.course.end}`}</small>
        </div>
        <div class="w">
          <b>{code()}</b>
          <small>
            {hw()}
            {[
              props.course.resource?.name,
              props.course.room,
              // Sans intitulé, l'enseignant (ou le groupe, pour une SAÉ) dit de quoi il s'agit.
              props.describe ? props.describe(props.course) : !props.course.resource?.name && props.course.teacher,
            ]
              .filter(Boolean)
              .join(' · ')}
          </small>
        </div>
        <div class="tags">{badges()}</div>
      </button>
    </Show>
  );
};
