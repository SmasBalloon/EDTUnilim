//! Reconstruction des créneaux à partir des primitives et de la grille.

use time::{Date, Duration, Month};

use crate::model::{ClockTime, Course, CourseKind, Period, Resource, Timetable};
use crate::{catalog, teachers};

use super::content::FilledArea;
use super::geometry::Span;
use super::grid::{self, CanonicalPage, Grid, GroupRow};

/// Part d'une ligne de groupe qu'une case doit recouvrir pour la concerner.
const ROW_COVERAGE: f64 = 0.6;
/// En deçà de cette fraction du pas de grille, la case ne vise qu'un demi-groupe.
const HALF_ROW_RATIO: f64 = 0.75;
/// Suffixes des demi-groupes, dans le sens de lecture de la grille. Le
/// département les note en minuscules (`G1a`, `G1b`).
const SUBGROUPS: [&str; 2] = ["a", "b"];
/// Débordement vertical toléré entre un texte et sa case, en fraction du pas de
/// grille : la dernière ligne d'un bloc dépasse souvent sous le rectangle.
const TEXT_OVERFLOW_RATIO: f64 = 0.3;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("l'en-tête du document est absent ou illisible")]
    MissingHeader,
}

/// Assemble l'emploi du temps final.
pub fn extract(page: &CanonicalPage, grid: &Grid, year: u8, week: u8) -> Result<Timetable, ExtractError> {
    let period = read_period(page).ok_or(ExtractError::MissingHeader)?;

    let cases: Vec<(FilledArea, CourseKind)> = deduplicate(&page.fills)
        .into_iter()
        .filter_map(|area| CourseKind::from_hex(&area.color.to_hex()).map(|kind| (area, kind)))
        .collect();
    let blocks = assign_texts(page, grid, &cases);

    let mut courses: Vec<Course> = cases
        .iter()
        .zip(blocks)
        .filter_map(|((area, kind), block)| build_course(grid, &period, *kind, area, block))
        .collect();

    courses.sort_by(|a, b| a.date.cmp(&b.date).then(a.start.cmp(&b.start)).then(a.groups.cmp(&b.groups)));
    Ok(Timetable { year, week, period, courses })
}

