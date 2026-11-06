//! Los diálogos de los filtros de Edit (Replace Color…, Adjustments y FX), como en Aseprite: con
//! la vista previa en el lienzo mientras se cambian los valores (si Preview está prendido). OK la
//! deja (un paso de undo) y cierra; Apply la deja y sigue abierto; Cancel, Escape o la × quitan
//! lo que no se dejó.

use cs_core::Rgba;
use cs_raster::{OutlineMatrix, OutlinePlace, OutlinePreset};
use eframe::egui::{self, Color32, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::editor::{Editor, TiledMode};
use crate::filters::{Curve, Filter, FilterTarget, Kernel, OutlineFx};
use crate::kit::button::{self as kit_button, ButtonState};
use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

/// El ancho de lo de adentro (el del selector de color, cuando lo hay).
const WIDTH: f32 = 200.0;
const SWATCH: egui::Vec2 = vec2(24.0, 16.0);
/// Una celda de la matriz de Outline: un botón del kit cuadrado, con 2 px entre celdas.
const CELL: f32 = kit_button::HEIGHT;
const CELL_GAP: f32 = 2.0;
/// El cuadrito de adentro de una celda prendida.
const CELL_DOT: f32 = 5.0;
/// La columna de los nombres de las filas de Outline y el aire entre filas.
const LABEL_WIDTH: f32 = 52.0;
const ROW_GAP: f32 = 4.0;

/// Lo que se pidió en un cuadro del diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    None,
    Apply,
    Accept,
    Cancel,
}

pub struct FilterDialog {
    filter: Filter,
    /// A qué cels va (solo Outline lo elige; los demás, al actual).
    target: Option<FilterTarget>,
    /// Ver el resultado en el lienzo mientras se cambian los valores.
    preview: bool,
    /// Lo último que se aplicó en el lienzo (de vista previa o con Apply), y si hay un paso de
    /// vista previa que quitar.
    tried: Option<(Filter, Option<FilterTarget>)>,
    applied: bool,
    /// Los valores con los que se dejó algo la última vez (Apply u OK), para recordarlos.
    committed: Option<(Filter, Option<FilterTarget>)>,
    /// En Replace Color, el selector edita `to` (si no, `from`).
    editing_to: bool,
    /// En Color Curve, el punto que se arrastra.
    curve_drag: Option<usize>,
}

impl FilterDialog {
    pub fn new(filter: Filter) -> Self {
        Self {
            filter,
            target: matches!(filter, Filter::Outline(_)).then_some(FilterTarget::Selected),
            preview: true,
            tried: None,
            applied: false,
            committed: None,
            editing_to: false,
            curve_drag: None,
        }
    }

    /// Con `target` (solo cuenta en Outline).
    pub fn with_target(mut self, target: FilterTarget) -> Self {
        if self.target.is_some() {
            self.target = Some(target);
        }
        self
    }

    /// Los valores de Outline para recordar, si se dejó algo con ellos.
    pub fn committed_outline(&self) -> Option<(OutlineFx, FilterTarget)> {
        match self.committed {
            Some((Filter::Outline(fx), Some(target))) => Some((fx, target)),
            _ => None,
        }
    }

    fn current(&self) -> (Filter, Option<FilterTarget>) {
        (self.filter, self.target)
    }

    /// Quita la vista previa del lienzo, si había.
    fn unpreview(&mut self, editor: &mut Editor) {
        if self.applied {
            editor.undo();
            self.applied = false;
        }
        self.tried = None;
    }

    /// Aplica los valores de ahora (un paso, si cambió algo).
    fn run(&mut self, editor: &mut Editor) {
        self.applied = match self.target {
            Some(target) => editor.apply_filter_to(&self.filter, target),
            None => editor.apply_filter(&self.filter),
        };
        self.tried = Some(self.current());
    }

