//! Herramientas de selección: marcos rectangular y elíptico, lazo, lazo poligonal y varita.
//! Cada una devuelve la forma seleccionada; el editor la combina con la selección existente.

use cs_core::{Mask, Rgba};
use cs_raster::{Point, constrain_square, ellipse_filled, flood_fill, line, polygon_filled};

use crate::{Drag, Tool, ToolContext, ToolOutput};

fn selection(mask: Option<Mask>) -> ToolOutput {
    ToolOutput {
        selection: Some(mask.filter(|m| !m.is_empty())),
        ..Default::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarqueeShape {
    Rectangle,
    Ellipse,
}

/// Selección rectangular o elíptica arrastrando. Un clic sin arrastrar deselecciona.
#[derive(Debug)]
pub struct Marquee {
    shape: MarqueeShape,
    drag: Drag,
}

impl Marquee {
    pub fn new(shape: MarqueeShape) -> Self {
        Self {
            shape,
            drag: Drag::default(),
        }
    }
}

impl Tool for Marquee {
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
            return selection(None);
        }
        let b = if ctx.constrain {
            constrain_square(a, b)
        } else {
            b
        };
        let (w, h) = (ctx.sprite.width, ctx.sprite.height);
        selection(Some(match self.shape {
            MarqueeShape::Rectangle => Mask::from_rect(w, h, a, b),
            MarqueeShape::Ellipse => Mask::from_points(w, h, ellipse_filled(a, b)),
        }))
    }

    fn is_active(&self) -> bool {
        self.drag.get().is_some()
    }
}

/// Lazo a mano alzada: se cierra solo al soltar.
#[derive(Debug, Default)]
pub struct Lasso {
    path: Vec<Point>,
}

impl Lasso {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for Lasso {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
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
        match self.path.as_slice() {
            [] => ToolOutput::default(),
            [_] => selection(None),
            path => selection(Some(Mask::from_points(
                ctx.sprite.width,
                ctx.sprite.height,
                polygon_filled(path),
            ))),
        }
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

/// Lazo poligonal: cada clic agrega un vértice; se cierra con clic en el primero, o con
/// `finish` (doble clic / Enter).
#[derive(Debug, Default)]
pub struct PolygonLasso {
    vertices: Vec<Point>,
    cursor: Option<Point>,
    /// El último clic cerró el polígono; se entrega al soltar.
    closed: bool,
}

impl PolygonLasso {
    pub fn new() -> Self {
        Self::default()
    }

    fn mask(&self, ctx: &ToolContext, with_cursor: bool) -> Mask {
        let mut points = self.vertices.clone();
        if with_cursor && let Some(c) = self.cursor {
            points.push(c);
        }
        Mask::from_points(ctx.sprite.width, ctx.sprite.height, polygon_filled(&points))
    }

    fn reset(&mut self) {
        self.vertices.clear();
        self.cursor = None;
        self.closed = false;
    }
}

impl Tool for PolygonLasso {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        let near = |a: Point| (a.0 - p.0).abs() <= 1 && (a.1 - p.1).abs() <= 1;
        match self.vertices.first() {
            Some(&first) if self.vertices.len() >= 3 && near(first) => self.closed = true,
            _ if self.vertices.last() == Some(&p) => {}
            _ => self.vertices.push(p),
        }
        self.cursor = Some(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        self.cursor = Some(p);
    }

    fn hover(&mut self, _ctx: &ToolContext, p: Point) {
        self.cursor = Some(p);
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        if !self.closed {
            return ToolOutput::default();
        }
        let out = selection(Some(self.mask(ctx, false)));
        self.reset();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if self.vertices.is_empty() {
            return ToolOutput::default();
        }
        selection(Some(self.mask(ctx, !self.closed)))
    }

    /// Solo "activo" el instante en que un clic cierra el polígono (para entregarlo al soltar).
    fn is_active(&self) -> bool {
        self.closed
    }

    fn is_pending(&self) -> bool {
        !self.vertices.is_empty() && !self.closed
    }

    fn finish(&mut self, ctx: &ToolContext) -> ToolOutput {
        if self.vertices.is_empty() {
            return ToolOutput::default();
        }
        let out = selection(Some(self.mask(ctx, false)));
        self.reset();
        out
    }

