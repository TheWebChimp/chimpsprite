//! Menú View: qué se muestra en el lienzo, grid, zoom y paneles.

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, GridSettings};
use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

use crate::editor::Editor;
use crate::widgets;

/// Preferencias de vista (globales, persisten entre sesiones).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewSettings {
    /// Interruptor general de los extras (`Ctrl+H`).
    pub extras: bool,
    pub pixel_grid: bool,
    pub grid: bool,
    pub selection_edges: bool,
    pub brush_preview: bool,
    pub artboard_names: bool,
    pub grid_color: [u8; 4],
    pub timeline: bool,
    /// Las reglas son parte de la interfaz: no dependen de Extras.
    pub rulers: bool,
    pub guides: bool,
    pub snap_to_guides: bool,
    pub snap_to_grid: bool,
    pub lock_guides: bool,
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            extras: true,
            pixel_grid: true,
            grid: false,
            selection_edges: true,
            brush_preview: true,
            artboard_names: true,
            grid_color: [80, 140, 255, 110],
            timeline: true,
            rulers: true,
            guides: true,
            snap_to_guides: true,
            snap_to_grid: false,
            lock_guides: false,
        }
    }
}

impl ViewSettings {
    /// Un extra se ve solo si el interruptor general está encendido.
    pub fn shows(&self, flag: bool) -> bool {
        self.extras && flag
    }

    pub fn grid_color(&self) -> Color32 {
        let [r, g, b, a] = self.grid_color;
        Color32::from_rgba_unmultiplied(r, g, b, a)
    }
}

/// Cambio de zoom pedido desde el menú o un atajo; el lienzo lo aplica al dibujarse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomRequest {
    In,
    Out,
    /// 100%.
    Actual,
    Fit,
}

/// Diálogo "Grid Settings" del artboard activo.
pub struct GridDialog {
    pub artboard: ArtboardId,
    pub grid: GridSettings,
    pub color: Color32,
}

pub enum GridDialogResult {
    Open,
    Cancel,
    Accept,
}

impl GridDialog {
    pub fn new(editor: &Editor, view: &ViewSettings) -> Self {
        Self {
            artboard: editor.active,
            grid: editor.sprite().grid,
            color: view.grid_color(),
        }
    }

    /// Comando que aplica el grid (o `None` si no cambió).
    pub fn command(&self, editor: &Editor) -> Option<DocCommand> {
        let current = editor.document.artboard(self.artboard)?.sprite.grid;
        (current != self.grid).then_some(DocCommand::OnArtboard {
            id: self.artboard,
            cmd: Command::SetGrid(self.grid),
        })
    }

    pub fn show(&mut self, ctx: &egui::Context) -> GridDialogResult {
        let mut result = GridDialogResult::Open;
        let mut open = true;
        egui::Window::new("Grid Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("grid_settings")
                    .num_columns(4)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Width:");
                        widgets::number(ui, &mut self.grid.size.0, 1..=4096, " px");
                        ui.label("Height:");
                        widgets::number(ui, &mut self.grid.size.1, 1..=4096, " px");
                        ui.end_row();
                        ui.label("Offset X:");
                        widgets::number(ui, &mut self.grid.offset.0, -4096..=4096, " px");
                        ui.label("Offset Y:");
                        widgets::number(ui, &mut self.grid.offset.1, -4096..=4096, " px");
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    ui.label("Color:");
                    egui::color_picker::color_edit_button_srgba(
                        ui,
                        &mut self.color,
                        egui::color_picker::Alpha::OnlyBlend,
                    );
                    ui.weak("(shared by all documents)");
                });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        result = GridDialogResult::Accept;
                    }
                    if ui.button("Cancel").clicked() {
                        result = GridDialogResult::Cancel;
                    }
                });
            });
        let (enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if !open || escape {
            GridDialogResult::Cancel
        } else if enter {
            GridDialogResult::Accept
        } else {
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;

    #[test]
    fn extras_switch_hides_everything() {
        let mut v = ViewSettings::default();
        assert!(v.shows(v.pixel_grid));
        v.extras = false;
        assert!(!v.shows(v.pixel_grid) && !v.shows(v.selection_edges));
    }

    #[test]
    fn grid_dialog_command_only_when_changed() {
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let view = ViewSettings::default();
        let mut d = GridDialog::new(&editor, &view);
        assert!(d.command(&editor).is_none());
        d.grid.size = (4, 2);
        let cmd = d.command(&editor).unwrap();
        editor.execute_doc(cmd);
        assert_eq!(editor.sprite().grid.size, (4, 2));
        editor.undo();
        assert_eq!(editor.sprite().grid.size, (16, 16));
    }

    #[test]
    fn settings_roundtrip_through_json_with_defaults() {
        let v: ViewSettings = serde_json::from_str(r#"{"grid":true}"#).unwrap();
        assert!(v.grid && v.pixel_grid && v.timeline);
    }
}
