//! Interprétation d'un flux de contenu PDF pour en extraire les primitives
//! utiles à la lecture d'un emploi du temps : les aplats de couleur (les cases
//! de cours) et les fragments de texte, avec leurs coordonnées.

use std::collections::BTreeSet;

use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Document, Object, ObjectId};

use super::encoding::decode_win_ansi;
use super::geometry::{Matrix, Point, Rect};
use super::metrics::text_width;

/// Profondeur maximale de récursion dans les XObjects de type Form.
const MAX_FORM_DEPTH: usize = 8;

/// Couleur RVB sur 8 bits, telle qu'écrite dans le PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    fn from_unit(r: f64, g: f64, b: f64) -> Self {
        let to_byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0) as u8;
        Self(to_byte(r), to_byte(g), to_byte(b))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// Un fragment de texte et le point où il est ancré.
#[derive(Debug, Clone)]
pub struct TextFragment {
    pub anchor: Point,
    /// Point d'arrivée du texte, quand sa largeur a pu être mesurée. Il donne
    /// le centre des étiquettes, qui sont centrées sur leur graduation.
    pub end: Option<Point>,
    pub text: String,
}

impl TextFragment {
    /// Centre du fragment si sa largeur est connue, son ancre sinon.
    pub fn center(&self) -> Point {
        match self.end {
            Some(end) => Point { x: (self.anchor.x + end.x) / 2.0, y: (self.anchor.y + end.y) / 2.0 },
            None => self.anchor,
        }
    }
}

/// Une zone remplie d'une couleur unie.
#[derive(Debug, Clone)]
pub struct FilledArea {
    pub color: Rgb,
    pub rect: Rect,
}

/// Les primitives d'une page, dans le repère de la page.
#[derive(Debug, Default, Clone)]
pub struct Primitives {
    pub texts: Vec<TextFragment>,
    pub fills: Vec<FilledArea>,
}

/// Extrait les primitives de la première page du document.
pub fn extract_first_page(document: &Document) -> Result<Primitives, lopdf::Error> {
    let page_id = document.get_pages().into_values().next().ok_or(lopdf::Error::PageNumberNotFound(1))?;

    let content = document.get_and_decode_page_content(page_id)?;
    let resources = document.get_page_resources(page_id)?.0.cloned().unwrap_or_default();

    let mut interpreter = Interpreter::new(document);
    interpreter.run(&content, &resources, 0);
    Ok(interpreter.finish())
}

/// État graphique sauvegardé/restauré par les opérateurs `q` et `Q`.
#[derive(Debug, Clone, Copy)]
struct GraphicsState {
    ctm: Matrix,
    fill_color: Rgb,
}

impl Default for GraphicsState {
    fn default() -> Self {
        Self { ctm: Matrix::IDENTITY, fill_color: Rgb(0, 0, 0) }
    }
}

/// État du bloc de texte courant (entre `BT` et `ET`).
#[derive(Debug, Clone, Copy)]
struct TextState {
    matrix: Matrix,
    line_matrix: Matrix,
    leading: f64,
    font_size: f64,
    char_spacing: f64,
}

impl Default for TextState {
    fn default() -> Self {
        Self {
            matrix: Matrix::IDENTITY,
            line_matrix: Matrix::IDENTITY,
            leading: 0.0,
            font_size: 0.0,
            char_spacing: 0.0,
        }
    }
}

struct Interpreter<'a> {
    document: &'a Document,
    state: GraphicsState,
    stack: Vec<GraphicsState>,
    text: TextState,
    /// Fragment en cours d'accumulation : les opérateurs d'affichage successifs
    /// sans repositionnement appartiennent au même mot.
    pending: Option<TextFragment>,
    path: Vec<Point>,
    /// XObjects en cours de visite, pour couper les références circulaires.
    visiting: BTreeSet<ObjectId>,
    output: Primitives,
}

impl<'a> Interpreter<'a> {
    fn new(document: &'a Document) -> Self {
        Self {
            document,
            state: GraphicsState::default(),
            stack: Vec::new(),
            text: TextState::default(),
            pending: None,
            path: Vec::new(),
            visiting: BTreeSet::new(),
            output: Primitives::default(),
        }
    }

    fn finish(mut self) -> Primitives {
        self.flush_text();
        self.output
    }

