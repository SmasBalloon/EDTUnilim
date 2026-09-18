//! Ce qui a bougé dans une semaine depuis la dernière fois qu'on l'a regardée.
//
// Le département republie ses PDF sans prévenir : une salle change, un cours
// disparaît, un autre s'ajoute. L'API le dit déjà en message privé Discord
// (`src/notify.rs`) ; ici, la même comparaison se fait dans le navigateur,
// entre ce qu'on a sous les yeux et ce qu'on avait vu la fois précédente.
//
// Les définitions sont celles de `notify.rs`, à dessein : un changement doit se
// lire pareil des deux côtés.

import type { Course } from './api';

export type ChangeKind = 'added' | 'removed' | 'modified';

export interface Change {
  kind: ChangeKind;
  /** Le cours tel qu'il est aujourd'hui — ou tel qu'il était, s'il a disparu. */
  course: Course;
  /** Pour un cours modifié : ce qu'il était avant. */
  before?: Course;
}

/** Tout ce qui décrit un créneau : deux créneaux égaux ici sont identiques. */
function sameCourse(a: Course, b: Course): boolean {
  return (
    sameSlot(a, b) &&
    a.kind === b.kind &&
    a.resource?.code === b.resource?.code &&
    a.title === b.title &&
    a.teacher === b.teacher &&
    a.room === b.room
  );
}

/** Le même créneau pour les mêmes groupes, quel qu'en soit le contenu. */
function sameSlot(a: Course, b: Course): boolean {
  return (
    a.date === b.date &&
    a.start === b.start &&
    a.end === b.end &&
    a.groups.join() === b.groups.join()
  );
}

/** Ce qui identifie un créneau à l'écran, pour retrouver ce qui l'a marqué. */
export function slotKey(course: Course): string {
  return `${course.date} ${course.start} ${course.end} ${course.groups.join()}`;
}

/**
 * Compare deux versions d'une même semaine.
 *
 * Un créneau retiré puis remis au même endroit n'a pas disparu : il a changé
 * de salle, d'enseignant ou de matière. C'est ce rapprochement qui distingue
 * « annulé » de « modifié ».
 */
export function diff(before: Course[], after: Course[]): Change[] {
  const removed = before.filter((old) => !after.some((now) => sameCourse(old, now)));
  const added = after.filter((now) => !before.some((old) => sameCourse(old, now)));

  const changes: Change[] = [];
  const leftover = [...added];
  for (const old of removed) {
    const index = leftover.findIndex((now) => sameSlot(old, now));
    if (index === -1) {
      changes.push({ kind: 'removed', course: old });
    } else {
      const [now] = leftover.splice(index, 1);
      if (now) changes.push({ kind: 'modified', course: now, before: old });
    }
  }
  for (const now of leftover) changes.push({ kind: 'added', course: now });

  return changes.sort(
    (a, b) =>
      a.course.date.localeCompare(b.course.date) || a.course.start.localeCompare(b.course.start),
  );
}

/** Ce qui a changé entre deux versions d'un même créneau. */
export function differences(before: Course, after: Course): string[] {
  const fields: [string, string | undefined, string | undefined][] = [
    ['matière', before.resource?.code ?? before.title, after.resource?.code ?? after.title],
    ['enseignant', before.teacher, after.teacher],
    ['salle', before.room, after.room],
    ['nature', before.kind, after.kind],
  ];
  return fields
    .filter(([, was, now]) => was !== now)
    .map(([what, was, now]) => `${what} : ${was ?? '—'} → ${now ?? '—'}`);
}

// --- Mémoire de ce qui a été vu --------------------------------------------

const KEY = 'edt-unilim/seen';
/** Au-delà, les semaines les plus anciennes sont oubliées. */
const KEPT = 8;

interface Snapshot {
  /** Quand cette version a été regardée. */
  at: string;
  courses: Course[];
}

type Memory = Record<string, Snapshot>;

/** La semaine d'un groupe : c'est à ce grain que se compare le contenu. */
export function weekKey(year: number, week: number, group: string): string {
  return `${year}:${week}:${group}`;
}

function read(): Memory {
  try {
    const stored = localStorage.getItem(KEY);
    return stored ? (JSON.parse(stored) as Memory) : {};
  } catch {
    // Navigation privée ou stockage refusé : on ne se souviendra de rien, ce
    // qui revient à ne jamais annoncer de changement.
    return {};
  }
}

/** La version gardée en mémoire, si cette semaine a déjà été regardée. */
export function recall(key: string): Snapshot | null {
  return read()[key] ?? null;
}

/** Retient la semaine telle qu'elle est maintenant : elle devient la référence. */
export function remember(key: string, courses: Course[]): void {
  const memory = read();
  memory[key] = { at: new Date().toISOString(), courses };

  // On ne garde que les dernières semaines consultées : la mémoire sert à
  // repérer un changement, pas à archiver l'année.
  const kept = Object.entries(memory)
    .sort(([, a], [, b]) => b.at.localeCompare(a.at))
    .slice(0, KEPT);

  try {
    localStorage.setItem(KEY, JSON.stringify(Object.fromEntries(kept)));
  } catch {
    // Stockage plein ou refusé : tant pis, rien ne sera signalé.
  }
}
