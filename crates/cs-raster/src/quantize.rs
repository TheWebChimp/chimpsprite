//! Reducción de colores: median cut y color más cercano.

use std::collections::HashMap;

use cs_core::Rgba;

/// Hasta `n` colores representativos (opacos) de `colors` (con repeticiones).
pub fn median_cut(colors: impl IntoIterator<Item = Rgba>, n: usize) -> Vec<Rgba> {
    let mut counts: HashMap<[u8; 3], u32> = HashMap::new();
    for c in colors {
        *counts.entry([c.r, c.g, c.b]).or_default() += 1;
    }
    let mut all: Vec<([u8; 3], u32)> = counts.into_iter().collect();
    all.sort_unstable();
    if all.is_empty() || n == 0 {
        return Vec::new();
    }
    let mut boxes = vec![all];
    while boxes.len() < n {
        // La caja con el canal más amplio se parte por su mediana (ponderada por cantidad).
        let Some((i, channel, _)) = boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.len() > 1)
            .map(|(i, b)| {
                let (channel, range) = widest(b);
                (i, channel, range)
            })
            .max_by_key(|&(_, _, range)| range)
        else {
            break;
        };
        let mut b = boxes.swap_remove(i);
        b.sort_unstable_by_key(|(c, _)| c[channel]);
        let total: u32 = b.iter().map(|(_, n)| n).sum();
        let mut acc = 0;
        let mut cut = 1;
        for (k, (_, count)) in b.iter().enumerate() {
            acc += count;
            if acc * 2 >= total {
                cut = (k + 1).clamp(1, b.len() - 1);
                break;
            }
        }
        let rest = b.split_off(cut);
        boxes.push(b);
        boxes.push(rest);
    }
    let mut out: Vec<Rgba> = boxes.iter().map(|b| average(b)).collect();
    out.sort_by_key(|c| {
        (
            c.r as u32 * 299 + c.g as u32 * 587 + c.b as u32 * 114,
            c.r,
            c.g,
            c.b,
        )
    });
    out.dedup();
    out
}

fn widest(b: &[([u8; 3], u32)]) -> (usize, u8) {
    (0..3)
        .map(|ch| {
            let (lo, hi) = b.iter().fold((255u8, 0u8), |(lo, hi), (c, _)| {
                (lo.min(c[ch]), hi.max(c[ch]))
            });
            (ch, hi - lo)
        })
        .max_by_key(|&(_, r)| r)
        .expect("three channels")
}

fn average(b: &[([u8; 3], u32)]) -> Rgba {
    let total: u64 = b.iter().map(|(_, n)| *n as u64).sum();
    let ch = |i: usize| {
        let sum: u64 = b.iter().map(|(c, n)| c[i] as u64 * *n as u64).sum();
        ((sum + total / 2) / total) as u8
    };
    Rgba::rgb(ch(0), ch(1), ch(2))
}

/// Índice del color de `palette` más cercano a `c` (distancia RGB al cuadrado).
pub fn nearest(palette: &[Rgba], c: Rgba) -> usize {
    let d = |p: &Rgba| {
        let dr = p.r as i32 - c.r as i32;
        let dg = p.g as i32 - c.g as i32;
        let db = p.b as i32 - c.b as i32;
        dr * dr + dg * dg + db * db
    };
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| d(p))
        .map_or(0, |(i, _)| i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_few_colors_and_splits_clusters() {
        let red = Rgba::rgb(250, 0, 0);
        let blue = Rgba::rgb(0, 0, 250);
        let colors = [red, red, Rgba::rgb(240, 10, 0), blue, Rgba::rgb(0, 5, 245)];
        let two = median_cut(colors, 2);
        assert_eq!(two.len(), 2);
        assert_eq!(nearest(&two, red), nearest(&two, Rgba::rgb(240, 10, 0)));
        assert_ne!(nearest(&two, red), nearest(&two, blue));
        let many = median_cut([red, blue], 16);
        assert_eq!(many.len(), 2, "never more colors than exist");
        assert!(many.contains(&red) && many.contains(&blue));
        assert!(median_cut([], 4).is_empty());
    }
}
