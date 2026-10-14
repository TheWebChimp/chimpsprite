//! Motor de comandos: toda mutación del documento pasa por aquí.

mod command;
mod draw;
mod history;

pub use command::{CmdError, Command, LayerProps};
pub use history::History;
