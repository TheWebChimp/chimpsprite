//! Los íconos sueltos que se tocan (paso 7 del UI kit): un ícono sin marco de botón (play, pausa
//! y centrar de la barra del preview, el swap de los colores) con los estados del botón del kit:
//!
//! - **Hover:** el ícono se aclara: cada píxel, a la mitad del camino hacia el blanco.
//! - **Presionado:** el ícono baja 1 px, como lo de adentro de un botón presionado.

use eframe::egui::{Color32, Pos2, Rect, Response, Shape, Ui, Vec2, vec2};

use crate::kit::button::ButtonState;
use crate::pixel_ui;
use crate::skin::{Piece, Skin, WidgetState};

/// Lo que dibuja `piece` con la esquina en `at` en `state`.
pub fn icon_shapes(piece: &Piece, at: Pos2, state: ButtonState, ppp: f32) -> Vec<Shape> {
    let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
    match state {
        ButtonState::Hover => {
            let [w, h] = piece.size;
            (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter_map(|(x, y)| {
                    let c = piece.pixel(x, y);
                    (c.a() > 0).then(|| {
                        Shape::rect_filled(
                            Rect::from_min_size(at + vec2(x as f32, y as f32), Vec2::splat(1.0)),
                            0.0,
                            lighter(c),
                        )
                    })
                })
                .collect()
        }
        ButtonState::Pressed => {
            pixel_ui::piece_shapes(piece, Rect::from_min_size(at + vec2(0.0, 1.0), size), ppp)
        }
        _ => pixel_ui::piece_shapes(piece, Rect::from_min_size(at, size), ppp),
    }
}

/// `c` a la mitad del camino hacia el blanco (con su alfa).
pub fn lighter(c: Color32) -> Color32 {
    let up = |v: u8| v + (255 - v) / 2;
    Color32::from_rgba_unmultiplied(up(c.r()), up(c.g()), up(c.b()), c.a())
}

/// Dibuja el ícono `name` del skin con la esquina en `at`, en el estado de `response` (que es
/// quien lo hace tocable). Devuelve si estaba en el skin.
pub fn paint(ui: &Ui, skin: &Skin, name: &str, at: Pos2, response: &Response) -> bool {
    let Some(piece) = skin.piece(name, WidgetState::Normal) else {
        return false;
    };
    let state = ButtonState::of(response, ui.is_enabled());
    let ppp = ui.ctx().pixels_per_point();
    ui.painter().extend(icon_shapes(piece, at, state, ppp));
    true
}

#[cfg(test)]
mod tests;
