use cs_core::PixelValue;
use cs_raster::{Point, line, pixel_perfect, stamp};

use crate::{Tool, ToolContext, ToolOutput};

#[derive(Debug, Default)]
pub struct Pencil {
    path: Vec<Point>,
}

impl Pencil {
    pub fn new() -> Self {
        Self::default()
    }

    fn output(&self, ctx: &ToolContext, color: PixelValue) -> ToolOutput {
        if self.path.is_empty() {
            return ToolOutput::default();
        }
        let path = if ctx.pixel_perfect && ctx.brush_size <= 1 {
            pixel_perfect(&self.path)
        } else {
            self.path.clone()
        };
        ToolOutput::command(ctx.draw(stamp(&path, ctx.brush_size), color))
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

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.output(ctx, ctx.color);
        self.path.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        self.output(ctx, ctx.color)
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

/// Lápiz que pinta con el valor transparente del sprite.
#[derive(Debug, Default)]
pub struct Eraser {
    pencil: Pencil,
}

impl Eraser {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for Eraser {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.press(ctx, p);
    }

    fn drag(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.drag(ctx, p);
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.pencil.path.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        self.pencil.output(ctx, ctx.sprite.color_mode.transparent())
    }

    fn is_active(&self) -> bool {
        self.pencil.is_active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use cs_commands::Command;

    #[test]
    fn drag_interpolates_with_line() {
        let s = sprite();
        let c = ctx(&s);
        let mut p = Pencil::new();
        p.press(&c, (0, 0));
        p.drag(&c, (3, 0));
        assert!(p.is_active());
        assert_eq!(coords(&p.preview(&c)), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert_eq!(coords(&p.release(&c)), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
        assert!(!p.is_active());
    }

    #[test]
    fn pixel_perfect_applies() {
        let s = sprite();
        let mut c = ctx(&s);
        c.pixel_perfect = true;
        let mut p = Pencil::new();
        p.press(&c, (0, 0));
        p.drag(&c, (1, 0));
        p.drag(&c, (1, 1));
        assert_eq!(coords(&p.release(&c)), vec![(0, 0), (1, 1)]);
    }

    #[test]
    fn eraser_uses_transparent_value() {
        let s = sprite();
        let c = ctx(&s);
        let mut e = Eraser::new();
        e.press(&c, (2, 2));
        let Some(Command::DrawPixels { pixels, .. }) = e.release(&c).command else {
            panic!()
        };
        assert_eq!(pixels, vec![(2, 2, s.color_mode.transparent())]);
    }

    #[test]
    fn release_without_press_is_empty() {
        let s = sprite();
        assert_eq!(Pencil::new().release(&ctx(&s)), ToolOutput::default());
    }
}
