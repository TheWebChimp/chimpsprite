//! Lo de las fuentes pixel en el editor: crear una, moverse entre glifos, agregar y quitar,
//! anchos, y desplazar, copiar, pegar y borrar un glifo.

use std::sync::Arc;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Cel, FontInfo, FontMode};

use super::Editor;

impl Editor {
    /// Una fuente nueva a la derecha de todo, activa y en su primer glifo.
    pub fn new_font(&mut self, name: &str, cell: (u32, u32), mode: FontMode) -> bool {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let (id, cmd) =
            cs_commands::font::new_font_artboard(&self.document, name, cell, mode, (x1 + 16, y0));
        if !self.execute_doc(cmd) {
            return false;
        }
        self.activate(id);
        self.set_frame(0);
        true
    }

    /// La fuente del artboard activo.
    pub fn font(&self) -> Option<&FontInfo> {
        self.artboard().font.as_ref()
    }

    /// El carácter del glifo actual (el frame activo).
    pub fn current_glyph(&self) -> Option<char> {
        self.font()?.glyphs.get(self.frame).map(|g| g.ch)
    }

    pub fn go_to_glyph(&mut self, ch: char) -> bool {
        let Some(index) = self.font().and_then(|f| f.index_of(ch)) else {
            return false;
        };
        self.set_frame(index);
        true
    }

    pub fn add_glyphs(&mut self, chars: &[char]) -> bool {
        cs_commands::font::add_glyphs(self.artboard(), chars).is_some_and(|c| self.execute_doc(c))
    }

    pub fn remove_glyph(&mut self, ch: char) -> bool {
        cs_commands::font::remove_glyph(self.artboard(), ch).is_some_and(|c| self.execute_doc(c))
    }

    pub fn set_glyph_advance(&mut self, ch: char, advance: Option<u32>) -> bool {
        cs_commands::font::set_advance(self.artboard(), ch, advance)
            .is_some_and(|c| self.execute_doc(c))
    }

    pub fn set_font_info(&mut self, info: FontInfo) -> bool {
        if self.font() == Some(&info) {
            return false;
        }
        self.execute_doc(DocCommand::SetFont {
            id: self.active,
            font: Some(Box::new(info)),
        })
    }

    /// El ancho efectivo del glifo actual.
    pub fn glyph_advance(&self) -> Option<u32> {
        let font = self.font()?;
        (self.frame < font.glyphs.len()).then(|| font.advance(self.sprite(), self.frame))
    }

    /// Mueve lo dibujado del glifo actual (su cel) `dx`, `dy` pixeles.
    pub fn shift_glyph(&mut self, dx: i32, dy: i32) -> bool {
        let (layer, frame) = (self.layer, self.frame);
        let Some(cel) = self.sprite().cel(layer, frame).copied() else {
            return false;
        };
        let moved = Cel {
            position: (cel.position.0 + dx, cel.position.1 + dy),
            ..cel
        };
        self.execute(Command::SetCel {
            layer,
            frame,
            cel: Some(moved),
        })
    }

    pub fn copy_glyph(&mut self) {
        let (layer, frame) = (self.layer, self.frame);
        self.glyph_clipboard = self.sprite().cel(layer, frame).and_then(|cel| {
            let image = self.sprite().image(cel.image)?.clone();
            Some((image, cel.position))
        });
    }

    pub fn paste_glyph(&mut self) -> bool {
        let Some((image, position)) = self.glyph_clipboard.clone() else {
            return false;
        };
        let (layer, frame) = (self.layer, self.frame);
        let id = self.sprite().next_image_id();
        self.execute(Command::Batch(vec![
            Command::SetImage {
                id,
                image: Some(image),
            },
            Command::SetCel {
                layer,
                frame,
                cel: Some(Cel {
                    position,
                    opacity: 255,
                    image: id,
                }),
            },
        ]))
    }

    pub fn clear_glyph(&mut self) -> bool {
        let (layer, frame) = (self.layer, self.frame);
        self.sprite().cel(layer, frame).is_some()
            && self.execute(Command::SetCel {
                layer,
                frame,
                cel: None,
            })
    }

    /// Las fuentes del documento (para el selector de la herramienta Texto).
    pub fn document_fonts(&self) -> Vec<(ArtboardId, String)> {
        self.document
            .artboards
            .iter()
            .filter_map(|a| Some((a.id, a.font.as_ref()?.name.clone())))
            .collect()
    }
}

/// Lo que guarda Copy glyph: la imagen del cel y dónde iba.
pub type GlyphClipboard = Option<(Arc<cs_core::Image>, (i32, i32))>;
