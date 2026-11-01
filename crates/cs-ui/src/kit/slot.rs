//! Los slots (paso 6 del UI kit): botones con una pieza propia (los tramos de la columna de
//! herramientas, `tool.slot*`, y los de la simetría, `sym.slot`) que toman los estados del botón
//! del kit sin piezas por estado:
//!
//! - **Hover**, como `button.hover`: la cara (el centro del 9-slice) en el color de la luz, y la
//!   fila de luz de arriba y la columna de luz de la derecha en el de la cara. Sin centro (el
//!   adentro transparente, como `sym.slot`), el adentro del contorno en blanco.
//! - **Presionado**, como `button.down`: la cara baja 1 px y el borde de abajo se queda; sin
//!   centro, la pieza entera baja 1 px. Lo de adentro baja con ella ([`content_offset`]).
//!
//! Lo elegido o prendido es un dato, no un estado del puntero: lo decide quien lo usa (el marco
//! rosa de la herramienta elegida, `sym.slot.active` del eje prendido).

use eframe::egui::{Color32, Painter, Pos2, Rect, Vec2, pos2, vec2};

use crate::kit::button::ButtonState;
use crate::pixel_ui;
use crate::skin::Piece;

/// Cuánto baja lo de adentro del slot (el ícono) en `state`: 1 px presionado.
pub fn content_offset(state: ButtonState) -> Vec2 {
    match state {
        ButtonState::Pressed => vec2(0.0, 1.0),
        _ => Vec2::ZERO,
    }
}

/// Dibuja `piece` en `rect` en `state` con `painter`.
pub fn paint(painter: &Painter, piece: &Piece, rect: Rect, state: ButtonState, ppp: f32) {
    let [w, h] = piece.size;
    match (state, piece.center) {
        (ButtonState::Hover, Some(c)) => {
            painter.extend(pixel_ui::piece_shapes(piece, rect, ppp));
            let face = piece.pixel(c.x, c.y);
            let light = piece.pixel(1, 1);
            let (right, bottom) = ((w - c.x - c.w) as f32, (h - c.y - c.h) as f32);
            let inner_bottom = rect.bottom() - bottom;
            fill(
                painter,
                Rect::from_min_max(
                    rect.min + vec2(1.0, 1.0),
                    pos2(rect.right() - 1.0, rect.top() + 2.0),
                ),
                face,
            );
            fill(
                painter,
                Rect::from_min_max(
                    pos2(rect.right() - right, rect.top() + 1.0),
                    pos2(rect.right() - right + 1.0, inner_bottom),
                ),
                face,
            );
            fill(
                painter,
                Rect::from_min_max(
                    rect.min + vec2(c.x as f32, c.y as f32),
                    pos2(rect.right() - right, inner_bottom),
                ),
                light,
            );
        }
        (ButtonState::Hover, None) => {
            if let Some(inside) = transparent_inside(piece) {
                let inside = inside.translate(rect.min.to_vec2());
                fill(painter, inside, Color32::WHITE);
            }
            painter.extend(pixel_ui::piece_shapes(piece, rect, ppp));
        }
        (ButtonState::Pressed, Some(c)) => {
            painter.extend(pixel_ui::piece_shapes(piece, rect, ppp));
            let bottom = (h - c.y - c.h) as f32;
            let face = Rect::from_min_max(
                pos2(rect.left(), rect.top() + 1.0),
                pos2(rect.right(), rect.bottom() - bottom),
            );
            painter
                .with_clip_rect(face.intersect(painter.clip_rect()))
                .extend(pixel_ui::piece_shapes(
                    piece,
                    rect.translate(vec2(0.0, 1.0)),
                    ppp,
                ));
        }
        (ButtonState::Pressed, None) => {
            painter.extend(pixel_ui::piece_shapes(
                piece,
                rect.translate(vec2(0.0, 1.0)),
                ppp,
            ));
        }
        _ => painter.extend(pixel_ui::piece_shapes(piece, rect, ppp)),
    }
}

/// Un rect liso en una malla, como el resto del pixel art.
fn fill(painter: &Painter, rect: Rect, color: Color32) {
    let mut mesh = eframe::egui::Mesh::default();
    mesh.add_colored_rect(rect, color);
    painter.add(mesh);
}

/// El adentro transparente de una pieza sin centro (un contorno, como `sym.slot`): desde la
/// mitad, lo transparente entre el contorno de cada lado. `None` si la mitad no cruza un contorno.
fn transparent_inside(piece: &Piece) -> Option<Rect> {
    let [w, h] = piece.size;
    let opaque = |x: u32, y: u32| piece.pixel(x, y).a() > 0;
    let (mx, my) = (w / 2, h / 2);
    if opaque(mx, my) {
        return None;
    }
    let left = (0..mx).rev().find(|&x| opaque(x, my))? + 1;
    let right = (mx..w).find(|&x| opaque(x, my))?;
    let top = (0..my).rev().find(|&y| opaque(mx, y))? + 1;
    let bottom = (my..h).find(|&y| opaque(mx, y))?;
    Some(Rect::from_min_max(
        Pos2::new(left as f32, top as f32),
        Pos2::new(right as f32, bottom as f32),
    ))
}

#[cfg(test)]
mod tests;
