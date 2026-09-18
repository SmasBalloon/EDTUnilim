import {
  For,
  Show,
  createEffect,
  createMemo,
  createSignal,
  on,
  onCleanup,
  onMount,
  type Component,
} from 'solid-js';

import type { Course, Week } from '../lib/api';
import { gridBounds, packDay } from '../lib/layout';
import { useCompact } from '../lib/media';
import { Spring, Velocity, project, rubberband } from '../lib/motion';
import type { ViewMode } from '../lib/settings';
import { clock, dayLabel, isRunning, nowMinutes, shortDate, toMinutes, todayIso } from '../lib/time';
import { CourseBlock } from './CourseBlock';
import { DayCard } from './DayCard';
import { DayTabs } from './DayTabs';

interface Props {
  week: Week;
  /** Colonnes à afficher, jours vides pour le groupe choisi compris. */
  days: { name: string; date: string }[];
  view: ViewMode;
  /** Ouvre le détail d'un cours. */
  onSelect: (course: Course) => void;
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

/** Avant ce déplacement, le doigt n'a encore rien décidé. */
const THRESHOLD = 10;

export const WeekView: Component<Props> = (props) => {
  const byDay = createMemo(() => coursesByDay(props.week));
  const compact = useCompact();

  // Sur écran étroit, la semaine se feuillette : un jour par page.
  const [index, setIndex] = createSignal(0);
  const selected = () => props.days[index()]?.date ?? null;

  let frame: HTMLDivElement | undefined;
  let track: HTMLDivElement | undefined;
  /** Largeur d'une page, relevée sur le cadre. */
  let width = 0;

  const slide = new Spring(
    0,
    (x) => {
      if (track) track.style.transform = `translate3d(${x}px, 0, 0)`;
    },
    0.05,
  );
  onCleanup(() => slide.stop());

  /** Ouvre un jour ; la vitesse du doigt se prolonge dans le mouvement. */
  const goTo = (next: number, velocity?: number) => {
    const clamped = Math.max(0, Math.min(props.days.length - 1, next));
    setIndex(clamped);
    if (!compact()) return;
    // Le rebond n'a de sens que derrière un geste qui portait de l'élan.
    slide.to(-clamped * width, {
      damping: velocity === undefined ? 1 : 0.85,
      response: velocity === undefined ? 0.4 : 0.35,
      velocity,
    });
  };

  const measure = () => {
    width = frame?.clientWidth ?? 0;
    slide.set(-index() * width);
  };

  onMount(() => {
    const onResize = () => measure();
    window.addEventListener('resize', onResize);
    onCleanup(() => window.removeEventListener('resize', onResize));
  });

  // À chaque semaine, on ouvre sur aujourd'hui s'il en fait partie.
  createEffect(
    on(
      () => props.days,
      (list) => {
        const today = todayIso(clock());
        const found = list.findIndex((day) => day.date === today);
        setIndex(found === -1 ? 0 : found);
        slide.set(-index() * width);
      },
    ),
  );

  // Le cadre n'existe qu'en mode feuilleté : on le mesure dès qu'il paraît,
  // et de nouveau quand l'écran change de largeur.
  createEffect(on(compact, () => measure()));

  // --- Geste : la page suit le doigt, au pixel près ------------------------

  const travel = new Velocity();
  let origin: { x: number; y: number; at: number } | null = null;
  let dragging = false;

  const last = () => props.days.length - 1;

  const onPointerDown = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!compact() || event.button !== 0) return;
    origin = { x: event.clientX, y: event.clientY, at: slide.value };
    dragging = false;
    travel.clear();
  };

  const onPointerMove = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin || width === 0) return;
    const dx = event.clientX - origin.x;
    const dy = event.clientY - origin.y;

    if (!dragging) {
      // On laisse le doigt dire s'il feuillette ou s'il fait défiler.
      if (Math.abs(dx) < THRESHOLD && Math.abs(dy) < THRESHOLD) return;
      if (Math.abs(dx) <= Math.abs(dy)) {
        origin = null;
        return;
      }
      dragging = true;
      slide.stop();
      event.currentTarget.setPointerCapture(event.pointerId);
    }

    let x = origin.at + dx;
    // Aux deux bouts de la semaine, le bord résiste au lieu de bloquer.
    const min = -last() * width;
    if (x > 0) x = rubberband(x, width);
    else if (x < min) x = min - rubberband(min - x, width);

    travel.add(x);
    slide.set(x);
  };

  const release = (event: PointerEvent & { currentTarget: HTMLDivElement }) => {
    if (!origin) return;
    const wasDragging = dragging;
    origin = null;
    dragging = false;
    if (!wasDragging || width === 0) return;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }

    const speed = travel.get();
    // On feuillette vers le jour que le geste visait, sans jamais sauter plus
    // d'une page : un élan ne doit pas faire perdre la semaine de vue.
    const landing = -(slide.value + project(speed)) / width;
    const aimed = Math.round(landing);
    goTo(Math.max(index() - 1, Math.min(index() + 1, aimed)), speed);
  };

  return (
    <div
      class="week-view"
      classList={{ 'week-view--single': compact() }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={release}
      onPointerCancel={release}
    >
      <Show when={compact() && selected()}>
        {(current) => (
          <DayTabs
            days={props.days}
            selected={current()}
            onSelect={(date) => goTo(props.days.findIndex((day) => day.date === date))}
          />
        )}
      </Show>

      <Show
        when={compact()}
        fallback={
          <Show
            when={props.view === 'grid'}
            fallback={<DayStrip {...props} days={props.days} byDay={byDay()} />}
          >
            <TimeGrid {...props} days={props.days} byDay={byDay()} />
          </Show>
        }
      >
        <div class="pager" ref={frame}>
          <div class="pager__track" ref={track}>
            <For each={props.days}>
              {(day, position) => (
                <div
                  class="pager__page"
                  classList={{ 'pager__page--shown': position() === index() }}
                  style={{ '--page': String(position()) }}
                  aria-hidden={position() === index() ? undefined : 'true'}
                >
                  <Show
                    when={props.view === 'grid'}
                    fallback={<DayStrip {...props} days={[day]} byDay={byDay()} />}
                  >
                    <TimeGrid {...props} days={[day]} byDay={byDay()} />
                  </Show>
                </div>
              )}
            </For>
          </div>
        </div>
      </Show>
    </div>
  );
};

type ViewProps = Props & { byDay: Map<string, Course[]> };

/** Les jours à la suite, chacun dans sa carte. */
const DayStrip: Component<ViewProps> = (props) => (
  <div class="day-strip">
    <For each={props.days}>
      {(day) => (
        <div class="day-strip__column">
          <DayCard
            date={day.date}
            courses={props.byDay.get(day.date) ?? []}
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
    <div
      class="week-grid"
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
          <div class="week-grid__day" classList={{ 'week-grid__day--today': day.date === today() }}>
            <div class="week-grid__day-name">{dayLabel(day.date)}</div>
            <div class="week-grid__day-date">{shortDate(day.date)}</div>
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
          <div
            class="week-grid__column"
            classList={{ 'week-grid__column--today': day.date === today() }}
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
              <div class="week-grid__now" style={{ top: `${offset(minutes())}px` }} />
            </Show>
          </div>
        )}
      </For>
    </div>
  );
};
