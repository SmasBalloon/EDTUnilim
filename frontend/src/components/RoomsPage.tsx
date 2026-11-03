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
import { button, cx, kindCard, kindTag, sectionTitle, segment } from '../lib/ui';
import { Empty, Foot, PageHead } from './Shell';

interface Props {
  years: YearSummary[];
  onSelect: (course: Course) => void;
}

type Mode = 'state' | 'planning';

/** « R1.01 · G1A » : le type est déjà dit par le tag. */
function brief(course: Course): string {
  return [
    course.resource?.code ?? course.title,
    course.groups.length > 0 ? course.groups.map(groupLabel).join(', ') : undefined,
  ]
    .filter(Boolean)
    .join(' · ');
}

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
    <>
      <PageHead title="Salles" sub="Libres ou occupées, maintenant ou à l’heure choisie.">
        <div class={segment} role="group" aria-label="Affichage">
          <button type="button" aria-pressed={mode() === 'state'} onClick={() => setMode('state')}>
            Libres / occupées
          </button>
          <button type="button" aria-pressed={mode() === 'planning'} onClick={() => setMode('planning')}>
            Planning
          </button>
        </div>
      </PageHead>

      <div class="card flex flex-wrap items-center gap-3.5 px-[18px] py-3.5">
        <label class="field">
          Date
          <input
            type="date"
            class="w-[8.5rem]"
            value={date()}
            onChange={(event) => pin(event.currentTarget.value, formatMinutes(minutes()))}
          />
        </label>
        <label class="field">
          De
          <input
            type="time"
            step="300"
            class="w-[5.5rem]"
            aria-label="Heure de début"
            value={formatMinutes(minutes())}
            onChange={(event) => pin(date(), event.currentTarget.value)}
          />
        </label>
        <label class="field" aria-invalid={invalidRange()}>
          à
          <input
            type="time"
            step="300"
            class="w-[5.5rem]"
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
          <Show when={end() !== null}>
            <button
              type="button"
              class="-mr-2 grid size-8 cursor-pointer place-items-center rounded-lg text-muted hover:bg-raised hover:text-text"
              aria-label="Retirer l'heure de fin"
              title="Retirer l'heure de fin"
              onClick={() => setEnd(null)}
            >
              <X size={16} aria-hidden="true" />
            </button>
          </Show>
        </label>
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
        <span class="ml-auto text-sm text-muted first-letter:uppercase">{caption()}</span>

        <Show when={invalidRange()}>
          <p class="basis-full text-[13px] text-accent-ink">L'heure de fin doit être après l'heure de début.</p>
        </Show>
      </div>

      <Show
        when={data.latest}
        fallback={
          <Show when={data.error} fallback={<Empty>Chargement des salles…</Empty>}>
            <Empty title="Salles indisponibles">{(data.error as Error).message}</Empty>
          </Show>
        }
      >
        {(loaded) => (
          <Show
            when={loaded().covered && rooms().length > 0}
            fallback={
              <Empty title="Rien de publié">{`Aucun emploi du temps publié pour le ${longDate(date())}.`}</Empty>
            }
          >
            <Show
              when={mode() === 'state'}
              fallback={
                <div class="card rooms-planning">
                  <RoomPlanning
                    rooms={rooms()}
                    minutes={minutes()}
                    end={rangeEnd()}
                    live={pinned() === null}
                    onSelect={props.onSelect}
                    onPick={(m) => pin(date(), formatMinutes(m))}
                  />
                </div>
              }
            >
              <RoomSection
                title={rangeEnd() === null ? 'Libres' : 'Libres sur toute la plage'}
                tone="t-sae"
                states={free()}
                range={rangeEnd() !== null}
                onSelect={props.onSelect}
              />
              <RoomSection
                title={rangeEnd() === null ? 'Occupées' : 'Occupées sur la plage'}
                tone="t-room"
                states={busy()}
                range={rangeEnd() !== null}
                onSelect={props.onSelect}
              />
            </Show>
          </Show>
        )}
      </Show>

      <Foot>
        <span>Salles relevées dans les emplois du temps de toutes les années, cette semaine-là.</span>
      </Foot>
    </>
  );
};

