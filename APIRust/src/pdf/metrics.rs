//! Largeur des chaînes, pour retrouver le centre d'une étiquette.
//!
//! Les étiquettes horaires sont centrées sur leur graduation, mais un PDF ne
//! stocke que le point de départ du texte. Sans la largeur de la chaîne, la
//! graduation est décalée d'une demi-étiquette — soit environ cinq minutes.
//!
//! Les emplois du temps n'utilisent que des Helvetica standard ; on se limite
//! aux caractères qui composent une heure, et on renonce à mesurer le reste
//! plutôt que d'inventer des métriques.

/// Largeur d'un caractère en millièmes de cadratin (Helvetica-Bold).
fn glyph_width(c: char) -> Option<f64> {
    match c {
        '0'..='9' => Some(556.0),
        ':' => Some(333.0),
        ' ' => Some(278.0),
        _ => None,
    }
}

/// Largeur d'une chaîne en unités de l'espace texte, ou `None` si elle contient
/// un caractère non mesuré.
pub fn text_width(text: &str, font_size: f64, char_spacing: f64) -> Option<f64> {
    let mut width = 0.0;
    for c in text.chars() {
        width += glyph_width(c)? / 1000.0 * font_size + char_spacing;
    }
    Some(width)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_clock_labels() {
        // " 9:30" est plus étroit que "10:00" d'une espace moins un chiffre.
        let narrow = text_width(" 9:30", 1000.0, 0.0).unwrap();
        let wide = text_width("10:00", 1000.0, 0.0).unwrap();
        assert!((wide - narrow - 278.0).abs() < 1e-9);
    }

    #[test]
    fn declines_unknown_glyphs() {
        assert_eq!(text_width("R1.01", 21.0, 0.0), None);
    }
}
