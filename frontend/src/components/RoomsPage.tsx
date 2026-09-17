import { For, Show, createMemo, createResource, createSignal, type Component } from 'solid-js';

import type { Course, YearSummary } from '../lib/api';
import { groupLabel } from '../lib/groups';
import { DAY_END, DAY_START, KIND_LABEL } from '../lib/layout';
import {
  atTime,
  formatMinutes,
  loadRooms,
  roomDays,
  roomState,
  type RoomDay,
  type RoomState,
} from '../lib/rooms';
import { clock, longDate, nowMinutes, todayIso, toMinutes } from '../lib/time';

interface Props {
  years: YearSummary[];
  onSelect: (course: Course) => void;
}

type Mode = 'state' | 'planning';

/** « TP R1.01 · G1A » */
function describe(course: Course): string {
  return [
    KIND_LABEL[course.kind],
    course.resource?.code ?? course.title,
    course.groups.length > 0 ? course.groups.map(groupLabel).join(', ') : undefined,
  ]
    .filter(Boolean)
    .join(' · ');
}

/**
 * Salles libres et occupées. Par défaut l'instant suit l'horloge ; on peut
 * aussi choisir un jour et une heure, puis revenir au temps réel.
 */
export const RoomsPage: Component<Props> = (props) => {
  /** Instant choisi, ou `null` pour suivre l'heure courante. */
  const [pinned, setPinned] = createSignal<Date | null>(null);
  const [mode, setMode] = createSignal<Mode>('state');

  const instant = () => pinned() ?? clock();
  const date = createMemo(() => todayIso(instant()));
  const minutes = createMemo(() => nowMinutes(instant()));

  const [data] = createResource(
    () => ({ years: props.years, date: date() }),
    ({ years, date: day }) => loadRooms(years, day),
  );

  const rooms = createMemo(() => {
    const loaded = data.latest;
    return loaded ? roomDays(loaded.courses, date()) : [];
  });

  const states = createMemo(() => rooms().map((room) => roomState(room, minutes())));
  const free = () => states().filter((state) => state.free);
  const busy = () => states().filter((state) => !state.free);

  const pin = (day: string, time: string) => {
    if (day && time) setPinned(atTime(day, time));
  };

  return (
    <div class="rooms">
      <div class="summary">
        <div class="card rooms-controls">
          <div class="rooms-controls__when">
            <input
              class="input"
              type="date"
              aria-label="Jour"
              value={date()}
              onChange={(event) => pin(event.currentTarget.value, formatMinutes(minutes()))}
            />
            <input
              class="input"
              type="time"
              step="300"
              aria-label="Heure"
              value={formatMinutes(minutes())}
              onChange={(event) => pin(date(), event.currentTarget.value)}
            />
            <button
              type="button"
              class="button"
              aria-pressed={pinned() === null}
              onClick={() => setPinned(null)}
            >
              Maintenant
            </button>
          </div>

          <div class="segment" role="group" aria-label="Affichage">
            <button
              type="button"
              class="segment__option"
              aria-pressed={mode() === 'state'}
              onClick={() => setMode('state')}
            >
              Libres / occupées
            </button>
            <button
              type="button"
              class="segment__option"
              aria-pressed={mode() === 'planning'}
              onClick={() => setMode('planning')}
            >
              Planning
            </button>
          </div>
        </div>
      </div>

      <main class="app__timetable">
        <Show
          when={data.latest}
          fallback={
            <div class="centered">
              <Show when={data.error} fallback={<p>Chargement des salles…</p>}>
                <p>{`Salles indisponibles : ${(data.error as Error).message}`}</p>
              </Show>
            </div>
          }
        >
          {(loaded) => (
            <Show
              when={loaded().covered && rooms().length > 0}
              fallback={
                <div class="centered">
                  <p>{`Aucun emploi du temps publié pour le ${longDate(date())}.`}</p>
                </div>
              }
            >
              <Show
                when={mode() === 'state'}
                fallback={
                  <RoomPlanning
                    rooms={rooms()}
                    minutes={minutes()}
                    live={pinned() === null}
                    onSelect={props.onSelect}
                    onPick={(m) => pin(date(), formatMinutes(m))}
                  />
                }
              >
                <div class="rooms-state">
                  <p class="rooms-state__caption">
                    {`${pinned() === null ? 'En ce moment' : 'Le'} ${longDate(date())} à ${formatMinutes(minutes())}`}
                  </p>
                  <RoomSection title="Libres" states={free()} onSelect={props.onSelect} />
                  <RoomSection title="Occupées" states={busy()} onSelect={props.onSelect} />
                </div>
              </Show>
            </Show>
          )}
        </Show>
      </main>

      <footer class="app__footer">
        <span>Salles relevées dans les emplois du temps de toutes les années, cette semaine-là.</span>
      </footer>
    </div>
  );
};

