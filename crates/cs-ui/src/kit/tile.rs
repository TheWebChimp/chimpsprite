//! Los botones de ícono planos, de prender y apagar (los modos de selección del panel 3D, como el
//! boceto del usuario `.refs/chimpcad/mockup-mode-icons.png`): una caja de 13×13 con el contorno
//! de `color.ruler_tick` (sin las esquinas), una fila de `color.highlight` debajo (con las dos
//! esquinas de abajo del contorno) y, adentro, el ícono de 11×11.
//!
//! - **Prendido** (o presionado): lo de adentro, de `color.highlight`.
//! - **Hover:** el ícono se aclara, como los íconos sueltos ([`crate::kit::icon`]).
//! - **Deshabilitado:** todo a media opacidad.

use eframe::egui::{self, Color32, Id, Pos2, Rect, Response, Sense, Shape, Ui, Vec2, vec2};

use crate::kit::button::ButtonState;
use crate::skin::{Skin, WidgetState};

/// La caja y la fila de debajo.
pub const SIZE: Vec2 = vec2(13.0, 14.0);
/// La caja sola (lo que se toca).
const BOX: f32 = 13.0;

/// Un píxel de `color` en `(x, y)` desde `at`.
fn dot(at: Pos2, x: f32, y: f32, w: f32, h: f32, color: Color32) -> Shape {
    Shape::rect_filled(Rect::from_min_size(at + vec2(x, y), vec2(w, h)), 0.0, color)
}

/// El marco (y lo de adentro, si va lleno) del botón con la esquina en `at`.
fn frame_shapes(skin: &Skin, at: Pos2, filled: bool) -> Vec<Shape> {
    let c = skin.colors();
    let (line, light) = (c.ruler_tick, c.highlight);
    let mut shapes = vec![
        dot(at, 1.0, 0.0, BOX - 2.0, 1.0, line),
        dot(at, 0.0, 1.0, 1.0, BOX - 2.0, line),
        dot(at, BOX - 1.0, 1.0, 1.0, BOX - 2.0, line),
        dot(at, 1.0, BOX - 1.0, BOX - 2.0, 1.0, line),
        dot(at, 0.0, BOX - 1.0, 1.0, 1.0, light),
        dot(at, BOX - 1.0, BOX - 1.0, 1.0, 1.0, light),
        dot(at, 1.0, BOX, BOX - 2.0, 1.0, light),
    ];
    if filled {
        shapes.push(dot(at, 1.0, 1.0, BOX - 2.0, BOX - 2.0, light));
    }
    shapes
}

/// El botón con la esquina en `at` (en puntos enteros) y el ícono `icon` del skin: `on` lo
/// muestra prendido. `label` es su nombre accesible y su tooltip (el globo de la barra de
/// íconos).
pub fn tile_toggle_at(
    ui: &mut Ui,
    skin: &Skin,
    at: Pos2,
    id: Id,
    icon: &str,
    label: &str,
    on: bool,
) -> Response {
    let rect = Rect::from_min_size(at, Vec2::splat(BOX));
    let response = ui.interact(rect, id, Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, label));
    if ui.is_rect_visible(rect) {
        let state = ButtonState::of(&response, enabled);
        let filled = on || state == ButtonState::Pressed;
        let mut shapes = frame_shapes(skin, at, filled);
        if let Some(piece) = skin.piece(icon, WidgetState::Normal) {
            // El ícono no baja al presionar (llena la caja): solo se aclara con el puntero.
            let look = match state {
                ButtonState::Hover => ButtonState::Hover,
                _ => ButtonState::Normal,
            };
            let ppp = ui.ctx().pixels_per_point();
            shapes.extend(crate::kit::icon::icon_shapes(
                piece,
                at + vec2(1.0, 1.0),
                look,
                ppp,
            ));
        }
        let mut painter = ui.painter().clone();
        painter.set_opacity(if enabled { 1.0 } else { 0.5 });
        painter.extend(shapes);
    }
    crate::pixel_ui::bubble_tooltip(response, skin, label)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin() -> Skin {
        Skin::load(&egui::Context::default(), crate::skin::EMBEDDED).expect("the built-in skin")
    }

    #[test]
    fn the_frame_is_the_mockups_box_with_the_light_row_below() {
        let skin = skin();
        let c = *skin.colors();
        let rects: Vec<(Rect, Color32)> = frame_shapes(&skin, Pos2::ZERO, false)
            .into_iter()
            .filter_map(|s| match s {
                Shape::Rect(r) => Some((r.rect, r.fill)),
                _ => None,
            })
            .collect();
        let at = |x: f32, y: f32| {
            rects
                .iter()
                .rev()
                .find(|(r, _)| r.contains(egui::pos2(x + 0.5, y + 0.5)))
                .map(|(_, c)| *c)
        };
        assert_eq!(at(0.0, 0.0), None, "open corner");
        assert_eq!(at(1.0, 0.0), Some(c.ruler_tick));
        assert_eq!(at(0.0, 6.0), Some(c.ruler_tick));
        assert_eq!(at(6.0, 6.0), None, "empty inside when off");
        assert_eq!(at(0.0, 12.0), Some(c.highlight));
        assert_eq!(at(6.0, 12.0), Some(c.ruler_tick));
        assert_eq!(at(6.0, 13.0), Some(c.highlight));
        assert_eq!(at(0.0, 13.0), None);
        let on = frame_shapes(&skin, Pos2::ZERO, true);
        assert_eq!(on.len(), rects.len() + 1, "filled inside when on");
    }

    #[test]
    fn the_mode_icons_are_in_the_skin() {
        let skin = skin();
        for name in ["object", "vertex", "edge", "face"] {
            let piece = skin
                .piece(&format!("icon.mode.{name}"), WidgetState::Normal)
                .unwrap_or_else(|| panic!("icon.mode.{name}"));
            assert_eq!(piece.size, [11, 11], "{name}");
        }
    }
}
