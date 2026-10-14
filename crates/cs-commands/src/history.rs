use crate::CmdError;

/// Algo que se aplica sobre un objetivo y devuelve el comando que lo deshace.
pub trait Undoable: Sized {
    type Target;

    fn apply(&self, target: &mut Self::Target) -> Result<Self, CmdError>;
}

/// Pilas de undo/redo. Cada entrada es el comando que deshace (o rehace) un paso.
#[derive(Debug)]
pub struct History<C> {
    undo: Vec<C>,
    redo: Vec<C>,
}

impl<C> Default for History<C> {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }
}

impl<C: Undoable> History<C> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, target: &mut C::Target, cmd: C) -> Result<(), CmdError> {
        let inverse = cmd.apply(target)?;
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Devuelve `false` si no había nada que deshacer.
    pub fn undo(&mut self, target: &mut C::Target) -> Result<bool, CmdError> {
        Self::step(&mut self.undo, &mut self.redo, target)
    }

    /// Devuelve `false` si no había nada que rehacer.
    pub fn redo(&mut self, target: &mut C::Target) -> Result<bool, CmdError> {
        Self::step(&mut self.redo, &mut self.undo, target)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn step(from: &mut Vec<C>, to: &mut Vec<C>, target: &mut C::Target) -> Result<bool, CmdError> {
        let Some(cmd) = from.pop() else {
            return Ok(false);
        };
        match cmd.apply(target) {
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
