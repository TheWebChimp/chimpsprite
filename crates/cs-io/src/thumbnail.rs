//! La miniatura de un `.chimp`: un `thumbnail.png` en la raíz del zip para los exploradores de
//! archivos (Windows, Finder, Linux). Ver `docs/superpowers/specs/2026-11-06-chimp-thumbnails-design.md`.

use std::io::{Read, Seek};

use cs_core::{ArtboardId, Document};

use crate::export::Bitmap;
use crate::{IoError, Result, chimp, import};

/// Nombre de la entrada dentro del zip.
pub const ENTRY: &str = "thumbnail.png";
/// Lado mayor de la miniatura que se guarda.
pub const SIZE: u32 = 256;
/// Topes al leer (el que la lee es el proceso de miniaturas del sistema, con archivos de
/// cualquier lado): bytes del PNG y lado de la imagen.
const MAX_PNG_BYTES: u64 = 8 * 1024 * 1024;
const MAX_SIDE: u32 = 2048;

/// Copia que cabe en `size × size` con vecino más cercano: si es más chica, ampliada por el
/// mayor factor entero que cabe; si es más grande, reducida.
pub fn fit(image: &Bitmap, size: u32) -> Bitmap {
    let size = size.max(1);
    let longest = image.width.max(image.height);
    if longest == 0 {
        return image.clone();
    }
    if longest <= size {
        return image.scaled(size / longest);
    }
    let w = (image.width as u64 * size as u64 / longest as u64).max(1) as u32;
    let h = (image.height as u64 * size as u64 / longest as u64).max(1) as u32;
    let pixels = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let sx = (x as u64 * image.width as u64 / w as u64) as u32;
            let sy = (y as u64 * image.height as u64 / h as u64) as u32;
            image.pixels[(sy * image.width + sx) as usize]
        })
        .collect();
    Bitmap {
        width: w,
        height: h,
        pixels,
    }
}

/// El artboard de la miniatura: `active` si existe y no es una fuente (su frame 1 es un glifo
/// suelto); si no, el primero del lienzo, o el primero del documento.
pub fn pick(doc: &Document, active: Option<ArtboardId>) -> Option<ArtboardId> {
    active
        .and_then(|id| doc.artboard(id))
        .filter(|a| a.font.is_none())
        .or_else(|| doc.canvas_artboards().next())
        .or_else(|| doc.artboards.first())
        .map(|a| a.id)
}

/// El frame 1 del artboard como se ve (instancias, tilemaps y máscaras; sin las capas de
/// referencia), ajustado a `SIZE`. `None` si no existe o si es todo transparente.
pub fn render(doc: &Document, id: ArtboardId) -> Option<Bitmap> {
    let sprite = &doc.artboard(id)?.sprite;
    if sprite.frames.is_empty() {
        return None;
    }
    let shown = cs_render::materialize(doc, sprite, 0..1);
    let frame = Bitmap::from_frame(&shown, 0, 1);
    if frame.pixels.iter().all(|c| c.a == 0) {
        return None;
    }
    Some(fit(&frame, SIZE))
}

/// Los bytes del `thumbnail.png` de un `.chimp`, leyendo solo el directorio del zip y esa entrada.
/// `None` si el archivo no trae miniatura (uno viejo).
pub fn read_png<R: Read + Seek>(reader: R) -> Result<Option<Vec<u8>>> {
    let mut zip = zip::ZipArchive::new(reader).map_err(|e| IoError::Zip(e.to_string()))?;
    let file = match zip.by_name(ENTRY) {
        Ok(file) => file,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(IoError::Zip(format!("{ENTRY}: {e}"))),
    };
    if file.size() > MAX_PNG_BYTES {
        return Err(IoError::Corrupt(format!("{ENTRY} is too large")));
    }
    let mut out = Vec::new();
    file.take(MAX_PNG_BYTES + 1).read_to_end(&mut out)?;
    if out.len() as u64 > MAX_PNG_BYTES {
        return Err(IoError::Corrupt(format!("{ENTRY} is too large")));
    }
    Ok(Some(out))
}

/// La miniatura de un `.chimp` ya decodificada (ver `read_png`).
pub fn read<R: Read + Seek>(reader: R) -> Result<Option<Bitmap>> {
    let Some(png) = read_png(reader)? else {
        return Ok(None);
    };
    let limit = (MAX_SIDE as usize) * (MAX_SIDE as usize) * 4;
    let image = import::decode_png_limited(&png, limit)?;
    if image.width > MAX_SIDE || image.height > MAX_SIDE || image.pixels.is_empty() {
        return Err(IoError::Corrupt(format!("{ENTRY} has a bad size")));
    }
    Ok(Some(image))
}

/// La miniatura de un `.chimp` en `size × size`: la incrustada o, en un archivo viejo, la que sale
/// de abrirlo entero. `None` si no hay nada que mostrar.
pub fn for_file(bytes: &[u8], size: u32) -> Result<Option<Bitmap>> {
    let image = match read(std::io::Cursor::new(bytes))? {
        Some(image) => Some(image),
        None => {
            let (doc, _) = chimp::read(bytes)?;
            pick(&doc, None).and_then(|id| render(&doc, id))
        }
    };
    Ok(image.map(|image| fit(&image, size)))
}

#[cfg(test)]
#[path = "thumbnail_tests.rs"]
mod tests;
