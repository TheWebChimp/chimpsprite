use cs_raster::{Point, line, stamp};

use crate::rng::Rng;
use crate::{Tool, ToolContext, ToolOutput};

/// Puntos por segundo por cada unidad de `spray_speed`.
const DOTS_PER_SPEED: f32 = 6.0;

/// Aerógrafo: puntos aleatorios dentro de un círculo alrededor del cursor, también con el
/// cursor quieto.
#[derive(Debug, Default)]
pub struct Spray {
    active: bool,
    at: Point,
    dots: Vec<Point>,
    rng: Option<Rng>,
    /// Fracción de punto pendiente entre ticks.
    carry: f32,
    strokes: u64,
}

impl Spray {
    pub fn new() -> Self {
        Self::default()
    }

    fn spray(&mut self, ctx: &ToolContext, center: Point, count: usize) {
        let r = ctx.spray_width.max(1) as f32 / 2.0;
        let rng = self.rng.get_or_insert_with(|| Rng::new(1));
        for _ in 0..count {
            // Raíz del radio: densidad uniforme en el círculo.
            let d = r * rng.unit().sqrt();
            let a = rng.unit() * std::f32::consts::TAU;
            self.dots.push((
                center.0 + (d * a.cos()).round() as i32,
                center.1 + (d * a.sin()).round() as i32,
            ));
        }
    }

    fn per_second(ctx: &ToolContext) -> f32 {
        ctx.spray_speed.max(1) as f32 * DOTS_PER_SPEED
    }
}

impl Tool for Spray {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        self.strokes += 1;
        self.active = true;
        self.at = p;
        self.dots.clear();
        self.carry = 0.0;
        self.rng = Some(Rng::new(self.strokes ^ ((p.0 as u64) << 20) ^ p.1 as u64));
        let first = (Self::per_second(ctx) / 30.0).ceil() as usize;
        self.spray(ctx, p, first);
    }

    fn drag(&mut self, ctx: &ToolContext, p: Point) {
        if !self.active || p == self.at {
            return;
        }
        // Al moverse rocía a lo largo del camino, proporcional a la distancia.
        let per_px = (Self::per_second(ctx) / 120.0).max(0.25);
        for q in line(self.at, p).into_iter().skip(1) {
            self.carry += per_px;
            let n = self.carry as usize;
            self.carry -= n as f32;
            self.spray(ctx, q, n);
        }
        self.at = p;
    }

    fn tick(&mut self, ctx: &ToolContext, dt_ms: f32) {
        if !self.active {
            return;
        }
        self.carry += Self::per_second(ctx) * dt_ms / 1000.0;
        let n = self.carry as usize;
        self.carry -= n as f32;
        let at = self.at;
        self.spray(ctx, at, n);
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.active = false;
        self.dots.clear();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        if !self.active || self.dots.is_empty() {
            return ToolOutput::default();
        }
        ToolOutput::command(ctx.draw(
            stamp(&self.dots, ctx.brush_size, ctx.brush_shape),
            ctx.color,
        ))
    }

    fn is_active(&self) -> bool {
        self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn dots_stay_inside_the_circle_and_keep_coming() {
        let s = sprite();
        let mut c = ctx(&s);
        c.spray_width = 6;
        let mut t = Spray::new();
        t.press(&c, (4, 4));
        let first = coords(&t.preview(&c)).len();
        assert!(first > 0);
        t.tick(&c, 500.0);
        let after = coords(&t.preview(&c));
        assert!(after.len() > first, "keeps spraying while still");
        assert!(
            after
                .iter()
                .all(|&(x, y)| (x - 4).abs() <= 3 && (y - 4).abs() <= 3)
        );
        // La vista previa es estable entre cuadros.
        assert_eq!(coords(&t.preview(&c)), after);
        t.drag(&c, (7, 4));
        assert!(coords(&t.release(&c)).iter().any(|&(x, _)| x >= 6));
        assert!(!t.is_active());
        assert_eq!(t.preview(&c), ToolOutput::default());
    }
}
