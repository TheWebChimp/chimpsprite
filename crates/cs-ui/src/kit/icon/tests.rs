use eframe::egui::pos2;

use super::*;
use crate::kit::button::tests::pixel_ctx;

fn bounds(shapes: &[Shape]) -> Rect {
    shapes
        .iter()
        .map(|s| s.visual_bounding_rect())
        .reduce(|a, b| a.union(b))
        .unwrap()
}

#[test]
fn normal_is_the_piece_and_pressed_goes_down_one_pixel() {
    let (_, skin) = pixel_ctx();
    let piece = skin.piece("fgbg.swap", WidgetState::Normal).unwrap();
    let at = pos2(10.0, 10.0);
    let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
    let normal = icon_shapes(piece, at, ButtonState::Normal, 1.0);
    assert_eq!(bounds(&normal), Rect::from_min_size(at, size));
    let pressed = icon_shapes(piece, at, ButtonState::Pressed, 1.0);
    assert_eq!(
        bounds(&pressed),
        Rect::from_min_size(at + vec2(0.0, 1.0), size)
    );
}

#[test]
fn hover_lightens_every_pixel_halfway_to_white() {
    let (_, skin) = pixel_ctx();
    let piece = skin.piece("preview.play", WidgetState::Normal).unwrap();
    let shapes = icon_shapes(piece, pos2(0.0, 0.0), ButtonState::Hover, 1.0);
    let [w, h] = piece.size;
    let visible = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| piece.pixel(x, y).a() > 0)
        .count();
    assert_eq!(shapes.len(), visible);
    for shape in &shapes {
        let Shape::Rect(r) = shape else {
            panic!("a pixel");
        };
        let (x, y) = (r.rect.min.x as u32, r.rect.min.y as u32);
        assert_eq!(r.fill, lighter(piece.pixel(x, y)));
        assert_ne!(r.fill, piece.pixel(x, y), "lighter");
    }
    assert_eq!(
        lighter(Color32::from_rgb(100, 54, 75)),
        Color32::from_rgb(177, 154, 165)
    );
}
