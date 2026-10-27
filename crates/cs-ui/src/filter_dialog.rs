//! Los diálogos de los filtros de Edit (Replace Color…, Adjustments y FX), como en Aseprite: con
//! la vista previa en el lienzo mientras se cambian los valores. OK la deja (un paso de undo);
//! Cancel, Escape o la × la quitan.

use cs_core::Rgba;
use eframe::egui::{self, Color32, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::editor::Editor;
use crate::filters::{Curve, Filter, Kernel};
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
    /// En Color Curve, el punto que se arrastra.
    curve_drag: Option<usize>,
}

impl FilterDialog {
    pub fn new(filter: Filter) -> Self {
        Self {
            filter,
            tried: None,
            applied: false,
            editing_to: false,
            curve_drag: None,
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
            self.filter.title(),
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
            Filter::ColorCurve { curve, channels } => {
                ui.horizontal(|ui| {
                    ui.label("Channels");
                    widgets::checkbox(ui, &mut channels.r, "R");
                    widgets::checkbox(ui, &mut channels.g, "G");
                    widgets::checkbox(ui, &mut channels.b, "B");
                    widgets::checkbox(ui, &mut channels.a, "A");
                });
                curve_editor(ui, skin, curve, &mut self.curve_drag);
                ui.label("Click to add a point, drag to move it, right-click to remove it.");
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

/// El editor de la curva: la entrada a lo ancho y la salida hacia arriba, de 0 a 255.
fn curve_editor(ui: &mut Ui, skin: Option<&Skin>, curve: &mut Curve, drag: &mut Option<usize>) {
    let side = ui.available_width().min(WIDTH);
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::click_and_drag());
    let to_screen = |(x, y): (u8, u8)| {
        egui::pos2(
            rect.left() + x as f32 / 255.0 * (rect.width() - 1.0),
            rect.bottom() - 1.0 - y as f32 / 255.0 * (rect.height() - 1.0),
        )
    };
    let to_curve = |p: egui::Pos2| {
        let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (
            f((p.x - rect.left()) / (rect.width() - 1.0)),
            f((rect.bottom() - 1.0 - p.y) / (rect.height() - 1.0)),
        )
    };
    let near = |curve: &Curve, p: egui::Pos2| {
        curve
            .points()
            .iter()
            .position(|&q| to_screen(q).distance(p) <= 5.0)
    };
    if response.drag_started()
        && let Some(p) = response.interact_pointer_pos()
    {
        *drag = near(curve, p).or_else(|| {
            let (x, y) = to_curve(p);
            curve.insert(x, y)
        });
    }
    if response.dragged()
        && let (Some(i), Some(p)) = (*drag, response.interact_pointer_pos())
    {
        let (x, y) = to_curve(p);
        curve.move_point(i, x, y);
    }
    if response.drag_stopped() {
        *drag = None;
    }
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
        && near(curve, p).is_none()
    {
        let (x, y) = to_curve(p);
        curve.insert(x, y);
    }
    if response.secondary_clicked()
        && let Some(i) = response.interact_pointer_pos().and_then(|p| near(curve, p))
    {
        curve.remove(i);
    }
    let (back, line, ink) = match skin {
        Some(s) => (Color32::WHITE, s.colors().ruler_tick, s.colors().outline),
        None => (
            ui.visuals().extreme_bg_color,
            ui.visuals().weak_text_color(),
            ui.visuals().text_color(),
        ),
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, back);
    // Las cuartas partes y la diagonal.
    for i in 1..4 {
        let t = i as f32 / 4.0;
        let x = (rect.left() + t * rect.width()).round() + 0.5;
        let y = (rect.top() + t * rect.height()).round() + 0.5;
        painter.vline(x, rect.y_range(), Stroke::new(1.0, line));
        painter.hline(rect.x_range(), y, Stroke::new(1.0, line));
    }
    // La curva en pixel crudo: un pixel por columna y, si salta, la columna llena hasta el de
    // la anterior (sin suavizado).
    let lut = curve.lut();
    let (w, h) = (rect.width() as i32, rect.height() as i32);
    let row_of = |y: u8| (h - 1) - (y as f32 / 255.0 * (h - 1) as f32).round() as i32;
    let col_of = |x: u8| (x as f32 / 255.0 * (w - 1) as f32).round() as i32;
    let row_at = |col: i32| row_of(lut[(col as f32 / (w - 1) as f32 * 255.0).round() as usize]);
    let mut mesh = egui::Mesh::default();
    let mut pixel = |col: i32, row: i32, size: f32| {
        let min = rect.min + vec2(col as f32, row as f32);
        mesh.add_colored_rect(egui::Rect::from_min_size(min, vec2(size, size)), ink);
    };
    let mut prev = row_at(0);
    for col in 0..w {
        let row = row_at(col);
        let (from, to) = if col == 0 || row == prev {
            (row, row)
        } else if row < prev {
            (row, prev - 1)
        } else {
            (prev + 1, row)
        };
        for r in from..=to {
            pixel(col, r, 1.0);
        }
        prev = row;
    }
    // Los puntos: cuadros de 5×5 sobre la curva.
    for &(x, y) in curve.points() {
        pixel(col_of(x) - 2, row_of(y) - 2, 5.0);
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, ink), StrokeKind::Inside);
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
        crate::pixel_ui::control_border(ui.ctx())
    };
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
    response
}

/// El selector de color de la app (con Pixel UI) o el de egui.
pub(crate) fn color_picker(ui: &mut Ui, skin: Option<&Skin>, color: &mut Rgba) {
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
