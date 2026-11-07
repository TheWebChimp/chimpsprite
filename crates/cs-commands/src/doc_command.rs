use cs_core::{Artboard, ArtboardId, Document, FontInfo, Guide, Model3D, Note, Sprite, Symmetry};
use serde::{Deserialize, Serialize};

use crate::{CmdError, Command};

/// Comandos a nivel documento. Igual que `Command`, `apply` devuelve el inverso.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DocCommand {
    /// Aplica un comando de sprite al artboard indicado.
    OnArtboard {
        id: ArtboardId,
        cmd: Command,
    },
    AddArtboard {
        index: usize,
        artboard: Box<Artboard>,
    },
    RemoveArtboard {
        id: ArtboardId,
    },
    SetArtboardProps {
        id: ArtboardId,
        name: Option<String>,
        position: Option<(i32, i32)>,
    },
    /// Manda un artboard a la biblioteca (`true`) o lo saca al lienzo (`false`).
    SetArtboardInLibrary {
        id: ArtboardId,
        in_library: bool,
    },
    /// Reemplaza las guías de un artboard.
    SetGuides {
        id: ArtboardId,
        guides: Vec<Guide>,
    },
    SetSymmetry {
        id: ArtboardId,
        symmetry: Symmetry,
    },
    /// Lo de fuente de un artboard (`None`: deja de ser fuente).
    SetFont {
        id: ArtboardId,
        font: Option<Box<FontInfo>>,
    },
    /// Lo de modelo 3D de un artboard (`None`: deja de ser modelo).
    SetModel {
        id: ArtboardId,
        model: Option<Box<Model3D>>,
    },
    /// Reemplaza el sprite completo de un artboard (p. ej. al cambiar el modo de color).
    SetSprite {
        id: ArtboardId,
        sprite: Box<Sprite>,
    },
    /// Reemplaza las notas del documento (agregar, mover, editar o borrar una).
    SetNotes(Vec<Note>),
    /// Agrega una nota en `index` del z-order de las notas (la última queda arriba).
    AddNote {
        index: usize,
        note: Note,
    },
    /// Reemplaza la nota con el mismo id (moverla, agrandarla, escribirla o cambiarle el color).
    SetNote(Note),
    RemoveNote {
        id: u32,
    },
    /// Reemplaza el documento entero (lo que dejó un script): el inverso es el de antes.
    SetDocument(Box<Document>),
    /// Transacción: se aplica completa o nada.
    Batch(Vec<DocCommand>),
}

