use crate::{Point, line};

/// Píxeles de un polígono cerrado: los que tienen el centro dentro (regla par-impar) más el
/// contorno, para que las figuras delgadas no queden vacías.
pub fn polygon_filled(vertices: &[Point]) -> Vec<Point> {
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
        // Cruces de la fila con cada arista (vértices en el centro del píxel).
        let mut xs: Vec<f64> = Vec::new();
        for (i, &a) in vertices.iter().enumerate() {
            let b = vertices.get(i + 1).copied().unwrap_or(first);
            let (ay, by) = (a.1 as f64 + 0.5, b.1 as f64 + 0.5);
            if (ay <= cy) != (by <= cy) {
                let t = (cy - ay) / (by - ay);
                xs.push(a.0 as f64 + 0.5 + t * (b.0 - a.0) as f64);
            }
        }
        xs.sort_by(f64::total_cmp);
        for pair in xs.as_chunks::<2>().0 {
            let from = (pair[0] - 0.5).ceil() as i32;
            let to = (pair[1] - 0.5).floor() as i32;
            out.extend((from..=to).map(|x| (x, y)));
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
}
