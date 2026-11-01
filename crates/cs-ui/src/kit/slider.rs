//! El slider (paso 5 del UI kit, mockup de componentes, frame 9): la pista `slider.track`
//! (9 filas: 7 de relleno (189,164,153), la oscura y la línea clara; se estira a lo ancho) y la
//! perilla `slider.thumb` (13×14: el cuerpo de 11 y una columna de sombra a cada lado, sobre la
//! pista). `.hover` (con el puntero encima o arrastrándola: el relleno blanco) y `.disabled` no
//! están en el diseño.
//!
//! Ocupa el alto de un botón ([`HEIGHT`]): la perilla desde la fila 1 y la pista desde la 4 (la
//! perilla le sobresale 3 filas arriba, como en el mockup). La perilla va de punta a punta de la
//! pista: en el mínimo, su sombra izquierda en la primera columna.

use std::ops::RangeInclusive;

use eframe::egui::{
    self, Id, Key, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, emath, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use super::button::{self, ButtonState};
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};

pub const TRACK: &str = "slider.track";
pub const THUMB: &str = "slider.thumb";

/// El alto en el layout: el de un botón.
pub const HEIGHT: f32 = button::HEIGHT;
/// La fila de arriba de la perilla y la de la pista.
pub const THUMB_TOP: f32 = 1.0;
pub const TRACK_TOP: f32 = 4.0;
/// Sin las piezas, los tamaños del mockup.
const THUMB_SIZE: Vec2 = vec2(13.0, 14.0);
const TRACK_HEIGHT: f32 = 9.0;

/// Cuánto se mueve con las flechas del teclado: 1 en los enteros; en los demás, 1/100 del rango.
fn key_step(lo: f64, hi: f64, integral: bool) -> f64 {
    if integral { 1.0 } else { (hi - lo) / 100.0 }
}

/// La variante de las piezas en `state`: presionado (arrastrando) se ve como encima.
fn piece_state(state: ButtonState) -> WidgetState {
    match state {
        ButtonState::Normal => WidgetState::Normal,
        ButtonState::Hover | ButtonState::Pressed => WidgetState::Hover,
        ButtonState::Disabled => WidgetState::Disabled,
    }
}

fn thumb_size(skin: &Skin) -> Vec2 {
    skin.piece(THUMB, WidgetState::Normal)
        .map_or(THUMB_SIZE, |p| vec2(p.size[0] as f32, p.size[1] as f32))
}

/// Por dónde va la perilla: lo que puede correr su borde izquierdo en `rect`.
fn travel(skin: &Skin, rect: Rect) -> f32 {
    (rect.width() - thumb_size(skin).x).max(1.0)
}

/// Un slider de `width` de ancho en el lugar que sigue de `ui`. Se arrastra la perilla, se hace
/// clic en la pista (la perilla salta ahí) o, con el foco, se mueve con las flechas (Inicio y Fin:
/// los extremos).
pub fn slider<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    width: f32,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.round(), HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let id = ui.id().with(id_salt);
    slider_at(ui, skin, rect, id, value, range)
}

