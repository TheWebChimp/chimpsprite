//! Groom: de una imagen con "pixel art falso" (una ilustración agrandada, borrosa y con píxeles
//! de tamaños irregulares, como la que sale de una IA) a pixel art de verdad: detectar su
//! cuadrícula, tomar un color por celda, quitar el fondo, limpiar el contorno y reducir los
//! colores. Todo trabaja sobre RGBA por filas (`pixels[y * width + x]`).

use std::collections::HashMap;

use cs_core::Rgba;

use crate::{median_cut, nearest};

/// La cuadrícula de una imagen agrandada: cuánto mide cada píxel "de verdad" (puede no ser
/// entero) y dónde empieza la primera celda.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridGuess {
    pub cell: (f32, f32),
    pub offset: (f32, f32),
}

impl GridGuess {
    /// Columnas y filas que entran en una imagen de `size` (la de la orilla cuenta si entra al
    /// menos la mitad).
    pub fn count(&self, size: (u32, u32)) -> (u32, u32) {
        let fit = |total: u32, offset: f32, cell: f32| {
            ((total as f32 - offset) / cell.max(1.0) + 0.5)
                .floor()
                .max(0.0) as u32
        };
        (
            fit(size.0, self.offset.0, self.cell.0),
            fit(size.1, self.offset.1, self.cell.1),
        )
    }
}

/// La distancia entre dos colores (suma de canales, el alfa incluido).
fn distance(a: Rgba, b: Rgba) -> u32 {
    a.r.abs_diff(b.r) as u32
        + a.g.abs_diff(b.g) as u32
        + a.b.abs_diff(b.b) as u32
        + a.a.abs_diff(b.a) as u32
}

/// Cuánto cambia el color al pasar de la columna `x - 1` a la `x` (o de fila, con `rows`),
/// sumado a lo largo de la imagen: los bordes de las celdas son los picos.
fn edge_profile(pixels: &[Rgba], width: u32, height: u32, rows: bool) -> Vec<f32> {
    let (len, across) = if rows {
        (height, width)
    } else {
        (width, height)
    };
    let at = |along: u32, other: u32| {
        if rows {
            pixels[(along * width + other) as usize]
        } else {
            pixels[(other * width + along) as usize]
        }
    };
    let mut profile = vec![0.0; len as usize];
    for i in 1..len {
        let mut sum = 0u64;
        for j in 0..across {
            sum += distance(at(i, j), at(i - 1, j)) as u64;
        }
        profile[i as usize] = sum as f32;
    }
    profile
}