impl DocCommand {
    pub fn apply(&self, doc: &mut Document) -> Result<DocCommand, CmdError> {
        match self {
            DocCommand::OnArtboard { id, cmd } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let inverse = cmd.apply(&mut artboard.sprite)?;
                // Un artboard no puede contenerse a sí mismo a través de sus instancias.
                if !doc.cycle_free(*id) {
                    let artboard = doc.artboard_mut(*id).expect("still there");
                    if let Err(e) = inverse.apply(&mut artboard.sprite) {
                        log::error!("instance cycle rollback failed: {e}");
                    }
                    return Err(CmdError::InstanceCycle);
                }
                Ok(DocCommand::OnArtboard {
                    id: *id,
                    cmd: inverse,
                })
            }
            DocCommand::AddArtboard { index, artboard } => {
                if *index > doc.artboards.len() {
                    return Err(CmdError::IndexOutOfRange(*index));
                }
                if doc.artboard(artboard.id).is_some() {
                    return Err(CmdError::DuplicateArtboardId(artboard.id));
                }
                doc.artboards.insert(*index, (**artboard).clone());
                let inverse = DocCommand::RemoveArtboard { id: artboard.id };
                // El nuevo puede instanciarse a sí mismo o cerrar un ciclo con keys que ya lo
                // apuntaban.
                let id = artboard.id;
                let cycle = doc
                    .artboards
                    .iter()
                    .filter(|a| a.id == id || doc.instance_sources(a.id).contains(&id))
                    .any(|a| !doc.cycle_free(a.id));
                if cycle {
                    doc.artboards.remove(*index);
                    return Err(CmdError::InstanceCycle);
                }
                Ok(inverse)
            }
            DocCommand::RemoveArtboard { id } => {
                let index = doc.index(*id).ok_or(CmdError::ArtboardNotFound(*id))?;
                let on_canvas = doc.artboards[index].on_canvas();
                if doc.artboards.len() == 1 || (on_canvas && doc.canvas_count() == 1) {
                    return Err(CmdError::LastArtboard);
                }
                let artboard = Box::new(doc.artboards.remove(index));
                Ok(DocCommand::AddArtboard { index, artboard })
            }
            DocCommand::SetArtboardProps { id, name, position } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old_name = name
                    .as_ref()
                    .map(|n| std::mem::replace(&mut artboard.name, n.clone()));
                let old_position = position.map(|p| std::mem::replace(&mut artboard.position, p));
                Ok(DocCommand::SetArtboardProps {
                    id: *id,
                    name: old_name,
                    position: old_position,
                })
            }
            DocCommand::SetArtboardInLibrary { id, in_library } => {
                let artboard = doc.artboard(*id).ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = artboard.in_library;
                if *in_library && artboard.on_canvas() && doc.canvas_count() == 1 {
                    return Err(CmdError::LastArtboard);
                }
                doc.artboard_mut(*id).expect("checked").in_library = *in_library;
                Ok(DocCommand::SetArtboardInLibrary {
                    id: *id,
                    in_library: old,
                })
            }
            DocCommand::SetGuides { id, guides } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.guides, guides.clone());
                Ok(DocCommand::SetGuides {
                    id: *id,
                    guides: old,
                })
            }
            DocCommand::SetSymmetry { id, symmetry } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.symmetry, *symmetry);
                Ok(DocCommand::SetSymmetry {
                    id: *id,
                    symmetry: old,
                })
            }
            DocCommand::SetFont { id, font } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.font, font.as_deref().cloned());
                Ok(DocCommand::SetFont {
                    id: *id,
                    font: old.map(Box::new),
                })
            }
            DocCommand::SetModel { id, model } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.model, model.as_deref().cloned());
                Ok(DocCommand::SetModel {
                    id: *id,
                    model: old.map(Box::new),
                })
            }
            DocCommand::SetNotes(notes) => {
                let old = std::mem::replace(&mut doc.notes, notes.clone());
                Ok(DocCommand::SetNotes(old))
            }
            DocCommand::AddNote { index, note } => {
                if *index > doc.notes.len() {
                    return Err(CmdError::IndexOutOfRange(*index));
                }
                if doc.note(note.id).is_some() {
                    return Err(CmdError::DuplicateNoteId(note.id));
                }
                doc.notes.insert(*index, note.clone());
                Ok(DocCommand::RemoveNote { id: note.id })
            }
            DocCommand::SetNote(note) => {
                let old = doc
                    .notes
                    .iter_mut()
                    .find(|n| n.id == note.id)
                    .ok_or(CmdError::NoteNotFound(note.id))?;
                Ok(DocCommand::SetNote(std::mem::replace(old, note.clone())))
            }
            DocCommand::RemoveNote { id } => {
                let index = doc
                    .notes
                    .iter()
                    .position(|n| n.id == *id)
                    .ok_or(CmdError::NoteNotFound(*id))?;
                let note = doc.notes.remove(index);
                Ok(DocCommand::AddNote { index, note })
            }
            DocCommand::SetDocument(document) => {
                if document.artboards.is_empty() {
                    return Err(CmdError::LastArtboard);
                }
                let old = std::mem::replace(doc, (**document).clone());
                Ok(DocCommand::SetDocument(Box::new(old)))
            }
            DocCommand::SetSprite { id, sprite } => {
                if sprite.width == 0 || sprite.height == 0 {
                    return Err(CmdError::InvalidSize(sprite.width, sprite.height));
                }
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.sprite, (**sprite).clone());
                if !doc.cycle_free(*id) {
                    doc.artboard_mut(*id).expect("still there").sprite = old;
                    return Err(CmdError::InstanceCycle);
                }
                Ok(DocCommand::SetSprite {
                    id: *id,
                    sprite: Box::new(old),
                })
            }
            DocCommand::Batch(cmds) => {
                let mut inverses = Vec::with_capacity(cmds.len());
                for cmd in cmds {
                    match cmd.apply(doc) {
                        Ok(inv) => inverses.push(inv),
                        Err(e) => {
                            for inv in inverses.iter().rev() {
                                if let Err(rollback) = inv.apply(doc) {
                                    log::error!("batch rollback failed: {rollback}");
                                }
                            }
                            return Err(e);
                        }
                    }
                }
                inverses.reverse();
                Ok(DocCommand::Batch(inverses))
            }
        }
    }
}

impl crate::Undoable for DocCommand {
    type Target = Document;

    fn apply(&self, target: &mut Document) -> Result<Self, CmdError> {
        DocCommand::apply(self, target)
    }

