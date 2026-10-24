//! Diálogos de tilemaps: nueva capa tilemap y tamaño de tile para convertir una capa.

use cs_commands::ops::MAX_TILE_SIDE;
use eframe::egui;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tile_ops::TilesetChoice;
use crate::{pixel_ui, widgets};

/// Lo que el usuario decidió en este frame.
pub enum DialogResult {
    Open,
    Accept,
    Cancel,
}

/// Tamaño de tile por defecto: el de la grilla de trabajo del sprite, acotado a 1–256.
pub fn default_tile_size(editor: &Editor) -> (u32, u32) {
    let (w, h) = editor.sprite().grid.size;
    (w.clamp(1, MAX_TILE_SIDE), h.clamp(1, MAX_TILE_SIDE))
}

/// Enter acepta y Escape (o cerrar la ventana) cancela, como en los demás diálogos.
fn finish(ctx: &egui::Context, open: bool, result: DialogResult) -> DialogResult {
    let (enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if !open || escape {
        DialogResult::Cancel
    } else if enter {
        DialogResult::Accept
    } else {
        result
    }
}

fn buttons(ui: &mut egui::Ui, result: &mut DialogResult) {
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("OK").clicked() {
            *result = DialogResult::Accept;
        }
        if ui.button("Cancel").clicked() {
            *result = DialogResult::Cancel;
        }
    });
}

/// La ventana de un diálogo de tilemaps con lo que agregue `add_contents` y, abajo, OK y Cancel.
/// Con `skin` (Pixel UI) es el diálogo pixel del skin con los botones pixel y el botón de cerrar
/// (que cancela); sin él, la ventana de egui de siempre. `id` es el del diálogo pixel. Devuelve
/// lo elegido con los botones y si sigue abierta.
fn dialog_window(
    ctx: &egui::Context,
    skin: Option<&Skin>,
    id: &str,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> (DialogResult, bool) {
    match skin {
        Some(skin) => {
            let out = pixel_ui::pixel_dialog(ctx, skin, egui::Id::new(id), title, true, |ui| {
                add_contents(ui);
                ui.add_space(6.0);
                let buttons = pixel_ui::pixel_buttons(ui, skin, &["OK", "Cancel"]);
                if buttons[0].clicked() {
                    DialogResult::Accept
                } else if buttons[1].clicked() {
                    DialogResult::Cancel
                } else {
                    DialogResult::Open
                }
            });
            // Cerrar desde la barra de título es lo mismo que Cancel.
            (out.inner, !out.closed)
        }
        None => {
            let mut result = DialogResult::Open;
            let mut open = true;
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ctx, |ui| {
                    add_contents(ui);
                    buttons(ui, &mut result);
                });
            (result, open)
        }
    }
}

fn tileset_label(t: &(u32, String, (u32, u32))) -> String {
    format!("{}  ({}×{})", t.1, t.2.0, t.2.1)
}

/// `Layer → New Tilemap Layer…`: tileset existente o nuevo (nombre, tamaño de tile, base index).
pub struct NewTilemapDialog {
    /// `None` = tileset nuevo.
    pub existing: Option<u32>,
    pub name: String,
    pub tile_size: (u32, u32),
    pub base_index: i16,
    /// Tilesets del sprite para el selector: índice, nombre y tamaño de tile.
    tilesets: Vec<(u32, String, (u32, u32))>,
}

impl NewTilemapDialog {
    pub fn new(editor: &Editor) -> Self {
        let sprite = editor.sprite();
        Self {
            existing: None,
            name: format!("Tileset {}", sprite.tilesets.len() + 1),
            tile_size: default_tile_size(editor),
            base_index: 1,
            tilesets: sprite
                .tilesets
                .iter()
                .enumerate()
                .map(|(i, t)| (i as u32, t.name.clone(), t.tile_size))
                .collect(),
        }
    }

    pub fn choice(&self) -> TilesetChoice {
        match self.existing {
            Some(i) => TilesetChoice::Existing(i),
            None => TilesetChoice::New {
                name: self.name.clone(),
                tile_size: self.tile_size,
                base_index: self.base_index,
            },
        }
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) =
            dialog_window(ctx, skin, "new_tilemap_layer", "New Tilemap Layer", |ui| {
                let selected = self
                    .existing
                    .and_then(|i| self.tilesets.iter().find(|t| t.0 == i))
                    .map_or_else(|| "New tileset".to_string(), tileset_label);
                egui::ComboBox::from_label("Tileset")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.existing, None, "New tileset");
                        for t in &self.tilesets {
                            ui.selectable_value(&mut self.existing, Some(t.0), tileset_label(t));
                        }
                    });
                if self.existing.is_none() {
                    egui::Grid::new("new_tileset")
                        .num_columns(2)
                        .spacing([8.0, 6.0])
                        .show(ui, |ui| {
                            ui.label("Name:");
                            ui.text_edit_singleline(&mut self.name);
                            ui.end_row();
                            ui.label("Tile width:");
                            widgets::number(ui, &mut self.tile_size.0, 1..=MAX_TILE_SIDE, " px");
                            ui.end_row();
                            ui.label("Tile height:");
                            widgets::number(ui, &mut self.tile_size.1, 1..=MAX_TILE_SIDE, " px");
                            ui.end_row();
                            ui.label("Base index:");
                            widgets::number(ui, &mut self.base_index, i16::MIN..=i16::MAX, "");
                            ui.end_row();
                        });
                }
            });
        finish(ctx, open, result)
    }
}

/// `Layer → Convert to Tilemap…`: tamaño de tile.
pub struct TileSizeDialog {
    pub tile_size: (u32, u32),
}

impl TileSizeDialog {
    pub fn new(editor: &Editor) -> Self {
        Self {
            tile_size: default_tile_size(editor),
        }
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) = dialog_window(
            ctx,
            skin,
            "convert_to_tilemap",
            "Convert to Tilemap",
            |ui| {
                egui::Grid::new("convert_tile_size")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Tile width:");
                        widgets::number(ui, &mut self.tile_size.0, 1..=MAX_TILE_SIDE, " px");
                        ui.end_row();
                        ui.label("Tile height:");
                        widgets::number(ui, &mut self.tile_size.1, 1..=MAX_TILE_SIDE, " px");
                        ui.end_row();
                    });
                ui.weak(
                    "Cuts the layer into cells of the canvas grid; identical cells share a tile.",
                );
            },
        );
        finish(ctx, open, result)
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;
    use crate::tile_ops::TilesetChoice;

    #[test]
    fn dialogs_default_to_the_work_grid_size() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        let d = NewTilemapDialog::new(&e);
        assert_eq!(
            d.choice(),
            TilesetChoice::New {
                name: "Tileset 1".into(),
                tile_size: (16, 16),
                base_index: 1
            }
        );
        e.execute(cs_commands::Command::SetGrid(cs_core::GridSettings {
            size: (8, 300),
            offset: (0, 0),
        }));
        assert_eq!(
            TileSizeDialog::new(&e).tile_size,
            (8, 256),
            "clamped to 1-256"
        );
    }
}
