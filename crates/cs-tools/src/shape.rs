use cs_raster::{
    Point, constrain_line, constrain_square, ellipse_filled, ellipse_outline, line,
    rounded_rect_filled, rounded_rect_outline, stamp,
};

use crate::{Drag, Tool, ToolContext, ToolOutput};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rectangle,
    Ellipse,
}

/// Línea, rectángulo o elipse definidos arrastrando de una esquina a otra.
#[derive(Debug)]
pub struct ShapeTool {
    kind: ShapeKind,
    drag: Drag,
}

impl ShapeTool {
    pub fn new(kind: ShapeKind) -> Self {
        Self {
            kind,
            drag: Drag::default(),
        }
    }

    pub fn kind(&self) -> ShapeKind {
        self.kind
    }

    fn points(&self, ctx: &ToolContext, a: Point, b: Point) -> Vec<Point> {
        let b = match (ctx.constrain, self.kind) {
            (false, _) => b,
            (true, ShapeKind::Line) => constrain_line(a, b),
            (true, _) => constrain_square(a, b),
        };
        let (size, shape) = (ctx.brush_size, ctx.brush_shape);
        match (self.kind, ctx.filled) {
            (ShapeKind::Line, _) => stamp(&line(a, b), size, shape),
            (ShapeKind::Rectangle, true) => rounded_rect_filled(a, b, ctx.corner_radius),
            (ShapeKind::Rectangle, false) => {
                stamp(&rounded_rect_outline(a, b, ctx.corner_radius), size, shape)
            }
            (ShapeKind::Ellipse, true) => ellipse_filled(a, b),
            (ShapeKind::Ellipse, false) => stamp(&ellipse_outline(a, b), size, shape),
        }
    }
}

impl Tool for ShapeTool {
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
        match self.drag.get() {
            Some((a, b)) => ToolOutput::command(ctx.draw(self.points(ctx, a, b), ctx.color)),
            None => ToolOutput::default(),
        }
    }

    fn is_active(&self) -> bool {
        self.drag.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    fn run(kind: ShapeKind, c: &ToolContext, a: Point, b: Point) -> Vec<Point> {
        let mut t = ShapeTool::new(kind);
        t.press(c, a);
        t.drag(c, b);
        coords(&t.release(c))
    }

    #[test]
    fn constrained_line_is_horizontal() {
        let s = sprite();
        let mut c = ctx(&s);
        c.constrain = true;
        assert_eq!(
            run(ShapeKind::Line, &c, (0, 0), (3, 1)),
            vec![(0, 0), (1, 0), (2, 0), (3, 0)]
        );
    }

    #[test]
    fn filled_rect_and_outline() {
        let s = sprite();
        let mut c = ctx(&s);
        assert_eq!(run(ShapeKind::Rectangle, &c, (0, 0), (2, 2)).len(), 8);
        c.filled = true;
        assert_eq!(run(ShapeKind::Rectangle, &c, (0, 0), (2, 2)).len(), 9);
    }

    #[test]
    fn corner_radius_rounds_the_rectangle() {
        let s = sprite();
        let mut c = ctx(&s);
        c.corner_radius = 2;
        let outline = run(ShapeKind::Rectangle, &c, (0, 0), (6, 6));
        assert!(!outline.contains(&(0, 0)) && outline.contains(&(3, 0)));
        c.filled = true;
        let filled = run(ShapeKind::Rectangle, &c, (0, 0), (6, 6));
        assert!(!filled.contains(&(0, 0)) && filled.contains(&(3, 3)));
    }

    #[test]
    fn constrained_ellipse_is_circle() {
        let s = sprite();
        let mut c = ctx(&s);
        c.constrain = true;
        let pts = run(ShapeKind::Ellipse, &c, (0, 0), (4, 2));
        assert!(pts.contains(&(2, 4)), "{pts:?}");
    }

    #[test]
    fn the_outline_uses_the_brush_shape() {
        let s = sprite();
        let mut c = ctx(&s);
        c.brush_size = 3;
        c.brush_shape = crate::BrushShape::Round;
        let mut t = ShapeTool::new(ShapeKind::Line);
        t.press(&c, (2, 2));
        let pts = coords(&t.release(&c));
        assert_eq!(pts.len(), 5, "a one-point line is a cross: {pts:?}");
        assert!(!pts.contains(&(1, 1)));
    }
}
