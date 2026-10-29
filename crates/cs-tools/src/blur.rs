use std::collections::HashMap;

use cs_core::{ColorMode, PixelValue, Rgba};
use cs_raster::{BrushShape, Point, blur_average, brush_points, line};

use crate::{Tool, ToolContext, ToolOutput};

/// Pincel que suaviza: cada píxel bajo el pincel toma el promedio de su vecindario de 3×3. Las
/// pasadas del mismo trazo se acumulan; no usa la simetría (como en Aseprite).
#[derive(Debug, Default)]
pub struct Blur {
    active: bool,
    at: Point,
    /// Lo ya suavizado en este trazo. El documento no cambia hasta soltar, así que lo demás se
    /// lee de la capa.
    done: HashMap<Point, PixelValue>,
    /// Píxeles del pincel alrededor de (0, 0), para el tamaño y la forma de `brush_key`: se
    /// calculan una vez y no en cada aplicación (como en `stamp`).
    brush: Vec<Point>,
    brush_key: Option<(u32, BrushShape)>,
}

impl Blur {
    pub fn new() -> Self {
        Self::default()
    }

    /// Valor de `p` en este momento del trazo (dentro del lienzo).
    fn value(&self, ctx: &ToolContext, p: Point) -> PixelValue {
        self.done
            .get(&p)
            .copied()
            .unwrap_or_else(|| ctx.sprite.pixel(ctx.layer, ctx.frame, p.0, p.1))
    }

    /// Una aplicación del pincel en `center`. Primero calcula todos los promedios con los
    /// colores de antes y después los guarda: así el orden de recorrido no sesga el resultado.
    fn apply(&mut self, ctx: &ToolContext, center: Point) {
        let key = (ctx.brush_size, ctx.brush_shape);
        if self.brush_key != Some(key) {
            self.brush = brush_points((0, 0), key.0, key.1);
            self.brush_key = Some(key);
        }
        let targets: Vec<Point> = self
            .brush
            .iter()
            .map(|&(dx, dy)| (center.0 + dx, center.1 + dy))
            .filter(|&p| ctx.in_canvas(p))
            .filter(|&(x, y)| ctx.selection.is_none_or(|m| m.contains(x, y)))
            .collect();
        // Cada píxel que hace falta se lee y se resuelve una sola vez por aplicación.
        let mut source: HashMap<Point, (PixelValue, Rgba)> = HashMap::new();
        for &(x, y) in &targets {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let q = (x + dx, y + dy);
                    if ctx.in_canvas(q) {
                        source.entry(q).or_insert_with(|| {
                            let v = self.value(ctx, q);
                            (v, cs_render::resolve(ctx.sprite, v))
                        });
                    }
                }
            }
        }
        let updates: Vec<(Point, PixelValue)> = targets
            .into_iter()
            .map(|p| {
                let average = blur_average(p, |q| source.get(&q).map(|&(_, c)| c));
                let (value, color) = source[&p];
                // Si el promedio es el color que ya tiene, se queda con su valor: en indexado,
                // con colores repetidos en la paleta, `nearest` podría elegir otro índice.
                let value = if average == color {
                    value
                } else {
                    to_value(ctx, average)
                };
                (p, value)
            })
            .collect();
        self.done.extend(updates);
    }
}

/// Promedio → valor de píxel. En indexado, el índice más cercano de la paleta (con alfa < 128,
/// el transparente).
fn to_value(ctx: &ToolContext, c: Rgba) -> PixelValue {
    match ctx.sprite.color_mode {
        ColorMode::Rgba | ColorMode::Grayscale => PixelValue::Rgba(c),
        ColorMode::Indexed { transparent_index } => {
            PixelValue::Index(ctx.sprite.palette.nearest(c, transparent_index))
        }
    }
}

