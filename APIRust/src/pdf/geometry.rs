//! Types géométriques minimaux utilisés par l'extraction PDF.

/// Matrice affine PDF `[a b c d e f]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const IDENTITY: Self = Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    pub fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Self {
        Self { a, b, c, d, e, f }
    }

    /// `self` puis `other` (produit matriciel `self × other`).
    pub fn then(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
            e: self.e * other.a + self.f * other.c + other.e,
            f: self.e * other.b + self.f * other.d + other.f,
        }
    }

    pub fn apply(self, x: f64, y: f64) -> Point {
        Point { x: self.a * x + self.c * y + self.e, y: self.b * x + self.d * y + self.f }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Rectangle aligné sur les axes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Rect {
    pub fn from_points(points: &[Point]) -> Option<Self> {
        let first = points.first()?;
        let mut rect = Self { x0: first.x, y0: first.y, x1: first.x, y1: first.y };
        for p in &points[1..] {
            rect.x0 = rect.x0.min(p.x);
            rect.y0 = rect.y0.min(p.y);
            rect.x1 = rect.x1.max(p.x);
            rect.y1 = rect.y1.max(p.y);
        }
        Some(rect)
    }

    /// Étendue verticale du rectangle.
    pub fn vertical_span(&self) -> Span {
        Span { start: self.y0, end: self.y1 }
    }

    /// Échange les axes (utilisé pour remettre l'axe temporel sur X).
    pub fn transposed(&self) -> Self {
        Self { x0: self.y0, y0: self.x0, x1: self.y1, y1: self.x1 }
    }

    /// Miroir horizontal, pour orienter le temps dans le sens croissant.
    pub fn mirrored_x(&self) -> Self {
        Self { x0: -self.x1, y0: self.y0, x1: -self.x0, y1: self.y1 }
    }
}

/// Un intervalle sur un axe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

impl Span {
    pub fn length(&self) -> f64 {
        self.end - self.start
    }

    pub fn center(&self) -> f64 {
        (self.start + self.end) / 2.0
    }

    /// Longueur de la partie commune aux deux intervalles.
    pub fn overlap(&self, other: &Self) -> f64 {
        (self.end.min(other.end) - self.start.max(other.start)).max(0.0)
    }

    /// Distance séparant un point de l'intervalle, nulle s'il est dedans.
    pub fn distance_to(&self, value: f64) -> f64 {
        (self.start - value).max(value - self.end).max(0.0)
    }
}
