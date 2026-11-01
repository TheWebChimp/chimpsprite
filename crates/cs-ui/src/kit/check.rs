//! Las casillas y los radios (paso 5 del UI kit, mockup de componentes, frame 9): `check`,
//! `check.on` y `check.mixed` (12×13, con su línea clara abajo) y `radio` y `radio.on` (10×11),
//! cada una con `.hover` (el borde (89,77,77): no está en el diseño) y `.disabled` (copiada del
//! mockup).
//!
//! Ocupan el alto de un campo ([`HEIGHT`]), para ir en la misma fila que campos y selects: las
//! mayúsculas del texto en la fila de las de los campos (la 5) y, 3 px antes de la tinta, la
//! casilla desde la fila 3 y el radio desde la 3, con su punto en las filas de las mayúsculas.

use eframe::egui::{
    self, Color32, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui, Vec2, pos2,
    vec2,
};
use eframe::emath::GuiRounding as _;

use super::button::{self, ButtonState};
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

/// El alto en el layout: el de un botón.
pub const HEIGHT: f32 = button::HEIGHT;
/// Las mayúsculas del texto, en la fila de las del texto de los campos (`kit::input`).
pub const CAPS_TOP: f32 = super::input::CAPS_TOP;
/// De la casilla (o el radio) a la tinta del texto.
pub const GAP: f32 = 3.0;

/// Lo que muestra una casilla.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Off,
    On,
    /// Ni sí ni no (por ejemplo, unas capas sí y otras no): la raya verde.
    Mixed,
}

impl Check {
    pub const ALL: [Check; 3] = [Check::On, Check::Mixed, Check::Off];

    pub fn of(on: bool) -> Self {
        if on { Check::On } else { Check::Off }
    }
}

/// Qué control es: una casilla o un radio, con lo que muestra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Check(Check),
    Radio(bool),
}

impl Mark {
    /// La pieza base (las variantes son `.hover` y `.disabled`).
    pub fn piece(self) -> &'static str {
        match self {
            Mark::Check(Check::Off) => "check",
            Mark::Check(Check::On) => "check.on",
            Mark::Check(Check::Mixed) => "check.mixed",
            Mark::Radio(false) => "radio",
            Mark::Radio(true) => "radio.on",
        }
    }

    /// El tamaño de la pieza (el del mockup, si el skin no la trae).
    fn size(self, skin: &Skin) -> Vec2 {
        match skin.piece(self.piece(), WidgetState::Normal) {
            Some(p) => vec2(p.size[0] as f32, p.size[1] as f32),
            None => self.fallback_size(),
        }
    }

    fn fallback_size(self) -> Vec2 {
        match self {
            Mark::Check(_) => vec2(12.0, 13.0),
            Mark::Radio(_) => vec2(10.0, 11.0),
        }
    }

    /// La fila de arriba de la pieza: la casilla, 2 filas arriba de las mayúsculas (su borde de
    /// abajo, a la par del de los campos); el radio, con su punto en las filas de las mayúsculas.
    pub fn top(self) -> f32 {
        match self {
            Mark::Check(_) => CAPS_TOP - 2.0,
            Mark::Radio(_) => CAPS_TOP - 2.0,
        }
    }

    fn widget_type(self) -> egui::WidgetType {
        match self {
            Mark::Check(_) => egui::WidgetType::Checkbox,
            Mark::Radio(_) => egui::WidgetType::RadioButton,
        }
    }

    fn selected(self) -> bool {
        matches!(self, Mark::Check(Check::On) | Mark::Radio(true))
    }
}

/// La variante de la pieza: presionado se ve como con el puntero encima.
fn piece_state(state: ButtonState) -> WidgetState {
    match state {
        ButtonState::Normal => WidgetState::Normal,
        ButtonState::Hover | ButtonState::Pressed => WidgetState::Hover,
        ButtonState::Disabled => WidgetState::Disabled,
    }
}

fn text_color(skin: &Skin, state: ButtonState) -> Color32 {
    button::text_color(skin.colors(), state)
}

fn galley(ui: &Ui, label: &str) -> std::sync::Arc<egui::Galley> {
    let font = TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER)
}

/// El ancho de un control con `label`: la pieza, el aire y la tinta (sin texto, solo la pieza).
pub fn width(ui: &Ui, skin: &Skin, mark: Mark, label: &str) -> f32 {
    let size = mark.size(skin);
    if label.is_empty() {
        size.x
    } else {
        size.x + GAP + crate::menus::ink_width(&galley(ui, label))
    }
}

