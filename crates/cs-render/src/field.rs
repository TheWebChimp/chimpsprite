//! Imágenes escalares para los normal maps: gaussianas, Sobel, distancia euclidiana exacta y la
//! luminosidad Oklab. Portado de Sprite Fusion Sprite Normals (ver el aviso en `normal.rs`).

use std::sync::LazyLock;

/// Una imagen escalar por filas, en píxeles del sprite.
#[derive(Clone)]
pub(crate) struct Field {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl Field {
    pub(crate) fn zero(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height],
        }
    }
}

/// Gaussiana separable con ceros afuera (la altura de la forma puede pasar del alfa).
pub(crate) fn gaussian(input: &Field, sigma: f32) -> Field {
    if sigma < 0.05 {
        return input.clone();
    }
    let radius = (sigma * 3.0).ceil() as isize;
    let mut kernel: Vec<f32> = (-radius..=radius)
        .map(|i| (-0.5 * (i as f32 / sigma).powi(2)).exp())
        .collect();
    let total: f32 = kernel.iter().sum();
    for value in &mut kernel {
        *value /= total;
    }
    let width = input.width;
    let mut horizontal = Field::zero(width, input.height);
    let rows = input.data.chunks_exact(width);
    for (output, source) in horizontal.data.chunks_exact_mut(width).zip(rows) {
        for (offset, &weight) in (-radius..=radius).zip(&kernel) {
            // La x de salida lee la x + offset de la fuente; lo que cae fuera de la fila es cero.
            let skip = offset.unsigned_abs().min(width);
            let (output, source) = if offset < 0 {
                (&mut output[skip..], &source[..width - skip])
            } else {
                (&mut output[..width - skip], &source[skip..])
            };
            add_scaled(output, source, weight);
        }
    }
    let mut result = Field::zero(width, input.height);
    for (y, output) in result.data.chunks_exact_mut(width).enumerate() {
        for (offset, &weight) in (-radius..=radius).zip(&kernel) {
            if let Some(row) = y
                .checked_add_signed(offset)
                .filter(|&row| row < input.height)
            {
                add_scaled(output, &horizontal.data[row * width..][..width], weight);
            }
        }
    }
    result
}

fn add_scaled(output: &mut [f32], source: &[f32], weight: f32) {
    for (value, sample) in output.iter_mut().zip(source) {
        *value += sample * weight;
    }
}

/// Convolución normalizada por la opacidad: el RGB de lo transparente nunca cuenta.
pub(crate) fn masked_gaussian(input: &Field, weight: &Field, sigma: f32) -> Field {
    let mut weighted = input.clone();
    for (value, alpha) in weighted.data.iter_mut().zip(&weight.data) {
        *value *= alpha;
    }
    let mut result = gaussian(&weighted, sigma);
    let denominator = gaussian(weight, sigma);
    for (value, weight) in result.data.iter_mut().zip(denominator.data) {
        *value /= weight.max(1e-8);
    }
    result
}

/// Sobel normalizado en un píxel lejos del borde: x crece a la derecha, y hacia abajo.
pub(crate) fn sobel(input: &Field, index: usize) -> (f32, f32) {
    let d = &input.data;
    let (up, down) = (index - input.width, index + input.width);
    let dx = (d[up + 1] + 2.0 * d[index + 1] + d[down + 1]
        - d[up - 1]
        - 2.0 * d[index - 1]
        - d[down - 1])
        * 0.125;
    let dy =
        (d[down - 1] + 2.0 * d[down] + d[down + 1] - d[up - 1] - 2.0 * d[up] - d[up + 1]) * 0.125;
    (dx, dy)
}

/// Una fila de la distancia al cuadrado (envolvente inferior de parábolas).
fn squared_line(source: &[f32], destination: &mut [f32]) {
    let mut sites: Vec<usize> = Vec::with_capacity(source.len());
    let mut starts: Vec<f64> = Vec::with_capacity(source.len());
    for (q, &height) in source.iter().enumerate() {
        if !height.is_finite() {
            continue;
        }
        let mut intersection = f64::NEG_INFINITY;
        while let Some(&p) = sites.last() {
            intersection = (f64::from(height) + (q as f64).powi(2)
                - f64::from(source[p])
                - (p as f64).powi(2))
                / (2.0 * (q - p) as f64);
            if intersection > *starts.last().expect("cada sitio tiene inicio") {
                break;
            }
            sites.pop();
            starts.pop();
        }
        if sites.is_empty() {
            intersection = f64::NEG_INFINITY;
        }
        sites.push(q);
        starts.push(intersection);
    }
    if sites.is_empty() {
        destination.fill(f32::INFINITY);
        return;
    }
    let mut active = 0;
    for (q, output) in destination.iter_mut().enumerate() {
        while active + 1 < starts.len() && starts[active + 1] < q as f64 {
            active += 1;
        }
        let delta = q as f32 - sites[active] as f32;
        *output = delta.mul_add(delta, source[sites[active]]);
    }
}

/// Distancia euclidiana exacta de cada píxel de `mask` al píxel de fuera más cercano.
pub(crate) fn edt(mask: &[bool], width: usize, height: usize) -> Field {
    let mut result = Field::zero(width, height);
    let mut source = vec![0.0; width.max(height)];
    let mut destination = source.clone();
    for y in 0..height {
        for x in 0..width {
            source[x] = if mask[y * width + x] {
                f32::INFINITY
            } else {
                0.0
            };
        }
        squared_line(&source[..width], &mut destination[..width]);
        result.data[y * width..(y + 1) * width].copy_from_slice(&destination[..width]);
    }
    for x in 0..width {
        for (y, value) in source.iter_mut().take(height).enumerate() {
            *value = result.data[y * width + x];
        }
        squared_line(&source[..height], &mut destination[..height]);
        for (y, value) in destination.iter().take(height).enumerate() {
            result.data[y * width + x] = value.sqrt();
        }
    }
    result
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Cada valor de 8 bits ya linealizado.
static LINEAR: LazyLock<[f32; 256]> =
    LazyLock::new(|| std::array::from_fn(|v| srgb_to_linear(v as f32 / 255.0)));

/// Luminosidad Oklab (0..1) de un color sRGB.
pub(crate) fn lightness(r: u8, g: u8, b: u8) -> f32 {
    let [r, g, b] = [r, g, b].map(|v| LINEAR[usize::from(v)]);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_distance_matches_brute_force_on_small_random_masks() {
        let mut state = 75_u32;
        for height in 1..13 {
            for width in 1..17 {
                let mask: Vec<bool> = (0..width * height)
                    .map(|_| {
                        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        !state.is_multiple_of(4)
                    })
                    .collect();
                let distances = edt(&mask, width, height);
                for (index, &actual) in distances.data.iter().enumerate() {
                    let expected = mask
                        .iter()
                        .enumerate()
                        .filter(|(_, inside)| !**inside)
                        .map(|(other, _)| {
                            let dx = (index % width) as f32 - (other % width) as f32;
                            let dy = (index / width) as f32 - (other / width) as f32;
                            dx.hypot(dy)
                        })
                        .fold(f32::INFINITY, f32::min);
                    assert!(
                        (actual.is_infinite() && expected.is_infinite())
                            || (actual - expected).abs() < 1e-5
                    );
                }
            }
        }
    }

    #[test]
    fn lightness_goes_from_black_to_white() {
        assert!(lightness(0, 0, 0).abs() < 1e-4);
        assert!((lightness(255, 255, 255) - 1.0).abs() < 1e-3);
        assert!(lightness(0, 255, 0) > lightness(0, 0, 255));
    }
}
