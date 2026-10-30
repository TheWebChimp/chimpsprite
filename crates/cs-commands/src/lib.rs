//! Motor de comandos: toda mutación del documento pasa por aquí.

mod command;
mod doc_command;
mod draw;
mod history;
pub mod ops;
pub mod sprite_ops;
pub mod symbol;

pub use command::{CmdError, Command, LayerProps};
pub use doc_command::DocCommand;
pub use history::{History, Undoable};
