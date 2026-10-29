use cs_core::{ColorMode, PixelValue, Rgba, Sprite};
use cs_raster::{Dither, GradientShape, Point, constrain_line, gradient_t};

use crate::{Drag, Tool, ToolContext, ToolOutput};

/// Degradado del color del gesto al otro color, sobre la selección o todo el lienzo.
#[derive(Debug, Default)]
pub struct GradientTool {
    drag: Drag,
}

impl GradientTool {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Valor del degradado en `t` (0 = `from`, 1 = `to`) para el píxel `(x, y)`.
pub fn gradient_value(
    sprite: &Sprite,
    from: PixelValue,
    to: PixelValue,
    t: f32,
    dither: Dither,
    (x, y): Point,
) -> PixelValue {
    if let Some(threshold) = dither.threshold(x, y) {
        return if t > threshold { to } else { from };
    }
    let (a, b) = (
        cs_render::resolve(sprite, from),
        cs_render::resolve(sprite, to),
    );
    let lerp = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t).round() as u8;
    let mixed = Rgba::new(
        lerp(a.r, b.r),
        lerp(a.g, b.g),
        lerp(a.b, b.b),
        lerp(a.a, b.a),
    );
    match sprite.color_mode {
        ColorMode::Rgba | ColorMode::Grayscale => PixelValue::Rgba(mixed),
        ColorMode::Indexed { .. } => {
            // Sin dithering en indexado: el color más cercano de la paleta.
            let d = |c: &Rgba| {
                let (dr, dg, db, da) = (
                    c.r as i32 - mixed.r as i32,
                    c.g as i32 - mixed.g as i32,
                    c.b as i32 - mixed.b as i32,
                    c.a as i32 - mixed.a as i32,
                );
                dr * dr + dg * dg + db * db + da * da
            };
            let index = sprite
                .palette
                .colors()
                .iter()
                .enumerate()
                .min_by_key(|(_, c)| d(c))
                .map_or(0, |(i, _)| i);
            PixelValue::Index(index as u8)
        }
    }
}

/// Píxeles del degradado de `a` a `b` sobre `points`.
pub fn gradient_pixels(
    sprite: &Sprite,
    (from, to): (PixelValue, PixelValue),
    (shape, dither): (GradientShape, Dither),
    (a, b): (Point, Point),
    points: impl IntoIterator<Item = Point>,
) -> Vec<(i32, i32, PixelValue)> {
    points
        .into_iter()
        .map(|p| {
            let t = gradient_t(shape, a, b, p);
            (p.0, p.1, gradient_value(sprite, from, to, t, dither, p))
        })
        .collect()
}

impl Tool for GradientTool {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.drag.press(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        self.drag.drag(p);
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.drag.take();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        let Some((a, b)) = self.drag.get() else {
            return ToolOutput::default();
        };
        if a == b {
            return ToolOutput::default();
        }
        let b = if ctx.constrain {
            constrain_line(a, b)
        } else {
            b
        };
        let s = ctx.sprite;
        let points: Vec<Point> = match ctx.selection {
            Some(mask) => mask.points().collect(),
            None => (0..s.height as i32)
                .flat_map(|y| (0..s.width as i32).map(move |x| (x, y)))
                .collect(),
        };
        let pixels = gradient_pixels(
            s,
            (ctx.color, ctx.secondary),
            (ctx.gradient_shape, ctx.dither),
            (a, b),
            points,
        );
        ToolOutput::command(cs_commands::Command::DrawPixels {
            layer: ctx.layer,
            frame: ctx.frame,
            pixels,
        })
    }

    fn is_active(&self) -> bool {
        self.drag.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{Mask, Palette};

    use super::*;
    use crate::test_util::*;

    fn pixels(out: ToolOutput) -> Vec<(i32, i32, PixelValue)> {
        match out.command {
            Some(Command::DrawPixels { pixels, .. }) => pixels,
            other => panic!("{other:?}"),
        }
    }

    fn at(px: &[(i32, i32, PixelValue)], x: i32, y: i32) -> PixelValue {
        px.iter().find(|p| (p.0, p.1) == (x, y)).unwrap().2
    }

    #[test]
    fn smooth_linear_gradient_fills_the_canvas() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = GradientTool::new();
        t.press(&c, (0, 0));
        t.drag(&c, (7, 0));
        let px = pixels(t.release(&c));
        assert_eq!(px.len(), 64);
        assert_eq!(at(&px, 0, 3), red());
        assert_eq!(at(&px, 7, 5), PixelValue::Rgba(Rgba::rgb(0, 0, 255)));
        let PixelValue::Rgba(mid) = at(&px, 4, 0) else {
            panic!()
        };
        assert!(mid.r > 0 && mid.b > 0, "{mid:?}");
    }

    #[test]
    fn dithered_gradient_uses_only_both_colors_and_mixes() {
        let s = sprite();
        let mut c = ctx(&s);
        c.dither = Dither::Bayer4;
        let mut t = GradientTool::new();
        t.press(&c, (0, 0));
        t.drag(&c, (7, 0));
        let px = pixels(t.release(&c));
        assert!(px.iter().all(|p| p.2 == c.color || p.2 == c.secondary));
        let column = |x| px.iter().filter(|p| p.0 == x && p.2 == c.secondary).count();
        assert_eq!(column(0), 0);
        assert!(column(4) > 0 && column(4) < 8, "middle columns mix");
        assert_eq!(column(7), 8);
    }

    #[test]
    fn respects_selection_and_indexed_palette() {
        let mut s = Sprite::new(
            4,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        s.palette = Palette::new(vec![
            Rgba::TRANSPARENT,
            Rgba::rgb(0, 0, 0),
            Rgba::rgb(128, 128, 128),
            Rgba::rgb(255, 255, 255),
        ]);
        let mask = Mask::from_rect(4, 1, (1, 0), (3, 0));
        let mut c = ctx(&s);
        c.color = PixelValue::Index(1);
        c.secondary = PixelValue::Index(3);
        c.selection = Some(&mask);
        let mut t = GradientTool::new();
        t.press(&c, (0, 0));
        t.drag(&c, (4, 0));
        let px = pixels(t.release(&c));
        assert_eq!(px.len(), 3, "only the selection");
        assert_eq!(at(&px, 2, 0), PixelValue::Index(2), "nearest palette gray");
    }
}