    fn run(&mut self, content: &Content<Vec<Operation>>, resources: &Dictionary, depth: usize) {
        for operation in &content.operations {
            self.execute(operation, resources, depth);
        }
    }

    fn execute(&mut self, op: &Operation, resources: &Dictionary, depth: usize) {
        let n = numbers(&op.operands);
        match op.operator.as_str() {
            "q" => self.stack.push(self.state),
            "Q" => self.state = self.stack.pop().unwrap_or_default(),
            "cm" => {
                if let [a, b, c, d, e, f] = n[..] {
                    self.state.ctm = Matrix::new(a, b, c, d, e, f).then(self.state.ctm);
                }
            }

            // Couleur de remplissage (les variantes majuscules concernent le tracé).
            "rg" => {
                if let [r, g, b] = n[..] {
                    self.state.fill_color = Rgb::from_unit(r, g, b);
                }
            }
            "g" => {
                if let [v] = n[..] {
                    self.state.fill_color = Rgb::from_unit(v, v, v);
                }
            }
            "k" => {
                if let [c, m, y, k] = n[..] {
                    self.state.fill_color = cmyk_to_rgb(c, m, y, k);
                }
            }
            "sc" | "scn" => match n[..] {
                [v] => self.state.fill_color = Rgb::from_unit(v, v, v),
                [r, g, b] => self.state.fill_color = Rgb::from_unit(r, g, b),
                [c, m, y, k] => self.state.fill_color = cmyk_to_rgb(c, m, y, k),
                _ => {}
            },

            // Construction de tracé : seule la boîte englobante nous intéresse.
            "m" | "l" => {
                if let [x, y] = n[..] {
                    self.push_path_point(x, y);
                }
            }
            "c" | "v" | "y" => {
                for pair in n.chunks_exact(2) {
                    self.push_path_point(pair[0], pair[1]);
                }
            }
            "re" => {
                if let [x, y, w, h] = n[..] {
                    self.push_path_point(x, y);
                    self.push_path_point(x + w, y + h);
                }
            }

            // Peinture : on ne retient que les remplissages.
            "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => self.fill_path(),
            "S" | "s" | "n" => self.path.clear(),

            // Texte.
            "BT" => {
                self.flush_text();
                self.text = TextState::default();
            }
            "ET" => self.flush_text(),
            "Tm" => {
                if let [a, b, c, d, e, f] = n[..] {
                    self.flush_text();
                    self.text.matrix = Matrix::new(a, b, c, d, e, f);
                    self.text.line_matrix = self.text.matrix;
                }
            }
            "Tf" => {
                if let [size] = n[..] {
                    self.text.font_size = size;
                }
            }
            "Tc" => {
                if let [spacing] = n[..] {
                    self.text.char_spacing = spacing;
                }
            }
            "TL" => {
                if let [leading] = n[..] {
                    self.text.leading = leading;
                }
            }
            "Td" | "TD" => {
                if let [tx, ty] = n[..] {
                    if op.operator == "TD" {
                        self.text.leading = -ty;
                    }
                    self.new_line(tx, ty);
                }
            }
            "T*" => self.new_line(0.0, -self.text.leading),
            "Tj" | "TJ" | "'" | "\"" => {
                if matches!(op.operator.as_str(), "'" | "\"") {
                    self.new_line(0.0, -self.text.leading);
                }
                self.show_text(&op.operands);
            }

            "Do" => self.draw_xobject(&op.operands, resources, depth),
            _ => {}
        }
    }

    fn push_path_point(&mut self, x: f64, y: f64) {
        self.path.push(self.state.ctm.apply(x, y));
    }

    fn fill_path(&mut self) {
        if let Some(rect) = Rect::from_points(&self.path) {
            self.output.fills.push(FilledArea { color: self.state.fill_color, rect });
        }
        self.path.clear();
    }

