use cs_raster::{Point, flood_fill};

use crate::{Tool, ToolContext, ToolOutput};

/// Cubeta: rellena la región del mismo color (contigua o global) en la capa actual.
#[derive(Debug, Default)]
pub struct Bucket {
    point: Option<Point>,
}

impl Bucket {
    pub fn new() -> Self {
        Self::default()
    }
}

fn fill(ctx: &ToolContext, p: Point) -> ToolOutput {
    if !ctx.in_canvas(p) {
        return ToolOutput::default();
    }
    let sprite = ctx.sprite;
    let target = sprite.pixel(ctx.layer, ctx.frame, p.0, p.1);
    if target == ctx.color {
        return ToolOutput::default();
    }
    let matches = |x: i32, y: i32| {
        ctx.selection.is_none_or(|m| m.contains(x, y))
            && sprite.pixel(ctx.layer, ctx.frame, x, y) == target
    };
    let points: Vec<Point> = if ctx.contiguous {
        flood_fill(sprite.width, sprite.height, p, matches)
    } else {
        (0..sprite.height as i32)
            .flat_map(|y| (0..sprite.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| matches(x, y))
            .collect()
    };
    if points.is_empty() {
        return ToolOutput::default();
    }
    ToolOutput::command(ctx.draw(points, ctx.color))
}

impl Tool for Bucket {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.point = Some(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        if self.point.is_some() {
            self.point = Some(p);
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        self.point
            .take()
            .map_or_else(ToolOutput::default, |p| fill(ctx, p))
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        self.point
            .map_or_else(ToolOutput::default, |p| fill(ctx, p))
    }

    fn is_active(&self) -> bool {
        self.point.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use cs_commands::Command;
    use cs_core::{LayerId, PixelValue, Rgba, Sprite};

    /// Sprite 8×8 con un muro vertical en x = 3.
    fn walled() -> Sprite {
        let mut s = sprite();
        let wall = PixelValue::Rgba(Rgba::rgb(0, 0, 0));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: (0..8).map(|y| (3, y, wall)).collect(),
        }
        .apply(&mut s)
        .unwrap();
        s
    }

    fn click(c: &ToolContext, p: Point) -> ToolOutput {
        let mut b = Bucket::new();
        b.press(c, p);
        b.release(c)
    }

    #[test]
    fn contiguous_fill_stops_at_wall() {
        let s = walled();
        let pts = coords(&click(&ctx(&s), (0, 0)));
        assert_eq!(pts.len(), 3 * 8);
        assert!(pts.iter().all(|&(x, _)| x < 3));
    }

    #[test]
    fn global_fill_crosses_wall() {
        let s = walled();
        let mut c = ctx(&s);
        c.contiguous = false;
        assert_eq!(coords(&click(&c, (0, 0))).len(), 7 * 8);
    }

    #[test]
    fn same_color_is_noop() {
        let s = sprite();
        let mut c = ctx(&s);
        c.color = s.color_mode.transparent();
        assert_eq!(click(&c, (0, 0)), ToolOutput::default());
    }

    #[test]
    fn selection_limits_fill() {
        let s = sprite();
        let mask = cs_core::Mask::from_rect(8, 8, (0, 0), (1, 1));
        let mut c = ctx(&s);
        c.selection = Some(&mask);
        assert_eq!(coords(&click(&c, (0, 0))).len(), 4);
    }
}