    /// Lo que pasa en el lienzo en un cuadro, según `action`.
    fn step(&mut self, editor: &mut Editor, action: Action) {
        // Lo que se ve ya es esto (de vista previa o recién aplicado).
        let fresh = self.tried == Some(self.current());
        match action {
            Action::Cancel => self.unpreview(editor),
            Action::Accept => {
                if !fresh {
                    self.unpreview(editor);
                    self.run(editor);
                }
                self.committed = Some(self.current());
            }
            Action::Apply => {
                // La vista previa de estos valores queda; si no la hay (sin Preview, o Apply otra
                // vez sin cambios), se aplica ahora.
                if !(fresh && self.applied) {
                    if !fresh {
                        self.unpreview(editor);
                    }
                    self.run(editor);
                }
                // Queda como paso y no se vuelve a previsualizar hasta que algo cambie.
                self.applied = false;
                self.tried = Some(self.current());
                self.committed = Some(self.current());
            }
            Action::None if self.preview && !fresh => {
                self.unpreview(editor);
                self.run(editor);
            }
            // Se apagó Preview: el lienzo vuelve a como estaba.
            Action::None if !self.preview && self.applied => self.unpreview(editor),
            Action::None => {}
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        skin: Option<&Skin>,
    ) -> DialogResult {
        let mut action = Action::None;
        let tiled = editor.tiled;
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
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    widgets::checkbox(ui, &mut self.preview, "Preview");
                    if let Filter::Outline(fx) = &mut self.filter {
                        ui.add_space(8.0);
                        tiled_checkbox(ui, fx, tiled);
                    }
                });
                ui.add_space(6.0);
                match pixel_ui::dialog_buttons(ui, skin, &["OK", "Apply", "Cancel"]) {
                    Some(0) => action = Action::Accept,
                    Some(1) => action = Action::Apply,
                    Some(_) => action = Action::Cancel,
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
            action = Action::Cancel;
        } else if enter {
            action = Action::Accept;
        }
        self.step(editor, action);
        match action {
            Action::Accept => DialogResult::Accept,
            Action::Cancel => DialogResult::Cancel,
            _ => DialogResult::Open,
        }
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
            Filter::Outline(fx) => {
                let target = self.target.get_or_insert_default();
                outline_contents(ui, skin, fx, target);
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

/// Los valores de Outline, como la ventana de Aseprite: los colores, la forma (presets y la
/// matriz), de qué lado, los canales y a qué cels.
fn outline_contents(
    ui: &mut Ui,
    skin: Option<&Skin>,
    fx: &mut OutlineFx,
    target: &mut FilterTarget,
) {
    labeled(ui, "Color", |ui| {
        widgets::color_edit(ui, &mut fx.color, true, "Outline color");
        ui.add_space(12.0);
        ui.label("Background");
        // Con alfa 0, Mask: solo lo transparente es vacío.
        widgets::color_edit(ui, &mut fx.background, true, "Background color");
    });
    labeled(ui, "Shape", |ui| {
        let preset = fx.shape.matrix.preset();
        let labels = OutlinePreset::ALL.map(|p| p.label());
        let active = |i: usize| preset == Some(OutlinePreset::ALL[i]);
        if let Some(i) = choice_group(ui, skin, &labels, &active) {
            fx.shape.matrix = OutlinePreset::ALL[i].matrix();
        }
    });
    labeled(ui, "Matrix", |ui| {
        matrix_editor(ui, skin, &mut fx.shape.matrix, fx.color);
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = ROW_GAP;
            ui.label("Place");
            let place = &mut fx.shape.place;
            widgets::radio_value(ui, place, OutlinePlace::Outside, "Outside");
            widgets::radio_value(ui, place, OutlinePlace::Inside, "Inside");
        });
    });
    labeled(ui, "Channels", |ui| {
        let c = &mut fx.channels;
        widgets::checkbox(ui, &mut c.r, "R");
        widgets::checkbox(ui, &mut c.g, "G");
        widgets::checkbox(ui, &mut c.b, "B");
        widgets::checkbox(ui, &mut c.a, "A");
    });
    labeled(ui, "Target", |ui| {
        widgets::combo(ui, "outline_target", target.label(), |ui| {
            for t in FilterTarget::ALL {
                widgets::choice(ui, target, t, t.label());
            }
        });
    });
}

/// Una fila con su nombre en una columna de [`LABEL_WIDTH`], a la altura del primer control (de
/// [`kit_button::HEIGHT`]); lo de `add`, de arriba hacia abajo desde el mismo borde.
fn labeled(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        let size = vec2(LABEL_WIDTH, kit_button::HEIGHT);
        let layout = egui::Layout::left_to_right(egui::Align::Center);
        ui.allocate_ui_with_layout(size, layout, |ui| {
            ui.set_min_size(size);
            ui.label(label);
        });
        add(ui);
    });
    ui.add_space(ROW_GAP);
}

