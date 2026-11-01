use eframe::egui::{Event, Pos2, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Las piezas dibujadas con su rect (las partes del 9-slice, juntas) y los textos.
type Drawn = (Vec<(TextureId, Rect)>, Vec<(String, Pos2)>);

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                let rect = mesh.calc_bounds();
                match out.0.last_mut() {
                    Some((t, r)) if *t == mesh.texture_id => *r = r.union(rect),
                    _ => out.0.push((mesh.texture_id, rect)),
                }
            }
            Shape::Text(t) => out.1.push((t.galley.text().to_owned(), t.pos)),
            _ => {}
        }
    }
    let mut out = Default::default();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out
}

fn id(skin: &Skin, name: &str, state: WidgetState) -> TextureId {
    skin.piece(name, state).unwrap().texture.id()
}

#[test]
fn the_pieces_are_the_mockup_ones() {
    let (_, skin) = pixel_ctx();
    let size = |n: &str| skin.piece(n, WidgetState::Normal).unwrap().size;
    assert_eq!(size(OPEN), [6, 16]);
    assert_eq!(size(LIST), [8, 13]);
    assert_eq!(size(ITEM_HOVER), [3, 15]);
    assert_eq!(size(ARROW), [5, 3]);
    let list = skin.piece(LIST, WidgetState::Normal).unwrap();
    assert_eq!(
        list.pixel(0, 0),
        Color32::from_rgb(5, 4, 3),
        "the black border"
    );
    // La sombra: el negro (5,4,3) con alfa 123, como lo marca el mockup.
    let data = crate::skin::decode(crate::skin::EMBEDDED).unwrap();
    let raw = &data.pieces[LIST];
    let i = ((8 * raw.size[0] + 7) * 4) as usize;
    assert_eq!(raw.rgba[i..i + 4], [5, 4, 3, 123]);
    assert_eq!(list.size, raw.size);
}

#[test]
fn closed_it_is_the_field_with_the_text_and_the_arrow() {
    let (ctx, skin) = pixel_ctx();
    let mut rect = Rect::NOTHING;
    let mut natural = 0.0;
    let shapes = frame(&ctx, vec![], |ui| {
        natural = width(ui, "Select an option…");
        rect = select(ui, &skin, "s", "Select an option…", Some(109.0), |_| {}).rect;
    });
    assert_eq!(rect.size(), vec2(109.0, HEIGHT));
    assert!(natural <= 109.0, "{natural}");
    let (pieces, texts) = drawn(&shapes);
    assert_eq!(
        pieces[0],
        (id(&skin, input::FIELD, WidgetState::Normal), rect)
    );
    assert_eq!(
        texts[0].0,
        format!("Select an option{}", tab_bar::ellipsis())
    );
    // La flecha: 15 puntos (5 + 3 + 1) en las filas 7 a 9, 4 px antes del borde derecho.
    let dots: Vec<Rect> = shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if r.rect.size() == Vec2::splat(1.0) => Some(r.rect),
            _ => None,
        })
        .collect();
    assert_eq!(dots.len(), 9);
    let right = dots.iter().map(|r| r.right()).fold(f32::MIN, f32::max);
    let top = dots.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
    assert_eq!(right, rect.right() - ARROW_RIGHT);
    assert_eq!(top, rect.top() + ARROW_TOP);
}

