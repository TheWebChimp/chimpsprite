use std::collections::BTreeSet;

use cs_commands::Command;
use cs_core::PixelValue;
use cs_raster::{Point, brush_square, line, pixel_perfect};

use crate::{Tool, ToolContext};

#[derive(Debug, Default)]
pub struct Pencil {
    path: Vec<Point>,
}

impl Pencil {
    pub fn new() -> Self {
        Self::default()
    }

    fn pixels(&self, ctx: &ToolContext, color: PixelValue) -> Vec<(i32, i32, PixelValue)> {
        let path = if ctx.pixel_perfect && ctx.brush_size <= 1 {
            pixel_perfect(&self.path)
        } else {
            self.path.clone()
        };
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for p in path {
            for q in brush_square(p, ctx.brush_size) {
                if seen.insert(q) {
                    out.push((q.0, q.1, color));
                }
            }
        }
        out
    }

    fn finish(&mut self, ctx: &ToolContext, color: PixelValue) -> Option<Command> {
        if self.path.is_empty() {
            return None;
        }
        let pixels = self.pixels(ctx, color);
        self.path.clear();
        Some(Command::DrawPixels {
            layer: ctx.layer,
            frame: ctx.frame,
            pixels,
        })
    }
}

impl Tool for Pencil {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.path = vec![p];
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        let Some(&last) = self.path.last() else {
            return;
        };
        if last != p {
            self.path.extend(line(last, p).into_iter().skip(1));
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> Option<Command> {
        self.finish(ctx, ctx.color)
    }

    fn preview(&self, ctx: &ToolContext) -> Vec<(i32, i32, PixelValue)> {
        self.pixels(ctx, ctx.color)
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

/// Lápiz que pinta con el valor transparente del sprite.
#[derive(Debug)]
pub struct Eraser {
    pencil: Pencil,
    transparent: PixelValue,
}

impl Eraser {
    pub fn new(transparent: PixelValue) -> Self {
        Self {
            pencil: Pencil::new(),
            transparent,
        }
    }
}

impl Tool for Eraser {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.press(ctx, p);
    }

    fn drag(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.drag(ctx, p);
    }

    fn release(&mut self, ctx: &ToolContext) -> Option<Command> {
        self.pencil.finish(ctx, self.transparent)
    }

    fn preview(&self, ctx: &ToolContext) -> Vec<(i32, i32, PixelValue)> {
        self.pencil.pixels(ctx, self.transparent)
    }

    fn is_active(&self) -> bool {
        self.pencil.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{LayerId, Rgba};

    fn ctx(pp: bool) -> ToolContext {
        ToolContext {
            layer: LayerId(1),
            frame: 0,
            color: PixelValue::Rgba(Rgba::rgb(9, 9, 9)),
            brush_size: 1,
            pixel_perfect: pp,
        }
    }

    fn coords(cmd: Command) -> Vec<(i32, i32)> {
        let Command::DrawPixels { pixels, .. } = cmd else {
            panic!("expected DrawPixels")
        };
        pixels.into_iter().map(|(x, y, _)| (x, y)).collect()
    }

    #[test]
    fn drag_interpolates_with_line() {
        let mut p = Pencil::new();
        let c = ctx(false);
        p.press(&c, (0, 0));
        p.drag(&c, (3, 0));
        assert!(p.is_active());
        assert_eq!(
            coords(p.release(&c).unwrap()),
            vec![(0, 0), (1, 0), (2, 0), (3, 0)]
        );
        assert!(!p.is_active());
    }

    #[test]
    fn pixel_perfect_applies_on_release() {
        let mut p = Pencil::new();
        let c = ctx(true);
        p.press(&c, (0, 0));
        p.drag(&c, (1, 0));
        p.drag(&c, (1, 1));
        assert_eq!(coords(p.release(&c).unwrap()), vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn eraser_uses_transparent_value() {
        let t = PixelValue::Index(0);
        let mut e = Eraser::new(t);
        let c = ctx(false);
        e.press(&c, (2, 2));
        let Command::DrawPixels { pixels, .. } = e.release(&c).unwrap() else {
            panic!()
        };
        assert_eq!(pixels, vec![(2, 2, t)]);
    }

    #[test]
    fn release_without_press_is_none() {
        assert!(Pencil::new().release(&ctx(false)).is_none());
    }
}