    fn new_line(&mut self, tx: f64, ty: f64) {
        self.flush_text();
        self.text.line_matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty).then(self.text.line_matrix);
        self.text.matrix = self.text.line_matrix;
    }

    fn show_text(&mut self, operands: &[Object]) {
        let mut chunk = String::new();
        collect_strings(operands, &mut chunk);
        if chunk.is_empty() {
            return;
        }
        match &mut self.pending {
            Some(fragment) => fragment.text.push_str(&chunk),
            None => {
                let anchor = self.text.matrix.then(self.state.ctm).apply(0.0, 0.0);
                self.pending = Some(TextFragment { anchor, end: None, text: chunk });
            }
        }
    }

    fn flush_text(&mut self) {
        let Some(mut fragment) = self.pending.take() else { return };
        if fragment.text.trim().is_empty() {
            return;
        }
        fragment.end = text_width(&fragment.text, self.text.font_size, self.text.char_spacing)
            .map(|width| self.text.matrix.then(self.state.ctm).apply(width, 0.0));
        self.output.texts.push(fragment);
    }

    fn draw_xobject(&mut self, operands: &[Object], resources: &Dictionary, depth: usize) {
        if depth >= MAX_FORM_DEPTH {
            return;
        }
        let Some(Object::Name(name)) = operands.first() else { return };
        let Some(id) = self.lookup_xobject_id(resources, name) else { return };
        if !self.visiting.insert(id) {
            return; // référence circulaire
        }

        if let Ok(stream) = self.document.get_object(id).and_then(Object::as_stream) {
            let is_form = stream.dict.get(b"Subtype").and_then(Object::as_name).is_ok_and(|s| s == b"Form");
            if is_form {
                let matrix =
                    stream.dict.get(b"Matrix").ok().and_then(matrix_from_object).unwrap_or(Matrix::IDENTITY);
                let inner_resources = stream
                    .dict
                    .get(b"Resources")
                    .ok()
                    .and_then(|o| self.document.dereference(o).ok()?.1.as_dict().ok())
                    .cloned()
                    .unwrap_or_else(|| resources.clone());

                if let Ok(content) = stream.decompressed_content().and_then(|d| Content::decode(&d)) {
                    self.stack.push(self.state);
                    self.state.ctm = matrix.then(self.state.ctm);
                    self.run(&content, &inner_resources, depth + 1);
                    self.state = self.stack.pop().unwrap_or_default();
                }
            }
        }

        self.visiting.remove(&id);
    }

    fn lookup_xobject_id(&self, resources: &Dictionary, name: &[u8]) -> Option<ObjectId> {
        let xobjects = self.document.dereference(resources.get(b"XObject").ok()?).ok()?.1.as_dict().ok()?;
        match xobjects.get(name).ok()? {
            Object::Reference(id) => Some(*id),
            _ => None,
        }
    }
}

fn numbers(operands: &[Object]) -> Vec<f64> {
    operands.iter().filter_map(|o| o.as_float().ok().map(f64::from)).collect()
}

fn matrix_from_object(object: &Object) -> Option<Matrix> {
    let values = numbers(object.as_array().ok()?);
    match values[..] {
        [a, b, c, d, e, f] => Some(Matrix::new(a, b, c, d, e, f)),
        _ => None,
    }
}

fn collect_strings(operands: &[Object], out: &mut String) {
    for operand in operands {
        match operand {
            Object::String(bytes, _) => out.push_str(&decode_win_ansi(bytes)),
            Object::Array(items) => collect_strings(items, out),
            _ => {}
        }
    }
}

fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> Rgb {
    Rgb::from_unit((1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k))
}

#[cfg(test)]
mod tests {
    use lopdf::{dictionary, Stream};

    use super::*;

    fn op(operator: &str, operands: Vec<Object>) -> Operation {
        Operation::new(operator, operands)
    }

    fn reals(values: &[f32]) -> Vec<Object> {
        values.iter().map(|value| Object::Real(*value)).collect()
    }

    fn text(value: &str) -> Vec<Object> {
        vec![Object::string_literal(value)]
    }

    /// Fait tourner des opérations sur un document vide.
    fn run(operations: Vec<Operation>) -> Primitives {
        let document = Document::new();
        let mut interpreter = Interpreter::new(&document);
        interpreter.run(&Content { operations }, &Dictionary::new(), 0);
        interpreter.finish()
    }

    /// Un carré rempli dans la couleur préparée par `paint`.
    fn square(mut paint: Vec<Operation>) -> Primitives {
        paint.push(op("re", reals(&[0.0, 0.0, 10.0, 10.0])));
        paint.push(op("f", Vec::new()));
        run(paint)
    }