    fn label(&self) -> &'static str {
        match self {
            DocCommand::OnArtboard { cmd, .. } => cmd.label(),
            DocCommand::AddArtboard { .. } => "New Artboard",
            DocCommand::RemoveArtboard { .. } => "Delete Artboard",
            DocCommand::SetArtboardProps { .. } => "Artboard",
            DocCommand::SetArtboardInLibrary { .. } => "Library",
            DocCommand::SetGuides { .. } => "Guides",
            DocCommand::SetSymmetry { .. } => "Symmetry",
            DocCommand::SetFont { .. } => "Font",
            DocCommand::SetModel { .. } => "Model",
            DocCommand::SetSprite { .. } => "Sprite",
            DocCommand::SetNotes(_) => "Notes",
            DocCommand::AddNote { .. } => "New Note",
            DocCommand::SetNote(_) => "Note",
            DocCommand::RemoveNote { .. } => "Delete Note",
            DocCommand::SetDocument(_) => "Script",
            DocCommand::Batch(cmds) => cmds.first().map_or("Change", crate::Undoable::label),
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use super::*;

    fn doc() -> Document {
        Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba))
    }

    fn second(d: &Document) -> Artboard {
        Artboard {
            model: None,
            in_library: false,
            font: None,
            guides: Vec::new(),
            symmetry: Default::default(),
            id: d.next_artboard_id(),
            name: "two".into(),
            position: (10, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        }
    }

    #[test]
    fn the_last_canvas_artboard_stays_even_with_a_model_around() {
        let mut d = doc();
        let mut model = second(&d);
        model.model = Some(Model3D::default());
        let model_id = model.id;
        d.artboards.push(model);
        assert_eq!(
            DocCommand::RemoveArtboard { id: ArtboardId(1) }.apply(&mut d),
            Err(CmdError::LastArtboard)
        );
        assert_eq!(
            DocCommand::SetArtboardInLibrary {
                id: ArtboardId(1),
                in_library: true
            }
            .apply(&mut d),
            Err(CmdError::LastArtboard)
        );
        DocCommand::RemoveArtboard { id: model_id }
            .apply(&mut d)
            .unwrap();
        assert_eq!(d.artboards.len(), 1);
    }

    #[test]
    fn set_notes_undoes() {
        let mut doc = Document::with_sprite("a", Sprite::new(4, 4, ColorMode::Rgba));
        let note = cs_core::Note {
            id: 1,
            position: (5, 5),
            size: (100, 80),
            text: "hola".into(),
            color: Default::default(),
        };
        let inverse = DocCommand::SetNotes(vec![note.clone()])
            .apply(&mut doc)
            .unwrap();
        assert_eq!(doc.notes, vec![note]);
        inverse.apply(&mut doc).unwrap();
        assert!(doc.notes.is_empty());
    }

    #[test]
    fn instance_cycles_are_rejected_and_nothing_changes() {
        let mut doc = Document::with_sprite("a", Sprite::new(4, 4, ColorMode::Rgba));
        doc.artboards.push(Artboard {
            model: None,
            in_library: false,
            font: None,
            id: ArtboardId(2),
            name: "b".into(),
            position: (10, 0),
            sprite: Sprite::new(4, 4, ColorMode::Rgba),
            guides: Vec::new(),
            symmetry: Default::default(),
        });
        let place = |doc: &Document, host: u32, source: u32| DocCommand::OnArtboard {
            id: ArtboardId(host),
            cmd: crate::ops::new_object_layer(
                &doc.artboard(ArtboardId(host)).unwrap().sprite,
                None,
                0,
                ArtboardId(source),
                "i",
                (0, 0),
            ),
        };
        place(&doc, 1, 2).apply(&mut doc).unwrap();
        let before = doc.clone();
        assert_eq!(
            place(&doc, 2, 1).apply(&mut doc),
            Err(CmdError::InstanceCycle)
        );
        assert_eq!(
            place(&doc, 1, 1).apply(&mut doc),
            Err(CmdError::InstanceCycle)
        );
        assert_eq!(doc, before);
    }

    /// Sprite con una capa de instancia que muestra `source`.
    fn instancing(source: u32) -> Sprite {
        let s = Sprite::new(4, 4, ColorMode::Rgba);
        let mut s2 = s.clone();
        crate::ops::new_object_layer(&s, None, 0, ArtboardId(source), "i", (0, 0))
            .apply(&mut s2)
            .unwrap();
        s2
    }

    #[test]
    fn set_sprite_rejects_instance_cycles() {
        let mut d = doc();
        let before = d.clone();
        assert_eq!(
            DocCommand::SetSprite {
                id: ArtboardId(1),
                sprite: Box::new(instancing(1)),
            }
            .apply(&mut d),
            Err(CmdError::InstanceCycle)
        );
        assert_eq!(d, before);
    }

    #[test]
    fn add_artboard_rejects_instance_cycles() {
        let mut d = doc();
        let before = d.clone();
        // Se instancia a sí mismo.
        let mut ab = second(&d);
        ab.sprite = instancing(ab.id.0);
        assert_eq!(
            DocCommand::AddArtboard {
                index: 1,
                artboard: Box::new(ab),
            }
            .apply(&mut d),
            Err(CmdError::InstanceCycle)
        );
        assert_eq!(d, before);
        // El 1 apunta (colgado) al 7; agregar un 7 que instancia al 1 cerraría el ciclo.
        let mut d = Document::with_sprite("a", instancing(7));
        let before = d.clone();
        let mut ab = second(&d);
        ab.id = ArtboardId(7);
        ab.sprite = instancing(1);
        assert_eq!(
            DocCommand::AddArtboard {
                index: 0,
                artboard: Box::new(ab.clone()),
            }
            .apply(&mut d),
            Err(CmdError::InstanceCycle)
        );
        assert_eq!(d, before);
        // Sin ciclo entra bien.
        ab.sprite = Sprite::new(2, 2, ColorMode::Rgba);
        DocCommand::AddArtboard {
            index: 0,
            artboard: Box::new(ab),
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.artboards.len(), 2);
    }

    #[test]
    fn set_sprite_roundtrip() {
        let mut d = doc();
        let before = d.clone();
        let inv = DocCommand::SetSprite {
            id: ArtboardId(1),
            sprite: Box::new(Sprite::new(
                3,
                2,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            )),
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.artboards[0].sprite.width, 3);
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
        assert!(
            DocCommand::SetSprite {
                id: ArtboardId(9),
                sprite: Box::new(Sprite::new(1, 1, ColorMode::Rgba)),
            }
            .apply(&mut d)
            .is_err()
        );
    }

    #[test]
    fn on_artboard_targets_the_right_sprite() {
        let mut d = doc();
        let ab = second(&d);
        DocCommand::AddArtboard {
            index: 1,
            artboard: Box::new(ab),
        }
        .apply(&mut d)
        .unwrap();
        let before = d.clone();
        let inv = DocCommand::OnArtboard {
            id: ArtboardId(2),
            cmd: Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(1, 1, PixelValue::Rgba(Rgba::rgb(9, 9, 9)))],
            },
        }
        .apply(&mut d)
        .unwrap();
        assert!(d.artboards[0].sprite.cels.is_empty());
        assert!(!d.artboards[1].sprite.cels.is_empty());
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
    }

    #[test]
    fn add_remove_props_roundtrip_and_last_artboard() {
        let mut d = doc();
        let before = d.clone();
        let inv_add = DocCommand::AddArtboard {
            index: 1,
            artboard: Box::new(second(&d)),
        }
        .apply(&mut d)
        .unwrap();
        let mid = d.clone();
        let inv_props = DocCommand::SetArtboardProps {
            id: ArtboardId(2),
            name: Some("renamed".into()),
            position: Some((-3, 7)),
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.artboards[1].name, "renamed");
        assert_eq!(d.artboards[1].position, (-3, 7));
        inv_props.apply(&mut d).unwrap();
        assert_eq!(d, mid);
        let inv_remove = DocCommand::RemoveArtboard { id: ArtboardId(2) }
            .apply(&mut d)
            .unwrap();
        inv_remove.apply(&mut d).unwrap();
        assert_eq!(d, mid);
        inv_add.apply(&mut d).unwrap();
        assert_eq!(d, before);
        assert_eq!(
            DocCommand::RemoveArtboard { id: ArtboardId(1) }.apply(&mut d),
            Err(CmdError::LastArtboard)
        );
    }

    #[test]
    fn failing_batch_rolls_back() {
        let mut d = doc();
        let before = d.clone();
        let r = DocCommand::Batch(vec![
            DocCommand::SetArtboardProps {
                id: ArtboardId(1),
                name: Some("x".into()),
                position: None,
            },
            DocCommand::RemoveArtboard { id: ArtboardId(42) },
        ])
        .apply(&mut d);
        assert_eq!(r, Err(CmdError::ArtboardNotFound(ArtboardId(42))));
        assert_eq!(d, before);
    }

    #[test]
    fn set_guides_roundtrip() {
        let mut d = doc();
        let before = d.clone();
        let guides = vec![
            Guide {
                axis: Axis::Vertical,
                position: 3,
            },
            Guide {
                axis: Axis::Horizontal,
                position: -1,
            },
        ];
        let inv = DocCommand::SetGuides {
            id: ArtboardId(1),
            guides: guides.clone(),
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.artboards[0].guides, guides);
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
    }

    #[test]
    fn set_model_roundtrip() {
        let mut d = doc();
        let before = d.clone();
        let model = Model3D {
            texture: Some(ArtboardId(7)),
            ..Default::default()
        };
        let inv = DocCommand::SetModel {
            id: ArtboardId(1),
            model: Some(Box::new(model.clone())),
        }
        .apply(&mut d)
        .unwrap();
        assert_eq!(d.artboards[0].model, Some(model));
        assert_eq!(crate::Undoable::label(&inv), "Model");
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
        assert_eq!(
            DocCommand::SetModel {
                id: ArtboardId(9),
                model: None,
            }
            .apply(&mut d),
            Err(CmdError::ArtboardNotFound(ArtboardId(9)))
        );
        let json = serde_json::to_string(&inv).unwrap();
        assert_eq!(serde_json::from_str::<DocCommand>(&json).unwrap(), inv);
    }

    #[test]
    fn doc_commands_serialize() {
        let d = doc();
        let c = DocCommand::AddArtboard {
            index: 1,
            artboard: Box::new(second(&d)),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<DocCommand>(&json).unwrap(), c);
    }

    fn note(id: u32, text: &str) -> Note {
        Note {
            id,
            position: (5, -3),
            size: (90, 60),
            text: text.into(),
            color: NoteColor::Pink,
        }
    }

    #[test]
    fn add_set_and_remove_a_note_undo_one_by_one() {
        let mut d = doc();
        d.notes.push(note(1, "a"));
        let before = d.clone();
        // Agregar: en su lugar del z-order; el inverso la quita.
        let add = DocCommand::AddNote {
            index: 0,
            note: note(4, "b"),
        };
        assert_eq!(crate::Undoable::label(&add), "New Note");
        let inv = add.apply(&mut d).unwrap();
        assert_eq!(d.notes.iter().map(|n| n.id).collect::<Vec<_>>(), [4, 1]);
        assert_eq!(inv, DocCommand::RemoveNote { id: 4 });
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
        // Cambiar: la reemplaza por id; el inverso es la de antes.
        let mut changed = note(1, "otra");
        changed.position = (40, 2);
        let set = DocCommand::SetNote(changed.clone());
        assert_eq!(crate::Undoable::label(&set), "Note");
        let inv = set.apply(&mut d).unwrap();
        assert_eq!(d.notes, vec![changed]);
        assert_eq!(inv, DocCommand::SetNote(note(1, "a")));
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
        // Borrar: el inverso la vuelve a poner donde estaba.
        let remove = DocCommand::RemoveNote { id: 1 };
        assert_eq!(crate::Undoable::label(&remove), "Delete Note");
        let inv = remove.apply(&mut d).unwrap();
        assert!(d.notes.is_empty());
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
    }

    #[test]
    fn note_commands_check_ids_and_index_and_serialize() {
        let mut d = doc();
        d.notes.push(note(1, "a"));
        let before = d.clone();
        assert_eq!(
            DocCommand::AddNote {
                index: 0,
                note: note(1, "x"),
            }
            .apply(&mut d),
            Err(CmdError::DuplicateNoteId(1))
        );
        assert_eq!(
            DocCommand::AddNote {
                index: 2,
                note: note(2, "x"),
            }
            .apply(&mut d),
            Err(CmdError::IndexOutOfRange(2))
        );
        assert_eq!(
            DocCommand::SetNote(note(7, "x")).apply(&mut d),
            Err(CmdError::NoteNotFound(7))
        );
        assert_eq!(
            DocCommand::RemoveNote { id: 7 }.apply(&mut d),
            Err(CmdError::NoteNotFound(7))
        );
        assert_eq!(d, before);
        for c in [
            DocCommand::AddNote {
                index: 1,
                note: note(2, "línea 1\nlínea 2"),
            },
            DocCommand::SetNote(note(1, "b")),
            DocCommand::RemoveNote { id: 1 },
        ] {
            let json = serde_json::to_string(&c).unwrap();
            assert_eq!(serde_json::from_str::<DocCommand>(&json).unwrap(), c);
        }
    }

    #[test]
    fn note_ids_and_lookup() {
        let mut d = doc();
        assert_eq!(d.next_note_id(), 1);
        d.notes.push(note(3, "a"));
        d.notes.push(note(1, "b"));
        assert_eq!(d.next_note_id(), 4);
        assert_eq!(d.note(1).map(|n| n.text.as_str()), Some("b"));
        assert!(d.note(2).is_none());
    }
}
