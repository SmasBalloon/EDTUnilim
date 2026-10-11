//! Comparaison de deux versions d'une même semaine.

use crate::model::{Course, Timetable};

/// Ce qui distingue deux versions d'une même semaine.
#[derive(Debug, Default)]
pub struct Changes<'a> {
    pub added: Vec<&'a Course>,
    pub removed: Vec<&'a Course>,
    /// Même créneau (jour, horaire, groupes), contenu modifié : `(avant, après)`.
    pub modified: Vec<(&'a Course, &'a Course)>,
}

impl Changes<'_> {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.modified.is_empty()
    }
}

/// Tout ce qui décrit un créneau : deux créneaux égaux ici sont identiques.
fn same_course(a: &Course, b: &Course) -> bool {
    same_slot(a, b)
        && a.kind == b.kind
        && a.resource.as_ref().map(|r| &r.code) == b.resource.as_ref().map(|r| &r.code)
        && a.title == b.title
        && a.teacher == b.teacher
        && a.room == b.room
}

/// Le même créneau pour les mêmes groupes, quel qu'en soit le contenu.
fn same_slot(a: &Course, b: &Course) -> bool {
    a.date == b.date && a.start == b.start && a.end == b.end && a.groups == b.groups
}

/// Compare deux versions d'une semaine, pour les cours d'un demi-groupe.
pub fn diff<'a>(before: &'a Timetable, after: &'a Timetable, group: &str) -> Changes<'a> {
    let before: Vec<&Course> = before.courses.iter().filter(|course| course.concerns(group)).collect();
    let after: Vec<&Course> = after.courses.iter().filter(|course| course.concerns(group)).collect();

    let mut removed: Vec<&Course> =
        before.iter().copied().filter(|old| !after.iter().any(|new| same_course(old, new))).collect();
    let mut added: Vec<&Course> =
        after.iter().copied().filter(|new| !before.iter().any(|old| same_course(old, new))).collect();

    // Un créneau retiré puis remis au même endroit a simplement changé de
    // salle, d'enseignant ou de matière.
    let mut modified = Vec::new();
    removed.retain(|old| match added.iter().position(|new| same_slot(old, new)) {
        Some(index) => {
            modified.push((*old, added.remove(index)));
            false
        }
        None => true,
    });

    let order = |c: &&Course| (c.date, c.start, c.groups.clone());
    added.sort_by_key(order);
    removed.sort_by_key(order);
    modified.sort_by_key(|(old, _)| (old.date, old.start, old.groups.clone()));
    Changes { added, removed, modified }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::fixtures::{course, week};

    #[test]
    fn ignores_an_identical_republication() {
        let before = week(vec![course(600, "G1A", "104")]);
        let after = week(vec![course(600, "G1A", "104")]);
        assert!(diff(&before, &after, "G1a").is_empty());
    }

    #[test]
    fn pairs_a_slot_whose_content_changed() {
        let before = week(vec![course(600, "G1A", "104"), course(840, "G1", "105")]);
        let after = week(vec![course(600, "G1A", "208"), course(480, "G1", "103")]);
        let changes = diff(&before, &after, "G1a");
        assert!(diff(&before, &after, "G3a").is_empty(), "un autre groupe n'est pas concerné");

        // G1A : salle changée. G1B : cours retiré. G1 entier : cours ajouté.
        assert_eq!(changes.modified.len(), 1);
        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.removed.len(), 1);
    }
}
