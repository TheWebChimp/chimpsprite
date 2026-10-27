//! Lo del menú Edit de Aseprite que trabaja sobre lo seleccionado (o, sin selección, la capa
//! entera en el frame actual): Fill, Rotate, Shift, Transform e Invert. Cada uno es un paso de
//! undo; los que levantan lo seleccionado lo dejan flotando como Flip.

use std::f32::consts::FRAC_PI_2;

use cs_commands::Command;
use cs_core::{ColorMode, Mask, PixelValue, Rgba};

use crate::editor::{Editor, ToolKind};

/// Un giro de Edit → Rotate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    Half,
    Clockwise,
    CounterClockwise,
}

impl Turn {
    pub(crate) fn angle(self) -> f32 {
        match self {
            Turn::Half => 2.0 * FRAC_PI_2,
            Turn::Clockwise => FRAC_PI_2,
            Turn::CounterClockwise => -FRAC_PI_2,
        }
    }
}

impl Editor {
    /// La selección o, sin selección, todo el lienzo.
    fn selection_or_all(&self) -> Mask {
        self.selection.clone().unwrap_or_else(|| {
            let s = self.sprite();
            Mask::all(s.width, s.height)
        })
    }

    /// El valor de `c` en el modo de color del sprite (en indexado, el color más parecido de la
    /// paleta).
    pub(crate) fn value_of(&self, c: Rgba) -> PixelValue {
        let sprite = self.sprite();
        match sprite.color_mode {
            _ if c.a == 0 => sprite.color_mode.transparent(),
            ColorMode::Rgba => PixelValue::Rgba(c),
            ColorMode::Indexed { transparent_index } => {
                PixelValue::Index(cs_io::import::nearest_index(sprite, c, transparent_index))
            }
        }
    }