    fn cancel(&mut self) {
        self.reset();
    }
}

/// Varita mágica: selecciona por color con tolerancia.
#[derive(Debug, Default)]
pub struct MagicWand {
    at: Option<Point>,
}

impl MagicWand {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Diferencia máxima entre canales (incluido alfa); los transparentes son todos iguales.
fn distance(a: Rgba, b: Rgba) -> u8 {
    if a.a == 0 && b.a == 0 {
        return 0;
    }
    [
        a.r.abs_diff(b.r),
        a.g.abs_diff(b.g),
        a.b.abs_diff(b.b),
        a.a.abs_diff(b.a),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

impl Tool for MagicWand {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.at = Some(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, _p: Point) {}

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.at = None;
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        let Some(p) = self.at else {
            return ToolOutput::default();
        };
        if !ctx.in_canvas(p) {
            return selection(None);
        }
        let s = ctx.sprite;
        let color = |x: i32, y: i32| {
            if ctx.sample_all {
                cs_render::composite_pixel(s, ctx.frame, x, y)
            } else {
                cs_render::resolve(s, s.pixel(ctx.layer, ctx.frame, x, y))
            }
        };
        let seed = color(p.0, p.1);
        let matches = |x: i32, y: i32| distance(color(x, y), seed) <= ctx.tolerance;
        let points = if ctx.contiguous {
            flood_fill(s.width, s.height, p, matches)
        } else {
            (0..s.height as i32)
                .flat_map(|y| (0..s.width as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| matches(x, y))
                .collect()
        };
        selection(Some(Mask::from_points(s.width, s.height, points)))
    }

    fn is_active(&self) -> bool {
        self.at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{LayerId, PixelValue};

    use super::*;
    use crate::test_util::*;

    fn mask_of(out: ToolOutput) -> Mask {
        out.selection.unwrap().unwrap()
    }

    #[test]
    fn marquee_rect_and_ellipse() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Marquee::new(MarqueeShape::Rectangle);
        t.press(&c, (1, 1));
        t.drag(&c, (3, 2));
        assert_eq!(mask_of(t.release(&c)).bounds(), Some(((1, 1), (3, 2))));
        t.press(&c, (5, 5));
        assert_eq!(t.release(&c).selection, Some(None), "click deselects");

        let mut e = Marquee::new(MarqueeShape::Ellipse);
        e.press(&c, (0, 0));
        e.drag(&c, (6, 6));
        let m = mask_of(e.release(&c));
        assert!(m.contains(3, 3) && m.contains(3, 0));
        assert!(!m.contains(0, 0), "corners are outside the ellipse");
    }

    #[test]
    fn lasso_closes_on_release() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = Lasso::new();
        t.press(&c, (0, 0));
        t.drag(&c, (6, 0));
        t.drag(&c, (0, 6));
        let m = mask_of(t.release(&c));
        assert!(m.contains(1, 1) && m.contains(5, 0) && !m.contains(5, 5));
        assert!(!t.is_active());
    }

    #[test]
    fn polygon_lasso_by_clicks() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = PolygonLasso::new();
        for p in [(0, 0), (6, 0), (6, 6)] {
            t.press(&c, p);
            assert_eq!(t.release(&c), ToolOutput::default());
        }
        assert!(t.is_pending() && !t.is_active());
        t.hover(&c, (0, 6));
        assert!(mask_of(t.preview(&c)).contains(1, 5), "rubber band");
        // Clic en el primer vértice: cierra y se entrega al soltar.
        t.press(&c, (0, 1));
        assert!(t.is_active());
        let m = mask_of(t.release(&c));
        assert!(m.contains(5, 1) && !m.contains(1, 5));
        assert!(!t.is_pending());

        t.press(&c, (0, 0));
        t.press(&c, (3, 0));
        t.press(&c, (3, 3));
        assert!(mask_of(t.finish(&c)).contains(2, 1));
        t.press(&c, (1, 1));
        t.cancel();
        assert!(!t.is_pending());
    }

    #[test]
    fn magic_wand_tolerance_contiguous_and_sample() {
        let mut s = sprite();
        let px = |x, y, r| (x, y, PixelValue::Rgba(Rgba::rgb(r, 0, 0)));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![px(0, 0, 200), px(1, 0, 210), px(2, 0, 50), px(3, 0, 200)],
        }
        .apply(&mut s)
        .unwrap();
        let mut c = ctx(&s);
        let mut t = MagicWand::new();
        let pick = |t: &mut MagicWand, c: &ToolContext| {
            t.press(c, (0, 0));
            mask_of(t.release(c))
        };
        let m = pick(&mut t, &c);
        assert_eq!(m.points().collect::<Vec<_>>(), vec![(0, 0)]);
        c.tolerance = 10;
        assert_eq!(pick(&mut t, &c).points().count(), 2);
        c.contiguous = false;
        assert_eq!(pick(&mut t, &c).points().count(), 3, "(3, 0) too");
        c.sample_all = true;
        assert_eq!(pick(&mut t, &c).points().count(), 3);
        t.press(&c, (9, 9));
        assert_eq!(t.release(&c).selection, Some(None));
    }
}
