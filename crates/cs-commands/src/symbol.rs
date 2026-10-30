//! Símbolos de la biblioteca: convertir capas o píxeles en un símbolo (F8), mandar artboards a
//! la biblioteca y sacarlos al lienzo.

use cs_core::{Artboard, ArtboardId, Document};

use crate::DocCommand;

#[cfg(test)]
mod tests;

/// Separación entre un artboard nuevo y los demás, como en `Editor::add_artboard`.
const GAP: i32 = 16;

/// Lugar libre para un artboard nuevo en el lienzo: a la derecha de todos.
pub fn free_position(doc: &Document) -> (i32, i32) {
    match doc.bounds() {
        Some(((_, y0), (x1, _))) => (x1 + GAP, y0),
        None => (0, 0),
    }
}

/// Manda `id` a la biblioteca. `None` si ya está ahí o si es el último artboard del lienzo.
pub fn move_to_library(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    (!artboard.in_library && doc.canvas_count() > 1).then_some(DocCommand::SetArtboardInLibrary {
        id,
        in_library: true,
    })
}

/// Saca `id` de la biblioteca a su lugar o, si choca con otro artboard del lienzo, a la derecha
/// de todos.
pub fn place_on_canvas(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    if !artboard.in_library {
        return None;
    }
    let show = DocCommand::SetArtboardInLibrary {
        id,
        in_library: false,
    };
    if !collides(doc, artboard) {
        return Some(show);
    }
    Some(DocCommand::Batch(vec![
        DocCommand::SetArtboardProps {
            id,
            name: None,
            position: Some(free_position(doc)),
        },
        show,
    ]))
}

/// ¿El rectángulo de `artboard` se encima con el de algún artboard del lienzo?
fn collides(doc: &Document, artboard: &Artboard) -> bool {
    let ((ax0, ay0), (ax1, ay1)) = artboard.rect();
    doc.canvas_artboards().any(|other| {
        let ((bx0, by0), (bx1, by1)) = other.rect();
        ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
    })
}
