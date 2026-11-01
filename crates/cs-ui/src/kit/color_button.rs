//! El botón de color (paso 5 del UI kit, mockup de componentes, frame 9, x 472, y 423): la muestra
//! de un color en el marco `swatch.frame` (16×17). El marco es transparente adentro y trae, sobre
//! el color, el brillo de arriba a la derecha (blanco y blanco al 34%) y la fila de sombra (negro
//! al 38%) que en el mockup son píxeles del amarillo; así sirven para cualquier color.
//! `.hover` (un velo blanco), `.down` (uno negro) y `.disabled` (el borde (136,110,106), el color
//! lavado y sin brillo) no están en el diseño.
//!
//! Un clic abre el selector de color de la app (`color_panel::picker`) en el panel de los menús,
//! debajo del botón. Con alfa, detrás del color va el damero de los colores primario y secundario.

use eframe::egui::{self, Color32, Id, Rect, Response, Sense, Shape, Ui, Vec2, pos2, vec2};

use cs_core::Rgba;

use super::button::ButtonState;
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};

pub const PIECE: &str = "swatch.frame";
/// Sin la pieza, el tamaño del mockup.
const SIZE: Vec2 = vec2(16.0, 17.0);
/// El color va adentro del borde (1 px) y hasta la fila de sombra incluida: abajo quedan el
/// borde y la línea clara.
const BOTTOM_ROWS: f32 = 2.0;
/// Abierto (mockup de componentes, frame 9, x 342, y 614): el botón queda en una pestaña
/// (`picker.tab`, 20×20: la muestra va 2 px adentro, sin su línea clara) unida al panel
/// (`picker.panel`, borde de una línea, desde la fila 18 de la pestaña). El selector, 7 px adentro
/// a los lados y 3 filas arriba y abajo (abajo: 1 de relleno, el bisel y el borde).
pub const TAB: &str = "picker.tab";
pub const PANEL: &str = "picker.panel";
const TAB_SIZE: Vec2 = vec2(20.0, 20.0);
/// De la esquina de la pestaña a la del botón.
const TAB_INSET: f32 = 2.0;
/// El panel empieza en la fila 18 de la pestaña.
const PANEL_TOP: f32 = 18.0;
const PANEL_PAD_X: f32 = 7.0;
const PANEL_PAD_TOP: f32 = 3.0;
const PANEL_PAD_BOTTOM: f32 = 3.0;
/// El ancho del selector que se abre (el del mockup).
pub const PICKER_WIDTH: f32 = 156.0;

/// Desarrollo: con este dato en el contexto, el próximo botón de color se abre (para capturas).
pub(crate) const DEMO_PICKER: &str = "kit_demo_picker";

fn piece_state(state: ButtonState) -> WidgetState {
    match state {
        ButtonState::Normal => WidgetState::Normal,
        ButtonState::Hover => WidgetState::Hover,
        ButtonState::Pressed => WidgetState::Down,
        ButtonState::Disabled => WidgetState::Disabled,
    }
}

/// El tamaño del botón: el de la pieza.
pub fn size(skin: &Skin) -> Vec2 {
    skin.piece(PIECE, WidgetState::Normal)
        .map_or(SIZE, |p| vec2(p.size[0] as f32, p.size[1] as f32))
}

/// Un botón de color en `rect`, en `state`: el damero (si el color tiene alfa), el color y el
/// marco encima.
pub fn swatch_shapes(
    skin: &Skin,
    rect: Rect,
    color: Rgba,
    state: ButtonState,
    ppp: f32,
) -> Vec<Shape> {
    let inner = Rect::from_min_max(
        rect.min + vec2(1.0, 1.0),
        pos2(rect.right() - 1.0, rect.bottom() - BOTTOM_ROWS),
    );
    let mut shapes = Vec::new();
    if color.a < 255 {
        shapes.extend(checker(inner));
    }
    let fill = Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    shapes.push(Shape::rect_filled(inner, 0.0, fill));
    match skin.piece(PIECE, piece_state(state)) {
        Some(piece) => shapes.extend(pixel_ui::piece_shapes(piece, rect, ppp)),
        None => shapes.push(Shape::rect_stroke(
            rect.with_max_y(rect.bottom() - 1.0),
            0.0,
            egui::Stroke::new(1.0, skin.colors().outline),
            egui::StrokeKind::Inside,
        )),
    }
    shapes
}

/// El damero detrás de un color con alfa: celdas de 4×4 (197,199,221) y (230,231,240), las de
/// `fgbg.checker`, recortadas a `rect`.
fn checker(rect: Rect) -> Vec<Shape> {
    const CELL: f32 = 4.0;
    let dark = Color32::from_rgb(197, 199, 221);
    let light = Color32::from_rgb(230, 231, 240);
    let mut shapes = vec![Shape::rect_filled(rect, 0.0, light)];
    let (cols, rows) = (
        (rect.width() / CELL).ceil() as i32,
        (rect.height() / CELL).ceil() as i32,
    );
    for row in 0..rows {
        for col in (row % 2..cols).step_by(2) {
            let cell = Rect::from_min_size(
                rect.min + vec2(col as f32 * CELL, row as f32 * CELL),
                Vec2::splat(CELL),
            );
            shapes.push(Shape::rect_filled(cell.intersect(rect), 0.0, dark));
        }
    }
    shapes
}

