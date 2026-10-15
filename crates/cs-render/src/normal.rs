//! Mapas de normales a partir del sprite, para iluminación 2D en motores (Godot, Unity, Bevy).
//! La altura sale de la distancia al borde (un bisel) más un poco de luminosidad; las normales usan
//! la convención OpenGL (verde = arriba), que es la de Godot y Unity.

use std::collections::VecDeque;

use cs_core::Rgba;

/// Ancho del bisel en píxeles: a esta distancia del borde la superficie ya es plana.
const BEVEL: f32 = 3.0;
/// Cuánto aporta la luminosidad a la altura.
const LUMA_WEIGHT: f32 = 0.35;

/// Normal map de una imagen `w`×`h`. `strength` acentúa el relieve (1.0 = normal). Los píxeles
/// transparentes quedan con la normal plana y alfa 0.
pub fn normal_map(pixels: &[Rgba], w: u32, h: u32, strength: f32) -> Vec<Rgba> {
    let (w, h) = (w as i32, h as i32);
    let solid =
        |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && pixels[(y * w + x) as usize].a > 0;
    // Distancia (8-vecinos) al píxel transparente o borde más cercano.
    let mut dist = vec![u32::MAX; (w * h) as usize];
    let mut queue = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            let edge = solid(x, y)
                && [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|(dx, dy)| !solid(x + dx, y + dy));
            if !solid(x, y) {
                dist[(y * w + x) as usize] = 0;
            } else if edge {
                dist[(y * w + x) as usize] = 1;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        let d = dist[(y * w + x) as usize];
        for (dx, dy) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (nx, ny) = (x + dx, y + dy);
            if solid(nx, ny) && dist[(ny * w + nx) as usize] > d + 1 {
                dist[(ny * w + nx) as usize] = d + 1;
                queue.push_back((nx, ny));
            }
        }
    }
    let height = |x: i32, y: i32| -> f32 {
        if !solid(x, y) {
            return 0.0;
        }
        let i = (y * w + x) as usize;
        let c = pixels[i];
        let luma = (c.r as f32 * 0.299 + c.g as f32 * 0.587 + c.b as f32 * 0.114) / 255.0;
        (dist[i] as f32 / BEVEL).min(1.0) + LUMA_WEIGHT * luma
    };
    let mut out = Vec::with_capacity(pixels.len());
    for y in 0..h {
        for x in 0..w {
            if !solid(x, y) {
                out.push(Rgba::new(128, 128, 255, 0));
                continue;
            }
            // Sobel.
            let hx = |dx: i32, dy: i32| height(x + dx, y + dy);
            let gx = (hx(1, -1) + 2.0 * hx(1, 0) + hx(1, 1))
                - (hx(-1, -1) + 2.0 * hx(-1, 0) + hx(-1, 1));
            let gy = (hx(-1, 1) + 2.0 * hx(0, 1) + hx(1, 1))
                - (hx(-1, -1) + 2.0 * hx(0, -1) + hx(1, -1));
            // Imagen con y hacia abajo → normal con y hacia arriba.
            let (nx, ny, nz) = (-gx * strength, gy * strength, 1.0f32);
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            let to_byte = |v: f32| ((v / len * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8;
            out.push(Rgba::new(
                to_byte(nx),
                to_byte(ny),
                to_byte(nz),
                pixels[(y * w + x) as usize].a,
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_bulges_outward() {
        let (w, h) = (9u32, 9u32);
        let px: Vec<Rgba> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                if (1..8).contains(&x) && (1..8).contains(&y) {
                    Rgba::rgb(128, 128, 128)
                } else {
                    Rgba::TRANSPARENT
                }
            })
            .collect();
        let n = normal_map(&px, w, h, 1.0);
        let at = |x: u32, y: u32| n[(y * w + x) as usize];
        assert_eq!(
            at(0, 0),
            Rgba::new(128, 128, 255, 0),
            "transparent stays flat"
        );
        assert!(at(1, 4).r < 110, "left edge faces left");
        assert!(at(7, 4).r > 146, "right edge faces right");
        assert!(at(4, 1).g > 146, "top edge faces up (OpenGL)");
        assert!(at(4, 7).g < 110, "bottom edge faces down");
        let center = at(4, 4);
        assert!((center.r as i32 - 128).abs() <= 2 && (center.g as i32 - 128).abs() <= 2);
        assert_eq!(center.a, 255);
    }
}
