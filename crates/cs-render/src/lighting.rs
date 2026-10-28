//! Una luz puntual sobre un sprite con su normal map (la vista previa de Lumen y lo que hornea).

use cs_core::Rgba;

use crate::{NormalFormat, field};

/// Una luz puntual. Posiciones y distancias en píxeles del sprite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// Posición sobre la imagen (puede caer fuera de ella).
    pub x: f32,
    pub y: f32,
    /// Altura sobre el sprite.
    pub height: f32,
    /// Distancia a la que la luz baja a la mitad.
    pub falloff: f32,
    pub color: Rgba,
    /// Multiplica la luz difusa.
    pub brightness: f32,
    /// Luz que llega a todo, 0..1.
    pub ambient: f32,
    /// Cuánto brilla solo lo que marca la máscara de luminance, 0..1.
    pub luminance: f32,
}

/// Decodifica un píxel de normal map a un vector con y hacia arriba.
fn decode(n: Rgba, format: NormalFormat) -> [f32; 3] {
    let v = |c: u8| c as f32 / 127.5 - 1.0;
    let y = v(n.g);
    [
        v(n.r),
        if format == NormalFormat::DirectX {
            -y
        } else {
            y
        },
        v(n.b),
    ]
}

/// El sprite `w`×`h` iluminado por `light`. `luminance` (0..255 por píxel) es lo que brilla solo.
/// El alfa es el del albedo.
pub fn lit(
    albedo: &[Rgba],
    normal: &[Rgba],
    luminance: Option<&[u8]>,
    w: u32,
    light: &Light,
    format: NormalFormat,
) -> Vec<Rgba> {
    let falloff = light.falloff.max(0.01);
    let tint = [light.color.r, light.color.g, light.color.b].map(|c| c as f32 / 255.0);
    albedo
        .iter()
        .zip(normal)
        .enumerate()
        .map(|(i, (&a, &n))| {
            if a.a == 0 {
                return a;
            }
            let (px, py) = ((i as u32 % w) as f32 + 0.5, (i as u32 / w) as f32 + 0.5);
            // La y de imagen va hacia abajo; la de la normal, hacia arriba.
            let to = [light.x - px, py - light.y, light.height];
            let d = (to[0] * to[0] + to[1] * to[1] + to[2] * to[2])
                .sqrt()
                .max(1e-4);
            let n = decode(n, format);
            let facing = ((n[0] * to[0] + n[1] * to[1] + n[2] * to[2]) / d).max(0.0);
            let diffuse = facing * light.brightness / (1.0 + (d / falloff).powi(2));
            let glow = luminance.map_or(0.0, |m| m[i] as f32 / 255.0) * light.luminance;
            let shade = |c: u8, t: f32| {
                let c = c as f32;
                (c * (light.ambient + diffuse * t) + c * glow)
                    .round()
                    .clamp(0.0, 255.0) as u8
            };
            Rgba::new(
                shade(a.r, tint[0]),
                shade(a.g, tint[1]),
                shade(a.b, tint[2]),
                a.a,
            )
        })
        .collect()
}

/// Máscara de lo que brilla solo: 255 donde la luminosidad Oklab llega a `threshold` (0..1).
pub fn luminance_mask(pixels: &[Rgba], threshold: f32) -> Vec<u8> {
    pixels
        .iter()
        .map(|p| {
            let on = p.a > 0 && field::lightness(p.r, p.g, p.b) >= threshold;
            if on { 255 } else { 0 }
        })
        .collect()
}

