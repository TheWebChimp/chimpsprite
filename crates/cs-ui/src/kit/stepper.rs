//! El stepper (mockup de componentes, frame 9, x 301, y 256 y x 380, y 276): − [número] + en 17
//! filas, como un campo.
//!
//! - Los botones: `stepper.left` y `stepper.right` (15×17, con la columna (136,110,106) que los
//!   separa del campo), con `.hover`, `.down` y `.disabled` (no están en el diseño: el relleno
//!   blanco; el bisel invertido; el borde tenue). Los íconos `icon.minus` e `icon.plus` (7×7, de
//!   una tinta) van centrados en la cara, con la sombra blanca 1 px abajo como en el mockup (salvo
//!   con el puntero encima); presionado bajan 1 px. En el mínimo (o el máximo) su botón se
//!   deshabilita.
//! - El campo: `stepper.field` (1×17, se estira; `.disabled`), con el número centrado en
//!   `color.text` y las mayúsculas en la fila 5. Se arrastra o, con un clic, se escribe (el
//!   `DragValue` del campo numérico del kit).
//! - Con unidad ("ms"), su caja `stepper.unit` (1×17, se estira) pegada al botón derecho: 3 + tinta
//!   + 3 (por lo menos 11), el texto centrado en `color.field_text`.
//! - Con etiqueta de eje (era el transform del panel 3D, como el boceto de la pestaña de un
//!   objeto; hoy la transform va en la barrita del objeto, con los campos de la barra):
//!   una cajita de [`TAG_W`] pegada a la izquierda del botón −, la misma `stepper.unit` (del alto
//!   del stepper), redondeada del lado de afuera (a la izquierda) y recta contra el botón (que
//!   también queda recto de ese lado), con la letra (`X`, `Y`, `Z`) centrada en
//!   `color.field_text`.

use std::ops::RangeInclusive;

use eframe::egui::{self, Align, Color32, Rect, Response, Sense, Shape, Ui, emath, pos2, vec2};

use super::button::{self, ButtonState};
use super::input::{self, NumberLook};
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

pub const HEIGHT: f32 = input::HEIGHT;
pub const LEFT: &str = "stepper.left";
pub const RIGHT: &str = "stepper.right";
pub const FIELD: &str = "stepper.field";
pub const UNIT: &str = "stepper.unit";
pub const MINUS: &str = "icon.minus";
pub const PLUS: &str = "icon.plus";
/// Cada botón, con la columna que lo separa del campo.
pub const BUTTON_W: f32 = 15.0;
/// La etiqueta de eje: 13 de ancho, como en el boceto.
pub const TAG_W: f32 = 13.0;

fn piece_state(state: ButtonState) -> WidgetState {
    match state {
        ButtonState::Normal => WidgetState::Normal,
        ButtonState::Hover => WidgetState::Hover,
        ButtonState::Pressed => WidgetState::Down,
        ButtonState::Disabled => WidgetState::Disabled,
    }
}

fn galley(ui: &Ui, text: &str, color: Color32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        color,
    )
}

/// El ancho de la caja de la unidad: 3 + tinta + 3, por lo menos 11.
pub fn unit_width(ui: &Ui, unit: &str) -> f32 {
    input::box_width(ui, unit)
}

/// Un botón del stepper en `rect` (15×17) en `state`: la pieza y el ícono.
pub fn button_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    piece: &str,
    icon: &str,
    state: ButtonState,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = match skin.piece(piece, piece_state(state)) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => button::frame_shapes(skin, rect, state, ppp),
    };
    if let Some(icon) = skin.piece(icon, WidgetState::Normal) {
        let size = vec2(icon.size[0] as f32, icon.size[1] as f32);
        let drop = if state == ButtonState::Pressed {
            1.0
        } else {
            0.0
        };
        let at = pos2(
            rect.left() + ((rect.width() - size.x) / 2.0).floor(),
            rect.top()
                + button::FACE_TOP
                + ((button::face_rows(rect.height()) - size.y) / 2.0).floor()
                + drop,
        );
        shapes.extend(button::mono_icon_shapes(
            skin,
            icon,
            at,
            state,
            Color32::WHITE,
        ));
    }
    shapes
}

