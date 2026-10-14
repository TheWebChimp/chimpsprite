use cs_core::Sprite;

use crate::{CmdError, Command};

/// Pilas de undo/redo. Cada entrada es el comando que deshace (o rehace) un paso.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, doc: &mut Sprite, cmd: Command) -> Result<(), CmdError> {
        let inverse = cmd.apply(doc)?;
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Devuelve `false` si no había nada que deshacer.
    pub fn undo(&mut self, doc: &mut Sprite) -> Result<bool, CmdError> {
        Self::step(&mut self.undo, &mut self.redo, doc)
    }

    /// Devuelve `false` si no había nada que rehacer.
    pub fn redo(&mut self, doc: &mut Sprite) -> Result<bool, CmdError> {
        Self::step(&mut self.redo, &mut self.undo, doc)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn step(
        from: &mut Vec<Command>,
        to: &mut Vec<Command>,
        doc: &mut Sprite,
    ) -> Result<bool, CmdError> {
        let Some(cmd) = from.pop() else {
            return Ok(false);
        };
        match cmd.apply(doc) {
            Ok(opposite) => {
                to.push(opposite);
                Ok(true)
            }
            Err(e) => {
                from.push(cmd);
                Err(e)
            }
        }
    }
}
