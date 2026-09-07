//! Groupes et jours réellement présents dans un emploi du temps.
//
// L'API n'expose pas de liste de groupes : ils n'apparaissent que dans les
// créneaux. On les relève donc sur la semaine complète (requête sans filtre).

import type { Week } from './api';
import { DAY_ORDER } from './time';

/** Les deux moitiés d'un groupe, telles que le département les note. */
export const HALVES = ['a', 'b'] as const;

const NUMBER = /\d+/;

function rank(group: string): number {
  return Number(NUMBER.exec(group)?.[0] ?? 0);
}

/** Vrai pour `G1a`, faux pour `G1`. */
function isHalf(group: string): boolean {
  return group.length > 2 && /[a-z]$/i.test(group);
}

/** Le groupe principal auquel appartient un libellé : `G1a` → `G1`. */
export function baseOf(group: string): string {
  return isHalf(group) ? group.slice(0, -1) : group;
}

/** La moitié désignée par un libellé : `G1a` → `a`, `G1` → `null`. */
export function halfOf(group: string): string | null {
  return isHalf(group) ? group.slice(-1).toLowerCase() : null;
}

/** Ramène un groupe principal à sa première moitié : `G1` → `G1a`. */
export function toHalf(group: string): string {
  return isHalf(group) ? group : `${group}${HALVES[0]}`;
}

/**
 * Groupes principaux trouvés dans la semaine, dans l'ordre des numéros.
 *
 * Les deux moitiés sont ensuite toujours proposées, même si l'une n'a aucun TP
 * cette semaine-là : la liste reste la même d'une semaine à l'autre.
 */
export function groupBases(week: Week): string[] {
  const bases = new Set<string>();
  for (const course of week.courses) {
    for (const group of course.groups) bases.add(baseOf(group));
  }
  return [...bases].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
}

/** Étiquette affichée pour un groupe : `G1a` s'écrit `G1A`. */
export function groupLabel(group: string): string {
  return group.toUpperCase();
}

/** Jours occupés dans la semaine, dans l'ordre du calendrier. */
export function weekDays(week: Week): { name: string; date: string }[] {
  const days = new Map<string, string>();
  for (const course of week.courses) {
    if (!days.has(course.day)) days.set(course.day, course.date);
  }
  return [...days.entries()]
    .map(([name, date]) => ({ name, date }))
    .sort((a, b) => DAY_ORDER.indexOf(a.name) - DAY_ORDER.indexOf(b.name));
}
