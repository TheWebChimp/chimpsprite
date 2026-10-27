//! Algoritmos de rasterizado puros sobre coordenadas enteras.

mod blur;
mod constrain;
mod ellipse;
mod fill;
mod gradient;
mod line;
mod polygon;
mod rect;
mod symmetry;
mod transform;

pub use blur::blur_average;
pub use constrain::{constrain_line, constrain_square};
pub use ellipse::{ellipse_filled, ellipse_outline};
pub use fill::flood_fill;
pub use gradient::{Dither, GradientShape, gradient_t};
pub use line::{
    BrushShape, brush_points, brush_square, line, pixel_perfect, set_custom_brush, stamp,
};
pub use polygon::{polygon_filled, polygon_filled_nonzero, polygon_filled_nonzero_in};
pub use rect::{rect_filled, rect_outline, rounded_rect_filled, rounded_rect_outline};
pub use symmetry::{SymmetryAxes, symmetric_points, symmetrize};
pub use transform::{
    Block, MAX_BLOCK_SIDE, ROTSPRITE_MAX_AREA, RotationAlgorithm, rotate_fast, rotated_size,
    rotsprite, scale_nearest,
};

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
        assert_eq!(
            stamp(&[(0, 0), (1, 0)], 1, BrushShape::Square),
            vec![(0, 0), (1, 0)]
        );
        assert_eq!(stamp(&[(0, 0), (1, 0)], 2, BrushShape::Square).len(), 6);
        // Dos cruces (redondo de 3) vecinas: 5 + 5 menos los 2 píxeles compartidos.
        assert_eq!(stamp(&[(0, 0), (1, 0)], 3, BrushShape::Round).len(), 8);
    }
}
