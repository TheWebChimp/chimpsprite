//! Mapas de normales a partir del sprite, para iluminación 2D en motores (Godot, Unity, Bevy) y
//! para Lumen. La altura sale de la silueta (distancia al borde) más el sombreado pintado
//! (luminosidad Oklab); cada isla de píxeles se procesa por separado.
//!
//! Portado de Sprite Fusion Sprite Normals (https://github.com/Hugo-Dz/spritefusion-sprite-normals):
//!
//! MIT License
//!
//! Copyright (c) 2026 Hugo Duprez
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//! and associated documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//! BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use cs_core::Rgba;

use crate::field::{self, Field};

/// Hacia dónde apunta el verde: OpenGL (arriba; Godot, Unity, Bevy) o DirectX (abajo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormalFormat {
    #[default]
    OpenGl,
    DirectX,
}

/// Ajustes del normal map. Distancias y radios en píxeles del sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalSettings {
    /// Los píxeles con esta opacidad o menos quedan con la normal plana.
    pub alpha_threshold: f32,
    /// Distancia en la que se apaga la pendiente de la silueta.
    pub shape_radius: f32,
    pub shape_strength: f32,
    pub shape_softness: f32,
    /// Cuánto relieve sale del sombreado pintado.
    pub detail_strength: f32,
    pub detail_scale: f32,
    pub normal_strength: f32,
    pub format: NormalFormat,
    pub invert_x: bool,
    pub invert_y: bool,
}

impl Default for NormalSettings {
    fn default() -> Self {
        Self {
            alpha_threshold: 0.03,
            shape_radius: 4.0,
            shape_strength: 1.15,
            shape_softness: 1.15,
            detail_strength: 6.0,
            detail_scale: 0.6,
            normal_strength: 1.5,
            format: NormalFormat::OpenGl,
            invert_x: false,
            invert_y: false,
        }
    }
}

/// Rangos válidos de cada ajuste.
pub const SHAPE_RADIUS: (f32, f32) = (0.5, 64.0);
pub const STRENGTH: (f32, f32) = (0.0, 8.0);
pub const DETAIL_SCALE: (f32, f32) = (0.6, 8.0);

impl NormalSettings {
    /// Los mismos ajustes dentro de sus rangos (lo que no es finito vuelve al default).
    pub fn clamped(self) -> Self {
        let d = Self::default();
        let fit = |v: f32, (lo, hi): (f32, f32), default: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                default
            }
        };
        Self {
            alpha_threshold: fit(self.alpha_threshold, (0.0, 0.99), d.alpha_threshold),
            shape_radius: fit(self.shape_radius, SHAPE_RADIUS, d.shape_radius),
            shape_strength: fit(self.shape_strength, STRENGTH, d.shape_strength),
            shape_softness: fit(self.shape_softness, STRENGTH, d.shape_softness),
            detail_strength: fit(self.detail_strength, STRENGTH, d.detail_strength),
            detail_scale: fit(self.detail_scale, DETAIL_SCALE, d.detail_scale),
            normal_strength: fit(self.normal_strength, STRENGTH, d.normal_strength),
            ..self
        }
    }

    fn padding(&self) -> usize {
        (3.0 * self.shape_softness.max(self.detail_scale * 2.0)).ceil() as usize + 3
    }
}

/// La normal plana, con el alfa del píxel.
fn flat(a: u8) -> Rgba {
    Rgba::new(128, 128, 255, a)
}

/// Normal map de una imagen `w`×`h` con las mismas medidas y el mismo alfa. Los píxeles
/// transparentes quedan con la normal plana.
pub fn normal_map(pixels: &[Rgba], w: u32, h: u32, settings: &NormalSettings) -> Vec<Rgba> {
    let settings = settings.clamped();
    let (w, h) = (w as usize, h as usize);
    let mut out: Vec<Rgba> = pixels.iter().map(|p| flat(p.a)).collect();
    let mask: Vec<bool> = pixels
        .iter()
        .map(|p| p.a as f32 / 255.0 > settings.alpha_threshold)
        .collect();
    for part in islands(&mask, w, h) {
        let (iw, ih) = (part.max_x - part.min_x + 1, part.max_y - part.min_y + 1);
        let mut island = vec![Rgba::TRANSPARENT; iw * ih];
        for &i in &part.pixels {
            let (x, y) = (i % w, i / w);
            island[(y - part.min_y) * iw + x - part.min_x] = pixels[i];
        }
        let relief = relief(&island, iw, ih, &settings);
        for &i in &part.pixels {
            let (x, y) = (i % w, i / w);
            out[i] = relief[(y - part.min_y) * iw + x - part.min_x];
        }
    }
    out
}

/// Una isla de píxeles de 8-vecinos (las diagonales del pixel art quedan juntas).
struct Island {
    pixels: Vec<usize>,
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
}