/// Tiled: el outline envuelve en los bordes. Al prenderlo, en los ejes de View → Tiled Mode (o
/// en los dos si está apagado).
fn tiled_checkbox(ui: &mut Ui, fx: &mut OutlineFx, tiled: TiledMode) {
    let shape = &mut fx.shape;
    let mut on = shape.wrap_x || shape.wrap_y;
    if widgets::checkbox(ui, &mut on, "Tiled").changed() {
        set_tiled(fx, on, tiled);
    }
}

/// Prende o apaga Tiled en `fx` (prendido, en los ejes de `tiled`; si está apagado, en los dos).
pub(crate) fn set_tiled(fx: &mut OutlineFx, on: bool, tiled: TiledMode) {
    let (x, y) = match tiled {
        TiledMode::None => (true, true),
        mode => (mode.x(), mode.y()),
    };
    fx.shape.wrap_x = on && x;
    fx.shape.wrap_y = on && y;
}

/// Un grupo de opciones de texto (con Pixel UI, el del kit); `active(i)`: si la `i` está elegida
/// (puede no haber ninguna). Devuelve la que se tocó.
fn choice_group(
    ui: &mut Ui,
    skin: Option<&Skin>,
    labels: &[&str],
    active: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    match skin {
        Some(skin) => {
            let items: Vec<crate::kit::GroupItem> = labels
                .iter()
                .map(|l| crate::kit::GroupItem::text(l))
                .collect();
            crate::kit::button_group(ui, skin, &items, active)
        }
        None => {
            ui.horizontal(|ui| {
                let mut clicked = None;
                for (i, label) in labels.iter().enumerate() {
                    if ui.selectable_label(active(i), *label).clicked() {
                        clicked = Some(i);
                    }
                }
                clicked
            })
            .inner
        }
    }
}

