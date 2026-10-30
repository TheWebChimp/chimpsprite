//! `Layer → Convert to Symbol…` (F8): el nombre del símbolo y su tipo.

use cs_commands::symbol::SymbolTiming;
use eframe::egui;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window, finish};
use crate::widgets;

/// Lo que se eligió en el diálogo.
pub struct SymbolDialog {
    pub name: String,
    pub timing: SymbolTiming,
}

impl SymbolDialog {
    /// Con el nombre que propone el editor y el tipo Graphic.
    pub fn new(editor: &Editor) -> Self {
        Self {
            name: editor.suggested_symbol_name(),
            timing: SymbolTiming::Graphic,
        }
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) =
            dialog_window(ctx, skin, "convert_to_symbol", "Convert to Symbol", |ui| {
                egui::Grid::new("convert_to_symbol_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Name");
                        widgets::text_line_named(ui, &mut self.name, "Name");
                        ui.end_row();
                        ui.label("Type");
                        ui.horizontal(|ui| {
                            widgets::radio_value(
                                ui,
                                &mut self.timing,
                                SymbolTiming::Graphic,
                                "Graphic",
                            );
                            widgets::radio_value(
                                ui,
                                &mut self.timing,
                                SymbolTiming::MovieClip,
                                "Movie Clip",
                            );
                        });
                        ui.end_row();
                    });
            });
        finish(ctx, open, result)
    }
}
