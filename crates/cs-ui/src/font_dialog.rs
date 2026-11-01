//! New Font…: el nombre, la celda y el modo de una fuente pixel nueva.

use cs_core::FontMode;
use eframe::egui;

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window, finish};
use crate::widgets;

/// Lado máximo de la celda de una fuente nueva.
const MAX_CELL_SIDE: u32 = 64;

pub struct NewFontDialog {
    pub name: String,
    pub cell: (u32, u32),
    pub mode: FontMode,
}

impl Default for NewFontDialog {
    fn default() -> Self {
        NewFontDialog {
            name: "New Font".into(),
            cell: (16, 16),
            mode: FontMode::OneBit,
        }
    }
}

impl NewFontDialog {
    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) = dialog_window(ctx, skin, "new_font", "New Font", |ui| {
            egui::Grid::new("new_font_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name:");
                    widgets::text_line(ui, &mut self.name);
                    ui.end_row();
                    ui.label("Cell width:");
                    widgets::number(ui, &mut self.cell.0, 1..=MAX_CELL_SIDE, " px");
                    ui.end_row();
                    ui.label("Cell height:");
                    widgets::number(ui, &mut self.cell.1, 1..=MAX_CELL_SIDE, " px");
                    ui.end_row();
                    ui.label("Mode:");
                    widgets::combo(ui, "new_font_mode", mode_name(self.mode), |ui| {
                        for mode in [FontMode::OneBit, FontMode::Color] {
                            widgets::choice(ui, &mut self.mode, mode, mode_name(mode));
                        }
                    });
                    ui.end_row();
                });
        });
        finish(ctx, open, result)
    }
}

pub fn mode_name(mode: FontMode) -> &'static str {
    match mode {
        FontMode::OneBit => "1 bit",
        FontMode::Color => "Color",
    }
}