fn islands(mask: &[bool], width: usize, height: usize) -> Vec<Island> {
    let mut visited = vec![false; mask.len()];
    let mut result = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || visited[start] {
            continue;
        }
        let mut part = Island {
            pixels: vec![start],
            min_x: start % width,
            min_y: start / width,
            max_x: start % width,
            max_y: start / width,
        };
        visited[start] = true;
        let mut cursor = 0;
        while cursor < part.pixels.len() {
            let index = part.pixels[cursor];
            let (x, y) = (index % width, index / width);
            part.min_x = part.min_x.min(x);
            part.max_x = part.max_x.max(x);
            part.min_y = part.min_y.min(y);
            part.max_y = part.max_y.max(y);
            for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                    let neighbor = ny * width + nx;
                    if mask[neighbor] && !visited[neighbor] {
                        visited[neighbor] = true;
                        part.pixels.push(neighbor);
                    }
                }
            }
            cursor += 1;
        }
        result.push(part);
    }
    result
}

/// Las normales de una isla (`w`×`h`, lo de fuera transparente).
fn relief(pixels: &[Rgba], w: usize, h: usize, settings: &NormalSettings) -> Vec<Rgba> {
    let pad = settings.padding();
    let (width, height) = (w + 2 * pad, h + 2 * pad);
    let mut opacity = Field::zero(width, height);
    let mut lightness = opacity.clone();
    let mut mask = vec![false; width * height];
    let (mut low, mut high) = (1.0_f32, 0.0_f32);
    for (k, p) in pixels.iter().enumerate() {
        let i = (k / w + pad) * width + k % w + pad;
        let alpha = p.a as f32 / 255.0;
        if alpha <= settings.alpha_threshold {
            continue;
        }
        mask[i] = true;
        opacity.data[i] = alpha;
        let value = field::lightness(p.r, p.g, p.b);
        lightness.data[i] = value;
        low = low.min(value);
        high = high.max(value);
    }
    // Una altura constante no tiene normal: centrarla evita ruido en el arte plano.
    for (i, value) in lightness.data.iter_mut().enumerate() {
        if mask[i] {
            *value -= (low + high) * 0.5;
        }
    }
    let mut shape = field::edt(&mask, width, height);
    let reach = settings.shape_radius;
    for value in &mut shape.data {
        let inset = (*value - 0.5).max(0.0);
        *value = settings.shape_strength * reach * (inset / reach).ln_1p();
    }
    let mut combined = field::gaussian(&shape, settings.shape_softness);
    let fine = field::masked_gaussian(&lightness, &opacity, settings.detail_scale);
    let broad = field::masked_gaussian(&lightness, &opacity, settings.detail_scale * 2.0);
    for (i, value) in combined.data.iter_mut().enumerate() {
        *value += settings.detail_strength * (0.7 * fine.data[i] + 0.3 * broad.data[i]);
    }
    pixels
        .iter()
        .enumerate()
        .map(|(k, p)| {
            let i = (k / w + pad) * width + k % w + pad;
            if !mask[i] {
                return flat(p.a);
            }
            // El relleno deja la vecindad 3×3 de cada píxel opaco dentro del campo.
            let (dx, dy) = field::sobel(&combined, i);
            let [r, g, b] =
                slopes_to_normal(dx, dy, settings).map(|v| ((v * 0.5 + 0.5) * 255.0).round() as u8);
            Rgba::new(r, g, b, p.a)
        })
        .collect()
}

