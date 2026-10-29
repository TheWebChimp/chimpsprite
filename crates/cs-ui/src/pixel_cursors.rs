//! Con Pixel UI, los cursores del diseño sobre el lienzo: la cruz de las herramientas de dibujo,
//! las esquinas de las de selección, el cuentagotas y la flecha de rotar. Los dibujos son piezas
//! del skin ([`CursorSet`]); las esquinas se arman acá. Cada píxel del dibujo mide un punto.

use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

use crate::skin::{Skin, WidgetState};
use crate::theme;

/// La tinta oscura de los cursores (la del diseño); la clara es el blanco.
pub(crate) const INK: Color32 = Color32::from_rgb(0x05, 0x04, 0x03);

/// Cursor a dibujar a mano sobre el lienzo (el del sistema se oculta).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelCursor {
    /// La cruz con el centro suelto: lápiz, goma, formas…
    Cross,
    /// Las 4 esquinas alrededor del píxel bajo el puntero: herramientas de selección.
    Marquee,
    /// El cuentagotas, con el color de debajo al lado.
    Eyedropper,
    /// La flecha curva de rotar (por fuera de una esquina de la caja). Trae su contorno blanco:
    /// no se invierte.
    Rotate,
}

/// Un cursor del skin: sus píxeles con el desfase desde el punto que cae en el puntero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bitmap {
    pixels: Vec<(i32, i32, Color32)>,
}

impl Bitmap {
    /// Los píxeles con algo de `rgba` (`size`, RGBA sin premultiplicar), con `hot` en el origen.
    fn new(size: [u32; 2], rgba: &[u8], hot: (u32, u32)) -> Self {
        let mut pixels = Vec::new();
        for y in 0..size[1] {
            for x in 0..size[0] {
                let i = ((y * size[0] + x) * 4) as usize;
                let c = &rgba[i..i + 4];
                if c[3] > 0 {
                    let color = Color32::from_rgb(c[0], c[1], c[2]);
                    pixels.push((x as i32 - hot.0 as i32, y as i32 - hot.1 as i32, color));
                }
            }
        }
        Self { pixels }
    }

    /// Cada píxel en pantalla con el punto caliente en el punto de `pointer`, y su color.
    fn placed(&self, pointer: Pos2) -> impl Iterator<Item = (Rect, Color32)> + '_ {
        let origin = point_at(pointer);
        self.pixels.iter().map(move |&(x, y, c)| {
            let min = origin + Vec2::new(x as f32, y as f32);
            (Rect::from_min_size(min, Vec2::splat(1.0)), c)
        })
    }
}

/// Los cursores del skin (`cursor.cross`, `cursor.eyedropper`, `cursor.rotate`). En la cruz lo
/// oscuro es tinta y lo claro su contrario: cada píxel se invierte según lo que tenga debajo
/// (como en Aseprite). El cuentagotas y el de rotar traen su contorno blanco (siempre se ven) y
/// se dibujan tal cual. Sin la pieza, el cursor del sistema.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CursorSet {
    cross: Option<Bitmap>,
    eyedropper: Option<Bitmap>,
    rotate: Option<Bitmap>,
}

impl CursorSet {
    pub fn from_skin(skin: &Skin) -> Self {
        Self::from_pieces(|name| {
            skin.piece(name, WidgetState::Normal)
                .map(|p| (p.size, p.rgba.clone()))
        })
    }

    /// Con `piece` que da el tamaño y los píxeles de cada pieza (sin egui, para los tests).
    pub fn from_pieces(piece: impl Fn(&str) -> Option<([u32; 2], Vec<u8>)>) -> Self {
        // El punto caliente: el centro de la cruz y de la flecha; la punta del cuentagotas, abajo
        // a la izquierda, un píxel adentro de su contorno blanco.
        let load = |name: &str, hot: fn([u32; 2]) -> (u32, u32)| {
            piece(name).map(|(size, rgba)| Bitmap::new(size, &rgba, hot(size)))
        };
        Self {
            cross: load("cursor.cross", |[w, h]| (w / 2, h / 2)),
            eyedropper: load("cursor.eyedropper", |[_, h]| (1, h.saturating_sub(2))),
            rotate: load("cursor.rotate", |[w, h]| (w / 2, h / 2)),
        }
    }

