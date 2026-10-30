//! El botón de texto (paso 1 del UI kit, `docs/superpowers/specs/2026-10-30-ui-kit-buttons.md`):
//! las piezas `button`, `button.hover`, `button.down` y `button.disabled` del skin (49×17, con la
//! línea clara de abajo adentro) y el texto con las medidas del mockup (frame 9).
//!
//! Todos los botones de texto de Pixel UI pasan por acá; los de ícono y los grupos de opciones,
//! todavía no.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, Galley, Id, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui,
    Vec2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::skin::{Skin, SkinColors, WidgetState};
use crate::tab_bar;

/// Presionado, la cara del botón baja 1 px: la pieza `button.down` empieza con una fila
/// transparente y no tiene el bisel oscuro de abajo, y el texto baja con ella.
pub const PRESS_DROP: f32 = 1.0;

/// La pieza del skin; la de cada estado es su variante (`.hover`, `.down`, `.disabled`).
const PIECE: &str = "button";

/// El alto del botón en el layout: el cuerpo de 16 y la línea clara de abajo, que es parte de la
/// pieza.
pub const HEIGHT: f32 = 17.0;

// A lo ancho: el contorno (1 px), la columna del bisel (1 px: `#ddcebf` a la izquierda y la
// clara a la derecha) y siempre 4 px de relleno hasta la tinta del texto; del otro lado, igual.
// El ancho justo es `1 + 5 + tinta + 5 + 1`.
pub const OUTLINE: f32 = 1.0;
pub const BEVEL_X: f32 = 1.0;
pub const INNER_PAD_X: f32 = 4.0;
/// Del contorno a la tinta: el bisel y el relleno.
pub const PAD_X: f32 = BEVEL_X + INNER_PAD_X;

// A lo alto, en filas de la pieza: el contorno y el brillo de arriba (0 y 1); 1 fila de aire
// sobre el glifo más alto (las mayúsculas con acento, que suben 1 px más que las mayúsculas: fila
// 3); las mayúsculas de 6 (filas 4 a 9, la línea base justo debajo); los descendentes, 2 más
// (hasta la 11); 2 filas de aire (12 y 13), y el bisel, el contorno y la línea clara (14 a 16).
pub const TOP_ROWS: f32 = 2.0;
pub const AIR_TOP: f32 = 1.0;
pub const ACCENT_ABOVE_CAPS: f32 = 1.0;
/// Las mayúsculas empiezan 4 filas debajo del borde de arriba.
pub const CAPS_TOP: f32 = TOP_ROWS + AIR_TOP + ACCENT_ABOVE_CAPS;
pub const DESCENT: f32 = 2.0;
pub const AIR_BOTTOM: f32 = 2.0;
pub const BOTTOM_ROWS: f32 = 3.0;
// Las filas suman el alto del botón.
const _: () =
    assert!(CAPS_TOP + tab_bar::CAP_HEIGHT + DESCENT + AIR_BOTTOM + BOTTOM_ROWS == HEIGHT);

/// La sombra del texto: 1 px abajo, como en el mockup (la spec decía 1 a la derecha y 1 abajo,
/// pero los píxeles del frame 9 la tienen justo debajo de la tinta). Con el puntero encima no hay.
pub const SHADOW: Vec2 = vec2(0.0, 1.0);

/// Los estados del botón, cada uno con su pieza.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Normal,
    Hover,
    Pressed,
    Disabled,
}

impl ButtonState {
    pub const ALL: [ButtonState; 4] = [
        ButtonState::Normal,
        ButtonState::Hover,
        ButtonState::Pressed,
        ButtonState::Disabled,
    ];

    /// El estado de un botón según lo que pasa con él; `enabled`: si se puede usar.
    pub fn of(response: &Response, enabled: bool) -> Self {
        if !enabled {
            ButtonState::Disabled
        } else if response.is_pointer_button_down_on() {
            ButtonState::Pressed
        } else if response.hovered() {
            ButtonState::Hover
        } else {
            ButtonState::Normal
        }
    }

    /// El nombre del estado (en el catálogo).
    pub fn name(self) -> &'static str {
        match self {
            ButtonState::Normal => "Normal",
            ButtonState::Hover => "Hover",
            ButtonState::Pressed => "Pressed",
            ButtonState::Disabled => "Disabled",
        }
    }

    /// La variante de la pieza.
    fn piece(self) -> WidgetState {
        match self {
            ButtonState::Normal => WidgetState::Normal,
            ButtonState::Hover => WidgetState::Hover,
            ButtonState::Pressed => WidgetState::Down,
            ButtonState::Disabled => WidgetState::Disabled,
        }
    }

    /// Si el texto lleva sombra: en todos menos con el puntero encima.
    pub fn has_shadow(self) -> bool {
        self != ButtonState::Hover
    }
}

/// El color del texto: `#050403`, casi negro, que es `color.text`; deshabilitado,
/// `color.button_text_disabled`.
pub fn text_color(colors: &SkinColors, state: ButtonState) -> Color32 {
    match state {
        ButtonState::Disabled => colors.button_text_disabled,
        _ => colors.text,
    }
}

/// El color de la sombra del texto: `#ddcebf`, el de la columna izquierda del bisel, que es
/// `color.background`.
pub fn shadow_color(colors: &SkinColors) -> Color32 {
    colors.background
}

