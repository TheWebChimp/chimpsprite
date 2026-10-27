//! Edit → Preferences…: las preferencias de la app (las que se guardan entre sesiones), por
//! sección como en Aseprite. Se aplican al momento; Reset vuelve a las de fábrica.

use eframe::egui::{self, Align2, RichText, Ui};

use crate::skin::Skin;
use crate::view::ViewSettings;
use crate::widgets;

const WIDTH: f32 = 240.0;

/// Lo que pidió la ventana además de cambiar las preferencias.
#[derive(Default)]
pub struct PreferencesOutput {
    pub closed: bool,
    /// Abrir Grid Settings (el color y el tamaño de la grilla).
    pub grid_settings: bool,
}

pub fn show(
    ctx: &egui::Context,
    view: &mut ViewSettings,
    skin: Option<&Skin>,
) -> PreferencesOutput {
    let mut out = PreferencesOutput::default();
    let anchor = (Align2::CENTER_CENTER, egui::Vec2::ZERO);
    let (_, closed) = crate::pixel_ui::app_window(
        ctx,
        skin,
        "preferences",
        "Preferences",
        true,
        anchor,
        |ui| {
            ui.set_width(WIDTH);
            contents(ui, view, skin, &mut out);
            ui.add_space(6.0);
            match crate::pixel_ui::dialog_buttons(ui, skin, &["OK", "Reset"]) {
                Some(0) => out.closed = true,
                Some(_) => {
                    *view = ViewSettings {
                        // La caja de la paleta y su candado no son de esta ventana.
                        palette_height: view.palette_height,
                        palette_locked: view.palette_locked,
                        ..ViewSettings::default()
                    }
                }
                None => {}
            }
        },
    );
    let done = ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter));
    out.closed |= closed || done;
    out
}

fn section(ui: &mut Ui, skin: Option<&Skin>, title: &str) {
    ui.add_space(4.0);
    let dim = skin.map_or(ui.visuals().weak_text_color(), |s| s.colors().text_dim);
    ui.label(RichText::new(title).color(dim));
}

fn contents(
    ui: &mut Ui,
    view: &mut ViewSettings,
    skin: Option<&Skin>,
    out: &mut PreferencesOutput,
) {
    section(ui, skin, "General");
    widgets::checkbox(ui, &mut view.pixel_ui, "Pixel UI");
    ui.add_enabled_ui(view.pixel_ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("UI Scale");
            let label = format!("{}×", view.ui_scale);
            widgets::combo(ui, "pref_ui_scale", &label, |ui| {
                for scale in 1..=3u8 {
                    widgets::choice(ui, &mut view.ui_scale, scale, &format!("{scale}×"));
                }
            });
        });
    });
    section(ui, skin, "Editor");
    widgets::checkbox(ui, &mut view.rulers, "Rulers");
    widgets::checkbox(ui, &mut view.brush_preview, "Brush Preview");
    widgets::checkbox(ui, &mut view.pixel_grid, "Pixel Grid");
    widgets::checkbox(ui, &mut view.selection_edges, "Selection Edges");
    widgets::checkbox(ui, &mut view.artboard_names, "Artboard Names");
    widgets::checkbox(ui, &mut view.slices, "Slices");
    section(ui, skin, "Guides & Grid");
    widgets::checkbox(ui, &mut view.guides, "Show Guides");
    widgets::checkbox(ui, &mut view.snap_to_guides, "Snap to Guides");
    widgets::checkbox(ui, &mut view.lock_guides, "Lock Guides");
    widgets::checkbox(ui, &mut view.grid, "Show Grid");
    widgets::checkbox(ui, &mut view.snap_to_grid, "Snap to Grid");
    if crate::pixel_ui::text_button(ui, "Grid Settings…").clicked() {
        out.grid_settings = true;
    }
    section(ui, skin, "Timeline");
    widgets::checkbox(ui, &mut view.timeline, "Show Timeline");
    widgets::checkbox(ui, &mut view.tile_numbers, "Tile Numbers");
}
