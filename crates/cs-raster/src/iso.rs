//! Pixel art isométrico (2:1): la caja de Isobox (cara de arriba, costados, contorno y
//! highlight), pilas de bloques en una rejilla y la cuadrícula isométrica. Todo en partes por
//! píxel; los colores van aparte ([`IsoColors`]).

use cs_core::Rgba;
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

/// Los colores de cada parte de una caja.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IsoColors {
    pub top: Rgba,
    pub left: Rgba,
    pub right: Rgba,
    pub outline: Rgba,
    pub highlight: Rgba,
}

impl IsoColors {
    /// Los tonos de `base` con la luz de arriba a la izquierda, como Isobox: la cara izquierda es
    /// `base`, la de arriba más clara, la derecha más oscura, el contorno muy oscuro y el
    /// highlight casi blanco. Como en el pixel art, las luces corren el matiz hacia el amarillo y
    /// las sombras hacia el azul (los grises quedan grises). El alfa es el de `base`.
    pub fn from_base(base: Rgba) -> Self {
        let (h, s, v) = to_hsv(base);
        // Los grises no tienen matiz: no se tiñen.
        let more = |s: f32, k: f32, add: f32| if s > 0.0 { (s * k + add).min(1.0) } else { 0.0 };
        let light = |v: f32, k: f32, add: f32| (v + (1.0 - v) * k + add).min(1.0);
        let shade = |h: f32, s: f32, v: f32| from_hsv(h, s, v, base.a);
        Self {
            top: shade(toward(h, 50.0, 6.0), s * 0.8, light(v, 0.3, 0.1)),
            left: base,
            right: shade(toward(h, 240.0, 8.0), more(s, 1.1, 0.05), v * 0.72),
            outline: shade(toward(h, 240.0, 15.0), more(s, 1.2, 0.1), v * 0.3),
            highlight: shade(toward(h, 50.0, 10.0), s * 0.45, light(v, 0.7, 0.15)),
        }
    }

    pub fn color(&self, part: IsoPart) -> Rgba {
        match part {
            IsoPart::Top => self.top,
            IsoPart::Left => self.left,
            IsoPart::Right => self.right,
            IsoPart::Outline => self.outline,
            IsoPart::Highlight => self.highlight,
        }
    }

    /// Cada color cambiado por el más cercano (en RGB) de `palette` sin los transparentes; sin
    /// colores opacos, como estaba.
    pub fn snapped(&self, palette: &[Rgba]) -> Self {
        let near = |c: Rgba| {
            let d = |p: &Rgba| {
                let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
                d(p.r, c.r) + d(p.g, c.g) + d(p.b, c.b)
            };
            palette
                .iter()
                .filter(|p| p.a > 0)
                .min_by_key(|p| d(p))
                .copied()
                .unwrap_or(c)
        };
        Self {
            top: near(self.top),
            left: near(self.left),
            right: near(self.right),
            outline: near(self.outline),
            highlight: near(self.highlight),
        }
    }
}

/// El color de cada píxel de `parts` (por filas; `None` = vacío).
pub fn paint_parts(parts: &IsoParts, colors: &IsoColors) -> Vec<Option<Rgba>> {
    parts
        .parts
        .iter()
        .map(|p| p.map(|p| colors.color(p)))
        .collect()
}

/// `h` corrido hacia `target` (grados) a lo sumo `step` grados, por el camino corto.
fn toward(h: f32, target: f32, step: f32) -> f32 {
    let d = (target - h + 540.0).rem_euclid(360.0) - 180.0;
    (h + d.clamp(-step, step)).rem_euclid(360.0)
}

/// Matiz (0–360), saturación y valor (0–1).
fn to_hsv(c: Rgba) -> (f32, f32, f32) {
    let (r, g, b) = (c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    let s = if max == 0.0 { 0.0 } else { d / max };
    (h, s, max)
}

fn from_hsv(h: f32, s: f32, v: f32, a: u8) -> Rgba {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let byte = |f: f32| ((f + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgba::new(byte(r), byte(g), byte(b), a)
}

#[cfg(test)]
#[path = "iso_tests.rs"]
mod tests;
