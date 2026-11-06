//! Outline, como Edit → FX → Outline de Aseprite 1.3: dónde va un borde alrededor (afuera) o por
//! dentro de lo pintado, según qué vecinos de la matriz de 3×3 cuentan. Puro: trabaja sobre una
//! función "vacío" (el filtro de la app la arma con el cel y el color de fondo; una fuente pixel,
//! con la tinta de un glifo). Medido contra Aseprite en `tests/outline_aseprite.rs`.

use cs_core::Rgba;
use serde::{Deserialize, Serialize};

/// Los vecinos que cuentan: 9 bits por filas desde arriba a la izquierda (el bit de `(dx, dy)` es
/// `(dy + 1) * 3 + (dx + 1)`), como el `matrix` de Aseprite. El del centro (el píxel mismo)
/// siempre está apagado.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutlineMatrix(u16);

impl OutlineMatrix {
    /// El bit del centro.
    const CENTER: u16 = 1 << 4;
    /// Los 4 de los lados (Aseprite: `0252`).
    pub const CIRCLE: Self = Self(0o252);
    /// Los 8 de alrededor (`0757`).
    pub const SQUARE: Self = Self(0o757);
    /// Izquierda y derecha (`0050`).
    pub const HORIZONTAL: Self = Self(0o050);
    /// Arriba y abajo (`0202`).
    pub const VERTICAL: Self = Self(0o202);

    /// Desde los bits de Aseprite (sin el centro ni los de más).
    pub fn from_bits(bits: u16) -> Self {
        Self(bits & 0o777 & !Self::CENTER)
    }

    pub fn bits(self) -> u16 {
        self.0
    }

    fn bit(dx: i32, dy: i32) -> Option<u16> {
        ((-1..=1).contains(&dx) && (-1..=1).contains(&dy) && (dx, dy) != (0, 0))
            .then(|| 1 << ((dy + 1) * 3 + (dx + 1)))
    }

    /// Si cuenta el vecino en `(dx, dy)` (de -1 a 1; el centro, nunca).
    pub fn get(self, dx: i32, dy: i32) -> bool {
        Self::bit(dx, dy).is_some_and(|b| self.0 & b != 0)
    }

    /// La matriz con el vecino `(dx, dy)` cambiado (el centro no cambia).
    pub fn toggled(self, dx: i32, dy: i32) -> Self {
        Self(self.0 ^ Self::bit(dx, dy).unwrap_or(0))
    }

    /// El preset que es, si es uno (si no, es una matriz a mano: "custom").
    pub fn preset(self) -> Option<OutlinePreset> {
        OutlinePreset::ALL.into_iter().find(|p| p.matrix() == self)
    }

    /// Los desplazamientos de los vecinos que cuentan.
    fn offsets(self) -> impl Iterator<Item = (i32, i32)> {
        (-1..=1)
            .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
            .filter(move |&(dx, dy)| self.get(dx, dy))
    }
}

impl Default for OutlineMatrix {
    /// Circle, como Aseprite.
    fn default() -> Self {
        Self::CIRCLE
    }
}

/// Las formas de la matriz con nombre (los botones de Aseprite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutlinePreset {
    Circle,
    Square,
    Horizontal,
    Vertical,
}

impl OutlinePreset {
    pub const ALL: [OutlinePreset; 4] = [
        OutlinePreset::Circle,
        OutlinePreset::Square,
        OutlinePreset::Horizontal,
        OutlinePreset::Vertical,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OutlinePreset::Circle => "Circle",
            OutlinePreset::Square => "Square",
            OutlinePreset::Horizontal => "Horizontal",
            OutlinePreset::Vertical => "Vertical",
        }
    }

    pub fn matrix(self) -> OutlineMatrix {
        match self {
            OutlinePreset::Circle => OutlineMatrix::CIRCLE,
            OutlinePreset::Square => OutlineMatrix::SQUARE,
            OutlinePreset::Horizontal => OutlineMatrix::HORIZONTAL,
            OutlinePreset::Vertical => OutlineMatrix::VERTICAL,
        }
    }
}

/// Dónde va el borde.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutlinePlace {
    /// En lo vacío que toca lo pintado.
    #[default]
    Outside,
    /// En lo pintado que toca lo vacío.
    Inside,
}