/// Un botón con la muestra de `color` en el lugar que sigue de `ui`. Devuelve su `Response`
/// (quien lo llama decide qué hace el clic).
pub fn swatch_button(ui: &mut Ui, skin: &Skin, color: Rgba, label: &str) -> Response {
    let size = size(skin);
    let (slot, response) = ui.allocate_exact_size(size, Sense::click());
    let rect = Rect::from_min_size(slot.min.round(), size);
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ColorButton, enabled, label));
    if ui.is_rect_visible(rect) {
        let state = ButtonState::of(&response, enabled);
        let ppp = ui.ctx().pixels_per_point();
        let mut painter = ui.painter().clone();
        painter.set_opacity(1.0);
        painter.extend(swatch_shapes(skin, rect, color, state, ppp));
    }
    response
}

/// Un botón de color que, con un clic, abre el selector de la app debajo. Sin `alpha`, el color
/// queda opaco. El `Response` está `changed()` si cambió el color.
pub fn color_edit(
    ui: &mut Ui,
    skin: &Skin,
    color: &mut Rgba,
    alpha: bool,
    label: &str,
) -> Response {
    let mut response = swatch_button(ui, skin, *color, label);
    // Desarrollo: el primero que se dibuja, abierto (para capturas).
    let demo = Id::new(DEMO_PICKER);
    let first = ui.ctx().data(|d| d.get_temp::<Id>(demo));
    if first == Some(Id::NULL) {
        ui.ctx().data_mut(|d| d.insert_temp(demo, response.id));
    }
    if first.is_some_and(|id| id == Id::NULL || id == response.id) {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&response));
    }
    let picked = open_picker(skin, &response, *color, |ui| {
        let next = crate::color_panel::picker(ui, skin, response.id.with("picker"), *color);
        if alpha { next } else { Rgba { a: 255, ..next } }
    });
    if let Some(next) = picked
        && next != *color
    {
        *color = next;
        response.mark_changed();
    }
    response
}

/// El selector abierto: la pestaña sobre el botón (con la muestra adentro) y el panel debajo con
/// lo que arma `content`. Se abre y se cierra con un clic en el botón (o en la pestaña); un clic
/// afuera lo cierra.
fn open_picker<R>(
    skin: &Skin,
    response: &Response,
    color: Rgba,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    // Donde se dibujó el botón (en píxeles enteros).
    let rect = Rect::from_min_size(response.rect.min.round(), response.rect.size());
    let corner = rect.min - Vec2::splat(TAB_INSET);
    let anchor = Rect::from_min_size(corner, TAB_SIZE);
    let popup_id = egui::Popup::default_response_id(response);
    let inner = egui::Popup::menu(response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .anchor(anchor)
        .align(egui::RectAlign {
            parent: egui::Align2::LEFT_TOP,
            child: egui::Align2::LEFT_TOP,
        })
        .align_alternatives(&[])
        .gap(0.0)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            let ppp = ui.ctx().pixels_per_point();
            let origin = ui.max_rect().min.round();
            let back = ui.painter().add(Shape::Noop);
            let inside = Rect::from_min_size(
                origin + vec2(PANEL_PAD_X, PANEL_TOP + PANEL_PAD_TOP),
                vec2(PICKER_WIDTH, f32::INFINITY),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inside)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.set_width(PICKER_WIDTH);
            let out = content(&mut child);
            let used = child.min_rect();
            let panel = Rect::from_min_size(
                origin + vec2(0.0, PANEL_TOP),
                vec2(
                    used.width().max(PICKER_WIDTH) + 2.0 * PANEL_PAD_X,
                    PANEL_PAD_TOP + used.height() + PANEL_PAD_BOTTOM,
                ),
            );
            ui.allocate_rect(
                panel.union(Rect::from_min_size(origin, TAB_SIZE)),
                Sense::hover(),
            );
            let mut shapes = match skin.piece(PANEL, WidgetState::Normal) {
                Some(p) => pixel_ui::piece_shapes(p, panel, ppp),
                None => {
                    let colors = skin.colors();
                    vec![
                        Shape::rect_filled(panel, 0.0, colors.panel),
                        Shape::rect_stroke(
                            panel,
                            0.0,
                            egui::Stroke::new(1.0, colors.outline),
                            egui::StrokeKind::Inside,
                        ),
                    ]
                }
            };
            // La pestaña con la muestra adentro (sin su línea clara: ahí sigue el relleno).
            let tab = Rect::from_min_size(origin, TAB_SIZE);
            if let Some(p) = skin.piece(TAB, WidgetState::Normal) {
                shapes.extend(pixel_ui::piece_shapes(p, tab, ppp));
            }
            let swatch = Rect::from_min_size(origin + Vec2::splat(TAB_INSET), rect.size());
            ui.painter().set(back, Shape::Vec(shapes));
            ui.painter()
                .with_clip_rect(swatch.with_max_y(swatch.bottom() - 1.0))
                .extend(swatch_shapes(skin, swatch, color, ButtonState::Normal, ppp));
            // Un clic en la muestra lo cierra.
            let again = ui.interact(swatch, response.id.with("tab"), Sense::click());
            if again.clicked() {
                egui::Popup::close_id(ui.ctx(), popup_id);
            }
            out
        });
    inner.map(|i| {
        response.ctx.move_to_top(i.response.layer_id);
        i.inner
    })
}

#[cfg(test)]
mod tests;
