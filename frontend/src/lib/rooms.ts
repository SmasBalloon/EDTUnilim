//! Occupation des salles, reconstituée à partir des cours de toutes les années.
//
// L'API ne publie pas de liste de salles : on relève celles qui apparaissent
// dans les créneaux de la semaine, toutes années confondues.

import type { Course, YearSummary } from './api';
import { ApiError, fetchWeek } from './api';
import { parseDate, resolveCurrentWeek, startOfDay, toMinutes } from './time';

/**
 * Salles désignées par un libellé du PDF. Deux salles voisines réservées
 * ensemble s'écrivent en abrégé : `108-9` pour 108 et 109, `111-2` pour 111
 * et 112.
 */
export function expandRooms(label: string): string[] {
  const match = /^(.+?)-(\w+)$/.exec(label);
  if (!match) return [label];
  const [, first = '', suffix = ''] = match;
  if (suffix.length >= first.length) return [first, suffix];
  return [first, first.slice(0, first.length - suffix.length) + suffix];
}

/** Ordre naturel : `103` avant `R46`, `R46` avant `R50`. */
const COLLATOR = new Intl.Collator('fr', { numeric: true });

export interface Interval {
  start: number;
  end: number;
  courses: Course[];
}

export interface RoomDay {
  room: string;
  /** Plages occupées de la journée, fusionnées quand les cours s'enchaînent. */
  busy: Interval[];
  /** Les cours de la journée dans cette salle, dans l'ordre. */
  courses: Course[];
}

/** Occupation de chaque salle connue pour une journée donnée. */
export function roomDays(weekCourses: Course[], date: string): RoomDay[] {
  const rooms = new Map<string, Course[]>();
  for (const course of weekCourses) {
    if (!course.room) continue;
    for (const room of expandRooms(course.room)) {
      const list = rooms.get(room) ?? [];
      if (course.date === date) list.push(course);
      rooms.set(room, list);
    }
  }

  return [...rooms.entries()]
    .sort(([a], [b]) => floorOf(a).rank - floorOf(b).rank || COLLATOR.compare(a, b))
    .map(([room, courses]) => {
      const sorted = [...courses].sort((a, b) => toMinutes(a.start) - toMinutes(b.start));
      const busy: Interval[] = [];
      for (const course of sorted) {
        const start = toMinutes(course.start);
        const end = toMinutes(course.end);
        const last = busy[busy.length - 1];
        if (last && start <= last.end) {
          last.end = Math.max(last.end, end);
          last.courses.push(course);
        } else {
          busy.push({ start, end, courses: [course] });
        }
      }
      return { room, busy, courses: sorted };
    });
}

export type RoomState =
  | { free: true; room: RoomDay; until: number | null }
  | { free: false; room: RoomDay; until: number; courses: Course[] };

/**
 * Libre ou occupée sur une plage de la journée, `[start, end[` ; sans `end`,
 * à l'instant `start`. Libre veut dire libre sur toute la plage.
 */
export function roomState(room: RoomDay, start: number, end: number = start + 1): RoomState {
  const courses = room.courses.filter(
    (course) => toMinutes(course.start) < end && toMinutes(course.end) > start,
  );
  if (courses.length === 0) {
    const next = room.busy.find((slot) => slot.start >= end);
    return { free: true, room, until: next?.start ?? null };
  }
  const last = room.busy.filter((slot) => slot.start < end && slot.end > start).at(-1);
  return { free: false, room, until: last?.end ?? end, courses };
}

export interface Floor {
  /** `R`, `100`, `200`… */
  name: string;
  label: string;
}

/** Étage d'une salle : `R46` au rez-de-chaussée, `103` au premier, `208` au deuxième. */
function floorOf(room: string): { rank: number; name: string; label: string } {
  if (/^R\d/i.test(room)) return { rank: 0, name: 'R', label: 'Rez-de-chaussée' };
  const level = /^(\d)\d\d/.exec(room)?.[1];
  if (level !== undefined) {
    const n = Number(level);
    return { rank: n, name: `${n}00`, label: n === 1 ? '1er étage' : `${n}e étage` };
  }
  return { rank: 99, name: 'Autres', label: 'Hors étages' };
}

/** Salles réparties par étage, du rez-de-chaussée vers le haut. */
export function byFloor<T>(items: T[], room: (item: T) => string): { floor: Floor; items: T[] }[] {
  const floors = new Map<string, { rank: number; floor: Floor; items: T[] }>();
  for (const item of items) {
    const { rank, name, label } = floorOf(room(item));
    const entry = floors.get(name) ?? { rank, floor: { name, label }, items: [] };
    entry.items.push(item);
    floors.set(name, entry);
  }
  return [...floors.values()].sort((a, b) => a.rank - b.rank).map(({ floor, items: list }) => ({ floor, items: list }));
}

/** `14:05` à partir de minutes depuis minuit. */
export function formatMinutes(minutes: number): string {
  return `${Math.floor(minutes / 60)}`.padStart(2, '0') + ':' + `${minutes % 60}`.padStart(2, '0');
}

export interface RoomsData {
  /** Cours de toutes les années sur la semaine contenant la date. */
  courses: Course[];
  /** Faux quand aucune semaine publiée ne couvre la date. */
  covered: boolean;
}

/** Charge, pour chaque année, la semaine qui contient la date demandée. */
export async function loadRooms(years: YearSummary[], date: string): Promise<RoomsData> {
  const day = parseDate(date);
  const weeks = await Promise.all(
    years
      .filter((summary) => summary.weeks.length > 0)
      .map(async (summary) => {
        try {
          const number = await resolveCurrentWeek(summary.year, summary, day);
          const week = await fetchWeek(summary.year, number);
          const covers = date >= week.period.start && date <= week.period.end;
          return covers ? week : null;
        } catch (error) {
          if (error instanceof ApiError && error.isMissing) return null;
          throw error;
        }
      }),
  );

  const found = weeks.filter((week) => week !== null);
  return { courses: found.flatMap((week) => week.courses), covered: found.length > 0 };
}

/** Date locale combinée à une heure `HH:MM`. */
export function atTime(date: string, time: string): Date {
  const day = startOfDay(parseDate(date));
  day.setMinutes(toMinutes(time));
  return day;
}
