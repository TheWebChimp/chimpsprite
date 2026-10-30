//! Los cambios al documento desde el editor: artboards, el sprite (tamaño, modo de color,
//! recorte) y las notas.

use super::*;

impl Editor {
    /// Crea un artboard vacío del tamaño dado a la derecha de los existentes y lo activa.
    pub fn add_artboard(&mut self, width: u32, height: u32) {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let template = self.sprite();
        let mut sprite = Sprite::new(width, height, template.color_mode);
        sprite.palette = template.palette.clone();
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            in_library: false,
            guides: Vec::new(),
            symmetry: Default::default(),
            id,
            name: format!("Artboard {}", id.0),
            position: (x1 + 16, y0),
            sprite,
        };
        if self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        }) {
            self.activate(id);
        }
    }

    /// ¿Se puede borrar el artboard activo? El último del lienzo no (la biblioteca no cuenta),
    /// y un símbolo (en modo símbolo o en la biblioteca) se borra desde la Library, que avisa.
    pub fn can_remove_active_artboard(&self) -> bool {
        self.document.canvas_count() > 1 && !self.in_symbol_mode() && !self.artboard().in_library
    }

    pub fn remove_active_artboard(&mut self) {
        if self.can_remove_active_artboard()
            && self.execute_doc(DocCommand::RemoveArtboard { id: self.active })
            && let Some(last) = self.document.canvas_artboards().last()
        {
            self.activate(last.id);
        }
    }

    pub fn rename_active_artboard(&mut self, name: String) {
        if name != self.artboard().name {
            self.execute_doc(DocCommand::SetArtboardProps {
                id: self.active,
                name: Some(name),
                position: None,
            });
        }
    }

    /// Reemplaza el sprite del artboard activo (un paso de undo); `None` no hace nada.
    fn replace_sprite(&mut self, sprite: Option<Sprite>) -> bool {
        let Some(sprite) = sprite else {
            return false;
        };
        self.execute_doc(DocCommand::SetSprite {
            id: self.active,
            sprite: Box::new(sprite),
        })
    }

    /// `Sprite → Rotate Canvas`: gira todo el sprite (no con instancias ni tilemaps).
    pub fn rotate_canvas(&mut self, rotation: sprite_ops::Rotation) -> bool {
        self.commit_floating();
        let rotated = sprite_ops::rotate(self.sprite(), rotation);
        self.replace_sprite(rotated)
    }

    /// `Sprite → Sprite Size`: escala todo el sprite (no con instancias ni tilemaps).
    pub fn scale_sprite(&mut self, width: u32, height: u32) -> bool {
        self.commit_floating();
        let scaled = sprite_ops::scale(self.sprite(), width, height);
        self.replace_sprite(scaled)
    }

    /// `Sprite → Color Mode`: pasa a RGBA o a indexado con la paleta del sprite.
    pub fn set_color_mode(&mut self, target: sprite_ops::ColorTarget) -> bool {
        self.commit_floating();
        let converted = sprite_ops::convert(self.sprite(), target);
        self.replace_sprite(converted)
    }

    /// Sprite → Properties → Pixel Aspect Ratio: cómo se ve cada píxel (los píxeles no cambian).
    pub fn set_pixel_ratio(&mut self, ratio: (u8, u8)) -> bool {
        if self.sprite().pixel_ratio == ratio {
            return false;
        }
        let mut sprite = self.sprite().clone();
        sprite.pixel_ratio = ratio;
        self.named("Pixel Aspect Ratio", |e| e.replace_sprite(Some(sprite)))
    }

    /// Cambia el índice transparente de un sprite indexado (los píxeles no cambian).
    pub fn set_transparent_index(&mut self, index: u8) -> bool {
        let ColorMode::Indexed { transparent_index } = self.sprite().color_mode else {
            return false;
        };
        if transparent_index == index {
            return false;
        }
        let mut sprite = self.sprite().clone();
        sprite.color_mode = ColorMode::Indexed {
            transparent_index: index,
        };
        self.replace_sprite(Some(sprite))
    }

    /// Recorta el lienzo al rectángulo `(x, y, w, h)` (local) y descarta lo de fuera; el
    /// contenido queda en su lugar del mundo.
    fn crop_canvas(&mut self, (x, y, w, h): (i32, i32, u32, u32)) -> bool {
        let sprite = self.sprite();
        if (x, y, w, h) == (0, 0, sprite.width, sprite.height) || w == 0 || h == 0 {
            return false;
        }
        let (ax, ay) = self.artboard().position;
        let id = self.active;
        self.execute_doc(DocCommand::Batch(vec![
            DocCommand::OnArtboard {
                id,
                cmd: Command::ResizeCanvas {
                    width: w,
                    height: h,
                    offset: (-x, -y),
                },
            },
            DocCommand::OnArtboard {
                id,
                cmd: Command::TrimToCanvas,
            },
            DocCommand::SetArtboardProps {
                id,
                name: None,
                position: Some((ax + x, ay + y)),
            },
        ]))
    }

    /// `Sprite → Crop`: el lienzo pasa a ser el rectángulo de la selección.
    pub fn crop_to_selection(&mut self) -> bool {
        self.commit_floating();
        let Some(((x0, y0), (x1, y1))) = self.selection.as_ref().and_then(|m| m.bounds()) else {
            return false;
        };
        self.crop_canvas((x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
    }

    /// `Sprite → Trim`: el lienzo se ajusta a lo que tiene píxeles visibles.
    pub fn trim_canvas(&mut self) -> bool {
        self.commit_floating();
        match sprite_ops::content_bounds(self.sprite()) {
            Some(bounds) => self.crop_canvas(bounds),
            None => false,
        }
    }

    /// `Sprite → Duplicate`: una copia del artboard activo con `name`, a la derecha de todo, y
    /// la activa.
    pub fn duplicate_artboard(&mut self, name: String) -> bool {
        self.commit_floating();
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            in_library: false,
            id,
            name,
            position: (x1 + 16, y0),
            ..self.artboard().clone()
        };
        let added = self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        });
        if added {
            self.activate(id);
        }
        added
    }

    /// Un artboard nuevo con `sprite` y `name`, a la derecha de todo, y lo activa (un paso de
    /// undo).
    pub fn add_artboard_with_sprite(&mut self, name: String, sprite: cs_core::Sprite) -> bool {
        self.commit_floating();
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            in_library: false,
            id,
            name,
            position: (x1 + 16, y0),
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
        };
        let added = self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        });
        if added {
            self.activate(id);
        }
        added
    }

    /// Reemplaza las notas del documento (un paso de undo).
    pub fn set_notes(&mut self, notes: Vec<cs_core::Note>) -> bool {
        if notes == self.document.notes {
            return false;
        }
        self.execute_doc(DocCommand::SetNotes(notes))
    }

    /// Una nota nueva con su esquina en `position` (mundo); devuelve su id.
    pub fn add_note(&mut self, position: (i32, i32)) -> Option<u32> {
        let id = self.document.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let mut notes = self.document.notes.clone();
        notes.push(cs_core::Note {
            id,
            position,
            size: crate::notes::NEW_SIZE,
            text: String::new(),
            color: Default::default(),
        });
        self.set_notes(notes).then_some(id)
    }

    pub fn set_note_text(&mut self, id: u32, text: String) -> bool {
        let mut notes = self.document.notes.clone();
        match notes.iter_mut().find(|n| n.id == id) {
            Some(n) if n.text != text => n.text = text,
            _ => return false,
        }
        self.set_notes(notes)
    }

    pub fn set_note_color(&mut self, id: u32, color: cs_core::NoteColor) -> bool {
        let mut notes = self.document.notes.clone();
        match notes.iter_mut().find(|n| n.id == id) {
            Some(n) if n.color != color => n.color = color,
            _ => return false,
        }
        self.set_notes(notes)
    }

    pub fn remove_note(&mut self, id: u32) -> bool {
        let mut notes = self.document.notes.clone();
        notes.retain(|n| n.id != id);
        self.set_notes(notes)
    }

    /// Lleva la nota `id` encima de las demás.
    pub fn note_to_front(&mut self, id: u32) -> bool {
        let mut notes = self.document.notes.clone();
        let Some(i) = notes.iter().position(|n| n.id == id) else {
            return false;
        };
        let note = notes.remove(i);
        notes.push(note);
        self.set_notes(notes)
    }
}
