//! Salles réservées par deux cours en même temps.
//!
//! Les cours s'enchaînent dans une salle sans se chevaucher : `8h-10h` puis
//! `10h-12h` est normal. Deux créneaux dont les horaires empiètent l'un sur
//! l'autre dans la même salle, le même jour, sont en revanche une erreur de
//! planning. Les salles sont partagées par les trois années, dont les PDF sont
//! distincts : la recherche porte donc sur plusieurs emplois du temps à la fois,
//! rapprochés par la date de leurs cours.

use std::collections::BTreeMap;

use time::Date;

use crate::model::{Course, Timetable};

/// Un créneau, avec l'année dont il vient : `G1a` existe dans chaque année.
#[derive(Debug, Clone, Copy)]
pub struct Booking<'a> {
    pub year: u8,
    pub course: &'a Course,
}

impl Booking<'_> {
    fn key(&self) -> String {
        let course = self.course;
        format!(
            "A{} {} {}-{} {} {:?} {:?} {:?}",
            self.year,
            course.date,
            course.start,
            course.end,
            course.groups.join(","),
            course.resource.as_ref().map(|r| &r.code),
            course.teacher,
            course.title,
        )
    }
}

/// Deux créneaux qui occupent la même salle au même moment.
#[derive(Debug, Clone)]
pub struct Conflict<'a> {
    /// Salle, en majuscules et sans espaces superflus.
    pub room: String,
    pub first: Booking<'a>,
    pub second: Booking<'a>,
}

impl Conflict<'_> {
    /// Un conflit concerne un demi-groupe d'une année dès que l'un de ses deux
    /// cours le concerne.
    pub fn concerns(&self, year: u8, group: &str) -> bool {
        [self.first, self.second].iter().any(|b| b.year == year && b.course.concerns(group))
    }

    /// Une des deux années du conflit.
    pub fn involves_year(&self, year: u8) -> bool {
        self.first.year == year || self.second.year == year
    }

    /// Identité du conflit, pour reconnaître le même d'une version à l'autre
    /// d'une semaine : le contenu des deux créneaux compte, pas leur ordre.
    pub fn key(&self) -> String {
        let mut slots = [self.first.key(), self.second.key()];
        slots.sort();
        format!("{}|{}|{}", self.room, slots[0], slots[1])
    }
}

/// Les salles doubles réservées par l'ensemble de ces emplois du temps (une
/// semaine de chaque année, typiquement), dans l'ordre chronologique.
pub fn find<'a>(timetables: &[&'a Timetable]) -> Vec<Conflict<'a>> {
    let mut by_room: BTreeMap<(Date, String), Vec<Booking<'a>>> = BTreeMap::new();
    for timetable in timetables {
        for course in &timetable.courses {
            let Some(room) = course.room.as_deref().map(normalise_room).filter(|room| !room.is_empty())
            else {
                continue;
            };
            by_room.entry((course.date, room)).or_default().push(Booking { year: timetable.year, course });
        }
    }

    let mut conflicts = Vec::new();
    for ((_, room), mut bookings) in by_room {
        bookings.sort_by_key(|b| (b.course.start, b.course.end, b.year));
        for (index, first) in bookings.iter().enumerate() {
            for second in &bookings[index + 1..] {
                // Triés par début : au-delà de la fin du premier, plus rien ne l'atteint.
                if second.course.start >= first.course.end {
                    break;
                }
                if !same_session(first.course, second.course) {
                    conflicts.push(Conflict { room: room.clone(), first: *first, second: *second });
                }
            }
        }
    }
    conflicts.sort_by_key(|c| (c.first.course.date, c.first.course.start, c.room.clone()));
    conflicts
}

fn normalise_room(room: &str) -> String {
    room.trim().to_uppercase()
}

