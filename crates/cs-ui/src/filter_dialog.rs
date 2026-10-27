//! Los diálogos de los filtros de Edit (Replace Color…, Adjustments y FX), como en Aseprite: con
//! la vista previa en el lienzo mientras se cambian los valores. OK la deja (un paso de undo);
//! Cancel, Escape o la × la quitan.

use cs_core::Rgba;
use eframe::egui::{self, Color32, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::editor::Editor;
use crate::filters::{Filter, Kernel};
use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

/// El ancho de lo de adentro (el del selector de color, cuando lo hay).
const WIDTH: f32 = 200.0;
const SWATCH: egui::Vec2 = vec2(24.0, 16.0);

pub struct FilterDialog {
    filter: Filter,
    /// Lo último que se probó en el lienzo y si cambió algo (entonces hay un paso que quitar).
    tried: Option<Filter>,
    applied: bool,
    /// En Replace Color, el selector edita `to` (si no, `from`).
    editing_to: bool,
}

impl FilterDialog {
    pub fn new(filter: Filter) -> Self {
        Self {
            filter,
            tried: None,
            applied: false,
            editing_to: false,
        }
    }

    fn title(&self) -> &'static str {
        match self.filter {
            Filter::ReplaceColor { .. } => "Replace Color",
            Filter::BrightnessContrast { .. } => "Brightness/Contrast",
            Filter::HueSaturation { .. } => "Hue/Saturation",
            Filter::Outline { .. } => "Outline",
            Filter::Despeckle { .. } => "Despeckle",
            Filter::Convolution(_) => "Convolution Matrix",
        }
    }

    /// Quita la vista previa del lienzo, si había.
    fn unpreview(&mut self, editor: &mut Editor) {
        if self.applied {
            editor.undo();
            self.applied = false;
        }
        self.tried = None;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        skin: Option<&Skin>,
    ) -> DialogResult {
        let mut result = DialogResult::Open;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "filter_dialog",
            self.title(),
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                ui.set_width(WIDTH);
                self.contents(ui, skin);
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
        let result = if closed || escape {
            DialogResult::Cancel
        } else if enter {
            DialogResult::Accept
        } else {
            result
        };
        match result {
            DialogResult::Cancel => self.unpreview(editor),
            // La vista previa con los valores de ahora (OK la deja como está).
            _ if self.tried != Some(self.filter) => {
                self.unpreview(editor);
                self.applied = editor.apply_filter(&self.filter);
                self.tried = Some(self.filter);
            }
            _ => {}
        }
        result
    }

    fn contents(&mut self, ui: &mut Ui, skin: Option<&Skin>) {
        match &mut self.filter {
            Filter::ReplaceColor {
                from,
                to,
                tolerance,
            } => {
                ui.horizontal(|ui| {
                    ui.label("From");
                    if swatch(ui, *from, !self.editing_to).clicked() {
                        self.editing_to = false;
                    }
                    ui.add_space(8.0);
                    ui.label("To");
                    if swatch(ui, *to, self.editing_to).clicked() {
                        self.editing_to = true;
                    }
                });
                let editing = if self.editing_to { to } else { from };
                color_picker(ui, skin, editing);
                ui.horizontal(|ui| {
                    ui.label("Tolerance");
                    widgets::number(ui, tolerance, 0..=255, "");
                });
            }
            Filter::BrightnessContrast {
                brightness,
                contrast,
            } => {
                rows(
                    ui,
                    [
                        ("Brightness", brightness, -100..=100),
                        ("Contrast", contrast, -100..=100),
                    ],
                );
            }
            Filter::HueSaturation {
                hue,
                saturation,
                lightness,
            } => {
                rows(
                    ui,
                    [
                        ("Hue", hue, -180..=180),
                        ("Saturation", saturation, -100..=100),
                        ("Lightness", lightness, -100..=100),
                    ],
                );
            }
            Filter::Outline { color, square } => {
                ui.horizontal(|ui| {
                    ui.label("Color");
                    swatch(ui, *color, true);
                });
                color_picker(ui, skin, color);
                widgets::checkbox(ui, square, "Square (corners too)");
            }
            Filter::Despeckle { size } => {
                ui.horizontal(|ui| {
                    ui.label("Size");
                    widgets::number(ui, size, 3..=9, " px");
                });
                // Siempre impar: el píxel en el centro.
                *size |= 1;
            }
            Filter::Convolution(kernel) => {
                ui.horizontal(|ui| {
                    ui.label("Matrix");
                    widgets::combo(ui, "convolution_kernel", kernel.label(), |ui| {
                        for k in Kernel::ALL {
                            widgets::choice(ui, kernel, k, k.label());
                        }
                    });
                });
            }
        }
    }
}