/// La forma de un outline: qué vecinos cuentan, de qué lado va y si los bordes envuelven (Tiled:
/// el vecino de afuera es el del otro lado; si no, el del borde más cercano, como Aseprite).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OutlineShape {
    pub matrix: OutlineMatrix,
    pub place: OutlinePlace,
    pub wrap_x: bool,
    pub wrap_y: bool,
}

/// Dónde va el borde en una imagen de `width`×`height`: un `bool` por píxel, por filas. `empty`
/// dice si un píxel (siempre dentro de la imagen) es vacío: transparente o el color de fondo.
/// Outside marca los vacíos con algún vecino de la matriz no vacío; Inside, los no vacíos con
/// algún vecino vacío.
pub fn outline_mask(
    width: u32,
    height: u32,
    empty: impl Fn(i32, i32) -> bool,
    shape: &OutlineShape,
) -> Vec<bool> {
    let (w, h) = (width as i32, height as i32);
    let mut out = vec![false; (width as usize) * (height as usize)];
    if w == 0 || h == 0 {
        return out;
    }
    let offsets: Vec<(i32, i32)> = shape.matrix.offsets().collect();
    let fold = |v: i32, len: i32, wrap: bool| {
        if wrap {
            v.rem_euclid(len)
        } else {
            v.clamp(0, len - 1)
        }
    };
    let empty_at = |x: i32, y: i32| empty(fold(x, w, shape.wrap_x), fold(y, h, shape.wrap_y));
    for y in 0..h {
        for x in 0..w {
            let here = empty(x, y);
            let hit = match shape.place {
                OutlinePlace::Outside => {
                    here && offsets.iter().any(|&(dx, dy)| !empty_at(x + dx, y + dy))
                }
                OutlinePlace::Inside => {
                    !here && offsets.iter().any(|&(dx, dy)| empty_at(x + dx, y + dy))
                }
            };
            out[(y * w + x) as usize] = hit;
        }
    }
    out
}

/// Qué canales de un color cambian (los botones R, G, B y A de los filtros de Aseprite).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Channels {
    pub r: bool,
    pub g: bool,
    pub b: bool,
    pub a: bool,
}

impl Channels {
    /// Los cuatro (el Outline de Aseprite).
    pub const RGBA: Self = Self {
        r: true,
        g: true,
        b: true,
        a: true,
    };
}

impl Default for Channels {
    /// Los de color (Color Curve, como en Aseprite).
    fn default() -> Self {
        Self {
            r: true,
            g: true,
            b: true,
            a: false,
        }
    }
}