    /// Pinta `pixels` en la capa y el frame actuales (si hay algo).
    fn draw(&mut self, pixels: Vec<(i32, i32, PixelValue)>) -> bool {
        !pixels.is_empty()
            && self.execute(Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            })
    }

    /// Edit → Fill (F): rellena lo seleccionado con el color primario.
    pub fn fill_selection(&mut self) -> bool {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return false;
        }
        let Some(mask) = &self.selection else {
            return false;
        };
        let color = self.primary;
        let pixels = mask.points().map(|(x, y)| (x, y, color)).collect();
        self.named("Fill", |e| e.draw(pixels))
    }

    /// Edit → Shift: corre lo seleccionado (sin selección, la capa entera) 1 píxel en
    /// `(dx, dy)`; lo que sale por un lado entra por el otro, dentro de la caja de la selección.
    pub fn shift_content(&mut self, dx: i32, dy: i32) -> bool {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return false;
        }
        let mask = self.selection_or_all();
        let Some(((x0, y0), (x1, y1))) = mask.bounds() else {
            return false;
        };
        let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
        let sprite = self.sprite();
        let wrap = |v: i32, from: i32, size: i32| from + (v - from).rem_euclid(size);
        let pixels = mask
            .points()
            .map(|(x, y)| {
                let (sx, sy) = (wrap(x - dx, x0, w), wrap(y - dy, y0, h));
                (x, y, sprite.pixel(self.layer, self.frame, sx, sy))
            })
            .collect();
        self.named("Shift", |e| e.draw(pixels))
    }

    /// Edit → Transform (Ctrl+T): levanta lo seleccionado (sin selección, la capa entera) con la
    /// caja para escalarlo y rotarlo; si la herramienta no transforma, pasa a Move.
    pub fn transform_selection(&mut self) -> bool {
        if !self.clip_layer_ok(true) {
            return false;
        }
        if self.selection.is_none() && self.floating.is_none() {
            self.select_all();
        }
        if !(self.tool.is_selection() || self.tool == ToolKind::Move) {
            self.set_tool(ToolKind::Move);
        }
        self.lift()
    }

    /// Edit → New Brush (Ctrl+B): lo pintado dentro de la selección (o, si no hay nada pintado,
    /// la forma de la selección) pasa a ser la forma del pincel personalizado, centrada en la
    /// caja de la selección; se elige ese pincel y se quita la selección. Pinta con el color
    /// primario.
    pub fn new_brush(&mut self) -> bool {
        self.commit_floating();
        let Some(mask) = &self.selection else {
            return false;
        };
        let Some(((x0, y0), (x1, y1))) = mask.bounds() else {
            return false;
        };
        let (cx, cy) = ((x0 + x1) / 2, (y0 + y1) / 2);
        let sprite = self.sprite();
        let painted =
            |&(x, y): &(i32, i32)| self.resolve(sprite.pixel(self.layer, self.frame, x, y)).a > 0;
        let mut points: Vec<(i32, i32)> = mask.points().filter(painted).collect();
        if points.is_empty() {
            points = mask.points().collect();
        }
        cs_raster::set_custom_brush(points.into_iter().map(|(x, y)| (x - cx, y - cy)).collect());
        self.options.brush_shape = cs_raster::BrushShape::Custom;
        self.deselect();
        true
    }

    /// Edit → Invert (Ctrl+I): invierte el color (no la transparencia) de lo seleccionado; sin
    /// selección, de la capa entera.
    pub fn invert_colors(&mut self) -> bool {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return false;
        }
        let mask = self.selection_or_all();
        let sprite = self.sprite();
        let colors: Vec<_> = mask
            .points()
            .filter_map(|(x, y)| {
                let c = self.resolve(sprite.pixel(self.layer, self.frame, x, y));
                (c.a > 0).then_some((x, y, Rgba::new(255 - c.r, 255 - c.g, 255 - c.b, c.a)))
            })
            .collect();
        let pixels = colors
            .into_iter()
            .map(|(x, y, c)| (x, y, self.value_of(c)))
            .collect();
        self.named("Invert", |e| e.draw(pixels))
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{LayerId, Sprite};

    use super::*;

    /// 6×4 con un punto rojo en (1, 1).
    fn editor() -> (Editor, PixelValue) {
        let mut e = Editor::with_sprite(Sprite::new(6, 4, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        (e, red)
    }

    fn at(e: &Editor, x: i32, y: i32) -> PixelValue {
        e.sprite().pixel(LayerId(1), 0, x, y)
    }

    #[test]
    fn fill_paints_the_whole_selection_with_the_primary_color() {
        let (mut e, _) = editor();
        assert!(!e.fill_selection(), "nothing selected");
        e.set_selection(Some(Mask::from_rect(6, 4, (2, 1), (3, 2))));
        assert!(e.fill_selection());
        for (x, y) in [(2, 1), (3, 1), (2, 2), (3, 2)] {
            assert_eq!(at(&e, x, y), e.primary);
        }
        assert_eq!(at(&e, 4, 2), e.sprite().color_mode.transparent());
    }

    #[test]
    fn shift_moves_one_pixel_and_wraps_inside_the_selection() {
        let (mut e, red) = editor();
        let t = e.sprite().color_mode.transparent();
        e.set_selection(Some(Mask::from_rect(6, 4, (0, 0), (2, 2))));
        assert!(e.shift_content(1, 0));
        assert_eq!(at(&e, 2, 1), red);
        assert_eq!(at(&e, 1, 1), t);
        // De la derecha de la caja vuelve a la izquierda.
        assert!(e.shift_content(1, 0));
        assert_eq!(at(&e, 0, 1), red);
        // Sin selección, la capa entera.
        e.deselect();
        assert!(e.shift_content(0, -1));
        assert_eq!(at(&e, 0, 0), red);
    }

    #[test]
    fn rotate_turns_the_selection_and_leaves_it_floating() {
        let (mut e, red) = editor();
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(6, 4, (0, 0), (2, 2))));
        assert!(e.rotate_content(Turn::Half));
        assert!(e.is_floating());
        e.commit_floating();
        // (1, 1) es el centro de la caja de 3×3: medio giro lo deja ahí; el de la esquina, no.
        assert_eq!(at(&e, 1, 1), red);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red)],
        });
        e.set_selection(Some(Mask::from_rect(6, 4, (0, 0), (2, 2))));
        assert!(e.rotate_content(Turn::Clockwise));
        e.commit_floating();
        assert_eq!(
            at(&e, 2, 0),
            red,
            "the top left corner goes to the top right"
        );
        assert_eq!(at(&e, 0, 0), e.sprite().color_mode.transparent());
    }

    #[test]
    fn new_brush_takes_the_painted_pixels_of_the_selection() {
        let (mut e, red) = editor();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 1, red)],
        });
        e.set_selection(Some(Mask::from_rect(6, 4, (0, 0), (2, 2))));
        assert!(e.new_brush());
        assert!(e.selection.is_none(), "deselected");
        assert_eq!(e.options.brush_shape, cs_raster::BrushShape::Custom);
        // Centrado en (1, 1): el punto de (1, 1) y el de (2, 1).
        let mut brush = cs_raster::brush_points((10, 10), 1, cs_raster::BrushShape::Custom);
        brush.sort();
        assert_eq!(brush, vec![(10, 10), (11, 10)]);
    }

    #[test]
    fn invert_keeps_the_transparency() {
        let (mut e, _) = editor();
        assert!(e.invert_colors());
        assert_eq!(at(&e, 1, 1), PixelValue::Rgba(Rgba::rgb(0, 255, 255)));
        assert_eq!(at(&e, 0, 0), e.sprite().color_mode.transparent());
    }

    #[test]
    fn transform_lifts_the_selection_with_the_move_tool() {
        let (mut e, _) = editor();
        e.set_tool(ToolKind::Pencil);
        assert!(e.transform_selection());
        assert_eq!(e.tool, ToolKind::Move);
        assert!(e.is_floating());
    }
}
