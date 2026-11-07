//! Lo de las fuentes pixel en el editor: crear una, moverse entre glifos, agregar y quitar,
//! anchos, y desplazar, copiar, pegar y borrar un glifo.

use std::sync::Arc;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Cel, ColorMode, FontInfo, FontMode, GlyphComponent};

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
        // Se edita en su workspace (su pestaña), fuera del lienzo.
        self.enter_workspace(id);
        self.set_frame(0);
        true
    }

    /// Agrega una fuente ya armada (una importada) a la derecha de todo, en un paso de undo, y
    /// entra a ella como con [`Editor::new_font`].
    pub fn add_font(&mut self, font: FontInfo, sprite: cs_core::Sprite) -> bool {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let (id, cmd) =
            cs_commands::font::add_font_artboard(&self.document, font, sprite, (x1 + 16, y0));
        if !self.named("Import Font", |e| e.execute_doc(cmd)) {
            return false;
        }
        self.enter_workspace(id);
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
        !self.is_structure_locked()
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
        let Some(cmd) = cs_commands::font::add_glyphs(self.artboard(), chars) else {
            return false;
        };
        self.named("Add Glyph", |e| e.execute_doc(cmd))
    }

    pub fn remove_glyph(&mut self, ch: char) -> bool {
        let Some(cmd) = cs_commands::font::remove_glyph(self.artboard(), ch) else {
            return false;
        };
        self.named("Remove Glyph", |e| e.execute_doc(cmd))
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

    /// El ancho efectivo del glifo de `ch`.
    pub fn advance_of(&self, ch: char) -> Option<u32> {
        let font = self.font()?;
        Some(font.advance(self.sprite(), font.index_of(ch)?))
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

    /// Los componentes del glifo actual (vacío si no es compuesto).
    pub fn glyph_components(&self) -> Vec<GlyphComponent> {
        self.font()
            .and_then(|f| f.glyphs.get(self.frame))
            .map_or_else(Vec::new, |g| g.components.clone())
    }

    /// El componente elegido del glifo actual (si sigue existiendo).
    pub fn selected_component(&self) -> Option<usize> {
        let (ch, i) = self.glyph_component?;
        (Some(ch) == self.current_glyph() && i < self.glyph_components().len()).then_some(i)
    }

    pub fn select_component(&mut self, index: Option<usize>) {
        self.glyph_component = index.and_then(|i| Some((self.current_glyph()?, i)));
    }

    /// Los componentes del glifo de `ch`, en un paso; no si haría un ciclo.
    pub fn set_glyph_components(&mut self, ch: char, components: Vec<GlyphComponent>) -> bool {
        let Some(cmd) = cs_commands::font::set_components(self.artboard(), ch, components) else {
            return false;
        };
        self.named("Glyph Components", |e| e.execute_doc(cmd))
    }

    /// Pone el componente `index` del glifo actual en `offset`.
    pub fn move_component(&mut self, index: usize, offset: (i32, i32)) -> bool {
        let Some(ch) = self.current_glyph() else {
            return false;
        };
        let mut components = self.glyph_components();
        let Some(c) = components.get_mut(index) else {
            return false;
        };
        c.offset = offset;
        self.named("Move Component", |e| e.set_glyph_components(ch, components))
    }

    /// Como `move_component`, pero los cambios seguidos del mismo `group` (el arrastre de un
    /// stepper del panel) son un solo paso.
    pub fn move_component_grouped(&mut self, index: usize, offset: (i32, i32), group: u64) -> bool {
        let Some(ch) = self.current_glyph() else {
            return false;
        };
        let mut components = self.glyph_components();
        let Some(c) = components.get_mut(index) else {
            return false;
        };
        c.offset = offset;
        let Some(cmd) = cs_commands::font::set_components(self.artboard(), ch, components) else {
            return false;
        };
        self.execute_doc_grouped(cmd, group)
    }

    /// Las flechas con Move en un compuesto: mueven el componente elegido.
    pub fn can_nudge_component(&self) -> bool {
        self.tool == super::ToolKind::Move
            && self.selected_component().is_some()
            && self.selection.is_none()
            && !self.can_transform()
    }

    pub fn nudge_component(&mut self, dx: i32, dy: i32) -> bool {
        let Some(i) = self.selected_component() else {
            return false;
        };
        let (x, y) = self.glyph_components()[i].offset;
        self.move_component(i, (x + dx, y + dy))
    }

    /// Agrega el glifo de `component` como componente del actual, en (0, 0), y lo elige.
    pub fn add_component(&mut self, component: char) -> bool {
        let Some(ch) = self.current_glyph() else {
            return false;
        };
        let mut components = self.glyph_components();
        components.push(GlyphComponent {
            ch: component,
            offset: (0, 0),
        });
        let index = components.len() - 1;
        if !self.named("Add Component", |e| e.set_glyph_components(ch, components)) {
            return false;
        }
        self.select_component(Some(index));
        true
    }

    pub fn remove_component(&mut self, index: usize) -> bool {
        let Some(ch) = self.current_glyph() else {
            return false;
        };
        let mut components = self.glyph_components();
        if index >= components.len() {
            return false;
        }
        components.remove(index);
        self.select_component(None);
        self.named("Remove Component", |e| {
            e.set_glyph_components(ch, components)
        })
    }

    /// Desvincula el compuesto de `ch` (pasa a pixeles propios).
    pub fn unlink_glyph(&mut self, ch: char) -> bool {
        let Some(cmd) = cs_commands::font::unlink_glyph(self.artboard(), ch) else {
            return false;
        };
        self.named("Unlink Glyph", |e| e.execute_doc(cmd))
    }

    /// Auto-build accents (o solo los de `only`; con `replace`, también los ya dibujados), en un
    /// paso. Devuelve lo que pasó.
    pub fn auto_build_accents(
        &mut self,
        only: Option<&[char]>,
        replace: bool,
    ) -> cs_commands::font::AccentReport {
        let (cmd, report) = cs_commands::font::auto_build_accents(self.artboard(), only, replace);
        if let Some(cmd) = cmd {
            let current = self.current_glyph();
            self.named("Auto-build Accents", |e| e.execute_doc(cmd));
            // Los glifos nuevos corren los frames: el actual sigue siendo el mismo carácter.
            if let Some(ch) = current {
                self.go_to_glyph(ch);
            }
        }
        report
    }

    /// Pone los pixeles de `glyphs` en la fuente activa (ver `cs_commands::font::fill_glyphs`),
    /// en un paso. Devuelve cuántos puso.
    pub fn fill_glyphs(&mut self, glyphs: &[cs_core::GlyphBuild], replace: bool) -> usize {
        let Some((cmd, count)) = cs_commands::font::fill_glyphs(self.artboard(), glyphs, replace)
        else {
            return 0;
        };
        let current = self.current_glyph();
        if !self.named("Generate Glyphs", |e| e.execute_doc(cmd)) {
            return 0;
        }
        if let Some(ch) = current {
            self.go_to_glyph(ch);
        }
        count
    }

    /// La fuente de la herramienta Texto, al día con el documento: si es una fuente del
    /// documento, con su nombre de ahora; si su artboard ya no está, la Pixel 5×7.
    pub fn sync_text_font(&mut self) {
        let cs_tools::text::FontChoice::Document { artboard, name } = &self.text_style.font else {
            return;
        };
        let font = self
            .document
            .artboard(ArtboardId(*artboard))
            .and_then(|a| a.font.as_ref());
        match font {
            Some(font) if font.full_name() != *name => {
                self.text_style.font = cs_tools::text::FontChoice::Document {
                    artboard: *artboard,
                    name: font.full_name(),
                };
            }
            Some(_) => {}
            None => {
                self.text_style.font = cs_tools::text::FontChoice::Pixel;
                self.text_style.size = 1;
            }
        }
    }

    /// Las fuentes del documento (para el selector de la herramienta Texto).
    pub fn document_fonts(&self) -> Vec<(ArtboardId, String)> {
        self.document
            .artboards
            .iter()
            .filter_map(|a| Some((a.id, a.font.as_ref()?.full_name())))
            .collect()
    }
}

/// La fuente nueva de `id` y, con ella, su nombre completo (familia y estilo) como nombre del
/// artboard: los dos van juntos y el inverso deshace ambos (también al juntar los pasos de un
/// arrastre).
pub(super) fn set_font_named(id: ArtboardId, info: FontInfo) -> DocCommand {
    let name = info.full_name();
    DocCommand::Batch(vec![
        DocCommand::SetFont {
            id,
            font: Some(Box::new(info)),
        },
        DocCommand::SetArtboardProps {
            id,
            name: Some(name),
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
