//! Generar una variante de una fuente pixel (como *Element → Style* de FontForge): Bold (cada
//! pixel también 1 a la derecha), Italic (las filas de arriba de la baseline corridas a la
//! derecha), Outline (el contorno con la matriz del filtro Outline), Shadow y Copy. Cada glifo
//! transforma sus pixeles propios: los compuestos siguen siendo compuestos.

use std::collections::BTreeMap;
use std::sync::Arc;

use cs_core::{
    Cel, ColorMode, FontInfo, FontMode, Image, ONE_BIT_INK, PixelValue, Sprite, VariantKind,
    VariantRecipe, font_sprite,
};

use crate::outline::{OutlineMatrix, OutlinePlace, OutlineShape, outline_mask};

/// El lado máximo de la celda de una variante.
pub const MAX_CELL: u32 = 256;

/// Pixeles por (columna, fila); pueden quedar fuera de la celda mientras se transforman.
type Pixels = BTreeMap<(i32, i32), PixelValue>;

/// Si `value` es un pixel pintado con el modo `mode`.
fn painted(mode: ColorMode, value: PixelValue) -> bool {
    match value {
        PixelValue::Index(i) => mode.transparent() != PixelValue::Index(i),
        PixelValue::Rgba(c) => c.a > 0,
    }
}

/// Los pixeles propios (los del cel) del glifo `frame`.
fn own_pixels(sprite: &Sprite, frame: usize) -> Pixels {
    let Some(layer) = sprite.layers.first().map(|l| l.id) else {
        return Pixels::new();
    };
    let mut out = Pixels::new();
    for y in 0..sprite.height as i32 {
        for x in 0..sprite.width as i32 {
            let v = sprite.pixel(layer, frame, x, y);
            if painted(sprite.color_mode, v) {
                out.insert((x, y), v);
            }
        }
    }
    out
}

/// Lo que se corre a la derecha la fila `y` en una itálica de `slant` con la baseline en
/// `baseline`: 1 columna cada `slant` filas desde la de arriba de la baseline; los descendentes
/// no se mueven.
pub fn italic_shift(y: i32, baseline: u32, slant: u32) -> i32 {
    let rows_up = baseline as i32 - 1 - y;
    if rows_up <= 0 || slant == 0 {
        0
    } else {
        rows_up / slant as i32
    }
}

fn bold(p: &Pixels) -> Pixels {
    let mut out = p.clone();
    for (&(x, y), &v) in p {
        out.entry((x + 1, y)).or_insert(v);
    }
    out
}

fn italic(p: &Pixels, baseline: u32, slant: u32) -> Pixels {
    p.iter()
        .map(|(&(x, y), &v)| ((x + italic_shift(y, baseline, slant), y), v))
        .collect()
}

/// El contorno de afuera de `p` (los vacíos que tocan lo pintado según `matrix`).
fn ring(p: &Pixels, matrix: OutlineMatrix) -> Vec<(i32, i32)> {
    let Some(x0) = p.keys().map(|k| k.0).min() else {
        return Vec::new();
    };
    let x1 = p.keys().map(|k| k.0).max().unwrap_or(x0);
    let y0 = p.keys().map(|k| k.1).min().unwrap_or(0);
    let y1 = p.keys().map(|k| k.1).max().unwrap_or(y0);
    // Una caja con 1 de aire alrededor: ahí cabe todo el contorno.
    let (ox, oy) = (x0 - 1, y0 - 1);
    let (w, h) = ((x1 - x0 + 3) as u32, (y1 - y0 + 3) as u32);
    let shape = OutlineShape {
        matrix,
        place: OutlinePlace::Outside,
        wrap_x: false,
        wrap_y: false,
    };
    let mask = outline_mask(w, h, |x, y| !p.contains_key(&(x + ox, y + oy)), &shape);
    mask.iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(i, _)| ((i as u32 % w) as i32 + ox, (i as u32 / w) as i32 + oy))
        .collect()
}

/// Los pixeles transformados de un glifo (sus pixeles propios `p`).
fn transform(p: &Pixels, recipe: &VariantRecipe, mode: FontMode, baseline: u32) -> Pixels {
    let color = PixelValue::Rgba(recipe.color);
    let matrix = if recipe.square {
        OutlineMatrix::SQUARE
    } else {
        OutlineMatrix::CIRCLE
    };
    match recipe.kind {
        VariantKind::Copy => p.clone(),
        VariantKind::Bold => bold(p),
        VariantKind::Italic => italic(p, baseline, recipe.slant),
        VariantKind::BoldItalic => italic(&bold(p), baseline, recipe.slant),
        VariantKind::Outline => {
            let ring = ring(p, matrix);
            match mode {
                // Hueca: solo el contorno.
                FontMode::OneBit => ring.into_iter().map(|k| (k, ONE_BIT_INK)).collect(),
                FontMode::Color => {
                    let mut out = p.clone();
                    out.extend(ring.into_iter().map(|k| (k, color)));
                    out
                }
            }
        }
        VariantKind::Shadow => match mode {
            FontMode::Color => {
                let mut out = p.clone();
                for &(x, y) in p.keys() {
                    out.entry((x + 1, y + 1)).or_insert(color);
                }
                out
            }
            // Hueca con el borde de abajo y la derecha doble: (contorno ∪ contorno corrido) −
            // la letra, con el contorno de 4 vecinos y la letra.
            FontMode::OneBit => {
                let mut grown: Vec<(i32, i32)> = ring(p, OutlineMatrix::CIRCLE);
                grown.extend(p.keys().copied());
                let mut out = Pixels::new();
                for &(x, y) in &grown {
                    for k in [(x, y), (x + 1, y + 1)] {
                        if !p.contains_key(&k) {
                            out.insert(k, ONE_BIT_INK);
                        }
                    }
                }
                out
            }
        },
    }
}

