use std::collections::HashSet;

use crate::Point;

pub use cs_core::SymmetryAxes;

/// Una de las 8 simetrías del cuadrado: matriz entera `[a, b, c, d]` que lleva la distancia al
/// centro, en medios píxeles, de `(dx, dy)` a `(a·dx + b·dy, c·dx + d·dy)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Transform([i32; 4]);

const IDENTITY: Transform = Transform([1, 0, 0, 1]);
/// Espejo izquierda ↔ derecha (eje vertical │).
const FLIP_X: Transform = Transform([-1, 0, 0, 1]);
/// Espejo arriba ↔ abajo (eje horizontal ─).
const FLIP_Y: Transform = Transform([1, 0, 0, -1]);
const ROTATE_180: Transform = Transform([-1, 0, 0, -1]);
/// Espejo sobre la diagonal ╲: (dx, dy) → (dy, dx).
const DIAGONAL_DOWN: Transform = Transform([0, 1, 1, 0]);
/// Espejo sobre la diagonal ╱: (dx, dy) → (−dy, −dx).
const DIAGONAL_UP: Transform = Transform([0, -1, -1, 0]);
const ROTATE_90: Transform = Transform([0, -1, 1, 0]);
const ROTATE_270: Transform = Transform([0, 1, -1, 0]);

/// Las 8 simetrías del cuadrado.
const SQUARE: [Transform; 8] = [
    IDENTITY,
    FLIP_X,
    FLIP_Y,
    ROTATE_180,
    DIAGONAL_DOWN,
    DIAGONAL_UP,
    ROTATE_90,
    ROTATE_270,
];

/// El grupo que generan los ejes prendidos, con la identidad primero. Va por tabla (no se
/// cierran los espejos uno con otro): así siempre son 1, 2, 4 u 8 transformaciones, aunque el
/// centro no caiga en el medio de un píxel para las diagonales.
fn group(axes: SymmetryAxes) -> &'static [Transform] {
    let SymmetryAxes {
        vertical,
        horizontal,
        diagonal_down,
        diagonal_up,
    } = axes;
    match (vertical, horizontal, diagonal_down, diagonal_up) {
        (false, false, false, false) => &[IDENTITY],
        (true, false, false, false) => &[IDENTITY, FLIP_X],
        (false, true, false, false) => &[IDENTITY, FLIP_Y],
        (true, true, false, false) => &[IDENTITY, FLIP_X, FLIP_Y, ROTATE_180],
        (false, false, true, false) => &[IDENTITY, DIAGONAL_DOWN],
        (false, false, false, true) => &[IDENTITY, DIAGONAL_UP],
        (false, false, true, true) => &[IDENTITY, DIAGONAL_DOWN, DIAGONAL_UP, ROTATE_180],
        // Un eje recto con una diagonal genera la rotación de 90°: el cuadrado completo.
        _ => &SQUARE,
    }
}

fn apply(t: Transform, (x, y): Point, (cx2, cy2): (i32, i32)) -> Point {
    let [a, b, c, d] = t.0;
    // Centro del píxel menos el centro de simetría, en medios píxeles.
    let (dx, dy) = (2 * x + 1 - cx2, 2 * y + 1 - cy2);
    let (qx, qy) = (cx2 + a * dx + b * dy, cy2 + c * dx + d * dy);
    // De medios píxeles al píxel. Si el centro cae en un borde (solo pasa con las diagonales y
    // un centro de x e y con distinta paridad), va al píxel de la izquierda o de arriba.
    ((qx - 1).div_euclid(2), (qy - 1).div_euclid(2))
}

/// Todas las copias de `p` (incluido `p`), sin repetir.
pub fn symmetric_points(p: Point, axes: SymmetryAxes, center2: (i32, i32)) -> Vec<Point> {
    let mut out = Vec::with_capacity(8);
    for &t in group(axes) {
        let q = apply(t, p, center2);
        if !out.contains(&q) {
            out.push(q);
        }
    }
    out
}