    #[test]
    fn reads_every_way_of_writing_a_fill_color() {
        let color = |paint: Vec<Operation>| square(paint).fills[0].color.to_hex();

        assert_eq!(color(vec![op("rg", reals(&[1.0, 0.0, 0.0]))]), "#ff0000");
        assert_eq!(color(vec![op("g", reals(&[1.0]))]), "#ffffff");
        assert_eq!(color(vec![op("k", reals(&[0.0, 0.0, 0.0, 1.0]))]), "#000000");
        assert_eq!(color(vec![op("k", reals(&[0.0, 0.0, 0.0, 0.0]))]), "#ffffff");
        assert_eq!(color(vec![op("sc", reals(&[1.0]))]), "#ffffff");
        assert_eq!(color(vec![op("scn", reals(&[0.0, 1.0, 0.0]))]), "#00ff00");
        assert_eq!(color(vec![op("sc", reals(&[0.0, 0.0, 0.0, 0.0]))]), "#ffffff");
        // Deux opérandes ne désignent aucune couleur : la précédente reste.
        assert_eq!(color(vec![op("rg", reals(&[0.0, 0.0, 1.0])), op("sc", reals(&[1.0, 1.0]))]), "#0000ff");
    }

    #[test]
    fn fills_the_bounding_box_of_whatever_was_drawn() {
        let drawn = run(vec![
            op("m", reals(&[0.0, 0.0])),
            op("l", reals(&[4.0, 1.0])),
            op("c", reals(&[5.0, 5.0, 6.0, 6.0, 8.0, 9.0])),
            op("f", Vec::new()),
        ]);
        let rect = drawn.fills[0].rect;
        assert_eq!((rect.x0, rect.y0, rect.x1, rect.y1), (0.0, 0.0, 8.0, 9.0));

        // Un tracé abandonné ne laisse rien à remplir.
        let abandoned =
            run(vec![op("re", reals(&[0.0, 0.0, 5.0, 5.0])), op("n", Vec::new()), op("f", Vec::new())]);
        assert!(abandoned.fills.is_empty());
    }

    #[test]
    fn applies_the_current_matrix_and_restores_it() {
        let drawn = run(vec![
            op("q", Vec::new()),
            op("cm", reals(&[1.0, 0.0, 0.0, 1.0, 100.0, 0.0])),
            op("re", reals(&[0.0, 0.0, 10.0, 10.0])),
            op("f", Vec::new()),
            op("Q", Vec::new()),
            op("re", reals(&[0.0, 0.0, 10.0, 10.0])),
            op("f", Vec::new()),
            // Un `Q` de trop revient à l'état de départ sans planter.
            op("Q", Vec::new()),
        ]);
        assert_eq!(drawn.fills[0].rect.x0, 100.0);
        assert_eq!(drawn.fills[1].rect.x0, 0.0);
    }

    #[test]
    fn keeps_a_word_written_in_pieces_together() {
        let drawn = run(vec![
            op("BT", Vec::new()),
            op("Tf", reals(&[10.0])),
            op("Tc", reals(&[0.0])),
            op("Tm", reals(&[1.0, 0.0, 0.0, 1.0, 50.0, 700.0])),
            op("Tj", text("Bon")),
            op("TJ", vec![Object::Array(vec![Object::string_literal("jour")])]),
            op("ET", Vec::new()),
        ]);
        assert_eq!(drawn.texts.len(), 1);
        assert_eq!(drawn.texts[0].text, "Bonjour");
        assert_eq!((drawn.texts[0].anchor.x, drawn.texts[0].anchor.y), (50.0, 700.0));
    }

    #[test]
    fn moves_to_the_next_line_and_drops_blank_text() {
        let drawn = run(vec![
            op("BT", Vec::new()),
            op("TL", reals(&[12.0])),
            op("Td", reals(&[10.0, 100.0])),
            op("Tj", text("un")),
            op("T*", Vec::new()),
            op("Tj", text("deux")),
            op("'", text("trois")),
            op("\"", text("quatre")),
            op("TD", reals(&[0.0, -20.0])),
            op("Tj", text("   ")),
            op("ET", Vec::new()),
        ]);
        let words: Vec<(&str, f64)> = drawn.texts.iter().map(|t| (t.text.as_str(), t.anchor.y)).collect();
        assert_eq!(words, [("un", 100.0), ("deux", 88.0), ("trois", 76.0), ("quatre", 64.0)]);
    }

