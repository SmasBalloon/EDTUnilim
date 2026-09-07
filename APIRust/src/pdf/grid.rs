//! Reconstruction de la grille de l'emploi du temps.
//!
//! Le PDF ne contient aucune structure : uniquement des rectangles et du texte.
//! Tout le repère (échelle horaire, bandes de jours, lignes de groupes) est donc
//! déduit des étiquettes imprimées, sans constante magique — le générateur change
//! d'échelle d'une semaine à l'autre.

use super::content::{FilledArea, Primitives, TextFragment};
use super::geometry::{Point, Rect, Span};

/// Les jours tels qu'imprimés, dans l'ordre de la semaine.
pub const DAY_NAMES: [&str; 6] = ["LUNDI", "MARDI", "MERCREDI", "JEUDI", "VENDREDI", "SAMEDI"];

/// Page ramenée dans un repère canonique : l'heure croît avec `x`.
#[derive(Debug)]
pub struct CanonicalPage {
    pub texts: Vec<TextFragment>,
    pub fills: Vec<FilledArea>,
}

/// Conversion affine d'une abscisse en minutes depuis minuit.
#[derive(Debug, Clone, Copy)]
pub struct TimeScale {
    minutes_per_unit: f64,
    origin: f64,
}

impl TimeScale {
    pub fn minutes_at(&self, x: f64) -> f64 {
        self.origin + self.minutes_per_unit * x
    }
}

/// Une ligne de groupe (`G1`, `G2`, …) à l'intérieur d'une bande de jour.
#[derive(Debug, Clone)]
pub struct GroupRow {
    pub label: String,
    pub span: Span,
}

/// Une bande horizontale correspondant à un jour de la semaine.
#[derive(Debug, Clone)]
pub struct DayBand {
    pub name: String,
    pub rows: Vec<GroupRow>,
}

#[derive(Debug)]
pub struct Grid {
    pub time: TimeScale,
    pub days: Vec<DayBand>,
    /// Hauteur d'une ligne de groupe, qui sert d'unité de tolérance.
    pub row_pitch: f64,
    /// Signe du déplacement quand on descend d'une ligne (G1 → G2).
    /// Il donne aussi le sens de lecture des lignes de texte dans une case.
    pub reading_sign: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum GridError {
    #[error("aucune étiquette horaire exploitable dans le PDF")]
    TimeScale,
    #[error("aucune étiquette de jour dans le PDF")]
    Days,
    #[error("aucune ligne de groupe dans le PDF")]
    Groups,
}

/// Remet la page dans le repère canonique puis en déduit la grille.
pub fn build(primitives: Primitives) -> Result<(CanonicalPage, Grid), GridError> {
    let page = canonicalize(primitives)?;
    let grid = Grid::from_page(&page)?;
    Ok((page, grid))
}

/// Vrai pour les textes qui composent la grille elle-même (heures, jours,
/// groupes) et non le contenu des cases.
pub fn is_grid_label(text: &str) -> bool {
    parse_clock(text).is_some() || parse_day_label(text).is_some() || parse_group_label(text).is_some()
}

/// Reconnaît une étiquette horaire (`8:00`, `13:30`) et la convertit en minutes.
fn parse_clock(text: &str) -> Option<f64> {
    let text = text.trim();
    let (hours, minutes) = text.split_once(':')?;
    let hours: u32 = hours.parse().ok()?;
    if minutes.len() != 2 {
        return None;
    }
    let minutes: u32 = minutes.parse().ok()?;
    (hours < 24 && minutes < 60).then(|| f64::from(hours * 60 + minutes))
}

fn parse_group_label(text: &str) -> Option<&str> {
    let text = text.trim();
    let mut chars = text.chars();
    (chars.next() == Some('G') && !text[1..].is_empty() && chars.all(|c| c.is_ascii_digit())).then_some(text)
}

fn parse_day_label(text: &str) -> Option<&'static str> {
    let text = text.trim();
    DAY_NAMES.iter().copied().find(|day| day.eq_ignore_ascii_case(text))
}

