use eframe::egui::{Event, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

fn pieces(shapes: &[Shape]) -> Vec<(TextureId, Rect)> {
    fn walk(shape: &Shape, out: &mut Vec<(TextureId, Rect)>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                let rect = mesh.calc_bounds();
                match out.last_mut() {
                    Some((t, r)) if *t == mesh.texture_id => *r = r.union(rect),
                    _ => out.push((mesh.texture_id, rect)),
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out
}

fn id(skin: &Skin, name: &str, state: WidgetState) -> TextureId {
    skin.piece(name, state).unwrap().texture.id()
}

fn click(at: egui::Pos2) -> [Vec<Event>; 2] {
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    [
        vec![Event::PointerMoved(at), press(true)],
        vec![press(false)],
    ]
}

#[test]
fn the_pieces_are_the_mockup_ones() {
    let (_, skin) = pixel_ctx();
    for name in [LEFT, RIGHT] {
        for state in [
            WidgetState::Normal,
            WidgetState::Hover,
            WidgetState::Down,
            WidgetState::Disabled,
        ] {
            assert_eq!(skin.piece(name, state).unwrap().size, [15, 17], "{name}");
        }
    }
    assert_eq!(
        skin.piece(FIELD, WidgetState::Normal).unwrap().size,
        [1, 17]
    );
    assert_eq!(skin.piece(UNIT, WidgetState::Normal).unwrap().size, [1, 17]);
    for icon in [MINUS, PLUS] {
        assert_eq!(skin.piece(icon, WidgetState::Normal).unwrap().size, [7, 7]);
    }
}

#[test]
fn the_buttons_step_the_value_and_disable_at_the_ends() {
    let (ctx, skin) = pixel_ctx();
    let run = |events: Vec<Event>, value: &mut i32| {
        let mut rect = Rect::NOTHING;
        let shapes = frame(&ctx, events, |ui| {
            rect = Rect::from_min_size(ui.cursor().min, vec2(70.0, HEIGHT));
            stepper(ui, &skin, 70.0, "s", value, 0..=255, 5, Some("ms"));
        });
        (shapes, rect)
    };
    let mut value = 250;
    let (shapes, rect) = run(vec![], &mut value);
    let drawn = pieces(&shapes);
    // El campo entre los botones, la caja de la unidad y los dos botones, de 15.
    assert!(drawn.contains(&(
        id(&skin, LEFT, WidgetState::Normal),
        Rect::from_min_size(rect.min, vec2(15.0, 17.0))
    )));
    assert!(drawn.contains(&(
        id(&skin, RIGHT, WidgetState::Normal),
        Rect::from_min_size(pos2(rect.right() - 15.0, rect.top()), vec2(15.0, 17.0))
    )));
    let plus = pos2(rect.right() - 7.0, rect.center().y);
    for events in click(plus) {
        run(events, &mut value);
    }
    assert_eq!(value, 255);
    // En el máximo, el + queda deshabilitado (y no sube más).
    let (shapes, _) = run(vec![], &mut value);
    assert!(
        pieces(&shapes)
            .iter()
            .any(|(t, _)| *t == id(&skin, RIGHT, WidgetState::Disabled))
    );
    for events in click(plus) {
        run(events, &mut value);
    }
    assert_eq!(value, 255);
    let minus = pos2(rect.left() + 7.0, rect.center().y);
    for events in click(minus) {
        run(events, &mut value);
    }
    assert_eq!(value, 250);
}
