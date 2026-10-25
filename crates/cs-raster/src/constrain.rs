use crate::Point;

/// Las pendientes del Shift de la línea (en el primer cuadrante): horizontal, 2×1, 45°, 1×2 y
/// vertical. Las de 2×1 y 1×2 son las del pixel art: tramos de 2 píxeles, todos iguales.
const SLOPES: [(i32, i32); 5] = [(1, 0), (2, 1), (1, 1), (1, 2), (0, 1)];

/// Ajusta `b` para que la línea `a`–`b` quede horizontal, vertical, a 45° o en 2×1 / 1×2 (la
/// pendiente más cercana). En 2×1 y 1×2 termina donde cierra el último tramo de 2, para que
/// todos los tramos midan lo mismo.
pub fn constrain_line(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (adx, ady) = (dx.abs() as f32, dy.abs() as f32);
    let angle = ady.atan2(adx);
    let slope_angle = |&(p, q): &(i32, i32)| (q as f32).atan2(p as f32);
    let (p, q) = *SLOPES
        .iter()
        .min_by(|s, t| {
            let ds = (slope_angle(s) - angle).abs();
            let dt = (slope_angle(t) - angle).abs();
            ds.total_cmp(&dt)
        })
        .expect("there are slopes");
    let len = adx.hypot(ady) / ((p * p + q * q) as f32).sqrt();
    let n = len.round() as i32;
    if n == 0 {
        return a;
    }
    // En 2×1 / 1×2, n tramos de 2 terminan uno antes en cada eje.
    let shorten = i32::from(p != q && p != 0 && q != 0);
    let ex = (n * p - shorten).max(0);
    let ey = (n * q - shorten).max(0);
    (a.0 + ex * sign(dx), a.1 + ey * sign(dy))
}

/// Ajusta `b` para que la caja `a`–`b` sea cuadrada.
pub fn constrain_square(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let d = dx.abs().max(dy.abs());
    (a.0 + d * sign(dx), a.1 + d * sign(dy))
}

fn sign(v: i32) -> i32 {
    if v < 0 { -1 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_snaps() {
        assert_eq!(constrain_line((0, 0), (5, 1)), (5, 0));
        assert_eq!(constrain_line((0, 0), (1, -6)), (0, -6));
        assert_eq!(constrain_line((0, 0), (4, 5)), (5, 5));
        assert_eq!(constrain_line((0, 0), (-4, 5)), (-5, 5));
        // 2×1 y 1×2: n tramos de 2.
        assert_eq!(constrain_line((0, 0), (8, 4)), (7, 3));
        assert_eq!(constrain_line((0, 0), (-3, -7)), (-2, -5));
    }

    #[test]
    fn two_by_one_lines_have_even_steps() {
        for n in 1..12 {
            let b = constrain_line((3, 3), (3 + 2 * n, 3 + n));
            let pixels = crate::line((3, 3), b);
            assert_eq!(pixels.len() as i32, 2 * n, "n = {n}");
            for (i, p) in pixels.iter().enumerate() {
                assert_eq!(*p, (3 + i as i32, 3 + i as i32 / 2), "n = {n}");
            }
        }
    }

    #[test]
    fn square_snaps() {
        assert_eq!(constrain_square((0, 0), (3, -7)), (7, -7));
        assert_eq!(constrain_square((1, 1), (1, 1)), (1, 1));
    }
}