/// [`slider`] en `rect` (ya ubicado, de [`HEIGHT`] de alto) con el id `id`.
pub fn slider_at<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id: Id,
    value: &mut Num,
    range: RangeInclusive<Num>,
) -> Response {
    let mut response = ui.interact(rect, id, Sense::click_and_drag());
    let (lo, hi) = (range.start().to_f64(), range.end().to_f64());
    let current = value.to_f64();
    let mut next = current;
    if let Some(p) = response.interact_pointer_pos()
        && (response.dragged() || response.clicked() || response.is_pointer_button_down_on())
    {
        let t =
            ((p.x - rect.left() - thumb_size(skin).x / 2.0) / travel(skin, rect)).clamp(0.0, 1.0);
        next = lo + t as f64 * (hi - lo);
    }
    if response.has_focus() {
        let step = key_step(lo, hi, Num::INTEGRAL);
        ui.input(|i| {
            if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown) {
                next -= step;
            }
            if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp) {
                next += step;
            }
            if i.key_pressed(Key::Home) {
                next = lo;
            }
            if i.key_pressed(Key::End) {
                next = hi;
            }
        });
    }
    let next = if Num::INTEGRAL { next.round() } else { next }.clamp(lo.min(hi), hi.max(lo));
    if next != current {
        *value = Num::from_f64(next);
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    let shown = value.to_f64();
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::new(egui::WidgetType::Slider);
        info.enabled = enabled;
        info.value = Some(shown);
        info
    });
    if ui.is_rect_visible(rect) {
        let t = if hi > lo {
            ((shown - lo) / (hi - lo)).clamp(0.0, 1.0)
        } else {
            0.0
        } as f32;
        let state = ButtonState::of(&response, enabled);
        let state = if enabled && response.dragged() {
            ButtonState::Pressed
        } else {
            state
        };
        let ppp = ui.ctx().pixels_per_point();
        let mut shapes = slider_shapes(skin, rect, t, state, ppp);
        if response.has_focus() {
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

/// El slider con su encabezado (mockup de componentes, frame 9, x 660..821, y 411..441): el
/// botón de restaurar (un botón con ícono del kit, `icon.reset`, de 13×15), el nombre 3 px
/// después y el valor pegado a la derecha, los dos en `color.text` con las mayúsculas en la fila
/// 4 del botón; debajo, el slider.
pub const RESET_SIZE: Vec2 = vec2(13.0, 15.0);
pub const RESET_ICON: &str = "icon.reset";
/// Del botón de restaurar a la tinta del nombre.
pub const LABEL_GAP: f32 = 3.0;
/// El alto del encabezado y el del slider con él.
pub const HEADER_HEIGHT: f32 = 15.0;
pub const LABELED_HEIGHT: f32 = HEADER_HEIGHT + HEIGHT;

/// [`slider`] con su encabezado, de `width` de ancho, en el lugar que sigue de `ui`: el botón de
/// restaurar (deshabilitado si el valor ya es `default`) lo vuelve a `default`; `format` escribe
/// el valor. El `Response` es el del slider (`changed()` también si se restauró).
#[allow(clippy::too_many_arguments)]
pub fn labeled_slider<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    width: f32,
    id_salt: &str,
    label: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    default: Num,
    format: impl Fn(Num) -> String,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.round(), LABELED_HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let id = ui.id().with(id_salt);
    labeled_slider_at(ui, skin, rect, id, label, value, range, default, format)
}

/// [`labeled_slider`] en `rect` (de [`LABELED_HEIGHT`] de alto) con el id `id`.
#[allow(clippy::too_many_arguments)]
pub fn labeled_slider_at<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id: Id,
    label: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    default: Num,
    format: impl Fn(Num) -> String,
) -> Response {
    let reset_rect = Rect::from_min_size(rect.min, RESET_SIZE);
    let mut header = ui.new_child(egui::UiBuilder::new().max_rect(reset_rect));
    if value.to_f64() == default.to_f64() {
        header.disable();
    }
    let reset = button::icon_button_at(
        &mut header,
        skin,
        reset_rect,
        id.with("reset"),
        RESET_ICON,
        "Back to the default",
    );
    let reset = pixel_ui::Tip::tip(reset, "Back to the default");
    let bar = Rect::from_min_size(
        pos2(rect.left(), rect.top() + HEADER_HEIGHT),
        vec2(rect.width(), HEIGHT),
    );
    let mut response = slider_at(ui, skin, bar, id, value, range);
    if reset.clicked() && value.to_f64() != default.to_f64() {
        *value = default;
        response.mark_changed();
    }
    // El nombre y el valor, con las mayúsculas en la fila de las del botón.
    let color = skin.colors().text;
    let caps = rect.top() + button::CAPS_TOP;
    let (name, _) =
        crate::pixel_draw::caps_text(ui, label, reset_rect.right() + LABEL_GAP, caps, color);
    let text = format(*value);
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.clone(), font, color);
    let ink = crate::menus::ink_width(&galley);
    let (shown, _) = crate::pixel_draw::caps_text(ui, &text, rect.right() - ink, caps, color);
    ui.painter().extend([name, shown]);
    response
}

/// Un slider dibujado en `rect` (de [`HEIGHT`] de alto) con la perilla en `t` (0 a 1) y en
/// `state` (el catálogo lo usa sin interacción).
pub fn slider_shapes(skin: &Skin, rect: Rect, t: f32, state: ButtonState, ppp: f32) -> Vec<Shape> {
    let colors = skin.colors();
    let piece = piece_state(state);
    let track_h = skin
        .piece(TRACK, WidgetState::Normal)
        .map_or(TRACK_HEIGHT, |p| p.size[1] as f32);
    let track = Rect::from_min_size(
        pos2(rect.left(), rect.top() + TRACK_TOP),
        vec2(rect.width(), track_h),
    );
    let mut shapes = match skin.piece(TRACK, piece) {
        Some(p) => pixel_ui::piece_shapes(p, track, ppp),
        None => vec![Shape::rect_filled(
            track.with_max_y(track.bottom() - 2.0),
            0.0,
            colors.canvas,
        )],
    };
    let size = thumb_size(skin);
    let left = (rect.left() + t * travel(skin, rect)).round();
    let thumb = Rect::from_min_size(pos2(left, rect.top() + THUMB_TOP), size);
    match skin.piece(THUMB, piece) {
        Some(p) => shapes.extend(pixel_ui::piece_shapes(p, thumb, ppp)),
        None => shapes.extend(pixel_ui::flat_frame(
            colors,
            thumb.shrink2(vec2(1.0, 0.0)),
            0.0,
            ppp,
        )),
    }
    shapes
}

#[cfg(test)]
mod tests;