/// Répartit les textes entre les cases.
///
/// Un texte peut déborder sous sa case et retomber dans celle du dessous. Ce
/// qui les départage est l'alignement : un bloc est calé sur le bord gauche de
/// sa case, donc le texte revient à la case la plus à droite qui commence avant
/// lui, la proximité verticale ne servant qu'à trancher les ex æquo.
fn assign_texts<'a>(
    page: &'a CanonicalPage,
    grid: &Grid,
    cases: &[(FilledArea, CourseKind)],
) -> Vec<Vec<(f64, &'a str)>> {
    let overflow = grid.row_pitch * TEXT_OVERFLOW_RATIO;
    let mut blocks = vec![Vec::new(); cases.len()];

    for fragment in &page.texts {
        if grid::is_grid_label(&fragment.text) {
            continue;
        }
        let text = fragment.text.trim();
        if text.is_empty() {
            continue;
        }
        let anchor = fragment.anchor;
        let owner = cases
            .iter()
            .enumerate()
            .filter(|(_, (area, _))| anchor.x >= area.rect.x0 && anchor.x <= area.rect.x1)
            .filter_map(|(index, (area, _))| {
                let gap = area.rect.vertical_span().distance_to(anchor.y);
                (gap <= overflow).then_some((index, gap, area.rect.x0))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2).then(b.1.total_cmp(&a.1)));

        if let Some((index, _, _)) = owner {
            blocks[index].push((anchor.y * grid.reading_sign, text));
        }
    }

    for block in &mut blocks {
        block.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    blocks
}

/// Deux cases identiques peuvent être peintes l'une sur l'autre.
fn deduplicate(fills: &[FilledArea]) -> Vec<FilledArea> {
    let key = |area: &FilledArea| {
        let round = |v: f64| (v * 10.0).round() as i64;
        (area.color, round(area.rect.x0), round(area.rect.y0), round(area.rect.x1), round(area.rect.y1))
    };
    let mut seen = std::collections::HashSet::new();
    fills.iter().filter(|area| seen.insert(key(area))).cloned().collect()
}

fn build_course(
    grid: &Grid,
    period: &Period,
    kind: CourseKind,
    area: &FilledArea,
    lines: Vec<(f64, &str)>,
) -> Option<Course> {
    let (day_index, day_name, groups) = locate(grid, area, kind)?;

    let start = ClockTime::rounded_to_5(grid.time.minutes_at(area.rect.x0));
    let end = ClockTime::rounded_to_5(grid.time.minutes_at(area.rect.x1));
    if end <= start {
        return None;
    }

    let block = parse_block(&lines.iter().map(|(_, text)| *text).collect::<Vec<_>>());
    // La case nomme l'enseignant comme elle peut ; le référentiel dit de qui
    // il s'agit, et écarte ce qui n'est pas quelqu'un.
    let teacher = block.teacher.as_deref().and_then(teachers::resolve);
    Some(Course {
        day: day_name,
        date: period.start + Duration::days(day_index as i64),
        start,
        end,
        duration_minutes: end.minutes() - start.minutes(),
        kind,
        groups,
        resource: block.code.map(|code| Resource { name: catalog::name_of(&code).map(str::to_owned), code }),
        title: block.title,
        teacher: teacher.as_ref().map(|named| named.name.clone()),
        teacher_id: teacher.map(|named| named.id),
        room: block.room,
    })
}

/// Détermine le jour et les groupes couverts par une case.
fn locate(grid: &Grid, area: &FilledArea, kind: CourseKind) -> Option<(usize, String, Vec<String>)> {
    let span = area.rect.vertical_span();
    let rows =
        grid.days.iter().enumerate().flat_map(|(day, band)| band.rows.iter().map(move |row| (day, row)));

    let covered: Vec<(usize, &GroupRow)> =
        rows.clone().filter(|(_, row)| span.overlap(&row.span) >= row.span.length() * ROW_COVERAGE).collect();

    if !covered.is_empty() {
        // Une case débordant sur deux jours revient à celui qu'elle couvre le plus.
        let day_index = covered.iter().map(|(day, _)| *day).max_by(|a, b| {
            total_overlap(&covered, *a, &span).total_cmp(&total_overlap(&covered, *b, &span))
        })?;
        let groups =
            covered.iter().filter(|(day, _)| *day == day_index).map(|(_, row)| row.label.clone()).collect();
        return Some((day_index, grid.days[day_index].name.clone(), groups));
    }

    // Aucune ligne entièrement couverte : la case ne vise qu'un demi-groupe.
    if !kind.splits_group() && span.length() > grid.row_pitch * HALF_ROW_RATIO {
        return None;
    }
    let (day_index, row) = rows
        .max_by(|a, b| span.overlap(&a.1.span).total_cmp(&span.overlap(&b.1.span)))
        .filter(|(_, row)| span.overlap(&row.span) > 0.0)?;

    let first_half = (span.center() - row.span.center()) * grid.reading_sign < 0.0;
    let suffix = SUBGROUPS[usize::from(!first_half)];

    Some((day_index, grid.days[day_index].name.clone(), vec![format!("{}{}", row.label, suffix)]))
}

/// Recouvrement cumulé des lignes d'un jour donné.
fn total_overlap(covered: &[(usize, &GroupRow)], day: usize, span: &Span) -> f64 {
    covered.iter().filter(|(index, _)| *index == day).map(|(_, row)| span.overlap(&row.span)).sum()
}

/// Le contenu textuel d'une case, une fois les lignes interprétées.
#[derive(Debug, Default)]
struct Block {
    code: Option<String>,
    title: Option<String>,
    teacher: Option<String>,
    room: Option<String>,
}

/// Interprète les lignes d'une case.
///
/// Les petites cases tiennent sur une ligne (`R1.10 - JP - R52`) ; les grandes
/// s'étalent : code et intitulé, l'intitulé pouvant se poursuivre, puis
/// l'enseignant et enfin la salle.
fn parse_block(lines: &[&str]) -> Block {
    let mut block = Block::default();
    if lines.is_empty() {
        return block;
    }

    if let [single] = lines {
        if let Some((code, teacher, room)) = split_compact(single) {
            block.code = Some(code);
            block.teacher = Some(teacher);
            block.room = Some(room);
            return block;
        }
    }

    let mut rest = lines;
    if rest.len() > 1 && looks_like_room(rest[rest.len() - 1]) {
        block.room = Some(rest[rest.len() - 1].to_owned());
        rest = &rest[..rest.len() - 1];
    }
    if rest.len() > 1 && looks_like_teacher(rest[rest.len() - 1]) {
        block.teacher = Some(rest[rest.len() - 1].to_owned());
        rest = &rest[..rest.len() - 1];
    }

    let (head, tail) = rest.split_first().expect("au moins une ligne subsiste");
    let (code, head_title) = split_head(head);
    block.code = code;

    let title: String = std::iter::once(head_title.as_str())
        .chain(tail.iter().copied())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    block.title = (!title.is_empty()).then_some(title);
    block
}

/// `R1.10 - JP - R52` → code, enseignant, salle.
fn split_compact(line: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = line.split(" - ").map(str::trim).collect();
    let [code, teacher, room] = parts[..] else { return None };
    is_resource_code(code).then(|| (code.to_owned(), teacher.to_owned(), room.to_owned()))
}

/// `R1.03 R1.03 - Architecture des` → code éventuel et début d'intitulé.
///
/// Le générateur répète le code de la ressource avant de l'introduire par un
/// tiret ; ce qui suit appartient à l'intitulé.
fn split_head(head: &str) -> (Option<String>, String) {
    let mut rest = head.trim();
    let mut code: Option<&str> = None;
    while let Some(first) = rest.split_whitespace().next() {
        if !is_resource_code(first) || code.is_some_and(|seen| seen != first) {
            break;
        }
        code = Some(first);
        rest = rest[first.len()..].trim_start();
    }
    let title = rest.trim_start_matches('-').trim().to_owned();
    (code.map(str::to_owned), title)
}

/// Un code du référentiel : lettres, numéro de semestre, puis un suffixe pointé
/// (`R1.01`, `S3.01A`, `R5A.07`, `R4.Real.08`).
fn is_resource_code(text: &str) -> bool {
    let Some((head, tail)) = text.split_once('.') else { return false };
    if text.contains(char::is_whitespace) || tail.is_empty() {
        return false;
    }
    if !tail.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
        return false;
    }

    let head = head.as_bytes();
    let letters = head.iter().take_while(|b| b.is_ascii_uppercase()).count();
    let digits = head[letters..].iter().take_while(|b| b.is_ascii_digit()).count();
    letters >= 1 && digits >= 1 && head[letters + digits..].iter().all(u8::is_ascii_uppercase)
}

