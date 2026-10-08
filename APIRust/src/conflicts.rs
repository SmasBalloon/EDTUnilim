//! Salles réservées par plusieurs cours en même temps.
//!
//! Les cours s'enchaînent dans une salle sans se chevaucher : `8h-10h` puis
//! `10h-12h` est normal. Deux créneaux dont les horaires empiètent l'un sur
//! l'autre dans la même salle, le même jour, sont en revanche une erreur de
//! planning. Les salles sont partagées par les trois années, dont les PDF sont
//! distincts : la recherche porte donc sur plusieurs emplois du temps à la fois,
//! rapprochés par la date de leurs cours.

use std::collections::{BTreeMap, HashSet};

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

    /// Le même créneau lu dans le même emploi du temps.
    fn is(&self, other: &Booking) -> bool {
        std::ptr::eq(self.course, other.course)
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

/// Les cours qui se disputent une salle, de proche en proche : un long cours
/// et les deux plus courts qu'il recouvre forment un seul ensemble.
#[derive(Debug, Clone)]
pub struct Overlap<'a> {
    pub room: String,
    /// Dans l'ordre de leurs horaires, sans doublon.
    pub bookings: Vec<Booking<'a>>,
}

impl<'a> Overlap<'a> {
    fn contains(&self, booking: &Booking) -> bool {
        self.bookings.iter().any(|b| b.is(booking))
    }

    /// Le premier créneau de l'ensemble, qui en donne le jour.
    pub fn first(&self) -> &Booking<'a> {
        &self.bookings[0]
    }
}

/// Les salles doubles réservées par l'ensemble de ces emplois du temps (une
/// semaine de chaque année, typiquement), dans l'ordre chronologique.
pub fn find<'a>(timetables: &[&'a Timetable]) -> Vec<Conflict<'a>> {
    let bookings: Vec<Booking<'a>> = timetables
        .iter()
        .flat_map(|timetable| timetable.courses.iter().map(|course| Booking { year: timetable.year, course }))
        .collect();
    // Une salle combinée (`111-2`) occupe chacune des salles qu'elle réunit.
    let mut by_room: BTreeMap<(Date, String), Vec<usize>> = BTreeMap::new();
    for (id, booking) in bookings.iter().enumerate() {
        for room in rooms(booking.course.room.as_deref().unwrap_or_default()) {
            by_room.entry((booking.course.date, room)).or_default().push(id);
        }
    }

    // Deux cours de la même salle combinée se croisent dans chacune : un seul conflit.
    let mut seen = HashSet::new();
    let mut conflicts = Vec::new();
    for ((_, room), mut ids) in by_room {
        ids.sort_by_key(|&id| (bookings[id].course.start, bookings[id].course.end, bookings[id].year));
        for (index, &first) in ids.iter().enumerate() {
            for &second in &ids[index + 1..] {
                let (a, b) = (bookings[first], bookings[second]);
                // Triés par début : au-delà de la fin du premier, plus rien ne l'atteint.
                if b.course.start >= a.course.end {
                    break;
                }
                if !same_session(a.course, b.course) && seen.insert((first.min(second), first.max(second))) {
                    conflicts.push(Conflict { room: room.clone(), first: a, second: b });
                }
            }
        }
    }
    conflicts.sort_by_key(|c| (c.first.course.date, c.first.course.start, c.room.clone()));
    conflicts
}

/// Rassemble les conflits d'une même salle qui partagent un cours, pour ne
/// nommer chaque cours qu'une fois. Dans l'ordre chronologique.
pub fn group<'a>(conflicts: &[&Conflict<'a>]) -> Vec<Overlap<'a>> {
    let mut overlaps: Vec<Overlap<'a>> = Vec::new();
    for conflict in conflicts {
        let pair = [conflict.first, conflict.second];
        // Un conflit peut relier deux ensembles jusque-là distincts : ils fusionnent.
        let joined: Vec<usize> = overlaps
            .iter()
            .enumerate()
            .filter(|(_, o)| o.room == conflict.room && pair.iter().any(|b| o.contains(b)))
            .map(|(index, _)| index)
            .collect();
        let mut merged = Overlap { room: conflict.room.clone(), bookings: Vec::new() };
        for index in joined.into_iter().rev() {
            merged.bookings.extend(overlaps.remove(index).bookings);
        }
        for booking in pair {
            if !merged.contains(&booking) {
                merged.bookings.push(booking);
            }
        }
        overlaps.push(merged);
    }
    for overlap in &mut overlaps {
        overlap.bookings.sort_by_key(|b| (b.course.start, b.course.end, b.year));
    }
    overlaps.sort_by_key(|o| (o.first().course.date, o.first().course.start, o.room.clone()));
    overlaps
}