/// El texto como se muestra: "…" con los puntos de la fuente pixel (el de la fuente de respaldo
/// no es pixel art), como en los menús.
fn shown(label: &str) -> String {
    label.replace('…', tab_bar::ellipsis())
}

/// El texto armado sin color (se pinta con el de cada forma).
fn galley(ui: &Ui, label: &str) -> Arc<Galley> {
    let font = TextStyle::Button.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(shown(label), font, Color32::PLACEHOLDER)
}

/// El ancho de la tinta de `label`.
pub fn ink_width(ui: &Ui, label: &str) -> f32 {
    crate::menus::ink_width(&galley(ui, label))
}

/// El ancho justo de un botón con `label`: `1 + 5 + tinta + 5 + 1`.
pub fn width(ui: &Ui, label: &str) -> f32 {
    2.0 * (OUTLINE + PAD_X) + ink_width(ui, label)
}

/// Un botón de texto con su ancho justo. El estado sale de la interacción y de `ui.is_enabled()`;
/// devuelve un `Response` normal (clic, foco y teclado: Enter o Espacio con foco).
pub fn button(ui: &mut Ui, skin: &Skin, label: &str) -> Response {
    button_sized(ui, skin, label, 0.0)
}

/// [`button`] de por lo menos `min_width` de ancho; si sobra, el texto va centrado.
pub fn button_sized(ui: &mut Ui, skin: &Skin, label: &str, min_width: f32) -> Response {
    let size = vec2(width(ui, label).max(min_width.round()), HEIGHT);
    let (slot, response) = ui.allocate_exact_size(size, Sense::click());
    let rect = Rect::from_min_size(slot.min.round(), size);
    paint(ui, skin, rect, &response, label, |state| {
        centered_label(ui, skin, rect, label, state)
    });
    response
}

/// Un botón de texto en `rect` (ya ubicado, de [`HEIGHT`] de alto) con el id `id`.
pub fn button_at(ui: &mut Ui, skin: &Skin, rect: Rect, id: Id, label: &str) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    paint(ui, skin, rect, &response, label, |state| {
        centered_label(ui, skin, rect, label, state)
    });
    response
}

/// Dibuja un botón en `rect` que responde con `response`: la pieza de su estado, lo de adentro
/// que arma `content` (por lo común, [`label_shapes`]) y, con el foco, el anillo `accent`.
/// Deshabilitado se ve entero (sin la transparencia de egui): cambian la pieza y el texto.
/// `label` es el nombre accesible.
pub fn paint(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    response: &Response,
    label: &str,
    content: impl FnOnce(ButtonState) -> Vec<Shape>,
) {
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let state = ButtonState::of(response, enabled);
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = frame_shapes(skin, rect, state, ppp);
    shapes.extend(content(state));
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

/// Un botón dibujado en el estado `state` (sin interacción: el catálogo muestra así los cuatro).
pub fn button_shapes(
    skin: &Skin,
    rect: Rect,
    label: &str,
    state: ButtonState,
    ui: &Ui,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = frame_shapes(skin, rect, state, ppp);
    shapes.extend(centered_label(ui, skin, rect, label, state));
    shapes
}

/// La pieza del estado en `rect`; sin la pieza, el marco plano del cuerpo (sin la línea clara).
pub fn frame_shapes(skin: &Skin, rect: Rect, state: ButtonState, ppp: f32) -> Vec<Shape> {
    match skin.piece(PIECE, state.piece()) {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => {
            let body = rect.with_max_y(rect.bottom() - 1.0);
            pixel_ui::flat_frame(skin.colors(), body, 0.0, ppp)
        }
    }
}

/// El texto centrado a lo ancho en `rect`: con el ancho justo, la tinta empieza en `left + 6`.
/// Si no cabe justo a la mitad, el píxel de sobra queda a la derecha.
fn centered_label(ui: &Ui, skin: &Skin, rect: Rect, label: &str, state: ButtonState) -> Vec<Shape> {
    let left = rect.left() + ((rect.width() - ink_width(ui, label)) / 2.0).floor();
    label_shapes(ui, skin, label, left, rect.top(), state)
}

/// El texto de un botón cuya pieza empieza en la fila `top`: la tinta desde `left`, las
/// mayúsculas en `top + CAPS_TOP` y, salvo con el puntero encima, la sombra (primero la sombra y
/// encima el texto). Presionado, baja `PRESS_DROP` con la cara del botón.
pub fn label_shapes(
    ui: &Ui,
    skin: &Skin,
    label: &str,
    left: f32,
    top: f32,
    state: ButtonState,
) -> Vec<Shape> {
    let colors = skin.colors();
    let galley = galley(ui, label);
    let ppp = ui.ctx().pixels_per_point();
    let drop = if state == ButtonState::Pressed {
        PRESS_DROP
    } else {
        0.0
    };
    let at = tab_bar::text_pos(&galley, left, top + CAPS_TOP + drop, ppp);
    let mut shapes = Vec::with_capacity(2);
    if state.has_shadow() {
        shapes.push(Shape::galley(
            at + SHADOW,
            galley.clone(),
            shadow_color(colors),
        ));
    }
    shapes.push(Shape::galley(at, galley, text_color(colors, state)));
    shapes
}

#[cfg(test)]
mod tests;
