//! Compositor CPU de referencia: capas visibles → buffer RGBA.

use std::sync::Arc;

mod materialize;
mod normal;
pub use materialize::{Bake, bake_layers, materialize, materialize_tilemaps};
pub use normal::normal_map;

use cs_core::{BlendMode, ColorMode, Image, ImageId, Layer, LayerKind, PixelValue, Rgba, Sprite};

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

/// Interpola de `a` hacia `b` (canales y alfa) en `t` (0–255), como `rgba_blender_merge` de
/// Aseprite: si un lado es transparente se toma el color del otro.
fn merge(a: Rgba, b: Rgba, t: u8) -> Rgba {
    let t = t as i32;
    let lerp = |x: u8, y: u8| (x as i32 + (y as i32 - x as i32) * t / 255) as u8;
    let (r, g, bl) = if a.a == 0 {
        (b.r, b.g, b.b)
    } else if b.a == 0 {
        (a.r, a.g, a.b)
    } else {
        (lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b))
    };
    let alpha = lerp(a.a, b.a);
    if alpha == 0 {
        Rgba::TRANSPARENT
    } else {
        Rgba::new(r, g, bl, alpha)
    }
}

/// Mezcla `src` sobre `dst` con un modo, igual que Aseprite: sobre fondo transparente se ve la
/// capa; sobre fondo opaco, el resultado del modo; sobre semitransparente, una mezcla de ambos
/// según el alfa del fondo y el de la capa.
pub fn blend(dst: Rgba, src: Rgba, alpha: u8, mode: BlendMode) -> Rgba {
    if matches!(mode, BlendMode::Normal | BlendMode::Other(_)) || dst.a == 0 {
        return blend_over(dst, src, alpha);
    }
    let ch = |b: u8, s: u8| {
        let mixed = blend_channel(mode, b as f32 / 255.0, s as f32 / 255.0);
        (mixed * 255.0).round().clamp(0.0, 255.0) as u8
    };
    let moded = Rgba::new(ch(dst.r, src.r), ch(dst.g, src.g), ch(dst.b, src.b), src.a);
    let blended = blend_over(dst, moded, alpha);
    if dst.a == 255 {
        return blended;
    }
    let normal = blend_over(dst, src, alpha);
    let toward_mode = merge(normal, blended, dst.a);
    merge(toward_mode, blended, mul(dst.a as u32, alpha as u32) as u8)
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

/// ¿La capa necesita máscara o recorte al componer?
fn is_masked(sprite: &Sprite, layer: &Layer) -> bool {
    if layer.clip {
        return true;
    }
    let mut current = Some(layer);
    while let Some(l) = current {
        if l.mask.as_ref().is_some_and(|m| m.enabled) {
            return true;
        }
        current = l.parent.and_then(|p| sprite.layer(p));
    }
    false
}

/// Alfa (0–255) de la capa base de un recorte en `(x, y)`: sus píxeles por su máscara. Una base
/// oculta oculta también a las capas que recorta; un grupo como base no recorta.
fn base_alpha(sprite: &Sprite, base: &Layer, frame: usize, x: i32, y: i32) -> u32 {
    if effective_opacity(sprite, base).is_none() {
        return 0;
    }
    if base.kind == LayerKind::Group {
        return 255;
    }
    let pixel = resolve(sprite, sprite.pixel(base.id, frame, x, y)).a as u32;
    mul(pixel, sprite.mask_value(base.id, x, y) as u32)
}

/// Cuánto deja ver la máscara (y el recorte) de la capa en `(x, y)`: 0–255.
pub fn coverage(sprite: &Sprite, layer: &Layer, frame: usize, x: i32, y: i32) -> u32 {
    let mut a = sprite.mask_value(layer.id, x, y) as u32;
    if let Some(base) = sprite.clip_base(layer.id).and_then(|b| sprite.layer(b)) {
        a = mul(a, base_alpha(sprite, base, frame, x, y));
    }
    a
}

/// Nivel de máscara para pintar con un color: blanco muestra, negro (o transparente) oculta.
pub fn mask_level(c: Rgba) -> u8 {
    let luma = (c.r as u32 * 299 + c.g as u32 * 587 + c.b as u32 * 114) / 1000;
    ((luma * c.a as u32 + 127) / 255) as u8
}

/// Tiñe de rojo lo que oculta la máscara de `layer` (para editarla viéndola).
pub fn mask_overlay(sprite: &Sprite, layer: cs_core::LayerId, pixels: &mut [Rgba]) {
    let Some(mask) = sprite.layer(layer).and_then(|l| l.mask.as_ref()) else {
        return;
    };
    let w = sprite.width as i32;
    for (i, px) in pixels.iter_mut().enumerate() {
        let (x, y) = (i as i32 % w, i as i32 / w);
        let hidden = 255 - mask.get(x, y);
        if hidden > 0 {
            *px = blend_over(*px, Rgba::rgb(230, 40, 60), hidden / 2);
        }
    }
}

/// Copia del sprite con las máscaras y recortes aplicados a los píxeles (para formatos que no
/// los soportan). `None` si no hay nada que aplicar. Los cels afectados dejan de estar vinculados.
pub fn bake_masks(sprite: &Sprite) -> Option<Sprite> {
    let affected: Vec<&Layer> = sprite
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Image && is_masked(sprite, l))
        .collect();
    let has_masks = sprite.layers.iter().any(|l| l.mask.is_some() || l.clip);
    if !has_masks {
        return None;
    }
    let mut out = sprite.clone();
    let mut next = sprite.next_image_id().0;
    for layer in affected {
        for frame in 0..sprite.frames.len() {
            let Some(cel) = sprite.cel(layer.id, frame) else {
                continue;
            };
            let Some(image) = sprite.image(cel.image) else {
                continue;
            };
            let mut baked = Image::clone(image);
            for iy in 0..image.height() {
                for ix in 0..image.width() {
                    let (x, y) = (cel.position.0 + ix as i32, cel.position.1 + iy as i32);
                    let cov = coverage(sprite, layer, frame, x, y);
                    let value = image.get(ix, iy).expect("in bounds");
                    let new = match (value, sprite.color_mode) {
                        (PixelValue::Rgba(c), _) => PixelValue::Rgba(Rgba {
                            a: mul(c.a as u32, cov) as u8,
                            ..c
                        }),
                        (PixelValue::Index(_), mode) if cov < 128 => mode.transparent(),
                        (v, _) => v,
                    };
                    baked.set(ix, iy, new).expect("same mode");
                }
            }
            let id = ImageId(next);
            next += 1;
            out.images.insert(id, Arc::new(baked));
            out.cels
                .get_mut(&(layer.id, frame))
                .expect("cel exists")
                .image = id;
        }
    }
    for layer in &mut out.layers {
        layer.mask = None;
        layer.clip = false;
    }
    // Quita imágenes que ya no usa ningún cel.
    let used: std::collections::HashSet<ImageId> = out.cels.values().map(|c| c.image).collect();
    out.images.retain(|id, _| used.contains(id));
    Some(out)
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
        let masked = is_masked(sprite, layer);
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
                let mut alpha = mul(src.a as u32, opacity);
                if masked && alpha > 0 {
                    alpha = mul(alpha, coverage(sprite, layer, frame, x, y));
                }
                let alpha = alpha as u8;
                let i = (y * w + x) as usize;
                out[i] = blend(out[i], src, alpha, layer.blend);
            }
        }
    }
    out
}

