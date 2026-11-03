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

// --- Échelle de la grille horaire (INTEGRATION.md § 5.1) ----------------------

/** 56 px par heure, comme la maquette. */
export const HOUR_PX = 56;
/** La grille va de 08:00 à 20:00, élargie si un cours en déborde. */
const SCALE_START = 8 * 60;
const SCALE_END = 20 * 60;

export interface Scale {
  /** Bornes en minutes, calées sur l'heure : les filets tombent juste. */
  start: number;
  end: number;
  /** Hauteur de la colonne, en pixels. */
  height: number;
  /** Les heures pleines à graduer : `08:00`, `09:00`, … */
  hours: string[];
  /** Ordonnée d'un instant de la journée, en pixels. */
  offset: (minutes: number) => number;
}

export function gridScale(courses: Course[]): Scale {
  const start = courses.reduce((min, course) => Math.min(min, toMinutes(course.start)), SCALE_START);
  const end = courses.reduce((max, course) => Math.max(max, toMinutes(course.end)), SCALE_END);
  const from = Math.floor(start / 60) * 60;
  const to = Math.ceil(end / 60) * 60;
  const hours: string[] = [];
  for (let m = from; m < to; m += 60) hours.push(`${`${m / 60}`.padStart(2, '0')}:00`);
  return {
    start: from,
    end: to,
    height: ((to - from) / 60) * HOUR_PX,
    hours,
    offset: (minutes) => ((minutes - from) / 60) * HOUR_PX,
  };
}

/**
 * Position d'un cours dans sa colonne : `top = début × 56 + 2`,
 * `height = durée × 56 − 4`. Deux cours qui se chevauchent se partagent la
 * largeur, chacun dans son couloir.
 */
export function blockStyle(placed: Placed, scale: Scale): Record<string, string> {
  const start = toMinutes(placed.course.start);
  const end = toMinutes(placed.course.end);
  const style: Record<string, string> = {
    top: `${scale.offset(start) + 2}px`,
    height: `${Math.max(((end - start) / 60) * HOUR_PX - 4, 20)}px`,
  };
  if (placed.lanes > 1) {
    style.left = `calc(${(placed.lane / placed.lanes) * 100}% + 3px)`;
    style.right = 'auto';
    style.width = `calc(${100 / placed.lanes}% - 6px)`;
  }
  return style;
}

/** Créneau d'une heure ou moins : la carte se resserre (titre, tag, horaire). */
export const isShort = (course: Course): boolean => toMinutes(course.end) - toMinutes(course.start) <= 60;
