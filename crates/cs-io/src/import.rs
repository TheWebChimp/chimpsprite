//! Importación de PNG: como sprite nuevo o como capa nueva.

use std::io::Cursor;
use std::sync::Arc;

use cs_commands::{Command, ops};
use cs_core::{Cel, ColorMode, Image, ImageId, LayerKind, PixelBuffer, PixelValue, Rgba, Sprite};

use crate::export::Bitmap;
use crate::{IoError, Result};

/// Tope de memoria para decodificar (protege contra PNG maliciosos).
const MAX_DECODE_BYTES: usize = 512 * 1024 * 1024;

/// Decodifica cualquier PNG (paleta, gris, RGB, RGBA, 16 bits) a RGBA de 8 bits.
pub fn decode_png(bytes: &[u8]) -> Result<Bitmap> {
    let png_error = |e: png::DecodingError| IoError::Png(e.to_string());
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: MAX_DECODE_BYTES,
        },
    );
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(png_error)?;
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| IoError::Png("image too large".into()))?;
    let mut buf = vec![0; size];
    let info = reader.next_frame(&mut buf).map_err(png_error)?;
    let data = &buf[..info.buffer_size()];
    let pixels: Vec<Rgba> = match info.color_type {
        png::ColorType::Rgba => data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| Rgba::new(p[0], p[1], p[2], p[3]))
            .collect(),
        png::ColorType::Rgb => data
            .as_chunks::<3>()
            .0
            .iter()
            .map(|p| Rgba::rgb(p[0], p[1], p[2]))
            .collect(),
        png::ColorType::GrayscaleAlpha => data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| Rgba::new(p[0], p[0], p[0], p[1]))
            .collect(),
        png::ColorType::Grayscale => data.iter().map(|&v| Rgba::rgb(v, v, v)).collect(),
        png::ColorType::Indexed => {
            return Err(IoError::Png("unexpected indexed output".into()));
        }
    };
    Ok(Bitmap {
        width: info.width,
        height: info.height,
        pixels,
    })
}

/// Color de la paleta más cercano (distancia euclidiana RGB); transparente si alpha < 128.
pub fn nearest_index(sprite: &Sprite, c: Rgba, transparent_index: u8) -> u8 {
    sprite.palette.nearest(c, transparent_index)
}

fn to_image(sprite: &Sprite, bitmap: &Bitmap) -> Image {
    let pixels = match sprite.color_mode {
        ColorMode::Rgba | ColorMode::Grayscale => PixelBuffer::Rgba(bitmap.pixels.clone()),
        ColorMode::Indexed { transparent_index } => PixelBuffer::Indexed(
            bitmap
                .pixels
                .iter()
                .map(|&c| nearest_index(sprite, c, transparent_index))
                .collect(),
        ),
    };
    Image::from_buffer(bitmap.width, bitmap.height, pixels).expect("bitmap size matches")
}

/// Sprite RGBA nuevo del tamaño del PNG, con la imagen en su única capa.
pub fn import_png(bytes: &[u8]) -> Result<Sprite> {
    let bitmap = decode_png(bytes)?;
    let mut sprite = Sprite::new(bitmap.width, bitmap.height, ColorMode::Rgba);
    let image = to_image(&sprite, &bitmap);
    sprite.images.insert(ImageId(1), Arc::new(image));
    let layer = sprite.layers[0].id;
    sprite.cels.insert(
        (layer, 0),
        Cel {
            position: (0, 0),
            opacity: 255,
            image: ImageId(1),
        },
    );
    Ok(sprite)
}

/// Cómo se parte una hoja de sprites: el tamaño de cada frame, el aire entre celdas y el margen
/// de la esquina de arriba a la izquierda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SheetGrid {
    pub cell: (u32, u32),
    pub padding: (u32, u32),
    pub offset: (u32, u32),
}

impl SheetGrid {
    /// Columnas y filas enteras que entran en una hoja de `size`.
    pub fn count(&self, size: (u32, u32)) -> (u32, u32) {
        let fit = |total: u32, offset: u32, cell: u32, pad: u32| {
            let cell = cell.max(1);
            let room = total.saturating_sub(offset);
            if room < cell {
                0
            } else {
                (room - cell) / (cell + pad) + 1
            }
        };
        (
            fit(size.0, self.offset.0, self.cell.0, self.padding.0),
            fit(size.1, self.offset.1, self.cell.1, self.padding.1),
        )
    }
}

/// File → Import Sprite Sheet: un sprite RGBA del tamaño de una celda con un frame por celda,
/// por filas (de izquierda a derecha y de arriba a abajo). Las celdas vacías del final no se
/// agregan. `None` si no entra ninguna celda.
pub fn sprite_from_sheet(sheet: &Bitmap, grid: SheetGrid) -> Option<Sprite> {
    let (cols, rows) = grid.count((sheet.width, sheet.height));
    let (w, h) = (grid.cell.0.max(1), grid.cell.1.max(1));
    if cols == 0 || rows == 0 {
        return None;
    }
    let cell = |col: u32, row: u32| -> Vec<Rgba> {
        let x0 = grid.offset.0 + col * (w + grid.padding.0);
        let y0 = grid.offset.1 + row * (h + grid.padding.1);
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x0 + x, y0 + y)))
            .map(|(x, y)| sheet.pixels[(y * sheet.width + x) as usize])
            .collect()
    };
    let mut cells: Vec<Vec<Rgba>> = (0..rows)
        .flat_map(|row| (0..cols).map(move |col| (col, row)))
        .map(|(col, row)| cell(col, row))
        .collect();
    while cells.len() > 1 && cells.last().is_some_and(|c| c.iter().all(|p| p.a == 0)) {
        cells.pop();
    }
    let mut sprite = Sprite::new(w, h, ColorMode::Rgba);
    let duration = sprite.frames[0].duration_ms;
    sprite.frames = vec![
        cs_core::Frame {
            duration_ms: duration
        };
        cells.len()
    ];
    let layer = sprite.layers[0].id;
    for (frame, pixels) in cells.into_iter().enumerate() {
        if pixels.iter().all(|p| p.a == 0) {
            continue;
        }
        let id = ImageId(frame as u32 + 1);
        let image = Image::from_buffer(w, h, PixelBuffer::Rgba(pixels)).expect("cell size");
        sprite.images.insert(id, Arc::new(image));
        sprite.cels.insert(
            (layer, frame),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }
    Some(sprite)
}

