use cs_raster::{Point, line, polygon_filled};

use crate::{Tool, ToolContext, ToolOutput};

/// Contorno a mano alzada: al soltar se cierra (del último punto al primero) y se rellena con el
/// contorno incluido. La vista previa es el trazo de 1 px; no usa el tamaño de pincel.
#[derive(Debug, Default)]
pub struct Contour {
    path: Vec<Point>,
}

impl Contour {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for Contour {
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
        if self.path.is_empty() {
            return ToolOutput::default();
        }
        // Un solo punto da un píxel; un trazo sin área queda como el trazo.
        let out = ToolOutput::command(ctx.draw(polygon_filled(&self.path), ctx.color));
        self.path.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if self.path.is_empty() {
            return ToolOutput::default();
        }
        ToolOutput::command(ctx.draw(self.path.iter().copied(), ctx.color))
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{PixelValue, Rgba};

    use super::*;
    use crate::test_util::*;

    fn stroke(t: &mut Contour, c: &ToolContext, points: &[Point]) {
        t.press(c, points[0]);
        for &p in &points[1..] {
            t.drag(c, p);
        }
    }

    #[test]
    fn a_closed_l_fills_the_triangle() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Contour::new();
        // Abajo y a la derecha; al soltar se cierra en diagonal hasta (0, 0).
        stroke(&mut t, &c, &[(0, 0), (0, 4), (4, 4)]);
        let mut got = coords(&t.release(&c));
        got.sort_unstable();
        let mut want: Vec<Point> = (0..=4).flat_map(|y| (0..=y).map(move |x| (x, y))).collect();
        want.sort_unstable();
        assert_eq!(got, want);
        assert!(!t.is_active());
    }

    #[test]
    fn the_preview_is_the_one_pixel_trace() {
        let s = sprite();
        let mut c = ctx(&s);
        c.brush_size = 3; // Contour no usa el pincel.
        let mut t = Contour::new();
        stroke(&mut t, &c, &[(0, 0), (0, 4), (4, 4)]);
        assert!(t.is_active());
        assert_eq!(
            coords(&t.preview(&c)),
            vec![
                (0, 0),
                (0, 1),
                (0, 2),
                (0, 3),
                (0, 4),
                (1, 4),
                (2, 4),
                (3, 4),
                (4, 4)
            ]
        );
    }

    #[test]
    fn a_click_is_one_pixel_and_a_line_stays_a_line() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Contour::new();
        t.press(&c, (2, 2));
        assert_eq!(coords(&t.release(&c)), vec![(2, 2)]);
        stroke(&mut t, &c, &[(0, 1), (3, 1)]);
        assert_eq!(coords(&t.release(&c)), vec![(0, 1), (1, 1), (2, 1), (3, 1)]);
        assert_eq!(t.release(&c), ToolOutput::default(), "nothing left");
    }

    #[test]
    fn paints_with_the_context_color() {
        let s = sprite();
        let mut c = ctx(&s);
        let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
        c.color = blue;
        let mut t = Contour::new();
        stroke(&mut t, &c, &[(0, 0), (3, 0), (3, 3)]);
        let Some(Command::DrawPixels { pixels, .. }) = t.release(&c).command else {
            panic!()
        };
        assert!(!pixels.is_empty());
        assert!(pixels.iter().all(|&(_, _, v)| v == blue));
    }
}
