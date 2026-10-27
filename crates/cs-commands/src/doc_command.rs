use cs_core::{Artboard, ArtboardId, Document, Guide, Note, Sprite, Symmetry};
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
    /// Reemplaza las guías de un artboard.
    SetGuides {
        id: ArtboardId,
        guides: Vec<Guide>,
    },
    SetSymmetry {
        id: ArtboardId,
        symmetry: Symmetry,
    },
    /// Reemplaza el sprite completo de un artboard (p. ej. al cambiar el modo de color).
    SetSprite {
        id: ArtboardId,
        sprite: Box<Sprite>,
    },
    /// Reemplaza las notas del documento (agregar, mover, editar o borrar una).
    SetNotes(Vec<Note>),
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
                if doc.artboards.len() == 1 {
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
            DocCommand::SetNotes(notes) => {
                let old = std::mem::replace(&mut doc.notes, notes.clone());
                Ok(DocCommand::SetNotes(old))
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
            DocCommand::SetGuides { .. } => "Guides",
            DocCommand::SetSymmetry { .. } => "Symmetry",
            DocCommand::SetSprite { .. } => "Sprite",
            DocCommand::SetNotes(_) => "Notes",
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
            guides: Vec::new(),
            symmetry: Default::default(),
            id: d.next_artboard_id(),
            name: "two".into(),
            position: (10, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        }
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
            id: ArtboardId(2),
            name: "b".into(),
            position: (10, 0),
            sprite: Sprite::new(4, 4, ColorMode::Rgba),
            guides: Vec::new(),
            symmetry: Default::default(),
        });
        let place = |doc: &Document, host: u32, source: u32| DocCommand::OnArtboard {
            id: ArtboardId(host),
            cmd: crate::ops::new_instance_layer(
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
        crate::ops::new_instance_layer(&s, None, 0, ArtboardId(source), "i", (0, 0))
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
    fn doc_commands_serialize() {
        let d = doc();
        let c = DocCommand::AddArtboard {
            index: 1,
            artboard: Box::new(second(&d)),
        };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<DocCommand>(&json).unwrap(), c);
    }
}
