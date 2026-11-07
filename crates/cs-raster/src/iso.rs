//! Pixel art isométrico (2:1): la caja de Isobox (cara de arriba, costados, contorno y
//! highlight), pilas de bloques en una rejilla y la cuadrícula isométrica. Todo en partes por
//! píxel; los colores van aparte ([`IsoColors`]).

use serde::{Deserialize, Serialize};

/// El ancho de las esquinas de arriba y de abajo de la cara de arriba: dónde se juntan las dos
/// aristas de atrás (y las dos de adelante).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsoCorner {
    /// Las aristas comparten el tramo de la punta: ancho par.
    Two,
    /// Se enciman un píxel: una columna central (la de Isobox de fábrica).
    #[default]
    Three,
    /// Quedan lado a lado: punta plana de 4 (la baldosa clásica de 32×16).
    Four,
}

impl IsoCorner {
    pub const ALL: [IsoCorner; 3] = [IsoCorner::Two, IsoCorner::Three, IsoCorner::Four];

    /// Los píxeles de la esquina.
    pub fn pixels(self) -> u32 {
        match self {
            IsoCorner::Two => 2,
            IsoCorner::Three => 3,
            IsoCorner::Four => 4,
        }
    }

    /// La esquina de `pixels` (2, 3 o 4).
    pub fn from_pixels(pixels: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.pixels() == pixels)
    }

    pub fn label(self) -> &'static str {
        match self {
            IsoCorner::Two => "2 px",
            IsoCorner::Three => "3 px",
            IsoCorner::Four => "4 px",
        }
    }

    /// Cuánto se enciman los tramos de 2 de las dos aristas en la punta.
    fn overlap(self) -> i32 {
        4 - self.pixels() as i32
    }
}

/// Una caja isométrica: `left` y `right` en unidades (un tramo de 2 px de ancho y 1 de alto por
/// unidad, como Isobox) y `height` en píxeles (0 = una baldosa plana).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IsoBox {
    pub left: u32,
    pub right: u32,
    pub height: u32,
    pub corner: IsoCorner,
}

impl Default for IsoBox {
    fn default() -> Self {
        Self {
            left: 4,
            right: 4,
            height: 8,
            corner: IsoCorner::Three,
        }
    }
}

/// Qué es cada píxel de una caja.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IsoPart {
    Top,
    Left,
    Right,
    Outline,
    Highlight,
}

/// Las partes de un dibujo isométrico, por filas (`None` = vacío).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IsoParts {
    pub width: u32,
    pub height: u32,
    pub parts: Vec<Option<IsoPart>>,
}

impl IsoParts {
    fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            parts: vec![None; (width * height) as usize],
        }
    }

    /// La parte en `(x, y)` (`None` fuera o vacío).
    pub fn get(&self, x: i32, y: i32) -> Option<IsoPart> {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return None;
        }
        self.parts[(y as u32 * self.width + x as u32) as usize]
    }

    fn set(&mut self, x: i32, y: i32, part: IsoPart) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            self.parts[(y as u32 * self.width + x as u32) as usize] = Some(part);
        }
    }

    /// Los píxeles no vacíos con su parte.
    pub fn points(&self) -> impl Iterator<Item = (i32, i32, IsoPart)> + '_ {
        self.parts.iter().enumerate().filter_map(|(i, p)| {
            let w = self.width as usize;
            p.map(|p| ((i % w) as i32, (i / w) as i32, p))
        })
    }

    /// Marca como contorno los píxeles no vacíos con algún vecino de los 4 vacío (o afuera).
    fn outline_silhouette(&mut self) {
        let edge: Vec<(i32, i32)> = self
            .points()
            .filter(|&(x, y, _)| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| self.get(x + dx, y + dy).is_none())
            })
            .map(|(x, y, _)| (x, y))
            .collect();
        for (x, y) in edge {
            self.set(x, y, IsoPart::Outline);
        }
    }
}

