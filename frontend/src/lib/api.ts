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
  start: string;
  end: string;
  duration_minutes: number;
  kind: Kind;
  groups: string[];
  resource?: Resource;
  title?: string;
  teacher?: string;
  room?: string;
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

async function get<T>(path: string): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${BASE}${path}`, { headers: { accept: 'application/json' } });
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

function cached<T>(key: string, load: () => Promise<T>): Promise<T> {
  const hit = cache.get(key) as Promise<T> | undefined;
  if (hit) return hit;
  const pending = load().catch((error: unknown) => {
    cache.delete(key);
    throw error;
  });
  cache.set(key, pending);
  return pending;
}

export function fetchYears(): Promise<YearSummary[]> {
  return cached('years', () => get<YearSummary[]>('/api/v1/years'));
}

/**
 * Emploi du temps d'une semaine. Sans `group`, la réponse porte tous les
 * groupes : c'est elle qui sert à construire la liste des groupes et des jours.
 */
export function fetchWeek(year: number, week: number, group?: string): Promise<Week> {
  const query = group ? `?group=${encodeURIComponent(group)}` : '';
  return cached(`w:${year}:${week}:${group ?? ''}`, () =>
    get<Week>(`/api/v1/years/${year}/weeks/${week}${query}`),
  );
}

/** Oublie les réponses mémorisées, pour forcer une relecture. */
export function clearCache(): void {
  cache.clear();
}
