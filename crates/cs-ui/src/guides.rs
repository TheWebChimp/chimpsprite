//! Lógica pura de reglas, guías y snap (sin egui).

use cs_core::{Axis, GridSettings, Guide};

/// Grosor de las reglas en pantalla.
pub const RULER_PX: f32 = 18.0;
/// Distancia en pantalla para tomar una guía o pegarse a ella.
pub const SNAP_PX: f32 = 6.0;

/// El menor paso "redondo" (1, 2, 5 × 10ⁿ píxeles) que ocupa al menos `min_px` en pantalla.
pub fn nice_step(zoom: f32, min_px: f32) -> i32 {
    let mut base = 1i64;
    loop {
        for m in [1, 2, 5] {
            let step = base * m;
            if step as f32 * zoom >= min_px || step >= 1_000_000 {
                return step as i32;
            }
        }
        base *= 10;
    }
}

/// Guía bajo el punto local `p` (la más cercana dentro de `tolerance`, en píxeles del sprite).
pub fn hit_guide(guides: &[Guide], p: (f32, f32), tolerance: f32) -> Option<usize> {
    guides
        .iter()
        .enumerate()
        .map(|(i, g)| {
            let v = match g.axis {
                Axis::Horizontal => p.1,
                Axis::Vertical => p.0,
            };
            (i, (v - g.position as f32).abs())
        })
        .filter(|(_, d)| *d <= tolerance)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Lista de guías tras soltar un arrastre: quita la original (si se movía una) y agrega la nueva
/// salvo que se haya soltado sobre la regla o ya exista igual.
pub fn after_drag(guides: &[Guide], moved: Option<usize>, new: Option<Guide>) -> Vec<Guide> {
    let mut out: Vec<Guide> = guides
        .iter()
        .enumerate()
        .filter(|(i, _)| Some(*i) != moved)
        .map(|(_, g)| *g)
        .collect();
    if let Some(g) = new
        && !out.contains(&g)
    {
        out.push(g);
    }
    out
}

/// A qué se pegan las herramientas (en coordenadas locales del artboard activo).
#[derive(Clone, Debug, Default)]
pub struct SnapContext {
    pub guides: Vec<Guide>,
    pub grid: Option<GridSettings>,
    /// Distancia máxima en píxeles del sprite.
    pub tolerance: f32,
}

impl SnapContext {
    fn snap_axis(&self, v: f32, axis: Axis) -> f32 {
        let mut best: Option<(f32, f32)> = None;
        let mut consider = |line: f32| {
            let d = (v - line).abs();
            if d <= self.tolerance && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((line, d));
            }
        };
        for g in self.guides.iter().filter(|g| g.axis == axis) {
            consider(g.position as f32);
        }
        if let Some(grid) = self.grid {
            let (size, offset) = match axis {
                Axis::Vertical => (grid.size.0, grid.offset.0),
                Axis::Horizontal => (grid.size.1, grid.offset.1),
            };
            let size = size.max(1) as f32;
            let k = ((v - offset as f32) / size).round();
            consider(offset as f32 + k * size);
        }
        match best {
            // Queda del lado de la línea donde está el puntero.
            Some((line, _)) if v < line => line - 0.01,
            Some((line, _)) => line + 0.01,
            None => v,
        }
    }

    /// Punto local pegado a guías/grid cercanos.
    pub fn snap(&self, p: (f32, f32)) -> (f32, f32) {
        (
            self.snap_axis(p.0, Axis::Vertical),
            self.snap_axis(p.1, Axis::Horizontal),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: i32) -> Guide {
        Guide {
            axis: Axis::Vertical,
            position: x,
        }
    }

    fn h(y: i32) -> Guide {
        Guide {
            axis: Axis::Horizontal,
            position: y,
        }
    }

    #[test]
    fn steps_grow_with_zoom_out() {
        assert_eq!(nice_step(8.0, 50.0), 10);
        assert_eq!(nice_step(64.0, 50.0), 1);
        assert_eq!(nice_step(1.0, 50.0), 50);
        assert_eq!(nice_step(0.5, 50.0), 100);
        assert_eq!(nice_step(3.0, 50.0), 20);
    }

    #[test]
    fn hit_picks_the_closest_guide_of_the_right_axis() {
        let guides = [v(10), h(10), v(12)];
        assert_eq!(hit_guide(&guides, (11.6, 30.0), 1.0), Some(2));
        assert_eq!(hit_guide(&guides, (30.0, 10.4), 1.0), Some(1));
        assert_eq!(hit_guide(&guides, (20.0, 20.0), 1.0), None);
    }

    #[test]
    fn drag_results() {
        let guides = [v(1), h(2)];
        assert_eq!(after_drag(&guides, Some(0), Some(v(5))), vec![h(2), v(5)]);
        assert_eq!(
            after_drag(&guides, Some(1), None),
            vec![v(1)],
            "dropped on ruler"
        );
        assert_eq!(
            after_drag(&guides, None, Some(h(2))),
            guides.to_vec(),
            "no duplicates"
        );
    }

    #[test]
    fn snapping_to_guides_and_grid_keeps_the_pointer_side() {
        let ctx = SnapContext {
            guides: vec![v(8), h(3)],
            grid: None,
            tolerance: 0.75,
        };
        let (x, y) = ctx.snap((8.4, 2.5));
        assert_eq!((x.floor() as i32, y.floor() as i32), (8, 2));
        assert!((x - 8.01).abs() < 1e-4);
        let (x, _) = ctx.snap((7.6, 0.0));
        assert_eq!(x.floor() as i32, 7);
        assert_eq!(ctx.snap((5.5, 5.5)), (5.5, 5.5), "too far");

        let grid = SnapContext {
            guides: vec![],
            grid: Some(GridSettings {
                size: (16, 16),
                offset: (4, 0),
            }),
            tolerance: 1.0,
        };
        let (x, y) = grid.snap((19.6, 31.5));
        assert_eq!((x.floor() as i32, y.floor() as i32), (19, 31));
        assert!((x - 19.99).abs() < 1e-4 && (y - 31.99).abs() < 1e-4);
    }
}
