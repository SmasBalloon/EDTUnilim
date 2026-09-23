import { For, Show, createMemo, createResource, createSignal, type Component } from 'solid-js';
import { X } from 'lucide-solid';

import type { Course, YearSummary } from '../lib/api';
import { groupLabel } from '../lib/groups';
import { DAY_END, DAY_START, KIND_LABEL } from '../lib/layout';
import {
  atTime,
  byFloor,
  formatMinutes,
  loadRooms,
  roomDays,
  roomState,
  type RoomDay,
  type RoomState,
} from '../lib/rooms';
import { clock, longDate, nowMinutes, todayIso, toMinutes } from '../lib/time';
import {
  button,
  card,
  centered,
  cx,
  gridCorner,
  gridHead,
  input,
  kindEdge,
  page,
  pill,
  press,
  sectionTitle,
  segment,
  segmentOption,
  summary,
} from '../lib/ui';
import { Footer } from './Footer';

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
 * aussi choisir un jour, une heure de début et, pour une plage, une heure de
 * fin — puis revenir au temps réel.
 */
export const RoomsPage: Component<Props> = (props) => {
  /** Début choisi, ou `null` pour suivre l'heure courante. */
  const [pinned, setPinned] = createSignal<Date | null>(null);
  /** Fin de plage en minutes, ou `null` pour un simple instant. */
  const [end, setEnd] = createSignal<number | null>(null);
  const [mode, setMode] = createSignal<Mode>('state');

  const instant = () => pinned() ?? clock();
  const date = createMemo(() => todayIso(instant()));
  const minutes = createMemo(() => nowMinutes(instant()));

  /** La plage n'a de sens que si sa fin suit son début. */
  const rangeEnd = createMemo(() => {
    const value = end();
    return value !== null && value > minutes() ? value : null;
  });
  const invalidRange = () => end() !== null && rangeEnd() === null;

  const [data] = createResource(
    () => ({ years: props.years, date: date() }),
    ({ years, date: day }) => loadRooms(years, day),
  );

  const rooms = createMemo(() => {
    const loaded = data.latest;
    return loaded ? roomDays(loaded.courses, date()) : [];
  });

  const states = createMemo(() =>
    rooms().map((room) => roomState(room, minutes(), rangeEnd() ?? undefined)),
  );
  const free = () => states().filter((state) => state.free);
  const busy = () => states().filter((state) => !state.free);

  const pin = (day: string, time: string) => {
    if (day && time) setPinned(atTime(day, time));
  };

  const caption = () => {
    const prefix = pinned() === null && rangeEnd() === null ? 'En ce moment' : 'Le';
    const span =
      rangeEnd() === null
        ? `à ${formatMinutes(minutes())}`
        : `de ${formatMinutes(minutes())} à ${formatMinutes(rangeEnd()!)}`;
    return `${prefix} ${longDate(date())} ${span}`;
  };

  return (
    <div class={page}>
      <div class={summary}>
        <div class={`${card} flex flex-wrap items-center justify-between gap-x-gap gap-y-2 px-2.5 py-2`}>
          <div class="flex flex-wrap items-center gap-1.5">
            <input
              class={input()}
              type="date"
              aria-label="Jour"
              value={date()}
              onChange={(event) => pin(event.currentTarget.value, formatMinutes(minutes()))}
            />
            <label class="inline-flex items-center gap-1 text-[0.8125rem] text-muted">
              <span>De</span>
              <input
                class={input()}
                type="time"
                step="300"
                aria-label="Heure de début"
                value={formatMinutes(minutes())}
                onChange={(event) => pin(date(), event.currentTarget.value)}
              />
            </label>
            <label class="inline-flex items-center gap-1 text-[0.8125rem] text-muted">
              <span>à</span>
              <input
                class={input({ invalid: invalidRange() })}
                type="time"
                step="300"
                aria-label="Heure de fin (facultative)"
                value={end() === null ? '' : formatMinutes(end()!)}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  if (!value) return setEnd(null);
                  // Saisir une fin fige le début là où il en est.
                  if (pinned() === null) setPinned(instant());
                  setEnd(toMinutes(value));
                }}
              />
            </label>
            <Show when={end() !== null}>
              <button
                type="button"
                class={button.plain}
                aria-label="Retirer l'heure de fin"
                title="Retirer l'heure de fin"
                onClick={() => setEnd(null)}
              >
                <X size={14} aria-hidden="true" />
              </button>
            </Show>
            <button
              type="button"
              class={button.plain}
              aria-pressed={pinned() === null && end() === null}
              onClick={() => {
                setPinned(null);
                setEnd(null);
              }}
            >
              Maintenant
            </button>
          </div>

          <div class={segment} role="group" aria-label="Affichage">
            <button
              type="button"
              class={segmentOption}
              aria-pressed={mode() === 'state'}
              onClick={() => setMode('state')}
            >
              Libres / occupées
            </button>
            <button
              type="button"
              class={segmentOption}
              aria-pressed={mode() === 'planning'}
              onClick={() => setMode('planning')}
            >
              Planning
            </button>
          </div>

          <Show when={invalidRange()}>
            <p class="m-0 basis-full text-[0.8125rem] text-accent">
              L'heure de fin doit être après l'heure de début.
            </p>
          </Show>
        </div>
      </div>

      <main class="timetable">
        <Show
          when={data.latest}
          fallback={
            <div class={centered}>
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
                <div class={centered}>
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
                    end={rangeEnd()}
                    live={pinned() === null}
                    onSelect={props.onSelect}
                    onPick={(m) => pin(date(), formatMinutes(m))}
                  />
                }
              >
                <div class="grid gap-5 px-edge pb-5">
                  <p class="m-0 text-sm text-muted first-letter:uppercase">{caption()}</p>
                  <RoomSection
                    title={rangeEnd() === null ? 'Libres' : 'Libres sur toute la plage'}
                    states={free()}
                    range={rangeEnd() !== null}
                    onSelect={props.onSelect}
                  />
                  <RoomSection
                    title={rangeEnd() === null ? 'Occupées' : 'Occupées sur la plage'}
                    states={busy()}
                    range={rangeEnd() !== null}
                    onSelect={props.onSelect}
                  />
                </div>
              </Show>
            </Show>
          )}
        </Show>
      </main>

      <Footer>
        <span>Salles relevées dans les emplois du temps de toutes les années, cette semaine-là.</span>
      </Footer>
    </div>
  );
};

