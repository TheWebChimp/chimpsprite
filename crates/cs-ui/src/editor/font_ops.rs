//! Lo de las fuentes pixel en el editor: crear una, moverse entre glifos, agregar y quitar,
//! anchos, y desplazar, copiar, pegar y borrar un glifo.

use std::sync::Arc;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Cel, ColorMode, FontInfo, FontMode};

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

    /// Si el artboard activo es una fuente: sus frames, su capa, su modo de color y su celda no
    /// se tocan con las acciones de siempre (solo con las de glifos).
    pub fn is_font(&self) -> bool {
        self.font().is_some()
    }

    /// Sprite Size, Canvas Size, Rotate Canvas, Crop y Trim: no en una fuente (la celda queda
    /// fija al crearla).
    pub fn can_resize_sprite(&self) -> bool {
        !self.is_font()
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
        self.execute_doc(set_font_named(self.active, info))
    }

    /// Como `set_font_info`, pero los cambios seguidos del mismo `group` (el arrastre de un
    /// stepper del panel Font) son un solo paso de undo.
    pub fn set_font_info_grouped(&mut self, info: FontInfo, group: u64) -> bool {
        if self.font() == Some(&info) {
            return false;
        }
        self.execute_doc_grouped(set_font_named(self.active, info), group)
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
        let sprite = self.sprite();
        self.glyph_clipboard = sprite.cel(layer, frame).and_then(|cel| {
            Some(GlyphClip {
                image: sprite.image(cel.image)?.clone(),
                position: cel.position,
                opacity: cel.opacity,
                color_mode: sprite.color_mode,
            })
        });
    }

    /// Pega el glifo copiado en el actual; no si es de otro modo de color (una fuente de color
    /// en una de 1 bit, por ejemplo).
    pub fn paste_glyph(&mut self) -> bool {
        let Some(clip) = self.glyph_clipboard.clone() else {
            return false;
        };
        if clip.color_mode != self.sprite().color_mode {
            return false;
        }
        let GlyphClip {
            image,
            position,
            opacity,
            ..
        } = clip;
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
                    opacity,
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

/// La fuente nueva de `id` y, con ella, su nombre como nombre del artboard: los dos van juntos
/// y el inverso deshace ambos (también al juntar los pasos de un arrastre).
pub(super) fn set_font_named(id: ArtboardId, info: FontInfo) -> DocCommand {
    DocCommand::Batch(vec![
        DocCommand::SetFont {
            id,
            font: Some(Box::new(info.clone())),
        },
        DocCommand::SetArtboardProps {
            id,
            name: Some(info.name),
            position: None,
        },
    ])
}

/// Lo que guarda Copy glyph: la imagen del cel, dónde iba, su opacidad y el modo de color de
/// la fuente de donde salió.
#[derive(Clone, Debug)]
pub struct GlyphClip {
    image: Arc<cs_core::Image>,
    position: (i32, i32),
    opacity: u8,
    color_mode: ColorMode,
}

pub type GlyphClipboard = Option<GlyphClip>;
