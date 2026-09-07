//! Conversions de dates et d'horaires, et repérage de la semaine courante.

import { createSignal } from 'solid-js';

import type { Course, Week, YearSummary } from './api';
import { fetchWeek } from './api';

/** Horloge partagée : l'heure courante, rafraîchie toutes les 30 secondes. */
const [clock, setClock] = createSignal(new Date());
setInterval(() => setClock(new Date()), 30_000);
export { clock };

/** Ordre d'affichage des colonnes. */
export const DAY_ORDER = ['LUNDI', 'MARDI', 'MERCREDI', 'JEUDI', 'VENDREDI', 'SAMEDI'];

/** Minutes depuis minuit à partir d'un `HH:MM`. */
export function toMinutes(clock: string): number {
  const [hours = '0', minutes = '0'] = clock.split(':');
  return Number(hours) * 60 + Number(minutes);
}

/** `2026-09-14` → `Date` locale à minuit (pas d'UTC, pour éviter les décalages). */
export function parseDate(iso: string): Date {
  const [year = '1970', month = '1', day = '1'] = iso.split('-');
  return new Date(Number(year), Number(month) - 1, Number(day));
}

export function startOfDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function todayIso(now: Date = new Date()): string {
  const month = `${now.getMonth() + 1}`.padStart(2, '0');
  const day = `${now.getDate()}`.padStart(2, '0');
  return `${now.getFullYear()}-${month}-${day}`;
}

/** Minutes écoulées depuis minuit. */
export function nowMinutes(now: Date = new Date()): number {
  return now.getHours() * 60 + now.getMinutes();
}

const DAY_LABEL = new Intl.DateTimeFormat('fr-FR', { weekday: 'long' });
const SHORT = new Intl.DateTimeFormat('fr-FR', { day: 'numeric', month: 'short' });
const LONG = new Intl.DateTimeFormat('fr-FR', { weekday: 'long', day: 'numeric', month: 'long' });

export function dayLabel(iso: string): string {
  return DAY_LABEL.format(parseDate(iso));
}

export function shortDate(iso: string): string {
  return SHORT.format(parseDate(iso));
}

export function longDate(iso: string): string {
  return LONG.format(parseDate(iso));
}

/** `14 – 19 sept.` */
export function periodLabel(week: Week): string {
  return `${SHORT.format(parseDate(week.period.start))} – ${SHORT.format(parseDate(week.period.end))}`;
}

/** `Du 7 sept. au 12 sept. 2026` */
export function periodSentence(week: Week): string {
  const start = parseDate(week.period.start);
  const end = parseDate(week.period.end);
  return `Du ${SHORT.format(start)} au ${SHORT.format(end)} ${end.getFullYear()}`;
}

/** Le créneau se déroule-t-il en ce moment ? */
export function isRunning(course: Course, iso: string, minutes: number): boolean {
  return course.date === iso && toMinutes(course.start) <= minutes && toMinutes(course.end) > minutes;
}

/** La semaine publiée la plus proche du numéro visé. */
function nearest(weeks: number[], target: number): number {
  return weeks.reduce((best, week) =>
    Math.abs(week - target) < Math.abs(best - target) ? week : best,
  );
}

/**
 * Trouve la semaine qui contient aujourd'hui.
 *
 * Les numéros de semaine sont ceux du département (`S1`, `S2`, …), sans lien
 * fixe avec le calendrier : on lit donc la période d'une semaine publiée, on
 * mesure l'écart en semaines avec aujourd'hui, et on saute d'autant. Deux
 * lectures suffisent en pratique.
 */
export async function resolveCurrentWeek(year: number, summary: YearSummary): Promise<number> {
  const weeks = summary.weeks;
  const first = weeks[0];
  if (first === undefined) throw new Error('aucune semaine publiée');

  let candidate = weeks[weeks.length - 1] ?? first;
  const today = startOfDay(new Date()).getTime();

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const week = await fetchWeek(year, candidate);
    const start = parseDate(week.period.start).getTime();
    const end = parseDate(week.period.end).getTime();
    if (today >= start && today <= end) return candidate;

    const offset = Math.round((today - start) / (7 * 24 * 3600 * 1000));
    const next = nearest(weeks, candidate + offset);
    if (next === candidate) return candidate;
    candidate = next;
  }
  return candidate;
}

/** `1h30`, `2h`, `45min` — durée écrite comme on la dit. */
export function humanDuration(minutes: number): string {
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  if (hours === 0) return `${rest}min`;
  return rest === 0 ? `${hours}h` : `${hours}h${`${rest}`.padStart(2, '0')}`;
}

/** `Lundi 07/09`, précédé de sa position dans la semaine quand elle parle. */
export function relativeDayLabel(iso: string, now: Date = new Date()): string {
  const date = parseDate(iso);
  const days = Math.round((date.getTime() - startOfDay(now).getTime()) / (24 * 3600 * 1000));
  const name = DAY_LABEL.format(date);
  const stamp = `${`${date.getDate()}`.padStart(2, '0')}/${`${date.getMonth() + 1}`.padStart(2, '0')}`;
  const capitalised = name.charAt(0).toUpperCase() + name.slice(1);
  const prefix = ['Aujourd’hui', 'Demain', 'Après-demain'][days];
  return prefix ? `${prefix} : ${capitalised} ${stamp}` : `${capitalised} ${stamp}`;
}

/** Le salut qui convient à l'heure qu'il est. */
export function greeting(now: Date = new Date()): string {
  const hour = now.getHours();
  if (hour < 12) return 'Bonjour';
  return hour < 18 ? 'Bon après-midi' : 'Bonsoir';
}
