use eframe::egui::{Event, Pos2, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

fn textures(shapes: &[Shape]) -> Vec<TextureId> {
    fn walk(shape: &Shape, out: &mut Vec<TextureId>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                out.push(mesh.texture_id)
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out.dedup();
    out
}

fn texts(shapes: &[Shape]) -> Vec<(Pos2, Color32, std::sync::Arc<egui::Galley>)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Text(t) => Some((t.pos, t.fallback_color, t.galley.clone())),
            _ => None,
        })
        .collect()
}

fn texture(skin: &Skin, name: &str, state: WidgetState) -> TextureId {
    skin.piece(name, state).unwrap().texture.id()
}

fn click(pos: Pos2) -> [Vec<Event>; 2] {
    let button = |pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    [
        vec![Event::PointerMoved(pos), button(true)],
        vec![button(false)],
    ]
}

#[test]
fn the_skin_has_every_piece_with_the_mockup_sizes() {
    let (_, skin) = pixel_ctx();
    for check in Check::ALL {
        let name = Mark::Check(check).piece();
        for state in [
            WidgetState::Normal,
            WidgetState::Hover,
            WidgetState::Disabled,
        ] {
            let suffixed = match state {
                WidgetState::Normal => name.to_owned(),
                WidgetState::Hover => format!("{name}.hover"),
                _ => format!("{name}.disabled"),
            };
            let piece = skin.piece(&suffixed, WidgetState::Normal).unwrap();
            assert_eq!(piece.size, [12, 13], "{suffixed}");
        }
    }
    for on in [false, true] {
        let name = Mark::Radio(on).piece();
        for suffix in ["", ".hover", ".disabled"] {
            let piece = skin.piece(&format!("{name}{suffix}"), WidgetState::Normal);
            assert_eq!(piece.unwrap().size, [10, 11], "{name}{suffix}");
        }
    }
}

#[test]
fn the_checkbox_toggles_and_its_text_sits_on_the_button_caps_row() {
    let (ctx, skin) = pixel_ctx();
    let mut checked = false;
    let mut rect = Rect::NOTHING;
    let run = |events, checked: &mut bool, rect: &mut Rect| {
        frame(&ctx, events, |ui| {
            *rect = checkbox(ui, &skin, checked, "Snap").rect;
        })
    };
    let shapes = run(vec![], &mut checked, &mut rect);
    assert_eq!(rect.height(), HEIGHT);
    assert_eq!(
        textures(&shapes),
        vec![texture(&skin, "check", WidgetState::Normal)]
    );
    let (at, color, galley) = texts(&shapes).pop().unwrap();
    assert_eq!(color, skin.colors().text);
    let row = &galley.rows[0];
    let glyph = &row.glyphs[0];
    // La tinta, 3 px después de la casilla; las mayúsculas, en la fila 4.
    assert_eq!(
        at.x + glyph.pos.x + glyph.uv_rect.offset.x,
        rect.left() + 12.0 + GAP
    );
    assert_eq!(
        at.y + row.pos.y + glyph.pos.y - tab_bar::CAP_HEIGHT,
        rect.top() + CAPS_TOP
    );
    for events in click(rect.center()) {
        run(events, &mut checked, &mut rect);
    }
    assert!(checked);
    let shapes = run(vec![], &mut checked, &mut rect);
    assert_eq!(
        textures(&shapes),
        vec![texture(&skin, "check.on", WidgetState::Hover)],
        "on, with the pointer over it"
    );
}

#[test]
fn a_disabled_checkbox_uses_its_piece_and_does_not_toggle() {
    let (ctx, skin) = pixel_ctx();
    let mut checked = true;
    let mut rect = Rect::NOTHING;
    let mut shapes = vec![];
    for events in [vec![]].into_iter().chain(click(pos2(5.0, 5.0))) {
        shapes = frame(&ctx, events, |ui| {
            ui.add_enabled_ui(false, |ui| {
                rect = checkbox(ui, &skin, &mut checked, "Snap").rect;
            });
        });
    }
    assert!(checked);
    assert_eq!(
        textures(&shapes),
        vec![texture(&skin, "check.on", WidgetState::Disabled)]
    );
    let (_, color, _) = texts(&shapes).pop().unwrap();
    assert_eq!(color, skin.colors().button_text_disabled);
}

#[test]
fn the_radio_picks_its_value_and_its_dot_sits_on_the_caps() {
    let (ctx, skin) = pixel_ctx();
    let mut current = 0;
    let mut rects = [Rect::NOTHING; 2];
    let run = |events, current: &mut i32, rects: &mut [Rect; 2]| {
        frame(&ctx, events, |ui| {
            ui.horizontal(|ui| {
                rects[0] = radio_value(ui, &skin, current, 0, "Layer").rect;
                rects[1] = radio_value(ui, &skin, current, 1, "All").rect;
            });
        })
    };
    run(vec![], &mut current, &mut rects);
    for events in click(rects[1].center()) {
        run(events, &mut current, &mut rects);
    }
    assert_eq!(current, 1);
    // El punto del radio (filas 3 a 6 de la pieza) cae en las mayúsculas (filas 4 a 9).
    assert_eq!(Mark::Radio(true).top() + 3.0, CAPS_TOP + 1.0);
    assert_eq!(Mark::Check(Check::On).top(), 1.0);
}
