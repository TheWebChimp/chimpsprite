//! Los últimos resultados de RotSprite de los objetos girados. RotSprite (Scale2x ×3, girar y
//! volver) es caro y la reproducción lo pide en cada frame para los mismos objetos: un objeto
//! girado quieto, o un tween que se repite en loop, se gira una sola vez. Tiene un tope de
//! pixeles (entradas y salidas) y, al pasarlo, se olvida lo usado hace más tiempo.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

use cs_core::Rgba;
use cs_raster::{Block, rotsprite};

/// Pixeles (de bloques de entrada y de salida) que guarda el caché de la app: unos 20 MB.
const BUDGET: usize = 4 << 20;

type Rotated = (Block<Rgba>, (i32, i32));

struct Entry {
    hash: u64,
    radians: u32,
    input: Block<Rgba>,
    output: Rotated,
    /// El `clock` de su último uso.
    used: u64,
}

impl Entry {
    fn size(&self) -> usize {
        self.input.pixels.len() + self.output.0.pixels.len()
    }
}

struct RotCache {
    entries: Vec<Entry>,
    budget: usize,
    size: usize,
    clock: u64,
    #[cfg(test)]
    misses: usize,
}

fn key(block: &Block<Rgba>, radians: u32) -> u64 {
    let mut h = DefaultHasher::new();
    (block.width, block.height, radians).hash(&mut h);
    block.pixels.hash(&mut h);
    h.finish()
}

impl RotCache {
    fn new(budget: usize) -> Self {
        Self {
            entries: Vec::new(),
            budget,
            size: 0,
            clock: 0,
            #[cfg(test)]
            misses: 0,
        }
    }

    /// Lo guardado para `block` girado `radians`.
    fn get(&mut self, hash: u64, block: &Block<Rgba>, radians: u32) -> Option<Rotated> {
        self.clock += 1;
        let now = self.clock;
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.hash == hash && e.radians == radians && e.input == *block)?;
        entry.used = now;
        Some(entry.output.clone())
    }

    /// Guarda un resultado; uno que ocuparía más de un cuarto del tope no se guarda.
    fn put(&mut self, hash: u64, input: Block<Rgba>, radians: u32, output: Rotated) {
        let entry = Entry {
            hash,
            radians,
            input,
            output,
            used: self.clock,
        };
        let size = entry.size();
        if size > self.budget / 4 {
            return;
        }
        while self.size + size > self.budget {
            let Some(oldest) = (0..self.entries.len()).min_by_key(|&i| self.entries[i].used) else {
                break;
            };
            self.size -= self.entries.swap_remove(oldest).size();
        }
        self.size += size;
        self.entries.push(entry);
    }

    /// Como `rotate`, con este caché (y contando los giros hechos), para los tests.
    #[cfg(test)]
    fn rotate(&mut self, block: Block<Rgba>, radians: f32) -> Rotated {
        let bits = radians.to_bits();
        let hash = key(&block, bits);
        if let Some(hit) = self.get(hash, &block, bits) {
            return hit;
        }
        self.misses += 1;
        let out = rotsprite(&block, radians);
        self.put(hash, block, bits, out.clone());
        out
    }
}

/// `rotsprite(&block, radians)` con el caché de la app. El giro se hace sin tener el caché
/// tomado (otro hilo puede leerlo mientras).
pub(crate) fn rotate(block: Block<Rgba>, radians: f32) -> Rotated {
    static CACHE: OnceLock<Mutex<RotCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(RotCache::new(BUDGET)));
    let bits = radians.to_bits();
    let hash = key(&block, bits);
    let hit = cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(hash, &block, bits);
    if let Some(hit) = hit {
        return hit;
    }
    let out = rotsprite(&block, radians);
    cache
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .put(hash, block, bits, out.clone());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(seed: u8) -> Block<Rgba> {
        let mut b = Block::new(6, 4);
        for (i, (x, y)) in [(0, 0), (1, 1), (2, 1), (5, 3)].into_iter().enumerate() {
            b.set(x, y, Some(Rgba::rgb(seed, i as u8 * 40, 7)));
        }
        b
    }

    #[test]
    fn a_repeated_rotation_comes_from_the_cache_and_matches_rotsprite() {
        let mut cache = RotCache::new(10_000);
        let angle = 30f32.to_radians();
        let first = cache.rotate(block(1), angle);
        assert_eq!(first, rotsprite(&block(1), angle));
        assert_eq!(cache.rotate(block(1), angle), first);
        assert_eq!(cache.misses, 1, "the second one is a hit");
        // Otro ángulo u otros pixeles se giran de nuevo.
        cache.rotate(block(1), 31f32.to_radians());
        cache.rotate(block(2), angle);
        assert_eq!(cache.misses, 3);
    }

    #[test]
    fn past_its_budget_it_forgets_the_least_recently_used() {
        let angle = 30f32.to_radians();
        let one = {
            let mut probe = RotCache::new(usize::MAX);
            probe.rotate(block(1), angle);
            probe.size
        };
        // Caben cuatro.
        let mut cache = RotCache::new(one * 4 + one / 2);
        for seed in 1..=4 {
            cache.rotate(block(seed), angle);
        }
        cache.rotate(block(1), angle);
        assert_eq!(cache.misses, 4);
        // El 5 saca al 2 (el 1 se usó después).
        cache.rotate(block(5), angle);
        assert!(cache.size <= cache.budget);
        cache.rotate(block(1), angle);
        assert_eq!(cache.misses, 5, "1 is still there");
        cache.rotate(block(2), angle);
        assert_eq!(cache.misses, 6, "2 was forgotten");
        // Uno demasiado grande para el tope no se guarda.
        let mut tiny = RotCache::new(one);
        tiny.rotate(block(1), angle);
        assert!(tiny.entries.is_empty());
    }
}
