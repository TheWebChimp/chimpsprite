//! Operaciones sobre el sprite completo del menú Sprite: rotar y escalar el lienzo, cambiar el
//! modo de color y medir el contenido (para Trim). Devuelven el sprite nuevo; la app lo aplica con
//! `DocCommand::SetSprite` (un solo paso de undo).

use std::sync::Arc;

use cs_core::{
    ColorMode, Image, LayerKind, LayerMask, PixelBuffer, PixelValue, Rgba, SliceKey, Sprite,
};

/// Los giros del lienzo de "Rotate Canvas".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    Half,
    Clockwise,
    CounterClockwise,
}

/// Rotar o escalar mueven píxeles que no tienen: los objetos (muestran otro artboard) y los
/// tilemaps (sus tiles tienen un tamaño fijo) no se pueden transformar así.
pub fn can_transform(sprite: &Sprite) -> bool {
    sprite.tilesets.is_empty()
        && sprite
            .layers
            .iter()
            .all(|l| !matches!(l.kind, LayerKind::Objects | LayerKind::Tilemap))
}

/// Una transformación del lienzo: a dónde va un rectángulo y, para cada píxel de destino, de qué
/// píxel del origen sale.
trait Geometry {
    /// El rectángulo `(x, y, w, h)` del lienzo, transformado.
    fn rect(&self, r: (i32, i32, u32, u32)) -> (i32, i32, u32, u32);
    /// El píxel local del origen (de `from`) que va al píxel local `(u, v)` de `to`.
    fn source(
        &self,
        u: u32,
        v: u32,
        from: (i32, i32, u32, u32),
        to: (i32, i32, u32, u32),
    ) -> (u32, u32);
}

struct Rotate {
    rotation: Rotation,
    /// El tamaño del lienzo antes de rotar.
    canvas: (u32, u32),
}

impl Geometry for Rotate {
    fn rect(&self, (x, y, w, h): (i32, i32, u32, u32)) -> (i32, i32, u32, u32) {
        let (cw, ch) = (self.canvas.0 as i32, self.canvas.1 as i32);
        match self.rotation {
            Rotation::Half => (cw - x - w as i32, ch - y - h as i32, w, h),
            Rotation::Clockwise => (ch - y - h as i32, x, h, w),
            Rotation::CounterClockwise => (y, cw - x - w as i32, h, w),
        }
    }

    fn source(
        &self,
        u: u32,
        v: u32,
        (_, _, w, h): (i32, i32, u32, u32),
        _: (i32, i32, u32, u32),
    ) -> (u32, u32) {
        match self.rotation {
            Rotation::Half => (w - 1 - u, h - 1 - v),
            Rotation::Clockwise => (v, h - 1 - u),
            Rotation::CounterClockwise => (w - 1 - v, u),
        }
    }
}

struct Scale {
    sx: f64,
    sy: f64,
}

impl Geometry for Scale {
    fn rect(&self, (x, y, w, h): (i32, i32, u32, u32)) -> (i32, i32, u32, u32) {
        let scale = |a: i32, len: u32, s: f64| {
            let a0 = (a as f64 * s).round() as i32;
            let a1 = ((a + len as i32) as f64 * s).round() as i32;
            (a0, (a1 - a0).max(1) as u32)
        };
        let (nx, nw) = scale(x, w, self.sx);
        let (ny, nh) = scale(y, h, self.sy);
        (nx, ny, nw, nh)
    }

    fn source(
        &self,
        u: u32,
        v: u32,
        (x, y, w, h): (i32, i32, u32, u32),
        (nx, ny, _, _): (i32, i32, u32, u32),
    ) -> (u32, u32) {
        // El centro del píxel de destino, de vuelta en el lienzo de origen.
        let back = |n: i32, d: u32, s: f64, a: i32, len: u32| {
            let at = ((n as f64 + d as f64 + 0.5) / s).floor() as i32 - a;
            at.clamp(0, len as i32 - 1) as u32
        };
        (back(nx, u, self.sx, x, w), back(ny, v, self.sy, y, h))
    }
}

