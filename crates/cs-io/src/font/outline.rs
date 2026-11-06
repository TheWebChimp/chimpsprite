//! Los contornos de los pixeles de un glifo: cada pixel es un cuadrado, pero los vecinos por un
//! lado se unen. Se juntan los lados de la frontera (con lo pintado a la derecha, como pide
//! TrueType), se encadenan en lazos doblando siempre a la derecha donde se cruzan dos (así dos
//! pixeles que solo se tocan por la esquina quedan separados) y se quitan los puntos de en medio
//! de los lados rectos.

use std::collections::{BTreeMap, BTreeSet};

/// Un punto de la cuadrícula de pixeles, con y hacia arriba: `y = 0` es la baseline.
pub type Point = (i32, i32);

/// Los contornos de `pixels` (columna y fila desde arriba de la celda), con la baseline en la
/// fila `baseline` (las filas `0..baseline` quedan arriba de ella). Los de afuera van en sentido
/// horario y los huecos al revés; cada uno empieza en una esquina y no repite el primer punto.
pub fn contours(pixels: &[(u32, u32)], baseline: u32) -> Vec<Vec<Point>> {
    let set: BTreeSet<(i64, i64)> = pixels.iter().map(|&(x, y)| (x as i64, y as i64)).collect();
    let filled = |x: i64, y: i64| set.contains(&(x, y));
    // Los lados de la frontera: de dónde salen y a dónde llegan.
    let mut edges: Vec<(Point, Point)> = Vec::new();
    for &(x, y) in &set {
        let (x0, x1) = (x as i32, x as i32 + 1);
        let bottom = baseline as i32 - y as i32 - 1;
        let top = bottom + 1;
        if !filled(x, y - 1) {
            edges.push(((x0, top), (x1, top)));
        }
        if !filled(x + 1, y) {
            edges.push(((x1, top), (x1, bottom)));
        }
        if !filled(x, y + 1) {
            edges.push(((x1, bottom), (x0, bottom)));
        }
        if !filled(x - 1, y) {
            edges.push(((x0, bottom), (x0, top)));
        }
    }
    edges.sort_unstable();
    let mut leaving: BTreeMap<Point, Vec<usize>> = BTreeMap::new();
    for (i, (from, _)) in edges.iter().enumerate() {
        leaving.entry(*from).or_default().push(i);
    }
    // El que sigue a cada lado: el que dobla a la derecha si hay dos; si no, el único.
    let next = |i: usize| -> usize {
        let (from, to) = edges[i];
        let heading = (to.0 - from.0, to.1 - from.1);
        let options = &leaving[&to];
        if options.len() == 1 {
            return options[0];
        }
        *options
            .iter()
            .min_by_key(|&&j| {
                let (a, b) = edges[j];
                let turn = (b.0 - a.0, b.1 - a.1);
                // Producto cruz: negativo es doblar a la derecha.
                heading.0 * turn.1 - heading.1 * turn.0
            })
            .expect("a boundary vertex has a way out")
    };
    let mut used = vec![false; edges.len()];
    let mut out = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut cycle = Vec::new();
        let mut i = start;
        while !used[i] {
            used[i] = true;
            cycle.push(i);
            i = next(i);
        }
        out.push(simplify(&edges, &cycle));
    }
    out
}

/// Los puntos de un cycle de lados, sin los de en medio de un lado recto, empezando en una
/// esquina.
fn simplify(edges: &[(Point, Point)], cycle: &[usize]) -> Vec<Point> {
    let n = cycle.len();
    let dir = |k: usize| {
        let (a, b) = edges[cycle[k % n]];
        (b.0 - a.0, b.1 - a.1)
    };
    // Un punto (el inicio del lado k) es esquina si el lado de antes va hacia otro lado.
    (0..n)
        .filter(|&k| dir(k + n - 1) != dir(k))
        .map(|k| edges[cycle[k]].0)
        .collect()
}

/// El área con signo de un contorno (positiva en sentido antihorario, con y hacia arriba).
pub fn signed_area(points: &[Point]) -> i64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.0 as i64 * b.1 as i64 - b.0 as i64 * a.1 as i64
        })
        .sum::<i64>()
        / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_pixel_is_a_clockwise_square() {
        let c = contours(&[(0, 0)], 1);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].len(), 4);
        assert_eq!(signed_area(&c[0]), -1, "clockwise");
        let set: BTreeSet<Point> = c[0].iter().copied().collect();
        assert_eq!(set, BTreeSet::from([(0, 0), (1, 0), (0, 1), (1, 1)]));
    }

    #[test]
    fn the_baseline_puts_rows_above_and_below_zero() {
        // La fila 3 con la baseline en 3 queda justo debajo: de y = -1 a 0.
        let c = contours(&[(2, 3)], 3);
        let ys: BTreeSet<i32> = c[0].iter().map(|p| p.1).collect();
        assert_eq!(ys, BTreeSet::from([-1, 0]));
    }

    #[test]
    fn neighbours_merge_and_straight_sides_lose_their_middle_points() {
        // Una L de tres pixeles: un contorno de 6 esquinas.
        let c = contours(&[(0, 0), (0, 1), (1, 1)], 2);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].len(), 6);
        assert_eq!(signed_area(&c[0]), -3);
        // Una fila de cinco: un rectángulo de 4 puntos.
        let row: Vec<(u32, u32)> = (0..5).map(|x| (x, 0)).collect();
        let c = contours(&row, 1);
        assert_eq!(c[0].len(), 4);
        assert_eq!(signed_area(&c[0]), -5);
    }

    #[test]
    fn a_ring_has_an_outside_and_a_counter_clockwise_hole() {
        let mut ring = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                if (x, y) != (1, 1) {
                    ring.push((x, y));
                }
            }
        }
        let c = contours(&ring, 3);
        assert_eq!(c.len(), 2);
        let mut areas: Vec<i64> = c.iter().map(|p| signed_area(p)).collect();
        areas.sort();
        assert_eq!(
            areas,
            vec![-9, 1],
            "outside clockwise, hole counter-clockwise"
        );
        assert!(c.iter().all(|p| p.len() == 4));
    }

    #[test]
    fn pixels_touching_by_a_corner_stay_two_contours() {
        let c = contours(&[(0, 0), (1, 1)], 2);
        assert_eq!(c.len(), 2);
        assert!(c.iter().all(|p| p.len() == 4 && signed_area(p) == -1));
        // Un tablero de 3×3: cinco cuadrados, el del centro tocando a los cuatro por la esquina.
        let c = contours(&[(0, 0), (2, 0), (1, 1), (0, 2), (2, 2)], 3);
        assert_eq!(c.len(), 5);
        assert!(c.iter().all(|p| p.len() == 4 && signed_area(p) == -1));
    }

    #[test]
    fn nothing_gives_no_contours() {
        assert!(contours(&[], 4).is_empty());
    }
}