/// El periodo (y desfase) de los bordes de `profile`. Primero, aproximado: el máximo de la
/// autocorrelación del perfil después de su primer cruce por cero (antes solo mide lo borroso de
/// cada borde). Después, fino: el periodo y el desfase cerca de ese que mejor alinean un peine con
/// los bordes. `None` si no hay un patrón.
fn best_period(profile: &[f32]) -> Option<(f32, f32)> {
    let len = profile.len();
    // Muy corta para ver un patrón: sin cuadrícula.
    if len < 12 {
        return None;
    }
    let mean = profile.iter().sum::<f32>() / len as f32;
    if mean <= 0.0 {
        return None;
    }
    let centered: Vec<f32> = profile.iter().map(|v| v - mean).collect();
    let energy: f32 = centered.iter().map(|v| v * v).sum();
    let ac = |lag: usize| -> f32 {
        centered[..len - lag]
            .iter()
            .zip(&centered[lag..])
            .map(|(a, b)| a * b)
            .sum::<f32>()
            / energy
    };
    let max_lag = (len / 3).max(3);
    let lags: Vec<f32> = (0..=max_lag).map(ac).collect();
    let zero = lags.iter().position(|&v| v <= 0.0)?;
    let (mut lag, mut best) = (0, 0.0f32);
    for (l, &v) in lags.iter().enumerate().skip(zero.max(2)) {
        if v > best {
            (lag, best) = (l, v);
        }
    }
    // Sin una repetición clara no hay cuadrícula.
    if lag == 0 || best < 0.15 {
        return None;
    }
    // Si la mitad también se repite casi igual, la celda es la mitad (con un periodo no entero,
    // la mitad cae entre dos desfases: el mejor de los dos).
    while lag / 2 >= 2 {
        let (lo, hi) = (lag / 2, lag.div_ceil(2));
        // Con celdas que alternan (7 y 8 px de un agrandado de 7.5), la repetición se reparte
        // entre los dos desfases vecinos.
        let value = if lo == hi {
            lags[lo]
        } else {
            lags[lo] + lags[hi]
        };
        if value < best * 0.7 {
            break;
        }
        let half = if lags[hi] > lags[lo] { hi } else { lo };
        (lag, best) = (half, lags[half].max(value / 2.0));
    }
    // Fino: el peine que mejor cae sobre los bordes, cerca de `lag`.
    let comb = |period: f32, offset: f32| -> f32 {
        let (mut sum, mut n) = (0.0f32, 0usize);
        let mut x = offset;
        while (x.round() as usize) < len {
            let i = x.round() as usize;
            // Lo más alto en ±1 px: los bordes borrosos caen entre dos columnas.
            let v = (i.saturating_sub(1)..(i + 2).min(len))
                .map(|j| profile[j])
                .fold(0.0f32, f32::max);
            sum += v;
            n += 1;
            x += period;
        }
        if n == 0 { 0.0 } else { sum / n as f32 }
    };
    // El mejor peine con periodo cerca de `around` (±1.5 px): periodo, desfase y su puntaje.
    let search = |around: f32| -> (f32, f32, f32) {
        let mut fine = (around, 0.0f32, 0.0f32);
        let mut period = (around - 1.5).max(2.0);
        while period <= around + 1.5 {
            let mut offset = 0.0f32;
            while offset < period {
                let value = comb(period, offset);
                if value > fine.2 {
                    fine = (period, offset, value);
                }
                offset += 0.25;
            }
            period += 0.05;
        }
        fine
    };
    let fine = search(lag as f32);
    // Afinado: la recta que mejor pasa por los bordes reales (el pico del perfil cerca de cada
    // diente del peine). Así un error chiquito del periodo no se acumula a lo largo de la imagen.
    let (period, offset) = (fine.0, fine.1);
    let reach = (period / 4.0).max(1.0);
    let mut points: Vec<(f32, f32)> = Vec::new();
    let mut k = 0.0f32;
    while offset + k * period < len as f32 {
        let guess = offset + k * period;
        let from = (guess - reach).ceil().max(1.0) as usize;
        let to = ((guess + reach).floor() as usize).min(len - 1);
        if from <= to
            && let Some(i) = (from..=to).max_by(|&a, &b| profile[a].total_cmp(&profile[b]))
            && profile[i] > mean
        {
            points.push((k, i as f32));
        }
        k += 1.0;
    }
    let fine = if points.len() >= 4 {
        let n = points.len() as f32;
        let (mk, mx) = points
            .iter()
            .fold((0.0, 0.0), |(a, b), &(k, x)| (a + k / n, b + x / n));
        let (num, den) = points.iter().fold((0.0, 0.0), |(a, b), &(k, x)| {
            (a + (k - mk) * (x - mx), b + (k - mk) * (k - mk))
        });
        let fitted = if den > 0.0 { num / den } else { period };
        if (fitted - period).abs() <= period * 0.1 {
            (fitted, mx - fitted * mk, fine.2)
        } else {
            fine
        }
    } else {
        fine
    };
    Some((fine.0, fine.1))
}

/// La cuadrícula de `pixels` (de `width` × `height`). Sin bordes claros, celdas de 1 px.
pub fn detect_grid(pixels: &[Rgba], width: u32, height: u32) -> GridGuess {
    let columns = best_period(&edge_profile(pixels, width, height, false));
    let rows = best_period(&edge_profile(pixels, width, height, true));
    // Si un eje no se ve (p. ej. una franja), el mismo tamaño que el otro.
    let (cx, ox) = columns.or(rows).unwrap_or((1.0, 0.0));
    let (cy, oy) = rows.or(columns).unwrap_or((1.0, 0.0));
    // La celda de la orilla cuenta si entra al menos la mitad: el desfase va de -½ a ½ celda.
    let centered = |o: f32, c: f32| {
        let o = o.rem_euclid(c);
        if o > c / 2.0 { o - c } else { o }
    };
    GridGuess {
        cell: (cx, cy),
        offset: (centered(ox, cx), centered(oy, cy)),
    }
}

