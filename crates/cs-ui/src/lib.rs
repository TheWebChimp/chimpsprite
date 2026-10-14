//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod anim_ops;
mod autosave;
mod canvas;
mod canvas_size;
mod capture;
mod demo;
mod editor;
mod export_dialog;
mod file_actions;
mod files;
mod panels;
mod platform;
mod theme;
mod timeline;
mod widgets;

use cs_core::{ArtboardId, ColorMode, Sprite};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

pub use editor::{Editor, ToolKind};

const NEW_SPRITE_SIZE: u32 = 64;

pub struct ChimpApp {
    editor: Editor,
    canvas: canvas::CanvasView,
    capture: capture::Capture,
    /// Texto del nombre del artboard activo mientras se edita.
    artboard_name: (ArtboardId, String),
    canvas_size: Option<canvas_size::CanvasSizeDialog>,
    timeline: timeline::TimelineState,
    location: file_actions::DocLocation,
    platform: platform::Platform,
    autosaver: autosave::Autosaver,
    restore: Option<autosave::Restore>,
    /// `Editor::doc_version` en el último guardado.
    saved_version: u64,
    pending: Option<file_actions::Pending>,
    message: Option<file_actions::Message>,
    export: Option<export_dialog::ExportDialog>,
    /// Opciones de la última exportación, para reabrir el diálogo igual.
    last_export: export_dialog::ExportDialog,
    /// Cerrar sin preguntar (ya se confirmó).
    allow_close: bool,
    title: String,
}

impl ChimpApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let launch = platform::launch_options();
        let editor = if launch.demo {
            demo::editor()
        } else {
            new_editor()
        };
        let mut app = Self {
            editor,
            canvas: canvas::CanvasView::default(),
            capture: capture::Capture::from_env(),
            artboard_name: (ArtboardId(0), String::new()),
            canvas_size: None,
            timeline: timeline::TimelineState::default(),
            location: file_actions::DocLocation::default(),
            platform: platform::Platform::default(),
            autosaver: autosave::Autosaver::default(),
            restore: None,
            saved_version: 0,
            pending: None,
            message: None,
            export: None,
            last_export: export_dialog::ExportDialog::default(),
            allow_close: false,
            title: String::new(),
        };
        app.saved_version = app.editor.doc_version;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &launch.open {
            app.platform.open_path(path, platform::OpenPurpose::Open);
        }
        // Sesión guardada en el navegador (solo web).
        if !launch.demo
            && let Some(text) = autosave::storage::load()
        {
            match autosave::decode(&text) {
                Ok((doc, name)) => app.restore = Some(autosave::Restore { name, doc }),
                Err(e) => {
                    log::warn!("discarding unreadable autosave: {e}");
                    autosave::storage::clear();
                }
            }
        }
        platform::install_unload_guard();
        // Desarrollo: abre el diálogo con un ancho de ejemplo para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_CANVAS_SIZE").is_some() {
            app.open_canvas_size();
            if let Some(d) = &mut app.canvas_size {
                d.set_anchor(0, 1);
                d.set_width(d.width() + 24);
            }
        }
        app
    }

    fn open_canvas_size(&mut self) {
        self.canvas_size = Some(canvas_size::CanvasSizeDialog::new(self.editor.artboard()));
    }

    fn update_canvas_size(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.canvas_size else {
            return;
        };
        if self.editor.document.artboard(dialog.artboard).is_none() {
            self.canvas_size = None;
            return;
        }
        match canvas_size::show(ctx, dialog) {
            canvas_size::DialogResult::Open => {}
            canvas_size::DialogResult::Accept => {
                if let Some(cmd) = dialog.command(&self.editor.document) {
                    self.editor.execute_doc(cmd);
                }
                self.canvas_size = None;
            }
            canvas_size::DialogResult::Cancel => self.canvas_size = None,
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Los diálogos son modales: manejan sus propias teclas.
        if self.is_modal() {
            return;
        }
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        let save_as = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        if ctx.input_mut(|i| i.consume_shortcut(&save_as)) {
            self.save_as();
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S))) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N))) {
            self.request(file_actions::Pending::New, ctx);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O))) {
            self.request(file_actions::Pending::Open, ctx);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::E))) {
            self.open_export();
        }
        let redo_shift = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        if ctx.input_mut(|i| i.consume_shortcut(&redo_shift) || i.consume_shortcut(&redo)) {
            self.editor.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.editor.undo();
        }
        let select_all = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
        let deselect = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
        if ctx.input_mut(|i| i.consume_shortcut(&select_all)) {
            self.editor.select_all();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&deselect)) {
            self.editor.deselect();
        }
        if ctx.egui_wants_keyboard_input() || self.editor.is_painting() {
            return;
        }
        let new_frame = KeyboardShortcut::new(Modifiers::ALT, Key::N);
        if ctx.input_mut(|i| i.consume_shortcut(&new_frame)) {
            self.editor.duplicate_frame();
        }
        let pressed: Vec<(Key, bool)> = ctx.input(|i| {
            if i.modifiers.command || i.modifiers.alt {
                return Vec::new();
            }
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, modifiers.shift)),
                    _ => None,
                })
                .collect()
        });
        for (key, shift) in pressed {
            let editor = &mut self.editor;
            match key {
                Key::O if shift => editor.tool = ToolKind::Artboard,
                Key::N if shift => editor.add_layer(cs_core::LayerKind::Image),
                Key::Enter => editor.toggle_play(),
                Key::Comma | Key::ArrowLeft => editor.prev_frame(),
                Key::Period | Key::ArrowRight => editor.next_frame(),
                Key::ArrowUp => editor.step_layer(true),
                Key::ArrowDown => editor.step_layer(false),
                Key::F3 => editor.onion_enabled = !editor.onion_enabled,
                Key::B => editor.tool = ToolKind::Pencil,
                Key::E => editor.tool = ToolKind::Eraser,
                Key::G => editor.tool = ToolKind::Bucket,
                Key::L => editor.tool = ToolKind::Line,
                Key::U => {
                    editor.tool = if editor.tool == ToolKind::Rectangle {
                        ToolKind::Ellipse
                    } else {
                        ToolKind::Rectangle
                    }
                }
                Key::M => editor.tool = ToolKind::Select,
                Key::V => editor.tool = ToolKind::Move,
                Key::I => editor.tool = ToolKind::Eyedropper,
                Key::X => editor.swap_colors(),
                Key::Delete | Key::Backspace => editor.clear_selection_content(),
                _ => {}
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let ctx = ui.ctx().clone();
                let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
                if ui.add(item("New", "Ctrl+N")).clicked() {
                    self.request(file_actions::Pending::New, &ctx);
                }
                if ui.add(item("Open…", "Ctrl+O")).clicked() {
                    self.request(file_actions::Pending::Open, &ctx);
                }
                ui.separator();
                let (save, save_as) = if self.platform.saves_by_download() {
                    ("Save (download)", "Save As… (download)")
                } else {
                    ("Save", "Save As…")
                };
                if ui.add(item(save, "Ctrl+S")).clicked() {
                    self.save();
                }
                if ui.add(item(save_as, "Ctrl+Shift+S")).clicked() {
                    self.save_as();
                }
                ui.separator();
                if ui.button("Import PNG as Layer…").clicked() {
                    self.import_layer(&ctx);
                }
                if ui.add(item("Export…", "Ctrl+E")).clicked() {
                    self.open_export();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.request(file_actions::Pending::Quit, &ctx);
                }
            });
            ui.menu_button("Sprite", |ui| {
                if ui.button("Canvas Size…").clicked() {
                    self.open_canvas_size();
                }
            });
            ui.menu_button("Edit", |ui| {
                let undo = egui::Button::new("Undo").shortcut_text("Ctrl+Z");
                if ui
                    .add_enabled(self.editor.history.can_undo(), undo)
                    .clicked()
                {
                    self.editor.undo();
                }
                let redo = egui::Button::new("Redo").shortcut_text("Ctrl+Y");
                if ui
                    .add_enabled(self.editor.history.can_redo(), redo)
                    .clicked()
                {
                    self.editor.redo();
                }
                ui.separator();
                if ui
                    .add(egui::Button::new("Select All").shortcut_text("Ctrl+A"))
                    .clicked()
                {
                    self.editor.select_all();
                }
                let has_selection = self.editor.selection.is_some();
                let deselect = egui::Button::new("Deselect").shortcut_text("Ctrl+D");
                if ui.add_enabled(has_selection, deselect).clicked() {
                    self.editor.deselect();
                }
                let clear = egui::Button::new("Clear").shortcut_text("Del");
                if ui.add_enabled(has_selection, clear).clicked() {
                    self.editor.clear_selection_content();
                }
            });
        });
    }
}

