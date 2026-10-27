//! Curvas de Bézier cúbicas en píxeles.

use crate::{Point, line, pixel_perfect};

/// La curva cúbica de `a` a `b` con los puntos de control `c1` y `c2`: un trazo continuo (cada
/// píxel toca al anterior), sin repetidos y sin las esquinas en L (pixel perfect).
pub fn cubic_bezier(a: Point, c1: Point, c2: Point, b: Point) -> Vec<Point> {
    let f = |p: Point| (p.0 as f32, p.1 as f32);
    let (a, c1, c2, b) = (f(a), f(c1), f(c2), f(b));
    // Suficientes muestras para que entre una y la siguiente haya pocos píxeles.
    let span = [a, c1, c2, b]
        .windows(2)
        .map(|w| (w[1].0 - w[0].0).abs() + (w[1].1 - w[0].1).abs())
        .sum::<f32>();
    let steps = (span.ceil() as usize).clamp(1, 4096);
    let at = |t: f32| {
        let u = 1.0 - t;
        let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (
            (k0 * a.0 + k1 * c1.0 + k2 * c2.0 + k3 * b.0).round() as i32,
            (k0 * a.1 + k1 * c1.1 + k2 * c2.1 + k3 * b.1).round() as i32,
        )
    };
    let mut path: Vec<Point> = vec![at(0.0)];
    for i in 1..=steps {
        let next = at(i as f32 / steps as f32);
        let last = *path.last().expect("not empty");
        if next != last {
            path.extend(line(last, next).into_iter().skip(1));
        }
    }
    pixel_perfect(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_the_controls_on_the_line_it_is_the_line() {
        assert_eq!(
            cubic_bezier((0, 0), (0, 0), (5, 0), (5, 0)),
            line((0, 0), (5, 0))
        );
    }

    #[test]
    fn a_curve_is_continuous_and_goes_through_its_ends() {
        let c = cubic_bezier((0, 0), (0, 10), (10, 10), (10, 0));
        assert_eq!(c.first(), Some(&(0, 0)));
        assert_eq!(c.last(), Some(&(10, 0)));
        for w in c.windows(2) {
            let (dx, dy) = ((w[1].0 - w[0].0).abs(), (w[1].1 - w[0].1).abs());
            assert!(dx <= 1 && dy <= 1 && (dx, dy) != (0, 0), "{w:?}");
        }
        assert!(c.iter().any(|p| p.1 >= 7), "it bends toward the controls");
    }
}
