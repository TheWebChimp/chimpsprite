//! Compositor CPU de referencia: capas visibles → buffer RGBA.

use cs_core::{ColorMode, Layer, LayerKind, PixelValue, Rgba, Sprite};

/// Convierte un valor de píxel a color, resolviendo la paleta en modo indexado.
pub fn resolve(sprite: &Sprite, value: PixelValue) -> Rgba {
    match value {
        PixelValue::Rgba(c) => c,
        PixelValue::Index(i) => match sprite.color_mode {
            ColorMode::Indexed { transparent_index } if i == transparent_index => Rgba::TRANSPARENT,
            _ => sprite.palette.get(i).unwrap_or(Rgba::TRANSPARENT),
        },
    }
}

/// Mezcla "normal" (src over dst) con colores no premultiplicados; `alpha` ya incluye opacidades.
pub fn blend_over(dst: Rgba, src: Rgba, alpha: u8) -> Rgba {
    let sa = alpha as u32;
    if sa == 0 {
        return dst;
    }
    let da = dst.a as u32;
    let out_a = sa * 255 + da * (255 - sa);
    let ch = |s: u8, d: u8| {
        ((s as u32 * sa * 255 + d as u32 * da * (255 - sa) + out_a / 2) / out_a) as u8
    };
    Rgba::new(
        ch(src.r, dst.r),
        ch(src.g, dst.g),
        ch(src.b, dst.b),
        out_a.div_ceil(255).min(255) as u8,
    )
}

fn mul(a: u32, b: u32) -> u32 {
    (a * b + 127) / 255
}

/// Opacidad efectiva de la capa considerando los grupos ancestros; `None` si no es visible.
fn effective_opacity(sprite: &Sprite, layer: &Layer) -> Option<u32> {
    if !layer.visible {
        return None;
    }
    let mut opacity = layer.opacity as u32;
    let mut current = layer;
    while let Some(parent) = current.parent.and_then(|p| sprite.layer(p)) {
        if !parent.visible {
            return None;
        }
        opacity = mul(opacity, parent.opacity as u32);
        current = parent;
    }
    Some(opacity)
}

/// Compone el frame en un buffer de `width * height` píxeles, fila por fila.
pub fn render_frame(sprite: &Sprite, frame: usize) -> Vec<Rgba> {
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    let mut out = vec![Rgba::TRANSPARENT; (w * h) as usize];
    for layer in sprite.layers.iter().filter(|l| l.kind == LayerKind::Image) {
        let Some(layer_opacity) = effective_opacity(sprite, layer) else {
            continue;
        };
        let Some(cel) = sprite.cel(layer.id, frame) else {
            continue;
        };
        let Some(image) = sprite.image(cel.image) else {
            continue;
        };
        let opacity = mul(layer_opacity, cel.opacity as u32);
        for iy in 0..image.height() {
            let y = cel.position.1 + iy as i32;
            if y < 0 || y >= h {
                continue;
            }
            for ix in 0..image.width() {
                let x = cel.position.0 + ix as i32;
                if x < 0 || x >= w {
                    continue;
                }
                let src = resolve(sprite, image.get(ix, iy).expect("in bounds"));
                let alpha = mul(src.a as u32, opacity) as u8;
                let i = (y * w + x) as usize;
                out[i] = blend_over(out[i], src, alpha);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_commands::Command;
    use cs_core::*;

    fn draw(s: &mut Sprite, layer: u32, pixels: Vec<(i32, i32, PixelValue)>) {
        Command::DrawPixels {
            layer: LayerId(layer),
            frame: 0,
            pixels,
        }
        .apply(s)
        .unwrap();
    }

    #[test]
    fn blend_over_math() {
        let blue = Rgba::rgb(0, 0, 255);
        assert_eq!(
            blend_over(blue, Rgba::rgb(255, 0, 0), 255),
            Rgba::rgb(255, 0, 0)
        );
        assert_eq!(
            blend_over(blue, Rgba::rgb(255, 0, 0), 128),
            Rgba::new(128, 0, 127, 255)
        );
        assert_eq!(
            blend_over(Rgba::TRANSPARENT, Rgba::rgb(9, 9, 9), 0),
            Rgba::TRANSPARENT
        );
        assert_eq!(
            blend_over(Rgba::TRANSPARENT, Rgba::rgb(9, 9, 9), 100),
            Rgba::new(9, 9, 9, 100)
        );
    }

    #[test]
    fn renders_layers_bottom_to_top_and_skips_hidden() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
        draw(&mut s, 1, vec![(0, 0, red), (1, 0, red)]);
        Command::AddLayer {
            index: 1,
            layer: Layer::new_image(LayerId(2), "top"),
        }
        .apply(&mut s)
        .unwrap();
        draw(&mut s, 2, vec![(1, 0, green)]);
        assert_eq!(
            render_frame(&s, 0),
            vec![Rgba::rgb(255, 0, 0), Rgba::rgb(0, 255, 0)]
        );
        s.layer_mut(LayerId(2)).unwrap().visible = false;
        assert_eq!(
            render_frame(&s, 0),
            vec![Rgba::rgb(255, 0, 0), Rgba::rgb(255, 0, 0)]
        );
    }

    #[test]
    fn indexed_resolves_palette_and_transparent_index() {
        let mut s = Sprite::new(
            2,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        draw(&mut s, 1, vec![(0, 0, PixelValue::Index(21))]);
        assert_eq!(
            render_frame(&s, 0),
            vec![Rgba::rgb(255, 255, 255), Rgba::TRANSPARENT]
        );
    }

    #[test]
    fn hidden_group_hides_children() {
        let mut s = Sprite::new(1, 1, ColorMode::Rgba);
        draw(
            &mut s,
            1,
            vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 1, 1)))],
        );
        let mut group = Layer::new_image(LayerId(2), "g");
        group.kind = LayerKind::Group;
        group.visible = false;
        s.layers.push(group);
        s.layers[0].parent = Some(LayerId(2));
        assert_eq!(render_frame(&s, 0), vec![Rgba::TRANSPARENT]);
    }
}
