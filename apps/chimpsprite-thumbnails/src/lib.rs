//! Miniaturas de los `.chimp` en el explorador de archivos del sistema.
//!
//! - **Windows:** `chimpsprite_thumbnails.dll`, una extensión del shell (`IThumbnailProvider`) que
//!   saca el `thumbnail.png` del zip. Se registra por usuario con `regsvr32` (ver
//!   `docs/thumbnails.md`).
//! - **macOS:** una función C (`chimp_thumbnail_png`) para la extensión de Quick Look (esqueleto en
//!   `macos/`).
//!
//! En Linux no hace falta: el thumbnailer llama a `chimpsprite --thumbnail`.

#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;
#[cfg(windows)]
mod shell;

#[cfg(windows)]
pub use shell::{CLSID, RegValue, ThumbnailProvider, registry_entries};
