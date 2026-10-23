//! Escalar y rotar bloques de píxeles (la selección flotante). Funciones puras: no saben del modo
//! de color ni de la UI.

use std::f32::consts::{FRAC_PI_2, TAU};

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

/// Área máxima (en píxeles del bloque original) para RotSprite: trabaja a ×8, con 64 veces más
/// píxeles. Los bloques más grandes se rotan con Fast.
pub const ROTSPRITE_MAX_AREA: u32 = 256 * 256;

/// Cómo se rota la selección (opción de las herramientas de selección y Move).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RotationAlgorithm {
    /// Vecino más cercano: rápido, con bordes dentados.
    Fast,
    /// Scale2x ×8, rotación y reducción: líneas más limpias en pixel art.
    #[default]
    RotSprite,
}

impl RotationAlgorithm {
    pub const ALL: [RotationAlgorithm; 2] = [RotationAlgorithm::Fast, RotationAlgorithm::RotSprite];

    pub fn label(self) -> &'static str {
        match self {
            RotationAlgorithm::Fast => "Fast",
            RotationAlgorithm::RotSprite => "RotSprite",
        }
    }

    pub fn rotate<P: Copy + PartialEq>(
        self,
        src: &Block<P>,
        radians: f32,
    ) -> (Block<P>, (i32, i32)) {
        match self {
            RotationAlgorithm::Fast => rotate_fast(src, radians),
            RotationAlgorithm::RotSprite => rotsprite(src, radians),
        }
    }
}

/// Cuartos de vuelta (0–3) si el ángulo es un múltiplo de 90°.
fn quarter_turns(radians: f32) -> Option<u32> {
    let turns = radians.rem_euclid(TAU) / FRAC_PI_2;
    let q = turns.round();
    ((turns - q).abs() < 1e-4).then_some(q as u32 % 4)
}

/// Tamaño del rectángulo que contiene al bloque rotado.
pub fn rotated_size(width: u32, height: u32, radians: f32) -> (u32, u32) {
    match quarter_turns(radians) {
        Some(q) if q.is_multiple_of(2) => (width, height),
        Some(_) => (height, width),
        None => {
            let (s, c) = radians.sin_cos();
            let (w, h) = (width as f32, height as f32);
            let side = |v: f32| (v - 1e-3).ceil().max(1.0) as u32;
            (
                side(w * c.abs() + h * s.abs()),
                side(w * s.abs() + h * c.abs()),
            )
        }
    }
}

/// Corrimiento de la esquina para que el centro del bloque rotado quede en su lugar.
fn centered_offset(width: u32, height: u32, rotated: (u32, u32)) -> (i32, i32) {
    (
        (width as i32 - rotated.0 as i32).div_euclid(2),
        (height as i32 - rotated.1 as i32).div_euclid(2),
    )
}

/// Giro exacto de `turns` cuartos de vuelta en sentido horario (una permutación).
fn quarter<P: Copy>(src: &Block<P>, turns: u32) -> (Block<P>, (i32, i32)) {
    let (w, h) = (src.width as i32, src.height as i32);
    let (rw, rh) = if turns.is_multiple_of(2) {
        (w, h)
    } else {
        (h, w)
    };
    let mut out = Block::new(rw as u32, rh as u32);
    for y in 0..rh {
        for x in 0..rw {
            let (sx, sy) = match turns {
                1 => (y, h - 1 - x),
                2 => (w - 1 - x, h - 1 - y),
                3 => (w - 1 - y, x),
                _ => (x, y),
            };
            out.set(x, y, src.get(sx, sy));
        }
    }
    let offset = centered_offset(src.width, src.height, (rw as u32, rh as u32));
    (out, offset)
}

/// Rotación por muestreo inverso: `sample` recibe el centro de cada píxel de destino en
/// coordenadas del bloque original (con decimales).
fn rotate_with<P: Copy>(
    width: u32,
    height: u32,
    radians: f32,
    sample: impl Fn(f32, f32) -> Option<P>,
) -> (Block<P>, (i32, i32)) {
    let size = rotated_size(width, height, radians);
    let (dx, dy) = centered_offset(width, height, size);
    let (cx, cy) = (width as f32 / 2.0, height as f32 / 2.0);
    let (s, c) = radians.sin_cos();
    let mut out = Block::new(size.0, size.1);
    for y in 0..size.1 as i32 {
        for x in 0..size.0 as i32 {
            let (px, py) = ((dx + x) as f32 + 0.5 - cx, (dy + y) as f32 + 0.5 - cy);
            // Rotación inversa (positivo = horario con y hacia abajo).
            let (sx, sy) = (c * px + s * py + cx, -s * px + c * py + cy);
            out.set(x, y, sample(sx, sy));
        }
    }
    (out, (dx, dy))
}

/// Rotación Fast: vecino más cercano. Los múltiplos de 90° son exactos.
pub fn rotate_fast<P: Copy>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32)) {
    if let Some(q) = quarter_turns(radians) {
        return quarter(src, q);
    }
    rotate_with(src.width, src.height, radians, |x, y| {
        src.get(x.floor() as i32, y.floor() as i32)
    })
}

/// Scale2x (EPX) comparando valores exactos; fuera del bloque es `None` (transparente).
fn scale2x<P: Copy + PartialEq>(src: &Block<P>) -> Block<P> {
    let mut out = Block::new(src.width * 2, src.height * 2);
    for y in 0..src.height as i32 {
        for x in 0..src.width as i32 {
            let p = src.get(x, y);
            let (a, b, c, d) = (
                src.get(x, y - 1),
                src.get(x + 1, y),
                src.get(x - 1, y),
                src.get(x, y + 1),
            );
            let e0 = if c == a && c != d && a != b { a } else { p };
            let e1 = if a == b && a != c && b != d { b } else { p };
            let e2 = if d == c && d != b && c != a { c } else { p };
            let e3 = if b == d && b != a && d != c { d } else { p };
            out.set(2 * x, 2 * y, e0);
            out.set(2 * x + 1, 2 * y, e1);
            out.set(2 * x, 2 * y + 1, e2);
            out.set(2 * x + 1, 2 * y + 1, e3);
        }
    }
    out
}

