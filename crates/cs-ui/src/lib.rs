//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod canvas;
mod capture;
mod demo;
mod editor;
mod panels;
mod theme;

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
}

impl ChimpApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let editor = if std::env::var_os("CHIMPSPRITE_DEMO").is_some() {
            demo::editor()
        } else {
            new_editor()
        };
        Self {
            editor,
            canvas: canvas::CanvasView::default(),
            capture: capture::Capture::from_env(),
            artboard_name: (ArtboardId(0), String::new()),
        }
    }

    fn new_sprite(&mut self) {
        self.editor = new_editor();
        self.canvas.reset();
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
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
                if ui.button("New").clicked() {
                    self.new_sprite();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
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
        self.handle_shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status")
            .show(ui, |ui| panels::status_bar(ui, &self.editor, &self.canvas));
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(150.0)
            .show(ui, |ui| panels::toolbar(ui, &mut self.editor));
        egui::Panel::right("palette")
            .default_size(220.0)
            .show(ui, |ui| {
                panels::palette(ui, &mut self.editor);
                ui.add_space(12.0);
                panels::artboards(ui, &mut self.editor, &mut self.artboard_name);
            });
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas.show(ui, &mut self.editor));

        self.capture.update(&ctx);
    }
}