/// Un píxel por celda de `grid`: el color dominante del centro de la celda (sin sus orillas,
/// que suelen venir borrosas). Devuelve los píxeles y el tamaño nuevo.
pub fn sample_grid(
    pixels: &[Rgba],
    width: u32,
    height: u32,
    grid: GridGuess,
) -> (Vec<Rgba>, u32, u32) {
    let (cols, rows) = grid.count((width, height));
    let mut out = Vec::with_capacity((cols * rows) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let x0 = grid.offset.0 + col as f32 * grid.cell.0;
            let y0 = grid.offset.1 + row as f32 * grid.cell.1;
            // El centro: sin un 20 % de cada lado (al menos un píxel).
            let inset = |cell: f32| (cell * 0.2).floor();
            let (ix, iy) = (inset(grid.cell.0), inset(grid.cell.1));
            // La celda de la orilla puede empezar antes de la imagen: lo que cae dentro.
            let span = |from: f32, to: f32, max: u32| {
                (from.floor().max(0.0) as u32).min(max)..(to.ceil().max(0.0) as u32).min(max)
            };
            let xs = span(x0 + ix, x0 + grid.cell.0 - ix, width);
            let ys = span(y0 + iy, y0 + grid.cell.1 - iy, height);
            let colors = ys
                .flat_map(|y| xs.clone().map(move |x| (x, y)))
                .map(|(x, y)| pixels[(y * width + x) as usize]);
            out.push(dominant(colors));
        }
    }
    (out, cols, rows)
}

/// El color que más se repite (agrupando los parecidos), promediado dentro de su grupo.
fn dominant(colors: impl Iterator<Item = Rgba>) -> Rgba {
    let mut groups: HashMap<[u8; 4], (u32, [u32; 4])> = HashMap::new();
    for c in colors {
        let key = [c.r >> 4, c.g >> 4, c.b >> 4, c.a >> 4];
        let g = groups.entry(key).or_default();
        g.0 += 1;
        for (sum, v) in g.1.iter_mut().zip([c.r, c.g, c.b, c.a]) {
            *sum += v as u32;
        }
    }
    let Some((n, sums)) = groups.into_values().max_by_key(|(n, _)| *n) else {
        return Rgba::TRANSPARENT;
    };
    let avg = |i: usize| (sums[i] / n) as u8;
    Rgba::new(avg(0), avg(1), avg(2), avg(3))
}

