use crate::Point;

/// tan(22.5°): umbral para ajustar a horizontal/vertical en vez de diagonal.
const SNAP: f32 = 0.414_213_56;

/// Ajusta `b` para que la línea `a`–`b` quede a 0°, 45° o 90°.
pub fn constrain_line(a: Point, b: Point) -> Point {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (adx, ady) = (dx.abs() as f32, dy.abs() as f32);
    if ady < adx * SNAP {
        (b.0, a.1)
    } else if adx < ady * SNAP {
        (a.0, b.1)
    } else {
        let d = dx.abs().max(dy.abs());
        (a.0 + d * sign(dx), a.1 + d * sign(dy))
    }
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
    }

    #[test]
    fn square_snaps() {
        assert_eq!(constrain_square((0, 0), (3, -7)), (7, -7));
        assert_eq!(constrain_square((1, 1), (1, 1)), (1, 1));
    }
}