fn looks_like_room(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 6
        && !text.contains(char::is_whitespace)
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

fn looks_like_teacher(text: &str) -> bool {
    text.ends_with('.') && text.contains(char::is_whitespace)
}

/// Lit l'en-tête : `… Semaine 1 (36) : du 31/08/2026 au 05/09/2026 …`.
fn read_period(page: &CanonicalPage) -> Option<Period> {
    let label = page
        .texts
        .iter()
        .map(|text| text.text.trim())
        .find(|text| text.contains(" du ") && text.contains(" au "))?;

    let start = parse_french_date(after(label, " du ")?)?;
    let end = parse_french_date(after(label, " au ")?)?;
    let week_number = after(label, "(")
        .and_then(|rest| rest.split(')').next())
        .and_then(|digits| digits.trim().parse().ok());

    Some(Period { week_number, start, end, label: label.to_owned() })
}

fn after<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.split_once(needle).map(|(_, rest)| rest)
}

/// `31/08/2026` — le champ peut être suivi d'autre chose.
fn parse_french_date(text: &str) -> Option<Date> {
    let mut parts = text.trim().splitn(3, '/');
    let day: u8 = parts.next()?.trim().parse().ok()?;
    let month: u8 = parts.next()?.trim().parse().ok()?;
    let year: i32 = parts.next()?.get(..4)?.parse().ok()?;
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_compact_block() {
        let block = parse_block(&["R1.10 - JP - R52"]);
        assert_eq!(block.code.as_deref(), Some("R1.10"));
        assert_eq!(block.teacher.as_deref(), Some("JP"));
        assert_eq!(block.room.as_deref(), Some("R52"));
        assert_eq!(block.title, None);
    }

    #[test]
    fn parses_multiline_block() {
        let block = parse_block(&["R1.03 R1.03 -", "Architecture des", "ordinateurs", "Monédière T.", "AC"]);
        assert_eq!(block.code.as_deref(), Some("R1.03"));
        assert_eq!(block.title.as_deref(), Some("Architecture des ordinateurs"));
        assert_eq!(block.teacher.as_deref(), Some("Monédière T."));
        assert_eq!(block.room.as_deref(), Some("AC"));
    }

    #[test]
    fn parses_block_without_resource_code() {
        let block = parse_block(&["SCO ACCUEIL/Rentrée", "Indéterminé I.", "AC"]);
        assert_eq!(block.code, None);
        assert_eq!(block.title.as_deref(), Some("SCO ACCUEIL/Rentrée"));
        assert_eq!(block.teacher.as_deref(), Some("Indéterminé I."));
        assert_eq!(block.room.as_deref(), Some("AC"));
    }

    #[test]
    fn recognises_resource_codes() {
        for code in ["R1.01", "S3.01A", "R5A.07", "R4.Real.08", "S4.St", "P1.01"] {
            assert!(is_resource_code(code), "{code} devrait être un code");
        }
        for other in ["SCO", "AC", "Onete C.", "112", ".", "1.01", "R.01"] {
            assert!(!is_resource_code(other), "{other} ne devrait pas être un code");
        }
    }

    #[test]
    fn parses_french_dates() {
        assert_eq!(
            parse_french_date("31/08/2026 au"),
            Date::from_calendar_date(2026, Month::August, 31).ok()
        );
        assert_eq!(parse_french_date("aujourd'hui"), None);
    }
}