const RoomSection: Component<{
  title: string;
  states: RoomState[];
  onSelect: (course: Course) => void;
}> = (props) => (
  <section class="rooms-section">
    <h2 class="rooms-section__title">
      {props.title} <span class="rooms-section__count">{props.states.length}</span>
    </h2>
    <Show when={props.states.length > 0} fallback={<p class="rooms-section__empty">Aucune.</p>}>
      <div class="rooms-section__list">
        <For each={props.states}>
          {(state) => (
            <Show
              when={!state.free && state}
              fallback={
                <div class="room-card room-card--free">
                  <span class="room-card__name">{state.room.room}</span>
                  <span class="room-card__until">
                    {state.until === null ? 'libre jusqu’au soir' : `libre jusqu’à ${formatMinutes(state.until)}`}
                  </span>
                </div>
              }
            >
              {(occupied) => (
                <button
                  type="button"
                  class={`room-card room-card--busy kind-${occupied().course.kind}`}
                  onClick={() => props.onSelect(occupied().course)}
                >
                  <span class="room-card__name">{occupied().room.room}</span>
                  <span class="room-card__until">{`jusqu’à ${formatMinutes(occupied().until)}`}</span>
                  <span class="room-card__course">{describe(occupied().course)}</span>
                </button>
              )}
            </Show>
          )}
        </For>
      </div>
    </Show>
  </section>
);

/** Largeur d'une heure dans le planning, en pixels. */
const HOUR_WIDTH = 88;

/** Une ligne par salle sur l'échelle horaire de la journée. */
const RoomPlanning: Component<{
  rooms: RoomDay[];
  minutes: number;
  live: boolean;
  onSelect: (course: Course) => void;
  onPick: (minutes: number) => void;
}> = (props) => {
  const bounds = createMemo(() => {
    const courses = props.rooms.flatMap((room) => room.courses);
    const start = courses.reduce((min, course) => Math.min(min, toMinutes(course.start)), DAY_START);
    const end = courses.reduce((max, course) => Math.max(max, toMinutes(course.end)), DAY_END);
    return { start: Math.floor(start / 60) * 60, end: Math.ceil(end / 60) * 60 };
  });

  const width = () => ((bounds().end - bounds().start) / 60) * HOUR_WIDTH;
  const offset = (m: number) => ((m - bounds().start) / 60) * HOUR_WIDTH;

  const hours = createMemo(() => {
    const list: number[] = [];
    for (let m = bounds().start; m <= bounds().end; m += 60) list.push(m);
    return list;
  });

  /** Un clic sur une zone libre place l'heure choisie, arrondie au quart d'heure. */
  const pick = (event: MouseEvent) => {
    const track = event.currentTarget as HTMLElement;
    const x = event.clientX - track.getBoundingClientRect().left;
    const m = bounds().start + Math.round(((x / HOUR_WIDTH) * 60) / 15) * 15;
    props.onPick(Math.min(Math.max(m, bounds().start), bounds().end));
  };

  const showNow = () => props.minutes >= bounds().start && props.minutes <= bounds().end;

  return (
    <div class="room-planning" style={{ '--hour-width': `${HOUR_WIDTH}px`, '--track-width': `${width()}px` }}>
      <div class="room-planning__corner" />
      <div class="room-planning__axis">
        <For each={hours()}>
          {(hour) => (
            <span class="room-planning__hour" style={{ left: `${offset(hour)}px` }}>
              {`${hour / 60}h`}
            </span>
          )}
        </For>
      </div>

      <For each={props.rooms}>
        {(room) => {
          const state = () => roomState(room, props.minutes);
          return (
            <>
              <div class="room-planning__room" classList={{ 'room-planning__room--busy': !state().free }}>
                <span class="room-planning__dot" aria-hidden="true" />
                {room.room}
              </div>
              <div class="room-planning__track" onClick={pick}>
                <For each={room.courses}>
                  {(course) => (
                    <button
                      type="button"
                      class={`room-planning__slot kind-${course.kind}`}
                      style={{
                        left: `${offset(toMinutes(course.start)) + 1}px`,
                        width: `${offset(toMinutes(course.end)) - offset(toMinutes(course.start)) - 2}px`,
                      }}
                      title={`${course.start} – ${course.end} · ${describe(course)}`}
                      onClick={(event) => {
                        event.stopPropagation();
                        props.onSelect(course);
                      }}
                    >
                      <span class="room-planning__slot-kind">{KIND_LABEL[course.kind]}</span>
                      {course.resource?.code ?? course.title ?? ''}
                    </button>
                  )}
                </For>
                <Show when={showNow()}>
                  <div
                    class="room-planning__now"
                    classList={{ 'room-planning__now--pinned': !props.live }}
                    style={{ left: `${offset(props.minutes)}px` }}
                  />
                </Show>
              </div>
            </>
          );
        }}
      </For>
    </div>
  );
};
