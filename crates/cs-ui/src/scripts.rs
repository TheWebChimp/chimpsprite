//! File → Scripts: los scripts de Rhai y Lua (`cs-script`) de la carpeta del usuario, o uno
//! elegido con Run Script File…. Corren sobre una copia del documento; lo que dejan es un paso
//! de undo.

use cs_commands::DocCommand;
use cs_script::{Language, ScriptError};

use crate::editor::Editor;
use crate::platform::FileFilter;

/// Los archivos que se pueden elegir con Run Script File… (en la web, solo Rhai).
pub const SCRIPT_FILTER: &[FileFilter] = &[FileFilter {
    name: "Scripts",
    extensions: if cfg!(target_arch = "wasm32") {
        &["rhai"]
    } else {
        &["rhai", "lua"]
    },
}];

impl Editor {
    /// Corre `source` sobre el documento, con el artboard, la capa y el frame de ahora. Si
    /// termina bien y cambió algo, queda como un paso de undo. Devuelve lo que imprimió.
    pub fn run_script(&mut self, language: Language, source: &str) -> Result<String, ScriptError> {
        self.commit_floating();
        let context = cs_script::Context {
            artboard: self.active.0,
            layer: self.layer.0,
            frame: self.frame,
        };
        let out = cs_script::run(language, source, self.document.clone(), context)?;
        if out.document != self.document {
            self.execute_doc(DocCommand::SetDocument(Box::new(out.document)));
        }
        Ok(out.output)
    }
}

/// La carpeta de los scripts del usuario (solo escritorio).
#[cfg(not(target_arch = "wasm32"))]
pub fn folder() -> Option<std::path::PathBuf> {
    eframe::storage_dir("ChimpSprite").map(|d| d.join("scripts"))
}

/// Los scripts de la carpeta, por nombre. La primera vez la crea con los ejemplos.
#[cfg(not(target_arch = "wasm32"))]
pub fn list() -> Vec<std::path::PathBuf> {
    let Some(dir) = folder() else {
        return Vec::new();
    };
    if !dir.exists() && std::fs::create_dir_all(&dir).is_ok() {
        for (name, source) in cs_script::EXAMPLES {
            let _ = std::fs::write(dir.join(name), source);
        }
    }
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(Language::of)
                .is_some()
        })
        .collect();
    files.sort();
    files
}

/// Cómo se ve un script en el menú: su nombre y el lenguaje.
#[cfg(any(test, not(target_arch = "wasm32")))]
pub fn menu_label(file_name: &str) -> String {
    let (stem, _) = file_name.rsplit_once('.').unwrap_or((file_name, ""));
    let name = stem.replace('_', " ");
    match Language::of(file_name) {
        Some(Language::Lua) => format!("{name} (Lua)"),
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, PixelValue, Rgba, Sprite};

    use super::*;

    #[test]
    fn a_script_paints_the_open_document_in_one_undo_step() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let steps = e.steps_done();
        let out = e
            .run_script(
                Language::Rhai,
                r##"draw_pixels(#{pixels: [#{x: 1, y: 1, color: "#ff0000"}]}); print("done");"##,
            )
            .expect("runs");
        assert_eq!(out, "done");
        assert_eq!(
            e.sprite().pixel(e.layer, 0, 1, 1),
            PixelValue::Rgba(Rgba::rgb(255, 0, 0))
        );
        assert_eq!(e.steps_done(), steps + 1);
        assert_eq!(e.history.done().last(), Some("Script"));
        e.undo();
        assert_eq!(
            e.sprite().pixel(e.layer, 0, 1, 1),
            e.sprite().color_mode.transparent()
        );
        // Un script que no cambia nada no deja paso; uno que falla, tampoco.
        e.run_script(Language::Rhai, "print(\"hi\");").unwrap();
        assert!(e.run_script(Language::Rhai, "draw_line(#{});").is_err());
        assert_eq!(e.steps_done(), steps);
    }

    #[test]
    fn scripts_show_their_name_and_language() {
        assert_eq!(menu_label("walk_cycle_tag.rhai"), "walk cycle tag");
        assert_eq!(menu_label("walk_cycle_tag.lua"), "walk cycle tag (Lua)");
    }
}