/// Lo que crece una letra a la derecha (sin contar lo que se corre todo).
fn right_growth(kind: VariantKind, mode: FontMode) -> u32 {
    match (kind, mode) {
        (VariantKind::Shadow, FontMode::OneBit) => 2,
        (VariantKind::Bold | VariantKind::BoldItalic | VariantKind::Outline, _) => 1,
        (VariantKind::Shadow, FontMode::Color) => 1,
        (VariantKind::Italic | VariantKind::Copy, _) => 0,
    }
}

/// La variante de `recipe` de la fuente `font` (con su `sprite`): la misma familia, el mismo
/// estilo (lo pone quien llama), el orden de los glifos, el kerning, el espaciado, el espacio,
/// Monospace y la letra fantasma. Si algo queda a la izquierda o arriba de la celda, todo se
/// corre (la baseline también); la celda crece lo que haga falta (hasta [`MAX_CELL`]). Los anchos
/// manuales crecen con la letra; en las itálicas, los componentes se ajustan a su altura.
pub fn generate_variant(
    font: &FontInfo,
    sprite: &Sprite,
    recipe: &VariantRecipe,
) -> (FontInfo, Sprite) {
    let baseline = font.metrics.baseline;
    let glyphs: Vec<Pixels> = (0..font.glyphs.len())
        .map(|i| transform(&own_pixels(sprite, i), recipe, font.mode, baseline))
        .collect();
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    let all = glyphs.iter().flat_map(|g| g.keys().copied());
    let (min_x, min_y, max_x, max_y) = all.fold((0, 0, w, h), |b, (x, y)| {
        (b.0.min(x), b.1.min(y), b.2.max(x + 1), b.3.max(y + 1))
    });
    let shift = (-min_x, -min_y);
    let cell = (
        ((max_x + shift.0) as u32).clamp(1, MAX_CELL),
        ((max_y + shift.1) as u32).clamp(1, MAX_CELL),
    );
    let mut out = font_sprite(cell, font.mode, font.glyphs.len());
    let layer = out.layers[0].id;
    for (frame, pixels) in glyphs.iter().enumerate() {
        if pixels.is_empty() {
            continue;
        }
        let mut image = Image::new(cell.0, cell.1, out.color_mode);
        let mut any = false;
        for (&(x, y), &v) in pixels {
            let (x, y) = (x + shift.0, y + shift.1);
            if x >= 0 && y >= 0 && image.set(x as u32, y as u32, v).is_ok() {
                any = true;
            }
        }
        if !any {
            continue;
        }
        let id = out.next_image_id();
        out.images.insert(id, Arc::new(image));
        out.cels.insert(
            (layer, frame),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }
    let mut info = font.clone();
    let grow = right_growth(recipe.kind, font.mode) + shift.0 as u32;
    let italic = recipe.kind.italic();
    // En una itálica, cada componente se corre lo que se corre su altura: la fila del medio de
    // la tinta de su glifo, bajada su offset.
    let centers: Vec<Option<i32>> = (0..font.glyphs.len())
        .map(|i| font.ink_bounds(sprite, i).map(|b| (b.y0 + b.y1 - 1) / 2))
        .collect();
    for g in &mut info.glyphs {
        if let Some(advance) = &mut g.advance {
            *advance += grow;
        }
        if !italic {
            continue;
        }
        for c in &mut g.components {
            let Some(center) = font.index_of(c.ch).and_then(|j| centers[j]) else {
                continue;
            };
            let at = |y| italic_shift(y, baseline, recipe.slant);
            c.offset.0 += at(center + c.offset.1) - at(center);
        }
    }
    let m = font.metrics;
    let below = cell.1 as i32 - shift.1 - h;
    info.metrics = cs_core::FontMetrics {
        baseline: baseline + shift.1 as u32,
        descent: (m.descent as i32 + below.max(0)) as u32,
        ..m
    }
    .clamped(cell.1);
    (info, out)
}

#[cfg(test)]
#[path = "font_style_tests.rs"]
mod tests;
