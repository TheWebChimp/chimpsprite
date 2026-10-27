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
    /// `export_folder`: lo que da `export_folder()` (la carpeta del documento).
    pub fn run_script(
        &mut self,
        language: Language,
        source: &str,
        export_folder: Option<String>,
    ) -> Result<String, ScriptError> {
        self.commit_floating();
        // Cada corrida con otra semilla para `random()`.
        static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let context = cs_script::Context {
            artboard: self.active.0,
            layer: self.layer.0,
            frame: self.frame,
            primary: self.resolve(self.primary),
            secondary: self.resolve(self.secondary),
            selection: self.selection.as_ref().map(|m| m.points().collect()),
            seed: (self.doc_version ^ run.rotate_left(32)).wrapping_mul(0x9e37_79b9_7f4a_7c15),
            export_folder,
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

/// Los scripts de la carpeta, por nombre. Agrega los ejemplos que todavía no ofreció (los que
/// se borran no vuelven: `.examples` lleva la cuenta).
#[cfg(not(target_arch = "wasm32"))]
pub fn list() -> Vec<std::path::PathBuf> {
    let Some(dir) = folder() else {
        return Vec::new();
    };
    offer_examples(&dir);
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

/// Copia a `dir` los ejemplos que nunca se ofrecieron ahí.
#[cfg(not(target_arch = "wasm32"))]
fn offer_examples(dir: &std::path::Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let ledger = dir.join(".examples");
    let offered = std::fs::read_to_string(&ledger).unwrap_or_default();
    let offered: std::collections::HashSet<&str> = offered.lines().collect();
    let new: Vec<&str> = cs_script::EXAMPLES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !offered.contains(name))
        .collect();
    if new.is_empty() {
        return;
    }
    for (name, source) in cs_script::EXAMPLES.iter().filter(|(n, _)| new.contains(n)) {
        let path = dir.join(name);
        if !path.exists() {
            let _ = std::fs::write(path, source);
        }
    }
    let all: Vec<&str> = offered.iter().copied().chain(new).collect();
    let _ = std::fs::write(
        ledger,
        all.join(
            "
",
        ),
    );
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
                None,
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
        e.run_script(Language::Rhai, "print(\"hi\");", None)
            .unwrap();
        assert!(
            e.run_script(Language::Rhai, "draw_line(#{});", None)
                .is_err()
        );
        assert_eq!(e.steps_done(), steps);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn examples_are_offered_once_and_deleted_ones_stay_deleted() {
        let dir = std::env::temp_dir().join(format!("cs-ui-examples-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        offer_examples(&dir);
        let (first, _) = cs_script::EXAMPLES[0];
        assert!(dir.join(first).exists());
        std::fs::remove_file(dir.join(first)).unwrap();
        offer_examples(&dir);
        assert!(!dir.join(first).exists(), "deleted stays deleted");
        // Un ejemplo nuevo (que el registro no tiene) sí llega.
        let (last, _) = cs_script::EXAMPLES[cs_script::EXAMPLES.len() - 1];
        std::fs::remove_file(dir.join(last)).unwrap();
        let ledger = std::fs::read_to_string(dir.join(".examples")).unwrap();
        let without: Vec<&str> = ledger.lines().filter(|l| *l != last).collect();
        std::fs::write(
            dir.join(".examples"),
            without.join(
                "
",
            ),
        )
        .unwrap();
        offer_examples(&dir);
        assert!(dir.join(last).exists(), "a new example arrives");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scripts_show_their_name_and_language() {
        assert_eq!(menu_label("walk_cycle_tag.rhai"), "walk cycle tag");
        assert_eq!(menu_label("walk_cycle_tag.lua"), "walk cycle tag (Lua)");
    }
}