    /// Si se puede dibujar `cursor` (las esquinas de selección no son una pieza: siempre).
    pub fn has(&self, cursor: PixelCursor) -> bool {
        match cursor {
            PixelCursor::Cross => self.cross.is_some(),
            PixelCursor::Marquee => true,
            PixelCursor::Eyedropper => self.eyedropper.is_some(),
            PixelCursor::Rotate => self.rotate.is_some(),
        }
    }
}

/// Si `c` es tinta (lo oscuro de la cruz y el cuentagotas).
fn is_ink(c: Color32) -> bool {
    (c.r() as u32 + c.g() as u32 + c.b() as u32) < 3 * 128
}

/// Largo de cada brazo de las esquinas de `Marquee`, contando el vértice.
const ARM: f32 = 3.0;

/// El punto de pantalla que contiene a `p`.
fn point_at(p: Pos2) -> Pos2 {
    p.floor()
}

/// La tinta que contrasta con `under`: la oscura sobre lo claro y el blanco sobre lo oscuro.
pub fn ink_over(under: Color32) -> Color32 {
    let luma = 0.299 * under.r() as f32 + 0.587 * under.g() as f32 + 0.114 * under.b() as f32;
    if luma >= 128.0 { INK } else { Color32::WHITE }
}

/// Lo contrario de [`ink_over`] (los píxeles `o`).
fn paper_over(under: Color32) -> Color32 {
    if ink_over(under) == INK {
        Color32::WHITE
    } else {
        INK
    }
}

/// Los píxeles de `bitmap` en `pointer`; `true` si son de tinta.
fn inked(bitmap: Option<&Bitmap>, pointer: Pos2) -> Vec<(Rect, bool)> {
    bitmap
        .map(|b| b.placed(pointer).map(|(r, c)| (r, is_ink(c))).collect())
        .unwrap_or_default()
}

/// Las 4 esquinas en L por fuera de `cell` (el píxel del sprite bajo el puntero, en pantalla):
/// se ajustan al píxel con cualquier zoom. Si el píxel mide menos de un punto (zoom muy lejano),
/// rodean el punto de `pointer`. Un rect por píxel (cada uno se invierte por su cuenta).
pub fn marquee_pixels(cell: Rect, pointer: Pos2) -> Vec<Rect> {
    let inner = if cell.width() < 1.0 || cell.height() < 1.0 {
        Rect::from_min_size(point_at(pointer), Vec2::splat(1.0))
    } else {
        cell
    };
    let mut out = Vec::new();
    // Por esquina: la x y la y del vértice (fuera de `inner`) y hacia dónde salen los brazos.
    for (vx, dx) in [(inner.left() - 1.0, -1.0), (inner.right(), 1.0)] {
        for (vy, dy) in [(inner.top() - 1.0, -1.0), (inner.bottom(), 1.0)] {
            let span = |v: f32, d: f32| {
                if d < 0.0 {
                    (v - (ARM - 1.0), v + 1.0)
                } else {
                    (v, v + ARM)
                }
            };
            let (x0, x1) = span(vx, dx);
            let (y0, y1) = span(vy, dy);
            let px = |x: f32, y: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::splat(1.0));
            let mut x = x0;
            while x < x1 {
                out.push(px(x, vy));
                x += 1.0;
            }
            // El vértice ya está en la fila.
            let mut y = y0;
            while y < y1 {
                if y != vy {
                    out.push(px(vx, y));
                }
                y += 1.0;
            }
        }
    }
    out
}