/// El campo del medio en `rect` (entre los botones) con la caja de la unidad, sin el número.
pub fn field_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    unit: Option<&str>,
    enabled: bool,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = skin.colors();
    let state = if enabled {
        WidgetState::Normal
    } else {
        WidgetState::Disabled
    };
    let mut shapes = match skin.piece(FIELD, state) {
        Some(p) => pixel_ui::piece_shapes(p, rect, ppp),
        None => vec![Shape::rect_filled(rect, 0.0, Color32::WHITE)],
    };
    if let Some(unit) = unit {
        let w = unit_width(ui, unit);
        let rect = Rect::from_min_max(pos2(rect.right() - w, rect.top()), rect.max);
        if let Some(p) = skin.piece(UNIT, WidgetState::Normal) {
            shapes.extend(pixel_ui::piece_shapes(p, rect, ppp));
        }
        let color = if enabled {
            colors.field_text
        } else {
            colors.button_text_disabled
        };
        let shown = galley(ui, unit, color);
        let ink = crate::menus::ink_width(&shown);
        let left = rect.left() + ((rect.width() - ink) / 2.0).floor();
        let at = tab_bar::text_pos(&shown, left, rect.top() + input::CAPS_TOP, ppp);
        shapes.push(Shape::galley(at, shown, color));
    }
    shapes
}

/// Dónde va el número: el campo sin la caja de la unidad.
fn number_area(ui: &Ui, field: Rect, unit: Option<&str>) -> Rect {
    match unit {
        Some(unit) => field.with_max_x(field.right() - unit_width(ui, unit)),
        None => field,
    }
}

/// El texto de un número centrado en `area` (para el catálogo, sin interacción).
pub fn number_shapes(ui: &Ui, skin: &Skin, area: Rect, text: &str, enabled: bool) -> Vec<Shape> {
    let colors = skin.colors();
    let color = if enabled {
        colors.text
    } else {
        colors.field_text_disabled
    };
    let shown = galley(ui, text, color);
    let ink = crate::menus::ink_width(&shown);
    let left = area.left() + ((area.width() - ink) / 2.0).floor();
    let at = tab_bar::text_pos(
        &shown,
        left,
        area.top() + input::CAPS_TOP,
        ui.ctx().pixels_per_point(),
    );
    vec![Shape::galley(at, shown, color)]
}

/// Un stepper dibujado en `rect` en estados forzados (el catálogo): cada botón en el suyo.
#[allow(clippy::too_many_arguments)]
pub fn forced_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    text: &str,
    unit: Option<&str>,
    minus: ButtonState,
    plus: ButtonState,
    enabled: bool,
) -> Vec<Shape> {
    let (left, field, right) = parts(rect);
    let mut shapes = field_shapes(ui, skin, field, unit, enabled);
    shapes.extend(number_shapes(
        ui,
        skin,
        number_area(ui, field, unit),
        text,
        enabled,
    ));
    shapes.extend(button_shapes(ui, skin, left, LEFT, MINUS, minus));
    shapes.extend(button_shapes(ui, skin, right, RIGHT, PLUS, plus));
    shapes
}

/// La columna izquierda de la etiqueta, fila por fila (1 a 14), como el boceto: `true` es el
/// borde oscuro (`color.control_border`) y `false` el claro de la caja de la unidad.
const TAG_EDGE: [bool; 14] = [
    false, false, false, false, false, false, true, false, true, true, true, true, true, true,
];

