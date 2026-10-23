//! Geometría de la caja de transformación: dónde caen los lados del original, con signo (volteos)
//! y ángulo. Pura: sin egui, en píxeles locales del artboard.

/// Caja de la transformación. `x0`/`y0` son el borde donde cae el lado izquierdo/superior del
/// original y `x1`/`y1` el del derecho/inferior (exclusivos). Con `x1 < x0` el contenido está
/// volteado. El ángulo (radianes, positivo = horario) gira alrededor del centro.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformBox {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub angle: f32,
}

impl TransformBox {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x0: x,
            y0: y,
            x1: x + width as i32,
            y1: y + height as i32,
            angle: 0.0,
        }
    }

    /// Ancho con signo (negativo = volteado).
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }

    /// Lado izquierdo en pantalla (sin rotar).
    pub fn left(&self) -> i32 {
        self.x0.min(self.x1)
    }

    pub fn top(&self) -> i32 {
        self.y0.min(self.y1)
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
            angle: self.angle,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_sides_and_flips() {
        let b = TransformBox::new(2, 3, 4, 5);
        assert_eq!((b.width(), b.height(), b.left(), b.top()), (4, 5, 2, 3));
        let flipped = TransformBox { x0: 6, x1: 2, ..b };
        assert_eq!((flipped.width(), flipped.left()), (-4, 2));
        assert_eq!(b.translated(1, -1), TransformBox::new(3, 2, 4, 5));
    }
}
