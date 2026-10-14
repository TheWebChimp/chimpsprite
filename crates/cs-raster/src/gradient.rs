//! Degradados: posición `t` de cada píxel y umbrales de dithering ordenado (Bayer).

use crate::Point;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GradientShape {
    #[default]
    Linear,
    Radial,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dither {
    /// Interpolación suave.
    #[default]
    None,
    Bayer2,
    Bayer4,
    Bayer8,
}

impl Dither {
    pub const ALL: [Dither; 4] = [Dither::None, Dither::Bayer2, Dither::Bayer4, Dither::Bayer8];

    pub fn label(self) -> &'static str {
        match self {
            Dither::None => "No dithering",
            Dither::Bayer2 => "Ordered 2×2",
            Dither::Bayer4 => "Ordered 4×4",
            Dither::Bayer8 => "Ordered 8×8",
        }
    }

    fn size(self) -> Option<u32> {
        match self {
            Dither::None => None,
            Dither::Bayer2 => Some(2),
            Dither::Bayer4 => Some(4),
            Dither::Bayer8 => Some(8),
        }
    }

    /// Umbral en `(0, 1)` para el píxel `(x, y)`; `None` sin dithering.
    pub fn threshold(self, x: i32, y: i32) -> Option<f32> {
        let n = self.size()?;
        let v = bayer(
            n,
            x.rem_euclid(n as i32) as u32,
            y.rem_euclid(n as i32) as u32,
        );
        Some((v as f32 + 0.5) / (n * n) as f32)
    }
}

/// Entrada de la matriz de Bayer de lado `n` (potencia de 2).
fn bayer(n: u32, x: u32, y: u32) -> u32 {
    if n == 1 {
        return 0;
    }
    let half = n / 2;
    let quadrant = match (x >= half, y >= half) {
        (false, false) => 0,
        (true, true) => 1,
        (true, false) => 2,
        (false, true) => 3,
    };
    4 * bayer(half, x % half, y % half) + quadrant
}

/// Posición (0 = inicio, 1 = fin) del centro del píxel `p` en el degradado de `a` a `b`.
pub fn gradient_t(shape: GradientShape, a: Point, b: Point, p: Point) -> f32 {
    let (dx, dy) = ((b.0 - a.0) as f32, (b.1 - a.1) as f32);
    let (px, py) = ((p.0 - a.0) as f32, (p.1 - a.1) as f32);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return 0.0;
    }
    let t = match shape {
        GradientShape::Linear => (px * dx + py * dy) / len2,
        GradientShape::Radial => ((px * px + py * py) / len2).sqrt(),
    };
    t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayer_matrices_are_permutations() {
        for d in [Dither::Bayer2, Dither::Bayer4, Dither::Bayer8] {
            let n = d.size().unwrap() as i32;
            let mut seen: Vec<f32> = (0..n)
                .flat_map(|y| (0..n).map(move |x| d.threshold(x, y).unwrap()))
                .collect();
            seen.sort_by(f32::total_cmp);
            seen.dedup();
            assert_eq!(seen.len(), (n * n) as usize);
            assert!(seen[0] > 0.0 && seen[seen.len() - 1] < 1.0);
        }
        assert_eq!(Dither::Bayer2.threshold(0, 0), Some(0.125));
        assert_eq!(Dither::Bayer2.threshold(1, 1), Some(0.375));
        assert_eq!(
            Dither::Bayer2.threshold(2, 2),
            Dither::Bayer2.threshold(0, 0)
        );
        assert_eq!(Dither::None.threshold(0, 0), None);
    }

    #[test]
    fn linear_and_radial_positions() {
        let lin = |p| gradient_t(GradientShape::Linear, (0, 0), (10, 0), p);
        assert_eq!(lin((0, 5)), 0.0);
        assert!((lin((5, 3)) - 0.5).abs() < 1e-6);
        assert_eq!(lin((20, 0)), 1.0);
        assert_eq!(lin((-5, 0)), 0.0);
        let rad = |p| gradient_t(GradientShape::Radial, (5, 5), (5, 9), p);
        assert_eq!(rad((5, 5)), 0.0);
        assert!((rad((7, 5)) - 0.5).abs() < 1e-6);
        assert_eq!(
            gradient_t(GradientShape::Linear, (1, 1), (1, 1), (3, 3)),
            0.0
        );
    }
}
