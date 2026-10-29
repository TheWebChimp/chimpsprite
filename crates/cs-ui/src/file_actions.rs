//! Acciones de archivo de la app: nuevo, abrir, guardar, importar, exportar, pestañas y cambios
//! sin guardar.

use eframe::egui;

use crate::editor::Editor;
use crate::export_dialog::ExportResult;
use crate::files::{self, FileError, FileKind};
use crate::pixel_ui;
use crate::platform::{FileFilter, OpenPurpose, PlatformEvent, SaveTarget};
use crate::skin::Skin;
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

/// Qué eligió el usuario en "Unsaved changes".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

/// Textos de "Unsaved changes": la pregunta, los documentos y los botones.
pub(crate) struct UnsavedPrompt {
    pub question: String,
    pub names: Vec<String>,
    pub save: &'static str,
    pub discard: &'static str,
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
            || self.sprite_dialog.is_some()
            || self.grid_dialog.is_some()
            || self.new_tilemap.is_some()
            || self.convert_tilemap.is_some()
            || self.export.is_some()
            || self.pending.is_some()
            || self.message.is_some()
            || self.restore.is_some()
            || self.preset_name.is_some()
            || self.palette_confirm.is_some()
            || self.filter_dialog.is_some()
            || self.select_dialog.is_some()
            || self.frame_dialog.is_some()
            || self.sheet_dialog.is_some()
            || self.shortcuts.is_some()
            || self.preferences
    }

    pub(crate) fn show_message(&mut self, title: &str, body: impl Into<String>) {
        self.message = Some(Message {
            title: title.into(),
            body: body.into(),
        });
    }

    /// Abre `doc` en una pestaña nueva (o en lugar de un Untitled intacto). Empieza con las
    /// opciones de herramienta de la pestaña activa.
    fn open_doc(&mut self, mut doc: DocState) {
        self.canvas_size = None;
        self.tool_options = self.current_tool_options();
        doc.editor.options = self.tool_options;
        self.tabs.open(&mut self.doc, doc);
    }

    /// Opciones de herramienta de la pestaña activa; en Home, las últimas conocidas.
    pub(crate) fn current_tool_options(&self) -> crate::editor::ToolOptions {
        if self.has_doc() {
            self.doc.editor.options
        } else {
            self.tool_options
        }
    }

    pub(crate) fn new_document(&mut self) {
        let name = self.tabs.untitled_name(&self.doc);
        self.open_doc(DocState::untitled_with_palette(
            name,
            self.palettes.default_palette(),
        ));
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

    /// Pasa a Home (la pestaña activa queda abierta). Home recuerda sus opciones de herramienta.
    pub(crate) fn go_home(&mut self) {
        self.tool_options = self.current_tool_options();
        self.tabs.go_home(&mut self.doc);
    }

    fn close_tab(&mut self, index: usize) {
        if self.tabs.active() == Some(index) {
            self.canvas_size = None;
        }
        // Si se cierra la última, Home se queda con las opciones de la pestaña activa.
        self.tool_options = self.current_tool_options();
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
                            self.import_layer_bytes(&loaded, false)
                        }
                        OpenPurpose::ImportReference if self.has_doc() => {
                            self.import_layer_bytes(&loaded, true)
                        }
                        OpenPurpose::ImportLayer | OpenPurpose::ImportReference => {}
                        OpenPurpose::ImportSheet => self.open_sheet(&loaded),
                        OpenPurpose::RunScript if self.has_doc() => {
                            let source = String::from_utf8_lossy(&loaded.bytes).into_owned();
                            self.run_script(&loaded.name, &source);
                        }
                        OpenPurpose::RunScript => {}
                        OpenPurpose::GroomImage => {
                            match crate::groom::decode_image(&loaded.bytes) {
                                Ok(image) => {
                                    let name = files::stem(&loaded.name).to_string();
                                    self.groom.load(name, image);
                                }
                                Err(e) => self.show_message("Could not read the image", e),
                            }
                        }
                        OpenPurpose::LumenMap(kind) => {
                            match crate::groom::decode_image(&loaded.bytes) {
                                Ok(image) => {
                                    let name = files::stem(&loaded.name).to_string();
                                    if let Err(e) = self.lumen.load_map(kind, name, image) {
                                        self.lumen.notify(e);
                                    }
                                }
                                Err(e) => self.show_message("Could not read the image", e),
                            }
                        }
                        OpenPurpose::LoadPalette if self.has_doc() => {
                            self.load_palette_bytes(&loaded.name, &loaded.bytes)
                        }
                        OpenPurpose::LoadPalette => {}
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
        match self.choose_save_target() {
            Some(target) => self.save_to(target),
            None => false,
        }
    }

    /// File → Save Copy As: escribe el documento en otro archivo sin que el abierto pase a ser
    /// ese (sigue con su nombre y sus cambios sin guardar).
    pub(crate) fn save_copy_as(&mut self) -> bool {
        match self.choose_save_target() {
            Some(target) => self.write_copy(target),
            None => false,
        }
    }

    /// Escribe una copia del documento en `target` (ver `save_copy_as`).
    fn write_copy(&mut self, target: SaveTarget) -> bool {
        let result = save_bytes(&mut self.doc.editor, &target.file_name).and_then(|bytes| {
            self.platform
                .write(&target, &bytes)
                .map_err(FileError::Platform)
        });
        match result {
            Ok(()) => {
                if let Some(path) = target.path() {
                    self.recent.add(&path);
                }
                true
            }
            Err(e) => {
                self.show_message("Could not save the copy", e.to_string());
                false
            }
        }
    }

    /// Pregunta dónde guardar (`.chimp`, o `.aseprite` si se abrió de uno con un solo artboard).
    fn choose_save_target(&mut self) -> Option<SaveTarget> {
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
        self.platform
            .choose_save(&format!("{stem}.{ext}"), &filters)
    }

    fn save_to(&mut self, target: SaveTarget) -> bool {
        let result = save_bytes(&mut self.doc.editor, &target.file_name).and_then(|bytes| {
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
                let missing =
                    files::tilesets_missing_tiles(&self.doc.editor.document, &target_name);
                let missing_note =
                    (!missing.is_empty()).then(|| files::missing_tiles_note(&missing));
                if baked {
                    let mut body = String::from(
                        ".aseprite files have no layer or clipping masks, so they were applied to \
                         the pixels in the saved file (your open document keeps them). Save as \
                         .chimp to keep them editable.",
                    );
                    if files::converts_tilemaps(&self.doc.editor.document, &target_name) {
                        body.push_str(
                            "\n\nTilemap layers with a mask or clipping were saved as image \
                             layers in that file so the mask could be applied.",
                        );
                    }
                    if let Some(note) = &missing_note {
                        body.push_str("\n\n");
                        body.push_str(note);
                    }
                    self.show_message("Masks applied in the file", body);
                } else if let Some(note) = missing_note {
                    self.show_message("Tiles missing from a tileset", note);
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

    /// Corre el script `name` (el lenguaje sale de su extensión) sobre el documento abierto; lo
    /// que imprime, o el error, se muestra en un mensaje.
    pub(crate) fn run_script(&mut self, name: &str, source: &str) {
        let Some(language) = cs_script::Language::of(name).filter(|l| l.available()) else {
            self.show_message(
                "Can't run this script",
                format!("{name}: use .rhai or .lua"),
            );
            return;
        };
        // `export_folder()`: la carpeta del documento o, si no está guardado, la de la app.
        #[cfg(not(target_arch = "wasm32"))]
        let folder = self
            .doc
            .location
            .target
            .as_ref()
            .and_then(|t| t.path())
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .or_else(|| eframe::storage_dir("ChimpSprite").map(|d| d.join("exports")))
            .map(|d| d.display().to_string());
        #[cfg(target_arch = "wasm32")]
        let folder = None;
        match self.doc.editor.run_script(language, source, folder) {
            Ok(output) if output.is_empty() => {}
            Ok(output) => self.show_message(name, output),
            Err(e) => {
                let mut body = e.message;
                if !e.output.is_empty() {
                    body = format!(
                        "{}

{body}",
                        e.output
                    );
                }
                self.show_message(&format!("{name} failed"), body);
            }
        }
    }

    /// File → Scripts → Run Script File…
    pub(crate) fn choose_script(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(crate::scripts::SCRIPT_FILTER, OpenPurpose::RunScript, ctx);
    }

    /// File → Import Sprite Sheet…: se elige el PNG y después cómo se parte.
    pub(crate) fn import_sheet(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(PNG_FILTER, OpenPurpose::ImportSheet, ctx);
    }

    /// El PNG leído: el diálogo para partirlo.
    fn open_sheet(&mut self, loaded: &Loaded) {
        match cs_io::import::decode_png(&loaded.bytes) {
            Ok(sheet) => {
                let name = files::stem(&loaded.name).to_string();
                self.sheet_dialog = Some(crate::sheet_dialog::SheetDialog::new(name, sheet));
            }
            Err(e) => self.show_message("Could not import", e.to_string()),
        }
    }

    /// Abre `sprite` (de una hoja partida) en una pestaña nueva, como un documento sin guardar.
    pub(crate) fn open_sheet_sprite(&mut self, name: &str, sprite: cs_core::Sprite) {
        let doc = cs_core::Document::with_sprite(name, sprite);
        self.open_doc(DocState::new(
            Editor::new(doc),
            DocLocation {
                name: format!("{name}.chimp"),
                target: None,
            },
        ));
    }

    /// Layer → New Reference Layer from File…: una imagen para calcar, que no se exporta.
    pub(crate) fn import_reference(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(PNG_FILTER, OpenPurpose::ImportReference, ctx);
    }

    /// Agrega la imagen de `loaded` como capa nueva (con `reference`, de referencia), en un
    /// paso de undo, y la elige.
    pub(crate) fn import_layer_bytes(&mut self, loaded: &Loaded, reference: bool) {
        let name = files::stem(&loaded.name).to_string();
        let result = import_layer_command(&mut self.doc.editor, &loaded.bytes, &name)
            .map(|cmd| if reference { as_reference(cmd) } else { cmd });
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
        self.doc.editor.commit_floating();
        // El normal map, con los ajustes de Lumen.
        self.export = Some(crate::export_dialog::ExportDialog {
            normals: self.lumen.normals,
            ..self.last_export.clone()
        });
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
        let skin = self.pixel.skin();
        crate::pixel_ui::app_window(
            ctx,
            skin,
            "restore_window",
            "Restore last session",
            false,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                ui.label("These documents have unsaved changes from your last visit:");
                for r in restore {
                    ui.label(format!("• {}", r.name));
                }
                ui.add_space(6.0);
                choice = crate::pixel_ui::dialog_buttons(ui, skin, &["Restore", "Discard"])
                    .map(|i| i == 0);
            },
        );
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
            let skin = self.pixel.skin();
            let (_, closed) = crate::pixel_ui::app_window(
                ctx,
                skin,
                "message_window",
                &message.title,
                true,
                (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
                |ui| {
                    ui.label(&message.body);
                    ui.add_space(6.0);
                    if crate::pixel_ui::dialog_buttons(ui, skin, &["OK"]).is_some() {
                        close = true;
                    }
                },
            );
            close |= closed;
            if close
                || ctx
                    .input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Escape))
            {
                self.message = None;
            }
        }

        if let Some(mut dialog) = self.export.take() {
            match dialog.show(ctx, &mut self.doc.editor, &self.platform, self.pixel.skin()) {
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

    fn unsaved_prompt(&self, pending: Pending) -> UnsavedPrompt {
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
        UnsavedPrompt {
            question,
            names,
            save,
            discard,
        }
    }

    /// "Unsaved changes". Con Pixel UI se dibuja con el skin; la lógica es la misma en los dos
    /// modos (Esc cancela).
    fn unsaved_dialog(&mut self, ctx: &egui::Context, pending: Pending) {
        let prompt = self.unsaved_prompt(pending);
        let mut choice = unsaved_window(ctx, self.pixel.skin(), &prompt);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            choice = Some(UnsavedChoice::Cancel);
        }
        let Some(choice) = choice else {
            return;
        };
        self.pending = None;
        match (pending, choice) {
            (Pending::CloseTab(i), UnsavedChoice::Save) => {
                self.tabs.switch(&mut self.doc, i);
                if self.save() {
                    self.close_tab(i);
                }
            }
            (Pending::CloseTab(i), UnsavedChoice::Discard) => self.close_tab(i),
            (Pending::Quit, UnsavedChoice::Save) => {
                // Guarda una por una; si alguna se cancela, no sale.
                for i in self.tabs.dirty(&self.doc) {
                    self.tabs.switch(&mut self.doc, i);
                    if !self.save() {
                        return;
                    }
                }
                self.quit(ctx);
            }
            (Pending::Quit, UnsavedChoice::Discard) => self.quit(ctx),
            (_, UnsavedChoice::Cancel) => {}
        }
    }
}

/// "Unsaved changes" en la ventana de la app (con `skin`, la de Pixel UI): la pregunta, los
/// documentos y Save / Discard / Cancel. Cerrar con la × es Cancel.
fn unsaved_window(
    ctx: &egui::Context,
    skin: Option<&Skin>,
    prompt: &UnsavedPrompt,
) -> Option<UnsavedChoice> {
    let mut choice = None;
    let (_, closed) = pixel_ui::app_window(
        ctx,
        skin,
        "unsaved_changes",
        "Unsaved changes",
        true,
        (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
        |ui| {
            ui.vertical_centered(|ui| {
                ui.label(&prompt.question);
                // `•` no está en la fuente pixel; `·` sí.
                let bullet = if skin.is_some() { "·" } else { "•" };
                for name in &prompt.names {
                    ui.label(format!("{bullet} {name}"));
                }
            });
            ui.add_space(6.0);
            let labels = [prompt.save, prompt.discard, "Cancel"];
            choice = pixel_ui::dialog_buttons(ui, skin, &labels).map(|i| match i {
                0 => UnsavedChoice::Save,
                1 => UnsavedChoice::Discard,
                _ => UnsavedChoice::Cancel,
            });
        },
    );
    if closed {
        return Some(UnsavedChoice::Cancel);
    }
    choice
}

/// Bytes del documento para guardarlo como `file_name`. Lo flotante todavía no está en el
/// documento: se confirma antes de escribir.
pub(crate) fn save_bytes(editor: &mut Editor, file_name: &str) -> Result<Vec<u8>, FileError> {
    editor.commit_floating();
    files::encode(&editor.document, file_name)
}

/// Comando que agrega el PNG como capa nueva en el frame actual. Lo flotante se confirma antes
/// de armarlo: el comando reserva el próximo id de imagen, que la confirmación podría usar.
/// El mismo comando, con la capa que agrega marcada como de referencia.
fn as_reference(cmd: cs_commands::Command) -> cs_commands::Command {
    use cs_commands::Command;
    match cmd {
        Command::AddLayer { index, mut layer } => {
            layer.reference = true;
            Command::AddLayer { index, layer }
        }
        Command::Batch(cmds) => Command::Batch(cmds.into_iter().map(as_reference).collect()),
        other => other,
    }
}

pub(crate) fn import_layer_command(
    editor: &mut Editor,
    bytes: &[u8],
    name: &str,
) -> cs_io::Result<cs_commands::Command> {
    editor.commit_floating();
    cs_io::import::import_png_layer(editor.sprite(), bytes, editor.frame, name)
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, Mask, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::editor::ToolKind;

    use egui::{Event, PointerButton, Pos2, RawInput, Rect, Vec2};

    use crate::palette_library::PaletteLibrary;
    use crate::palette_library::test_util::NoPresets;
    use crate::view::ViewSettings;

    /// La app con Pixel UI encendido y una pestaña con cambios a punto de cerrarse, manejada con
    /// eventos crudos. Los botones se buscan por su nombre en el árbol de accesibilidad.
    struct DialogHarness {
        ctx: egui::Context,
        app: ChimpApp,
        time: f64,
        /// Botones del último frame: nombre y rectángulo (en puntos).
        buttons: Vec<(String, Rect)>,
    }

    impl DialogHarness {
        fn new() -> Self {
            let view = ViewSettings {
                pixel_ui: true,
                ..Default::default()
            };
            let palettes = PaletteLibrary::new(Box::new(NoPresets));
            let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
            app.new_document();
            app.doc.editor.execute(Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
            });
            app.request_close_tab(0);
            assert_eq!(app.pending, Some(Pending::CloseTab(0)));
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut h = Self {
                ctx,
                app,
                time: 0.0,
                buttons: Vec::new(),
            };
            // La escala cambia un frame después y el diálogo se mide en el primero.
            for _ in 0..3 {
                h.frame(vec![]);
            }
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let app = &mut self.app;
            let mut output = self.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                app.pixel.update(&ctx, &app.view);
                app.show_dialogs(&ctx);
            });
            // Las texturas (fuentes y skin) no se suben a ninguna GPU.
            output.textures_delta.clear();
            self.buttons = output
                .platform_output
                .accesskit_update
                .map(|update| {
                    update
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                        .filter_map(|(_, n)| {
                            let b = n.bounds()?;
                            let rect = Rect::from_min_max(
                                Pos2::new(b.x0 as f32, b.y0 as f32),
                                Pos2::new(b.x1 as f32, b.y1 as f32),
                            );
                            Some((n.label()?.to_string(), rect))
                        })
                        .collect()
                })
                .unwrap_or_default();
        }

        fn labels(&self) -> Vec<&str> {
            let mut labels: Vec<&str> = self.buttons.iter().map(|(l, _)| l.as_str()).collect();
            labels.sort();
            labels
        }

        fn click(&mut self, label: &str) {
            let pos = self
                .buttons
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, r)| r.center())
                .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.labels()));
            let button = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![button(true)]);
            self.frame(vec![button(false)]);
        }
    }

    #[test]
    fn with_pixel_ui_the_unsaved_dialog_shows_the_three_buttons_and_close() {
        let h = DialogHarness::new();
        assert!(h.app.pixel.skin().is_some());
        assert_eq!(h.labels(), ["Cancel", "Close", "Discard", "Save"]);
    }

    #[test]
    fn cancel_closes_the_dialog_without_changes() {
        let mut h = DialogHarness::new();
        h.click("Cancel");
        assert_eq!(h.app.pending, None);
        assert!(
            h.app.has_doc() && h.app.any_dirty(),
            "the tab is still there, unsaved"
        );
    }

    #[test]
    fn the_close_button_cancels() {
        let mut h = DialogHarness::new();
        h.click("Close");
        assert_eq!(h.app.pending, None, "the dialog is gone");
        assert!(
            h.app.has_doc() && h.app.any_dirty(),
            "the tab is still there, unsaved"
        );
    }

    #[test]
    fn discard_closes_the_tab() {
        let mut h = DialogHarness::new();
        h.click("Discard");
        assert_eq!(h.app.pending, None);
        assert!(!h.app.has_doc(), "the only tab is gone");
    }

    #[test]
    fn escape_cancels_in_pixel_ui_too() {
        let mut h = DialogHarness::new();
        let key = |pressed| Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        h.frame(vec![key(true), key(false)]);
        assert_eq!(h.app.pending, None);
        assert!(h.app.has_doc());
    }

    #[test]
    fn saving_applies_floating_content_first() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red)],
        });
        e.tool = ToolKind::RectSelect;
        e.set_selection(Some(Mask::from_rect(4, 4, (0, 0), (0, 0))));
        e.pointer_press((0.5, 0.5), false, false, 0.1);
        e.pointer_drag((2.5, 2.5));
        e.pointer_release();
        assert!(e.is_floating());
        let bytes = save_bytes(&mut e, "hero.chimp").unwrap();
        assert!(!e.is_floating(), "applied before writing");
        let (doc, _) = files::decode("hero.chimp", &bytes).unwrap();
        let saved = &doc.artboards[0].sprite;
        assert_eq!(saved.pixel(LayerId(1), 0, 2, 2), red, "the file has it");
        assert_ne!(saved.pixel(LayerId(1), 0, 0, 0), red);
    }

    #[test]
    fn importing_a_layer_while_a_paste_floats_keeps_the_pasted_pixels() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        assert!(e.sprite().cel(LayerId(1), 0).is_none(), "empty layer");
        let clip = crate::clipboard::Clip::from_rgba(1, 1, &[255, 0, 0, 255]).unwrap();
        e.paste(&clip);
        assert!(e.is_floating());
        let mut png = Sprite::new(2, 2, ColorMode::Rgba);
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Rgba(Rgba::rgb(0, 0, 255)))],
        }
        .apply(&mut png)
        .unwrap();
        let bytes = cs_io::export::export_png(&png, 0, 1).unwrap();
        let cmd = import_layer_command(&mut e, &bytes, "imported").unwrap();
        assert!(e.execute(cmd));
        let s = e.sprite();
        assert_eq!(
            s.pixel(LayerId(1), 0, 0, 0),
            red,
            "the pasted pixel survives"
        );
        let top = s.layers.last().unwrap().id;
        let pasted = s.cel(LayerId(1), 0).unwrap().image;
        let imported = s.cel(top, 0).unwrap().image;
        assert_ne!(pasted, imported, "each cel has its own image");
    }

    #[test]
    fn importing_a_sprite_sheet_opens_a_tab_with_one_frame_per_cell() {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), ViewSettings::default(), palettes);
        // Una tira de 3 cuadrados de 4×4, cada uno de un color.
        let mut strip = cs_io::export::Bitmap::new(12, 4);
        for (i, c) in [(255, 0, 0), (0, 255, 0), (0, 0, 255)]
            .into_iter()
            .enumerate()
        {
            for y in 0..4 {
                for x in 0..4 {
                    strip.pixels[y * 12 + i * 4 + x] = Rgba::rgb(c.0, c.1, c.2);
                }
            }
        }
        let png = strip.to_png().unwrap();
        app.open_sheet(&Loaded {
            name: "walk.png".into(),
            target: None,
            bytes: png,
        });
        let dialog = app.sheet_dialog.as_ref().expect("the dialog");
        assert_eq!(dialog.grid.cell, (4, 4), "square cells guessed for a strip");
        let sprite = dialog.sprite().expect("sprite");
        app.sheet_dialog = None;
        app.open_sheet_sprite("walk", sprite);
        let s = app.doc.editor.sprite();
        assert_eq!((s.width, s.height, s.frames.len()), (4, 4, 3));
        assert_eq!(
            s.pixel(s.layers[0].id, 2, 1, 1),
            PixelValue::Rgba(Rgba::rgb(0, 0, 255))
        );
        assert_eq!(app.doc.location.name, "walk.chimp");
    }

    #[test]
    fn a_reference_layer_from_a_file_is_marked_in_one_step() {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), ViewSettings::default(), palettes);
        app.new_document();
        let png = cs_io::export::export_png(&Sprite::new(4, 4, ColorMode::Rgba), 0, 1).unwrap();
        let steps = app.doc.editor.steps_done();
        app.import_layer_bytes(
            &Loaded {
                name: "sketch.png".into(),
                target: None,
                bytes: png,
            },
            true,
        );
        let top = app.doc.editor.sprite().layers.last().unwrap();
        assert!(top.reference, "{top:?}");
        assert_eq!(app.doc.editor.layer, top.id, "chosen");
        assert_eq!(app.doc.editor.steps_done(), steps + 1);
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn save_copy_as_writes_the_file_and_keeps_the_open_document_as_it_was() {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), ViewSettings::default(), palettes);
        app.new_document();
        app.doc.editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        });
        let name = app.doc.location.name.clone();
        let saved = app.doc.saved_version;
        let dir = std::env::temp_dir().join(format!("chimpsprite-copy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("copy.chimp");
        let target = SaveTarget::from_path(&path).unwrap();
        assert!(app.write_copy(target));
        assert!(path.exists());
        assert_eq!(app.doc.location.name, name, "still the same document");
        assert_eq!(app.doc.saved_version, saved, "still unsaved");
        assert_eq!(app.recent.files().first(), Some(&path), "in Open Recent");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_new_tab_starts_with_the_tool_options_of_the_active_one() {
        use cs_tools::BrushShape;
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), ViewSettings::default(), palettes);
        // Lo leído al arrancar.
        app.tool_options.brush_shape = BrushShape::Square;
        app.new_document();
        assert_eq!(app.doc.editor.options.brush_shape, BrushShape::Square);
        app.doc.editor.options.brush_size = 7;
        app.doc.editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        });
        app.new_document();
        assert_eq!(app.tabs.len(), 2, "a second tab");
        assert_eq!(app.doc.editor.options.brush_size, 7);
        assert_eq!(app.current_tool_options().brush_size, 7, "what gets saved");
        // Cerrar hasta llegar a Home guarda las opciones de la última pestaña.
        app.doc.editor.options.brush_size = 9;
        app.close_tab(0);
        app.close_tab(0);
        assert!(!app.has_doc(), "Home");
        assert_eq!(app.current_tool_options().brush_size, 9, "kept on Home");
        app.new_document();
        assert_eq!(
            app.doc.editor.options.brush_size, 9,
            "the new tab gets them"
        );
        // Ir a Home con el botón también.
        app.doc.editor.options.brush_size = 11;
        app.go_home();
        assert!(!app.has_doc());
        assert_eq!(app.current_tool_options().brush_size, 11);
    }
}