/// `pixel` con los canales elegidos tomados de `color` (los demás, como estaban).
pub fn paint_channels(pixel: Rgba, color: Rgba, channels: Channels) -> Rgba {
    let pick = |on: bool, new: u8, old: u8| if on { new } else { old };
    Rgba::new(
        pick(channels.r, color.r, pixel.r),
        pick(channels.g, color.g, pixel.g),
        pick(channels.b, color.b, pixel.b),
        pick(channels.a, color.a, pixel.a),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La máscara como filas de texto ('k' = borde), sobre `rows` ('.' = vacío).
    fn run(rows: &[&str], shape: OutlineShape) -> Vec<String> {
        let (w, h) = (rows[0].len(), rows.len());
        let empty = |x: i32, y: i32| rows[y as usize].as_bytes()[x as usize] == b'.';
        let mask = outline_mask(w as u32, h as u32, empty, &shape);
        (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        if mask[y * w + x] {
                            'k'
                        } else {
                            rows[y].as_bytes()[x] as char
                        }
                    })
                    .collect()
            })
            .collect()
    }

    fn shape(matrix: OutlineMatrix, place: OutlinePlace) -> OutlineShape {
        OutlineShape {
            matrix,
            place,
            ..Default::default()
        }
    }

    #[test]
    fn the_matrix_uses_aseprites_bits_and_never_the_center() {
        assert_eq!(OutlineMatrix::CIRCLE.bits(), 170);
        assert!(OutlineMatrix::CIRCLE.get(0, -1) && !OutlineMatrix::CIRCLE.get(1, 1));
        assert!(OutlineMatrix::SQUARE.get(-1, -1) && !OutlineMatrix::SQUARE.get(0, 0));
        assert_eq!(OutlineMatrix::from_bits(1).bits(), 1, "top left is bit 0");
        assert!(OutlineMatrix::from_bits(1).get(-1, -1));
        assert_eq!(OutlineMatrix::from_bits(0o777), OutlineMatrix::SQUARE);
        assert_eq!(OutlineMatrix::CIRCLE.toggled(0, 0), OutlineMatrix::CIRCLE);
        assert_eq!(OutlineMatrix::default(), OutlineMatrix::CIRCLE);
    }

    #[test]
    fn toggling_a_cell_leaves_the_preset() {
        let m = OutlineMatrix::CIRCLE;
        assert_eq!(m.preset(), Some(OutlinePreset::Circle));
        let custom = m.toggled(1, 1);
        assert_eq!(custom.preset(), None);
        assert_eq!(custom.toggled(1, 1), m);
        let square = [(-1, -1), (1, -1), (-1, 1), (1, 1)]
            .into_iter()
            .fold(m, |m, (dx, dy)| m.toggled(dx, dy));
        assert_eq!(square.preset(), Some(OutlinePreset::Square));
        for p in OutlinePreset::ALL {
            assert_eq!(p.matrix().preset(), Some(p));
        }
    }

    #[test]
    fn outside_and_inside_around_a_dot() {
        let dot = [".....", ".....", "..#..", ".....", "....."];
        assert_eq!(
            run(&dot, shape(OutlineMatrix::CIRCLE, OutlinePlace::Outside)),
            [".....", "..k..", ".k#k.", "..k..", "....."]
        );
        assert_eq!(
            run(&dot, shape(OutlineMatrix::SQUARE, OutlinePlace::Outside)),
            [".....", ".kkk.", ".k#k.", ".kkk.", "....."]
        );
        assert_eq!(
            run(
                &dot,
                shape(OutlineMatrix::HORIZONTAL, OutlinePlace::Outside)
            ),
            [".....", ".....", ".k#k.", ".....", "....."]
        );
        assert_eq!(
            run(&dot, shape(OutlineMatrix::CIRCLE, OutlinePlace::Inside)),
            [".....", ".....", "..k..", ".....", "....."]
        );
    }

    #[test]
    fn the_canvas_edge_clamps_unless_tiled() {
        let edge = ["#..", "...", "..."];
        // Sin Tiled, afuera es el píxel del borde: el de la esquina no se marca por dentro.
        let full = ["##", "##"];
        assert_eq!(
            run(&full, shape(OutlineMatrix::SQUARE, OutlinePlace::Inside)),
            ["##", "##"]
        );
        assert_eq!(
            run(&edge, shape(OutlineMatrix::CIRCLE, OutlinePlace::Outside)),
            ["#k.", "k..", "..."]
        );
        let tiled = OutlineShape {
            wrap_x: true,
            wrap_y: true,
            ..shape(OutlineMatrix::CIRCLE, OutlinePlace::Outside)
        };
        assert_eq!(run(&edge, tiled), ["#kk", "k..", "k.."]);
        let only_x = OutlineShape {
            wrap_y: false,
            ..tiled
        };
        assert_eq!(run(&edge, only_x), ["#kk", "k..", "..."]);
    }

    #[test]
    fn an_empty_image_has_no_outline() {
        assert!(outline_mask(0, 3, |_, _| true, &OutlineShape::default()).is_empty());
        let none = outline_mask(3, 3, |_, _| true, &OutlineShape::default());
        assert!(none.iter().all(|&b| !b));
    }

    #[test]
    fn paint_channels_keeps_the_others() {
        let red = Rgba::rgb(255, 0, 0);
        let half_green = Rgba::new(0, 255, 0, 128);
        assert_eq!(paint_channels(red, half_green, Channels::RGBA), half_green);
        assert_eq!(
            paint_channels(red, half_green, Channels::default()),
            Rgba::new(0, 255, 0, 255)
        );
        let green_only = Channels {
            r: false,
            g: true,
            b: false,
            a: false,
        };
        assert_eq!(
            paint_channels(red, half_green, green_only),
            Rgba::new(255, 255, 0, 255)
        );
        let alpha_only = Channels {
            r: false,
            g: false,
            b: false,
            a: true,
        };
        assert_eq!(
            paint_channels(Rgba::TRANSPARENT, Rgba::new(0, 255, 0, 255), alpha_only),
            Rgba::new(0, 0, 0, 255),
            "a transparent pixel turns black, as in Aseprite"
        );
    }
}
