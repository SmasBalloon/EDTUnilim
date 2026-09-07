//! Export de la semaine affichée au format iCalendar.

import type { Course, Week } from './api';
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

/** Repli les lignes à 75 octets, comme l'exige la RFC 5545. */
function fold(line: string): string {
  const chunks: string[] = [];
  let rest = line;
  while (rest.length > 74) {
    chunks.push(rest.slice(0, 74));
    rest = rest.slice(74);
  }
  chunks.push(rest);
  return chunks.join('\r\n ');
}

function summary(course: Course): string {
  const label = course.title ?? course.resource?.name ?? course.resource?.code ?? 'Cours';
  const code = course.resource?.code;
  return code && !label.startsWith(code) ? `${code} · ${label}` : label;
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
  const blob = new Blob([buildIcs(week, courses, group)], { type: 'text/calendar;charset=utf-8' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = `edt-A${week.year}-S${week.week}${group ? `-${group}` : ''}.ics`;
  document.body.append(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}