const RoomSection: Component<{
  title: string;
  states: RoomState[];
  /** Vrai quand on regarde une plage plutôt qu'un instant. */
  range: boolean;
  onSelect: (course: Course) => void;
}> = (props) => (
  <section>
    <h2 class={sectionTitle}>
      {props.title} <span class={pill}>{props.states.length}</span>
    </h2>
    <Show
      when={props.states.length > 0}
      fallback={<p class="m-0 text-sm text-muted">Aucune.</p>}
    >
      <For each={byFloor(props.states, (state) => state.room.room)}>
        {(group) => (
          <div class="[&+&]:mt-3">
            <h3 class="mb-1.5 flex items-center gap-2 text-xs font-medium text-muted">
              <span class="min-w-8 rounded-md bg-elevated px-[0.4rem] py-[0.05rem] text-center text-xs font-bold tabular-nums">
                {group.floor.name}
              </span>
              {group.floor.label}
            </h3>
            <div class="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-2">
              <For each={group.items}>
                {(state) => (
                  <Show
                    when={!state.free && state}
                    fallback={
                      <div class={cx(roomCard, 'kind-sae bg-surface', kindEdge)}>
                        <span class="text-base font-semibold tabular-nums">{state.room.room}</span>
                        <span class="text-[0.8125rem] text-muted tabular-nums">
                          {state.until === null
                            ? 'libre jusqu’au soir'
                            : `libre jusqu’à ${formatMinutes(state.until)}`}
                        </span>
                      </div>
                    }
                  >
                    {(occupied) => (
                      <div
                        class={cx(roomCard, `kind-${occupied().courses[0]!.kind} bg-kind-tint`, kindEdge)}
                      >
                        <span class="text-base font-semibold tabular-nums">{occupied().room.room}</span>
                        <Show when={!props.range}>
                          <span class="text-[0.8125rem] text-muted tabular-nums">
                            {`jusqu’à ${formatMinutes(occupied().until)}`}
                          </span>
                        </Show>
                        <For each={props.range ? occupied().courses : occupied().courses.slice(0, 1)}>
                          {(course) => (
                            <button
                              type="button"
                              class="overflow-hidden truncate border-0 bg-transparent p-0 text-start text-2xs text-inherit [font:inherit] cursor-pointer hover:underline"
                              onClick={() => props.onSelect(course)}
                            >
                              {props.range
                                ? `${course.start}–${course.end} · ${describe(course)}`
                                : describe(course)}
                            </button>
                          )}
                        </For>
                      </div>
                    )}
                  </Show>
                )}
              </For>
            </div>
          </div>
        )}
      </For>
    </Show>
  </section>
);

/** Une salle, libre (liseré vert) ou occupée (teinte du cours en place). */
const roomCard = `${press} grid gap-0.5 rounded-control border border-line px-3 py-2 text-start`;

/** Largeur d'une heure dans le planning, en pixels. */
const HOUR_WIDTH = 88;