/// Pendientes de la altura (y de imagen hacia abajo) → normal unitaria en la convención pedida.
fn slopes_to_normal(dx: f32, dy_image: f32, settings: &NormalSettings) -> [f32; 3] {
    let mut x = -dx * settings.normal_strength;
    let mut y = dy_image * settings.normal_strength;
    if settings.format == NormalFormat::DirectX {
        y = -y;
    }
    if settings.invert_x {
        x = -x;
    }
    if settings.invert_y {
        y = -y;
    }
    let length = x.mul_add(x, y.mul_add(y, 1.0)).sqrt();
    [x / length, y / length, 1.0 / length]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk(size: u32) -> Vec<Rgba> {
        let center = (size / 2) as f32;
        (0..size * size)
            .map(|i| {
                let (x, y) = ((i % size) as f32, (i / size) as f32);
                let a = if (x - center).hypot(y - center) <= center - 3.0 {
                    255
                } else {
                    0
                };
                Rgba::new(145, 99, 63, a)
            })
            .collect()
    }

    fn map(px: &[Rgba], w: u32, h: u32, s: NormalSettings) -> Vec<Rgba> {
        normal_map(px, w, h, &s)
    }

    #[test]
    fn normals_point_outward_and_decode_to_unit_vectors() {
        let n = map(&disk(31), 31, 31, NormalSettings::default());
        let at = |x: usize, y: usize| n[y * 31 + x];
        assert!(at(7, 15).r < 100, "left faces left");
        assert!(at(23, 15).r > 155, "right faces right");
        assert!(at(15, 7).g > 155, "top faces up (OpenGL)");
        assert!(at(15, 23).g < 100, "bottom faces down");
        assert_eq!(at(15, 15), Rgba::new(128, 128, 255, 255));
        for p in &n {
            let len = [p.r, p.g, p.b]
                .iter()
                .map(|&c| (c as f32 / 127.5 - 1.0).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!((len - 1.0).abs() < 0.01);
        }
    }

    #[test]
    fn zero_strength_gives_flat_normals() {
        let px = disk(31);
        for s in [
            NormalSettings {
                normal_strength: 0.0,
                ..Default::default()
            },
            NormalSettings {
                shape_strength: 0.0,
                ..Default::default()
            },
        ] {
            for (src, n) in px.iter().zip(map(&px, 31, 31, s)) {
                assert_eq!(n, flat(src.a));
            }
        }
    }

    #[test]
    fn painted_shading_gives_relief() {
        let px: Vec<Rgba> = (0..64 * 32)
            .map(|i| {
                let v = (40 + (i % 64) * 3) as u8;
                Rgba::rgb(v, v, v)
            })
            .collect();
        let n = map(
            &px,
            64,
            32,
            NormalSettings {
                shape_strength: 0.0,
                ..Default::default()
            },
        );
        for x in 12..52 {
            let p = n[16 * 64 + x];
            assert!(p.r < 123, "brighter to the right → leans left");
            assert!((p.g as i16 - 128).abs() <= 1);
        }
    }

    #[test]
    fn alpha_is_kept_and_hidden_colors_do_not_count() {
        let original = disk(31);
        let changed: Vec<Rgba> = original
            .iter()
            .map(|&p| {
                if p.a == 0 {
                    Rgba::new(255, 20, 200, 0)
                } else {
                    p
                }
            })
            .collect();
        let a = map(&original, 31, 31, Default::default());
        assert_eq!(a, map(&changed, 31, 31, Default::default()));
        assert!(original.iter().zip(&a).all(|(s, n)| s.a == n.a));
        let empty = vec![Rgba::new(255, 80, 10, 0); 27];
        assert!(
            map(&empty, 3, 9, Default::default())
                .iter()
                .all(|&p| p == flat(0))
        );
    }

    #[test]
    fn nearby_islands_do_not_change_a_sprite() {
        let sprite: Vec<Rgba> = (0..81)
            .map(|i| Rgba::rgb((30 + (i % 9) * 20) as u8, 80, 120))
            .collect();
        let expected = map(&sprite, 9, 9, Default::default());
        let (w, h) = (42usize, 34usize);
        let mut sheet = vec![Rgba::TRANSPARENT; w * h];
        for y in 0..9 {
            for x in 0..9 {
                sheet[(y + 5) * w + x + 5] = sprite[y * 9 + x];
            }
        }
        for y in 5..28 {
            for x in 15..36 {
                sheet[y * w + x] = Rgba::rgb(255, 255, 255);
            }
        }
        let n = map(&sheet, w as u32, h as u32, Default::default());
        for y in 0..9 {
            for x in 0..9 {
                assert_eq!(n[(y + 5) * w + x + 5], expected[y * 9 + x]);
            }
        }
    }

    #[test]
    fn tiny_shapes_work() {
        for (w, h) in [(1u32, 1u32), (1, 19), (23, 1), (20, 20)] {
            let px: Vec<Rgba> = (0..w * h)
                .map(|i| {
                    let on = w == 1 || h == 1 || i % w == i / w;
                    Rgba::new(80, 80, 80, if on { 255 } else { 0 })
                })
                .collect();
            assert_eq!(map(&px, w, h, Default::default()).len(), (w * h) as usize);
        }
    }

    #[test]
    fn conventions_only_flip_the_requested_channels() {
        let px = disk(31);
        let gl = map(&px, 31, 31, Default::default());
        let dx = map(
            &px,
            31,
            31,
            NormalSettings {
                format: NormalFormat::DirectX,
                ..Default::default()
            },
        );
        let flipped = map(
            &px,
            31,
            31,
            NormalSettings {
                invert_x: true,
                invert_y: true,
                ..Default::default()
            },
        );
        for ((a, b), c) in gl.iter().zip(&dx).zip(&flipped) {
            assert_eq!([a.r, a.b, a.a], [b.r, b.b, b.a]);
            assert!((a.g as i16 + b.g as i16 - 255).abs() <= 1);
            assert!((a.r as i16 + c.r as i16 - 255).abs() <= 1);
            assert_eq!(b.g, c.g);
        }
    }

    #[test]
    fn settings_out_of_range_are_clamped() {
        let s = NormalSettings {
            normal_strength: f32::NAN,
            shape_radius: 0.0,
            detail_strength: 99.0,
            ..Default::default()
        }
        .clamped();
        assert_eq!(s.normal_strength, 1.5);
        assert_eq!(s.shape_radius, 0.5);
        assert_eq!(s.detail_strength, 8.0);
    }
}