impl Tool for Blur {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        self.active = true;
        self.at = p;
        self.done.clear();
        self.apply(ctx, p);
    }

    fn drag(&mut self, ctx: &ToolContext, p: Point) {
        if !self.active || p == self.at {
            return;
        }
        for q in line(self.at, p).into_iter().skip(1) {
            self.apply(ctx, q);
        }
        self.at = p;
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.active = false;
        self.done.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if !self.active {
            return ToolOutput::default();
        }
        // Solo lo que cambió: sin cambios no hay comando (ni paso de undo).
        let mut pixels: Vec<(i32, i32, PixelValue)> = self
            .done
            .iter()
            .filter(|&(&(x, y), &v)| v != ctx.sprite.pixel(ctx.layer, ctx.frame, x, y))
            .map(|(&(x, y), &v)| (x, y, v))
            .collect();
        if pixels.is_empty() {
            return ToolOutput::default();
        }
        pixels.sort_unstable_by_key(|&(x, y, _)| (y, x));
        ToolOutput::command(cs_commands::Command::DrawPixels {
            layer: ctx.layer,
            frame: ctx.frame,
            pixels,
        })
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, Mask, Palette, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::test_util::*;

    fn paint(s: &mut Sprite, pixels: Vec<(i32, i32, PixelValue)>) {
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels,
        }
        .apply(s)
        .unwrap();
    }

    fn pixels(out: &ToolOutput) -> Vec<(i32, i32, PixelValue)> {
        match &out.command {
            Some(Command::DrawPixels { pixels, .. }) => pixels.clone(),
            other => panic!("expected DrawPixels, got {other:?}"),
        }
    }

    /// Rojo con un noveno del alfa: un punto rojo repartido en su vecindario.
    fn soft() -> PixelValue {
        PixelValue::Rgba(Rgba::new(255, 0, 0, 28))
    }

    #[test]
    fn a_dot_spreads_under_the_brush() {
        let mut s = sprite();
        paint(&mut s, vec![(4, 4, red())]);
        let mut c = ctx(&s);
        c.brush_size = 3;
        let mut t = Blur::new();
        t.press(&c, (4, 4));
        assert!(t.is_active());
        let px = pixels(&t.release(&c));
        // Los 9 se calculan con los colores de antes, así que todos quedan iguales.
        assert_eq!(px.len(), 9);
        assert!(px.iter().all(|&(_, _, v)| v == soft()), "{px:?}");
        assert!(!t.is_active());
    }

    #[test]
    fn going_over_again_blurs_more() {
        let mut s = sprite();
        paint(&mut s, vec![(4, 4, red())]);
        let c = ctx(&s);
        let mut t = Blur::new();
        t.press(&c, (4, 4)); // (4, 4) → alfa 28
        t.drag(&c, (5, 4)); // (5, 4) lee el 28 de (4, 4) → alfa 3
        t.drag(&c, (4, 4)); // (4, 4) otra vez: (28 + 3) / 9 → alfa 3
        let px = pixels(&t.release(&c));
        let faint = PixelValue::Rgba(Rgba::new(255, 0, 0, 3));
        assert_eq!(px, vec![(4, 4, faint), (5, 4, faint)]);
    }

    #[test]
    fn only_selected_pixels_change() {
        let mut s = sprite();
        paint(&mut s, vec![(4, 4, red())]);
        let mask = Mask::from_rect(8, 8, (0, 0), (3, 7));
        let mut c = ctx(&s);
        c.brush_size = 3;
        c.selection = Some(&mask);
        let mut t = Blur::new();
        t.press(&c, (4, 4));
        assert_eq!(
            pixels(&t.release(&c)),
            vec![(3, 3, soft()), (3, 4, soft()), (3, 5, soft())]
        );
    }

    #[test]
    fn nothing_to_blur_gives_no_command() {
        let s = sprite(); // todo transparente
        let c = ctx(&s);
        let mut t = Blur::new();
        t.press(&c, (4, 4));
        t.drag(&c, (6, 4));
        assert_eq!(t.preview(&c), ToolOutput::default());
        assert_eq!(t.release(&c), ToolOutput::default());
        let mut s = sprite();
        let all = (0..8).flat_map(|y| (0..8).map(move |x| (x, y, red())));
        paint(&mut s, all.collect());
        let c = ctx(&s);
        t.press(&c, (4, 4));
        assert_eq!(
            t.release(&c),
            ToolOutput::default(),
            "a flat area stays the same"
        );
    }

    #[test]
    fn indexed_takes_the_nearest_palette_index() {
        let palette = Palette::new(vec![
            Rgba::rgb(0, 0, 0),       // 0: transparente
            Rgba::rgb(0, 0, 0),       // 1: negro
            Rgba::rgb(255, 255, 255), // 2: blanco
            Rgba::rgb(128, 128, 128), // 3: gris
        ]);
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        // Negro junto a blanco: el promedio es gris.
        let mut s = Sprite::new(2, 1, mode);
        s.palette = palette.clone();
        paint(
            &mut s,
            vec![(0, 0, PixelValue::Index(1)), (1, 0, PixelValue::Index(2))],
        );
        let c = ctx(&s);
        let mut t = Blur::new();
        t.press(&c, (0, 0));
        assert_eq!(pixels(&t.release(&c)), vec![(0, 0, PixelValue::Index(3))]);
        // Un punto blanco aislado: alfa 28 < 128 → el índice transparente.
        let mut s = Sprite::new(3, 3, mode);
        s.palette = palette;
        paint(&mut s, vec![(1, 1, PixelValue::Index(2))]);
        let c = ctx(&s);
        t.press(&c, (1, 1));
        assert_eq!(pixels(&t.release(&c)), vec![(1, 1, PixelValue::Index(0))]);
    }

    #[test]
    fn indexed_flat_area_with_a_duplicate_color_is_not_remapped() {
        // 1 y 2 son el mismo rojo: `nearest` daría 1, pero el área pintada con 2 ya tiene el
        // promedio y no debe cambiar (ni dejar un paso de undo).
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut s = Sprite::new(4, 4, mode);
        s.palette = Palette::new(vec![
            Rgba::rgb(0, 0, 0),
            Rgba::rgb(255, 0, 0),
            Rgba::rgb(255, 0, 0),
        ]);
        let all = (0..4).flat_map(|y| (0..4).map(move |x| (x, y, PixelValue::Index(2))));
        paint(&mut s, all.collect());
        let mut c = ctx(&s);
        c.brush_size = 3;
        let mut t = Blur::new();
        t.press(&c, (1, 1));
        t.drag(&c, (2, 2));
        assert_eq!(t.release(&c), ToolOutput::default());
    }

    #[test]
    fn a_round_brush_blurs_a_cross() {
        let mut s = sprite();
        paint(&mut s, vec![(4, 4, red())]);
        let mut c = ctx(&s);
        c.brush_size = 3;
        c.brush_shape = crate::BrushShape::Round;
        let mut t = Blur::new();
        t.press(&c, (4, 4));
        let px = pixels(&t.release(&c));
        assert_eq!(px.len(), 5, "{px:?}");
        assert!(px.iter().all(|&(_, _, v)| v == soft()));
        assert!(!px.iter().any(|&(x, y, _)| (x, y) == (3, 3)), "no corner");
    }
}
