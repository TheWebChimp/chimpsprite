//! Acciones de archivo de la app: nuevo, abrir, guardar, importar, exportar, pestañas y cambios
//! sin guardar.

use eframe::egui;

use crate::editor::Editor;
use crate::export_dialog::ExportResult;
use crate::files::{self, FileError, FileKind};
use crate::platform::{FileFilter, OpenPurpose, PlatformEvent, SaveTarget};
use crate::tabs::DocState;
use crate::{ChimpApp, demo};

pub(crate) const SPRITE_FILTERS: &[FileFilter] = &[
    FileFilter {
        name: "Sprites",
        extensions: &["chimp", "aseprite", "ase", "png"],
    },
    FileFilter {
        name: "ChimpSprite",
        extensions: &["chimp"],
    },
    FileFilter {
        name: "Aseprite",
        extensions: &["aseprite", "ase"],
    },
    FileFilter {
        name: "PNG",
        extensions: &["png"],
    },
];

const PNG_FILTER: &[FileFilter] = &[FileFilter {
    name: "PNG",
    extensions: &["png"],
}];

/// Un archivo ya leído.
pub(crate) struct Loaded {
    pub name: String,
    pub target: Option<SaveTarget>,
    pub bytes: Vec<u8>,
}

/// Acción en espera de que el usuario decida qué hacer con los cambios sin guardar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    CloseTab(usize),
    Quit,
}

pub(crate) struct Message {
    pub title: String,
    pub body: String,
}

/// Nombre y destino del documento abierto.
#[derive(Clone, Debug)]
pub(crate) struct DocLocation {
    pub name: String,
    /// Dónde guardar con "Save" sin preguntar (`None` si nunca se guardó).
    pub target: Option<SaveTarget>,
}

impl Default for DocLocation {
    fn default() -> Self {
        Self {
            name: "Untitled".into(),
            target: None,
        }
    }
}

impl ChimpApp {
    /// Hay cambios sin guardar en alguna pestaña.
    pub(crate) fn any_dirty(&self) -> bool {
        !self.tabs.dirty(&self.doc).is_empty()
    }

    pub(crate) fn is_modal(&self) -> bool {
        self.canvas_size.is_some()
            || self.grid_dialog.is_some()
            || self.export.is_some()
            || self.pending.is_some()
            || self.message.is_some()
            || self.restore.is_some()
    }

    pub(crate) fn show_message(&mut self, title: &str, body: impl Into<String>) {
        self.message = Some(Message {
            title: title.into(),
            body: body.into(),
        });
    }

    /// Abre `doc` en una pestaña nueva (o en lugar de un Untitled intacto).
    fn open_doc(&mut self, doc: DocState) {
        self.canvas_size = None;
        self.tabs.open(&mut self.doc, doc);
    }

    pub(crate) fn new_document(&mut self) {
        let name = self.tabs.untitled_name(&self.doc);
        self.open_doc(DocState::untitled(name));
    }

    pub(crate) fn open_demo(&mut self) {
        self.open_doc(DocState::new(
            demo::editor(),
            DocLocation {
                name: "Demo".into(),
                target: None,
            },
        ));
    }

    pub(crate) fn open_dialog(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(SPRITE_FILTERS, OpenPurpose::Open, ctx);
    }

    /// Cierra una pestaña, preguntando antes si tiene cambios.
    pub(crate) fn request_close_tab(&mut self, index: usize) {
        if self.tabs.dirty(&self.doc).contains(&index) {
            self.pending = Some(Pending::CloseTab(index));
        } else {
            self.close_tab(index);
        }
    }

    fn close_tab(&mut self, index: usize) {
        if self.tabs.active() == Some(index) {
            self.canvas_size = None;
        }
        self.tabs.close(&mut self.doc, index);
    }

    pub(crate) fn request_quit(&mut self, ctx: &egui::Context) {
        if self.any_dirty() {
            self.pending = Some(Pending::Quit);
        } else {
            self.quit(ctx);
        }
    }

