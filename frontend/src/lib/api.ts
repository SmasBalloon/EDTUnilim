//! Accès à l'API `edt-unilim-api`.
//
// Les URL sont relatives : en développement Vite proxyfie `/api` vers le
// service Rust, en production le front est servi derrière la même origine.

export type Kind = 'amphi' | 'td' | 'tp' | 'sae' | 'controle';

export interface Resource {
  code: string;
  name?: string;
}

export interface Course {
  day: string;
  date: string;
  /**
   * Année du BUT. L'API ne la porte pas sur un créneau — elle est celle de la
   * semaine demandée : la page des enseignants, qui réunit les trois années
   * dans une même grille, l'attache elle-même à chaque cours.
   */
  year?: number;
  start: string;
  end: string;
  duration_minutes: number;
  kind: Kind;
  groups: string[];
  resource?: Resource;
  title?: string;
  teacher?: string;
  /** Identifiant de l'enseignant, le même quelle que soit l'écriture du PDF. */
  teacher_id?: string;
  room?: string;
}

/** Un enseignant du référentiel de l'API. */
export interface Teacher {
  id: string;
  name: string;
  /** Autres écritures rencontrées dans les PDF : `LD`, `BX`. */
  aliases: string[];
}

export interface Period {
  week_number?: number;
  start: string;
  end: string;
  label: string;
}

export interface Week {
  year: number;
  week: number;
  period: Period;
  source: { url: string; fetched_at: string };
  count: number;
  courses: Course[];
}

export interface YearSummary {
  year: number;
  weeks: number[];
}

/** Une erreur rendue par l'API, avec son code machine (`unknown_week`, …). */
export class ApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
  ) {
    super(message);
  }

  /** Vrai quand la semaine demandée n'est tout simplement pas publiée. */
  get isMissing(): boolean {
    return this.code === 'unknown_week' || this.code === 'unknown_year';
  }
}

const BASE = import.meta.env.VITE_API_URL ?? '';

/**
 * Avec `fresh`, le cache HTTP du navigateur est contourné et la requête porte
 * `Cache-Control: no-cache`, ce qui demande à l'API de revalider auprès du
 * département au lieu de rendre sa copie en mémoire.
 */
async function get<T>(path: string, fresh = false): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      headers: { accept: 'application/json' },
      cache: fresh ? 'reload' : 'default',
    });
  } catch {
    throw new ApiError('offline', 'service injoignable — vérifie ta connexion', 0);
  }

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string; message?: string } | null;
    throw new ApiError(
      body?.error ?? 'http_error',
      body?.message ?? `erreur ${response.status}`,
      response.status,
    );
  }
  return (await response.json()) as T;
}

/** Les requêtes identiques ne partent qu'une fois par session. */
const cache = new Map<string, Promise<unknown>>();

/** Requêtes oubliées par `clearCache`, à relire en contournant tous les caches. */
const stale = new Set<string>();

function cached<T>(key: string, load: (fresh: boolean) => Promise<T>): Promise<T> {
  const hit = cache.get(key) as Promise<T> | undefined;
  if (hit) return hit;
  const pending = load(stale.delete(key)).catch((error: unknown) => {
    cache.delete(key);
    throw error;
  });
  cache.set(key, pending);
  return pending;
}

/** Les enseignants connus. La liste ne change qu'avec l'équipe pédagogique. */
export function fetchTeachers(): Promise<Teacher[]> {
  return cached('teachers', (fresh) => get<Teacher[]>('/api/v1/teachers', fresh));
}

export function fetchYears(): Promise<YearSummary[]> {
  return cached('years', (fresh) => get<YearSummary[]>('/api/v1/years', fresh));
}

/**
 * Emploi du temps d'une semaine. Sans `group`, la réponse porte tous les
 * groupes : c'est elle qui sert à construire la liste des groupes et des jours.
 */
export function fetchWeek(year: number, week: number, group?: string): Promise<Week> {
  const query = group ? `?group=${encodeURIComponent(group)}` : '';
  return cached(`w:${year}:${week}:${group ?? ''}`, (fresh) =>
    get<Week>(`/api/v1/years/${year}/weeks/${week}${query}`, fresh),
  );
}

/** Oublie les réponses mémorisées : leur prochaine lecture repart de la source. */
export function clearCache(): void {
  for (const key of cache.keys()) stale.add(key);
  cache.clear();
}

/** La personne connectée avec Discord. */
export interface Me {
  id: string;
  name: string;
  avatar_url: string | null;
  year: number | null;
  /** Demi-groupe suivi : `G1a`. */
  group: string | null;
  /** Recevoir les emplois du temps et leurs changements en message privé. */
  notify: boolean;
}

/** Issue du message privé de confirmation. */
export type Delivery = 'none' | 'sent' | 'closed' | 'failed';

/** Requête vers les routes du compte : jamais mises en cache. */
async function send<T>(method: string, path: string, body?: unknown): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, {
      method,
      credentials: 'same-origin',
      cache: 'no-store',
      headers: body === undefined ? { accept: 'application/json' } : { accept: 'application/json', 'content-type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError('offline', 'service injoignable — vérifie ta connexion', 0);
  }
  if (!response.ok) {
    const error = (await response.json().catch(() => null)) as { error?: string; message?: string } | null;
    throw new ApiError(error?.error ?? 'http_error', error?.message ?? `erreur ${response.status}`, response.status);
  }
  return (response.status === 204 ? undefined : await response.json()) as T;
}

/** Le compte connecté, ou `null` sans session. */
export async function fetchMe(): Promise<Me | null> {
  try {
    return await send<Me>('GET', '/api/v1/me');
  } catch (error) {
    if (error instanceof ApiError && error.code === 'unauthenticated') return null;
    throw error;
  }
}

export function saveMe(change: Pick<Me, 'year' | 'group' | 'notify'>): Promise<{ user: Me; dm: Delivery }> {
  return send('PUT', '/api/v1/me', change);
}

export function logout(): Promise<void> {
  return send('POST', '/api/v1/auth/logout');
}

export function deleteAccount(): Promise<void> {
  return send('DELETE', '/api/v1/me');
}

/** Page d'autorisation Discord, via l'API qui prépare l'aller-retour. */
export const LOGIN_URL = `${BASE}/api/v1/auth/login`;

/**
 * Adresse de l'abonnement iCalendar d'un demi-groupe, en `https://` : c'est
 * elle qu'on colle dans Google Agenda (« À partir de l'URL »).
 */
export function calendarUrl(year: number, group: string): string {
  return new URL(`${BASE}/api/v1/years/${year}/calendar/${group}.ics`, location.href).href;
}

/** La même en `webcal://` : un clic l'ouvre dans l'agenda du système. */
export function webcalUrl(year: number, group: string): string {
  return calendarUrl(year, group).replace(/^https?:/, 'webcal:');
}
