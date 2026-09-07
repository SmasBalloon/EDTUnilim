//! Export de la semaine affichée au format iCalendar.

import type { Course, Week } from './api';
import { KIND_LABEL } from './layout';
import { toMinutes } from './time';

function stamp(date: string, clock: string): string {
  const [year = '1970', month = '1', day = '1'] = date.split('-');
  const minutes = toMinutes(clock);
  // Interprété dans le fuseau de l'appareil, puis écrit en UTC : les agendas
  // replacent alors le cours à la bonne heure où qu'ils soient lus.
  const local = new Date(Number(year), Number(month) - 1, Number(day), 0, minutes);
  return `${local.toISOString().replace(/[-:]/g, '').slice(0, 15)}Z`;
}

function escape(text: string): string {
  return text.replace(/([,;\\])/g, '\\$1').replace(/\n/g, '\\n');
}

/**
 * Replie les lignes à 75 octets, comme l'exige la RFC 5545.
 *
 * La limite porte sur les octets UTF-8, pas sur les caractères : une ligne
 * d'accents pèse le double, et couper au milieu d'un code point produirait le
 * « ? » que les agendas affichent à la place de la lettre.
 */
const encoder = new TextEncoder();

function fold(line: string): string {
  const chunks: string[] = [];
  let current = '';
  let bytes = 0;
  for (const char of line) {
    const width = encoder.encode(char).length;
    // 74 octets utiles : les lignes suivantes sont préfixées d'une espace.
    if (bytes + width > 74) {
      chunks.push(current);
      current = '';
      bytes = 0;
    }
    current += char;
    bytes += width;
  }
  chunks.push(current);
  return chunks.join('\r\n ');
}

/** `R3.01 - Développement web / TD / Monédière T.`, les parties absentes en moins. */
function summary(course: Course): string {
  const code = course.resource?.code;
  const label = course.resource?.name ?? course.title;
  const resource = [code, label].filter(Boolean).join(' - ') || 'Cours';
  return [resource, KIND_LABEL[course.kind], course.teacher].filter(Boolean).join(' / ');
}

function event(course: Course, index: number, week: Week): string[] {
  const description = [
    course.resource?.code && `Ressource : ${course.resource.code}`,
    course.resource?.name && `Intitulé : ${course.resource.name}`,
    `Type : ${course.kind}`,
    `Groupes : ${course.groups.join(', ')}`,
    course.teacher && `Enseignant : ${course.teacher}`,
  ]
    .filter(Boolean)
    .join('\n');

  return [
    'BEGIN:VEVENT',
    fold(`UID:${week.year}-${week.week}-${index}-${course.date}-${course.start.replace(':', '')}@edt-unilim`),
    `DTSTAMP:${new Date().toISOString().replace(/[-:]/g, '').slice(0, 15)}Z`,
    `DTSTART:${stamp(course.date, course.start)}`,
    `DTEND:${stamp(course.date, course.end)}`,
    fold(`SUMMARY:${escape(summary(course))}`),
    course.room ? fold(`LOCATION:${escape(course.room)}`) : '',
    fold(`DESCRIPTION:${escape(description)}`),
    'END:VEVENT',
  ].filter(Boolean);
}

export function buildIcs(week: Week, courses: Course[], group: string | null): string {
  const lines = [
    'BEGIN:VCALENDAR',
    'VERSION:2.0',
    'PRODID:-//edt-unilim//fr',
    'CALSCALE:GREGORIAN',
    'METHOD:PUBLISH',
    fold(`X-WR-CALNAME:EDT A${week.year} S${week.week}${group ? ` ${group.toUpperCase()}` : ''}`),
    ...courses.flatMap((course, index) => event(course, index, week)),
    'END:VCALENDAR',
  ];
  return `${lines.join('\r\n')}\r\n`;
}

/** Déclenche le téléchargement du fichier .ics de la semaine affichée. */
export function downloadIcs(week: Week, courses: Course[], group: string | null): void {
  // BOM en tête : sans lui, Outlook et quelques agendas de bureau lisent le
  // fichier en Latin-1 et rendent chaque accent par un « ? ».
  const blob = new Blob(['\uFEFF', buildIcs(week, courses, group)], {
    type: 'text/calendar;charset=utf-8',
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `edt-A${week.year}-S${week.week}${group ? `-${group}` : ''}.ics`;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