/** Une ligne par salle sur l'échelle horaire de la journée, regroupées par étage. */
const RoomPlanning: Component<{
  rooms: RoomDay[];
  minutes: number;
  end: number | null;
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
  const clamp = (m: number) => Math.min(Math.max(m, bounds().start), bounds().end);

  const hours = createMemo(() => {
    const list: number[] = [];
    for (let m = bounds().start; m <= bounds().end; m += 60) list.push(m);
    return list;
  });

  const floors = createMemo(() => byFloor(props.rooms, (room) => room.room));

  /** Un clic sur une zone libre place l'heure choisie, arrondie au quart d'heure. */
  const pick = (event: MouseEvent) => {
    const track = event.currentTarget as HTMLElement;
    const x = event.clientX - track.getBoundingClientRect().left;
    props.onPick(clamp(bounds().start + Math.round(((x / HOUR_WIDTH) * 60) / 15) * 15));
  };

  const showNow = () => props.minutes >= bounds().start && props.minutes <= bounds().end;

  return (
    // Une ligne par salle, l'échelle horaire défile à l'horizontale.
    <div
      class="grid w-max grid-cols-[clamp(3.75rem,14vw,5rem)_var(--track-width)] pr-edge pb-4"
      style={{ '--hour-width': `${HOUR_WIDTH}px`, '--track-width': `${width()}px` }}
    >
      <div class={`${gridCorner} h-7`} />
      <div class={`${gridHead} h-7`}>
        <For each={hours()}>
          {(hour) => (
            <span
              class="absolute bottom-[0.3rem] -translate-x-1/2 text-2xs text-muted tabular-nums first:translate-x-0"
              style={{ left: `${offset(hour)}px` }}
            >
              {`${hour / 60}h`}
            </span>
          )}
        </For>
      </div>

      <For each={floors()}>
        {(group) => (
          <>
            <div class="sticky left-0 z-10 flex h-7 items-center gap-1.5 whitespace-nowrap border-b border-line bg-elevated pl-edge text-2xs text-muted">
              <span class="min-w-8 rounded-md bg-elevated px-[0.4rem] py-[0.05rem] text-center text-xs font-bold tabular-nums">
                {group.floor.name}
              </span>
              {group.floor.label}
            </div>
            <div class="h-7 border-b border-line bg-elevated" />

            <For each={group.items}>
              {(room) => {
                const state = () => roomState(room, props.minutes, props.end ?? undefined);
                return (
                  <>
                    <div class="sticky left-0 z-10 flex h-9 items-center gap-1.5 border-b border-line bg-bg pl-edge text-[0.8125rem] font-semibold tabular-nums">
                      <span
                        aria-hidden="true"
                        class={`h-[0.4375rem] w-[0.4375rem] shrink-0 rounded-full ${
                          state().free ? 'bg-kind-sae' : 'bg-accent'
                        }`}
                      />
                      {room.room}
                    </div>
                    <div class="hour-lines-x relative h-9 cursor-crosshair border-b border-line" onClick={pick}>
                      <Show when={props.end}>
                        {(end) => (
                          // Plage choisie, en fond de chaque ligne.
                          <div
                            class="pointer-events-none absolute inset-y-0 border-x border-dashed border-accent bg-accent/14"
                            style={{
                              left: `${offset(clamp(props.minutes))}px`,
                              width: `${offset(clamp(end())) - offset(clamp(props.minutes))}px`,
                            }}
                          />
                        )}
                      </Show>
                      <For each={room.courses}>
                        {(course) => (
                          <button
                            type="button"
                            class={cx(
                              `kind-${course.kind}`,
                              'absolute inset-y-1 flex cursor-pointer items-center gap-1 overflow-hidden whitespace-nowrap',
                              'rounded-md border border-kind/26 bg-kind-tint px-1.5 text-2xs font-semibold text-text hover:shadow-card',
                              kindEdge,
                            )}
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
                            <span class="text-kind-ink text-[0.625rem] font-bold uppercase tracking-[0.04em]">
                              {KIND_LABEL[course.kind]}
                            </span>
                            {course.resource?.code ?? course.title ?? ''}
                          </button>
                        )}
                      </For>
                      <Show when={showNow() && props.end === null}>
                        <div
                          class={cx(
                            'pointer-events-none absolute top-0 -bottom-px z-[5] -ml-px w-0.5',
                            props.live
                              ? 'bg-accent'
                              : 'bg-[repeating-linear-gradient(to_bottom,var(--color-accent)_0_4px,transparent_4px_7px)]',
                          )}
                          style={{ left: `${offset(props.minutes)}px` }}
                        />
                      </Show>
                    </div>
                  </>
                );
              }}
            </For>
          </>
        )}
      </For>
    </div>
  );
};
