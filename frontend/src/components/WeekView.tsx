import { For, Show, createMemo, createSignal, type Component, type ParentComponent } from 'solid-js';

import type { Course, Week } from '../lib/api';
import { blockStyle, gridScale, isShort, packDay } from '../lib/layout';
import { useCompact } from '../lib/media';
import type { ViewMode } from '../lib/settings';
import { clock, dayIndex, dayShort, isRunning, nowMinutes, shortDate, todayIso } from '../lib/time';
import { cx } from '../lib/ui';
import { CourseBlock } from './CourseBlock';
import { DayCard } from './DayCard';
import { DayTabs } from './DayTabs';
import { Pager } from './Pager';

export interface Day {
  name: string;
  date: string;
}

interface Props {
  week: Week;
  /** Colonnes à afficher, jours vides pour le groupe choisi compris. */
  days: Day[];
  view: ViewMode;
  /** Ouvre le détail d'un cours. */
  onSelect: (course: Course) => void;
  /** Remplace l'enseignant dans la ligne de détail d'un cours. */
  describe?: (course: Course) => string;
  /** Dit d'un cours qu'il a bougé depuis la dernière visite. */
  changed?: (course: Course) => string | undefined;
}

function coursesByDay(courses: Course[]): Map<string, Course[]> {
  const byDay = new Map<string, Course[]>();
  for (const course of courses) {
    byDay.set(course.date, [...(byDay.get(course.date) ?? []), course]);
  }
  return byDay;
}

/**
 * La semaine, en grille horaire (`semaine-grille.html`) ou en liste par jour
 * (`semaine-liste.html`). Sur écran étroit, elle se lit un jour à la fois
 * (`mobile-semaine.html`) : sélecteur Lun–Ven, et le doigt feuillette.
 */
export const WeekView: Component<Props> = (props) => {
  const byDay = createMemo(() => coursesByDay(props.week.courses));
  const compact = useCompact();

  // On retient le jour ouvert, pas son rang : la page est juste dès le premier
  // rendu, sans saut ni glissement parasite.
  const [chosen, setChosen] = createSignal<string | null>(null);
  const index = () => dayIndex(props.days, chosen());
  const open = (position: number) => setChosen(props.days[position]?.date ?? null);

  const view = (only: Day[], single: boolean) => (
    <Show
      when={props.view === 'grid'}
      fallback={
        <Show
          when={single}
          fallback={
            <div class="week-scroll">
              <div class="week-days" style={{ '--day-count': String(only.length) }}>
                <For each={only}>
                  {(day) => (
                    <DayCard
                      day={day}
                      courses={byDay().get(day.date) ?? []}
                      describe={props.describe}
                      changed={props.changed}
                      onSelect={props.onSelect}
                    />
                  )}
                </For>
              </div>
            </div>
          }
        >
          <DayCard
            day={only[0]!}
            courses={byDay().get(only[0]!.date) ?? []}
            describe={props.describe}
            changed={props.changed}
            onSelect={props.onSelect}
            mobile
          />
        </Show>
      }
    >
      <TimeGrid
        days={only}
        courses={props.week.courses}
        byDay={byDay()}
        single={single}
        describe={props.describe}
        changed={props.changed}
        onSelect={props.onSelect}
      />
    </Show>
  );

  return (
    <Show when={compact()} fallback={view(props.days, false)}>
      <WeekFrame>
        <DayTabs days={props.days} selected={props.days[index()]?.date ?? ''} onSelect={setChosen} />
        <Pager items={props.days} index={index()} onIndex={open} enabled>
          {(only) => view([only], true)}
        </Pager>
      </WeekFrame>
    </Show>
  );
};

/**
 * Le cadre d'une semaine lue un jour à la fois. Le geste horizontal y est
 * celui de l'application : on interdit le panoramique latéral au navigateur,
 * sinon il le prend pour un retour en arrière et quitte la page.
 */
export const WeekFrame: ParentComponent = (props) => (
  <div class="flex touch-pan-y flex-col gap-3.5">{props.children}</div>
);

/** En-tête de colonne d'une grille : `LUN` puis `19 oct.` */
export const DayHead: Component<{ day: Day }> = (props) => (
  <div class={cx('dh', props.day.date === todayIso(clock()) && 'today')}>
    <span>{dayShort(props.day.name)}</span>
    <b>{shortDate(props.day.date)}</b>
  </div>
);

/** Repère de l'heure courante, posé dans la colonne du jour. */
export const NowLine: Component<{ top: number }> = (props) => (
  <div
    class="pointer-events-none absolute inset-x-0 z-[5] h-0.5 bg-accent before:absolute before:-left-1 before:-top-[3px] before:size-2 before:rounded-full before:bg-accent"
    style={{ top: `${props.top}px` }}
    aria-hidden="true"
  />
);

/** Les graduations de la colonne des heures. */
export const Hours: Component<{ hours: string[] }> = (props) => (
  <div class="hrs tabular-nums">
    <For each={props.hours}>{(hour) => <div>{hour}</div>}</For>
  </div>
);

/** La semaine sur l'échelle horaire 08:00 → 20:00 (56 px par heure). */
const TimeGrid: Component<{
  days: Day[];
  /** Tous les cours de la semaine : l'échelle est la même pour chaque jour. */
  courses: Course[];
  byDay: Map<string, Course[]>;
  single: boolean;
  describe?: (course: Course) => string;
  changed?: (course: Course) => string | undefined;
  onSelect: (course: Course) => void;
}> = (props) => {
  const today = createMemo(() => todayIso(clock()));
  const minutes = createMemo(() => nowMinutes(clock()));
  const scale = createMemo(() => gridScale(props.courses));

  return (
    <div class={cx('card week-grid', !props.single && 'week-scroll')}>
      <div
        class="grid"
        style={{
          'grid-template-columns': `56px repeat(${props.days.length}, minmax(${props.single ? 0 : 128}px, 1fr))`,
        }}
      >
        <div />
        <For each={props.days}>{(day) => <DayHead day={day} />}</For>

        <Hours hours={scale().hours} />

        <For each={props.days}>
          {(day) => (
            <div class="sched" style={{ height: `${scale().height}px` }}>
              <For each={packDay(props.byDay.get(day.date) ?? [])}>
                {(placed) => (
                  <CourseBlock
                    course={placed.course}
                    live={isRunning(placed.course, today(), minutes())}
                    dense={isShort(placed.course)}
                    describe={props.describe}
                    changed={props.changed?.(placed.course)}
                    onSelect={() => props.onSelect(placed.course)}
                    style={blockStyle(placed, scale())}
                  />
                )}
              </For>

              <Show when={day.date === today() && minutes() >= scale().start && minutes() <= scale().end}>
                <NowLine top={scale().offset(minutes())} />
              </Show>
            </div>
          )}
        </For>
      </div>
    </div>
  );
};