/// Una casilla con su texto, como `ui.checkbox(checked, label)`: un clic la prende o la apaga.
pub fn checkbox(ui: &mut Ui, skin: &Skin, checked: &mut bool, label: &str) -> Response {
    let mut response = control(ui, skin, Mark::Check(Check::of(*checked)), label);
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    response
}

/// Una casilla que muestra `value` (también [`Check::Mixed`]); qué hacer con el clic lo decide
/// quien la llama.
pub fn tri_checkbox(ui: &mut Ui, skin: &Skin, value: Check, label: &str) -> Response {
    control(ui, skin, Mark::Check(value), label)
}

/// Un radio, prendido si `on`; qué hacer con el clic lo decide quien lo llama.
pub fn radio(ui: &mut Ui, skin: &Skin, on: bool, label: &str) -> Response {
    control(ui, skin, Mark::Radio(on), label)
}

/// Una opción de un grupo, como `ui.radio_value(current, value, label)`: elegirla cambia `current`
/// a `value`.
pub fn radio_value<T: PartialEq>(
    ui: &mut Ui,
    skin: &Skin,
    current: &mut T,
    value: T,
    label: &str,
) -> Response {
    let mut response = radio(ui, skin, *current == value, label);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

/// El control entero (pieza y texto) en el lugar que sigue de `ui`.
fn control(ui: &mut Ui, skin: &Skin, mark: Mark, label: &str) -> Response {
    let size = vec2(width(ui, skin, mark, label), HEIGHT);
    let (slot, response) = ui.allocate_exact_size(size, Sense::click());
    let rect = Rect::from_min_size(slot.min.round(), size);
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(mark.widget_type(), enabled, mark.selected(), label)
    });
    if ui.is_rect_visible(rect) {
        let state = ButtonState::of(&response, enabled);
        let mut shapes = shapes(ui, skin, rect.min, mark, label, state);
        if response.has_focus() {
            let ppp = ui.ctx().pixels_per_point();
            shapes.push(Shape::rect_stroke(
                rect.expand(1.0).round_to_pixels(ppp),
                0.0,
                Stroke::new(1.0, skin.colors().accent),
                StrokeKind::Outside,
            ));
        }
        let mut painter = ui.painter().clone();
        painter.set_opacity(1.0);
        painter.extend(shapes);
    }
    response
}

/// Un control dibujado en `state` desde `at` (la esquina de arriba a la izquierda de su lugar, de
/// [`HEIGHT`] de alto): la pieza y, si hay, el texto.
pub fn shapes(
    ui: &Ui,
    skin: &Skin,
    at: egui::Pos2,
    mark: Mark,
    label: &str,
    state: ButtonState,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let size = mark.size(skin);
    let piece_rect = Rect::from_min_size(pos2(at.x, at.y + mark.top()), size);
    let mut shapes = match skin.piece(mark.piece(), piece_state(state)) {
        Some(piece) => pixel_ui::piece_shapes(piece, piece_rect, ppp),
        None => fallback(skin, piece_rect, mark, ppp),
    };
    if !label.is_empty() {
        let galley = galley(ui, label);
        let pos = tab_bar::text_pos(&galley, at.x + size.x + GAP, at.y + CAPS_TOP, ppp);
        shapes.push(Shape::galley(pos, galley, text_color(skin, state)));
    }
    shapes
}

/// Sin la pieza: un cuadro plano y, si está marcada, un cuadrito `text` adentro.
fn fallback(skin: &Skin, rect: Rect, mark: Mark, ppp: f32) -> Vec<Shape> {
    let colors = skin.colors();
    let body = rect.with_max_y(rect.bottom() - 1.0);
    let mut shapes = pixel_ui::flat_frame(colors, body, 0.0, ppp);
    let inner = body.shrink(3.0);
    match mark {
        Mark::Check(Check::On) | Mark::Radio(true) => {
            shapes.push(Shape::rect_filled(inner, 0.0, colors.text));
        }
        Mark::Check(Check::Mixed) => {
            let bar = Rect::from_center_size(inner.center(), vec2(inner.width(), 2.0));
            shapes.push(Shape::rect_filled(
                bar.round_to_pixels(ppp),
                0.0,
                colors.text,
            ));
        }
        _ => {}
    }
    shapes
}

#[cfg(test)]
mod tests;
