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
mod mask_ops;
mod slice_ops;
use cs_io::files;
mod guides;
mod home;
mod instance_ops;
mod panels;
mod platform;
mod recent;
mod tabs;
mod theme;
mod timeline;
mod view;
mod widgets;

use cs_core::{ColorMode, Sprite};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

pub use editor::{Editor, ToolKind};

const NEW_SPRITE_SIZE: u32 = 64;
const RECENT_KEY: &str = "chimpsprite.recent";
const VIEW_KEY: &str = "chimpsprite.view";

pub struct ChimpApp {
    /// Documento de la pestaña activa (un marcador vacío si se está en Home).
    doc: tabs::DocState,
    tabs: tabs::Tabs,
    capture: capture::Capture,
    canvas_size: Option<canvas_size::CanvasSizeDialog>,
    platform: platform::Platform,
    autosaver: autosave::Autosaver,
    restore: Option<Vec<autosave::Restore>>,
    recent: recent::RecentFiles,
    view: view::ViewSettings,
    grid_dialog: Option<view::GridDialog>,
    fullscreen: bool,
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
        let recent = cc
            .storage
            .and_then(|s| eframe::get_value(s, RECENT_KEY))
            .unwrap_or_default();
        let view = cc
            .storage
            .and_then(|s| eframe::get_value(s, VIEW_KEY))
            .unwrap_or_default();
        let mut app = Self {
            doc: tabs::DocState::untitled(""),
            tabs: tabs::Tabs::default(),
            capture: capture::Capture::from_env(),
            canvas_size: None,
            platform: platform::Platform::default(),
            autosaver: autosave::Autosaver::default(),
            restore: None,
            recent,
            view,
            grid_dialog: None,
            fullscreen: false,
            pending: None,
            message: None,
            export: None,
            last_export: export_dialog::ExportDialog::default(),
            allow_close: false,
            title: String::new(),
        };
        if launch.demo {
            app.open_demo();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &launch.open {
            app.platform.open_path(path, platform::OpenPurpose::Open);
        }
        // Sesiones guardadas en el navegador (solo web).
        if !launch.demo
            && let Some(text) = autosave::storage::load()
        {
            match autosave::decode(&text) {
                Ok(docs) if !docs.is_empty() => app.restore = Some(docs),
                Ok(_) => {}
                Err(e) => {
                    log::warn!("discarding unreadable autosave: {e}");
                    autosave::storage::clear();
                }
            }
        }
        platform::install_unload_guard();
        // Desarrollo: abre el diálogo con un ancho de ejemplo para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_CANVAS_SIZE").is_some() && app.has_doc() {
            app.open_canvas_size();
            if let Some(d) = &mut app.canvas_size {
                d.set_anchor(0, 1);
                d.set_width(d.width() + 24);
            }
        }
        app
    }

    /// `false` en Home.
    pub(crate) fn has_doc(&self) -> bool {
        self.tabs.active().is_some()
    }

    fn open_canvas_size(&mut self) {
        self.canvas_size = Some(canvas_size::CanvasSizeDialog::new(
            self.doc.editor.artboard(),
        ));
    }

    fn update_canvas_size(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.canvas_size else {
            return;
        };
        if self.tabs.active().is_none()
            || self.doc.editor.document.artboard(dialog.artboard).is_none()
        {
            self.canvas_size = None;
            return;
        }
        match canvas_size::show(ctx, dialog) {
            canvas_size::DialogResult::Open => {}
            canvas_size::DialogResult::Accept => {
                if let Some(cmd) = dialog.command(&self.doc.editor.document) {
                    self.doc.editor.execute_doc(cmd);
                }
                self.canvas_size = None;
            }
            canvas_size::DialogResult::Cancel => self.canvas_size = None,
        }
    }

    fn update_grid_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.grid_dialog else {
            return;
        };
        match dialog.show(ctx) {
            view::GridDialogResult::Open => {}
            view::GridDialogResult::Accept => {
                let [r, g, b, a] = dialog.color.to_srgba_unmultiplied();
                self.view.grid_color = [r, g, b, a];
                self.view.grid = true;
                if self.tabs.active().is_some()
                    && let Some(cmd) = dialog.command(&self.doc.editor)
                {
                    self.doc.editor.execute_doc(cmd);
                }
                self.grid_dialog = None;
            }
            view::GridDialogResult::Cancel => self.grid_dialog = None,
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    /// Atajos del menú View (funcionan aunque no haya foco en el lienzo).
    fn view_shortcuts(&mut self, ctx: &egui::Context) {
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::H))) {
            self.view.extras = !self.view.extras;
        }
        let fit = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Num0);
        if ctx.input_mut(|i| i.consume_shortcut(&fit)) {
            self.doc.canvas.request_zoom(view::ZoomRequest::Fit);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Num0))) {
            self.doc.canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (zoom_in, zoom_out, tab, f11) = ctx.input(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            (
                plain && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                plain && i.key_pressed(Key::Minus),
                plain && !i.modifiers.shift && i.key_pressed(Key::Tab),
                i.key_pressed(Key::F11),
            )
        });
        if zoom_in {
            self.doc.canvas.request_zoom(view::ZoomRequest::In);
        }
        if zoom_out {
            self.doc.canvas.request_zoom(view::ZoomRequest::Out);
        }
        if tab {
            self.view.timeline = !self.view.timeline;
        }
        if f11 && !cfg!(target_arch = "wasm32") {
            self.toggle_fullscreen(ctx);
        }
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
        let v = &mut self.view;
        ui.add(egui::Checkbox::new(&mut v.extras, "Extras"))
            .on_hover_text("Ctrl+H: show or hide all extras");
        ui.menu_button("Show", |ui| {
            ui.checkbox(&mut v.rulers, "Rulers");
            ui.add_enabled_ui(v.extras, |ui| {
                ui.checkbox(&mut v.guides, "Guides");
                ui.checkbox(&mut v.pixel_grid, "Pixel Grid");
                ui.checkbox(&mut v.grid, "Grid");
                ui.checkbox(&mut v.selection_edges, "Selection Edges");
                ui.checkbox(&mut v.slices, "Slices");
                ui.checkbox(&mut v.brush_preview, "Brush Preview");
                ui.checkbox(&mut v.artboard_names, "Artboard Names");
            });
        });
        ui.menu_button("Guides", |ui| {
            ui.checkbox(&mut self.view.guides, "Show Guides");
            ui.checkbox(&mut self.view.snap_to_guides, "Snap to Guides");
            ui.checkbox(&mut self.view.lock_guides, "Lock Guides");
            ui.separator();
            let has_guides = !self.doc.editor.guides().is_empty();
            if ui
                .add_enabled(has_guides, egui::Button::new("Clear Guides"))
                .clicked()
            {
                self.doc.editor.set_guides(Vec::new());
                ui.close();
            }
            ui.weak("Drag from a ruler to add a guide;\ndrag it back to remove it.");
        });
        ui.menu_button("Grid", |ui| {
            ui.checkbox(&mut self.view.grid, "Show Grid");
            ui.checkbox(&mut self.view.snap_to_grid, "Snap to Grid");
            if ui.button("Grid Settings…").clicked() {
                self.grid_dialog = Some(view::GridDialog::new(&self.doc.editor, &self.view));
                ui.close();
            }
        });
        ui.separator();
        ui.add(egui::Checkbox::new(
            &mut self.doc.editor.onion_enabled,
            "Onion Skin",
        ))
        .on_hover_text("F3");
        ui.add(egui::Checkbox::new(&mut self.view.timeline, "Timeline"))
            .on_hover_text("Tab");
        if !cfg!(target_arch = "wasm32") {
            let mut fullscreen = self.fullscreen;
            if ui
                .add(egui::Checkbox::new(&mut fullscreen, "Full Screen"))
                .on_hover_text("F11")
                .changed()
            {
                self.toggle_fullscreen(ui.ctx());
            }
        }
        ui.separator();
        let canvas = &mut self.doc.canvas;
        if ui.add(item("Zoom In", "+")).clicked() {
            canvas.request_zoom(view::ZoomRequest::In);
        }
        if ui.add(item("Zoom Out", "-")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Out);
        }
        if ui.add(item("Actual Size", "Ctrl+0")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ui.add(item("Fit to Screen", "Ctrl+Shift+0")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Fit);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Los diálogos son modales: manejan sus propias teclas.
        if self.is_modal() {
            return;
        }
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N))) {
            self.new_document();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O))) {
            self.open_dialog(ctx);
        }
        let prev_tab = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab);
        if ctx.input_mut(|i| i.consume_shortcut(&prev_tab)) {
            self.tabs.cycle(&mut self.doc, false);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Tab))) {
            self.tabs.cycle(&mut self.doc, true);
        }
        if !self.has_doc() {
            return;
        }
        self.view_shortcuts(ctx);
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::W)))
            && let Some(i) = self.tabs.active()
        {
            self.request_close_tab(i);
        }
        let save_as = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        if ctx.input_mut(|i| i.consume_shortcut(&save_as)) {
            self.save_as();
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S))) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::E))) {
            self.open_export();
        }
        let redo_shift = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        if ctx.input_mut(|i| i.consume_shortcut(&redo_shift) || i.consume_shortcut(&redo)) {
            self.doc.editor.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.doc.editor.undo();
        }
        let select_all = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
        let reselect = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::D);
        let deselect = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
        let inverse = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::I);
        let clipping = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::G);
        if ctx.input_mut(|i| i.consume_shortcut(&clipping)) {
            let layer = self.doc.editor.layer;
            self.doc.editor.toggle_clip(layer);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&select_all)) {
            self.doc.editor.select_all();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&reselect)) {
            self.doc.editor.reselect();
        } else if ctx.input_mut(|i| i.consume_shortcut(&deselect)) {
            self.doc.editor.deselect();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&inverse)) {
            self.doc.editor.invert_selection();
        }
        if ctx.egui_wants_keyboard_input() || self.doc.editor.is_painting() {
            return;
        }
        let new_frame = KeyboardShortcut::new(Modifiers::ALT, Key::N);
        if ctx.input_mut(|i| i.consume_shortcut(&new_frame)) {
            self.doc.editor.duplicate_frame();
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
            let editor = &mut self.doc.editor;
            let letter = key.name().chars().next().filter(|_| key.name().len() == 1);
            if let Some(tool) = letter.and_then(|c| ToolKind::from_key(c, shift)) {
                editor.set_tool(tool);
                continue;
            }
            match key {
                Key::N if shift => editor.add_layer(cs_core::LayerKind::Image),
                Key::Enter if editor.is_pending() => editor.finish_gesture(),
                Key::Escape => editor.cancel_gesture(),
                Key::Enter => editor.toggle_play(),
                Key::Comma | Key::ArrowLeft => editor.prev_frame(),
                Key::Period | Key::ArrowRight => editor.next_frame(),
                Key::ArrowUp => editor.step_layer(true),
                Key::ArrowDown => editor.step_layer(false),
                Key::F3 => editor.onion_enabled = !editor.onion_enabled,
                Key::X => editor.swap_colors(),
                Key::Delete | Key::Backspace => editor.clear_selection_content(),
                _ => {}
            }
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let has_doc = self.has_doc();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let ctx = ui.ctx().clone();
                let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
                if ui.add(item("New", "Ctrl+N")).clicked() {
                    self.new_document();
                }
                if ui.add(item("Open…", "Ctrl+O")).clicked() {
                    self.open_dialog(&ctx);
                }
                ui.separator();
                let (save, save_as) = if self.platform.saves_by_download() {
                    ("Save (download)", "Save As… (download)")
                } else {
                    ("Save", "Save As…")
                };
                if ui.add_enabled(has_doc, item(save, "Ctrl+S")).clicked() {
                    self.save();
                }
                if ui
                    .add_enabled(has_doc, item(save_as, "Ctrl+Shift+S"))
                    .clicked()
                {
                    self.save_as();
                }
                let close = item(
                    "Close",
                    if cfg!(target_arch = "wasm32") {
                        ""
                    } else {
                        "Ctrl+W"
                    },
                );
                if ui.add_enabled(has_doc, close).clicked()
                    && let Some(i) = self.tabs.active()
                {
                    self.request_close_tab(i);
                }
                ui.separator();
                if ui
                    .add_enabled(has_doc, egui::Button::new("Import PNG as Layer…"))
                    .clicked()
                {
                    self.import_layer(&ctx);
                }
                if ui.add_enabled(has_doc, item("Export…", "Ctrl+E")).clicked() {
                    self.open_export();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.request_quit(&ctx);
                }
            });
            ui.add_enabled_ui(has_doc, |ui| {
                ui.menu_button("Sprite", |ui| {
                    if ui.button("Canvas Size…").clicked() {
                        self.open_canvas_size();
                    }
                });
                ui.menu_button("View", |ui| self.view_menu(ui));
                ui.menu_button("Edit", |ui| {
                    let undo = egui::Button::new("Undo").shortcut_text("Ctrl+Z");
                    if ui
                        .add_enabled(self.doc.editor.history.can_undo(), undo)
                        .clicked()
                    {
                        self.doc.editor.undo();
                    }
                    let redo = egui::Button::new("Redo").shortcut_text("Ctrl+Y");
                    if ui
                        .add_enabled(self.doc.editor.history.can_redo(), redo)
                        .clicked()
                    {
                        self.doc.editor.redo();
                    }
                    ui.separator();
                    let has_selection = self.doc.editor.selection.is_some();
                    let clear = egui::Button::new("Clear").shortcut_text("Del");
                    if ui.add_enabled(has_selection, clear).clicked() {
                        self.doc.editor.clear_selection_content();
                    }
                });
                ui.menu_button("Layer", |ui| {
                    let layer = self.doc.editor.layer;
                    timeline::mask_menu(ui, &mut self.doc.editor, layer);
                });
                ui.menu_button("Select", |ui| {
                    let editor = &mut self.doc.editor;
                    let has_selection = editor.selection.is_some();
                    if ui
                        .add(egui::Button::new("All").shortcut_text("Ctrl+A"))
                        .clicked()
                    {
                        editor.select_all();
                    }
                    let deselect = egui::Button::new("Deselect").shortcut_text("Ctrl+D");
                    if ui.add_enabled(has_selection, deselect).clicked() {
                        editor.deselect();
                    }
                    let reselect = egui::Button::new("Reselect").shortcut_text("Ctrl+Shift+D");
                    if ui
                        .add_enabled(!has_selection && editor.can_reselect(), reselect)
                        .clicked()
                    {
                        editor.reselect();
                    }
                    let inverse = egui::Button::new("Inverse").shortcut_text("Ctrl+Shift+I");
                    if ui.add(inverse).clicked() {
                        editor.invert_selection();
                    }
                });
            });
        });
    }

    /// Barra de pestañas: Home + documentos.
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut switch_to = None;
        let mut close = None;
        let mut home = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if ui.selectable_label(!self.has_doc(), "🏠 Home").clicked() {
                home = true;
            }
            for (i, tab) in self.tabs.infos(&self.doc).into_iter().enumerate() {
                let active = self.tabs.active() == Some(i);
                let label = format!("{}{}", tab.name, if tab.dirty { " •" } else { "" });
                ui.add_space(4.0);
                let response = ui.selectable_label(active, label);
                if response.clicked() {
                    switch_to = Some(i);
                }
                if response.middle_clicked() {
                    close = Some(i);
                }
                if ui.small_button("×").on_hover_text("Close").clicked() {
                    close = Some(i);
                }
            }
        });
        if home {
            self.tabs.go_home(&mut self.doc);
        }
        if let Some(i) = switch_to {
            self.tabs.switch(&mut self.doc, i);
        }
        if let Some(i) = close {
            self.request_close_tab(i);
        }
    }

    /// Paneles del editor (cuando hay un documento activo).
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) {
        if self.view.timeline {
            egui::Panel::bottom("timeline")
                .resizable(true)
                .default_size(190.0)
                .show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| {
                        timeline::show(ui, &mut self.doc.editor, &mut self.doc.timeline)
                    });
                });
        }
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(184.0)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor));
            });
        egui::Panel::right("palette")
            .default_size(220.0)
            .show(ui, |ui| {
                let open = egui::ScrollArea::vertical()
                    .show(ui, |ui| {
                        panels::palette(ui, &mut self.doc.editor);
                        ui.add_space(12.0);
                        let open = ui
                            .add_enabled_ui(!modal, |ui| {
                                panels::artboards(
                                    ui,
                                    &mut self.doc.editor,
                                    &mut self.doc.artboard_name,
                                )
                            })
                            .inner;
                        ui.add_space(12.0);
                        ui.add_enabled_ui(!modal, |ui| panels::slices(ui, &mut self.doc.editor));
                        open
                    })
                    .inner;
                if open {
                    self.open_canvas_size();
                }
            });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            self.doc.canvas.show(
                ui,
                &mut self.doc.editor,
                self.canvas_size.as_mut(),
                &self.view,
            )
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
        if self.has_doc() && self.doc.editor.playing {
            let dt_ms = ctx.input(|i| i.stable_dt) as f64 * 1000.0;
            self.doc.editor.tick(dt_ms);
            ctx.request_repaint();
        }

        let modal = self.is_modal();
        egui::Panel::top("menu").show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| self.menu_bar(ui));
        });
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| self.tab_bar(ui));
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            let notice = self
                .autosaver
                .disabled_reason
                .as_ref()
                .map(|r| format!("Autosave off: {r}"));
            if self.has_doc() {
                panels::status_bar(ui, &self.doc.editor, &self.doc.canvas, notice.as_deref());
            } else {
                ui.label(format!("ChimpSprite {}", env!("CARGO_PKG_VERSION")));
            }
        });
        if self.has_doc() {
            self.editor_panels(ui, modal);
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.home(ui));
            });
        }
        self.update_canvas_size(&ctx);
        self.update_grid_dialog(&ctx);
        self.show_dialogs(&ctx);
        self.show_restore(&ctx);
        self.autosave(&ctx);
        self.update_title(&ctx);
        platform::set_unsaved(self.any_dirty());

        self.capture.update(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, RECENT_KEY, &self.recent);
        eframe::set_value(storage, VIEW_KEY, &self.view);
    }
}
