//! Algoritmos de rasterizado puros sobre coordenadas enteras.

mod constrain;
mod ellipse;
mod fill;
mod gradient;
mod line;
mod polygon;
mod rect;
mod symmetry;
mod transform;

pub use constrain::{constrain_line, constrain_square};
pub use ellipse::{ellipse_filled, ellipse_outline};
pub use fill::flood_fill;
pub use gradient::{Dither, GradientShape, gradient_t};
pub use line::{brush_square, line, pixel_perfect, stamp};
pub use polygon::polygon_filled;
pub use rect::{rect_filled, rect_outline, rounded_rect_filled, rounded_rect_outline};
pub use symmetry::{SymmetryMode, symmetric_points, symmetrize};
pub use transform::{Block, MAX_BLOCK_SIDE, scale_nearest};

pub type Point = (i32, i32);

/// Normaliza dos esquinas a (mínimo, máximo).
pub(crate) fn normalize(a: Point, b: Point) -> (Point, Point) {
    ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_dedupes_overlaps() {
        assert_eq!(stamp(&[(0, 0), (1, 0)], 1), vec![(0, 0), (1, 0)]);
        assert_eq!(stamp(&[(0, 0), (1, 0)], 2).len(), 6);
    }
}