/// Une même séance lue en deux cases (un cours donné à plusieurs groupes
/// côte à côte) : mêmes horaires, même nature, même matière, même enseignant.
/// Ce n'est pas un conflit, la salle est partagée à dessein.
fn same_session(a: &Course, b: &Course) -> bool {
    a.start == b.start
        && a.end == b.end
        && a.kind == b.kind
        && a.resource.as_ref().map(|r| &r.code) == b.resource.as_ref().map(|r| &r.code)
        && a.title == b.title
        && a.teacher.is_some()
        && a.teacher == b.teacher
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use super::*;
    use crate::model::{ClockTime, CourseKind, Period, Resource};

    fn course(
        day: u8,
        start: i64,
        end: i64,
        room: Option<&str>,
        group: &str,
        teacher: &str,
        code: &str,
    ) -> Course {
        Course {
            day: "LUNDI".into(),
            date: date!(2026 - 09 - 14) + time::Duration::days(i64::from(day)),
            start: ClockTime::from_minutes(start),
            end: ClockTime::from_minutes(end),
            duration_minutes: (end - start) as u16,
            kind: CourseKind::Td,
            groups: vec![group.into()],
            resource: Some(Resource { code: code.into(), name: None }),
            title: None,
            teacher: Some(teacher.into()),
            teacher_id: None,
            room: room.map(Into::into),
        }
    }

    fn week(courses: Vec<Course>) -> Timetable {
        year(1, courses)
    }

    fn year(year: u8, courses: Vec<Course>) -> Timetable {
        Timetable {
            year,
            week: 3,
            period: Period {
                week_number: None,
                start: date!(2026 - 09 - 14),
                end: date!(2026 - 09 - 19),
                label: String::new(),
            },
            courses,
        }
    }

    #[test]
    fn back_to_back_courses_do_not_conflict() {
        let timetable = week(vec![
            course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(0, 600, 720, Some("104"), "G2", "Martin P.", "R1.02"),
        ]);
        assert!(find(&[&timetable]).is_empty());
    }

    #[test]
    fn overlapping_courses_in_one_room_conflict() {
        let timetable = week(vec![
            course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(0, 570, 690, Some("104"), "G2", "Martin P.", "R1.02"),
        ]);
        let conflicts = find(&[&timetable]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].room, "104");
        assert!(conflicts[0].concerns(1, "G1a") && conflicts[0].concerns(1, "G2b"));
        assert!(!conflicts[0].concerns(1, "G3"));
    }

    #[test]
    fn courses_of_two_years_in_one_room_conflict() {
        let first = year(1, vec![course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01")]);
        let second = year(2, vec![course(0, 540, 660, Some("104"), "G1", "Martin P.", "R3.02")]);
        let conflicts = find(&[&first, &second]);
        assert_eq!(conflicts.len(), 1);
        assert!(
            conflicts[0].involves_year(1) && conflicts[0].involves_year(2) && !conflicts[0].involves_year(3)
        );
        // « G1a » existe dans chaque année : l'année départage.
        assert!(conflicts[0].concerns(1, "G1a") && conflicts[0].concerns(2, "G1a"));
        assert!(!conflicts[0].concerns(3, "G1a"));
        assert!(find(&[&first]).is_empty() && find(&[&second]).is_empty());
    }

    #[test]
    fn room_names_are_compared_loosely() {
        let timetable = week(vec![
            course(0, 480, 600, Some("r52"), "G1", "Dupont M.", "R1.01"),
            course(0, 480, 600, Some(" R52 "), "G2", "Martin P.", "R1.02"),
        ]);
        assert_eq!(find(&[&timetable]).len(), 1);
    }

    #[test]
    fn other_days_other_rooms_and_missing_rooms_are_ignored() {
        let timetable = week(vec![
            course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(1, 480, 600, Some("104"), "G2", "Martin P.", "R1.02"),
            course(0, 480, 600, Some("105"), "G3", "Martin P.", "R1.02"),
            course(0, 480, 600, None, "G4", "Martin P.", "R1.02"),
            course(0, 480, 600, None, "G5", "Martin P.", "R1.02"),
        ]);
        assert!(find(&[&timetable]).is_empty());
    }

    #[test]
    fn one_session_read_in_two_boxes_is_not_a_conflict() {
        let timetable = week(vec![
            course(0, 480, 600, Some("AC"), "G1", "Dupont M.", "R1.01"),
            course(0, 480, 600, Some("AC"), "G2", "Dupont M.", "R1.01"),
        ]);
        assert!(find(&[&timetable]).is_empty());

        // Mêmes horaires mais autre enseignant : deux cours distincts.
        let timetable = week(vec![
            course(0, 480, 600, Some("AC"), "G1", "Dupont M.", "R1.01"),
            course(0, 480, 600, Some("AC"), "G2", "Martin P.", "R1.01"),
        ]);
        assert_eq!(find(&[&timetable]).len(), 1);
    }

    #[test]
    fn a_long_course_conflicts_with_every_course_it_covers() {
        let timetable = week(vec![
            course(0, 480, 720, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(0, 480, 540, Some("104"), "G2", "Martin P.", "R1.02"),
            course(0, 600, 660, Some("104"), "G3", "Durand J.", "R1.03"),
        ]);
        assert_eq!(find(&[&timetable]).len(), 2);
    }

    #[test]
    fn a_conflict_has_the_same_key_whatever_the_order() {
        let a = course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01");
        let b = course(0, 540, 660, Some("104"), "G2", "Martin P.", "R1.02");
        let (a, b) = (Booking { year: 1, course: &a }, Booking { year: 2, course: &b });
        let one = Conflict { room: "104".into(), first: a, second: b };
        let two = Conflict { room: "104".into(), first: b, second: a };
        assert_eq!(one.key(), two.key());
    }
}
