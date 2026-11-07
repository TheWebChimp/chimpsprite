//! Expand to Spritesheet… y Slice into Frames… del menú de los artboards: los frames de un sprite
//! acomodados en una hoja (otro sprite de un frame) y la inversa (ver
//! `cs_io::import::sprite_from_sheet_ordered`).

use std::sync::Arc;

use cs_core::{Cel, ColorMode, GridSettings, Image, ImageId, Palette, PixelBuffer, Rgba, Sprite};

use crate::export::Bitmap;

/// En qué orden se recorren las celdas de una hoja.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SheetOrder {
    /// De izquierda a derecha y después hacia abajo.
    #[default]
    ByRows,
    /// De arriba a abajo y después hacia la derecha.
    ByColumns,
}

impl SheetOrder {
    /// Columna y fila de la celda `i` en una hoja de `columns` × `rows`.
    pub fn cell(self, i: u32, columns: u32, rows: u32) -> (u32, u32) {
        match self {
            SheetOrder::ByRows => (i % columns.max(1), i / columns.max(1)),
            SheetOrder::ByColumns => (i / rows.max(1), i % rows.max(1)),
        }
    }
}

/// Columnas de una hoja casi cuadrada para `n` frames: `ceil(sqrt(n))` (al menos 1).
pub fn auto_columns(n: usize) -> u32 {
    let n = n.max(1) as u32;
    let mut c = (n as f64).sqrt() as u32;
    while c * c < n {
        c += 1;
    }
    while c > 1 && (c - 1) * (c - 1) >= n {
        c -= 1;
    }
    c.max(1)
}

/// Lo que falta del otro lado para que entren `n` frames con `fixed` columnas (o filas).
pub fn fit(n: usize, fixed: u32) -> u32 {
    (n.max(1) as u32).div_ceil(fixed.max(1))
}

/// Cómo se arma la hoja de Expand to Spritesheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpandSettings {
    pub columns: u32,
    pub rows: u32,
    /// Aire entre celdas.
    pub spacing: u32,
    /// Margen alrededor de toda la hoja.
    pub border: u32,
    pub order: SheetOrder,
}

impl ExpandSettings {
    /// Casi cuadrada (`auto_columns`), sin aire ni margen, por filas.
    pub fn auto(n: usize) -> Self {
        let columns = auto_columns(n);
        Self {
            columns,
            rows: fit(n, columns),
            spacing: 0,
            border: 0,
            order: SheetOrder::ByRows,
        }
    }

    /// Lo que mide la hoja con celdas de `cell`.
    pub fn sheet_size(&self, cell: (u32, u32)) -> (u32, u32) {
        let side = |n: u32, c: u32| 2 * self.border + n.max(1) * c + (n.max(1) - 1) * self.spacing;
        (side(self.columns, cell.0), side(self.rows, cell.1))
    }

    /// La esquina de la celda `i` en la hoja.
    pub fn cell_origin(&self, i: u32, cell: (u32, u32)) -> (u32, u32) {
        let (col, row) = self.order.cell(i, self.columns, self.rows);
        (
            self.border + col * (cell.0 + self.spacing),
            self.border + row * (cell.1 + self.spacing),
        )
    }

    /// Celdas que hay.
    pub fn capacity(&self) -> usize {
        (self.columns.max(1) * self.rows.max(1)) as usize
    }
}

/// La hoja con `frames` de `sprite` (ya materializado: sin instancias), aplanados como se ven, en
/// un sprite nuevo de un frame y una capa. Con la paleta de `sprite`; indexada si `sprite` lo es y
/// todos los colores están en la paleta. Sin aire, la grilla queda del tamaño de la celda. `None`
/// si no hay frames o no entran.
pub fn expand_frames(
    sprite: &Sprite,
    frames: &[usize],
    settings: ExpandSettings,
) -> Option<Sprite> {
    if frames.is_empty()
        || frames.len() > settings.capacity()
        || frames.iter().any(|&f| f >= sprite.frames.len())
    {
        return None;
    }
    let cell = (sprite.width, sprite.height);
    let (w, h) = settings.sheet_size(cell);
    let mut sheet = Bitmap::new(w, h);
    for (i, &f) in frames.iter().enumerate() {
        let (x, y) = settings.cell_origin(i as u32, cell);
        sheet.blit(&Bitmap::from_frame(sprite, f, 1), x, y);
    }
    let mut out = Sprite::new(w, h, ColorMode::Rgba);
    out.frames[0].duration_ms = sprite.frames[frames[0]].duration_ms;
    let layer = out.layers[0].id;
    if sheet.pixels.iter().any(|p| p.a > 0) {
        let image = Image::from_buffer(w, h, PixelBuffer::Rgba(sheet.pixels)).expect("sheet size");
        out.images.insert(ImageId(1), Arc::new(image));
        out.cels.insert(
            (layer, 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
    }
    if settings.spacing == 0 {
        out.grid = GridSettings {
            size: cell,
            offset: (settings.border as i32, settings.border as i32),
        };
    }
    adopt_palette(&mut out, &sprite.palette, sprite.color_mode);
    Some(out)
}

/// Le pone a `sprite` (RGBA, recién armado) la paleta y, si se puede, el modo de color de otro
/// sprite: en gris queda en gris; indexado solo si cada color opaco está tal cual en la paleta
/// (los transparentes van al índice transparente); si no, queda RGBA.
pub fn adopt_palette(sprite: &mut Sprite, palette: &Palette, mode: ColorMode) {
    sprite.palette = palette.clone();
    match mode {
        ColorMode::Rgba => {}
        ColorMode::Grayscale => sprite.color_mode = ColorMode::Grayscale,
        ColorMode::Indexed { transparent_index } => {
            let index = |c: Rgba| -> Option<u8> {
                if c.a == 0 {
                    return Some(transparent_index);
                }
                palette
                    .colors()
                    .iter()
                    .take(256) // los índices son de un byte
                    .enumerate()
                    .find(|&(i, &p)| p == c && i != transparent_index as usize)
                    .map(|(i, _)| i as u8)
            };
            let mut indexed = Vec::new();
            for (&id, image) in &sprite.images {
                let PixelBuffer::Rgba(pixels) = image.pixels() else {
                    return;
                };
                let Some(px) = pixels
                    .iter()
                    .map(|&c| index(c))
                    .collect::<Option<Vec<u8>>>()
                else {
                    return;
                };
                let image =
                    Image::from_buffer(image.width(), image.height(), PixelBuffer::Indexed(px))
                        .expect("same size");
                indexed.push((id, Arc::new(image)));
            }
            sprite.images.extend(indexed);
            sprite.color_mode = mode;
        }
    }
}

#[cfg(test)]
#[path = "spritesheet_tests.rs"]
mod tests;
