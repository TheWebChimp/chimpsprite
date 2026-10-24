//! Reglas con Pixel UI, como en el mockup (`.design/Design.aseprite`, frame 3):
//!
//! - **Grosor:** 12. Diez de relleno (`background`), el brillo (`highlight`) y la línea oscura
//!   (`ruler_tick`) contra el lienzo.
//! - **Marcas grandes:** una columna (o fila) `ruler_tick` de todo el relleno y el brillo, con el
//!   número 1 px después: a la derecha en la regla de arriba y encima en la de la izquierda.
//! - **Marcas chicas:** 4 px contra el lienzo; la punta, del color del lienzo (`canvas`).
//! - **Números:** las cifras del skin (`digit.0` … `digit.9`), con 1 px entre cifra y cifra. El
//!   mockup no tiene signo menos: es un guion de 2×1 en el color de las marcas.
//! - **Esquina:** la carita (`ruler.corner`).
//!
//! Todo se arma con mallas de color liso y piezas a tamaño de skin, alineadas a píxeles.

use eframe::egui::{Color32, Mesh, Pos2, Rect, Shape, TextureId, Vec2, pos2, vec2};

use crate::skin::{Skin, WidgetState};

/// Grosor de las reglas: el relleno, el brillo y la línea oscura.
pub const RULER_PX: f32 = FILL + 2.0;
/// Separación mínima entre marcas grandes: la del mockup, una cada 5 píxeles del sprite a zoom 8.
pub const MAJOR_MIN_PX: f32 = 40.0;
/// Relleno de la regla, sin el brillo ni la línea.
const FILL: f32 = 10.0;
/// Fila (o columna) de la punta clara de las marcas chicas; de ahí al brillo son oscuras.
const MINOR_TIP: f32 = 7.0;
/// Del borde de la regla a los números (arriba en la de arriba; a la izquierda en la otra).
const LABEL_INSET: f32 = 1.0;
/// Aire entre la marca y su número, y entre cifra y cifra.
const GAP: f32 = 1.0;
/// Alto de las cifras y su ancho si el skin no las trae.
const DIGIT_HEIGHT: f32 = 5.0;
const FALLBACK_DIGIT_WIDTH: f32 = 3.0;
/// El signo menos: un guion en la fila del medio de las cifras.
const MINUS_WIDTH: f32 = 2.0;
const MINUS_ROW: f32 = 2.0;

const CORNER_PIECE: &str = "ruler.corner";
const DIGIT_PIECES: [&str; 10] = [
    "digit.0", "digit.1", "digit.2", "digit.3", "digit.4", "digit.5", "digit.6", "digit.7",
    "digit.8", "digit.9",
];

/// Una pieza del skin a su tamaño.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Glyph {
    texture: TextureId,
    size: Vec2,
}

impl Glyph {
    fn of(skin: &Skin, name: &str) -> Option<Glyph> {
        skin.piece(name, WidgetState::Normal).map(|p| Glyph {
            texture: p.texture.id(),
            size: vec2(p.size[0] as f32, p.size[1] as f32),
        })
    }

    fn shape(self, at: Pos2) -> Shape {
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        Shape::image(
            self.texture,
            Rect::from_min_size(at, self.size),
            uv,
            Color32::WHITE,
        )
    }
}

/// Las piezas y los colores de las reglas con Pixel UI. La app lo arma del skin en cada frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelRulers {
    digits: [Option<Glyph>; 10],
    corner: Option<Glyph>,
    fill: Color32,
    tick: Color32,
    tip: Color32,
    highlight: Color32,
    /// La posición del puntero en las reglas.
    pointer: Color32,
}

impl PixelRulers {
    pub fn from_skin(skin: &Skin) -> Self {
        let colors = skin.colors();
        Self {
            digits: DIGIT_PIECES.map(|name| Glyph::of(skin, name)),
            corner: Glyph::of(skin, CORNER_PIECE),
            fill: colors.background,
            tick: colors.ruler_tick,
            tip: colors.canvas,
            highlight: colors.highlight,
            pointer: colors.accent,
        }
    }

    /// La regla de arriba (sin la esquina): de ahí salen las marcas y los números.
    pub fn top_strip(rect: Rect) -> Rect {
        Rect::from_min_max(
            pos2(rect.left() + RULER_PX, rect.top()),
            pos2(rect.right(), rect.top() + FILL + 1.0),
        )
    }

