//! Les devoirs : ce qu'il y a à faire, pour soi ou pour sa classe.
//
// La liste est partagée par toute l'application — la page des devoirs, le
// détail d'un cours, les pastilles de la grille — donc elle vit ici, lue une
// fois et tenue à jour à chaque écriture.

import { createSignal } from 'solid-js';

import { ApiError, type Course } from './api';

const BASE = import.meta.env.VITE_API_URL ?? '';

/** Qui voit un devoir. */
export type Audience = { kind: 'personal' } | { kind: 'class'; group?: string };

/** Le créneau auquel un devoir est rattaché. */
export interface CourseRef {
  year: number;
  week: number;
  date: string;
  start: string;
}

export interface Homework {
  id: string;
  author: string;
  author_name: string;
  year: number;
  audience: Audience;
  title: string;
  details?: string;
  resource?: string;
  due?: string;
  course?: CourseRef;
  created_at: string;
  confirmed_by?: string;
  confirmed_name?: string;
  done?: string[];
  /** En attente d'une confirmation par quelqu'un d'autre. */
  pending: boolean;
  /** Coché « fait » par la personne connectée. */
  done_by_me: boolean;
  /** Elle l'a noté elle-même. */
  mine: boolean;
  can_confirm: boolean;
}

/** Ce qu'on envoie pour noter ou corriger un devoir. */
export interface Draft {
  title: string;
  details?: string | null;
  resource?: string | null;
  due?: string | null;
  audience: Audience;
  course?: CourseRef | null;
}

const [items, setItems] = createSignal<Homework[]>([]);
const [loading, setLoading] = createSignal(false);
/** `null` tant qu'on n'a pas essayé de lire la liste. */
const [failure, setFailure] = createSignal<ApiError | null>(null);

export { items as homework, loading as homeworkLoading, failure as homeworkError };

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      method,
      credentials: 'same-origin',
      cache: 'no-store',
      headers:
        body === undefined
          ? { accept: 'application/json' }
          : { accept: 'application/json', 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError('offline', 'service injoignable — vérifie ta connexion', 0);
  }
  if (!response.ok) {
    const error = (await response.json().catch(() => null)) as { error?: string; message?: string } | null;
    throw new ApiError(
      error?.error ?? 'http_error',
      error?.message ?? `erreur ${response.status}`,
      response.status,
    );
  }
  return (response.status === 204 ? undefined : await response.json()) as T;
}

/** Relit la liste. Sans session, elle reste vide — et c'est normal. */
export async function loadHomework(): Promise<void> {
  setLoading(true);
  try {
    setItems(await call<Homework[]>('GET', '/api/v1/homework'));
    setFailure(null);
  } catch (error) {
    setItems([]);
    setFailure(error instanceof ApiError ? error : null);
  } finally {
    setLoading(false);
  }
}

/** Remplace un devoir dans la liste, ou l'y ajoute. */
function keep(item: Homework): Homework {
  setItems((current) => {
    const rest = current.filter((other) => other.id !== item.id);
    return [...rest, item].sort(order);
  });
  return item;
}

/** Ce qui est dû en premier passe devant ; sans date, on attend son tour. */
function order(a: Homework, b: Homework): number {
  if (a.due && b.due && a.due !== b.due) return a.due.localeCompare(b.due);
  if (a.due !== b.due) return a.due ? -1 : 1;
  return a.created_at.localeCompare(b.created_at);
}

export function createHomework(draft: Draft): Promise<Homework> {
  return call<Homework>('POST', '/api/v1/homework', draft).then(keep);
}

export function editHomework(id: string, draft: Draft): Promise<Homework> {
  return call<Homework>('PUT', `/api/v1/homework/${id}`, draft).then(keep);
}

export function confirmHomework(id: string): Promise<Homework> {
  return call<Homework>('POST', `/api/v1/homework/${id}/confirm`).then(keep);
}

export function setDone(id: string, done: boolean): Promise<Homework> {
  return call<Homework>('PUT', `/api/v1/homework/${id}/done`, { done }).then(keep);
}

export async function deleteHomework(id: string): Promise<void> {
  await call<void>('DELETE', `/api/v1/homework/${id}`);
  setItems((current) => current.filter((item) => item.id !== id));
}

// --- Rattachement à un cours ------------------------------------------------

/** Le créneau que désigne un cours de l'emploi du temps. */
export function courseRef(course: Course, year: number, week: number): CourseRef {
  return { year, week, date: course.date, start: course.start };
}

/** Ce devoir parle-t-il de ce créneau précis ? */
export function onCourse(item: Homework, course: Course): boolean {
  return item.course?.date === course.date && item.course?.start === course.start;
}

/**
 * Les devoirs qui concernent un cours : ceux qu'on y a rattachés, et ceux de
 * la même matière qui n'ont pas encore trouvé leur créneau.
 */
export function about(course: Course): Homework[] {
  const code = course.resource?.code;
  return items().filter(
    (item) => onCourse(item, course) || (!item.course && code !== undefined && item.resource === code),
  );
}

/** La portée qui va de soi pour un cours : un TP ne concerne qu'un demi-groupe. */
export function audienceFor(course: Course, group: string | null): Audience {
  if (course.kind === 'tp') return { kind: 'class', group: group ?? undefined };
  if (course.kind === 'td') return { kind: 'class', group: base(group) };
  return { kind: 'class' };
}

/** `G1a` → `G1` */
export function base(group: string | null): string | undefined {
  if (!group) return undefined;
  return /[a-z]$/i.test(group) && group.length > 2 ? group.slice(0, -1) : group;
}

/** « pour toi », « pour G1A », « pour tout le BUT 1 ». */
export function audienceLabel(item: Homework): string {
  if (item.audience.kind === 'personal') return 'pour toi';
  const group = item.audience.group;
  return group ? `pour ${group.toUpperCase()}` : `pour tout le BUT ${item.year}`;
}