#[test]
fn a_click_opens_the_list_below_and_picking_an_option_closes_it() {
    let (ctx, skin) = pixel_ctx();
    let mut pick = 0usize;
    let options = ["Nearest", "Bilinear", "RotSprite"];
    let run = |events: Vec<Event>, pick: &mut usize| {
        let mut rect = Rect::NOTHING;
        let shapes = frame(&ctx, events, |ui| {
            rect = select(ui, &skin, "s", options[*pick], Some(109.0), |ui| {
                for (i, label) in options.iter().enumerate() {
                    crate::widgets::choice(ui, pick, i, label);
                }
            })
            .rect;
        });
        (shapes, rect)
    };
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (_, rect) = run(vec![], &mut pick);
    let at = rect.center();
    run(vec![Event::PointerMoved(at), press(at, true)], &mut pick);
    run(vec![press(at, false)], &mut pick);
    run(vec![], &mut pick);
    let (shapes, _) = run(vec![], &mut pick);
    let (pieces, texts) = drawn(&shapes);
    // El panel desde la fila 15 del campo, 11 px más ancho (más la sombra).
    let list = pieces
        .iter()
        .find(|(t, _)| *t == id(&skin, LIST, WidgetState::Normal))
        .expect("the list")
        .1;
    assert_eq!(list.min, rect.min + vec2(0.0, LIST_TOP));
    assert_eq!(list.width(), 109.0 + LIST_EXTRA + SHADOW);
    assert_eq!(
        list.height(),
        LIST_PAD_TOP + 3.0 * ITEM_HEIGHT + LIST_PAD_BOTTOM + SHADOW
    );
    // Abierto, el campo es `select.open`, encima del panel.
    assert!(
        pieces
            .iter()
            .any(|(t, _)| *t == id(&skin, OPEN, WidgetState::Normal))
    );
    // Sin el puntero en el panel, la franja va en la elegida (la primera).
    let hover = pieces
        .iter()
        .find(|(t, _)| *t == id(&skin, ITEM_HOVER, WidgetState::Normal))
        .expect("the highlight")
        .1;
    let first = Rect::from_min_size(
        list.min + vec2(LIST_SIDE, LIST_PAD_TOP),
        vec2(109.0 + LIST_EXTRA - 2.0 * LIST_SIDE, ITEM_HEIGHT),
    );
    assert_eq!(hover, first);
    assert!(texts.iter().any(|(t, _)| t == "RotSprite"));
    // Elegir la tercera.
    let third = first.center() + vec2(0.0, 2.0 * ITEM_HEIGHT);
    run(vec![Event::PointerMoved(third)], &mut pick);
    run(vec![press(third, true)], &mut pick);
    run(vec![press(third, false)], &mut pick);
    let (shapes, _) = run(vec![], &mut pick);
    assert_eq!(pick, 2);
    let (pieces, _) = drawn(&shapes);
    assert!(
        !pieces
            .iter()
            .any(|(t, _)| *t == id(&skin, LIST, WidgetState::Normal)),
        "closed"
    );
}

/// Un select con `n` opciones en `at` (en una pantalla de 400×300), abierto. Devuelve lo dibujado
/// del último frame y el rect del campo.
fn open_long(at: Pos2, n: usize) -> (Skin, Vec<Shape>, Rect) {
    let (ctx, skin) = pixel_ctx();
    // Con Pixel UI, como en la app: el scroll del skin.
    crate::pixel_ui::set_bubble(&ctx, Some(std::sync::Arc::new(skin.clone())));
    let labels: Vec<String> = (0..n).map(|i| format!("Option {i}")).collect();
    let mut pick = 0usize;
    let mut run = |events: Vec<Event>| {
        let mut rect = Rect::NOTHING;
        let shapes = frame(&ctx, events, |ui| {
            let slot = Rect::from_min_size(at, vec2(109.0, HEIGHT));
            ui.scope_builder(UiBuilder::new().max_rect(slot), |ui| {
                rect = select(ui, &skin, "s", &labels[pick], Some(109.0), |ui| {
                    for (i, label) in labels.iter().enumerate() {
                        crate::widgets::choice(ui, &mut pick, i, label);
                    }
                })
                .rect;
            });
        });
        (shapes, rect)
    };
    let (_, rect) = run(vec![]);
    let c = rect.center();
    let press = |pressed| Event::PointerButton {
        pos: c,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(vec![Event::PointerMoved(c), press(true)]);
    run(vec![press(false)]);
    for _ in 0..3 {
        run(vec![Event::PointerMoved(pos2(399.0, 299.0))]);
    }
    let (shapes, rect) = run(vec![]);
    (skin, shapes, rect)
}

#[test]
fn a_long_list_below_the_field_scrolls_inside_the_window() {
    let (skin, shapes, rect) = open_long(pos2(10.0, 10.0), 40);
    let (pieces, _) = drawn(&shapes);
    let list = pieces
        .iter()
        .find(|(t, _)| *t == id(&skin, LIST, WidgetState::Normal))
        .expect("the list below")
        .1;
    assert_eq!(list.min, rect.min + vec2(0.0, LIST_TOP));
    assert!(list.bottom() <= 300.0, "{list:?}");
    // La barra de scroll.
    assert!(
        pieces
            .iter()
            .any(|(t, _)| *t == id(&skin, "scroll.thumb.v", WidgetState::Normal))
    );
}

#[test]
fn without_room_below_the_list_opens_above_with_its_round_corner() {
    let (skin, shapes, rect) = open_long(pos2(10.0, 250.0), 12);
    let (pieces, _) = drawn(&shapes);
    let list = pieces
        .iter()
        .find(|(t, _)| *t == id(&skin, LIST_ALONE, WidgetState::Normal))
        .expect("the list above")
        .1;
    assert_eq!(list.bottom(), rect.top());
    assert!(list.top() >= 0.0);
    // El campo queda cerrado (sin `select.open`).
    assert!(
        !pieces
            .iter()
            .any(|(t, _)| *t == id(&skin, OPEN, WidgetState::Normal))
    );
}
