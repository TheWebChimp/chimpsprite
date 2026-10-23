//! Geometría del 9-slice: qué parte de una pieza del skin va a qué parte del destino. Pura, sin
//! egui, en píxeles enteros (de la pieza y de la pantalla).

/// Rectángulo en píxeles enteros. Con `w` o `h` en 0 está vacío.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl IRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// Centro del 9-slice, relativo a la pieza: lo que se estira en las dos direcciones. Alrededor
/// quedan las esquinas (fijas) y los bordes (se estiran a lo largo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect9 {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect9 {
    /// El centro de un slice; `None` si se sale de la pieza de tamaño `size`.
    pub fn new(x: i32, y: i32, w: u32, h: u32, size: [u32; 2]) -> Option<Self> {
        let fits = x >= 0
            && y >= 0
            && (x as u32).checked_add(w).is_some_and(|r| r <= size[0])
            && (y as u32).checked_add(h).is_some_and(|b| b <= size[1]);
        fits.then_some(Self {
            x: x as u32,
            y: y as u32,
            w,
            h,
        })
    }
}

/// Reparte `total` entre el borde de antes (`a`), el medio y el de después (`b`). Si los bordes no
/// entran, se achican en proporción y el medio queda en 0.
fn split(total: i32, a: i32, b: i32) -> (i32, i32, i32) {
    let total = total.max(0);
    if a + b <= total {
        return (a, total - a - b, b);
    }
    // Acá `a + b > total >= 0`: no se divide por cero.
    let first = (total as i64 * a as i64 / (a + b) as i64) as i32;
    (first, 0, total - first)
}

/// Los 9 pares `(origen en la pieza, destino)`, fila por fila desde la esquina de arriba a la
/// izquierda. `scale` es cuántos píxeles de pantalla mide un píxel de la pieza.
///
/// Las esquinas conservan su tamaño ×`scale`. Los bordes se estiran a lo largo y el centro en las
/// dos direcciones. Si `dest` es más chico que las esquinas, se achican para no salirse.
pub fn nine_slice(dest: IRect, size: [u32; 2], center: Rect9, scale: u32) -> [(IRect, IRect); 9] {
    let s = scale.max(1) as i32;
    let (w, h) = (size[0] as i32, size[1] as i32);
    let (cx, cy) = (center.x as i32, center.y as i32);
    let (cw, ch) = (center.w as i32, center.h as i32);
    let (right, bottom) = (w - cx - cw, h - cy - ch);
    let src_x = [(0, cx), (cx, cw), (cx + cw, right)];
    let src_y = [(0, cy), (cy, ch), (cy + ch, bottom)];
    let (l, mw, r) = split(dest.w, cx * s, right * s);
    let (t, mh, b) = split(dest.h, cy * s, bottom * s);
    let dst_x = [(dest.x, l), (dest.x + l, mw), (dest.x + l + mw, r)];
    let dst_y = [(dest.y, t), (dest.y + t, mh), (dest.y + t + mh, b)];
    std::array::from_fn(|i| {
        let (col, row) = (i % 3, i / 3);
        (
            IRect::new(src_x[col].0, src_y[row].0, src_x[col].1, src_y[row].1),
            IRect::new(dst_x[col].0, dst_y[row].0, dst_x[col].1, dst_y[row].1),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Como `window` en el skin: 64×48, barra de título de 13 px y bordes de 3.
    const SIZE: [u32; 2] = [64, 48];
    const CENTER: Rect9 = Rect9 {
        x: 3,
        y: 13,
        w: 58,
        h: 32,
    };

    #[test]
    fn corners_keep_their_size_times_scale() {
        let parts = nine_slice(IRect::new(10, 20, 300, 200), SIZE, CENTER, 2);
        assert_eq!(
            parts[0],
            (IRect::new(0, 0, 3, 13), IRect::new(10, 20, 6, 26))
        );
        assert_eq!(
            parts[2],
            (IRect::new(61, 0, 3, 13), IRect::new(304, 20, 6, 26))
        );
        assert_eq!(
            parts[6],
            (IRect::new(0, 45, 3, 3), IRect::new(10, 214, 6, 6))
        );
        assert_eq!(
            parts[8],
            (IRect::new(61, 45, 3, 3), IRect::new(304, 214, 6, 6))
        );
        // El centro se estira en las dos direcciones.
        assert_eq!(
            parts[4],
            (IRect::new(3, 13, 58, 32), IRect::new(16, 46, 288, 168))
        );
    }

    #[test]
    fn at_its_own_size_each_part_lands_in_place() {
        for (src, dst) in nine_slice(IRect::new(0, 0, 64, 48), SIZE, CENTER, 1) {
            assert_eq!(src, dst);
        }
        // Corrida y a ×3: cada parte cae en su lugar, multiplicada.
        for (src, dst) in nine_slice(IRect::new(5, 7, 192, 144), SIZE, CENTER, 3) {
            assert_eq!(
                dst,
                IRect::new(5 + src.x * 3, 7 + src.y * 3, src.w * 3, src.h * 3)
            );
        }
    }

    #[test]
    fn a_destination_smaller_than_the_corners_stays_inside() {
        let dest = IRect::new(5, 5, 4, 10);
        let parts = nine_slice(dest, SIZE, CENTER, 2);
        for (_, d) in parts {
            assert!(d.w >= 0 && d.h >= 0, "{d:?}");
            assert!(d.x >= dest.x && d.x + d.w <= dest.x + dest.w, "{d:?}");
            assert!(d.y >= dest.y && d.y + d.h <= dest.y + dest.h, "{d:?}");
        }
        let row: i32 = parts[..3].iter().map(|(_, d)| d.w).sum();
        assert_eq!(row, 4, "the row still fills the width");
    }

    #[test]
    fn a_center_outside_the_piece_is_rejected() {
        assert_eq!(Rect9::new(3, 13, 58, 32, SIZE), Some(CENTER));
        assert_eq!(Rect9::new(3, 13, 62, 32, SIZE), None);
        assert_eq!(Rect9::new(-1, 0, 4, 4, SIZE), None);
    }
}