    /// La regla de la izquierda (sin la esquina).
    pub fn left_strip(rect: Rect) -> Rect {
        Rect::from_min_max(
            pos2(rect.left(), rect.top() + RULER_PX),
            pos2(rect.left() + FILL + 1.0, rect.bottom()),
        )
    }

    /// El fondo de las dos reglas de `rect` (el área del lienzo): relleno, brillo, la línea
    /// oscura contra el lienzo y la carita en la esquina.
    pub fn frame_shapes(&self, rect: Rect) -> Vec<Shape> {
        let (left, top) = (rect.left(), rect.top());
        let line = FILL + 1.0;
        let mut mesh = Mesh::default();
        // Relleno.
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(left, top), pos2(rect.right(), top + FILL)),
            self.fill,
        );
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(left, top), pos2(left + FILL, rect.bottom())),
            self.fill,
        );
        // Brillo, desde la línea de la esquina.
        mesh.add_colored_rect(
            Rect::from_min_max(
                pos2(left + RULER_PX, top + FILL),
                pos2(rect.right(), top + line),
            ),
            self.highlight,
        );
        mesh.add_colored_rect(
            Rect::from_min_max(
                pos2(left + FILL, top + RULER_PX),
                pos2(left + line, rect.bottom()),
            ),
            self.highlight,
        );
        // La línea oscura, de punta a punta.
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(left, top + line), pos2(rect.right(), top + RULER_PX)),
            self.tick,
        );
        mesh.add_colored_rect(
            Rect::from_min_max(pos2(left + line, top), pos2(left + RULER_PX, rect.bottom())),
            self.tick,
        );
        let corner_rect = Rect::from_min_size(rect.min, Vec2::splat(line));
        let mut shapes = vec![];
        match self.corner {
            Some(corner) => {
                shapes.push(Shape::mesh(mesh));
                shapes.push(corner.shape(rect.min));
            }
            None => {
                mesh.add_colored_rect(corner_rect, self.tip);
                shapes.push(Shape::mesh(mesh));
            }
        }
        shapes
    }

    /// Una marca de la regla de arriba en la columna `x`; `top` es el borde de la regla.
    pub fn top_tick(&self, x: f32, top: f32, major: bool) -> Shape {
        let mut mesh = Mesh::default();
        let column =
            |y0: f32, y1: f32| Rect::from_min_max(pos2(x, top + y0), pos2(x + 1.0, top + y1));
        if major {
            mesh.add_colored_rect(column(0.0, FILL + 1.0), self.tick);
        } else {
            mesh.add_colored_rect(column(MINOR_TIP, MINOR_TIP + 1.0), self.tip);
            mesh.add_colored_rect(column(MINOR_TIP + 1.0, FILL + 1.0), self.tick);
        }
        Shape::mesh(mesh)
    }

    /// Una marca de la regla de la izquierda en la fila `y`; `left` es el borde de la regla.
    pub fn left_tick(&self, y: f32, left: f32, major: bool) -> Shape {
        let mut mesh = Mesh::default();
        let row =
            |x0: f32, x1: f32| Rect::from_min_max(pos2(left + x0, y), pos2(left + x1, y + 1.0));
        if major {
            mesh.add_colored_rect(row(0.0, FILL + 1.0), self.tick);
        } else {
            mesh.add_colored_rect(row(MINOR_TIP, MINOR_TIP + 1.0), self.tip);
            mesh.add_colored_rect(row(MINOR_TIP + 1.0, FILL + 1.0), self.tick);
        }
        Shape::mesh(mesh)
    }

    /// El número de la marca grande de la columna `x` de la regla de arriba: 1 px a la derecha.
    pub fn top_label(&self, value: i32, x: f32, top: f32) -> Vec<Shape> {
        self.label(&value.to_string(), pos2(x + 1.0 + GAP, top + LABEL_INSET))
    }

    /// El número de la marca grande de la fila `y` de la regla de la izquierda: termina 1 px
    /// arriba. Si a lo ancho no entra en el relleno, las cifras van una debajo de la otra.
    pub fn left_label(&self, value: i32, y: f32, left: f32) -> Vec<Shape> {
        let text = value.to_string();
        let bottom = y - GAP;
        if self.width(&text) <= FILL - LABEL_INSET {
            return self.label(&text, pos2(left + LABEL_INSET, bottom - DIGIT_HEIGHT));
        }
        let n = text.chars().count() as f32;
        let mut at = pos2(
            left + LABEL_INSET,
            bottom - n * DIGIT_HEIGHT - (n - 1.0) * GAP,
        );
        let mut shapes = Vec::new();
        for c in text.chars() {
            shapes.extend(self.glyph(c, at));
            at.y += DIGIT_HEIGHT + GAP;
        }
        shapes
    }

    /// La raya de la posición del puntero en las dos reglas.
    pub fn pointer_shapes(&self, rect: Rect, p: Pos2) -> Shape {
        let (x, y) = (p.x.floor(), p.y.floor());
        let mut mesh = Mesh::default();
        let top = Self::top_strip(rect);
        if top.x_range().contains(x) {
            mesh.add_colored_rect(
                Rect::from_min_max(pos2(x, top.top()), pos2(x + 1.0, top.top() + FILL)),
                self.pointer,
            );
        }
        let left = Self::left_strip(rect);
        if left.y_range().contains(y) {
            mesh.add_colored_rect(
                Rect::from_min_max(pos2(left.left(), y), pos2(left.left() + FILL, y + 1.0)),
                self.pointer,
            );
        }
        Shape::mesh(mesh)
    }

    /// Ancho de `text` (cifras y "-") con 1 px entre caracteres.
    fn width(&self, text: &str) -> f32 {
        let widths: Vec<f32> = text.chars().map(|c| self.glyph_width(c)).collect();
        widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32
    }

    fn glyph_width(&self, c: char) -> f32 {
        match c.to_digit(10) {
            Some(d) => self.digits[d as usize].map_or(FALLBACK_DIGIT_WIDTH, |g| g.size.x),
            None => MINUS_WIDTH,
        }
    }

    /// Las formas de `text` en una fila, desde `at` (arriba a la izquierda).
    fn label(&self, text: &str, mut at: Pos2) -> Vec<Shape> {
        let mut shapes = Vec::new();
        for c in text.chars() {
            shapes.extend(self.glyph(c, at));
            at.x += self.glyph_width(c) + GAP;
        }
        shapes
    }

    /// Una cifra (su pieza; sin ella, nada) o el guion del menos.
    fn glyph(&self, c: char, at: Pos2) -> Option<Shape> {
        match c.to_digit(10) {
            Some(d) => self.digits[d as usize].map(|g| g.shape(at)),
            None => {
                let mut mesh = Mesh::default();
                let dash = Rect::from_min_size(at + vec2(0.0, MINUS_ROW), vec2(MINUS_WIDTH, 1.0));
                mesh.add_colored_rect(dash, self.tick);
                Some(Shape::mesh(mesh))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use super::*;

    fn rulers() -> (egui::Context, Skin, PixelRulers) {
        let ctx = egui::Context::default();
        let skin = Skin::embedded(&ctx);
        let rulers = PixelRulers::from_skin(&skin);
        (ctx, skin, rulers)
    }

    fn texture(skin: &Skin, name: &str) -> TextureId {
        skin.piece(name, WidgetState::Normal).unwrap().texture.id()
    }

    /// Las piezas dibujadas: textura y rect.
    fn images(shapes: &[Shape]) -> Vec<(TextureId, Rect)> {
        shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Mesh(m) if m.texture_id != TextureId::default() => {
                    Some((m.texture_id, m.calc_bounds()))
                }
                _ => None,
            })
            .collect()
    }

    fn rects(shapes: &[Shape]) -> Vec<(Color32, Rect)> {
        shapes
            .iter()
            .flat_map(crate::pixel_ui::colored_rects)
            .collect()
    }

    #[test]
    fn the_label_10_is_the_digits_1_and_0_one_pixel_apart() {
        let (_ctx, skin, rulers) = rulers();
        // La marca del 10 en la columna 100 de una regla que empieza en la fila 20.
        let drawn = images(&rulers.top_label(10, 100.0, 20.0));
        assert_eq!(
            drawn,
            vec![
                (
                    texture(&skin, "digit.1"),
                    Rect::from_min_size(pos2(102.0, 21.0), vec2(1.0, 5.0))
                ),
                (
                    texture(&skin, "digit.0"),
                    Rect::from_min_size(pos2(104.0, 21.0), vec2(3.0, 5.0))
                ),
            ],
            "like the mockup's 10: tick, 1 px of air, the 1, 1 px and the 0"
        );
    }

    #[test]
    fn negative_labels_start_with_a_dash_in_the_tick_color() {
        let (_ctx, skin, rulers) = rulers();
        let shapes = rulers.top_label(-5, 0.0, 0.0);
        let dash = rects(&shapes);
        let tick = skin.colors().ruler_tick;
        assert_eq!(
            dash,
            vec![(tick, Rect::from_min_size(pos2(2.0, 3.0), vec2(2.0, 1.0)))]
        );
        assert_eq!(
            images(&shapes),
            vec![(
                texture(&skin, "digit.5"),
                Rect::from_min_size(pos2(5.0, 1.0), vec2(3.0, 5.0))
            )]
        );
    }

    #[test]
    fn left_labels_end_one_pixel_above_their_tick_and_stack_when_wide() {
        let (_ctx, skin, rulers) = rulers();
        // El "5" del mockup: columna 69 (la regla empieza en la 68) y filas 140..144; la marca,
        // en la 146.
        let five = images(&rulers.left_label(5, 146.0, 68.0));
        assert_eq!(
            five,
            vec![(
                texture(&skin, "digit.5"),
                Rect::from_min_size(pos2(69.0, 140.0), vec2(3.0, 5.0))
            )]
        );
        // "200" mide 11: no entra a lo ancho y va en columna, terminando en la misma fila.
        let tall = images(&rulers.left_label(200, 146.0, 68.0));
        let tops: Vec<f32> = tall.iter().map(|(_, r)| r.top()).collect();
        assert_eq!(tops, vec![128.0, 134.0, 140.0]);
        assert!(tall.iter().all(|(_, r)| r.left() == 69.0));
    }

    #[test]
    fn ticks_have_the_mockup_lengths_and_colors() {
        let (_ctx, skin, rulers) = rulers();
        let c = skin.colors();
        // La marca grande del 5 del mockup: columna 119, filas 98..108.
        assert_eq!(
            rects(&[rulers.top_tick(119.0, 98.0, true)]),
            vec![(
                c.ruler_tick,
                Rect::from_min_max(pos2(119.0, 98.0), pos2(120.0, 109.0))
            )]
        );
        // Una chica: la punta (189,164,153) en la fila 105 y oscura de la 106 a la 108.
        assert_eq!(
            rects(&[rulers.top_tick(87.0, 98.0, false)]),
            vec![
                (
                    c.canvas,
                    Rect::from_min_max(pos2(87.0, 105.0), pos2(88.0, 106.0))
                ),
                (
                    c.ruler_tick,
                    Rect::from_min_max(pos2(87.0, 106.0), pos2(88.0, 109.0))
                ),
            ]
        );
        // En la de la izquierda, lo mismo de costado: la chica de la fila 114 del mockup.
        assert_eq!(
            rects(&[rulers.left_tick(114.0, 68.0, false)]),
            vec![
                (
                    c.canvas,
                    Rect::from_min_max(pos2(75.0, 114.0), pos2(76.0, 115.0))
                ),
                (
                    c.ruler_tick,
                    Rect::from_min_max(pos2(76.0, 114.0), pos2(79.0, 115.0))
                ),
            ]
        );
    }

    #[test]
    fn the_frame_is_fill_highlight_line_and_the_smiley_corner() {
        let (_ctx, skin, rulers) = rulers();
        let c = skin.colors();
        // El área de las reglas del mockup: desde (68,98).
        let rect = Rect::from_min_max(pos2(68.0, 98.0), pos2(763.0, 546.0));
        let shapes = rulers.frame_shapes(rect);
        assert_eq!(
            images(&shapes),
            vec![(
                texture(&skin, "ruler.corner"),
                Rect::from_min_size(pos2(68.0, 98.0), vec2(11.0, 11.0))
            )]
        );
        let drawn = rects(&shapes);
        for expected in [
            // Brillo en la fila 108 desde la 80, y en la columna 78 desde la 110.
            (
                c.highlight,
                Rect::from_min_max(pos2(80.0, 108.0), pos2(763.0, 109.0)),
            ),
            (
                c.highlight,
                Rect::from_min_max(pos2(78.0, 110.0), pos2(79.0, 546.0)),
            ),
            // La línea oscura en la fila 109 y en la columna 79, enteras.
            (
                c.ruler_tick,
                Rect::from_min_max(pos2(68.0, 109.0), pos2(763.0, 110.0)),
            ),
            (
                c.ruler_tick,
                Rect::from_min_max(pos2(79.0, 98.0), pos2(80.0, 546.0)),
            ),
        ] {
            assert!(drawn.contains(&expected), "{expected:?} in {drawn:?}");
        }
        assert_eq!(c.ruler_tick, Color32::from_rgb(136, 110, 106));
        assert_eq!(c.background, Color32::from_rgb(221, 206, 191), "the fill");
    }
}