/// Los píxeles de `data` (de `from.2 × from.3`) llevados al rectángulo `to`.
fn remap<T: Copy>(
    geo: &dyn Geometry,
    data: &[T],
    from: (i32, i32, u32, u32),
    to: (i32, i32, u32, u32),
) -> Vec<T> {
    let mut out = Vec::with_capacity(to.2 as usize * to.3 as usize);
    for v in 0..to.3 {
        for u in 0..to.2 {
            let (su, sv) = geo.source(u, v, from, to);
            out.push(data[(sv * from.2 + su) as usize]);
        }
    }
    out
}

fn transform_image(geo: &dyn Geometry, image: &Image, at: (i32, i32)) -> (Image, (i32, i32)) {
    let from = (at.0, at.1, image.width(), image.height());
    let to = geo.rect(from);
    let pixels = match image.pixels() {
        PixelBuffer::Rgba(px) => PixelBuffer::Rgba(remap(geo, px, from, to)),
        PixelBuffer::Indexed(px) => PixelBuffer::Indexed(remap(geo, px, from, to)),
    };
    let image = Image::from_buffer(to.2, to.3, pixels).expect("remap fills the new size");
    (image, (to.0, to.1))
}

fn transform_mask(geo: &dyn Geometry, mask: &LayerMask) -> LayerMask {
    let from = (mask.position.0, mask.position.1, mask.width, mask.height);
    let to = geo.rect(from);
    LayerMask {
        position: (to.0, to.1),
        width: to.2,
        height: to.3,
        data: Arc::new(remap(geo, &mask.data, from, to)),
        ..mask.clone()
    }
}

fn transform_slice_key(
    geo: &dyn Geometry,
    key: &SliceKey,
    inner: &dyn Fn(u32, u32) -> Box<dyn Geometry>,
) -> SliceKey {
    let (_, _, w, h) = key.bounds;
    if w == 0 || h == 0 {
        return *key;
    }
    let bounds = geo.rect(key.bounds);
    // El centro y el pivote son relativos a los bordes del slice: se transforman como dentro de
    // un lienzo del tamaño del slice.
    let local = inner(w, h);
    let center = key.center.map(|c| local.rect(c));
    let pivot = key.pivot.map(|(px, py)| {
        let (cx, cy, _, _) = local.rect((px, py, 1, 1));
        (cx, cy)
    });
    SliceKey {
        bounds,
        center,
        pivot,
        ..*key
    }
}

/// Aplica `geo` a todo el sprite: imágenes (una vez cada una, aunque esté vinculada) con sus
/// cels, máscaras y slices. `canvas` es el tamaño nuevo.
fn transform(
    sprite: &Sprite,
    geo: &dyn Geometry,
    canvas: (u32, u32),
    inner: &dyn Fn(u32, u32) -> Box<dyn Geometry>,
) -> Sprite {
    let mut out = sprite.clone();
    out.width = canvas.0;
    out.height = canvas.1;
    for (id, image) in &sprite.images {
        // Una imagen vinculada tiene la misma posición en todos sus cels (como en Trim).
        let Some(cel) = sprite.cels.values().find(|c| c.image == *id) else {
            continue;
        };
        let (new, at) = transform_image(geo, image, cel.position);
        out.images.insert(*id, Arc::new(new));
        for cel in out.cels.values_mut().filter(|c| c.image == *id) {
            cel.position = at;
        }
    }
    for layer in &mut out.layers {
        if let Some(mask) = &layer.mask {
            layer.mask = Some(transform_mask(geo, mask));
        }
    }
    for slice in &mut out.slices {
        for key in &mut slice.keys {
            *key = transform_slice_key(geo, key, inner);
        }
    }
    out
}

/// El sprite girado; `None` si tiene instancias o tilemaps (ver [`can_transform`]).
pub fn rotate(sprite: &Sprite, rotation: Rotation) -> Option<Sprite> {
    if !can_transform(sprite) {
        return None;
    }
    let canvas = match rotation {
        Rotation::Half => (sprite.width, sprite.height),
        _ => (sprite.height, sprite.width),
    };
    let geo = Rotate {
        rotation,
        canvas: (sprite.width, sprite.height),
    };
    let inner = move |w, h| -> Box<dyn Geometry> {
        Box::new(Rotate {
            rotation,
            canvas: (w, h),
        })
    };
    Some(transform(sprite, &geo, canvas, &inner))
}

