use serde::{Deserialize, Serialize};

/// Máscara de bits del tamaño del lienzo (selección; base de las máscaras de capa).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mask {
    width: u32,
    height: u32,
    bits: Vec<bool>,
}

impl Mask {
    /// Máscara vacía.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bits: vec![false; (width * height) as usize],
        }
    }

    /// Todo el lienzo seleccionado.
    pub fn all(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            bits: vec![true; (width * height) as usize],
        }
    }

    /// Rectángulo inclusivo entre dos esquinas cualesquiera, recortado al lienzo.
    pub fn from_rect(width: u32, height: u32, a: (i32, i32), b: (i32, i32)) -> Self {
        let mut mask = Self::new(width, height);
        let (x0, x1) = (a.0.min(b.0).max(0), a.0.max(b.0).min(width as i32 - 1));
        let (y0, y1) = (a.1.min(b.1).max(0), a.1.max(b.1).min(height as i32 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                mask.set(x, y, true);
            }
        }
        mask
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32)
            .then(|| (y as u32 * self.width + x as u32) as usize)
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.index(x, y).is_some_and(|i| self.bits[i])
    }

    /// Fuera del lienzo no hace nada.
    pub fn set(&mut self, x: i32, y: i32, value: bool) {
        if let Some(i) = self.index(x, y) {
            self.bits[i] = value;
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.bits.iter().any(|&b| b)
    }

    pub fn points(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let w = self.width as usize;
        self.bits
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(move |(i, _)| ((i % w) as i32, (i / w) as i32))
    }

    /// Esquinas mínima y máxima (inclusivas) de lo seleccionado.
    pub fn bounds(&self) -> Option<((i32, i32), (i32, i32))> {
        self.points().fold(None, |acc, (x, y)| match acc {
            None => Some(((x, y), (x, y))),
            Some(((x0, y0), (x1, y1))) => Some(((x0.min(x), y0.min(y)), (x1.max(x), y1.max(y)))),
        })
    }

    /// Máscara con los puntos dados (los de fuera del lienzo se ignoran).
    pub fn from_points(
        width: u32,
        height: u32,
        points: impl IntoIterator<Item = (i32, i32)>,
    ) -> Self {
        let mut mask = Self::new(width, height);
        for (x, y) in points {
            mask.set(x, y, true);
        }
        mask
    }

    fn zip(&self, other: &Mask, f: impl Fn(bool, bool) -> bool) -> Self {
        debug_assert_eq!((self.width, self.height), (other.width, other.height));
        Self {
            width: self.width,
            height: self.height,
            bits: self
                .bits
                .iter()
                .zip(&other.bits)
                .map(|(&a, &b)| f(a, b))
                .collect(),
        }
    }

    /// Lo que está en cualquiera de las dos.
    pub fn union(&self, other: &Mask) -> Self {
        self.zip(other, |a, b| a || b)
    }

    /// Lo de `self` que no está en `other`.
    pub fn subtract(&self, other: &Mask) -> Self {
        self.zip(other, |a, b| a && !b)
    }

    /// Lo que está en ambas.
    pub fn intersect(&self, other: &Mask) -> Self {
        self.zip(other, |a, b| a && b)
    }

    /// Todo lo no seleccionado.
    pub fn inverted(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            bits: self.bits.iter().map(|b| !b).collect(),
        }
    }

    /// Copia desplazada; lo que sale del lienzo se pierde.
    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        let mut out = Self::new(self.width, self.height);
        for (x, y) in self.points() {
            out.set(x + dx, y + dy, true);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_rect_normalizes_and_clips() {
        let m = Mask::from_rect(4, 4, (5, 2), (2, -3));
        assert_eq!(m.bounds(), Some(((2, 0), (3, 2))));
        assert!(m.contains(3, 0) && !m.contains(1, 0) && !m.contains(9, 9));
    }

    #[test]
    fn empty_mask_has_no_bounds() {
        assert!(Mask::new(3, 3).is_empty());
        assert_eq!(Mask::new(3, 3).bounds(), None);
        assert!(!Mask::all(3, 3).is_empty());
    }

    #[test]
    fn boolean_operations() {
        let a = Mask::from_rect(4, 1, (0, 0), (2, 0));
        let b = Mask::from_rect(4, 1, (1, 0), (3, 0));
        let pts = |m: Mask| m.points().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(pts(a.union(&b)), [0, 1, 2, 3]);
        assert_eq!(pts(a.subtract(&b)), [0]);
        assert_eq!(pts(a.intersect(&b)), [1, 2]);
        assert_eq!(pts(a.inverted()), [3]);
        assert_eq!(pts(Mask::from_points(4, 1, [(1, 0), (9, 0)])), [1]);
    }

    #[test]
    fn translated_clips_at_edges() {
        let m = Mask::from_rect(4, 4, (0, 0), (1, 1)).translated(3, 0);
        assert_eq!(m.points().collect::<Vec<_>>(), vec![(3, 0), (3, 1)]);
    }
}
