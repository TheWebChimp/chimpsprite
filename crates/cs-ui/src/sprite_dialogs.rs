//! Diálogos del menú Sprite: propiedades, duplicar y tamaño del sprite (escalar).

use cs_core::{Artboard, ColorMode};
use eframe::egui;

use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

const MAX_SIZE: u32 = 16384;

/// El diálogo del menú Sprite que está abierto.
pub enum SpriteDialog {
    Properties(PropertiesDialog),
    Duplicate(DuplicateDialog),
    Size(SpriteSizeDialog),
}

impl SpriteDialog {
    /// Con `skin` (Pixel UI), en el diálogo pixel del skin. Enter acepta; Escape y la × cancelan.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (id, title) = match self {
            SpriteDialog::Properties(_) => ("sprite_properties", "Sprite Properties"),
            SpriteDialog::Duplicate(_) => ("duplicate_sprite", "Duplicate Sprite"),
            SpriteDialog::Size(_) => ("sprite_size", "Sprite Size"),
        };
        let mut result = DialogResult::Open;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            id,
            title,
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                match self {
                    SpriteDialog::Properties(d) => d.contents(ui),
                    SpriteDialog::Duplicate(d) => d.contents(ui),
                    SpriteDialog::Size(d) => d.contents(ui),
                }
                ui.add_space(6.0);
                match pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"]) {
                    Some(0) => result = DialogResult::Accept,
                    Some(_) => result = DialogResult::Cancel,
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
            DialogResult::Cancel
        } else if enter {
            DialogResult::Accept
        } else {
            result
        }
    }
}

/// `Sprite → Properties…`: el nombre, lo que es el sprite y, si es indexado, su índice
/// transparente.
pub struct PropertiesDialog {
    pub name: String,
    /// Solo en indexado.
    pub transparent_index: Option<u8>,
    /// Proporción de píxel (ancho, alto).
    pub pixel_ratio: (u8, u8),
    info: Vec<(&'static str, String)>,
}

/// Las proporciones de píxel de Aseprite.
const PIXEL_RATIOS: [((u8, u8), &str); 3] = [
    ((1, 1), "Square Pixels (1:1)"),
    ((2, 1), "Double-wide Pixels (2:1)"),
    ((1, 2), "Double-high Pixels (1:2)"),
];

impl PropertiesDialog {
    pub fn new(artboard: &Artboard) -> Self {
        let s = &artboard.sprite;
        let mode = match s.color_mode {
            ColorMode::Rgba => "RGB Color".to_owned(),
            ColorMode::Grayscale => "Grayscale".to_owned(),
            ColorMode::Indexed { .. } => format!("Indexed ({} colors)", s.palette.len()),
        };
        Self {
            name: artboard.name.clone(),
            transparent_index: match s.color_mode {
                ColorMode::Indexed { transparent_index } => Some(transparent_index),
                ColorMode::Rgba | ColorMode::Grayscale => None,
            },
            pixel_ratio: s.pixel_ratio,
            info: vec![
                ("Color Mode:", mode),
                ("Size:", format!("{} × {} px", s.width, s.height)),
                ("Frames:", s.frames.len().to_string()),
                ("Layers:", s.layers.len().to_string()),
            ],
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("sprite_properties")
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                ui.label("Name:");
                widgets::text_line(ui, &mut self.name);
                ui.end_row();
                for (label, value) in &self.info {
                    ui.label(*label);
                    ui.label(value);
                    ui.end_row();
                }
                if let Some(index) = &mut self.transparent_index {
                    ui.label("Transparent Color:");
                    widgets::number(ui, index, 0..=255, "");
                    ui.end_row();
                }
                ui.label("Pixel Aspect Ratio:");
                let current = PIXEL_RATIOS
                    .iter()
                    .find(|(r, _)| *r == self.pixel_ratio)
                    .map_or("Custom", |(_, label)| *label);
                widgets::combo(ui, "pixel_ratio", current, |ui| {
                    for (ratio, label) in PIXEL_RATIOS {
                        widgets::choice(ui, &mut self.pixel_ratio, ratio, label);
                    }
                });
                ui.end_row();
            });
    }
}

/// `Sprite → Duplicate…`: una copia del sprite en un artboard nuevo, con este nombre.
pub struct DuplicateDialog {
    pub name: String,
}

impl DuplicateDialog {
    pub fn new(artboard: &Artboard) -> Self {
        Self {
            name: format!("{} Copy", artboard.name),
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Duplicate as:");
            widgets::text_line(ui, &mut self.name);
        });
    }
}

/// `Sprite → Sprite Size…`: escala todo el sprite (vecino más cercano), en píxeles o en
/// porcentaje y con la proporción trabada o no.
pub struct SpriteSizeDialog {
    original: (u32, u32),
    pub width: u32,
    pub height: u32,
    pub lock_ratio: bool,
}

impl SpriteSizeDialog {
    pub fn new(artboard: &Artboard) -> Self {
        let size = (artboard.sprite.width, artboard.sprite.height);
        Self {
            original: size,
            width: size.0,
            height: size.1,
            lock_ratio: true,
        }
    }

    pub fn set_width(&mut self, width: u32) {
        self.width = width.clamp(1, MAX_SIZE);
        if self.lock_ratio {
            let h = self.width as f64 * self.original.1 as f64 / self.original.0 as f64;
            self.height = (h.round() as u32).clamp(1, MAX_SIZE);
        }
    }

    pub fn set_height(&mut self, height: u32) {
        self.height = height.clamp(1, MAX_SIZE);
        if self.lock_ratio {
            let w = self.height as f64 * self.original.0 as f64 / self.original.1 as f64;
            self.width = (w.round() as u32).clamp(1, MAX_SIZE);
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let percent = |px: u32, of: u32| (px as f64 * 100.0 / of as f64).round() as u32;
        let from_percent = |pc: u32, of: u32| (of as f64 * pc as f64 / 100.0).round() as u32;
        widgets::checkbox(ui, &mut self.lock_ratio, "Lock Ratio");
        ui.add_space(4.0);
        egui::Grid::new("sprite_size")
            .num_columns(3)
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                let (ow, oh) = self.original;
                ui.label("Width:");
                let mut w = self.width;
                if widgets::number(ui, &mut w, 1..=MAX_SIZE, " px").changed() {
                    self.set_width(w);
                }
                let mut pw = percent(self.width, ow);
                if widgets::number(ui, &mut pw, 1..=10000, " %").changed() {
                    self.set_width(from_percent(pw, ow));
                }
                ui.end_row();
                ui.label("Height:");
                let mut h = self.height;
                if widgets::number(ui, &mut h, 1..=MAX_SIZE, " px").changed() {
                    self.set_height(h);
                }
                let mut ph = percent(self.height, oh);
                if widgets::number(ui, &mut ph, 1..=10000, " %").changed() {
                    self.set_height(from_percent(ph, oh));
                }
                ui.end_row();
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{Document, Sprite};

    #[test]
    fn sprite_size_keeps_the_ratio_when_locked() {
        let d = Document::with_sprite("a", Sprite::new(40, 20, ColorMode::Rgba));
        let mut s = SpriteSizeDialog::new(&d.artboards[0]);
        s.set_width(80);
        assert_eq!((s.width, s.height), (80, 40));
        s.lock_ratio = false;
        s.set_height(10);
        assert_eq!((s.width, s.height), (80, 10));
    }
}
