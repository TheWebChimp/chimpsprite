//! Escalar y rotar bloques de píxeles (la selección flotante). Funciones puras: no saben del modo
//! de color ni de la UI.

/// Lado máximo de un bloque transformado; más grande no se calcula.
pub const MAX_BLOCK_SIDE: u32 = 4096;

/// Bloque rectangular de píxeles. `None` es un punto fuera de la máscara: la forma de una
/// selección no rectangular viaja con los píxeles.
#[derive(Clone, Debug, PartialEq)]
pub struct Block<P> {
    pub width: u32,
    pub height: u32,
    /// Fila por fila.
    pub pixels: Vec<Option<P>>,
}

impl<P: Copy> Block<P> {
    /// Bloque vacío (todo fuera de la máscara).
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![None; (width * height) as usize],
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32)
            .then(|| (y as u32 * self.width + x as u32) as usize)
    }

    /// Fuera del bloque es `None`.
    pub fn get(&self, x: i32, y: i32) -> Option<P> {
        self.index(x, y).and_then(|i| self.pixels[i])
    }

    /// Fuera del bloque no hace nada.
    pub fn set(&mut self, x: i32, y: i32, value: Option<P>) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = value;
        }
    }

    /// Puntos con valor, en coordenadas del bloque.
    pub fn points(&self) -> impl Iterator<Item = (i32, i32, P)> + '_ {
        let w = self.width.max(1) as usize;
        self.pixels
            .iter()
            .enumerate()
            .filter_map(move |(i, v)| v.map(|v| ((i % w) as i32, (i / w) as i32, v)))
    }
}

/// Escala por vecino más cercano con muestreo inverso (el centro de cada píxel de destino).
/// Un tamaño negativo voltea ese eje; 0 cuenta como 1.
pub fn scale_nearest<P: Copy>(src: &Block<P>, width: i32, height: i32) -> Block<P> {
    let (w, h) = (width.unsigned_abs().max(1), height.unsigned_abs().max(1));
    let mut out = Block::new(w, h);
    if src.width == 0 || src.height == 0 {
        return out;
    }
    let sample = |d: u32, size: u32, src_size: u32| {
        ((d as u64 * 2 + 1) * src_size as u64 / (size as u64 * 2)) as i32
    };
    for y in 0..h {
        let sy = sample(y, h, src.height);
        let sy = if height < 0 {
            src.height as i32 - 1 - sy
        } else {
            sy
        };
        for x in 0..w {
            let sx = sample(x, w, src.width);
            let sx = if width < 0 {
                src.width as i32 - 1 - sx
            } else {
                sx
            };
            out.set(x as i32, y as i32, src.get(sx, sy));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(width: u32, height: u32, values: &[Option<u8>]) -> Block<u8> {
        Block {
            width,
            height,
            pixels: values.to_vec(),
        }
    }

    /// 1 2 .
    /// 4 5 6
    fn sample() -> Block<u8> {
        block(3, 2, &[Some(1), Some(2), None, Some(4), Some(5), Some(6)])
    }

    #[test]
    fn block_get_set_and_points() {
        let mut b = Block::<u8>::new(2, 2);
        b.set(1, 0, Some(7));
        b.set(5, 5, Some(9)); // fuera: no hace nada
        assert_eq!(b.get(1, 0), Some(7));
        assert_eq!(b.get(-1, 0), None);
        assert_eq!(b.get(0, 0), None);
        assert_eq!(b.points().collect::<Vec<_>>(), vec![(1, 0, 7)]);
    }

    #[test]
    fn scaling_by_two_repeats_each_pixel() {
        let src = block(2, 1, &[Some(1), Some(2)]);
        let out = scale_nearest(&src, 4, 2);
        assert_eq!((out.width, out.height), (4, 2));
        assert_eq!(out.pixels, [1, 1, 2, 2, 1, 1, 2, 2].map(Some).to_vec());
    }

    #[test]
    fn negative_sizes_flip() {
        let x = scale_nearest(&sample(), -3, 2);
        assert_eq!(
            x.pixels,
            vec![None, Some(2), Some(1), Some(6), Some(5), Some(4)]
        );
        let y = scale_nearest(&sample(), 3, -2);
        assert_eq!(
            y.pixels,
            vec![Some(4), Some(5), Some(6), Some(1), Some(2), None]
        );
    }

    #[test]
    fn same_size_is_identity_and_zero_becomes_one() {
        assert_eq!(scale_nearest(&sample(), 3, 2), sample());
        let tiny = scale_nearest(&sample(), 0, 0);
        assert_eq!((tiny.width, tiny.height), (1, 1));
        assert_eq!(tiny.get(0, 0), Some(5), "the center of the original");
    }

    #[test]
    fn halving_takes_every_other_pixel() {
        let src = block(4, 1, &[Some(1), Some(2), Some(3), Some(4)]);
        assert_eq!(scale_nearest(&src, 2, 1).pixels, vec![Some(2), Some(4)]);
    }
}
