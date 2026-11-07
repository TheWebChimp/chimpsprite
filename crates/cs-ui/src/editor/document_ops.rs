//! Los cambios al documento desde el editor: artboards, el sprite (tamaño, modo de color,
//! recorte) y las notas.

use super::*;

/// Cuánto se corre (en el mundo, abajo y a la derecha) la copia de una nota duplicada.
const DUPLICATE_NOTE_OFFSET: i32 = 12;

impl Editor {
    /// Crea un artboard vacío del tamaño dado a la derecha de los existentes y lo activa.
    pub fn add_artboard(&mut self, width: u32, height: u32) {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let template = self.sprite();
        let mut sprite = Sprite::new(width, height, template.color_mode);
        sprite.palette = template.palette.clone();
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            model: None,
            in_library: false,
            font: None,
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
        self.document.canvas_count() > 1 && !self.in_symbol_mode() && self.artboard().on_canvas()
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
        self.rename_artboard(self.active, name);
    }

    /// Le cambia el nombre a un artboard; si es una fuente, también a la fuente (un paso).
    pub fn rename_artboard(&mut self, id: ArtboardId, name: String) -> bool {
        let Some(artboard) = self.document.artboard(id) else {
            return false;
        };
        if artboard.name == name && artboard.font.as_ref().is_none_or(|f| f.full_name() == name) {
            return false;
        }
        let cmd = match artboard.font.clone() {
            // La familia (sin el estilo, si el nombre termina en él); el artboard, el completo.
            Some(font) => super::font_ops::set_font_named(
                id,
                cs_core::FontInfo {
                    name: font.family_from_name(&name),
                    ..font
                },
            ),
            None => DocCommand::SetArtboardProps {
                id,
                name: Some(name),
                position: None,
            },
        };
        self.named("Artboard", |e| e.execute_doc(cmd))
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
        if !self.can_resize_sprite() {
            return false;
        }
        self.commit_floating();
        let rotated = sprite_ops::rotate(self.sprite(), rotation);
        self.replace_sprite(rotated)
    }

    /// `Sprite → Sprite Size`: escala todo el sprite (no con instancias ni tilemaps).
    pub fn scale_sprite(&mut self, width: u32, height: u32) -> bool {
        if !self.can_resize_sprite() {
            return false;
        }
        self.commit_floating();
        let scaled = sprite_ops::scale(self.sprite(), width, height);
        self.replace_sprite(scaled)
    }

    /// `Sprite → Color Mode`: pasa a RGBA o a indexado con la paleta del sprite. No en una
    /// fuente: su modo de color es el de la fuente.
    pub fn set_color_mode(&mut self, target: sprite_ops::ColorTarget) -> bool {
        if self.is_structure_locked() {
            return false;
        }
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
        if (x, y, w, h) == (0, 0, sprite.width, sprite.height)
            || w == 0
            || h == 0
            || !self.can_resize_sprite()
        {
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
    /// la activa. La copia de un modelo o una fuente (como el Duplicate de la Library: la fuente
    /// lleva el nombre también en su info) se abre en su workspace y la app le abre su pestaña.
    pub fn duplicate_artboard(&mut self, name: String) -> bool {
        self.commit_floating();
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let mut artboard = Artboard {
            in_library: false,
            id,
            name,
            position: (x1 + 16, y0),
            ..self.artboard().clone()
        };
        if let Some(font) = &mut artboard.font {
            font.name = font.family_from_name(&artboard.name);
            artboard.name = font.full_name();
        }
        let added = self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        });
        if added && self.has_workspace(id) {
            let entered = self.enter_workspace(id);
            self.view_request = entered.then_some(id);
        } else if added {
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
            model: None,
            in_library: false,
            font: None,
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

    /// Una nota nueva con su esquina en `position` (mundo), arriba de las demás; devuelve su id.
    pub fn add_note(&mut self, position: (i32, i32)) -> Option<u32> {
        let id = self.document.next_note_id();
        let note = cs_core::Note {
            id,
            position,
            size: crate::notes::NEW_SIZE,
            text: String::new(),
            color: Default::default(),
        };
        self.insert_note(note)
    }

    /// Pone `note` arriba de las demás (un paso de undo); devuelve su id.
    fn insert_note(&mut self, note: cs_core::Note) -> Option<u32> {
        let id = note.id;
        let index = self.document.notes.len();
        self.execute_doc(DocCommand::AddNote { index, note })
            .then_some(id)
    }

    /// Una copia de la nota `id` un poco más abajo y a la derecha, arriba de todas.
    pub fn duplicate_note(&mut self, id: u32) -> Option<u32> {
        let mut note = self.document.note(id)?.clone();
        note.id = self.document.next_note_id();
        note.position = (
            note.position.0 + DUPLICATE_NOTE_OFFSET,
            note.position.1 + DUPLICATE_NOTE_OFFSET,
        );
        self.insert_note(note)
    }

    /// Reemplaza la nota con el id de `note` (un paso de undo, si cambió).
    pub fn set_note(&mut self, note: cs_core::Note) -> bool {
        match self.document.note(note.id) {
            Some(old) if *old != note => self.execute_doc(DocCommand::SetNote(note)),
            _ => false,
        }
    }

    pub fn set_note_text(&mut self, id: u32, text: String) -> bool {
        let Some(mut note) = self.document.note(id).cloned() else {
            return false;
        };
        note.text = text;
        self.set_note(note)
    }

    pub fn set_note_color(&mut self, id: u32, color: cs_core::NoteColor) -> bool {
        let Some(mut note) = self.document.note(id).cloned() else {
            return false;
        };
        note.color = color;
        self.set_note(note)
    }

    pub fn remove_note(&mut self, id: u32) -> bool {
        self.document.note(id).is_some() && self.execute_doc(DocCommand::RemoveNote { id })
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
