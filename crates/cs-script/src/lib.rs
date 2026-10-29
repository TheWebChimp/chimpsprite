//! Scripts de ChimpSprite, en Rhai (escritorio y web) o Lua (escritorio). Los dos llaman a las
//! mismas operaciones que el agente (`cs_agent::Session::call`), sobre una copia del documento
//! abierto: si el script termina bien, la app deja el resultado como un solo paso de undo.
//!
//! En los dos lenguajes cada operación es una función global con sus parámetros en un mapa
//! (`draw_line(#{points: [[0, 0], [7, 7]], color: "#ff0000"})` en Rhai,
//! `draw_line{points = {{0, 0}, {7, 7}}, color = "#ff0000"}` en Lua), además de
//! `op(nombre, parámetros)`. El documento, el artboard, la capa y el frame actuales van solos si
//! no se dicen. `print` junta lo que se muestra al terminar.

mod host;
#[cfg(not(target_arch = "wasm32"))]
mod lua;
mod rhai_engine;

use cs_core::Document;

pub use host::Context;

/// Los scripts de ejemplo, en los dos lenguajes: (nombre de archivo, código). La app los copia a
/// la carpeta de scripts la primera vez.
pub const EXAMPLES: &[(&str, &str)] = &[
    ("outline.rhai", include_str!("../scripts/outline.rhai")),
    ("outline.lua", include_str!("../scripts/outline.lua")),
    (
        "color_ramp.rhai",
        include_str!("../scripts/color_ramp.rhai"),
    ),
    ("color_ramp.lua", include_str!("../scripts/color_ramp.lua")),
    (
        "export_tags.rhai",
        include_str!("../scripts/export_tags.rhai"),
    ),
    (
        "export_tags.lua",
        include_str!("../scripts/export_tags.lua"),
    ),
    (
        "checkerboard.rhai",
        include_str!("../scripts/checkerboard.rhai"),
    ),
    (
        "checkerboard.lua",
        include_str!("../scripts/checkerboard.lua"),
    ),
    (
        "mirror_all_frames.rhai",
        include_str!("../scripts/mirror_all_frames.rhai"),
    ),
    (
        "mirror_all_frames.lua",
        include_str!("../scripts/mirror_all_frames.lua"),
    ),
    (
        "walk_cycle_tag.rhai",
        include_str!("../scripts/walk_cycle_tag.rhai"),
    ),
    (
        "walk_cycle_tag.lua",
        include_str!("../scripts/walk_cycle_tag.lua"),
    ),
];

/// Tope de instrucciones de un script: un ciclo sin fin no traba la app.
pub const MAX_OPERATIONS: u64 = 50_000_000;

/// El lenguaje de un script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Rhai,
    Lua,
}

impl Language {
    /// Por la extensión del archivo (`.rhai` o `.lua`).
    pub fn of(file_name: &str) -> Option<Self> {
        let ext = file_name.rsplit('.').next()?.to_ascii_lowercase();
        match ext.as_str() {
            "rhai" => Some(Language::Rhai),
            "lua" => Some(Language::Lua),
            _ => None,
        }
    }

    /// Si se puede usar aquí (Lua no corre en la web).
    pub fn available(self) -> bool {
        match self {
            Language::Rhai => true,
            Language::Lua => cfg!(not(target_arch = "wasm32")),
        }
    }
}

/// Lo que dejó un script que terminó bien: el documento y lo que imprimió.
#[derive(Debug)]
pub struct Outcome {
    pub document: Document,
    pub output: String,
}

/// Un script que falló: el error (con la línea) y lo que alcanzó a imprimir.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ScriptError {
    pub message: String,
    pub output: String,
}

/// Corre `source` en `language` sobre `document` (una copia: el original no cambia).
pub fn run(
    language: Language,
    source: &str,
    document: Document,
    context: Context,
) -> Result<Outcome, ScriptError> {
    let host = host::Host::new(document, context);
    match language {
        Language::Rhai => rhai_engine::run(source, host),
        #[cfg(not(target_arch = "wasm32"))]
        Language::Lua => lua::run(source, host),
        #[cfg(target_arch = "wasm32")]
        Language::Lua => Err(ScriptError {
            message: "Lua scripts only run in the desktop app".into(),
            output: String::new(),
        }),
    }
}