    fn quit(&mut self, ctx: &egui::Context) {
        self.allow_close = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    /// Procesa lo que llegó de la plataforma (archivos abiertos o errores).
    pub(crate) fn handle_platform_events(&mut self, _ctx: &egui::Context) {
        for event in self.platform.poll() {
            match event {
                PlatformEvent::Opened {
                    name,
                    target,
                    bytes,
                    purpose,
                } => {
                    let loaded = Loaded {
                        name,
                        target,
                        bytes,
                    };
                    match purpose {
                        OpenPurpose::Open | OpenPurpose::Dropped => self.load(loaded),
                        OpenPurpose::ImportLayer if self.has_doc() => {
                            self.import_layer_bytes(&loaded)
                        }
                        OpenPurpose::ImportLayer => {}
                    }
                }
                PlatformEvent::Error(e) => self.show_message("Could not read the file", e),
            }
        }
    }

    fn load(&mut self, loaded: Loaded) {
        // Si ya está abierto, solo cambia a esa pestaña.
        if let Some(path) = loaded.target.as_ref().and_then(|t| t.path())
            && let Some(i) = self.tabs.find_path(&self.doc, &path)
        {
            self.tabs.switch(&mut self.doc, i);
            return;
        }
        match files::decode(&loaded.name, &loaded.bytes) {
            Ok((doc, report)) => {
                // Un PNG se abre como sprite nuevo: guardarlo pide elegir .chimp o .aseprite.
                let is_png = files::kind_of(&loaded.name) == Some(FileKind::Png);
                if let Some(path) = loaded.target.as_ref().and_then(|t| t.path()) {
                    self.recent.add(&path);
                }
                let location = DocLocation {
                    name: if is_png {
                        format!("{}.chimp", files::stem(&loaded.name))
                    } else {
                        loaded.name.clone()
                    },
                    target: loaded.target.filter(|_| !is_png),
                };
                self.open_doc(DocState::new(Editor::new(doc), location));
                if !report.warnings.is_empty() {
                    let body = report
                        .warnings
                        .iter()
                        .map(|w| format!("• {w}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.show_message("Opened with warnings", body);
                }
            }
            Err(e) => self.show_message("Could not open the file", e.to_string()),
        }
    }

    /// Guarda en el archivo actual (o pregunta dónde). Devuelve si se guardó.
    pub(crate) fn save(&mut self) -> bool {
        match self.doc.location.target.clone() {
            Some(target) => self.save_to(target),
            None => self.save_as(),
        }
    }

    pub(crate) fn save_as(&mut self) -> bool {
        let multiple = self.doc.editor.document.artboards.len() > 1;
        let aseprite =
            files::kind_of(&self.doc.location.name) == Some(FileKind::Aseprite) && !multiple;
        let ext = if aseprite { "aseprite" } else { "chimp" };
        let stem = match &self.doc.location.target {
            Some(t) => t.stem().to_string(),
            None if !self.doc.location.name.starts_with("Untitled") => {
                files::stem(&self.doc.location.name).to_string()
            }
            None => self.doc.editor.artboard().name.clone(),
        };
        let chimp = FileFilter {
            name: "ChimpSprite",
            extensions: &["chimp"],
        };
        let ase = FileFilter {
            name: "Aseprite (one artboard)",
            extensions: &["aseprite", "ase"],
        };
        let filters = if aseprite { [ase, chimp] } else { [chimp, ase] };
        match self
            .platform
            .choose_save(&format!("{stem}.{ext}"), &filters)
        {
            Some(target) => self.save_to(target),
            None => false,
        }
    }

    fn save_to(&mut self, target: SaveTarget) -> bool {
        let result =
            files::encode(&self.doc.editor.document, &target.file_name).and_then(|bytes| {
                self.platform
                    .write(&target, &bytes)
                    .map_err(FileError::Platform)
            });
        let target_name = target.file_name.clone();
        match result {
            Ok(()) => {
                if let Some(path) = target.path() {
                    self.recent.add(&path);
                }
                self.doc.location = DocLocation {
                    name: target.file_name.clone(),
                    target: Some(target),
                };
                self.doc.saved_version = self.doc.editor.doc_version;
                let lost_guides = files::kind_of(&self.doc.location.name)
                    == Some(FileKind::Aseprite)
                    && self
                        .doc
                        .editor
                        .document
                        .artboards
                        .iter()
                        .any(|a| !a.guides.is_empty());
                let baked = files::bakes_masks(&self.doc.editor.document, &target_name);
                if baked {
                    self.show_message(
                        "Masks applied in the file",
                        ".aseprite files have no layer or clipping masks, so they were applied to                          the pixels in the saved file (your open document keeps them). Save as                          .chimp to keep them editable.",
                    );
                } else if lost_guides {
                    self.show_message(
                        "Saved without guides",
                        ".aseprite files can't store guides. Save as .chimp to keep them.",
                    );
                }
                true
            }
            Err(e @ FileError::MultipleArtboards(_)) => {
                self.show_message("Save as .chimp", e.to_string());
                false
            }
            Err(e) => {
                self.show_message("Could not save", e.to_string());
                false
            }
        }
    }

    pub(crate) fn import_layer(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(PNG_FILTER, OpenPurpose::ImportLayer, ctx);
    }

    fn import_layer_bytes(&mut self, loaded: &Loaded) {
        let name = files::stem(&loaded.name).to_string();
        let result = cs_io::import::import_png_layer(
            self.doc.editor.sprite(),
            &loaded.bytes,
            self.doc.editor.frame,
            &name,
        );
        match result {
            Ok(cmd) => {
                if self.doc.editor.execute(cmd)
                    && let Some(top) = self.doc.editor.sprite().layers.last()
                {
                    let id = top.id;
                    self.doc.editor.select_layer(id);
                }
            }
            Err(e) => self.show_message("Could not import", e.to_string()),
        }
    }

    pub(crate) fn open_export(&mut self) {
        self.export = Some(self.last_export.clone());
    }

    /// Archivos soltados sobre la ventana: abre el primero en una pestaña nueva.
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().cloned());
        if let Some(file) = dropped
            && !self.is_modal()
        {
            self.platform.open_dropped(&file, ctx);
        }
    }

    /// Cerrar la ventana con cambios sin guardar pregunta primero.
    pub(crate) fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.any_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
    }

