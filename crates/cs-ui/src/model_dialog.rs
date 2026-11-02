//! New 3D Model…: el nombre y el tamaño del artboard de un modelo 3D nuevo.

use eframe::egui;

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window, finish};
use crate::widgets;

/// Lados mínimo y máximo del artboard de un modelo nuevo.
const MIN_SIDE: u32 = 16;
const MAX_SIDE: u32 = 512;

pub struct NewModelDialog {
    pub name: String,
    pub size: (u32, u32),
}

impl Default for NewModelDialog {
    fn default() -> Self {
        NewModelDialog {
            name: "Model".into(),
            size: (128, 128),
        }
    }
}

impl NewModelDialog {
    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) = dialog_window(ctx, skin, "new_model", "New 3D Model", |ui| {
            egui::Grid::new("new_model_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name:");
                    widgets::text_line(ui, &mut self.name);
                    ui.end_row();
                    ui.label("Width:");
                    widgets::number(ui, &mut self.size.0, MIN_SIDE..=MAX_SIDE, " px");
                    ui.end_row();
                    ui.label("Height:");
                    widgets::number(ui, &mut self.size.1, MIN_SIDE..=MAX_SIDE, " px");
                    ui.end_row();
                });
        });
        finish(ctx, open, result)
    }
}
