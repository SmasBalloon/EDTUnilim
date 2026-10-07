//! Analyse d'emplois du temps réels.
//!
//! Les attentes ci-dessous ont été vérifiées sur les PDF que le département
//! publie par demi-groupe, qui donnent la même semaine sous une autre forme.

use edt_unilim_api::model::{Course, CourseKind};
use edt_unilim_api::{conflicts, pdf};

const A1_S3: &[u8] = include_bytes!("fixtures/A1_S3.pdf");
const A3_S2: &[u8] = include_bytes!("fixtures/A3_S2.pdf");

fn find<'a>(courses: &'a [Course], day: &str, start: &str, group: &str) -> &'a Course {
    courses
        .iter()
        .find(|course| {
            course.day == day && course.start.to_string() == start && course.groups.iter().any(|g| g == group)
        })
        .unwrap_or_else(|| panic!("créneau {day} {start} {group} introuvable"))
}

#[test]
fn reads_the_week_header() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");

    assert_eq!(timetable.year, 1);
    assert_eq!(timetable.week, 3);
    assert_eq!(timetable.period.week_number, Some(38));
    assert_eq!(timetable.period.start.to_string(), "2026-09-14");
    assert_eq!(timetable.period.end.to_string(), "2026-09-19");
}

#[test]
fn reads_a_full_group_slot() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
    let course = find(&timetable.courses, "LUNDI", "08:00", "G1");

    assert_eq!(course.end.to_string(), "10:00");
    assert_eq!(course.duration_minutes, 120);
    assert_eq!(course.kind, CourseKind::Td);
    assert_eq!(course.groups, ["G1"]);
    assert_eq!(course.resource.as_ref().map(|r| r.code.as_str()), Some("R1.05"));
    assert_eq!(course.room.as_deref(), Some("208"));
    assert_eq!(course.date.to_string(), "2026-09-14");
}

#[test]
fn splits_practicals_between_half_groups() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");

    // Le PDF du groupe G1a place bien l'anglais en TP le lundi à 10 h.
    let first_half = find(&timetable.courses, "LUNDI", "10:00", "G1a");
    assert_eq!(first_half.kind, CourseKind::Tp);
    assert_eq!(first_half.resource.as_ref().map(|r| r.code.as_str()), Some("R1.10"));
    assert_eq!(first_half.room.as_deref(), Some("R52"));

    // …tandis que G1b est en organisations sur le même créneau.
    let second_half = find(&timetable.courses, "LUNDI", "10:00", "G1b");
    assert_eq!(second_half.resource.as_ref().map(|r| r.code.as_str()), Some("R1.08"));
}

#[test]
fn reads_a_lecture_shared_by_every_group() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
    let course = find(&timetable.courses, "LUNDI", "13:30", "G1");

    assert_eq!(course.kind, CourseKind::Amphi);
    assert_eq!(course.groups, ["G1", "G2", "G3"]);
    assert_eq!(course.title.as_deref(), Some("Initiation au développement"));
    assert_eq!(course.teacher.as_deref(), Some("Onete C."));
    assert_eq!(course.room.as_deref(), Some("AC"));
}

#[test]
fn resolves_resource_names_from_the_catalog() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
    let course = find(&timetable.courses, "LUNDI", "13:30", "G1");
    let resource = course.resource.as_ref().expect("ressource identifiée");

    assert_eq!(resource.code, "R1.01");
    assert_eq!(resource.name.as_deref(), Some("Initiation au développement"));
}

#[test]
fn reads_third_year_codes() {
    // La troisième année note ses ressources `R5A.11`, hors référentiel initial.
    let timetable = pdf::parse(A3_S2, 3, 2).expect("PDF analysable");
    let course = find(&timetable.courses, "LUNDI", "15:30", "G7");

    assert_eq!(course.kind, CourseKind::Amphi);
    assert_eq!(course.resource.as_ref().map(|r| r.code.as_str()), Some("R5A.11"));
    assert_eq!(course.teacher.as_deref(), Some("Dubreuil L."));
}

#[test]
fn keeps_courses_ordered_and_consistent() {
    let timetable = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");

    assert!(timetable.courses.len() > 30, "une semaine complète est attendue");
    for pair in timetable.courses.windows(2) {
        assert!((pair[0].date, pair[0].start) <= (pair[1].date, pair[1].start));
    }
    for course in &timetable.courses {
        assert!(course.start < course.end, "créneau vide : {course:?}");
        assert!(!course.groups.is_empty(), "créneau sans groupe : {course:?}");
    }
}

#[test]
fn rejects_documents_that_are_not_timetables() {
    assert!(pdf::parse(b"pas un PDF", 1, 1).is_err());
}

#[test]
fn finds_no_room_conflict_in_real_weeks() {
    // Les cases d'une même séance et les salles réunies (`108-9`, `111-2`) ne
    // doivent pas passer pour des salles réservées deux fois.
    let first = pdf::parse(A1_S3, 1, 3).expect("PDF analysable");
    let third = pdf::parse(A3_S2, 3, 2).expect("PDF analysable");
    assert!(third.courses.iter().any(|course| course.room.as_deref() == Some("111-2")));
    for timetables in [vec![&first], vec![&third], vec![&first, &third]] {
        assert!(conflicts::find(&timetables).is_empty());
    }
}
