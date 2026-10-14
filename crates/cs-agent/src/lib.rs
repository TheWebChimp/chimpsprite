//! Operaciones de ChimpSprite para agentes (servidor MCP): una sesión con documentos abiertos y
//! herramientas semánticas que se traducen a `DocCommand` (con undo), más lectura de píxeles como
//! texto y render a PNG. No sabe nada del protocolo.

mod error;
mod ops;
pub mod params;
mod quantize;
mod session;
mod target;
mod text;

pub use error::{AgentError, Result};
pub use session::{Reply, Session};