/// Copias de una lista de píxeles con su valor; el primero que cae en un lugar gana.
pub fn symmetrize<T: Copy>(
    pixels: &[(i32, i32, T)],
    axes: SymmetryAxes,
    center2: (i32, i32),
) -> Vec<(i32, i32, T)> {
    if !axes.any() {
        return pixels.to_vec();
    }
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(pixels.len() * group(axes).len());
    for &(x, y, v) in pixels {
        for (qx, qy) in symmetric_points((x, y), axes, center2) {
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

    /// Ejes en el orden de la barra: │ ─ ╲ ╱.
    fn axes(
        vertical: bool,
        horizontal: bool,
        diagonal_down: bool,
        diagonal_up: bool,
    ) -> SymmetryAxes {
        SymmetryAxes {
            vertical,
            horizontal,
            diagonal_down,
            diagonal_up,
        }
    }

    const V: SymmetryAxes = SymmetryAxes {
        vertical: true,
        ..SymmetryAxes::NONE
    };
    const H: SymmetryAxes = SymmetryAxes {
        horizontal: true,
        ..SymmetryAxes::NONE
    };
    const DOWN: SymmetryAxes = SymmetryAxes {
        diagonal_down: true,
        ..SymmetryAxes::NONE
    };
    const UP: SymmetryAxes = SymmetryAxes {
        diagonal_up: true,
        ..SymmetryAxes::NONE
    };

    fn sorted(mut v: Vec<Point>) -> Vec<Point> {
        v.sort();
        v
    }

    fn compose(t: Transform, u: Transform) -> Transform {
        let ([a, b, c, d], [e, f, g, h]) = (t.0, u.0);
        Transform([a * e + b * g, a * f + b * h, c * e + d * g, c * f + d * h])
    }

    #[test]
    fn vertical_axis_with_even_and_odd_widths() {
        // Ancho 8: eje entre 3 y 4 (center2 = 8).
        assert_eq!(symmetric_points((1, 2), V, (8, 8)), vec![(1, 2), (6, 2)]);
        // Ancho 7: eje en el centro del píxel 3 (center2 = 7); ese píxel es su propio reflejo.
        assert_eq!(symmetric_points((3, 0), V, (7, 7)), vec![(3, 0)]);
        assert_eq!(symmetric_points((0, 0), V, (7, 7)), vec![(0, 0), (6, 0)]);
    }

    #[test]
    fn horizontal_axis_and_both_straight_axes() {
        assert_eq!(symmetric_points((2, 1), H, (8, 8)), vec![(2, 1), (2, 6)]);
        assert_eq!(
            sorted(symmetric_points(
                (1, 2),
                axes(true, true, false, false),
                (8, 8)
            )),
            vec![(1, 2), (1, 5), (6, 2), (6, 5)]
        );
    }

    #[test]
    fn diagonals() {
        // Sprite 8×8: ╲ intercambia x e y; ╱ refleja sobre la antidiagonal.
        assert_eq!(symmetric_points((1, 5), DOWN, (8, 8)), vec![(1, 5), (5, 1)]);
        assert_eq!(symmetric_points((1, 5), UP, (8, 8)), vec![(1, 5), (2, 6)]);
        // Centro desplazado: ╲ por (6, 2) en medios píxeles.
        assert_eq!(
            symmetric_points((3, 1), DOWN, (12, 4)),
            vec![(3, 1), (5, -1)]
        );
    }

    #[test]
    fn both_diagonals_add_the_half_turn() {
        // ╲ + ╱: los dos espejos diagonales y la rotación de 180°.
        assert_eq!(
            sorted(symmetric_points(
                (1, 2),
                axes(false, false, true, true),
                (8, 8)
            )),
            vec![(1, 2), (2, 1), (5, 6), (6, 5)]
        );
    }

    #[test]
    fn a_straight_axis_and_a_diagonal_give_the_eight_copies() {
        let eight = vec![
            (1, 2),
            (1, 5),
            (2, 1),
            (2, 6),
            (5, 1),
            (5, 6),
            (6, 2),
            (6, 5),
        ];
        for a in [
            axes(true, false, true, false),
            axes(false, true, false, true),
            axes(true, true, true, false),
            axes(true, true, true, true),
        ] {
            assert_eq!(sorted(symmetric_points((1, 2), a, (8, 8))), eight, "{a:?}");
        }
        // El centro exacto no se duplica.
        assert_eq!(
            symmetric_points((3, 3), axes(true, true, true, true), (7, 7)),
            vec![(3, 3)]
        );
    }

    #[test]
    fn every_combination_is_a_group_of_1_2_4_or_8() {
        for bits in 0..16u8 {
            let a = axes(bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
            let g = group(a);
            let straight = usize::from(a.vertical) + usize::from(a.horizontal);
            let diagonal = usize::from(a.diagonal_down) + usize::from(a.diagonal_up);
            let expected = match (straight, diagonal) {
                (0, 0) => 1,
                (1, 0) | (0, 1) => 2,
                (2, 0) | (0, 2) => 4,
                _ => 8,
            };
            assert_eq!(g.len(), expected, "{a:?}");
            assert_eq!(g[0], IDENTITY, "{a:?}");
            // Cerrado: componer dos transformaciones del grupo da otra del grupo.
            for &t in g {
                for &u in g {
                    assert!(g.contains(&compose(t, u)), "{a:?}");
                }
            }
        }
    }

    #[test]
    fn an_off_center_diagonal_still_gives_a_few_copies() {
        // 8×7: el centro de ╲ no cae en el medio de un píxel. Cerrar los espejos uno con otro
        // (como antes) no terminaba nunca; por tabla son a lo sumo las del grupo.
        assert_eq!(symmetric_points((1, 2), DOWN, (8, 7)).len(), 2);
        assert!(symmetric_points((1, 2), axes(true, true, true, true), (8, 7)).len() <= 8);
    }

    #[test]
    fn symmetrize_keeps_values_and_dedupes() {
        let px = [(0, 0, 'a'), (7, 0, 'b')];
        let out = symmetrize(&px, V, (8, 8));
        assert_eq!(out, vec![(0, 0, 'a'), (7, 0, 'a')], "first writer wins");
        assert_eq!(symmetrize(&px, SymmetryAxes::NONE, (8, 8)), px.to_vec());
    }
}
