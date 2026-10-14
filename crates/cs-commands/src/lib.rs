//! Motor de comandos: toda mutación del documento pasa por aquí.

mod command;
mod doc_command;
mod draw;
mod history;

pub use command::{CmdError, Command, LayerProps};
pub use doc_command::DocCommand;
pub use history::{History, Undoable};
