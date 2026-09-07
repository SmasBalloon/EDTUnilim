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
