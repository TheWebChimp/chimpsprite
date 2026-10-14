//! Pseudoaleatorio pequeño y determinista (xorshift64*), para que la vista previa sea estable.

#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniforme en `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Hash de un punto con una semilla (para valores aleatorios sin estado).
pub(crate) fn hash(seed: u64, x: i32, y: i32) -> u64 {
    Rng::new(seed ^ ((x as u32 as u64) << 32 | y as u32 as u64)).next_u64()
}
