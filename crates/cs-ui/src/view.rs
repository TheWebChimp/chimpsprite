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
    /// Rectángulos de los slices (hitboxes, pivotes).
    pub slices: bool,
    pub grid_color: [u8; 4],
    pub timeline: bool,
    /// Las reglas son parte de la interfaz: no dependen de Extras.
    pub rulers: bool,
    pub guides: bool,
    pub snap_to_guides: bool,
    pub snap_to_grid: bool,
    pub lock_guides: bool,
    /// Número de cada celda no vacía de la capa tilemap activa.
    pub tile_numbers: bool,
    /// Interfaz en pixel art (View → Pixel UI): fuente pixel, escala entera y skin. Encendida
    /// de fábrica, también si lo guardado no la trae (`serde(default)`); apagada solo si se
    /// guardó apagada.
    pub pixel_ui: bool,
    /// Escala de la UI con Pixel UI (1, 2 o 3): píxeles de pantalla por píxel del skin.
    pub ui_scale: u8,
    /// El candado de la paleta (Pixel UI): bloqueada, sus colores no se editan uno por uno
    /// (`Editor::palette_locked`). Bloqueada de fábrica.
    pub palette_locked: bool,
    /// El alto de la caja de la paleta (Pixel UI), elegido con la división que la separa del
    /// editor de color. De fábrica, el del diseño; el panel lo achica si no entra.
    pub palette_height: f32,
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
            slices: true,
            grid_color: [80, 140, 255, 110],
            timeline: true,
            rulers: true,
            guides: true,
            snap_to_guides: true,
            snap_to_grid: false,
            lock_guides: false,
            tile_numbers: false,
            pixel_ui: true,
            ui_scale: 2,
            palette_locked: true,
            palette_height: crate::color_panel::PALETTE_DEFAULT_HEIGHT,
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

    /// `ui_scale` entre 1 y 3 (un valor guardado a mano puede salirse).
    pub fn pixel_scale(&self) -> u8 {
        self.ui_scale.clamp(1, 3)
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
    /// Centra el artboard activo sin cambiar el zoom.
    CenterActive,
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

    /// La ventana (la de la app: con `skin`, la de Pixel UI).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        skin: Option<&crate::skin::Skin>,
    ) -> GridDialogResult {
        let mut result = GridDialogResult::Open;
        let (_, closed) = crate::pixel_ui::app_window(
            ctx,
            skin,
            "grid_settings_window",
            "Grid Settings",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
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
                ui.add_space(6.0);
                match crate::pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"]) {
                    Some(0) => result = GridDialogResult::Accept,
                    Some(_) => result = GridDialogResult::Cancel,
                    None => {}
                }
            },
        );
        let (enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if closed || escape {
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
    fn pixel_ui_settings_roundtrip_and_start_on() {
        // La app abre con Pixel UI, también con lo guardado antes de que existiera.
        assert!(ViewSettings::default().pixel_ui);
        let v: ViewSettings = serde_json::from_str(r#"{"grid":true}"#).unwrap();
        assert!(v.pixel_ui);
        assert_eq!(v.ui_scale, 2);
        // Quien lo apagó y guardó, lo sigue teniendo apagado.
        let off = ViewSettings {
            pixel_ui: false,
            ..Default::default()
        };
        let back: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
        assert_eq!(back, off);
        let v: ViewSettings = serde_json::from_str(r#"{"grid":true,"pixel_ui":false}"#).unwrap();
        assert!(!v.pixel_ui && v.grid);
        let on = ViewSettings {
            pixel_ui: true,
            ui_scale: 3,
            ..Default::default()
        };
        let back: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert_eq!(back, on);
        assert_eq!(ViewSettings { ui_scale: 9, ..on }.pixel_scale(), 3);
        assert_eq!(ViewSettings { ui_scale: 0, ..on }.pixel_scale(), 1);
    }

    #[test]
    fn the_palette_lock_is_saved_and_starts_locked() {
        // Bloqueada de fábrica, también con lo guardado antes de que existiera.
        assert!(ViewSettings::default().palette_locked);
        let v: ViewSettings = serde_json::from_str(r#"{"grid":true}"#).unwrap();
        assert!(v.palette_locked);
        let open = ViewSettings {
            palette_locked: false,
            ..Default::default()
        };
        let back: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&open).unwrap()).unwrap();
        assert_eq!(back, open);
    }

    #[test]
    fn the_palette_height_is_saved_and_starts_at_the_design_one() {
        assert_eq!(ViewSettings::default().palette_height, 207.0);
        // Lo guardado antes de que existiera la división: el de fábrica.
        let v: ViewSettings = serde_json::from_str(r#"{"grid":true}"#).unwrap();
        assert_eq!(v.palette_height, 207.0);
        let tall = ViewSettings {
            palette_height: 260.0,
            ..Default::default()
        };
        let back: ViewSettings =
            serde_json::from_str(&serde_json::to_string(&tall).unwrap()).unwrap();
        assert_eq!(back.palette_height, 260.0);
    }

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
        assert!(!v.tile_numbers, "tile numbers start hidden");
        let v: ViewSettings = serde_json::from_str(r#"{"tile_numbers":true}"#).unwrap();
        assert!(v.tile_numbers && v.timeline);
    }
}
