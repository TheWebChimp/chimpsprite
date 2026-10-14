//! Acciones de archivo de la app: nuevo, abrir, guardar, importar, exportar, cambios sin guardar.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::editor::Editor;
use crate::export_dialog::ExportResult;
use crate::files::{self, FileError, FileKind};
use crate::{ChimpApp, new_editor};

/// Acción en espera de que el usuario decida qué hacer con los cambios sin guardar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Pending {
    New,
    Open,
    OpenPath(PathBuf),
    Quit,
}

pub(crate) struct Message {
    pub title: String,
    pub body: String,
}

impl ChimpApp {
    pub(crate) fn is_dirty(&self) -> bool {
        self.editor.doc_version != self.saved_version
    }

    pub(crate) fn is_modal(&self) -> bool {
        self.canvas_size.is_some()
            || self.export.is_some()
            || self.pending.is_some()
            || self.message.is_some()
    }

    fn show_message(&mut self, title: &str, body: impl Into<String>) {
        self.message = Some(Message {
            title: title.into(),
            body: body.into(),
        });
    }

    fn reset_with(&mut self, editor: Editor, path: Option<PathBuf>) {
        self.editor = editor;
        self.path = path;
        self.saved_version = self.editor.doc_version;
        self.canvas.reset();
        self.canvas_size = None;
        self.timeline = Default::default();
    }

    /// Ejecuta `action`, o pregunta antes si hay cambios sin guardar.
    pub(crate) fn request(&mut self, action: Pending, ctx: &egui::Context) {
        if self.is_dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }

    fn perform(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New => self.reset_with(new_editor(), None),
            Pending::Open => self.open_dialog(),
            Pending::OpenPath(path) => self.open_path(&path),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Sprites", &["chimp", "aseprite", "ase", "png"])
            .add_filter("ChimpSprite", &["chimp"])
            .add_filter("Aseprite", &["aseprite", "ase"])
            .add_filter("PNG", &["png"])
            .pick_file()
        {
            self.open_path(&path);
        }
    }

    pub(crate) fn open_path(&mut self, path: &Path) {
        match files::open(path) {
            Ok((doc, report)) => {
                // Un PNG se abre como sprite nuevo: guardarlo pide elegir .chimp o .aseprite.
                let keep_path = files::kind_of(path) != Some(FileKind::Png);
                self.reset_with(Editor::new(doc), keep_path.then(|| path.to_path_buf()));
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
        match self.path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    pub(crate) fn save_as(&mut self) -> bool {
        let multiple = self.editor.document.artboards.len() > 1;
        let current_ext = self
            .path
            .as_ref()
            .and_then(|p| files::kind_of(p))
            .filter(|k| *k == FileKind::Aseprite && !multiple)
            .map_or("chimp", |_| "aseprite");
        let stem = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .and_then(|s| s.to_str())
            .map_or_else(|| self.editor.artboard().name.clone(), str::to_string);
        let mut dialog = rfd::FileDialog::new().set_file_name(format!("{stem}.{current_ext}"));
        dialog = if current_ext == "chimp" {
            dialog
                .add_filter("ChimpSprite", &["chimp"])
                .add_filter("Aseprite (one artboard)", &["aseprite", "ase"])
        } else {
            dialog
                .add_filter("Aseprite", &["aseprite", "ase"])
                .add_filter("ChimpSprite", &["chimp"])
        };
        match dialog.save_file() {
            Some(path) => self.save_to(&path),
            None => false,
        }
    }

    fn save_to(&mut self, path: &Path) -> bool {
        match files::save(&self.editor.document, path) {
            Ok(()) => {
                self.path = Some(path.to_path_buf());
                self.saved_version = self.editor.doc_version;
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

    pub(crate) fn import_layer(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .pick_file()
        else {
            return;
        };
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported")
            .to_string();
        let result = std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                cs_io::import::import_png_layer(
                    self.editor.sprite(),
                    &bytes,
                    self.editor.frame,
                    &name,
                )
                .map_err(|e| e.to_string())
            });
        match result {
            Ok(cmd) => {
                if self.editor.execute(cmd)
                    && let Some(top) = self.editor.sprite().layers.last()
                {
                    let id = top.id;
                    self.editor.select_layer(id);
                }
            }
            Err(e) => self.show_message("Could not import", e),
        }
    }

    pub(crate) fn open_export(&mut self) {
        self.export = Some(self.last_export.clone());
    }

    /// Archivos soltados sobre la ventana: abre el primero.
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()));
        if let Some(path) = dropped
            && !self.is_modal()
        {
            self.request(Pending::OpenPath(path), ctx);
        }
    }

    /// Cerrar la ventana con cambios sin guardar pregunta primero.
    pub(crate) fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
    }

    pub(crate) fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");
        let title = format!(
            "{name}{} — ChimpSprite",
            if self.is_dirty() { " •" } else { "" }
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Ventanas modales: cambios sin guardar, mensajes y exportación.
    pub(crate) fn show_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(pending) = self.pending.clone() {
            let mut choice = None;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Do you want to save your changes first?");
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            choice = Some(0);
                        }
                        if ui.button("Discard").clicked() {
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
            match choice {
                Some(0) => {
                    self.pending = None;
                    if self.save() {
                        self.perform(pending, ctx);
                    }
                }
                Some(1) => {
                    self.pending = None;
                    self.perform(pending, ctx);
                }
                Some(_) => self.pending = None,
                None => {}
            }
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
            match dialog.show(ctx, &self.editor) {
                ExportResult::Open => self.export = Some(dialog),
                ExportResult::Cancel => self.last_export = dialog,
                ExportResult::Done(files) => {
                    self.last_export = dialog;
                    let list = files
                        .iter()
                        .map(|f| format!("• {}", f.display()))
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
}