/// La etiqueta de eje en `rect` ([`TAG_W`]×[`HEIGHT`]): la caja de la unidad con la letra `axis`
/// centrada, redondeada del lado de afuera (a la izquierda; sin pixel en las esquinas) y recta del
/// lado del botón −, como el boceto.
pub fn tag_shapes(ui: &Ui, skin: &Skin, rect: Rect, axis: &str, enabled: bool) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = skin.colors();
    // La caja de la unidad empieza una columna adentro: la primera es el borde redondeado.
    let boxed = Rect::from_min_max(pos2(rect.left() + 1.0, rect.top()), rect.max);
    let unit = skin.piece(UNIT, WidgetState::Normal);
    let mut shapes = match unit {
        Some(p) => pixel_ui::piece_shapes(p, boxed, ppp),
        None => vec![Shape::rect_filled(boxed, 0.0, colors.canvas)],
    };
    // El borde izquierdo: sin la esquina de arriba, el claro y el oscuro de las filas del boceto,
    // y la luz de abajo en lugar de la esquina de abajo.
    let light = unit.map_or(colors.control_border, |p| p.pixel(0, 0));
    let cream = unit.map_or(colors.canvas, |p| p.pixel(0, HEIGHT as u32 - 1));
    let cell = |row: usize, color: Color32| {
        let top = rect.top() + row as f32;
        let at = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.left() + 1.0, top + 1.0));
        Shape::rect_filled(at, 0.0, color)
    };
    for (i, dark) in TAG_EDGE.iter().enumerate() {
        let color = if *dark { colors.control_border } else { light };
        shapes.push(cell(i + 1, color));
    }
    shapes.push(cell(HEIGHT as usize - 2, cream));
    let color = if enabled {
        colors.field_text
    } else {
        colors.button_text_disabled
    };
    let shown = galley(ui, axis, color);
    let ink = crate::menus::ink_width(&shown);
    let left = rect.left() + 1.0 + ((rect.width() - 1.0 - ink) / 2.0).floor();
    let at = tab_bar::text_pos(&shown, left, rect.top() + input::CAPS_TOP, ppp);
    shapes.push(Shape::galley(at, shown, color));
    shapes
}

/// El borde izquierdo del botón − pegado a una etiqueta: recto, sin las esquinas redondeadas del
/// botón solo (borde arriba y abajo, luz debajo), como el boceto. Con los colores de la pieza del
/// − en `state`, el estado con el que se dibujó el botón.
fn square_left_edge(skin: &Skin, button: Rect, state: ButtonState) -> Vec<Shape> {
    let Some(p) = skin.piece(LEFT, piece_state(state)) else {
        return Vec::new();
    };
    let cell = |row: u32, color: Color32| {
        let top = button.top() + row as f32;
        let at = Rect::from_min_max(
            pos2(button.left(), top),
            pos2(button.left() + 1.0, top + 1.0),
        );
        Shape::rect_filled(at, 0.0, color)
    };
    let h = HEIGHT as u32;
    let (edge, cream) = (p.pixel(0, 1), p.pixel(0, h - 2));
    vec![cell(0, edge), cell(h - 2, edge), cell(h - 1, cream)]
}

/// Los botones y el campo de un stepper en `rect`.
fn parts(rect: Rect) -> (Rect, Rect, Rect) {
    let left = Rect::from_min_size(rect.min, vec2(BUTTON_W, HEIGHT));
    let right = Rect::from_min_size(
        pos2(rect.right() - BUTTON_W, rect.top()),
        vec2(BUTTON_W, HEIGHT),
    );
    let field = Rect::from_min_max(
        pos2(left.right(), rect.top()),
        pos2(right.left(), rect.bottom()),
    );
    (left, field, right)
}

/// Un stepper de `width` en el lugar que sigue de `ui`: − y + cambian `value` en `step` (sin salir
/// de `range`); el número se arrastra o se escribe. `changed()` si cambió.
#[allow(clippy::too_many_arguments)]
pub fn stepper<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    width: f32,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    step: Num,
    unit: Option<&str>,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.round(), HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    stepper_at(ui, skin, rect, id_salt, value, range, step, unit)
}