/// RotSprite: Scale2x tres veces (×8), rotación Fast a ×8 y reducción tomando el píxel central de
/// cada bloque de 8×8 (muestrear el ×8 en el centro de cada píxel de destino es lo mismo).
pub fn rotsprite<P: Copy + PartialEq>(src: &Block<P>, radians: f32) -> (Block<P>, (i32, i32)) {
    if let Some(q) = quarter_turns(radians) {
        return quarter(src, q);
    }
    if src.width * src.height > ROTSPRITE_MAX_AREA {
        return rotate_fast(src, radians);
    }
    let big = scale2x(&scale2x(&scale2x(src)));
    rotate_with(src.width, src.height, radians, |x, y| {
        big.get((x * 8.0).floor() as i32, (y * 8.0).floor() as i32)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

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

    #[test]
    fn rotated_sizes() {
        assert_eq!(rotated_size(4, 2, 0.0), (4, 2));
        assert_eq!(rotated_size(4, 2, FRAC_PI_2), (2, 4));
        assert_eq!(rotated_size(4, 2, PI), (4, 2));
        assert_eq!(rotated_size(4, 2, -FRAC_PI_2), (2, 4));
        assert_eq!(rotated_size(4, 2, FRAC_PI_4), (5, 5));
    }

    #[test]
    fn zero_degrees_changes_nothing() {
        for algorithm in RotationAlgorithm::ALL {
            assert_eq!(algorithm.rotate(&sample(), 0.0), (sample(), (0, 0)));
            assert_eq!(algorithm.rotate(&sample(), TAU), (sample(), (0, 0)));
        }
    }

    #[test]
    fn quarter_turns_are_exact_and_carry_the_mask() {
        for algorithm in RotationAlgorithm::ALL {
            // Horario: la primera fila (1 2 .) queda como última columna.
            let (cw, offset) = algorithm.rotate(&sample(), FRAC_PI_2);
            assert_eq!((cw.width, cw.height), (2, 3));
            assert_eq!(
                cw.pixels,
                vec![Some(4), Some(1), Some(5), Some(2), Some(6), None]
            );
            assert_eq!(offset, (0, -1), "the center stays in place");
            let (half, offset) = algorithm.rotate(&sample(), PI);
            assert_eq!(
                half.pixels,
                vec![Some(6), Some(5), Some(4), None, Some(2), Some(1)]
            );
            assert_eq!(offset, (0, 0));
            let (ccw, _) = algorithm.rotate(&sample(), -FRAC_PI_2);
            assert_eq!(
                ccw.pixels,
                vec![None, Some(6), Some(2), Some(5), Some(1), Some(4)]
            );
        }
    }

    #[test]
    fn fast_rotation_is_clockwise() {
        let line = block(5, 1, &[Some(1), Some(2), Some(3), Some(4), Some(5)]);
        let (out, offset) = rotate_fast(&line, 30f32.to_radians());
        assert_eq!((out.width, out.height), (5, 4));
        assert_eq!(offset, (0, -2));
        assert_eq!(out.get(4, 3), Some(5), "the right end goes down");
        assert_eq!(out.get(0, 1), Some(1), "the left end goes up");
    }

    #[test]
    fn scale2x_rounds_diagonals() {
        // A .
        // . A
        let src = block(2, 2, &[Some(1), None, None, Some(1)]);
        let big = scale2x(&src);
        assert_eq!(
            big.get(2, 1),
            Some(1),
            "the gap between the two pixels is filled"
        );
        assert_eq!(big.get(3, 0), None);
    }

    #[test]
    fn rotsprite_line_at_45_degrees_has_no_loose_pixels() {
        let line = block(8, 1, &[Some(1); 8]);
        let (out, _) = rotsprite(&line, FRAC_PI_4);
        let filled: Vec<(i32, i32)> = out.points().map(|(x, y, _)| (x, y)).collect();
        assert!(filled.len() >= 6, "{filled:?}");
        for &(x, y) in &filled {
            let touches = filled
                .iter()
                .any(|&(a, b)| (a, b) != (x, y) && (a - x).abs() <= 1 && (b - y).abs() <= 1);
            assert!(touches, "loose pixel at ({x}, {y})");
            // Un punto que solo toca en diagonal también es una mota suelta.
            let joined = filled
                .iter()
                .any(|&(a, b)| (a - x).abs() + (b - y).abs() == 1);
            assert!(joined, "pixel only touching diagonally at ({x}, {y})");
        }
        // Fast deja puntas sueltas en los extremos; RotSprite los redondea y queda con menos.
        let fast = rotate_fast(&line, FRAC_PI_4).0.points().count();
        assert!(filled.len() < fast, "{} vs fast {fast}", filled.len());
        for y in 0..out.height as i32 {
            let row = (0..out.width as i32)
                .filter(|&x| out.get(x, y).is_some())
                .count();
            assert!(row <= 2, "row {y} has {row} pixels");
        }
    }

    #[test]
    fn big_blocks_use_fast() {
        let mut b = Block::<u8>::new(257, 256);
        b.set(0, 0, Some(1));
        b.set(200, 100, Some(2));
        assert_eq!(rotsprite(&b, 0.5), rotate_fast(&b, 0.5));
    }
}