fn new_editor() -> Editor {
    Editor::with_sprite(Sprite::new(
        NEW_SPRITE_SIZE,
        NEW_SPRITE_SIZE,
        ColorMode::Rgba,
    ))
}

impl eframe::App for ChimpApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close_request(&ctx);
        self.handle_dropped_files(&ctx);
        self.handle_platform_events(&ctx);
        self.handle_shortcuts(&ctx);
        if self.editor.playing {
            let dt_ms = ctx.input(|i| i.stable_dt) as f64 * 1000.0;
            self.editor.tick(dt_ms);
            ctx.request_repaint();
        }

        let modal = self.is_modal();
        egui::Panel::top("menu").show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| self.menu_bar(ui));
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            let notice = self
                .autosaver
                .disabled_reason
                .as_ref()
                .map(|r| format!("Autosave off: {r}"));
            panels::status_bar(ui, &self.editor, &self.canvas, notice.as_deref())
        });
        egui::Panel::bottom("timeline")
            .resizable(true)
            .default_size(190.0)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    timeline::show(ui, &mut self.editor, &mut self.timeline)
                });
            });
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(150.0)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.editor));
            });
        egui::Panel::right("palette")
            .default_size(220.0)
            .show(ui, |ui| {
                panels::palette(ui, &mut self.editor);
                ui.add_space(12.0);
                let open = ui
                    .add_enabled_ui(!modal, |ui| {
                        panels::artboards(ui, &mut self.editor, &mut self.artboard_name)
                    })
                    .inner;
                if open {
                    self.open_canvas_size();
                }
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.canvas
                .show(ui, &mut self.editor, self.canvas_size.as_mut())
        });
        self.update_canvas_size(&ctx);
        self.show_dialogs(&ctx);
        self.show_restore(&ctx);
        self.autosave(&ctx);
        self.update_title(&ctx);
        platform::set_unsaved(self.is_dirty());

        self.capture.update(&ctx);
    }
}
