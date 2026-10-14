//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod canvas;
mod capture;
mod demo;
mod editor;
mod panels;
mod theme;

use cs_core::{ColorMode, Sprite};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

pub use editor::{Editor, ToolKind};

const NEW_SPRITE_SIZE: u32 = 64;

pub struct ChimpApp {
    editor: Editor,
    canvas: canvas::CanvasView,
    capture: capture::Capture,
}

impl ChimpApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let mut editor = Editor::new(Sprite::new(NEW_SPRITE_SIZE, NEW_SPRITE_SIZE, ColorMode::Rgba));
        if std::env::var_os("CHIMPSPRITE_DEMO").is_some() {
            demo::draw(&mut editor);
        }
        Self {
            editor,
            canvas: canvas::CanvasView::default(),
            capture: capture::Capture::from_env(),
        }
    }

    fn new_sprite(&mut self) {
        self.editor = Editor::new(Sprite::new(NEW_SPRITE_SIZE, NEW_SPRITE_SIZE, ColorMode::Rgba));
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
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            if !i.modifiers.any() {
                if i.key_pressed(Key::B) {
                    self.editor.tool = ToolKind::Pencil;
                }
                if i.key_pressed(Key::E) {
                    self.editor.tool = ToolKind::Eraser;
                }
                if i.key_pressed(Key::X) {
                    self.editor.swap_colors();
                }
            }
        });
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
                if ui.add_enabled(self.editor.history.can_undo(), undo).clicked() {
                    self.editor.undo();
                }
                let redo = egui::Button::new("Redo").shortcut_text("Ctrl+Y");
                if ui.add_enabled(self.editor.history.can_redo(), redo).clicked() {
                    self.editor.redo();
                }
            });
        });
    }
}

impl eframe::App for ChimpApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            panels::status_bar(ui, &self.editor, &self.canvas)
        });
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(150.0)
            .show(ui, |ui| panels::toolbar(ui, &mut self.editor));
        egui::Panel::right("palette")
            .default_size(220.0)
            .show(ui, |ui| panels::palette(ui, &mut self.editor));
        egui::CentralPanel::no_frame().show(ui, |ui| self.canvas.show(ui, &mut self.editor));

        self.capture.update(&ctx);
    }
}
