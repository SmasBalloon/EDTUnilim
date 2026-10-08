//! Analyse de PDF déposés : les salles réservées par plusieurs cours en même
//! temps, toutes années confondues.
//!
//! Rien n'est gardé : les PDF sont lus, comparés, puis oubliés. On peut donc y
//! déposer une version que le département n'a pas encore publiée.

use axum::extract::multipart::{Multipart, MultipartError, MultipartRejection};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use time::Date;

use crate::config::YEARS;
use crate::conflicts;
use crate::error::ApiError;
use crate::model::{Course, Period, Timetable};
use crate::pdf;

/// Taille maximale d'une analyse : un PDF du département pèse une quinzaine de
/// Ko, et nginx refuse de toute façon au-delà d'un Mo.
pub const MAX_BODY: usize = 1024 * 1024;

/// Ce que l'analyse a lu, et ce qu'elle a trouvé.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Les semaines lues, par année.
    weeks: Vec<WeekRead>,
    /// Vrai quand toutes les semaines se recoupent : sinon, rien ne pouvait
    /// entrer en conflit, et c'est sans doute une erreur de fichier.
    same_dates: bool,
    /// Une entrée par salle disputée, dans l'ordre chronologique.
    conflicts: Vec<RoomConflict>,
}

#[derive(Debug, Serialize)]
struct WeekRead {
    file: String,
    year: u8,
    period: Period,
    courses: usize,
}

#[derive(Debug, Serialize)]
struct RoomConflict {
    room: String,
    #[serde(with = "crate::model::iso_date")]
    date: Date,
    day: String,
    /// Les cours qui se disputent la salle, dans l'ordre de leurs horaires.
    bookings: Vec<BookingView>,
}

#[derive(Debug, Serialize)]
struct BookingView {
    year: u8,
    course: Course,
}

/// Un PDF déposé, avec l'année qu'on lui a donnée.
struct Upload {
    file: String,
    year: u8,
    bytes: Vec<u8>,
}

/// `POST /api/v1/analysis` : un champ par PDF, nommé d'après son année (`1`, `2`, `3`).
pub async fn analyse(multipart: Result<Multipart, MultipartRejection>) -> Result<Json<Report>, ApiError> {
    let mut multipart = multipart.map_err(|rejection| ApiError::BadRequest(rejection.body_text()))?;
    let uploads = read(&mut multipart).await?;
    let weeks = uploads.into_iter().map(parse).collect::<Result<Vec<_>, _>>()?;
    Ok(Json(report(weeks)))
}

/// Les PDF du formulaire : un par année au plus, trois au plus.
async fn read(multipart: &mut Multipart) -> Result<Vec<Upload>, ApiError> {
    let mut uploads: Vec<Upload> = Vec::new();
    while let Some(field) = multipart.next_field().await.map_err(rejected)? {
        let name = field.name().unwrap_or_default().to_owned();
        let year = name
            .parse::<u8>()
            .ok()
            .filter(|year| YEARS.contains(year))
            .ok_or_else(|| ApiError::BadRequest(format!("année inconnue : « {name} » (1, 2 ou 3)")))?;
        // Deux versions d'une même année se disputeraient toutes leurs salles.
        if uploads.iter().any(|upload| upload.year == year) {
            return Err(ApiError::BadRequest(format!("BUT {year} : un seul PDF par année")));
        }
        let file = field.file_name().unwrap_or("PDF sans nom").to_owned();
        let bytes = field.bytes().await.map_err(rejected)?.to_vec();
        uploads.push(Upload { file, year, bytes });
    }
    if uploads.is_empty() {
        return Err(ApiError::BadRequest("dépose au moins un PDF".into()));
    }
    Ok(uploads)
}

/// Un formulaire trop lourd ou mal formé.
fn rejected(error: MultipartError) -> ApiError {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::TooLarge { max_kb: MAX_BODY / 1024 }
    } else {
        ApiError::BadRequest(format!("formulaire illisible : {}", error.body_text()))
    }
}

fn parse(upload: Upload) -> Result<(String, Timetable), ApiError> {
    // Le numéro de semaine du dépôt n'est pas connu : l'en-tête du PDF le donne
    // (`period.week_number`).
    match pdf::parse(&upload.bytes, upload.year, 0) {
        Ok(timetable) => Ok((upload.file, timetable)),
        Err(source) => Err(ApiError::UnreadableUpload { file: upload.file, source }),
    }
}

