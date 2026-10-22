use crate::{Point, ellipse_filled, ellipse_outline, normalize};

pub fn rect_filled(a: Point, b: Point) -> Vec<Point> {
    let ((x0, y0), (x1, y1)) = normalize(a, b);
    (y0..=y1)
        .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
        .collect()
}

pub fn rect_outline(a: Point, b: Point) -> Vec<Point> {
    let ((x0, y0), (x1, y1)) = normalize(a, b);
    if x0 == x1 || y0 == y1 {
        return rect_filled(a, b);
    }
    let mut out: Vec<Point> = (x0..=x1).map(|x| (x, y0)).collect();
    for y in y0 + 1..y1 {
        out.push((x0, y));
        out.push((x1, y));
    }
    out.extend((x0..=x1).map(|x| (x, y1)));
    out
}

/// Rectángulo de esquinas redondeadas: cada esquina es un cuarto de la elipse de `2r+1` px de lado
/// (el mismo trazado que la herramienta Ellipse) y los lados son rectos. El radio se limita para
/// que entre: con el máximo queda una píldora o un círculo. Con radio 0 es el rectángulo normal.
pub fn rounded_rect_outline(a: Point, b: Point, radius: u32) -> Vec<Point> {
    rounded(a, b, radius, false)
}

pub fn rounded_rect_filled(a: Point, b: Point, radius: u32) -> Vec<Point> {
    rounded(a, b, radius, true)
}

fn rounded(a: Point, b: Point, radius: u32, filled: bool) -> Vec<Point> {
    let ((x0, y0), (x1, y1)) = normalize(a, b);
    let r = (radius as i32).min((x1 - x0) / 2).min((y1 - y0) / 2);
    if r <= 0 {
        return if filled {
            rect_filled(a, b)
        } else {
            rect_outline(a, b)
        };
    }
    let corner = if filled {
        ellipse_filled((0, 0), (2 * r, 2 * r))
    } else {
        ellipse_outline((0, 0), (2 * r, 2 * r))
    };
    // Cada punto del círculo va a su esquina; los del eje central se estiran por todo el lado.
    let span = |e: i32, lo: i32, hi: i32| {
        if e < r {
            lo + e..=lo + e
        } else if e > r {
            hi - 2 * r + e..=hi - 2 * r + e
        } else {
            lo + r..=hi - r
        }
    };
    let mut out: Vec<Point> = corner
        .into_iter()
        .flat_map(|(ex, ey)| {
            let ys = span(ey, y0, y1);
            span(ex, x0, x1).flat_map(move |x| ys.clone().map(move |y| (x, y)))
        })
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_sizes() {
        assert_eq!(rect_outline((2, 2), (2, 2)), vec![(2, 2)]);
        assert_eq!(rect_outline((0, 0), (2, 2)).len(), 8);
        assert_eq!(rect_filled((0, 0), (2, 2)).len(), 9);
        assert_eq!(rect_outline((0, 0), (3, 0)).len(), 4);
    }

    #[test]
    fn rounded_rect_cuts_the_corners_and_keeps_straight_sides() {
        let outline = rounded_rect_outline((0, 0), (9, 6), 2);
        assert!(
            !outline.contains(&(0, 0)) && !outline.contains(&(9, 6)),
            "corners cut"
        );
        for p in [(4, 0), (5, 6), (0, 3), (9, 3)] {
            assert!(outline.contains(&p), "{p:?} is on a straight side");
        }
        assert!(!outline.contains(&(4, 3)), "hollow");
        let filled = rounded_rect_filled((0, 0), (9, 6), 2);
        assert!(filled.contains(&(4, 3)) && !filled.contains(&(0, 0)));
        assert!(outline.iter().all(|p| filled.contains(p)));
        // Simétrico en los cuatro cuadrantes.
        for &(x, y) in &outline {
            assert!(outline.contains(&(9 - x, y)) && outline.contains(&(x, 6 - y)));
        }
    }

    #[test]
    fn rounded_rect_radius_zero_is_a_rect_and_huge_radius_is_clamped() {
        assert_eq!(
            rounded_rect_outline((0, 0), (5, 3), 0),
            rect_outline((0, 0), (5, 3))
        );
        assert_eq!(
            rounded_rect_filled((0, 0), (6, 6), 99),
            {
                let mut e = ellipse_filled((0, 0), (6, 6));
                e.sort_unstable();
                e
            },
            "a square with the max radius is a circle"
        );
        assert_eq!(
            rounded_rect_outline((3, 3), (0, 0), 1),
            rounded_rect_outline((0, 0), (3, 3), 1)
        );
    }

    #[test]
    fn inverted_corners_are_normalized() {
        assert_eq!(rect_outline((2, 3), (0, 0)), rect_outline((0, 0), (2, 3)));
        assert_eq!(rect_filled((2, 3), (0, 0)), rect_filled((0, 0), (2, 3)));
    }
}
