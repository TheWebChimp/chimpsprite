//! El bloqueo de estructura: las fuentes y los modelos 3D no cambian de frames ni capas (ni de
//! tamaño) con los comandos de siempre, solo con los suyos (que traen su `SetFont` o `SetModel`).

use cs_core::{Artboard, ArtboardId, Document};

use crate::{CmdError, Command, DocCommand, History};

/// Lo que no puede cambiar de las fuentes y los modelos de un documento con los comandos de
/// siempre: sus frames y capas (solo con los comandos que traen la fuente o el modelo nuevo),
/// lo que pide su `check` y su tamaño. Se toma antes de un comando y se comprueba después
/// ([`StructureLock::execute`]).
#[derive(Clone, Debug, Default)]
pub struct StructureLock(Vec<(ArtboardId, (u32, u32))>);

/// Si el artboard es una fuente o un modelo bien armado (uno mal armado no se traba).
fn well_formed(a: &Artboard) -> bool {
    a.font.as_ref().is_none_or(|f| f.check(&a.sprite).is_ok())
        && a.model.as_ref().is_none_or(|m| m.check(&a.sprite).is_ok())
}

impl StructureLock {
    /// Las fuentes y los modelos bien armados de `doc` con su tamaño.
    pub fn of(doc: &Document) -> StructureLock {
        StructureLock(
            doc.artboards
                .iter()
                .filter(|a| (a.font.is_some() || a.model.is_some()) && well_formed(a))
                .map(|a| (a.id, (a.sprite.width, a.sprite.height)))
                .collect(),
        )
    }

    fn locked(&self, id: ArtboardId) -> bool {
        self.0.iter().any(|(a, _)| *a == id)
    }

    /// Antes de aplicar: `cmd` no agrega, quita ni mueve frames o capas de una fuente o un
    /// modelo, salvo que traiga también la fuente o el modelo nuevo de ese artboard.
    pub fn allows(&self, cmd: &DocCommand) -> Result<(), CmdError> {
        let mut escape = Vec::new();
        sets_structure(cmd, &mut escape);
        self.allows_in(cmd, &escape)
    }

    fn allows_in(&self, cmd: &DocCommand, escape: &[ArtboardId]) -> Result<(), CmdError> {
        match cmd {
            DocCommand::OnArtboard { id, cmd }
                if self.locked(*id) && !escape.contains(id) && moves_structure(cmd) =>
            {
                Err(CmdError::StructureLocked(*id))
            }
            DocCommand::Batch(cmds) => cmds.iter().try_for_each(|c| self.allows_in(c, escape)),
            _ => Ok(()),
        }
    }

    /// Después de aplicar: cada fuente o modelo que sigue siéndolo está bien armado y con su
    /// tamaño.
    pub fn holds(&self, doc: &Document) -> Result<(), CmdError> {
        for &(id, size) in &self.0 {
            let Some(a) = doc.artboard(id) else {
                continue;
            };
            if a.font.is_none() && a.model.is_none() {
                continue;
            }
            if !well_formed(a) || (a.sprite.width, a.sprite.height) != size {
                return Err(CmdError::StructureLocked(id));
            }
        }
        Ok(())
    }

    /// Ejecuta `cmd` en `history` (como continuación del paso anterior con `merged`) solo si
    /// deja las fuentes y los modelos de `doc` como estaban armados; si no, nada cambia.
    pub fn execute(
        history: &mut History<DocCommand>,
        doc: &mut Document,
        cmd: DocCommand,
        merged: bool,
    ) -> Result<(), CmdError> {
        let lock = StructureLock::of(doc);
        lock.allows(&cmd)?;
        history.execute_checked(doc, cmd, merged, |d| lock.holds(d))
    }
}

/// Los artboards a los que `cmd` les pone fuente o modelo.
fn sets_structure(cmd: &DocCommand, out: &mut Vec<ArtboardId>) {
    match cmd {
        DocCommand::SetFont { id, font: Some(_) } | DocCommand::SetModel { id, model: Some(_) } => {
            out.push(*id)
        }
        DocCommand::Batch(cmds) => cmds.iter().for_each(|c| sets_structure(c, out)),
        _ => {}
    }
}

/// Si `cmd` agrega, quita o mueve frames o capas.
fn moves_structure(cmd: &Command) -> bool {
    match cmd {
        Command::AddFrame { .. }
        | Command::RemoveFrame { .. }
        | Command::MoveFrame { .. }
        | Command::AddLayer { .. }
        | Command::RemoveLayer { .. }
        | Command::MoveLayer { .. } => true,
        Command::Batch(cmds) => cmds.iter().any(moves_structure),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::new_model_artboard;
    use crate::{Command, History};
    use cs_core::{ColorMode, Document, Frame, Layer, LayerId, Sprite};

    fn doc_with_model() -> (Document, ArtboardId, ArtboardId) {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (model, texture, cmd) = new_model_artboard(&doc, "Box", (32, 32), (40, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, model, texture)
    }

    #[test]
    fn the_lock_refuses_frames_and_layers_on_a_model_but_not_on_a_plain_artboard() {
        let (mut doc, model, texture) = doc_with_model();
        let mut history = History::new();
        let before = doc.clone();
        let add_frame = |id| DocCommand::OnArtboard {
            id,
            cmd: Command::AddFrame {
                index: 0,
                frame: Frame { duration_ms: 100 },
            },
        };
        let add_layer = |id| DocCommand::OnArtboard {
            id,
            cmd: Command::AddLayer {
                index: 1,
                layer: Layer::new_image(LayerId(9), "extra"),
            },
        };
        for cmd in [add_frame(model), add_layer(model)] {
            assert_eq!(
                StructureLock::execute(&mut history, &mut doc, cmd, false),
                Err(CmdError::StructureLocked(model))
            );
        }
        assert_eq!(doc, before);
        assert!(!history.can_undo());
        // Un cambio de tamaño (sin tocar frames ni capas) se nota después.
        let resize = DocCommand::OnArtboard {
            id: model,
            cmd: Command::ResizeCanvas {
                width: 8,
                height: 8,
                offset: (0, 0),
            },
        };
        assert_eq!(
            StructureLock::execute(&mut history, &mut doc, resize, false),
            Err(CmdError::StructureLocked(model))
        );
        assert_eq!(doc, before);
        // La textura y el artboard normal no se traban.
        for id in [ArtboardId(1), texture] {
            assert!(StructureLock::execute(&mut history, &mut doc, add_frame(id), false).is_ok());
            assert!(StructureLock::execute(&mut history, &mut doc, add_layer(id), false).is_ok());
        }
    }

    #[test]
    fn a_command_that_brings_the_model_passes() {
        let (mut doc, model, _) = doc_with_model();
        let mut history = History::new();
        let m = doc.artboard(model).unwrap().model.clone().unwrap();
        let cmd = DocCommand::Batch(vec![
            DocCommand::OnArtboard {
                id: model,
                cmd: Command::MoveFrame { from: 0, to: 0 },
            },
            DocCommand::SetModel {
                id: model,
                model: Some(Box::new(m)),
            },
        ]);
        assert!(StructureLock::of(&doc).allows(&cmd).is_ok());
        assert!(StructureLock::execute(&mut history, &mut doc, cmd, false).is_ok());
    }
}