/// La matriz de 3×3 de Outline: cada vecino es un botón del kit que se prende y se apaga
/// (prendido, presionado y con un cuadrito de `ink`, el color del borde); el del centro es el
/// píxel: deshabilitado, con un cuadrito del color de acento.
fn matrix_editor(ui: &mut Ui, skin: Option<&Skin>, matrix: &mut OutlineMatrix, ink: Rgba) {
    let step = CELL + CELL_GAP;
    let side = 3.0 * CELL + 2.0 * CELL_GAP;
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let origin = rect.min.round();
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = Vec::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let min = origin + vec2((dx + 1) as f32 * step, (dy + 1) as f32 * step);
            let cell = egui::Rect::from_min_size(min, vec2(CELL, CELL));
            let center = (dx, dy) == (0, 0);
            let id = ui.id().with(("outline_matrix", dx, dy));
            let sense = if center {
                Sense::hover()
            } else {
                Sense::click()
            };
            let response = ui.interact(cell, id, sense);
            if response.clicked() {
                *matrix = matrix.toggled(dx, dy);
            }
            let on = matrix.get(dx, dy);
            response.widget_info(|| {
                let label = format!("Neighbor {dx}, {dy}");
                egui::WidgetInfo::selected(egui::WidgetType::Button, !center, on, label)
            });
            let state = match ButtonState::of(&response, !center) {
                ButtonState::Disabled => ButtonState::Disabled,
                _ if on => ButtonState::Pressed,
                state => state,
            };
            // El cuadrito, centrado en la cara (baja con ella al presionar).
            let drop = if state == ButtonState::Pressed {
                kit_button::PRESS_DROP
            } else {
                0.0
            };
            let face = kit_button::face_rows(CELL);
            let dot_min = min
                + vec2(
                    ((CELL - CELL_DOT) / 2.0).floor(),
                    kit_button::FACE_TOP + ((face - CELL_DOT) / 2.0).floor() + drop,
                );
            let dot = egui::Rect::from_min_size(dot_min.round(), vec2(CELL_DOT, CELL_DOT));
            match skin {
                Some(skin) => {
                    shapes.extend(kit_button::frame_shapes(skin, cell, state, ppp));
                    if on {
                        let ink = crate::panels::to_color32(Rgba { a: 255, ..ink });
                        shapes.push(egui::Shape::rect_filled(dot, 0.0, ink));
                    } else if center {
                        // El píxel mismo, en el color de acento.
                        let accent = skin.colors().accent;
                        shapes.push(egui::Shape::rect_filled(dot, 0.0, accent));
                    }
                }
                None => {
                    let visuals = ui.visuals();
                    let fill = if on || center {
                        visuals.selection.bg_fill
                    } else {
                        visuals.extreme_bg_color
                    };
                    shapes.push(egui::Shape::rect_filled(cell, 0.0, fill));
                    shapes.push(egui::Shape::rect_stroke(
                        cell,
                        0.0,
                        Stroke::new(1.0, visuals.weak_text_color()),
                        StrokeKind::Inside,
                    ));
                }
            }
        }
    }
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    painter.extend(shapes);
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

    /// 5×5 con un punto rojo en el centro, y el diálogo de Outline (Circle, afuera, negro).
    fn outline_setup() -> (Editor, FilterDialog) {
        let mut e = Editor::with_sprite(Sprite::new(5, 5, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 2, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        });
        let d = FilterDialog::new(Filter::Outline(OutlineFx::new(Rgba::rgb(0, 0, 0))));
        (e, d)
    }

    fn painted(e: &Editor, x: i32, y: i32) -> bool {
        e.resolve(e.sprite().pixel(LayerId(1), 0, x, y)).a > 0
    }

    #[test]
    fn apply_keeps_a_step_and_does_not_preview_it_again() {
        let (mut e, mut d) = outline_setup();
        let steps = e.steps_done();
        let ctx = egui::Context::default();
        frame(&ctx, &mut d, &mut e, None);
        assert_eq!(e.steps_done(), steps + 1, "previewed");
        d.step(&mut e, Action::Apply);
        frame(&ctx, &mut d, &mut e, None);
        assert_eq!(e.steps_done(), steps + 1, "the preview became the step");
        assert!(painted(&e, 2, 1) && !painted(&e, 2, 0));
        // Apply otra vez sin cambios: otro borde alrededor.
        d.step(&mut e, Action::Apply);
        assert_eq!(e.steps_done(), steps + 2);
        assert!(painted(&e, 2, 0));
        // Cancel no quita lo que se dejó; OK sin cambios tampoco agrega.
        d.step(&mut e, Action::Cancel);
        assert_eq!(e.steps_done(), steps + 2);
        assert!(d.committed_outline().is_some(), "remembered");
    }

    #[test]
    fn a_change_after_apply_previews_on_top_and_ok_keeps_it() {
        let (mut e, mut d) = outline_setup();
        let steps = e.steps_done();
        d.step(&mut e, Action::Apply);
        assert_eq!(e.steps_done(), steps + 1);
        let Filter::Outline(fx) = &mut d.filter else {
            unreachable!()
        };
        fx.color = Rgba::rgb(0, 255, 0);
        d.step(&mut e, Action::None);
        assert_eq!(e.steps_done(), steps + 2, "the preview of the change");
        d.step(&mut e, Action::Accept);
        assert_eq!(e.steps_done(), steps + 2, "OK keeps the preview");
    }

    #[test]
    fn without_preview_the_canvas_waits_for_ok() {
        let (mut e, mut d) = outline_setup();
        let steps = e.steps_done();
        d.step(&mut e, Action::None);
        assert_eq!(e.steps_done(), steps + 1);
        // Apagar Preview quita la vista previa.
        d.preview = false;
        d.step(&mut e, Action::None);
        assert_eq!(e.steps_done(), steps);
        assert!(!painted(&e, 2, 1));
        assert!(d.committed_outline().is_none());
        d.step(&mut e, Action::Accept);
        assert_eq!(e.steps_done(), steps + 1);
        assert!(painted(&e, 2, 1));
    }

    #[test]
    fn tiled_follows_the_tiled_mode_axes() {
        let mut fx = OutlineFx::new(Rgba::rgb(0, 0, 0));
        set_tiled(&mut fx, true, TiledMode::None);
        assert!(fx.shape.wrap_x && fx.shape.wrap_y);
        set_tiled(&mut fx, true, TiledMode::X);
        assert!(fx.shape.wrap_x && !fx.shape.wrap_y);
        set_tiled(&mut fx, false, TiledMode::Both);
        assert!(!fx.shape.wrap_x && !fx.shape.wrap_y);
    }
}