impl IsoBox {
    /// Las unidades de cada arista (al menos 1).
    fn units(&self) -> (i32, i32) {
        (self.left.max(1) as i32, self.right.max(1) as i32)
    }

    /// Ancho y alto en píxeles: `2·(left + right) − o` (o = 2, 1, 0 para 2, 3 y 4 px) por
    /// `left + right + height`.
    pub fn size(&self) -> (u32, u32) {
        let (l, r) = self.units();
        (
            (2 * (l + r) - self.corner.overlap()) as u32,
            (l + r) as u32 + self.height,
        )
    }

    /// De qué columna a qué columna (inclusive) va la fila `y` de la cara de arriba.
    fn top_span(&self, y: i32) -> Option<(i32, i32)> {
        let (l, r) = self.units();
        if y < 0 || y >= l + r {
            return None;
        }
        // La punta de atrás-izquierda (`a`) y la de atrás-derecha (`b`): donde empiezan los
        // tramos de las dos aristas de atrás en la fila 0.
        let a = 2 * (l - 1);
        let b = 2 * l - self.corner.overlap();
        let start = if y < l { a - 2 * y } else { 2 * (y - l) };
        let end = if y < r {
            b + 2 * y + 1
        } else {
            b + 2 * (r - 1) + 1 - 2 * (y - r)
        };
        Some((start, end))
    }

    /// La columna de la arista vertical del frente, en medios píxeles (el centro del píxel `x`
    /// es `2x + 1`): a la izquierda la cara izquierda, a la derecha la derecha.
    fn front_edge2(&self) -> i32 {
        let (_, r) = self.units();
        4 * r - self.corner.overlap()
    }

    /// Las caras (y el highlight) sin contorno.
    fn faces(&self, highlight: bool) -> IsoParts {
        let (w, h) = self.size();
        let mut out = IsoParts::empty(w, h);
        let (l, r) = self.units();
        // De qué fila a qué fila va cada columna en la cara de arriba.
        let mut columns = vec![(i32::MAX, i32::MIN); w as usize];
        for y in 0..l + r {
            let (x0, x1) = self.top_span(y).expect("a row of the top face");
            for c in &mut columns[x0 as usize..=x1 as usize] {
                *c = (c.0.min(y), c.1.max(y));
            }
        }
        let in_top = |x: i32, y: i32| self.top_span(y).is_some_and(|(a, b)| a <= x && x <= b);
        let edge2 = self.front_edge2();
        for (x, &(top, bottom)) in columns.iter().enumerate() {
            let x = x as i32;
            for y in top..=bottom + self.height as i32 {
                let part = if in_top(x, y) {
                    // El borde de adelante de la cara de arriba: debajo hay un costado.
                    let rim = self.height > 0 && !in_top(x, y + 1);
                    if rim && highlight {
                        IsoPart::Highlight
                    } else {
                        IsoPart::Top
                    }
                } else {
                    match (2 * x + 1).cmp(&edge2) {
                        std::cmp::Ordering::Less => IsoPart::Left,
                        std::cmp::Ordering::Greater => IsoPart::Right,
                        std::cmp::Ordering::Equal if highlight => IsoPart::Highlight,
                        std::cmp::Ordering::Equal => IsoPart::Left,
                    }
                };
                out.set(x, y, part);
            }
        }
        out
    }
}

/// La caja `shape`: sus caras, con `outline` el contorno de la silueta (1 px en los costados
/// verticales, tramos de 2 en las aristas) y con `highlight` el borde de adelante de la cara de
/// arriba y (con esquina de 3 px) la arista vertical del frente.
pub fn iso_box(shape: &IsoBox, outline: bool, highlight: bool) -> IsoParts {
    let mut parts = shape.faces(highlight);
    if outline {
        parts.outline_silhouette();
    }
    parts
}

#[cfg(test)]
#[path = "iso_tests.rs"]
mod tests;
