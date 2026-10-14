//! Acciones de archivo de la app: nuevo, abrir, guardar, importar, exportar, cambios sin guardar.

use eframe::egui;

use crate::editor::Editor;
use crate::export_dialog::ExportResult;
use crate::files::{self, FileError, FileKind};
use crate::platform::{FileFilter, OpenPurpose, PlatformEvent, SaveTarget};
use crate::{ChimpApp, new_editor};

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

/// Un archivo ya leído, esperando a que el usuario confirme si descarta los cambios.
pub(crate) struct Loaded {
    pub name: String,
    pub target: Option<SaveTarget>,
    pub bytes: Vec<u8>,
}

/// Acción en espera de que el usuario decida qué hacer con los cambios sin guardar.
pub(crate) enum Pending {
    New,
    Open,
    Load(Box<Loaded>),
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
    pub(crate) fn is_dirty(&self) -> bool {
        self.editor.doc_version != self.saved_version
    }

    pub(crate) fn is_modal(&self) -> bool {
        self.canvas_size.is_some()
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

    pub(crate) fn reset_with(&mut self, editor: Editor, location: DocLocation) {
        self.editor = editor;
        self.location = location;
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
            Pending::New => self.reset_with(new_editor(), DocLocation::default()),
            Pending::Open => self
                .platform
                .open_file(SPRITE_FILTERS, OpenPurpose::Open, ctx),
            Pending::Load(loaded) => self.load(*loaded),
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Procesa lo que llegó de la plataforma (archivos abiertos o errores).
    pub(crate) fn handle_platform_events(&mut self, ctx: &egui::Context) {
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
                        OpenPurpose::Open => self.load(loaded),
                        OpenPurpose::Dropped => self.request(Pending::Load(Box::new(loaded)), ctx),
                        OpenPurpose::ImportLayer => self.import_layer_bytes(&loaded),
                    }
                }
                PlatformEvent::Error(e) => self.show_message("Could not read the file", e),
            }
        }
    }

    fn load(&mut self, loaded: Loaded) {
        match files::decode(&loaded.name, &loaded.bytes) {
            Ok((doc, report)) => {
                // Un PNG se abre como sprite nuevo: guardarlo pide elegir .chimp o .aseprite.
                let is_png = files::kind_of(&loaded.name) == Some(FileKind::Png);
                let location = DocLocation {
                    name: if is_png {
                        format!("{}.chimp", files::stem(&loaded.name))
                    } else {
                        loaded.name.clone()
                    },
                    target: loaded.target.filter(|_| !is_png),
                };
                self.reset_with(Editor::new(doc), location);
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
        match self.location.target.clone() {
            Some(target) => self.save_to(target),
            None => self.save_as(),
        }
    }

    pub(crate) fn save_as(&mut self) -> bool {
        let multiple = self.editor.document.artboards.len() > 1;
        let aseprite = files::kind_of(&self.location.name) == Some(FileKind::Aseprite) && !multiple;
        let ext = if aseprite { "aseprite" } else { "chimp" };
        let stem = match &self.location.target {
            Some(t) => t.stem().to_string(),
            None if self.location.name != "Untitled" => {
                files::stem(&self.location.name).to_string()
            }
            None => self.editor.artboard().name.clone(),
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
        let result = files::encode(&self.editor.document, &target.file_name).and_then(|bytes| {
            self.platform
                .write(&target, &bytes)
                .map_err(FileError::Platform)
        });
        match result {
            Ok(()) => {
                self.location = DocLocation {
                    name: target.file_name.clone(),
                    target: Some(target),
                };
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

    pub(crate) fn import_layer(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(PNG_FILTER, OpenPurpose::ImportLayer, ctx);
    }

    fn import_layer_bytes(&mut self, loaded: &Loaded) {
        let name = files::stem(&loaded.name).to_string();
        let result = cs_io::import::import_png_layer(
            self.editor.sprite(),
            &loaded.bytes,
            self.editor.frame,
            &name,
        );
        match result {
            Ok(cmd) => {
                if self.editor.execute(cmd)
                    && let Some(top) = self.editor.sprite().layers.last()
                {
                    let id = top.id;
                    self.editor.select_layer(id);
                }
            }
            Err(e) => self.show_message("Could not import", e.to_string()),
        }
    }

    pub(crate) fn open_export(&mut self) {
        self.export = Some(self.last_export.clone());
    }

    /// Archivos soltados sobre la ventana: abre el primero.
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
        if ctx.input(|i| i.viewport().close_requested()) && self.is_dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
        }
    }

    /// Guarda en el navegador cada tanto mientras haya cambios; limpia al quedar todo guardado.
    pub(crate) fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let (version, dirty) = (self.editor.doc_version, self.is_dirty());
        if !dirty {
            if self.autosaver.due(now, version, true) {
                crate::autosave::storage::clear();
                self.autosaver.mark_saved(now, version);
            }
            return;
        }
        if !self.autosaver.due(now, version, dirty) {
            return;
        }
        let result = crate::autosave::encode(&self.editor.document, &self.location.name)
            .and_then(|text| crate::autosave::storage::store(&text));
        match result {
            Ok(()) => self.autosaver.mark_saved(now, version),
            Err(e) => {
                log::warn!("autosave disabled: {e}");
                self.autosaver.disabled_reason = Some(e);
            }
        }
    }

    /// Pregunta si restaurar la sesión guardada en el navegador.
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
                ui.label(format!(
                    "\"{}\" has unsaved changes from your last visit.",
                    restore.name
                ));
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
                let restore = self.restore.take().expect("restore");
                let location = DocLocation {
                    name: restore.name,
                    target: None,
                };
                self.reset_with(Editor::new(restore.doc), location);
                // Lo restaurado no está guardado en ningún archivo.
                self.saved_version = u64::MAX;
            }
            Some(false) => {
                self.restore = None;
                crate::autosave::storage::clear();
            }
            None => {}
        }
    }

    pub(crate) fn update_title(&mut self, ctx: &egui::Context) {
        let title = format!(
            "{}{} — ChimpSprite",
            self.location.name,
            if self.is_dirty() { " •" } else { "" }
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Ventanas modales: cambios sin guardar, mensajes y exportación.
    pub(crate) fn show_dialogs(&mut self, ctx: &egui::Context) {
        if self.pending.is_some() {
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
                    let pending = self.pending.take().expect("pending");
                    if self.save() {
                        self.perform(pending, ctx);
                    }
                }
                Some(1) => {
                    let pending = self.pending.take().expect("pending");
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
            match dialog.show(ctx, &self.editor, &self.platform) {
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
}