/// Dibuja `cursor` en `pointer`. `cell` es el píxel del sprite bajo el puntero en pantalla (para
/// `Marquee`), `color` el que tomaría el cuentagotas y `under` el color que hay debajo de un
/// punto de pantalla (para invertir cada píxel).
pub fn draw(
    painter: &Painter,
    cursors: &CursorSet,
    cursor: PixelCursor,
    pointer: Pos2,
    cell: Rect,
    color: Option<cs_core::Rgba>,
    under: impl Fn(Pos2) -> Color32,
) {
    let pixels = match cursor {
        PixelCursor::Cross => inked(cursors.cross.as_ref(), pointer),
        PixelCursor::Marquee => marquee_pixels(cell, pointer)
            .into_iter()
            .map(|r| (r, true))
            .collect(),
        PixelCursor::Eyedropper => {
            for (r, c) in cursors.eyedropper.iter().flat_map(|b| b.placed(pointer)) {
                painter.rect_filled(r, 0.0, c);
            }
            eyedropper_swatch(painter, pointer, color);
            return;
        }
        PixelCursor::Rotate => {
            for (r, c) in cursors.rotate.iter().flat_map(|b| b.placed(pointer)) {
                painter.rect_filled(r, 0.0, c);
            }
            return;
        }
    };
    for (r, ink) in pixels {
        let below = under(r.center());
        let c = if ink {
            ink_over(below)
        } else {
            paper_over(below)
        };
        painter.rect_filled(r, 0.0, c);
    }
}

