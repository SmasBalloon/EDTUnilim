//! Placement des créneaux d'une journée dans la grille.

import type { Course, Kind } from './api';
import { toMinutes } from './time';

/** Amplitude de la grille : 8 h – 19 h 30, quelle que soit la semaine. */
export const DAY_START = 8 * 60;
export const DAY_END = 19 * 60 + 30;

export const KIND_LABEL: Record<Kind, string> = {
  amphi: 'CM',
  td: 'TD',
  tp: 'TP',
  sae: 'SAÉ',
  controle: 'Contrôle',
};

export interface Placed {
  course: Course;
  /** Colonne occupée quand deux créneaux se chevauchent. */
  lane: number;
  lanes: number;
}

/**
 * Répartit les créneaux d'une journée en couloirs : deux cours qui se
 * chevauchent (cas des demi-groupes affichés ensemble) se partagent la largeur
 * de la colonne au lieu de se recouvrir.
 */
export function packDay(courses: Course[]): Placed[] {
  const sorted = [...courses].sort(
    (a, b) => toMinutes(a.start) - toMinutes(b.start) || toMinutes(b.end) - toMinutes(a.end),
  );

  const placed: Placed[] = [];
  let cluster: Placed[] = [];
  let clusterEnd = -1;

  const close = () => {
    const lanes = cluster.reduce((max, item) => Math.max(max, item.lane + 1), 0);
    for (const item of cluster) item.lanes = lanes;
    placed.push(...cluster);
    cluster = [];
    clusterEnd = -1;
  };

  for (const course of sorted) {
    const start = toMinutes(course.start);
    if (start >= clusterEnd && cluster.length > 0) close();

    const busy = new Set(
      cluster.filter((item) => toMinutes(item.course.end) > start).map((item) => item.lane),
    );
    let lane = 0;
    while (busy.has(lane)) lane += 1;

    cluster.push({ course, lane, lanes: 1 });
    clusterEnd = Math.max(clusterEnd, toMinutes(course.end));
  }
  if (cluster.length > 0) close();

  return placed;
}

/** Bornes réellement utilisées, pour ne pas rogner un cours hors amplitude. */
export function gridBounds(courses: Course[]): { start: number; end: number } {
  const start = courses.reduce((min, course) => Math.min(min, toMinutes(course.start)), DAY_START);
  const end = courses.reduce((max, course) => Math.max(max, toMinutes(course.end)), DAY_END);
  return { start: Math.floor(start / 30) * 30, end: Math.ceil(end / 30) * 30 };
}