/// Libellé de salle comparable : en majuscules, sans espaces superflus.
fn normalise_room(room: &str) -> String {
    room.trim().to_uppercase()
}

/// Les salles qu'un libellé désigne. Les salles réunies s'écrivent comme une
/// plage abrégée : `111-2` pour 111 et 112, `108-9` ou `108-109` pour 108 et 109.
fn rooms(label: &str) -> Vec<String> {
    let label = normalise_room(label);
    if label.is_empty() {
        return Vec::new();
    }
    combined(&label).unwrap_or_else(|| vec![label])
}

/// Écart maximal d'une plage de salles : au-delà, le libellé est autre chose.
const MAX_COMBINED: u32 = 4;

/// `111-2` → `111`, `112` ; `None` si le libellé n'est pas une plage de salles.
fn combined(label: &str) -> Option<Vec<String>> {
    let (first, last) = label.split_once('-')?;
    let (first, last) = (first.trim(), last.trim());
    let is_number = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    if !is_number(first) || !is_number(last) || last.len() > first.len() {
        return None;
    }
    // La fin ne reprend que les derniers chiffres : on complète par le début.
    let last = format!("{}{last}", &first[..first.len() - last.len()]);
    let (from, to): (u32, u32) = (first.parse().ok()?, last.parse().ok()?);
    if to <= from || to - from > MAX_COMBINED {
        return None;
    }
    Some((from..=to).map(|room| format!("{room:0width$}", width = first.len())).collect())
}