/// El color de fondo sólido de una imagen: el de las esquinas, si al menos tres coinciden y es
/// opaco.
pub fn detect_background(pixels: &[Rgba], w: u32, h: u32) -> Option<Rgba> {
    if w == 0 || h == 0 {
        return None;
    }
    let (w, h) = (w as usize, h as usize);
    let corners = [0, w - 1, (h - 1) * w, h * w - 1].map(|i| pixels[i]);
    corners
        .iter()
        .find(|c| c.a == 255 && corners.iter().filter(|o| o == c).count() >= 3)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAT: Rgba = Rgba::new(128, 128, 255, 255);

    fn light() -> Light {
        Light {
            x: 0.5,
            y: 0.5,
            height: 10.0,
            falloff: 100.0,
            color: Rgba::rgb(255, 255, 255),
            brightness: 1.0,
            ambient: 0.0,
            luminance: 1.0,
        }
    }

    #[test]
    fn a_pixel_facing_the_light_is_brighter() {
        let albedo = vec![Rgba::rgb(200, 200, 200); 2];
        // Luz a la izquierda: la normal que mira a la izquierda gana a la que mira a la derecha.
        let normal = [Rgba::new(30, 128, 200, 255), Rgba::new(226, 128, 200, 255)];
        let l = Light {
            x: -20.0,
            y: 0.5,
            height: 4.0,
            ..light()
        };
        let both = lit(&albedo, &normal, None, 1, &l, NormalFormat::OpenGl);
        assert!(both[0].r > both[1].r);
    }

    #[test]
    fn up_is_up_in_opengl_and_down_in_directx() {
        let albedo = vec![Rgba::rgb(200, 200, 200)];
        let up = [Rgba::new(128, 230, 200, 255)];
        let above = Light {
            y: -20.0,
            height: 4.0,
            ..light()
        };
        let gl = lit(&albedo, &up, None, 1, &above, NormalFormat::OpenGl)[0].r;
        let dx = lit(&albedo, &up, None, 1, &above, NormalFormat::DirectX)[0].r;
        assert!(gl > dx);
    }

    #[test]
    fn full_ambient_without_light_is_the_albedo() {
        let albedo = vec![Rgba::rgb(10, 120, 240), Rgba::TRANSPARENT];
        let l = Light {
            brightness: 0.0,
            ambient: 1.0,
            ..light()
        };
        let out = lit(&albedo, &[FLAT, FLAT], None, 2, &l, NormalFormat::OpenGl);
        assert_eq!(out, albedo);
    }

    #[test]
    fn luminance_lights_the_mask_without_light() {
        let albedo = vec![Rgba::rgb(100, 100, 100); 2];
        let l = Light {
            brightness: 0.0,
            ..light()
        };
        let out = lit(
            &albedo,
            &[FLAT, FLAT],
            Some(&[255, 0]),
            2,
            &l,
            NormalFormat::OpenGl,
        );
        assert_eq!(out[0], Rgba::rgb(100, 100, 100));
        assert_eq!(out[1], Rgba::rgb(0, 0, 0));
    }

    #[test]
    fn the_light_fades_with_distance() {
        let albedo = vec![Rgba::rgb(200, 200, 200)];
        let near = lit(&albedo, &[FLAT], None, 1, &light(), NormalFormat::OpenGl)[0].r;
        let far = Light {
            falloff: 5.0,
            ..light()
        };
        let far = lit(&albedo, &[FLAT], None, 1, &far, NormalFormat::OpenGl)[0].r;
        assert!(near > far);
    }

    #[test]
    fn the_mask_takes_the_bright_pixels() {
        let px = [
            Rgba::rgb(250, 240, 120),
            Rgba::rgb(40, 30, 60),
            Rgba::new(255, 255, 255, 0),
        ];
        assert_eq!(luminance_mask(&px, 0.6), vec![255, 0, 0]);
    }

    #[test]
    fn the_background_is_the_color_of_the_corners() {
        let bg = Rgba::rgb(0, 255, 0);
        let mut px = vec![bg; 9];
        px[4] = Rgba::rgb(1, 2, 3);
        assert_eq!(detect_background(&px, 3, 3), Some(bg));
        px[0] = Rgba::rgb(9, 9, 9);
        assert_eq!(
            detect_background(&px, 3, 3),
            Some(bg),
            "three corners are enough"
        );
        px[2] = Rgba::rgb(8, 8, 8);
        assert_eq!(detect_background(&px, 3, 3), None);
    }
}