/// Oriente la page pour que le temps s'écoule vers les `x` croissants.
///
/// Les emplois du temps sont produits en paysage puis pivotés : selon la
/// semaine, l'axe du temps se retrouve sur `x` ou sur `y`, dans un sens ou dans
/// l'autre. On tranche à partir de la dispersion des étiquettes horaires.
fn canonicalize(primitives: Primitives) -> Result<CanonicalPage, GridError> {
    let clocks: Vec<(Point, f64)> =
        primitives.texts.iter().filter_map(|t| parse_clock(&t.text).map(|m| (t.center(), m))).collect();
    if clocks.len() < 2 {
        return Err(GridError::TimeScale);
    }

    let spread = |values: &[f64]| {
        let (min, max) = values.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
        max - min
    };
    let xs: Vec<f64> = clocks.iter().map(|(p, _)| p.x).collect();
    let ys: Vec<f64> = clocks.iter().map(|(p, _)| p.y).collect();
    let transpose = spread(&ys) > spread(&xs);

    let along = |p: Point| if transpose { p.y } else { p.x };
    let (earliest, latest) = clocks.iter().fold((clocks[0], clocks[0]), |(min, max), &c| {
        (if c.1 < min.1 { c } else { min }, if c.1 > max.1 { c } else { max })
    });
    let mirror = along(latest.0) < along(earliest.0);

    let map_point = |p: Point| {
        let p = if transpose { Point { x: p.y, y: p.x } } else { p };
        if mirror {
            Point { x: -p.x, y: p.y }
        } else {
            p
        }
    };
    let map_rect = |r: Rect| {
        let r = if transpose { r.transposed() } else { r };
        if mirror {
            r.mirrored_x()
        } else {
            r
        }
    };

    Ok(CanonicalPage {
        texts: primitives
            .texts
            .into_iter()
            .map(|t| TextFragment { anchor: map_point(t.anchor), end: t.end.map(map_point), text: t.text })
            .collect(),
        fills: primitives
            .fills
            .into_iter()
            .map(|f| FilledArea { color: f.color, rect: map_rect(f.rect) })
            .collect(),
    })
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|a, b| a.total_cmp(b));
    values[values.len() / 2]
}

impl Grid {
    fn from_page(page: &CanonicalPage) -> Result<Self, GridError> {
        let time = Self::time_scale(page)?;
        let Layout { days, row_pitch, reading_sign } = Layout::read(page)?;
        Ok(Self { time, days, row_pitch, reading_sign })
    }

    /// Déduit l'échelle horaire des étiquettes imprimées.
    ///
    /// Chaque étiquette est centrée dans la demi-heure qu'elle ouvre, et non
    /// posée sur sa graduation : on ramène donc son centre au bord gauche de sa
    /// case avant d'ajuster la droite `x → minutes`.
    fn time_scale(page: &CanonicalPage) -> Result<TimeScale, GridError> {
        let mut labels: Vec<(f64, f64)> =
            page.texts.iter().filter_map(|t| parse_clock(&t.text).map(|m| (m, t.center().x))).collect();
        if labels.len() < 2 {
            return Err(GridError::TimeScale);
        }

        // Les heures sont imprimées en haut et en bas de la grille : on
        // fusionne les doublons avant de mesurer la largeur d'une case.
        labels.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut samples: Vec<(f64, f64)> = Vec::new();
        for (minutes, x) in labels {
            match samples.last_mut() {
                Some((last_minutes, last_x)) if *last_minutes == minutes => *last_x = (*last_x + x) / 2.0,
                _ => samples.push((minutes, x)),
            }
        }
        if samples.len() < 2 {
            return Err(GridError::TimeScale);
        }

        let mut gaps: Vec<f64> = samples.windows(2).map(|w| w[1].1 - w[0].1).collect();
        let slot = median(&mut gaps);

        let n = samples.len() as f64;
        let mean_x = samples.iter().map(|(_, x)| x - slot / 2.0).sum::<f64>() / n;
        let mean_m = samples.iter().map(|(m, _)| m).sum::<f64>() / n;
        let covariance: f64 = samples.iter().map(|(m, x)| (x - slot / 2.0 - mean_x) * (m - mean_m)).sum();
        let variance: f64 = samples.iter().map(|(_, x)| (x - slot / 2.0 - mean_x).powi(2)).sum();
        if variance <= f64::EPSILON {
            return Err(GridError::TimeScale);
        }

        let minutes_per_unit = covariance / variance;
        Ok(TimeScale { minutes_per_unit, origin: mean_m - minutes_per_unit * mean_x })
    }
}