/// Une même séance lue en deux cases (un cours donné à plusieurs groupes qui
/// ne sont pas côte à côte) : mêmes horaires, même nature, même matière, même
/// enseignant. Ce n'est pas un conflit, la salle est partagée à dessein.
///
/// Un enseignant absent du référentiel est lu `None` des deux côtés : la
/// matière suffit alors, mais sans matière ni titre, rien ne relie les cases.
fn same_session(a: &Course, b: &Course) -> bool {
    let subject = a.resource.as_ref().map(|r| &r.code);
    a.start == b.start
        && a.end == b.end
        && a.kind == b.kind
        && subject == b.resource.as_ref().map(|r| &r.code)
        && a.title == b.title
        && (subject.is_some() || a.title.is_some())
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
        assert_eq!(conflicts[0].first.course.groups, ["G1"]);
        assert_eq!(conflicts[0].second.course.groups, ["G2"]);
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

    #[test]
    fn reads_combined_rooms_as_each_of_their_rooms() {
        assert_eq!(rooms("111-2"), ["111", "112"]);
        assert_eq!(rooms("108-9"), ["108", "109"]);
        assert_eq!(rooms("108-109"), ["108", "109"]);
        assert_eq!(rooms("208-10"), ["208", "209", "210"]);
        assert_eq!(rooms(" r52 "), ["R52"]);
        assert!(rooms("  ").is_empty());
        // Ni une plage croissante, ni une plage courte, ni des numéros : un libellé comme un autre.
        assert_eq!(rooms("112-1"), ["112-1"]);
        assert_eq!(rooms("100-300"), ["100-300"]);
        assert_eq!(rooms("TP-A"), ["TP-A"]);
    }

    #[test]
    fn a_combined_room_conflicts_with_each_room_it_joins() {
        for room in ["111", "112"] {
            let timetable = week(vec![
                course(0, 480, 600, Some("111-2"), "G1", "Dupont M.", "R1.01"),
                course(0, 540, 660, Some(room), "G2", "Martin P.", "R1.02"),
            ]);
            let conflicts = find(&[&timetable]);
            assert_eq!(conflicts.len(), 1, "{room}");
            assert_eq!(conflicts[0].room, room);
        }
    }

    #[test]
    fn two_courses_in_one_combined_room_conflict_once() {
        let timetable = week(vec![
            course(0, 480, 600, Some("111-2"), "G1", "Dupont M.", "R1.01"),
            course(0, 540, 660, Some("111-2"), "G2", "Martin P.", "R1.02"),
        ]);
        let conflicts = find(&[&timetable]);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].room, "111");
    }

    #[test]
    fn an_unknown_teacher_does_not_split_one_session() {
        let mut first = course(0, 480, 600, Some("AC"), "G1", "Dupont M.", "R1.01");
        let mut second = course(0, 480, 600, Some("AC"), "G3", "Dupont M.", "R1.01");
        (first.teacher, second.teacher) = (None, None);
        assert!(find(&[&week(vec![first.clone(), second.clone()])]).is_empty());

        // Sans matière ni titre, deux cases anonymes restent deux cours.
        (first.resource, second.resource) = (None, None);
        assert_eq!(find(&[&week(vec![first, second])]).len(), 1);
    }

    #[test]
    fn an_unknown_teacher_on_one_side_only_is_another_course() {
        let first = course(0, 480, 600, Some("AC"), "G1", "Dupont M.", "R1.01");
        let mut second = course(0, 480, 600, Some("AC"), "G3", "Dupont M.", "R1.01");
        second.teacher = None;
        assert_eq!(find(&[&week(vec![first, second])]).len(), 1);
    }

    #[test]
    fn groups_the_courses_a_long_course_covers() {
        let timetable = week(vec![
            course(0, 480, 720, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(0, 480, 540, Some("104"), "G2", "Martin P.", "R1.02"),
            course(0, 600, 660, Some("104"), "G3", "Durand J.", "R1.03"),
            course(0, 480, 540, Some("105"), "G4", "Durand J.", "R1.03"),
            course(0, 480, 540, Some("105"), "G5", "Martin P.", "R1.02"),
        ]);
        let conflicts = find(&[&timetable]);
        let overlaps = group(&conflicts.iter().collect::<Vec<_>>());
        assert_eq!(overlaps.len(), 2);
        assert_eq!(overlaps[0].room, "104");
        let groups: Vec<&str> = overlaps[0].bookings.iter().map(|b| b.course.groups[0].as_str()).collect();
        assert_eq!(groups, ["G2", "G1", "G3"]);
        assert_eq!(overlaps[1].room, "105");
        assert_eq!(overlaps[1].bookings.len(), 2);
        assert!(group(&[]).is_empty());
    }

    #[test]
    fn merges_two_groups_that_a_conflict_links() {
        let courses: Vec<Course> = (0..4)
            .map(|index| {
                course(0, 480 + index * 50, 560 + index * 50, Some("104"), "G1", "Dupont M.", "R1.01")
            })
            .collect();
        let bookings: Vec<Booking> = courses.iter().map(|course| Booking { year: 1, course }).collect();
        let conflict =
            |a: usize, b: usize| Conflict { room: "104".into(), first: bookings[a], second: bookings[b] };
        let (ab, cd, bc) = (conflict(0, 1), conflict(2, 3), conflict(1, 2));
        let overlaps = group(&[&ab, &cd, &bc]);
        assert_eq!(overlaps.len(), 1);
        assert_eq!(overlaps[0].bookings.len(), 4);
    }

    #[test]
    fn keeps_apart_the_same_room_on_two_days() {
        let timetable = week(vec![
            course(0, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(0, 540, 660, Some("104"), "G2", "Martin P.", "R1.02"),
            course(1, 480, 600, Some("104"), "G1", "Dupont M.", "R1.01"),
            course(1, 540, 660, Some("104"), "G2", "Martin P.", "R1.02"),
        ]);
        let conflicts = find(&[&timetable]);
        assert_eq!(group(&conflicts.iter().collect::<Vec<_>>()).len(), 2);
    }
}