/// El sprite escalado a `width × height` (vecino más cercano, como en el pixel art); `None` si
/// el tamaño es 0, es el mismo o el sprite tiene instancias o tilemaps.
pub fn scale(sprite: &Sprite, width: u32, height: u32) -> Option<Sprite> {
    if width == 0 || height == 0 || (width, height) == (sprite.width, sprite.height) {
        return None;
    }
    if !can_transform(sprite) {
        return None;
    }
    let sx = width as f64 / sprite.width as f64;
    let sy = height as f64 / sprite.height as f64;
    let inner = move |_, _| -> Box<dyn Geometry> { Box::new(Scale { sx, sy }) };
    Some(transform(
        sprite,
        &Scale { sx, sy },
        (width, height),
        &inner,
    ))
}

/// Sprite → Color Mode: a qué modo pasar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Rgb,
    Grayscale,
    Indexed,
}

impl ColorTarget {
    pub const ALL: [ColorTarget; 3] = [
        ColorTarget::Rgb,
        ColorTarget::Grayscale,
        ColorTarget::Indexed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColorTarget::Rgb => "RGB Color",
            ColorTarget::Grayscale => "Grayscale",
            ColorTarget::Indexed => "Indexed",
        }
    }

    /// El de `mode`.
    pub fn of(mode: ColorMode) -> Self {
        match mode {
            ColorMode::Rgba => ColorTarget::Rgb,
            ColorMode::Grayscale => ColorTarget::Grayscale,
            ColorMode::Indexed { .. } => ColorTarget::Indexed,
        }
    }
}

/// El sprite en el modo `target`; `None` si ya está en él (o no se puede).
pub fn convert(sprite: &Sprite, target: ColorTarget) -> Option<Sprite> {
    match target {
        ColorTarget::Rgb => to_rgba(sprite),
        ColorTarget::Grayscale => to_grayscale(sprite),
        ColorTarget::Indexed => to_indexed(sprite),
    }
}

/// El sprite en escala de grises: cada color, su gris (también los de la paleta). Desde
/// indexado, primero sus colores. `None` si ya está en grises.
pub fn to_grayscale(sprite: &Sprite) -> Option<Sprite> {
    let rgba = match sprite.color_mode {
        ColorMode::Grayscale => return None,
        ColorMode::Indexed { .. } => to_rgba(sprite)?,
        ColorMode::Rgba => sprite.clone(),
    };
    let convert = |image: &Image| -> Image {
        let PixelBuffer::Rgba(px) = image.pixels() else {
            return image.clone();
        };
        let grays = px.iter().map(|c| c.to_gray()).collect();
        Image::from_buffer(image.width(), image.height(), PixelBuffer::Rgba(grays))
            .expect("same size")
    };
    let mut out = convert_images(&rgba, ColorMode::Grayscale, &convert);
    out.palette =
        cs_core::Palette::new(rgba.palette.colors().iter().map(|c| c.to_gray()).collect());
    Some(out)
}

/// El sprite en RGBA: cada índice, su color de la paleta (el transparente, transparente). Desde
/// escala de grises, los mismos píxeles. `None` si ya es RGBA.
pub fn to_rgba(sprite: &Sprite) -> Option<Sprite> {
    let transparent_index = match sprite.color_mode {
        ColorMode::Rgba => return None,
        ColorMode::Grayscale => {
            let mut out = sprite.clone();
            out.color_mode = ColorMode::Rgba;
            return Some(out);
        }
        ColorMode::Indexed { transparent_index } => transparent_index,
    };
    let convert = |image: &Image| -> Image {
        let PixelBuffer::Indexed(px) = image.pixels() else {
            return image.clone();
        };
        let colors = px
            .iter()
            .map(|&i| {
                if i == transparent_index {
                    Rgba::TRANSPARENT
                } else {
                    sprite.palette.get(i).unwrap_or(Rgba::TRANSPARENT)
                }
            })
            .collect();
        Image::from_buffer(image.width(), image.height(), PixelBuffer::Rgba(colors))
            .expect("same size")
    };
    Some(convert_images(sprite, ColorMode::Rgba, &convert))
}

