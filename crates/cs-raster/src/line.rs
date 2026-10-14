//! Líneas, trazos a mano alzada y pinceles.

use std::collections::HashSet;

use crate::Point;

/// Aplica el pincel cuadrado a cada punto, sin duplicados y en orden estable.
pub fn stamp(points: &[Point], size: u32) -> Vec<Point> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for &p in points {
        for q in brush_square(p, size) {
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
}
