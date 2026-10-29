use cs_raster::{Point, cubic_bezier, line};

use crate::{Tool, ToolContext, ToolOutput};

/// Curva, como la de Aseprite: se arrastra la recta de A a B; después un clic (o arrastre) pone
/// el primer punto de control y otro el segundo, y entonces se pinta con el pincel. `finish`
/// (Enter o doble clic) la pinta con los controles que tenga.
#[derive(Debug, Default)]
pub struct Curve {
    /// Los puntos: A, B y los controles que ya se pusieron.
    points: Vec<Point>,
    /// El punto que se mueve ahora (el puntero, con o sin botón).
    cursor: Option<Point>,
    /// El botón está apretado.
    down: bool,
}

impl Curve {
    pub fn new() -> Self {
        Self::default()
    }

    /// A, B y los dos controles: los que faltan, sobre el puntero o sobre la recta.
    fn controls(&self) -> Option<(Point, Point, Point, Point)> {
        let cursor = self.cursor;
        match self.points[..] {
            [] => None,
            [a] => {
                let b = cursor.unwrap_or(a);
                Some((a, a, b, b))
            }
            [a, b] => {
                let c1 = cursor.unwrap_or(a);
                Some((a, c1, c1, b))
            }
            [a, b, c1, ..] => {
                let c2 = self.points.get(3).copied().or(cursor).unwrap_or(c1);
                Some((a, c1, c2, b))
            }
        }
    }

    fn output(&self, ctx: &ToolContext) -> ToolOutput {
        match self.controls() {
            Some((a, c1, c2, b)) => {
                let path = if (c1, c2) == (a, b) {
                    line(a, b)
                } else {
                    cubic_bezier(a, c1, c2, b)
                };
                ToolOutput::command(ctx.stamp_draw(&path, ctx.color))
            }
            None => ToolOutput::default(),
        }
    }

    fn reset(&mut self) {
        self.points.clear();
        self.cursor = None;
        self.down = false;
    }
}

impl Tool for Curve {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.down = true;
        if self.points.is_empty() {
            self.points.push(p);
        }
        self.cursor = Some(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        self.cursor = Some(p);
    }

    fn hover(&mut self, _ctx: &ToolContext, p: Point) {
        if !self.points.is_empty() {
            self.cursor = Some(p);
        }
    }

    /// Al soltar se fija el punto que se movía: B, el primer control o el segundo (y entonces
    /// se pinta).
    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        self.down = false;
        let Some(p) = self.cursor else {
            return ToolOutput::default();
        };
        if self.points.len() == 1 && self.points[0] == p {
            // Un clic sin arrastrar no es una recta.
            self.reset();
            return ToolOutput::default();
        }
        self.points.push(p);
        if self.points.len() < 4 {
            return ToolOutput::default();
        }
        let out = self.output(ctx);
        self.reset();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        self.output(ctx)
    }

    /// Activa mientras se arrastra la recta del principio (como Line).
    fn is_active(&self) -> bool {
        self.down && self.points.len() == 1
    }

    fn is_pending(&self) -> bool {
        !self.points.is_empty() && !self.is_active()
    }

    fn finish(&mut self, ctx: &ToolContext) -> ToolOutput {
        if self.points.len() < 2 {
            self.reset();
            return ToolOutput::default();
        }
        // Los controles que falten, sobre la recta.
        self.cursor = None;
        let out = self.output(ctx);
        self.reset();
        out
    }

    fn cancel(&mut self) {
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn drag_a_line_then_two_clicks_bend_it_and_paint_it() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Curve::new();
        t.press(&c, (0, 7));
        t.drag(&c, (7, 7));
        assert!(t.is_active());
        assert_eq!(t.release(&c), ToolOutput::default(), "not yet");
        assert!(t.is_pending());
        // El primer control.
        t.hover(&c, (0, 0));
        t.press(&c, (0, 0));
        assert_eq!(t.release(&c), ToolOutput::default());
        // El segundo: se pinta.
        t.press(&c, (7, 0));
        let out = coords(&t.release(&c));
        assert!(!t.is_pending() && !t.is_active());
        assert_eq!(out.first(), Some(&(0, 7)));
        assert_eq!(out.last(), Some(&(7, 7)));
        assert!(out.iter().any(|p| p.1 < 7), "bent: {out:?}");
    }

    #[test]
    fn finish_paints_the_straight_line_if_there_are_no_controls_yet() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Curve::new();
        t.press(&c, (0, 1));
        t.drag(&c, (3, 1));
        t.release(&c);
        assert_eq!(coords(&t.finish(&c)), vec![(0, 1), (1, 1), (2, 1), (3, 1)]);
    }
}