    /// Guarda en el navegador las pestañas con cambios cada tanto; limpia si no queda ninguna.
    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let dirty = self.tabs.dirty(&self.doc);
        // Huella de versiones: cambia si cualquier pestaña cambia.
        let fingerprint = self
            .tabs
            .docs(&self.doc)
            .iter()
            .enumerate()
            .fold(0u64, |acc, (i, d)| {
                acc.wrapping_mul(1_000_003)
                    .wrapping_add(d.editor.doc_version ^ ((i as u64) << 48))
            });
        if dirty.is_empty() {
            if self.autosaver.due(now, fingerprint, true) {
                crate::autosave::storage::clear();
                self.autosaver.mark_saved(now, fingerprint);
            }
            return;
        }
        if !self.autosaver.due(now, fingerprint, true) {
            return;
        }
        let docs = self.tabs.docs(&self.doc);
        let entries: Vec<_> = dirty
            .iter()
            .map(|&i| (docs[i].location.name.as_str(), &docs[i].editor.document))
            .collect();
        let result = crate::autosave::encode(&entries)
            .and_then(|text| crate::autosave::storage::store(&text));
        match result {
            Ok(()) => self.autosaver.mark_saved(now, fingerprint),
            Err(e) => {
                log::warn!("autosave disabled: {e}");
                self.autosaver.disabled_reason = Some(e);
            }
        }
    }

    /// Pregunta si restaurar las sesiones guardadas en el navegador.
    pub(crate) fn show_restore(&mut self, ctx: &egui::Context) {
        let Some(restore) = &self.restore else {
            return;
        };
        let mut choice = None;
        egui::Window::new("Restore last session")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label("These documents have unsaved changes from your last visit:");
                for r in restore {
                    ui.label(format!("• {}", r.name));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Restore").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(false);
                    }
                });
            });
        match choice {
            Some(true) => {
                for r in self.restore.take().expect("restore") {
                    let mut doc = DocState::new(
                        Editor::new(r.doc),
                        DocLocation {
                            name: r.name,
                            target: None,
                        },
                    );
                    // Lo restaurado no está guardado en ningún archivo.
                    doc.saved_version = u64::MAX;
                    self.open_doc(doc);
                }
            }
            Some(false) => {
                self.restore = None;
                crate::autosave::storage::clear();
            }
            None => {}
        }
    }

    pub(crate) fn update_title(&mut self, ctx: &egui::Context) {
        let title = if self.has_doc() {
            format!(
                "{}{} — ChimpSprite",
                self.doc.location.name,
                if self.doc.is_dirty() { " •" } else { "" }
            )
        } else {
            "ChimpSprite".to_string()
        };
        if title != self.title {
            crate::platform::set_title(ctx, &title);
            self.title = title;
        }
    }

    /// Ventanas modales: cambios sin guardar, mensajes y exportación.
    pub(crate) fn show_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(pending) = self.pending {
            self.unsaved_dialog(ctx, pending);
        }

        if let Some(message) = &self.message {
            let mut close = false;
            egui::Window::new(&message.title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(&message.body);
                    ui.add_space(6.0);
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            if close
                || ctx
                    .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))
            {
                self.message = None;
            }
        }

        if let Some(mut dialog) = self.export.take() {
            match dialog.show(ctx, &self.doc.editor, &self.platform) {
                ExportResult::Open => self.export = Some(dialog),
                ExportResult::Cancel => self.last_export = dialog,
                ExportResult::Done(files) => {
                    self.last_export = dialog;
                    let list = files
                        .iter()
                        .map(|f| format!("• {f}"))
                        .collect::<Vec<_>>()
                        .join("\n");
                    self.show_message("Exported", list);
                }
                ExportResult::Failed(e) => {
                    self.export = Some(dialog);
                    self.show_message("Could not export", e);
                }
            }
        }
    }

    fn unsaved_dialog(&mut self, ctx: &egui::Context, pending: Pending) {
        let infos = self.tabs.infos(&self.doc);
        let (question, names): (String, Vec<String>) = match pending {
            Pending::CloseTab(i) => (
                format!(
                    "Save changes to \"{}\" before closing?",
                    infos.get(i).map_or("", |t| t.name.as_str())
                ),
                Vec::new(),
            ),
            Pending::Quit => (
                "These documents have unsaved changes:".to_string(),
                self.tabs
                    .dirty(&self.doc)
                    .iter()
                    .map(|&i| infos[i].name.clone())
                    .collect(),
            ),
        };
        let (save, discard) = match pending {
            Pending::CloseTab(_) => ("Save", "Discard"),
            Pending::Quit => ("Save all", "Discard all"),
        };
        let mut choice = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(question);
                for name in &names {
                    ui.label(format!("• {name}"));
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(save).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(discard).clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(2);
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            choice = Some(2);
        }
        let Some(choice) = choice else {
            return;
        };
        self.pending = None;
        match (pending, choice) {
            (Pending::CloseTab(i), 0) => {
                self.tabs.switch(&mut self.doc, i);
                if self.save() {
                    self.close_tab(i);
                }
            }
            (Pending::CloseTab(i), 1) => self.close_tab(i),
            (Pending::Quit, 0) => {
                // Guarda una por una; si alguna se cancela, no sale.
                for i in self.tabs.dirty(&self.doc) {
                    self.tabs.switch(&mut self.doc, i);
                    if !self.save() {
                        return;
                    }
                }
                self.quit(ctx);
            }
            (Pending::Quit, 1) => self.quit(ctx),
            _ => {}
        }
    }
}
