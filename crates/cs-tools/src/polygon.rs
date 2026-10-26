use cs_raster::{Point, line, polygon_filled, stamp};

use crate::{Tool, ToolContext, ToolOutput};

/// Polígono: cada clic agrega un vértice (con la línea elástica hasta el puntero); se cierra con
/// clic en el primero o con `finish` (doble clic / Enter) y pinta el polígono con el color: relleno
/// con "Filled" o su contorno con el pincel, como el rectángulo y la elipse.
#[derive(Debug, Default)]
pub struct Polygon {
    vertices: Vec<Point>,
    cursor: Option<Point>,
    /// El último clic cerró el polígono; se pinta al soltar.
    closed: bool,
}

impl Polygon {
    pub fn new() -> Self {
        Self::default()
    }

    /// Los píxeles del polígono de `vertices` (cerrado o, mientras se arma, abierto).
    fn points(ctx: &ToolContext, vertices: &[Point], closed: bool) -> Vec<Point> {
        match vertices {
            [] => Vec::new(),
            [p] => stamp(&[*p], ctx.brush_size, ctx.brush_shape),
            _ if closed && ctx.filled && vertices.len() >= 3 => polygon_filled(vertices),
            _ => {
                let mut edges: Vec<Point> =
                    vertices.windows(2).flat_map(|w| line(w[0], w[1])).collect();
                if closed {
                    edges.extend(line(*vertices.last().unwrap(), vertices[0]));
                }
                stamp(&edges, ctx.brush_size, ctx.brush_shape)
            }
        }
    }

    fn output(&self, ctx: &ToolContext, closed: bool, with_cursor: bool) -> ToolOutput {
        let mut vertices = self.vertices.clone();
        if with_cursor && let Some(c) = self.cursor {
            vertices.push(c);
        }
        let points = Self::points(ctx, &vertices, closed);
        if points.is_empty() {
            return ToolOutput::default();
        }
        ToolOutput::command(ctx.draw(points, ctx.color))
    }

    fn reset(&mut self) {
        self.vertices.clear();
        self.cursor = None;
        self.closed = false;
    }
}

impl Tool for Polygon {
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
        let out = self.output(ctx, true, false);
        self.reset();
        out
    }

    /// Mientras se arma: los lados hasta el puntero (abierto); al cerrarse, como queda.
    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if self.vertices.is_empty() {
            return ToolOutput::default();
        }
        if self.closed {
            self.output(ctx, true, false)
        } else {
            self.output(ctx, false, true)
        }
    }

    /// Solo "activo" el instante en que un clic cierra el polígono (para pintarlo al soltar).
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
        let out = self.output(ctx, true, false);
        self.reset();
        out
    }

    fn cancel(&mut self) {
        self.reset();
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};

    use super::*;

    fn ctx(sprite: &Sprite, filled: bool) -> ToolContext<'_> {
        ToolContext {
            sprite,
            layer: LayerId(1),
            frame: 0,
            color: PixelValue::Rgba(Rgba::rgb(255, 0, 0)),
            secondary: PixelValue::Rgba(Rgba::rgb(0, 0, 0)),
            brush_size: 1,
            brush_shape: Default::default(),
            pixel_perfect: false,
            filled,
            corner_radius: 0,
            contiguous: true,
            constrain: false,
            selection: None,
            tolerance: 0,
            sample_all: false,
            spray_width: 1,
            spray_speed: 1,
            gradient_shape: Default::default(),
            dither: Default::default(),
            selected_slice: None,
            tiles: false,
        }
    }

    fn pixels(out: &ToolOutput) -> Vec<(i32, i32)> {
        match &out.command {
            Some(Command::DrawPixels { pixels, .. }) => pixels.iter().map(|p| (p.0, p.1)).collect(),
            _ => Vec::new(),
        }
    }

    /// Un triángulo por clics, cerrado con un clic en el primer vértice.
    fn triangle(t: &mut Polygon, c: &ToolContext) -> ToolOutput {
        for p in [(1, 1), (8, 1), (1, 8)] {
            t.press(c, p);
            assert!(t.release(c).command.is_none(), "nothing until it closes");
        }
        assert!(t.is_pending());
        t.press(c, (1, 1));
        t.release(c)
    }

    #[test]
    fn clicking_back_on_the_first_vertex_paints_the_outline() {
        let sprite = Sprite::new(10, 10, ColorMode::Rgba);
        let c = ctx(&sprite, false);
        let mut t = Polygon::new();
        let px = pixels(&triangle(&mut t, &c));
        assert!(px.contains(&(4, 1)) && px.contains(&(1, 4)) && px.contains(&(4, 5)));
        assert!(!px.contains(&(2, 3)), "not filled");
        assert!(!t.is_pending(), "done");
    }

    #[test]
    fn filled_paints_the_inside_and_enter_closes_it() {
        let sprite = Sprite::new(10, 10, ColorMode::Rgba);
        let c = ctx(&sprite, true);
        let mut t = Polygon::new();
        for p in [(1, 1), (8, 1), (1, 8)] {
            t.press(&c, p);
            t.release(&c);
        }
        let px = pixels(&t.finish(&c));
        assert!(px.contains(&(2, 3)), "filled");
        assert!(!t.is_pending());
    }

    #[test]
    fn the_preview_follows_the_pointer_and_escape_cancels() {
        let sprite = Sprite::new(10, 10, ColorMode::Rgba);
        let c = ctx(&sprite, false);
        let mut t = Polygon::new();
        t.press(&c, (1, 1));
        t.release(&c);
        t.hover(&c, (6, 1));
        assert!(pixels(&t.preview(&c)).contains(&(5, 1)));
        t.cancel();
        assert!(t.preview(&c).command.is_none());
    }
}
