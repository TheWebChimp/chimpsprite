use cs_core::{Artboard, ArtboardId, Document};
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
                Ok(DocCommand::RemoveArtboard { id: artboard.id })
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
            id: d.next_artboard_id(),
            name: "two".into(),
            position: (10, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        }
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
