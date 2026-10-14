//! Compositor CPU de referencia: capas visibles → buffer RGBA.

use cs_core::{BlendMode, ColorMode, Layer, LayerKind, PixelValue, Rgba, Sprite};

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

/// Fórmula separable de cada modo (canales en 0..=1): `b` fondo, `s` capa.
fn blend_channel(mode: BlendMode, b: f32, s: f32) -> f32 {
    match mode {
        BlendMode::Normal | BlendMode::Other(_) => s,
        BlendMode::Multiply => b * s,
        BlendMode::Screen => b + s - b * s,
        BlendMode::Overlay => {
            if b <= 0.5 {
                2.0 * b * s
            } else {
                1.0 - 2.0 * (1.0 - b) * (1.0 - s)
            }
        }
        BlendMode::Darken => b.min(s),
        BlendMode::Lighten => b.max(s),
        BlendMode::Addition => (b + s).min(1.0),
        BlendMode::Subtract => (b - s).max(0.0),
        BlendMode::Difference => (b - s).abs(),
    }
}

/// Mezcla `src` sobre `dst` con un modo. Donde el fondo es transparente, se ve el color de la capa;
/// donde es opaco, el resultado del modo (como en la especificación W3C de compositing).
pub fn blend(dst: Rgba, src: Rgba, alpha: u8, mode: BlendMode) -> Rgba {
    if matches!(mode, BlendMode::Normal | BlendMode::Other(_)) || dst.a == 0 {
        return blend_over(dst, src, alpha);
    }
    let ab = dst.a as f32 / 255.0;
    let ch = |b: u8, s: u8| {
        let (b, s) = (b as f32 / 255.0, s as f32 / 255.0);
        let mixed = (1.0 - ab) * s + ab * blend_channel(mode, b, s);
        (mixed * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let mixed = Rgba::new(ch(dst.r, src.r), ch(dst.g, src.g), ch(dst.b, src.b), src.a);
    blend_over(dst, mixed, alpha)
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
                out[i] = blend(out[i], src, alpha, layer.blend);
            }
        }
    }
    out
}

/// Opciones del papel cebolla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnionSkin {
    /// Frames anteriores a mostrar.
    pub before: usize,
    /// Frames siguientes a mostrar.
    pub after: usize,
    /// Opacidad del frame vecino más cercano; los más lejanos se desvanecen.
    pub opacity: u8,
    /// Teñir de rojo los anteriores y de azul los siguientes.
    pub tint: bool,
}

impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            before: 1,
            after: 1,
            opacity: 96,
            tint: true,
        }
    }
}

const TINT_BEFORE: Rgba = Rgba::rgb(255, 40, 40);
const TINT_AFTER: Rgba = Rgba::rgb(40, 90, 255);

fn tinted(c: Rgba, tint: Rgba) -> Rgba {
    let mix = |a: u8, b: u8| ((a as u32 + b as u32) / 2) as u8;
    Rgba::new(mix(c.r, tint.r), mix(c.g, tint.g), mix(c.b, tint.b), c.a)
}

/// Frame actual con los vecinos semitransparentes debajo (papel cebolla).
pub fn render_onion(sprite: &Sprite, frame: usize, onion: &OnionSkin) -> Vec<Rgba> {
    let mut out = vec![Rgba::TRANSPARENT; (sprite.width * sprite.height) as usize];
    let mut ghost = |f: usize, distance: usize, count: usize, tint: Rgba| {
        let alpha = onion.opacity as u32 * (count - distance + 1) as u32 / count as u32;
        for (o, c) in out.iter_mut().zip(render_frame(sprite, f)) {
            let c = if onion.tint { tinted(c, tint) } else { c };
            *o = blend_over(*o, c, mul(c.a as u32, alpha) as u8);
        }
    };
    // De lo más lejano a lo más cercano, para que lo cercano quede encima.
    for d in (1..=onion.before).rev() {
        if let Some(f) = frame.checked_sub(d) {
            ghost(f, d, onion.before, TINT_BEFORE);
        }
    }
    for d in (1..=onion.after).rev() {
        if frame + d < sprite.frames.len() {
            ghost(frame + d, d, onion.after, TINT_AFTER);
        }
    }
    for (o, c) in out.iter_mut().zip(render_frame(sprite, frame)) {
        *o = blend_over(*o, c, c.a);
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
    fn blend_modes_on_opaque_backdrop() {
        let b = Rgba::rgb(200, 100, 50);
        let s = Rgba::rgb(100, 100, 100);
        let px = |mode| blend(b, s, 255, mode);
        assert_eq!(px(BlendMode::Normal), s);
        assert_eq!(px(BlendMode::Multiply), Rgba::rgb(78, 39, 20));
        assert_eq!(px(BlendMode::Screen), Rgba::rgb(222, 161, 130));
        assert_eq!(px(BlendMode::Overlay), Rgba::rgb(188, 78, 39));
        assert_eq!(px(BlendMode::Darken), Rgba::rgb(100, 100, 50));
        assert_eq!(px(BlendMode::Lighten), Rgba::rgb(200, 100, 100));
        assert_eq!(px(BlendMode::Addition), Rgba::rgb(255, 200, 150));
        assert_eq!(px(BlendMode::Subtract), Rgba::rgb(100, 0, 0));
        assert_eq!(px(BlendMode::Difference), Rgba::rgb(100, 0, 50));
        assert_eq!(px(BlendMode::Other(99)), s);
    }

    #[test]
    fn blend_mode_over_transparent_shows_layer_color() {
        let s = Rgba::rgb(100, 100, 100);
        assert_eq!(blend(Rgba::TRANSPARENT, s, 255, BlendMode::Multiply), s);
    }

    #[test]
    fn onion_shows_previous_frame_under_current() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
        draw(&mut s, 1, vec![(0, 0, red), (1, 0, red)]);
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 1,
            pixels: vec![(1, 0, green)],
        }
        .apply(&mut s)
        .unwrap();
        let onion = OnionSkin {
            before: 1,
            after: 1,
            opacity: 128,
            tint: false,
        };
        let out = render_onion(&s, 1, &onion);
        assert_eq!(
            out[0],
            Rgba::new(255, 0, 0, 128),
            "fantasma del frame anterior"
        );
        assert_eq!(
            out[1],
            Rgba::rgb(0, 255, 0),
            "el frame actual tapa al fantasma"
        );
        // Sin vecinos que mostrar, igual al render normal.
        let none = OnionSkin {
            before: 0,
            after: 0,
            ..onion
        };
        assert_eq!(render_onion(&s, 1, &none), render_frame(&s, 1));
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
