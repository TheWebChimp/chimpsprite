use cs_raster::{Point, line, stamp};

use crate::rng::hash;
use crate::{Tool, ToolContext, ToolOutput};

/// Revuelve píxeles: cada punto del trazo toma el valor de un vecino aleatorio (radio = pincel)
/// de la capa tal como estaba al empezar.
#[derive(Debug, Default)]
pub struct Jumble {
    path: Vec<Point>,
    seed: u64,
}

impl Jumble {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for Jumble {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.seed = self.seed.wrapping_add(0x5851_F42D);
        self.path = vec![p];
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        if let Some(&last) = self.path.last()
            && last != p
        {
            self.path.extend(line(last, p).into_iter().skip(1));
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.path.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if self.path.is_empty() {
            return ToolOutput::default();
        }
        let r = ctx.brush_size.max(1) as i32;
        let span = 2 * r as u64 + 1;
        let mut points = stamp(&self.path, ctx.brush_size, ctx.brush_shape);
        points.sort_unstable();
        points.dedup();
        let pixels = points
            .into_iter()
            .filter(|&p| ctx.in_canvas(p))
            .filter_map(|(x, y)| {
                let h = hash(self.seed, x, y);
                let dx = (h % span) as i32 - r;
                let dy = ((h / span) % span) as i32 - r;
                let (sx, sy) = (x + dx, y + dy);
                ctx.in_canvas((sx, sy))
                    .then(|| (x, y, ctx.sprite.pixel(ctx.layer, ctx.frame, sx, sy)))
            })
            .collect();
        ToolOutput::command(cs_commands::Command::DrawPixels {
            layer: ctx.layer,
            frame: ctx.frame,
            pixels,
        })
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{LayerId, PixelValue, Rgba};

    use super::*;
    use crate::test_util::*;

    #[test]
    fn uses_only_nearby_colors_and_is_stable() {
        let mut s = sprite();
        // Columnas de colores distintos: rojo = 10 * x.
        let pixels = (0..8)
            .flat_map(|y| {
                (0..8).map(move |x| (x, y, PixelValue::Rgba(Rgba::rgb(10 * x as u8, 0, 0))))
            })
            .collect();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels,
        }
        .apply(&mut s)
        .unwrap();
        let mut c = ctx(&s);
        c.brush_size = 2;
        let mut t = Jumble::new();
        t.press(&c, (4, 4));
        t.drag(&c, (5, 4));
        let out = t.preview(&c);
        assert_eq!(out, t.preview(&c));
        let Some(Command::DrawPixels { pixels, .. }) = out.command else {
            panic!()
        };
        assert!(!pixels.is_empty());
        let mut moved = 0;
        for (x, _, v) in pixels {
            let PixelValue::Rgba(c) = v else { panic!() };
            let src_x = c.r as i32 / 10;
            assert!((src_x - x).abs() <= 2, "x={x} took from {src_x}");
            moved += (src_x != x) as usize;
        }
        assert!(moved > 0, "something got jumbled");
    }
}
