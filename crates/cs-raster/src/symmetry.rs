use std::collections::HashSet;

use crate::Point;

pub use cs_core::SymmetryMode;

#[derive(Clone, Copy)]
enum Mirror {
    H,
    V,
    D1,
    D2,
}

fn apply(m: Mirror, (x, y): Point, (cx2, cy2): (i32, i32)) -> Point {
    // Reflejo de centros de píxel: x' + ½ = 2c − (x + ½)  ⇒  x' = 2c − 1 − x.
    match m {
        Mirror::H => (cx2 - 1 - x, y),
        Mirror::V => (x, cy2 - 1 - y),
        // (dx, dy) → (dy, dx) respecto del centro.
        Mirror::D1 => (y + (cx2 - cy2).div_euclid(2), x + (cy2 - cx2).div_euclid(2)),
        // (dx, dy) → (−dy, −dx).
        Mirror::D2 => {
            let s = (cx2 + cy2).div_euclid(2) - 1;
            (s - y, s - x)
        }
    }
}

fn generators(mode: SymmetryMode) -> &'static [Mirror] {
    match mode {
        SymmetryMode::None => &[],
        SymmetryMode::Horizontal => &[Mirror::H],
        SymmetryMode::Vertical => &[Mirror::V],
        SymmetryMode::Both => &[Mirror::H, Mirror::V],
        SymmetryMode::DiagonalDown => &[Mirror::D1],
        SymmetryMode::DiagonalUp => &[Mirror::D2],
        SymmetryMode::All => &[Mirror::H, Mirror::V, Mirror::D1],
    }
}

/// Todos los reflejos de `p` (incluido `p`), sin repetir.
pub fn symmetric_points(p: Point, mode: SymmetryMode, center2: (i32, i32)) -> Vec<Point> {
    let gens = generators(mode);
    let mut out = vec![p];
    let mut i = 0;
    // Cierre del grupo generado: aplica cada espejo a cada punto nuevo.
    while i < out.len() {
        for &g in gens {
            let q = apply(g, out[i], center2);
            if !out.contains(&q) {
                out.push(q);
            }
        }
        i += 1;
    }
    out
}

/// Reflejos de una lista de píxeles con su valor; el primero que cae en un lugar gana.
pub fn symmetrize<T: Copy>(
    pixels: &[(i32, i32, T)],
    mode: SymmetryMode,
    center2: (i32, i32),
) -> Vec<(i32, i32, T)> {
    if mode == SymmetryMode::None {
        return pixels.to_vec();
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(pixels.len() * 2);
    for &(x, y, v) in pixels {
        for (qx, qy) in symmetric_points((x, y), mode, center2) {
            if seen.insert((qx, qy)) {
                out.push((qx, qy, v));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_even_and_odd_widths() {
        // Ancho 8: eje entre 3 y 4 (center2 = 8).
        assert_eq!(
            symmetric_points((1, 2), SymmetryMode::Horizontal, (8, 8)),
            vec![(1, 2), (6, 2)]
        );
        // Ancho 7: eje en el centro del píxel 3 (center2 = 7); ese píxel es su propio reflejo.
        assert_eq!(
            symmetric_points((3, 0), SymmetryMode::Horizontal, (7, 7)),
            vec![(3, 0)]
        );
        assert_eq!(
            symmetric_points((0, 0), SymmetryMode::Horizontal, (7, 7)),
            vec![(0, 0), (6, 0)]
        );
    }

    #[test]
    fn vertical_and_both() {
        assert_eq!(
            symmetric_points((2, 1), SymmetryMode::Vertical, (8, 8)),
            vec![(2, 1), (2, 6)]
        );
        let mut both = symmetric_points((1, 2), SymmetryMode::Both, (8, 8));
        both.sort();
        assert_eq!(both, vec![(1, 2), (1, 5), (6, 2), (6, 5)]);
    }

    #[test]
    fn diagonals() {
        // Sprite 8×8: ╲ intercambia x e y; ╱ refleja sobre la antidiagonal.
        assert_eq!(
            symmetric_points((1, 5), SymmetryMode::DiagonalDown, (8, 8)),
            vec![(1, 5), (5, 1)]
        );
        assert_eq!(
            symmetric_points((1, 5), SymmetryMode::DiagonalUp, (8, 8)),
            vec![(1, 5), (2, 6)]
        );
        // Centro desplazado: ╲ por (6, 2) en medios píxeles.
        assert_eq!(
            symmetric_points((3, 1), SymmetryMode::DiagonalDown, (12, 4)),
            vec![(3, 1), (5, -1)]
        );
    }

    #[test]
    fn all_gives_eight_copies() {
        let mut all = symmetric_points((1, 2), SymmetryMode::All, (8, 8));
        all.sort();
        assert_eq!(
            all,
            vec![
                (1, 2),
                (1, 5),
                (2, 1),
                (2, 6),
                (5, 1),
                (5, 6),
                (6, 2),
                (6, 5)
            ]
        );
        // El centro exacto no se duplica.
        assert_eq!(
            symmetric_points((3, 3), SymmetryMode::All, (7, 7)),
            vec![(3, 3)]
        );
    }

    #[test]
    fn symmetrize_keeps_values_and_dedupes() {
        let px = [(0, 0, 'a'), (7, 0, 'b')];
        let out = symmetrize(&px, SymmetryMode::Horizontal, (8, 8));
        assert_eq!(out, vec![(0, 0, 'a'), (7, 0, 'a')], "first writer wins");
        assert_eq!(symmetrize(&px, SymmetryMode::None, (8, 8)), px.to_vec());
    }
}
