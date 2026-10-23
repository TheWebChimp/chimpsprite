//! Lectura y escritura de archivos: `.aseprite`, `.chimp`, export e import.

pub mod aseprite;
mod bytes;
pub mod chimp;
pub mod engines;
mod error;
pub mod export;
pub mod files;
pub mod import;
pub mod palette_files;

pub use error::{IoError, Result};

/// Avisos no fatales al abrir un archivo (cosas que no se pudieron conservar).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    pub warnings: Vec<String>,
}

impl LoadReport {
    pub(crate) fn warn(&mut self, message: impl Into<String>) {
        let message = message.into();
        if !self.warnings.contains(&message) {
            log::warn!("{message}");
            self.warnings.push(message);
        }
    }
}