/// Color compuesto de un solo píxel (lo que se ve en pantalla), sin renderizar todo el frame.
pub fn composite_pixel(sprite: &Sprite, frame: usize, x: i32, y: i32) -> Rgba {
    if x < 0 || y < 0 || x >= sprite.width as i32 || y >= sprite.height as i32 {
        return Rgba::TRANSPARENT;
    }
    let mut out = Rgba::TRANSPARENT;
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
        let (ix, iy) = (x - cel.position.0, y - cel.position.1);
        if ix < 0 || iy < 0 {
            continue;
        }
        let Some(value) = image.get(ix as u32, iy as u32) else {
            continue;
        };
        let src = resolve(sprite, value);
        let mut alpha = mul(src.a as u32, mul(layer_opacity, cel.opacity as u32));
        if is_masked(sprite, layer) {
            alpha = mul(alpha, coverage(sprite, layer, frame, x, y));
        }
        out = blend(out, src, alpha as u8, layer.blend);
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

/// Frames que lee `render_onion` para `frame` (recortados a `0..len`): los que hay que
/// materializar antes de llamarla.
pub fn onion_frames(onion: &OnionSkin, frame: usize, len: usize) -> std::ops::Range<usize> {
    let end = frame.saturating_add(onion.after).saturating_add(1).min(len);
    frame.saturating_sub(onion.before).min(end)..end
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

    #[test]
    fn onion_frames_cover_neighbours_within_bounds() {
        let onion = |before, after| OnionSkin {
            before,
            after,
            ..Default::default()
        };
        assert_eq!(onion_frames(&onion(1, 1), 3, 10), 2..5);
        assert_eq!(onion_frames(&onion(2, 3), 1, 10), 0..5);
        assert_eq!(onion_frames(&onion(2, 3), 8, 10), 6..10);
        assert_eq!(onion_frames(&onion(0, 0), 4, 10), 4..5);
        assert_eq!(onion_frames(&onion(usize::MAX, usize::MAX), 4, 10), 0..10);
    }
    use cs_commands::Command;
    use cs_core::*;

    fn dot(s: &mut Sprite, layer: u32, x: i32, y: i32, c: Rgba) {
        draw(s, layer, vec![(x, y, PixelValue::Rgba(c))]);
    }

    #[test]
    fn layer_masks_hide_and_can_be_disabled() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let red = Rgba::rgb(255, 0, 0);
        dot(&mut s, 1, 0, 0, red);
        dot(&mut s, 1, 1, 0, red);
        let mut mask = LayerMask::filled(2, 1, 255);
        mask.data = std::sync::Arc::new(vec![0, 128]);
        s.layers[0].mask = Some(mask);
        let out = render_frame(&s, 0);
        assert_eq!(out[0].a, 0);
        assert_eq!(out[1].a, 128);
        assert_eq!(composite_pixel(&s, 0, 1, 0), out[1]);
        s.layers[0].mask.as_mut().unwrap().enabled = false;
        assert_eq!(render_frame(&s, 0)[0], red);
    }

    #[test]
    fn group_masks_apply_to_children() {
        let mut s = Sprite::new(1, 1, ColorMode::Rgba);
        let mut group = Layer::new_image(LayerId(2), "g");
        group.kind = LayerKind::Group;
        group.mask = Some(LayerMask::filled(1, 1, 0));
        s.layers.insert(0, group);
        s.layers[1].parent = Some(LayerId(2));
        dot(&mut s, 1, 0, 0, Rgba::rgb(1, 2, 3));
        assert_eq!(render_frame(&s, 0)[0].a, 0);
    }

    #[test]
    fn clipping_follows_the_base_layer() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let mut shade = Layer::new_image(LayerId(2), "shade");
        shade.clip = true;
        s.layers.push(shade);
        dot(&mut s, 1, 0, 0, Rgba::rgb(255, 0, 0));
        draw(
            &mut s,
            2,
            vec![
                (0, 0, PixelValue::Rgba(Rgba::rgb(0, 0, 255))),
                (1, 0, PixelValue::Rgba(Rgba::rgb(0, 0, 255))),
            ],
        );
        let out = render_frame(&s, 0);
        assert_eq!(out[0], Rgba::rgb(0, 0, 255), "shade over the base");
        assert_eq!(out[1].a, 0, "no base pixel, no shade");
        s.layers[0].visible = false;
        assert_eq!(
            render_frame(&s, 0)[0].a,
            0,
            "hidden base hides clipped layers"
        );
    }

    #[test]
    fn baking_applies_masks_and_clipping() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        assert!(bake_masks(&s).is_none());
        let mut shade = Layer::new_image(LayerId(2), "shade");
        shade.clip = true;
        s.layers.push(shade);
        dot(&mut s, 1, 0, 0, Rgba::rgb(255, 0, 0));
        draw(
            &mut s,
            2,
            vec![
                (0, 0, PixelValue::Rgba(Rgba::rgb(0, 0, 255))),
                (1, 0, PixelValue::Rgba(Rgba::rgb(0, 0, 255))),
            ],
        );
        let mut mask = LayerMask::filled(2, 1, 255);
        mask.data = std::sync::Arc::new(vec![255, 0]);
        s.layers[0].mask = Some(mask);
        let baked = bake_masks(&s).unwrap();
        assert!(baked.layers.iter().all(|l| l.mask.is_none() && !l.clip));
        assert_eq!(render_frame(&baked, 0), render_frame(&s, 0));
    }

    #[test]
    fn mask_overlay_tints_hidden_areas() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        let mut mask = LayerMask::filled(2, 1, 255);
        mask.data = std::sync::Arc::new(vec![0, 255]);
        s.layers[0].mask = Some(mask);
        let mut px = vec![Rgba::TRANSPARENT; 2];
        mask_overlay(&s, LayerId(1), &mut px);
        assert!(px[0].a > 0 && px[0].r > px[0].b);
        assert_eq!(px[1], Rgba::TRANSPARENT);
    }

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
    fn composite_pixel_matches_full_render() {
        let mut s = Sprite::new(3, 2, ColorMode::Rgba);
        draw(
            &mut s,
            1,
            vec![
                (0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0))),
                (2, 1, PixelValue::Rgba(Rgba::rgb(0, 0, 255))),
            ],
        );
        Command::AddLayer {
            index: 1,
            layer: Layer::new_image(LayerId(2), "top"),
        }
        .apply(&mut s)
        .unwrap();
        draw(
            &mut s,
            2,
            vec![(0, 0, PixelValue::Rgba(Rgba::new(0, 255, 0, 128)))],
        );
        s.layer_mut(LayerId(2)).unwrap().blend = BlendMode::Screen;
        let full = render_frame(&s, 0);
        for y in 0..2 {
            for x in 0..3 {
                assert_eq!(composite_pixel(&s, 0, x, y), full[(y * 3 + x) as usize]);
            }
        }
        assert_eq!(composite_pixel(&s, 0, -1, 0), Rgba::TRANSPARENT);
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