/// [`stepper`] con la etiqueta de eje `axis` pegada a la izquierda: `width` es todo, la etiqueta
/// ([`TAG_W`]) y el stepper. La etiqueta no se toca (solo dice el eje).
#[allow(clippy::too_many_arguments)]
pub fn axis_stepper<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    width: f32,
    id_salt: &str,
    axis: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    step: Num,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.round(), HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let tag = Rect::from_min_size(rect.min, vec2(TAG_W, HEIGHT));
    let shapes = tag_shapes(ui, skin, tag, axis, ui.is_enabled());
    // Como el resto del stepper: deshabilitado se pinta con sus colores, no con transparencia.
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    painter.extend(shapes);
    let rest = Rect::from_min_max(pos2(tag.right(), rect.top()), rect.max);
    let (response, minus_state) =
        stepper_with_minus(ui, skin, rest, id_salt, value, range, step, None);
    // El botón − queda recto del lado de la etiqueta, en el estado con el que se dibujó.
    let (minus, ..) = parts(rest);
    painter.extend(square_left_edge(skin, minus, minus_state));
    response
}

/// [`stepper`] en `rect` (de [`HEIGHT`] de alto).
#[allow(clippy::too_many_arguments)]
pub fn stepper_at<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    step: Num,
    unit: Option<&str>,
) -> Response {
    stepper_with_minus(ui, skin, rect, id_salt, value, range, step, unit).0
}

/// [`stepper_at`], que además dice en qué estado se dibujó el botón −.
#[allow(clippy::too_many_arguments)]
fn stepper_with_minus<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    step: Num,
    unit: Option<&str>,
) -> (Response, ButtonState) {
    let (left, field, right) = parts(rect);
    let enabled = ui.is_enabled();
    let colors = *skin.colors();
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let before = value.to_f64();
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    let back = painter.add(Shape::Noop);
    // El número: el `DragValue` del campo numérico, centrado.
    let area = number_area(ui, field, unit);
    let probe = galley(ui, "0", colors.text);
    let ppp = ui.ctx().pixels_per_point();
    let top = tab_bar::text_pos(&probe, 0.0, rect.top() + input::CAPS_TOP, ppp).y;
    let inner = Rect::from_min_max(
        pos2(area.left() + 2.0, top),
        pos2(area.right() - 2.0, top + probe.size().y),
    );
    let text = if enabled {
        colors.text
    } else {
        colors.field_text_disabled
    };
    let look = NumberLook {
        rows: Some(input::selection_rows(rect.top())),
        ..NumberLook::new(&colors, text, Align::Center)
    };
    let speed = ((hi - lo) / 128.0).clamp(0.1, 1.0);
    let (mut response, _) = input::drag_number(
        ui,
        area,
        inner,
        (id_salt, "stepper"),
        value,
        range,
        speed,
        look,
    );
    painter.set(
        back,
        Shape::Vec(field_shapes(ui, skin, field, unit, enabled)),
    );
    // Los botones.
    let id = ui.id().with(("kit_stepper", id_salt));
    let mut shapes = Vec::new();
    let mut minus_state = ButtonState::Normal;
    for (rect, piece, icon, sign) in [(left, LEFT, MINUS, -1.0), (right, RIGHT, PLUS, 1.0)] {
        let current = value.to_f64();
        let can = if sign < 0.0 {
            current > lo
        } else {
            current < hi
        };
        let b = ui.interact(rect, id.with(piece), Sense::click());
        let usable = enabled && can;
        let label = if sign < 0.0 { "Decrease" } else { "Increase" };
        b.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, usable, label));
        if usable && b.clicked() {
            let next = (current + sign * step.to_f64()).clamp(lo, hi);
            *value = Num::from_f64(next);
        }
        let state = ButtonState::of(&b, usable);
        if sign < 0.0 {
            minus_state = state;
        }
        shapes.extend(button_shapes(ui, skin, rect, piece, icon, state));
    }
    painter.extend(shapes);
    if value.to_f64() != before {
        response.mark_changed();
    }
    (response, minus_state)
}

#[cfg(test)]
mod tests;
