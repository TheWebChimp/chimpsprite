use eframe::egui::{Event, Pos2, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Las piezas dibujadas, con su rect (las partes del 9-slice, juntas).
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

fn pointer(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

#[test]
fn the_slider_is_a_button_high_with_the_thumb_over_the_track_as_in_the_mockup() {
    let (ctx, skin) = pixel_ctx();
    assert_eq!(
        skin.piece(TRACK, WidgetState::Normal).unwrap().size,
        [16, 9]
    );
    assert_eq!(
        skin.piece(THUMB, WidgetState::Normal).unwrap().size,
        [13, 14]
    );
    let mut value = 0;
    let mut rect = Rect::NOTHING;
    let shapes = frame(&ctx, vec![], |ui| {
        rect = slider(ui, &skin, 140.0, "s", &mut value, 0..=100).rect;
    });
    assert_eq!(rect.size(), vec2(140.0, HEIGHT));
    let drawn = pieces(&shapes);
    let track = Rect::from_min_size(rect.min + vec2(0.0, 4.0), vec2(140.0, 9.0));
    let thumb = Rect::from_min_size(rect.min + vec2(0.0, 1.0), vec2(13.0, 14.0));
    assert_eq!(
        drawn,
        vec![
            (id(&skin, TRACK, WidgetState::Normal), track),
            (id(&skin, THUMB, WidgetState::Normal), thumb),
        ]
    );
    // En el máximo, la perilla contra el borde derecho.
    value = 100;
    let shapes = frame(&ctx, vec![], |ui| {
        slider(ui, &skin, 140.0, "s", &mut value, 0..=100);
    });
    assert_eq!(pieces(&shapes)[1].1.right(), rect.right());
}

#[test]
fn a_click_on_the_track_moves_the_thumb_there_and_dragging_follows_the_pointer() {
    let (ctx, skin) = pixel_ctx();
    let mut value = 0;
    let run = |events: Vec<Event>, value: &mut i32| {
        let mut rect = Rect::NOTHING;
        let shapes = frame(&ctx, events, |ui| {
            rect = slider(ui, &skin, 113.0, "s", value, 0..=100).rect;
        });
        (shapes, rect)
    };
    let (_, rect) = run(vec![], &mut value);
    // El centro de la perilla, a la mitad de su recorrido (100 de 113 - 13).
    let middle = pos2(rect.left() + 6.5 + 50.0, rect.center().y);
    run(
        vec![Event::PointerMoved(middle), pointer(middle, true)],
        &mut value,
    );
    assert_eq!(value, 50);
    let (shapes, _) = run(
        vec![Event::PointerMoved(middle + vec2(25.0, 0.0))],
        &mut value,
    );
    assert_eq!(value, 75);
    assert_eq!(
        pieces(&shapes)[1].0,
        id(&skin, THUMB, WidgetState::Hover),
        "dragging"
    );
    let far = middle + vec2(500.0, 0.0);
    run(vec![Event::PointerMoved(far)], &mut value);
    run(vec![pointer(far, false)], &mut value);
    assert_eq!(value, 100, "clamped");
}

#[test]
fn disabled_it_uses_its_pieces_and_does_not_move() {
    let (ctx, skin) = pixel_ctx();
    let mut value = 10;
    let mut shapes = vec![];
    for events in [
        vec![],
        vec![
            Event::PointerMoved(pos2(60.0, 8.0)),
            pointer(pos2(60.0, 8.0), true),
        ],
        vec![pointer(pos2(60.0, 8.0), false)],
    ] {
        shapes = frame(&ctx, events, |ui| {
            ui.add_enabled_ui(false, |ui| {
                slider(ui, &skin, 100.0, "s", &mut value, 0..=100);
            });
        });
    }
    assert_eq!(value, 10);
    let drawn: Vec<TextureId> = pieces(&shapes).into_iter().map(|(t, _)| t).collect();
    assert_eq!(
        drawn,
        vec![
            id(&skin, TRACK, WidgetState::Disabled),
            id(&skin, THUMB, WidgetState::Disabled),
        ]
    );
}
