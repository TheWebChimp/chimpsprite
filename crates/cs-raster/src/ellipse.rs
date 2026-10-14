use std::collections::BTreeMap;

use crate::{Point, normalize};

/// Contorno de la elipse inscrita en la caja `a`–`b` (algoritmo de Alois Zingl).
pub fn ellipse_outline(a: Point, b: Point) -> Vec<Point> {
    let ((ax, ay), (bx, by)) = normalize(a, b);
    let (mut x0, mut y0, mut x1) = (ax as i64, ay as i64, bx as i64);
    let mut ea = x1 - x0;
    let eb = by as i64 - y0;
    let mut b1 = eb & 1;
    let mut dx = 4 * (1 - ea) * eb * eb;
    let mut dy = 4 * (b1 + 1) * ea * ea;
    let mut err = dx + dy + b1 * ea * ea;
    y0 += (eb + 1) / 2;
    let mut y1 = y0 - b1;
    ea = 8 * ea * ea;
    b1 = 8 * eb * eb;

    let mut out = Vec::new();
    let mut push = |x: i64, y: i64| out.push((x as i32, y as i32));
    loop {
        push(x1, y0);
        push(x0, y0);
        push(x0, y1);
        push(x1, y1);
        let e2 = 2 * err;
        if e2 <= dy {
            y0 += 1;
            y1 -= 1;
            dy += ea;
            err += dy;
        }
        if e2 >= dx || 2 * err > dy {
            x0 += 1;
            x1 -= 1;
            dx += b1;
            err += dx;
        }
        if x0 > x1 {
            break;
        }
    }
    // Las elipses muy planas terminan antes de tiempo: completa las puntas.
    while y0 - y1 <= eb {
        push(x0 - 1, y0);
        push(x1 + 1, y0);
        y0 += 1;
        push(x0 - 1, y1);
        push(x1 + 1, y1);
        y1 -= 1;
    }
    out.sort_by_key(|&(x, y)| (y, x));
    out.dedup();
    out
}

/// Elipse rellena: cada fila del contorno se llena de extremo a extremo.
pub fn ellipse_filled(a: Point, b: Point) -> Vec<Point> {
    let mut rows: BTreeMap<i32, (i32, i32)> = BTreeMap::new();
    for (x, y) in ellipse_outline(a, b) {
        let e = rows.entry(y).or_insert((x, x));
        e.0 = e.0.min(x);
        e.1 = e.1.max(x);
    }
    rows.into_iter()
        .flat_map(|(y, (x0, x1))| (x0..=x1).map(move |x| (x, y)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_shape(a: Point, b: Point) {
        let pts = ellipse_outline(a, b);
        let ((x0, y0), (x1, y1)) = normalize(a, b);
        assert!(
            pts.iter()
                .all(|&(x, y)| x >= x0 && x <= x1 && y >= y0 && y <= y1),
            "{pts:?}"
        );
        assert!(pts.iter().any(|p| p.0 == x0) && pts.iter().any(|p| p.0 == x1));
        assert!(pts.iter().any(|p| p.1 == y0) && pts.iter().any(|p| p.1 == y1));
        for &(x, y) in &pts {
            assert!(
                pts.contains(&(x0 + x1 - x, y)),
                "not h-symmetric at {x},{y}"
            );
            assert!(
                pts.contains(&(x, y0 + y1 - y)),
                "not v-symmetric at {x},{y}"
            );
        }
        let filled = ellipse_filled(a, b);
        assert!(pts.iter().all(|p| filled.contains(p)));
        assert!(filled.contains(&((x0 + x1) / 2, (y0 + y1) / 2)));
    }

    #[test]
    fn degenerate_boxes() {
        assert_eq!(ellipse_outline((3, 3), (3, 3)), vec![(3, 3)]);
        assert_eq!(
            ellipse_outline((0, 0), (3, 0)),
            vec![(0, 0), (1, 0), (2, 0), (3, 0)]
        );
        assert_eq!(
            ellipse_outline((0, 0), (0, 2)),
            vec![(0, 0), (0, 1), (0, 2)]
        );
    }

    #[test]
    fn shapes_are_inscribed_and_symmetric() {
        for (a, b) in [
            ((0, 0), (4, 4)),
            ((0, 0), (7, 5)),
            ((10, 2), (1, 9)),
            ((0, 0), (1, 1)),
            ((0, 0), (15, 3)),
        ] {
            check_shape(a, b);
        }
    }
}