/// Abajo a la derecha de la punta del cuentagotas, el color que se tomaría: en un marco negro con
/// el contorno blanco, como el cursor (siempre se ve).
fn eyedropper_swatch(painter: &Painter, pointer: Pos2, color: Option<cs_core::Rgba>) {
    let swatch = Rect::from_min_size(point_at(pointer) + Vec2::new(4.0, 3.0), Vec2::splat(10.0));
    painter.rect_filled(swatch.expand(1.0), 0.0, Color32::WHITE);
    painter.rect_filled(swatch, 0.0, INK);
    let inside = swatch.shrink(1.0);
    match color {
        Some(c) if c.a > 0 => {
            painter.rect_filled(inside, 0.0, Color32::from_rgb(c.r, c.g, c.b));
        }
        _ => {
            painter.rect_filled(inside, 0.0, theme::CHECKER_LIGHT);
            painter.line_segment(
                [inside.left_bottom(), inside.right_top()],
                Stroke::new(1.0, Color32::from_rgb(0xd9, 0x57, 0x63)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Los cursores del skin embebido.
    fn embedded() -> CursorSet {
        let data = crate::skin::decode(crate::skin::EMBEDDED).expect("the embedded skin");
        CursorSet::from_pieces(|name| data.pieces.get(name).map(|p| (p.size, p.rgba.clone())))
    }

    fn cross_pixels(pointer: Pos2) -> Vec<(Rect, bool)> {
        inked(embedded().cross.as_ref(), pointer)
    }

    fn eyedropper_pixels(pointer: Pos2) -> Vec<(Rect, bool)> {
        inked(embedded().eyedropper.as_ref(), pointer)
    }

    /// La cruz del diseño.
    const CROSS: [&str; 7] = [
        "...#...", //
        "...#...", //
        ".......", //
        "##.#.##", //
        ".......", //
        "...#...", //
        "...#...", //
    ];

    /// `rects` pintados como en el diseño: `#` donde hay algo, relativo a `origin`.
    fn ascii(rects: &[Rect], origin: Pos2, size: (usize, usize)) -> Vec<String> {
        (0..size.1)
            .map(|y| {
                (0..size.0)
                    .map(|x| {
                        let p = origin + Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                        if rects.iter().any(|r| r.contains(p)) {
                            '#'
                        } else {
                            '.'
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_cross_is_centered_on_the_pointer() {
        let px = cross_pixels(Pos2::new(10.6, 20.2));
        let center = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::splat(1.0));
        assert!(px.iter().any(|(r, _)| *r == center), "{px:?}");
        let rects: Vec<Rect> = px.iter().map(|(r, _)| *r).collect();
        assert!(px.iter().all(|(_, ink)| *ink));
        assert_eq!(ascii(&rects, Pos2::new(7.0, 17.0), (7, 7)), CROSS);
    }

    #[test]
    fn the_eyedropper_tip_is_on_the_pointer_inside_its_white_outline() {
        let px = eyedropper_pixels(Pos2::new(30.5, 40.5));
        let tip = Rect::from_min_size(Pos2::new(30.0, 40.0), Vec2::splat(1.0));
        assert!(px.iter().any(|(r, ink)| *r == tip && *ink), "{px:?}");
        assert!(px.iter().any(|(_, ink)| !*ink), "the white outline");
        // Entero, como en el diseño: 18×18, con el contorno un píxel alrededor de la punta.
        let bounds = px.iter().fold(Rect::NOTHING, |b, (r, _)| b.union(*r));
        assert_eq!(bounds.size(), Vec2::new(18.0, 18.0));
        assert!(
            px.iter()
                .all(|(r, _)| r.left() >= 29.0 && r.bottom() <= 42.0)
        );
    }

    #[test]
    fn the_eyedropper_keeps_its_own_colors_over_anything() {
        let ctx = eframe::egui::Context::default();
        let skin = Skin::embedded(&ctx);
        let cursors = CursorSet::from_skin(&skin);
        // Lo que dibuja sobre un fondo de color `below`: los rects y sus colores.
        let draw_over = |below: Color32| {
            let out = ctx.run_ui(eframe::egui::RawInput::default(), |ui| {
                let painter = ui.ctx().layer_painter(eframe::egui::LayerId::background());
                draw(
                    &painter,
                    &cursors,
                    PixelCursor::Eyedropper,
                    Pos2::new(30.5, 40.5),
                    Rect::NOTHING,
                    None,
                    |_| below,
                );
            });
            let mut out = out;
            out.textures_delta.clear();
            out.shapes
                .iter()
                .map(|c| format!("{:?}", c.shape))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            draw_over(Color32::BLACK),
            draw_over(Color32::WHITE),
            "no inversion"
        );
    }

    #[test]
    fn the_marquee_corners_hug_the_pixel_under_the_pointer() {
        // Al 300 %: el píxel de 3×3 queda en el hueco, como en el diseño.
        let cell = Rect::from_min_size(Pos2::new(23.0, 17.0), Vec2::splat(3.0));
        let rects = marquee_pixels(cell, Pos2::new(24.5, 18.5));
        assert_eq!(
            ascii(&rects, Pos2::new(20.0, 14.0), (9, 9)),
            [
                "..#...#..",
                "..#...#..",
                "###...###",
                ".........",
                ".........",
                ".........",
                "###...###",
                "..#...#..",
                "..#...#..",
            ]
        );
        // Al 1600 %, alrededor del píxel de 16×16.
        let cell = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::splat(16.0));
        let rects = marquee_pixels(cell, Pos2::new(107.0, 109.0));
        let bounds = rects.iter().fold(Rect::NOTHING, |b, r| b.union(*r));
        assert_eq!(bounds, cell.expand(3.0));
    }

    #[test]
    fn each_pixel_inverts_over_what_is_below() {
        assert_eq!(ink_over(Color32::WHITE), INK);
        assert_eq!(ink_over(Color32::from_rgb(0xde, 0xc9, 0xb6)), INK);
        assert_eq!(ink_over(Color32::BLACK), Color32::WHITE);
        assert_eq!(
            ink_over(Color32::from_rgb(0x30, 0x20, 0x40)),
            Color32::WHITE
        );
        assert_eq!(paper_over(Color32::BLACK), INK);
        assert_eq!(paper_over(Color32::WHITE), Color32::WHITE);
    }

    #[test]
    fn the_marquee_is_one_rect_per_pixel() {
        let cell = Rect::from_min_size(Pos2::new(23.0, 17.0), Vec2::splat(3.0));
        let rects = marquee_pixels(cell, Pos2::new(24.5, 18.5));
        assert_eq!(rects.len(), 4 * 5);
        assert!(rects.iter().all(|r| r.size() == Vec2::splat(1.0)));
    }

    #[test]
    fn far_away_the_marquee_hugs_the_pointer() {
        // Al 50 % el píxel mide medio punto: las esquinas rodean el punto del puntero.
        let cell = Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::splat(0.5));
        let rects = marquee_pixels(cell, Pos2::new(40.3, 40.2));
        assert_eq!(
            ascii(&rects, Pos2::new(37.0, 37.0), (7, 7)),
            [
                "..#.#..", "..#.#..", "###.###", ".......", "###.###", "..#.#..", "..#.#..",
            ]
        );
    }
}