/// El sprite indexado con su paleta: cada color, el más cercano de la paleta (como Aseprite, los
/// píxeles con alfa < 128 quedan transparentes). El índice transparente es el primer color
/// transparente de la paleta o, si no tiene, uno nuevo al principio (si hay lugar; si no, el 0).
/// `None` si ya es indexado o la paleta está vacía.
pub fn to_indexed(sprite: &Sprite) -> Option<Sprite> {
    if !sprite.color_mode.stores_rgba() || sprite.palette.is_empty() {
        return None;
    }
    let mut palette = sprite.palette.clone();
    let transparent_index = match palette.colors().iter().position(|c| c.a == 0) {
        Some(i) if i < 256 => i as u8,
        _ if palette.len() < 256 => {
            let mut colors = vec![Rgba::TRANSPARENT];
            colors.extend_from_slice(palette.colors());
            palette = cs_core::Palette::new(colors);
            0
        }
        _ => 0,
    };
    let convert = |image: &Image| -> Image {
        let PixelBuffer::Rgba(px) = image.pixels() else {
            return image.clone();
        };
        let indices = px
            .iter()
            .map(|&c| palette.nearest(c, transparent_index))
            .collect();
        Image::from_buffer(image.width(), image.height(), PixelBuffer::Indexed(indices))
            .expect("same size")
    };
    let mut out = convert_images(sprite, ColorMode::Indexed { transparent_index }, &convert);
    out.palette = palette.clone();
    Some(out)
}

fn convert_images(sprite: &Sprite, mode: ColorMode, convert: &dyn Fn(&Image) -> Image) -> Sprite {
    let mut out = sprite.clone();
    out.color_mode = mode;
    for image in out.images.values_mut() {
        *image = Arc::new(convert(image));
    }
    for tileset in &mut out.tilesets {
        for tile in &mut tileset.tiles {
            *tile = Arc::new(convert(tile));
        }
    }
    out
}

