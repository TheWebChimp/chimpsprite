//! Líneas, trazos a mano alzada y pinceles.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use cs_core::Rgba;

use crate::{Point, ellipse_filled};

/// Forma del pincel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushShape {
    /// Círculo relleno, con el mismo rasterizador que la herramienta Ellipse.
    #[default]
    Round,
    Square,
    /// El pincel personalizado (Edit → New Brush): la forma que dejó [`set_custom_brush`], sin
    /// importar el tamaño. Sin forma, un píxel.
    Custom,
}

thread_local! {
    /// El pincel personalizado: sus puntos, relativos a su centro, con el color de cada uno (el
    /// del dibujo del que salió). Es del hilo de la interfaz (cada test tiene el suyo).
    static CUSTOM_BRUSH: std::cell::RefCell<Vec<(Point, Rgba)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Deja `brush` (puntos relativos al centro y sus colores) como [`BrushShape::Custom`].
pub fn set_custom_brush(brush: Vec<(Point, Rgba)>) {
    CUSTOM_BRUSH.with(|custom| *custom.borrow_mut() = brush);
}

/// El pincel personalizado: sus puntos relativos al centro y sus colores (vacío si no hay).
pub fn custom_brush() -> Vec<(Point, Rgba)> {
    CUSTOM_BRUSH.with(|custom| custom.borrow().clone())
}

/// Puntos del pincel de lado `size` centrado en `center`. Las dos formas ocupan la misma caja
/// (sesgo hacia abajo/derecha en tamaños pares); en 1 y 2 son iguales.
pub fn brush_points(center: Point, size: u32, shape: BrushShape) -> Vec<Point> {
    match shape {
        BrushShape::Custom => {
            let brush = custom_brush();
            if brush.is_empty() {
                return vec![center];
            }
            brush
                .into_iter()
                .map(|((x, y), _)| (center.0 + x, center.1 + y))
                .collect()
        }
        BrushShape::Square => brush_square(center, size),
        BrushShape::Round => {
            let size = size.max(1) as i32;
            let start = -(size - 1) / 2;
            let a = (center.0 + start, center.1 + start);
            ellipse_filled(a, (a.0 + size - 1, a.1 + size - 1))
        }
    }
}

/// Aplica el pincel a cada punto, sin duplicados y en orden estable.
pub fn stamp(points: &[Point], size: u32, shape: BrushShape) -> Vec<Point> {
    // La forma se calcula una sola vez y se desplaza a cada punto.
    let brush = brush_points((0, 0), size, shape);
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &(px, py) in points {
        for &(dx, dy) in &brush {
            let q = (px + dx, py + dy);
            if seen.insert(q) {
                out.push(q);
            }
        }
    }
    out
}

/// Línea de Bresenham de `a` a `b`, ambos incluidos.
pub fn line(a: Point, b: Point) -> Vec<Point> {
    let (mut x, mut y) = a;
    let (x1, y1) = b;
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::with_capacity((dx - dy + 1) as usize);
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

/// Quita duplicados consecutivos y esquinas en "L" de un trazo a mano alzada.
pub fn pixel_perfect(path: &[Point]) -> Vec<Point> {
    let mut deduped: Vec<Point> = Vec::with_capacity(path.len());
    for &p in path {
        if deduped.last() != Some(&p) {
            deduped.push(p);
        }
    }
    let mut out: Vec<Point> = Vec::with_capacity(deduped.len());
    for (i, &c) in deduped.iter().enumerate() {
        if let (Some(&p), Some(&n)) = (out.last(), deduped.get(i + 1)) {
            let touches_prev = p.0 == c.0 || p.1 == c.1;
            let touches_next = n.0 == c.0 || n.1 == c.1;
            if touches_prev && touches_next && p.0 != n.0 && p.1 != n.1 {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Pincel cuadrado de lado `size` centrado en `center` (sesgo hacia abajo/derecha en tamaños pares).
pub fn brush_square(center: Point, size: u32) -> Vec<Point> {
    let size = size.max(1) as i32;
    let start = -(size - 1) / 2;
    let mut out = Vec::with_capacity((size * size) as usize);
    for dy in start..start + size {
        for dx in start..start + size {
            out.push((center.0 + dx, center.1 + dy));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_single_point() {
        assert_eq!(line((2, 3), (2, 3)), vec![(2, 3)]);
    }

    #[test]
    fn line_shallow() {
        assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
    }

    #[test]
    fn line_reverse_and_vertical() {
        assert_eq!(line((0, 2), (0, 0)), vec![(0, 2), (0, 1), (0, 0)]);
        assert_eq!(line((2, 2), (0, 0)), vec![(2, 2), (1, 1), (0, 0)]);
    }

    #[test]
    fn pixel_perfect_removes_l_corners() {
        assert_eq!(
            pixel_perfect(&[(0, 0), (1, 0), (1, 1)]),
            vec![(0, 0), (1, 1)]
        );
    }

    #[test]
    fn pixel_perfect_keeps_straight_lines_and_dedupes() {
        assert_eq!(
            pixel_perfect(&[(0, 0), (1, 0), (1, 0), (2, 0)]),
            vec![(0, 0), (1, 0), (2, 0)]
        );
    }

    #[test]
    fn brush_sizes() {
        assert_eq!(brush_square((5, 5), 1), vec![(5, 5)]);
        assert_eq!(
            brush_square((0, 0), 2),
            vec![(0, 0), (1, 0), (0, 1), (1, 1)]
        );
        assert_eq!(brush_square((0, 0), 3).len(), 9);
        assert!(brush_square((0, 0), 3).contains(&(-1, -1)));
    }

    #[test]
    fn round_brush_matches_square_at_1_and_2() {
        for size in [1, 2] {
            assert_eq!(
                brush_points((5, 5), size, BrushShape::Round),
                brush_points((5, 5), size, BrushShape::Square),
                "size {size}"
            );
        }
    }

    #[test]
    fn round_brush_of_3_is_a_cross() {
        assert_eq!(
            brush_points((0, 0), 3, BrushShape::Round),
            vec![(0, -1), (-1, 0), (0, 0), (1, 0), (0, 1)]
        );
    }

    #[test]
    fn round_brush_of_5_is_symmetric_without_corners() {
        let pts = brush_points((0, 0), 5, BrushShape::Round);
        // .###.
        // #####
        // #####
        // #####
        // .###.
        assert_eq!(pts.len(), 21);
        for c in [(-2, -2), (2, -2), (-2, 2), (2, 2)] {
            assert!(!pts.contains(&c), "corner {c:?}");
        }
        assert!(pts.iter().all(|&(x, y)| x.abs() <= 2 && y.abs() <= 2));
        for &(x, y) in &pts {
            assert!(pts.contains(&(-x, y)), "h-symmetric at {x},{y}");
            assert!(pts.contains(&(x, -y)), "v-symmetric at {x},{y}");
            assert!(pts.contains(&(y, x)), "diagonal-symmetric at {x},{y}");
        }
    }

    #[test]
    fn square_brush_is_unchanged() {
        assert_eq!(
            brush_points((0, 0), 3, BrushShape::Square),
            brush_square((0, 0), 3)
        );
        assert_eq!(brush_points((0, 0), 4, BrushShape::Square).len(), 16);
    }

    #[test]
    fn the_default_brush_is_round() {
        assert_eq!(BrushShape::default(), BrushShape::Round);
    }
}