/// Valores con su nombre a la izquierda, los campos en columna.
fn rows<const N: usize>(ui: &mut Ui, values: [(&str, &mut i32, std::ops::RangeInclusive<i32>); N]) {
    egui::Grid::new("filter_values")
        .spacing(vec2(8.0, 4.0))
        .show(ui, |ui| {
            for (label, value, range) in values {
                ui.label(label);
                widgets::number(ui, value, range, "");
                ui.end_row();
            }
        });
}

/// Un cuadro con `color` (con un borde rosa si es el que edita el selector); se toca para
/// elegirlo.
fn swatch(ui: &mut Ui, color: Rgba, editing: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(SWATCH, Sense::click());
    let painter = ui.painter();
    if color.a < 255 {
        crate::panels::checker(painter, rect);
    }
    painter.rect_filled(rect, 0.0, crate::panels::to_color32(color));
    let border = if editing {
        crate::theme::ACCENT
    } else {
        Color32::from_rgb(89, 77, 77)
    };
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    response
}

/// El selector de color de la app (con Pixel UI) o el de egui.
fn color_picker(ui: &mut Ui, skin: Option<&Skin>, color: &mut Rgba) {
    match skin {
        Some(skin) => {
            let id = ui.id().with("filter_picker");
            *color = crate::color_panel::picker(ui, skin, id, *color);
        }
        None => {
            let mut c = crate::panels::to_color32(*color);
            egui::color_picker::color_picker_color32(
                ui,
                &mut c,
                egui::color_picker::Alpha::OnlyBlend,
            );
            let [r, g, b, a] = c.to_srgba_unmultiplied();
            *color = Rgba::new(r, g, b, a);
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Sprite};
    use eframe::egui::{Event, Key, RawInput};

    use super::*;

    fn frame(
        ctx: &egui::Context,
        d: &mut FilterDialog,
        e: &mut Editor,
        key: Option<Key>,
    ) -> DialogResult {
        let events = key
            .map(|key| {
                vec![Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                }]
            })
            .unwrap_or_default();
        let mut result = DialogResult::Open;
        let mut output = ctx.run_ui(
            RawInput {
                events,
                ..Default::default()
            },
            |ui| result = d.show(ui.ctx(), e, None),
        );
        output.textures_delta.clear();
        result
    }

    #[test]
    fn the_preview_is_on_the_canvas_and_cancel_takes_it_back() {
        let red = Rgba::rgb(255, 0, 0);
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Rgba(red))],
        });
        let steps = e.steps_done();
        let ctx = egui::Context::default();
        let filter = Filter::HueSaturation {
            hue: 120,
            saturation: 0,
            lightness: 0,
        };
        let mut d = FilterDialog::new(filter);
        frame(&ctx, &mut d, &mut e, None);
        let green = Rgba::rgb(0, 255, 0);
        assert_eq!(
            e.resolve(e.sprite().pixel(LayerId(1), 0, 1, 1)),
            green,
            "previewed"
        );
        assert!(matches!(
            frame(&ctx, &mut d, &mut e, Some(Key::Escape)),
            DialogResult::Cancel
        ));
        assert_eq!(
            e.resolve(e.sprite().pixel(LayerId(1), 0, 1, 1)),
            red,
            "taken back"
        );
        assert_eq!(e.steps_done(), steps);
        // OK la deja, en un paso.
        let mut d = FilterDialog::new(filter);
        frame(&ctx, &mut d, &mut e, None);
        assert!(matches!(
            frame(&ctx, &mut d, &mut e, Some(Key::Enter)),
            DialogResult::Accept
        ));
        assert_eq!(e.resolve(e.sprite().pixel(LayerId(1), 0, 1, 1)), green);
        assert_eq!(e.steps_done(), steps + 1);
    }
}
