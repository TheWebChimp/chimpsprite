use eframe::egui::{Shape, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};
use crate::skin::WidgetState;

/// Lo dibujado: las piezas (su textura y su rect) y los rects lisos (su rect y su color).
#[derive(Default)]
struct Drawn {
    pieces: Vec<(TextureId, Rect)>,
    fills: Vec<(Rect, Color32)>,
}

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id == TextureId::default() => {
                let color = mesh.vertices[0].color;
                out.fills.push((mesh.calc_bounds(), color));
            }
            Shape::Mesh(mesh) => {
                let rect = mesh.calc_bounds();
                match out.pieces.last_mut() {
                    Some((t, r)) if *t == mesh.texture_id && r.intersects(rect) => {
                        *r = r.union(rect)
                    }
                    _ => out.pieces.push((mesh.texture_id, rect)),
                }
            }
            _ => {}
        }
    }
    let mut out = Drawn::default();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out
}

/// La pieza `name` dibujada en (20,20) en `state`.
fn slot(name: &str, state: ButtonState) -> (Rect, Drawn, Piece) {
    let (ctx, skin) = pixel_ctx();
    let piece = skin.piece(name, WidgetState::Normal).unwrap().clone();
    let rect = Rect::from_min_size(
        pos2(20.0, 20.0),
        vec2(piece.size[0] as f32, piece.size[1] as f32),
    );
    let shapes = frame(&ctx, vec![], |ui| {
        paint(ui.painter(), &piece, rect, state, 1.0);
    });
    (rect, drawn(&shapes), piece)
}

#[test]
fn normal_is_the_piece_alone() {
    let (rect, drawn, piece) = slot("tool.slot", ButtonState::Normal);
    assert_eq!(drawn.pieces, [(piece.texture.id(), rect)]);
    assert!(drawn.fills.is_empty());
}

#[test]
fn hover_swaps_the_face_and_the_light_like_the_kit_button() {
    let (rect, drawn, piece) = slot("tool.slot", ButtonState::Hover);
    assert_eq!(drawn.pieces, [(piece.texture.id(), rect)]);
    let face = piece.pixel(2, 2);
    let light = piece.pixel(1, 1);
    assert_eq!(light, Color32::WHITE);
    // 23×22: la luz de arriba (fila 1) y de la derecha (columna 21) toman la cara; la cara
    // (columnas 2 a 20, filas 2 a 20) se vuelve blanca. El borde de abajo (fila 21) se queda.
    let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Rect::from_min_max(rect.min + vec2(x0, y0), rect.min + vec2(x1, y1))
    };
    assert_eq!(
        drawn.fills,
        [
            (at(1.0, 1.0, 22.0, 2.0), face),
            (at(21.0, 1.0, 22.0, 21.0), face),
            (at(2.0, 2.0, 21.0, 21.0), light),
        ]
    );
}

#[test]
fn hover_without_a_face_fills_the_inside_of_the_outline_in_white() {
    let (rect, drawn, _) = slot("sym.slot", ButtonState::Hover);
    // sym.slot (15×16): el contorno en las columnas 1 y 13 y las filas 1 y 13.
    let inside = Rect::from_min_max(rect.min + vec2(2.0, 2.0), rect.min + vec2(13.0, 13.0));
    assert_eq!(drawn.fills, [(inside, Color32::WHITE)]);
}

#[test]
fn pressed_the_face_goes_down_one_pixel_and_the_bottom_border_stays() {
    let (rect, drawn, piece) = slot("tool.slot", ButtonState::Pressed);
    let id = piece.texture.id();
    // La pieza en su lugar (la fila de arriba y el borde de abajo) y, encima, la misma 1 px más
    // abajo (recortada entre la fila 1 y el borde de abajo): juntas, 1 px más altas.
    assert_eq!(
        drawn.pieces,
        [(id, Rect::from_min_max(rect.min, rect.max + vec2(0.0, 1.0)))]
    );
    assert_eq!(content_offset(ButtonState::Pressed), vec2(0.0, 1.0));
    assert_eq!(content_offset(ButtonState::Hover), Vec2::ZERO);
}

#[test]
fn pressed_without_a_face_the_whole_piece_goes_down() {
    let (rect, drawn, piece) = slot("sym.slot", ButtonState::Pressed);
    assert_eq!(
        drawn.pieces,
        [(piece.texture.id(), rect.translate(vec2(0.0, 1.0)))]
    );
}