/// El fondo transparente: lo que toca el borde de la imagen y se parece (hasta `tolerance` por
/// canal) al color más común del borde, con lo que se le une.
pub fn remove_background(pixels: &[Rgba], width: u32, height: u32, tolerance: u8) -> Vec<Rgba> {
    let mut out = pixels.to_vec();
    if width == 0 || height == 0 {
        return out;
    }
    let index = |x: u32, y: u32| (y * width + x) as usize;
    let border: Vec<(u32, u32)> = (0..width)
        .flat_map(|x| [(x, 0), (x, height - 1)])
        .chain((0..height).flat_map(|y| [(0, y), (width - 1, y)]))
        .collect();
    let background = dominant(border.iter().map(|&(x, y)| pixels[index(x, y)]));
    if background.a == 0 {
        return out;
    }
    let near = |c: Rgba| {
        c.r.abs_diff(background.r) <= tolerance
            && c.g.abs_diff(background.g) <= tolerance
            && c.b.abs_diff(background.b) <= tolerance
    };
    let mut seen = vec![false; pixels.len()];
    let mut stack: Vec<(u32, u32)> = border
        .into_iter()
        .filter(|&(x, y)| near(pixels[index(x, y)]))
        .collect();
    while let Some((x, y)) = stack.pop() {
        let i = index(x, y);
        if seen[i] || !near(pixels[i]) {
            continue;
        }
        seen[i] = true;
        out[i] = Rgba::TRANSPARENT;
        if x > 0 {
            stack.push((x - 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if x + 1 < width {
            stack.push((x + 1, y));
        }
        if y + 1 < height {
            stack.push((x, y + 1));
        }
    }
    out
}

/// El contorno limpio: cada píxel opaco o transparente (sin medias transparencias), sin los
/// píxeles sueltos (opacos sin ningún vecino opaco) y, con `outline`, rodeado por un contorno
/// de 1 px de ese color.
pub fn clean_outline(pixels: &[Rgba], width: u32, height: u32, outline: Option<Rgba>) -> Vec<Rgba> {
    let mut out: Vec<Rgba> = pixels
        .iter()
        .map(|&c| {
            if c.a < 128 {
                Rgba::TRANSPARENT
            } else {
                Rgba { a: 255, ..c }
            }
        })
        .collect();
    let opaque = |px: &[Rgba], x: i64, y: i64| {
        x >= 0
            && y >= 0
            && x < width as i64
            && y < height as i64
            && px[(y as u32 * width + x as u32) as usize].a > 0
    };
    let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let snapshot = out.clone();
    for y in 0..height as i64 {
        for x in 0..width as i64 {
            let alone = opaque(&snapshot, x, y)
                && !sides
                    .iter()
                    .any(|(dx, dy)| opaque(&snapshot, x + dx, y + dy));
            if alone {
                out[(y as u32 * width + x as u32) as usize] = Rgba::TRANSPARENT;
            }
        }
    }
    if let Some(color) = outline {
        let snapshot = out.clone();
        for y in 0..height as i64 {
            for x in 0..width as i64 {
                let touches = sides
                    .iter()
                    .any(|(dx, dy)| opaque(&snapshot, x + dx, y + dy));
                if !opaque(&snapshot, x, y) && touches {
                    out[(y as u32 * width + x as u32) as usize] = color;
                }
            }
        }
    }
    out
}

/// A lo más `n` colores (median cut): cada píxel opaco, el más cercano de esos. Devuelve los
/// píxeles y la paleta.
pub fn reduce_colors(pixels: &[Rgba], n: usize) -> (Vec<Rgba>, Vec<Rgba>) {
    let palette = median_cut(pixels.iter().copied().filter(|c| c.a > 0), n);
    if palette.is_empty() {
        return (pixels.to_vec(), palette);
    }
    let out = pixels
        .iter()
        .map(|&c| {
            if c.a == 0 {
                c
            } else {
                Rgba {
                    a: c.a,
                    ..palette[nearest(&palette, c)]
                }
            }
        })
        .collect();
    (out, palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Rgba = Rgba::TRANSPARENT;

    /// Un sprite de 6×5 con fondo blanco, un cuerpo rojo y un ojo negro.
    fn sprite() -> (Vec<Rgba>, u32, u32) {
        let w = Rgba::rgb(250, 250, 250);
        let r = Rgba::rgb(200, 40, 40);
        let k = Rgba::rgb(10, 10, 10);
        let rows = [
            [w, w, w, w, w, w],
            [w, r, r, r, r, w],
            [w, r, k, r, r, w],
            [w, r, r, r, r, w],
            [w, w, w, w, w, w],
        ];
        (rows.iter().flatten().copied().collect(), 6, 5)
    }

    /// `pixels` agrandado `scale` veces (no entero: celdas de 7 y 8 px), con las orillas de cada
    /// celda mezcladas con la de al lado y un poco de ruido, como una imagen de IA.
    fn upscale(pixels: &[Rgba], w: u32, h: u32, scale: f32) -> (Vec<Rgba>, u32, u32) {
        let (bw, bh) = ((w as f32 * scale) as u32, (h as f32 * scale) as u32);
        let mut out = Vec::with_capacity((bw * bh) as usize);
        let mut seed = 12345u32;
        for y in 0..bh {
            for x in 0..bw {
                let (sx, sy) = (x as f32 / scale, y as f32 / scale);
                let (cx, cy) = (sx as u32, sy as u32);
                let mut c = pixels[(cy.min(h - 1) * w + cx.min(w - 1)) as usize];
                // En la orilla (el último 10 % de la celda), mezclado con la siguiente.
                if sx.fract() > 0.9 && cx + 1 < w {
                    let n = pixels[(cy * w + cx + 1) as usize];
                    c = Rgba::rgb(
                        ((c.r as u16 + n.r as u16) / 2) as u8,
                        ((c.g as u16 + n.g as u16) / 2) as u8,
                        ((c.b as u16 + n.b as u16) / 2) as u8,
                    );
                }
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let noise = ((seed >> 16) % 7) as i16 - 3;
                let f = |v: u8| (v as i16 + noise).clamp(0, 255) as u8;
                out.push(Rgba::rgb(f(c.r), f(c.g), f(c.b)));
            }
        }
        (out, bw, bh)
    }

    /// 16×12 de colores al azar de una paleta de 6 (como un sprite con detalle).
    fn detailed() -> (Vec<Rgba>, u32, u32) {
        let (w, h) = (16u32, 12u32);
        let mut seed = 7u32;
        let pixels = (0..w * h)
            .map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let v = ((seed >> 16) % 6) as u8;
                Rgba::rgb(v * 40, 255 - v * 30, ((v as u32 * 70) % 255) as u8)
            })
            .collect();
        (pixels, w, h)
    }

    #[test]
    fn the_grid_of_a_blurry_noisy_upscale_is_found_and_sampled_back() {
        let (original, w, h) = detailed();
        let (big, bw, bh) = upscale(&original, w, h, 7.5);
        let grid = detect_grid(&big, bw, bh);
        assert!((grid.cell.0 - 7.5).abs() <= 0.26, "{grid:?}");
        assert!((grid.cell.1 - 7.5).abs() <= 0.26, "{grid:?}");
        let (small, sw, sh) = sample_grid(&big, bw, bh, grid);
        assert_eq!((sw, sh), (w, h), "{grid:?}");
        for (i, (a, b)) in small.iter().zip(&original).enumerate() {
            assert!(distance(*a, *b) <= 24, "pixel {i}: {a:?} vs {b:?}");
        }
    }

    #[test]
    fn an_image_without_a_grid_stays_one_to_one() {
        let (original, w, h) = sprite();
        let grid = detect_grid(&original, w, h);
        assert_eq!(grid.cell, (1.0, 1.0));
        assert_eq!(grid.count((w, h)), (w, h));
    }

    #[test]
    fn the_background_goes_but_not_what_is_inside() {
        let (pixels, w, h) = sprite();
        let out = remove_background(&pixels, w, h, 16);
        assert_eq!(out[0], T, "the white border");
        assert_eq!(
            out[(2 * w + 2) as usize],
            Rgba::rgb(10, 10, 10),
            "the eye stays"
        );
        assert_eq!(out[(w + 1) as usize], Rgba::rgb(200, 40, 40));
    }

    #[test]
    fn the_outline_drops_lonely_pixels_and_half_alphas_and_can_add_a_ring() {
        let r = Rgba::rgb(200, 40, 40);
        let half = Rgba::new(200, 40, 40, 100);
        // 5×3: un par rojo, un rojo suelto y un medio transparente.
        let pixels = vec![
            T, T, T, T, r, //
            T, r, r, T, T, //
            T, T, half, T, T,
        ];
        let out = clean_outline(&pixels, 5, 3, None);
        assert_eq!(out[4], T, "the lonely one");
        assert_eq!(out[6], r);
        assert_eq!(out[12], T, "half alpha goes");
        let black = Rgba::rgb(0, 0, 0);
        let ringed = clean_outline(&pixels, 5, 3, Some(black));
        assert_eq!(ringed[5], black, "left of the pair");
        assert_eq!(ringed[8], black, "right of the pair");
        assert_eq!(ringed[0], T, "no corners");
    }

    #[test]
    fn colors_are_reduced_to_a_palette() {
        let pixels: Vec<Rgba> = (0..64)
            .map(|i| Rgba::rgb(i as u8 * 4, 0, 0))
            .chain([T])
            .collect();
        let (out, palette) = reduce_colors(&pixels, 4);
        assert_eq!(palette.len(), 4);
        assert!(out.iter().filter(|c| c.a > 0).all(|c| palette.contains(c)));
        assert_eq!(*out.last().unwrap(), T);
    }

    #[test]
    fn a_shifted_upscale_finds_its_offset_too() {
        // 16×16 de colores que cambian por bloques irregulares.
        let (w, h) = (16u32, 16u32);
        let original: Vec<Rgba> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                let v = ((x * 7 + y * 13 + (x * y) % 5) % 6) as u8;
                Rgba::rgb(v * 40, 255 - v * 30, ((v as u32 * 70) % 255) as u8)
            })
            .collect();
        let (big, bw, bh) = upscale(&original, w, h, 8.0);
        // Recortado 3 px a la izquierda y arriba.
        let (cw, ch) = (bw - 3, bh - 3);
        let cropped: Vec<Rgba> = (3..bh)
            .flat_map(|y| (3..bw).map(move |x| (x, y)))
            .map(|(x, y)| big[(y * bw + x) as usize])
            .collect();
        let grid = detect_grid(&cropped, cw, ch);
        assert!((grid.cell.0 - 8.0).abs() <= 0.26, "{grid:?}");
        // Los bordes caen en x = 5, 13, …: el desfase es 5 (o -3, la misma posición con la
        // celda partida de la orilla contada).
        assert!(
            ((grid.offset.0 - 5.0).rem_euclid(8.0)).min((5.0 - grid.offset.0).rem_euclid(8.0))
                <= 1.0,
            "{grid:?}"
        );
        let (small, sw, sh) = sample_grid(&cropped, cw, ch, grid);
        assert_eq!(
            (sw, sh),
            (w, h),
            "the cut cells on the edge count: {grid:?}"
        );
        for (i, (a, b)) in small.iter().zip(&original).enumerate() {
            assert!(
                distance(*a, *b) <= 24,
                "pixel {i}: {a:?} vs {b:?} ({grid:?})"
            );
        }
    }
}
