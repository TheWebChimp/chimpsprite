//! Lo que comparten los dibujos de Pixel UI hechos en código: los íconos escritos como texto
//! (una letra por píxel, cada una con su color), los textos con las mayúsculas en una fila y el
//! botón chico con la pieza `button` del skin.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, TextStyle, Ui, Vec2, vec2};

use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

/// Ancho y alto de un dibujo de texto.
pub fn ascii_size(rows: &[&str]) -> Vec2 {
    vec2(
        rows.first().map_or(0, |r| r.len()) as f32,
        rows.len() as f32,
    )
}

/// Los píxeles de `rows` con la esquina en `at`: un cuadrado de un punto por letra con color
/// (`color` devuelve `None` para lo transparente).
pub fn ascii_shapes(rows: &[&str], at: Pos2, color: impl Fn(u8) -> Option<Color32>) -> Vec<Shape> {
    let mut out = Vec::new();
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if let Some(color) = color(c) {
                let r = Rect::from_min_size(at + vec2(x as f32, y as f32), Vec2::splat(1.0));
                out.push(Shape::rect_filled(r, 0.0, color));
            }
        }
    }
    out
}

/// [`ascii_shapes`] centrado en `rect` (redondeado hacia arriba y a la izquierda).
pub fn ascii_centered(
    rows: &[&str],
    rect: Rect,
    color: impl Fn(u8) -> Option<Color32>,
) -> Vec<Shape> {
    let at = (rect.min + (rect.size() - ascii_size(rows)) / 2.0).floor();
    ascii_shapes(rows, at, color)
}

/// `text` con su borde izquierdo en `left` y las mayúsculas empezando en la fila `cap_top`, en
/// la fuente de los textos. Devuelve la forma y el ancho.
pub fn caps_text(ui: &Ui, text: &str, left: f32, cap_top: f32, color: Color32) -> (Shape, f32) {
    let font = TextStyle::Body.resolve(ui.style());
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

/// Un botón en `rect` con la pieza `button` del skin según su estado: apretado, prendido
/// (`selected`), con el puntero encima o normal (sin la pieza, un borde de 1 px). `content`
/// dibuja lo de adentro en el rect que recibe (baja una fila mientras se aprieta). Desactivado
/// no se aprieta. El tooltip lo pone quien llama.
pub fn skin_button(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    info: ButtonInfo,
    content: impl FnOnce(Rect) -> Vec<Shape>,
) -> Response {
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
    let state = if !enabled {
        WidgetState::Normal
    } else if response.is_pointer_button_down_on() {
        WidgetState::Down
    } else if selected == Some(true) {
        WidgetState::Active
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = match skin.piece("button", state) {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => vec![Shape::rect_stroke(
            rect.shrink(0.5),
            0.0,
            (1.0, skin.colors().outline),
            egui::StrokeKind::Middle,
        )],
    };
    let down = if state == WidgetState::Down { 1.0 } else { 0.0 };
    shapes.extend(content(rect.translate(vec2(0.0, down))));
    ui.painter().extend(shapes);
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_drawings_are_one_rect_per_colored_letter() {
        let rows = ["a.", " a"];
        let shapes = ascii_shapes(&rows, Pos2::new(10.0, 20.0), |c| {
            (c == b'a').then_some(Color32::RED)
        });
        let rects: Vec<Rect> = shapes
            .iter()
            .map(|s| match s {
                Shape::Rect(r) => r.rect,
                _ => panic!("{s:?}"),
            })
            .collect();
        assert_eq!(
            rects,
            vec![
                Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::splat(1.0)),
                Rect::from_min_size(Pos2::new(11.0, 21.0), Vec2::splat(1.0)),
            ]
        );
        assert_eq!(ascii_size(&rows), vec2(2.0, 2.0));
    }

    #[test]
    fn centered_drawings_round_toward_the_top_left() {
        let rows = ["aaa"];
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(8.0, 4.0));
        let shapes = ascii_centered(&rows, rect, |_| Some(Color32::RED));
        let Shape::Rect(first) = &shapes[0] else {
            panic!()
        };
        assert_eq!(first.rect.min, Pos2::new(2.0, 1.0));
    }
}
