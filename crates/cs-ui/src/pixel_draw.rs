//! Lo que comparten los dibujos de Pixel UI: las piezas sueltas del skin (íconos) a su tamaño o
//! centradas, los textos con las mayúsculas en una fila y el botón chico con ícono, con la pieza
//! `button`.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, TextStyle, Ui, Vec2, vec2};

use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

/// El tamaño de la pieza `name` del skin (cero si no está).
pub fn piece_size(skin: &Skin, name: &str) -> Vec2 {
    skin.piece(name, WidgetState::Normal)
        .map_or(Vec2::ZERO, |p| vec2(p.size[0] as f32, p.size[1] as f32))
}

/// La pieza `name` del skin a su tamaño, con la esquina en `at` (nada si no está). `ppp`: los
/// píxeles físicos por punto, para alinearla.
pub fn piece_at(skin: &Skin, name: &str, at: Pos2, ppp: f32) -> Vec<Shape> {
    skin.piece(name, WidgetState::Normal)
        .map(|p| {
            let size = vec2(p.size[0] as f32, p.size[1] as f32);
            pixel_ui::piece_shapes(p, Rect::from_min_size(at, size), ppp)
        })
        .unwrap_or_default()
}

/// [`piece_at`] centrada en `rect` (redondeada hacia arriba y a la izquierda).
pub fn piece_centered(skin: &Skin, name: &str, rect: Rect, ppp: f32) -> Vec<Shape> {
    let at = (rect.min + (rect.size() - piece_size(skin, name)) / 2.0).floor();
    piece_at(skin, name, at, ppp)
}

/// `shapes` medio transparentes: un ícono desactivado.
pub fn dimmed(shapes: Vec<Shape>) -> Vec<Shape> {
    crate::artboard_window::tinted(shapes, Color32::WHITE.gamma_multiply(DIM_ALPHA))
}

/// Cuánto se ve un ícono desactivado.
pub const DIM_ALPHA: f32 = 0.35;

/// `text` con su borde izquierdo en `left` y las mayúsculas empezando en la fila `cap_top`, en
/// la fuente de los textos. Devuelve la forma y el ancho.
pub fn caps_text(ui: &Ui, text: &str, left: f32, cap_top: f32, color: Color32) -> (Shape, f32) {
    caps_text_in(ui, TextStyle::Body, text, left, cap_top, color)
}

/// [`caps_text`] en el estilo `style` (`Small`: los textos chicos).
pub fn caps_text_in(
    ui: &Ui,
    style: TextStyle,
    text: &str,
    left: f32,
    cap_top: f32,
    color: Color32,
) -> (Shape, f32) {
    let font = style.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let width = galley.size().x.ceil();
    let ppp = ui.ctx().pixels_per_point();
    let at = tab_bar::text_pos(&galley, left, cap_top, ppp);
    (Shape::galley(at, galley, color), width)
}

/// Qué es un botón de [`skin_button`]: su id, su nombre accesible, si se puede apretar y, si es
/// de prender y apagar, si está prendido.
pub struct ButtonInfo<'a> {
    pub id: egui::Id,
    pub label: &'a str,
    pub enabled: bool,
    /// De prender y apagar: si está prendido (como la herramienta elegida: sin hundirse, con su
    /// marco rosa).
    pub selected: Option<bool>,
}

impl<'a> ButtonInfo<'a> {
    /// Un botón común, que se puede apretar.
    pub fn new(id: egui::Id, label: &'a str) -> Self {
        Self {
            id,
            label,
            enabled: true,
            selected: None,
        }
    }
}

/// Un botón con ícono en `rect` (el cuerpo): el botón del UI kit ([`crate::kit::button`]) con
/// sus estados (normal, con el puntero encima, presionado y deshabilitado). Uno de prender y
/// apagar (`selected`) prendido se ve presionado, como el elegido de un grupo. La línea clara de
/// la pieza queda debajo de `rect`, en la fila siguiente. `content` dibuja lo de adentro en el
/// rect que recibe (baja una fila con la cara presionada). Desactivado no se aprieta. El tooltip
/// lo pone quien llama.
pub fn skin_button(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    info: ButtonInfo,
    content: impl FnOnce(Rect) -> Vec<Shape>,
) -> Response {
    use crate::kit::button::ButtonState;
    let ButtonInfo {
        id,
        label,
        enabled,
        selected,
    } = info;
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    let on = enabled && ui.is_enabled();
    response.widget_info(|| match selected {
        Some(s) => egui::WidgetInfo::selected(egui::WidgetType::Button, on, s, label),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, on, label),
    });
    let state = match ButtonState::of(&response, on) {
        ButtonState::Disabled => ButtonState::Disabled,
        _ if selected == Some(true) => ButtonState::Pressed,
        state => state,
    };
    let shapes = button_shapes(ui, skin, rect, state, content);
    let mut painter = ui.painter().clone();
    painter.set_opacity(1.0);
    painter.extend(shapes);
    response
}

/// Lo que pinta [`skin_button`] en el estado `state`: la pieza del kit (con la línea clara debajo
/// de `rect`) y `content` (una fila más abajo con la cara presionada).
fn button_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    state: crate::kit::button::ButtonState,
    content: impl FnOnce(Rect) -> Vec<Shape>,
) -> Vec<Shape> {
    use crate::kit::button::{self, ButtonState};
    let ppp = ui.ctx().pixels_per_point();
    let drawn = rect.with_max_y(rect.bottom() + 1.0);
    let mut shapes = button::frame_shapes(skin, drawn, state, ppp);
    let down = if state == ButtonState::Pressed {
        button::PRESS_DROP
    } else {
        0.0
    };
    shapes.extend(content(rect.translate(vec2(0.0, down))));
    shapes
}

/// El fantasma de un [`skin_button`]: el mismo botón del kit, solo pintado (no responde a nada),
/// con `opacity`. Se ve normal o, si `selected`, presionado (como el elegido de un grupo). Su
/// nombre accesible es el de `info` (sin poder apretarse: el id de `info` no es el del botón de
/// verdad).
pub fn faded_skin_button(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    info: ButtonInfo,
    opacity: f32,
    content: impl FnOnce(Rect) -> Vec<Shape>,
) -> Response {
    use crate::kit::button::ButtonState;
    let response = ui.interact(rect, info.id, Sense::hover());
    let on = info.selected == Some(true);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), on, info.label)
    });
    let state = if on {
        ButtonState::Pressed
    } else {
        ButtonState::Normal
    };
    let shapes = button_shapes(ui, skin, rect, state, content);
    let mut painter = ui.painter().clone();
    painter.set_opacity(opacity);
    painter.extend(shapes);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skin_icons_are_drawn_at_their_size_and_centered() {
        let ctx = egui::Context::default();
        let skin = Skin::embedded(&ctx);
        assert_eq!(piece_size(&skin, "bar.info"), vec2(12.0, 13.0));
        assert_eq!(piece_size(&skin, "nope"), Vec2::ZERO);
        assert!(piece_at(&skin, "nope", Pos2::ZERO, 1.0).is_empty());
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(25.0, 16.0));
        let shapes = piece_centered(&skin, "icon.section.add", rect, 1.0);
        let bounds = shapes
            .iter()
            .fold(Rect::NOTHING, |b, s| b.union(s.visual_bounding_rect()));
        // 7×7 en 25×16: desde (9, 4).
        assert_eq!(
            bounds,
            Rect::from_min_size(Pos2::new(9.0, 4.0), vec2(7.0, 7.0))
        );
    }
}
