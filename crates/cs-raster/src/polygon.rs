use crate::{Point, line};

/// Píxeles de un polígono cerrado: los que tienen el centro dentro (regla par-impar) más el
/// contorno, para que las figuras delgadas no queden vacías.
pub fn polygon_filled(vertices: &[Point]) -> Vec<Point> {
    fill(vertices, false)
}

/// Como `polygon_filled`, pero con la regla de winding no cero: donde el contorno se cruza a sí
/// mismo o se solapa, el relleno no deja hoyos.
pub fn polygon_filled_nonzero(vertices: &[Point]) -> Vec<Point> {
    fill(vertices, true)
}

fn fill(vertices: &[Point], nonzero: bool) -> Vec<Point> {
    let Some(&first) = vertices.first() else {
        return Vec::new();
    };
    let mut out: Vec<Point> = Vec::new();
    for (i, &a) in vertices.iter().enumerate() {
        let b = vertices.get(i + 1).copied().unwrap_or(first);
        out.extend(line(a, b));
    }
    let (y0, y1) = vertices.iter().fold((i32::MAX, i32::MIN), |(lo, hi), p| {
        (lo.min(p.1), hi.max(p.1))
    });
    for y in y0..=y1 {
        let cy = y as f64 + 0.5;
        // Cruces de la fila con cada arista (vértices en el centro del píxel), con el sentido
        // de la arista: +1 hacia abajo, -1 hacia arriba.
        let mut xs: Vec<(f64, i32)> = Vec::new();
        for (i, &a) in vertices.iter().enumerate() {
            let b = vertices.get(i + 1).copied().unwrap_or(first);
            let (ay, by) = (a.1 as f64 + 0.5, b.1 as f64 + 0.5);
            if (ay <= cy) != (by <= cy) {
                let t = (cy - ay) / (by - ay);
                let dir = if by > ay { 1 } else { -1 };
                xs.push((a.0 as f64 + 0.5 + t * (b.0 - a.0) as f64, dir));
            }
        }
        xs.sort_by(|p, q| p.0.total_cmp(&q.0));
        let mut winding = 0;
        for (i, pair) in xs.windows(2).enumerate() {
            winding += pair[0].1;
            // Par-impar: tramos entre el cruce 0 y 1, 2 y 3...; no cero: donde el winding != 0.
            let inside = if nonzero { winding != 0 } else { i % 2 == 0 };
            if inside {
                let from = (pair[0].0 - 0.5).ceil() as i32;
                let to = (pair[1].0 - 0.5).floor() as i32;
                out.extend((from..=to).map(|x| (x, y)));
            }
        }
    }
    out.sort_unstable_by_key(|&(x, y)| (y, x));
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_and_degenerate_shapes() {
        let tri = polygon_filled(&[(0, 0), (4, 0), (0, 4)]);
        assert!(tri.contains(&(1, 1)) && tri.contains(&(0, 4)) && tri.contains(&(2, 2)));
        assert!(!tri.contains(&(3, 3)));
        let square = polygon_filled(&[(0, 0), (3, 0), (3, 3), (0, 3)]);
        assert_eq!(square.len(), 16);
        assert_eq!(polygon_filled(&[(2, 2)]), vec![(2, 2)]);
        assert_eq!(
            polygon_filled(&[(0, 0), (2, 0)]),
            vec![(0, 0), (1, 0), (2, 0)]
        );
        assert!(polygon_filled(&[]).is_empty());
    }

    // Un cuadrado recorrido dos veces: winding 2 adentro (par-impar lo deja hueco).
    const TWICE: [Point; 8] = [
        (0, 0),
        (6, 0),
        (6, 6),
        (0, 6),
        (0, 0),
        (6, 0),
        (6, 6),
        (0, 6),
    ];

    #[test]
    fn nonzero_fills_overlaps_that_even_odd_leaves_empty() {
        assert!(!polygon_filled(&TWICE).contains(&(3, 3)));
        let nz = polygon_filled_nonzero(&TWICE);
        assert!(nz.contains(&(3, 3)));
        assert_eq!(nz.len(), 49);
    }

    #[test]
    fn a_figure_eight_fills_both_lobes() {
        let nz = polygon_filled_nonzero(&[(0, 0), (6, 6), (6, 0), (0, 6)]);
        // Los lóbulos son los triángulos izquierdo y derecho; arriba y abajo quedan vacíos.
        assert!(nz.contains(&(1, 3)) && nz.contains(&(5, 3)));
        assert!(!nz.contains(&(3, 1)) && !nz.contains(&(3, 5)));
    }

    #[test]
    fn simple_polygons_agree_in_both_rules() {
        for poly in [
            vec![(0, 0), (4, 0), (0, 4)],
            vec![(0, 0), (3, 0), (3, 3), (0, 3)],
            vec![(1, 0), (6, 2), (4, 7), (0, 5)],
        ] {
            assert_eq!(polygon_filled(&poly), polygon_filled_nonzero(&poly));
        }
    }
}