/// El rectángulo `(x, y, w, h)` del lienzo con píxeles visibles en algún cel (lo que deja Trim);
/// `None` si no hay ninguno.
pub fn content_bounds(sprite: &Sprite) -> Option<(i32, i32, u32, u32)> {
    let transparent = sprite.color_mode.transparent();
    let visible = |v: PixelValue| match v {
        PixelValue::Rgba(c) => c.a > 0,
        PixelValue::Index(_) => v != transparent,
    };
    let (cw, ch) = (sprite.width as i32, sprite.height as i32);
    let mut bounds: Option<(i32, i32, i32, i32)> = None;
    for cel in sprite.cels.values() {
        let Some(image) = sprite.images.get(&cel.image) else {
            continue;
        };
        let (px, py) = cel.position;
        for v in 0..image.height() {
            let y = py + v as i32;
            if !(0..ch).contains(&y) {
                continue;
            }
            for u in 0..image.width() {
                let x = px + u as i32;
                if !(0..cw).contains(&x) || !image.get(u, v).is_some_and(visible) {
                    continue;
                }
                bounds = Some(match bounds {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    bounds.map(|(x0, y0, x1, y1)| (x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
}

#[cfg(test)]
mod tests {

    #[test]
    fn grayscale_turns_every_color_into_its_gray_and_back_to_rgb_keeps_them() {
        let mut s = Sprite::new(2, 1, ColorMode::Rgba);
        crate::Command::DrawPixels {
            layer: cs_core::LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(0, 0, 255)))],
        }
        .apply(&mut s)
        .unwrap();
        let gray = to_grayscale(&s).expect("converted");
        assert_eq!(gray.color_mode, ColorMode::Grayscale);
        assert_eq!(
            gray.pixel(cs_core::LayerId(1), 0, 0, 0),
            PixelValue::Rgba(Rgba::rgb(29, 29, 29))
        );
        assert!(
            gray.palette
                .colors()
                .iter()
                .all(|c| c.r == c.g && c.g == c.b)
        );
        assert!(to_grayscale(&gray).is_none(), "already");
        let rgb = to_rgba(&gray).expect("back");
        assert_eq!(rgb.color_mode, ColorMode::Rgba);
        assert_eq!(
            rgb.pixel(cs_core::LayerId(1), 0, 0, 0),
            gray.pixel(cs_core::LayerId(1), 0, 0, 0)
        );
        // Y a indexado, como desde RGB.
        assert!(to_indexed(&gray).is_some());
    }

    use super::*;
    use cs_core::{LayerId, Palette};

    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    fn draw(s: &mut Sprite, points: &[(i32, i32)]) {
        crate::Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: points.iter().map(|&(x, y)| (x, y, red())).collect(),
        }
        .apply(s)
        .unwrap();
    }

    /// 4×2 con píxeles rojos en (0, 0), (1, 0) y (3, 1).
    fn sprite() -> Sprite {
        let mut s = Sprite::new(4, 2, ColorMode::Rgba);
        draw(&mut s, &[(0, 0), (1, 0), (3, 1)]);
        s
    }

    fn reds(s: &Sprite) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in 0..s.height as i32 {
            for x in 0..s.width as i32 {
                if s.pixel(LayerId(1), 0, x, y) == red() {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn rotations_move_pixels_and_swap_the_size() {
        let s = sprite();
        let cw = rotate(&s, Rotation::Clockwise).unwrap();
        assert_eq!((cw.width, cw.height), (2, 4));
        assert_eq!(reds(&cw), vec![(1, 0), (1, 1), (0, 3)]);
        let ccw = rotate(&s, Rotation::CounterClockwise).unwrap();
        assert_eq!(reds(&ccw), vec![(1, 0), (0, 2), (0, 3)]);
        let half = rotate(&s, Rotation::Half).unwrap();
        assert_eq!((half.width, half.height), (4, 2));
        assert_eq!(reds(&half), vec![(0, 0), (2, 1), (3, 1)]);
        let back = rotate(&cw, Rotation::CounterClockwise).unwrap();
        assert_eq!(reds(&back), reds(&s));
    }

    #[test]
    fn scaling_is_nearest_neighbour() {
        let s = sprite();
        let big = scale(&s, 8, 4).unwrap();
        assert_eq!((big.width, big.height), (8, 4));
        assert_eq!(
            reds(&big),
            vec![
                (0, 0),
                (1, 0),
                (2, 0),
                (3, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (3, 1),
                (6, 2),
                (7, 2),
                (6, 3),
                (7, 3)
            ]
        );
        assert_eq!(scale(&s, 4, 2), None, "same size");
        let small = scale(&big, 4, 2).unwrap();
        assert_eq!(reds(&small), reds(&s));
    }

    #[test]
    fn color_modes_round_trip_through_the_palette() {
        let mut s = sprite();
        s.palette = Palette::new(vec![Rgba::rgb(0, 0, 0), Rgba::rgb(250, 0, 0)]);
        let indexed = to_indexed(&s).unwrap();
        assert_eq!(
            indexed.color_mode,
            ColorMode::Indexed {
                transparent_index: 0
            },
            "a transparent entry is added first"
        );
        assert_eq!(indexed.palette.len(), 3);
        assert_eq!(indexed.pixel(LayerId(1), 0, 0, 0), PixelValue::Index(2));
        assert_eq!(indexed.pixel(LayerId(1), 0, 2, 0), PixelValue::Index(0));
        assert_eq!(to_indexed(&indexed), None);
        let rgba = to_rgba(&indexed).unwrap();
        assert_eq!(
            rgba.pixel(LayerId(1), 0, 0, 0),
            PixelValue::Rgba(Rgba::rgb(250, 0, 0))
        );
        assert_eq!(
            rgba.pixel(LayerId(1), 0, 2, 0),
            PixelValue::Rgba(Rgba::TRANSPARENT)
        );
    }

    #[test]
    fn content_bounds_cover_visible_pixels_inside_the_canvas() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        assert_eq!(content_bounds(&s), None);
        draw(&mut s, &[(2, 3), (5, 4)]);
        assert_eq!(content_bounds(&s), Some((2, 3, 4, 2)));
    }
}
