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

    /// Aplica `cmd` como continuación del paso anterior (p. ej. un arrastre): si hay un paso para
    /// deshacer, se conserva su inverso y se descarta el nuevo, así un solo undo vuelve al estado
    /// de antes de todo el gesto. Solo es correcto si el inverso anterior restaura todo lo que
    /// `cmd` toca (comandos que reemplazan un estado completo, como `SetInstanceTrack`).
    pub fn execute_merged(&mut self, target: &mut C::Target, cmd: C) -> Result<(), CmdError> {
        let inverse = cmd.apply(target)?;
        if self.undo.is_empty() {
            self.undo.push(inverse);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Reemplaza el valor entero.
    struct Set(i32);

    impl Undoable for Set {
        type Target = i32;

        fn apply(&self, target: &mut i32) -> Result<Self, CmdError> {
            Ok(Set(std::mem::replace(target, self.0)))
        }
    }

    #[test]
    fn merged_steps_undo_together() {
        let mut v = 0;
        let mut h = History::new();
        h.execute(&mut v, Set(1)).unwrap();
        h.execute(&mut v, Set(2)).unwrap();
        h.execute_merged(&mut v, Set(3)).unwrap();
        h.execute_merged(&mut v, Set(4)).unwrap();
        assert_eq!(v, 4);
        assert!(h.undo(&mut v).unwrap());
        assert_eq!(v, 1, "one undo goes back to before the whole gesture");
        assert!(h.redo(&mut v).unwrap());
        assert_eq!(v, 4);
        // Sin paso previo, el primero merged crea uno.
        let mut w = 0;
        let mut h = History::new();
        for i in 1..4 {
            h.execute_merged(&mut w, Set(i)).unwrap();
        }
        assert!(h.undo(&mut w).unwrap());
        assert_eq!(w, 0);
        assert!(!h.can_undo());
    }
}
