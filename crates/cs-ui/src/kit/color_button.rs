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
/// El ancho del selector que se abre.
pub const PICKER_WIDTH: f32 = 160.0;

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
    let rect = response.rect;
    // Desarrollo: el primero que se dibuja, abierto (para capturas).
    let demo = Id::new(DEMO_PICKER);
    let first = ui.ctx().data(|d| d.get_temp::<Id>(demo));
    if first == Some(Id::NULL) {
        ui.ctx().data_mut(|d| d.insert_temp(demo, response.id));
    }
    if first.is_some_and(|id| id == Id::NULL || id == response.id) {
        egui::Popup::open_id(ui.ctx(), egui::Popup::default_response_id(&response));
    }
    let look = crate::menus::MenuLook::new(skin, rect.top());
    let picked = crate::menus::dropdown_panel(&look, &response, rect, |ui| {
        ui.set_width(PICKER_WIDTH);
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

#[cfg(test)]
mod tests;