/// Découpage vertical de la page.
struct Layout {
    days: Vec<DayBand>,
    row_pitch: f64,
    reading_sign: f64,
}

impl Layout {
    fn read(page: &CanonicalPage) -> Result<Self, GridError> {
        let mut days: Vec<(&'static str, f64)> =
            page.texts.iter().filter_map(|t| parse_day_label(&t.text).map(|d| (d, t.anchor.y))).collect();
        if days.is_empty() {
            return Err(GridError::Days);
        }
        days.sort_by(|a, b| a.1.total_cmp(&b.1));
        days.dedup_by_key(|(name, _)| *name);

        let mut labels: Vec<(String, f64)> = page
            .texts
            .iter()
            .filter_map(|t| parse_group_label(&t.text).map(|g| (g.to_owned(), t.anchor.y)))
            .collect();
        if labels.is_empty() {
            return Err(GridError::Groups);
        }
        labels.sort_by(|a, b| a.1.total_cmp(&b.1));

        // Pas de la grille : l'écart médian entre deux étiquettes voisines.
        let mut gaps: Vec<f64> = labels.windows(2).map(|w| w[1].1 - w[0].1).collect();
        let row_pitch = if gaps.is_empty() { f64::MAX } else { median(&mut gaps) };

        // Sens de lecture : direction dans laquelle les groupes se succèdent.
        let reading_sign = labels.windows(2).find(|w| w[0].0 != w[1].0).map_or(1.0, |w| {
            if w[0].0 < w[1].0 {
                1.0
            } else {
                -1.0
            }
        });

        // Chaque ligne est bornée par les médiatrices avec ses voisines.
        let rows: Vec<GroupRow> = labels
            .iter()
            .enumerate()
            .map(|(index, (label, y))| {
                let low = index
                    .checked_sub(1)
                    .and_then(|previous| labels.get(previous))
                    .map_or(y - row_pitch / 2.0, |p| (p.1 + y) / 2.0);
                let high = labels.get(index + 1).map_or(y + row_pitch / 2.0, |n| (y + n.1) / 2.0);
                GroupRow { label: label.clone(), span: Span { start: low, end: high } }
            })
            .collect();

        // Chaque ligne rejoint le jour dont l'étiquette est la plus proche.
        let mut bands: Vec<DayBand> =
            days.iter().map(|(name, _)| DayBand { name: (*name).to_owned(), rows: Vec::new() }).collect();
        for row in rows {
            let center = row.span.center();
            let nearest = days
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 .1 - center).abs().total_cmp(&(b.1 .1 - center).abs()))
                .map_or(0, |(index, _)| index);
            bands[nearest].rows.push(row);
        }
        for band in &mut bands {
            band.rows.sort_by(|a, b| (reading_sign * a.span.start).total_cmp(&(reading_sign * b.span.start)));
        }
        bands.retain(|band| !band.rows.is_empty());

        // Rétablit l'ordre de la semaine plutôt que l'ordre géométrique.
        bands.sort_by_key(|band| DAY_NAMES.iter().position(|d| *d == band.name).unwrap_or(usize::MAX));
        Ok(Self { days: bands, row_pitch, reading_sign })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clock_labels() {
        assert_eq!(parse_clock(" 8:30"), Some(510.0));
        assert_eq!(parse_clock("13:00"), Some(780.0));
        assert_eq!(parse_clock("R1.01"), None);
        assert_eq!(parse_clock("8:3"), None);
    }

    #[test]
    fn parses_group_labels() {
        assert_eq!(parse_group_label("G1"), Some("G1"));
        assert_eq!(parse_group_label("G12"), Some("G12"));
        assert_eq!(parse_group_label("GX"), None);
        assert_eq!(parse_group_label("G"), None);
    }
}