/// Les salles disputées entre ces semaines, avec ce qui a été lu.
fn report(mut weeks: Vec<(String, Timetable)>) -> Report {
    weeks.sort_by_key(|(_, timetable)| timetable.year);
    let timetables: Vec<&Timetable> = weeks.iter().map(|(_, timetable)| timetable).collect();
    let same_dates = timetables
        .iter()
        .all(|a| timetables.iter().all(|b| a.period.start <= b.period.end && b.period.start <= a.period.end));
    let found = conflicts::find(&timetables);
    let conflicts = conflicts::group(&found.iter().collect::<Vec<_>>())
        .into_iter()
        .map(|overlap| {
            let first = overlap.first().course;
            RoomConflict {
                room: overlap.room.clone(),
                date: first.date,
                day: first.day.clone(),
                bookings: overlap
                    .bookings
                    .iter()
                    .map(|booking| BookingView { year: booking.year, course: booking.course.clone() })
                    .collect(),
            }
        })
        .collect();

    let weeks = weeks
        .into_iter()
        .map(|(file, timetable)| WeekRead {
            file,
            year: timetable.year,
            courses: timetable.courses.len(),
            period: timetable.period,
        })
        .collect();
    Report { weeks, same_dates, conflicts }
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::model::{ClockTime, CourseKind, Resource};

    /// Un TP en salle `room`, de `start` à `start + 120` minutes, daté par [`week`].
    fn course(start: i64, room: &str, teacher: &str) -> Course {
        Course {
            day: "LUNDI".into(),
            date: date!(2026 - 10 - 19),
            start: ClockTime::from_minutes(start),
            end: ClockTime::from_minutes(start + 120),
            duration_minutes: 120,
            kind: CourseKind::Tp,
            groups: vec!["G1A".into()],
            resource: Some(Resource { code: format!("R.{teacher}"), name: None }),
            title: None,
            teacher: Some(teacher.into()),
            teacher_id: None,
            room: Some(room.into()),
        }
    }

    fn week(year: u8, start: Date, courses: Vec<Course>) -> (String, Timetable) {
        let period = Period {
            week_number: Some(43),
            start,
            end: start + time::Duration::days(5),
            label: String::new(),
        };
        // Les cours tombent le premier jour de leur semaine.
        let courses = courses.into_iter().map(|course| Course { date: start, ..course }).collect();
        (format!("A{year}_S8.pdf"), Timetable { year, week: 0, period, courses })
    }

    #[test]
    fn groups_the_courses_that_share_a_room_across_years() {
        let report = report(vec![
            week(3, date!(2026 - 10 - 19), vec![course(480, "108-9", "Blasquez I.")]),
            week(2, date!(2026 - 10 - 19), vec![course(480, "108", "Brunel M.")]),
            week(1, date!(2026 - 10 - 19), vec![course(480, "104", "Dupont M.")]),
        ]);
        assert!(report.same_dates);
        let years: Vec<u8> = report.weeks.iter().map(|week| week.year).collect();
        assert_eq!(years, [1, 2, 3], "les semaines sont rangées par année");
        assert_eq!(report.weeks[2].file, "A3_S8.pdf");
        assert_eq!(report.weeks[2].courses, 1);

        assert_eq!(report.conflicts.len(), 1);
        let conflict = &report.conflicts[0];
        assert_eq!(
            (conflict.room.as_str(), conflict.date, conflict.day.as_str()),
            ("108", date!(2026 - 10 - 19), "LUNDI")
        );
        let years: Vec<u8> = conflict.bookings.iter().map(|booking| booking.year).collect();
        assert_eq!(years, [2, 3]);
    }

    #[test]
    fn finds_nothing_when_every_room_is_booked_once() {
        let report = report(vec![
            week(
                1,
                date!(2026 - 10 - 19),
                vec![course(480, "104", "Dupont M."), course(600, "104", "Martin P.")],
            ),
            week(2, date!(2026 - 10 - 19), vec![course(480, "105", "Brunel M.")]),
        ]);
        assert!(report.same_dates);
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn says_when_the_weeks_do_not_cover_the_same_dates() {
        let mixed = report(vec![
            week(1, date!(2026 - 10 - 19), vec![course(480, "104", "Dupont M.")]),
            week(2, date!(2026 - 10 - 12), vec![course(480, "104", "Martin P.")]),
        ]);
        assert!(!mixed.same_dates);
        assert!(mixed.conflicts.is_empty());
        // Un seul PDF couvre toujours ses propres dates.
        assert!(report(vec![week(1, date!(2026 - 10 - 19), Vec::new())]).same_dates);
    }

    #[test]
    fn serialises_each_booking_with_its_year_and_course() {
        let report = report(vec![
            week(1, date!(2026 - 10 - 19), vec![course(480, "104", "Dupont M.")]),
            week(2, date!(2026 - 10 - 19), vec![course(540, "104", "Martin P.")]),
        ]);
        let json = serde_json::to_value(&report).expect("JSON");
        assert_eq!(json["conflicts"][0]["date"], "2026-10-19");
        assert_eq!(json["conflicts"][0]["bookings"][1]["year"], 2);
        assert_eq!(json["conflicts"][0]["bookings"][1]["course"]["start"], "09:00");
        assert_eq!(json["weeks"][0]["period"]["start"], "2026-10-19");
    }
}