const RoomSection: Component<{
  title: string;
  /** Teinte du compteur : vert pour les libres, accent pour les occupées. */
  tone: string;
  states: RoomState[];
  /** Vrai quand on regarde une plage plutôt qu'un instant. */
  range: boolean;
  onSelect: (course: Course) => void;
}> = (props) => (
  <section>
    <div class="row mb-[18px] gap-2.5">
      <h2 class={sectionTitle}>{props.title}</h2>
      <span class={`tag ${props.tone}`}>{props.states.length}</span>
    </div>
    <Show when={props.states.length > 0} fallback={<p class="text-sm text-muted">Aucune.</p>}>
      <div class="flex flex-col gap-[22px]">
        <For each={byFloor(props.states, (state) => state.room.room)}>
          {(group) => (
            <div>
              <div class="rooms-fl">
                <span class="tag t-gray">{group.floor.name}</span>
                <b>{group.floor.label}</b>
              </div>
              <div class="rooms-grid">
                <For each={group.items}>
                  {(state) => (
                    <Show
                      when={!state.free && state}
                      fallback={
                        <div class="rooms-rm">
                          <b>{state.room.room}</b>
                          <span>
                            {state.until === null
                              ? 'libre jusqu’au soir'
                              : `libre jusqu’à ${formatMinutes(state.until)}`}
                          </span>
                        </div>
                      }
                    >
                      {(occupied) => (
                        <div class={cx('rooms-rm', kindCard(occupied().courses[0]!.kind))}>
                          <b>{occupied().room.room}</b>
                          {/* Sur une plage, chaque cours porte son horaire. */}
                          <Show when={!props.range}>
                            <span>{`${occupied().courses[0]!.start} – ${occupied().courses[0]!.end}`}</span>
                          </Show>
                          <For each={props.range ? occupied().courses : occupied().courses.slice(0, 1)}>
                            {(course) => (
                              <div class="tg">
                                <span class={kindTag(course.kind)}>{KIND_LABEL[course.kind]}</span>
                                <button type="button" onClick={() => props.onSelect(course)} title={describe(course)}>
                                  {props.range ? `${course.start}–${course.end} · ${brief(course)}` : brief(course)}
                                </button>
                              </div>
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
      </div>
    </Show>
  </section>
);

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
      class="grid w-max grid-cols-[clamp(4.5rem,16vw,6rem)_var(--track-width)] pr-4 pb-2"
      style={{ '--hour-width': `${HOUR_WIDTH}px`, '--track-width': `${width()}px` }}
    >
      <div class="sticky top-0 left-0 z-30 h-8 border-b border-line bg-surface" />
      <div class="sticky top-0 z-20 h-8 border-b border-line bg-surface">
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
            <div class="sticky left-0 z-10 flex h-8 items-center gap-2 whitespace-nowrap border-b border-line bg-elevated pl-4 text-xs font-semibold text-muted">
              <span class="tag t-gray h-5!">{group.floor.name}</span>
              {group.floor.label}
            </div>
            <div class="h-8 border-b border-line bg-elevated" />

            <For each={group.items}>
              {(room) => {
                const state = () => roomState(room, props.minutes, props.end ?? undefined);
                return (
                  <>
                    <div class="sticky left-0 z-10 flex h-10 items-center gap-2 border-b border-line bg-surface pl-4 font-display text-[15px] font-semibold tabular-nums">
                      <span
                        aria-hidden="true"
                        class={`h-[0.4375rem] w-[0.4375rem] shrink-0 rounded-full ${
                          state().free ? 'bg-ok' : 'bg-accent'
                        }`}
                      />
                      {room.room}
                    </div>
                    <div class="hour-lines-x relative h-10 cursor-crosshair border-b border-line" onClick={pick}>
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
                              kindCard(course.kind),
                              'absolute inset-y-1 flex cursor-pointer items-center gap-1.5 overflow-hidden whitespace-nowrap',
                              'rounded-lg border px-2 text-xs font-semibold text-text',
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
                            <span class={`${kindTag(course.kind)} h-[18px]! px-1.5! text-[10px]!`}>
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
                              : 'bg-[repeating-linear-gradient(to_bottom,var(--edt-accent)_0_4px,transparent_4px_7px)]',
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