    #[test]
    fn centers_a_fragment_on_its_measured_end() {
        let anchor = Point { x: 0.0, y: 0.0 };
        let fragment = TextFragment { anchor, end: Some(Point { x: 10.0, y: 4.0 }), text: "x".into() };
        let center = fragment.center();
        assert_eq!((center.x, center.y), (5.0, 2.0));
        let unmeasured = TextFragment { anchor: Point { x: 3.0, y: 1.0 }, end: None, text: "x".into() };
        assert_eq!((unmeasured.center().x, unmeasured.center().y), (3.0, 1.0));
    }

    #[test]
    fn converts_cmyk_to_rgb() {
        assert_eq!(cmyk_to_rgb(0.0, 0.0, 0.0, 0.0), Rgb(255, 255, 255));
        assert_eq!(cmyk_to_rgb(0.0, 0.0, 0.0, 1.0), Rgb(0, 0, 0));
        assert_eq!(cmyk_to_rgb(1.0, 0.0, 0.0, 0.0), Rgb(0, 255, 255));
    }

    /// Un document avec un XObject `Form` prêt à servir, nommé `Fm0`.
    fn with_form(form_operations: Vec<Operation>, matrix: Option<Vec<Object>>) -> (Document, Dictionary) {
        let mut document = Document::new();
        let content = Content { operations: form_operations }.encode().expect("flux encodé");
        let mut dict = dictionary! { "Type" => "XObject", "Subtype" => "Form" };
        if let Some(matrix) = matrix {
            dict.set("Matrix", Object::Array(matrix));
        }
        let form = document.add_object(Stream::new(dict, content));
        let resources = dictionary! { "XObject" => dictionary! { "Fm0" => Object::Reference(form) } };
        (document, resources)
    }

    fn draw(document: &Document, resources: &Dictionary, depth: usize) -> Primitives {
        let mut interpreter = Interpreter::new(document);
        let operations = vec![op("Do", vec![Object::Name(b"Fm0".to_vec())])];
        interpreter.run(&Content { operations }, resources, depth);
        interpreter.finish()
    }

    #[test]
    fn draws_a_form_in_its_own_coordinates() {
        let square = vec![op("re", reals(&[0.0, 0.0, 10.0, 10.0])), op("f", Vec::new())];
        let matrix = reals(&[1.0, 0.0, 0.0, 1.0, 30.0, 0.0]);
        let (document, resources) = with_form(square, Some(matrix));

        let drawn = draw(&document, &resources, 0);
        assert_eq!(drawn.fills.len(), 1);
        assert_eq!(drawn.fills[0].rect.x0, 30.0);
    }

    #[test]
    fn stops_at_the_depth_limit_and_on_unknown_names() {
        let square = vec![op("re", reals(&[0.0, 0.0, 10.0, 10.0])), op("f", Vec::new())];
        let (document, resources) = with_form(square, None);

        assert!(draw(&document, &resources, MAX_FORM_DEPTH).fills.is_empty());

        let mut interpreter = Interpreter::new(&document);
        let unknown = vec![op("Do", vec![Object::Name(b"Autre".to_vec())]), op("Do", Vec::new())];
        interpreter.run(&Content { operations: unknown }, &resources, 0);
        assert!(interpreter.finish().fills.is_empty());
    }

    #[test]
    fn a_form_that_calls_itself_is_drawn_once() {
        let mut document = Document::new();
        let recursive = Content {
            operations: vec![
                op("re", reals(&[0.0, 0.0, 1.0, 1.0])),
                op("f", Vec::new()),
                op("Do", vec![Object::Name(b"Fm0".to_vec())]),
            ],
        };
        let dict = dictionary! { "Type" => "XObject", "Subtype" => "Form" };
        let id = document.add_object(Stream::new(dict, recursive.encode().expect("flux encodé")));
        let resources = dictionary! { "XObject" => dictionary! { "Fm0" => Object::Reference(id) } };

        assert_eq!(draw(&document, &resources, 0).fills.len(), 1);
    }

    #[test]
    fn reads_a_matrix_only_when_it_has_six_numbers() {
        let six = Object::Array(reals(&[1.0, 0.0, 0.0, 1.0, 5.0, 6.0]));
        assert!(matrix_from_object(&six).is_some());
        assert!(matrix_from_object(&Object::Array(reals(&[1.0, 2.0]))).is_none());
        assert!(matrix_from_object(&Object::Integer(3)).is_none());
    }

    #[test]
    fn extracts_nothing_from_an_empty_document() {
        assert!(extract_first_page(&Document::new()).is_err());
    }
}
