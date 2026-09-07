//! Lecture d'un emploi du temps au format PDF.
//!
//! Le document ne contient aucune structure exploitable : le générateur dessine
//! des rectangles de couleur et pose du texte par-dessus. La lecture se fait
//! donc en trois temps : extraction des primitives ([`content`]), reconstruction
//! du repère ([`grid`]), puis interprétation des cases ([`timetable`]).

mod content;
mod encoding;
mod geometry;
mod grid;
mod metrics;
mod timetable;

use lopdf::Document;

use crate::model::Timetable;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("PDF illisible : {0}")]
    Malformed(#[from] lopdf::Error),
    #[error("grille introuvable : {0}")]
    Grid(#[from] grid::GridError),
    #[error("contenu inattendu : {0}")]
    Content(#[from] timetable::ExtractError),
}

/// Analyse un emploi du temps hebdomadaire.
pub fn parse(bytes: &[u8], year: u8, week: u8) -> Result<Timetable, PdfError> {
    let document = Document::load_mem(bytes)?;
    let primitives = content::extract_first_page(&document)?;
    let (page, grid) = grid::build(primitives)?;
    Ok(timetable::extract(&page, &grid, year, week)?)
}