/// Comando que agrega el PNG como capa nueva (arriba de todo) en `frame`. En sprites indexados
/// cada color se aproxima al más cercano de la paleta.
pub fn import_png_layer(
    sprite: &Sprite,
    bytes: &[u8],
    frame: usize,
    name: &str,
) -> Result<Command> {
    let bitmap = decode_png(bytes)?;
    let mut add = ops::new_layer(sprite, None, LayerKind::Image);
    let Command::AddLayer { layer, .. } = &mut add else {
        unreachable!("new_layer devuelve AddLayer");
    };
    layer.name = name.to_string();
    let layer_id = layer.id;
    let image_id = sprite.next_image_id();
    Ok(Command::Batch(vec![
        add,
        Command::SetImage {
            id: image_id,
            image: Some(Arc::new(to_image(sprite, &bitmap))),
        },
        Command::SetCel {
            layer: layer_id,
            frame,
            cel: Some(Cel {
                position: (0, 0),
                opacity: 255,
                image: image_id,
            }),
        },
    ]))
}

/// Para tests y la UI: valor de píxel de un sprite RGBA importado.
pub fn pixel_rgba(sprite: &Sprite, x: i32, y: i32) -> Rgba {
    match sprite.pixel(sprite.layers[0].id, 0, x, y) {
        PixelValue::Rgba(c) => c,
        PixelValue::Index(i) => sprite.palette.get(i).unwrap_or(Rgba::TRANSPARENT),
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_sheet_splits_into_one_frame_per_cell_by_rows() {
        // 3 celdas de 2×1 en una fila con 1 de aire, la del medio vacía, y una de más vacía.
        let red = Rgba::rgb(255, 0, 0);
        let blue = Rgba::rgb(0, 0, 255);
        let t = Rgba::TRANSPARENT;
        let sheet = Bitmap {
            width: 11,
            height: 1,
            pixels: vec![red, red, t, t, t, t, blue, blue, t, t, t],
        };
        let grid = SheetGrid {
            cell: (2, 1),
            padding: (1, 0),
            offset: (0, 0),
        };
        assert_eq!(grid.count((11, 1)), (4, 1));
        let s = sprite_from_sheet(&sheet, grid).expect("sprite");
        assert_eq!((s.width, s.height), (2, 1));
        assert_eq!(s.frames.len(), 3, "the empty one at the end is dropped");
        let at = |f: usize| s.pixel(s.layers[0].id, f, 1, 0);
        assert_eq!(at(0), PixelValue::Rgba(red));
        assert_eq!(at(1), PixelValue::Rgba(t), "an empty cell in between stays");
        assert_eq!(at(2), PixelValue::Rgba(blue));
    }

    use cs_core::*;

    use super::*;
    use crate::export::export_png;

    fn checker_png() -> Vec<u8> {
        let mut s = Sprite::new(3, 2, ColorMode::Rgba);
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![
                (0, 0, PixelValue::Rgba(Rgba::rgb(250, 10, 10))),
                (2, 1, PixelValue::Rgba(Rgba::new(10, 10, 250, 255))),
            ],
        }
        .apply(&mut s)
        .unwrap();
        export_png(&s, 0, 1).unwrap()
    }

    #[test]
    fn import_as_new_sprite() {
        let s = import_png(&checker_png()).unwrap();
        assert_eq!((s.width, s.height), (3, 2));
        assert_eq!(pixel_rgba(&s, 0, 0), Rgba::rgb(250, 10, 10));
        assert_eq!(pixel_rgba(&s, 1, 0), Rgba::TRANSPARENT);
    }

    #[test]
    fn import_as_layer_maps_to_palette_in_indexed_sprites() {
        let mut s = Sprite::new(
            3,
            2,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        let before = s.clone();
        let inv = import_png_layer(&s, &checker_png(), 0, "ref")
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let top = s.layers.last().unwrap();
        assert_eq!(top.name, "ref");
        // DB32: #ac3232 (27) es el rojo más cercano; #5b6ee1 (17) el azul.
        assert_eq!(s.pixel(top.id, 0, 0, 0), PixelValue::Index(27));
        assert_eq!(s.pixel(top.id, 0, 1, 0), PixelValue::Index(0));
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn decodes_paletted_png() {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, 2, 1);
            e.set_color(png::ColorType::Indexed);
            e.set_depth(png::BitDepth::Eight);
            e.set_palette(vec![0, 0, 0, 255, 200, 0]);
            e.set_trns(vec![0, 255]);
            e.write_header().unwrap().write_image_data(&[0, 1]).unwrap();
        }
        let b = decode_png(&out).unwrap();
        assert_eq!(b.pixels, vec![Rgba::TRANSPARENT, Rgba::rgb(255, 200, 0)]);
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(decode_png(b"definitely not a png").is_err());
    }
}
