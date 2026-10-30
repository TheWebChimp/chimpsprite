use eframe::egui::{Event, Key, Modifiers, Pos2, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Lo dibujado: las imágenes (textura y rect en pantalla), los rects de un color y los textos
/// (posición, color de la forma, galería).
#[derive(Default)]
struct Drawn {
    images: Vec<(TextureId, Rect)>,
    fills: Vec<(Rect, Color32)>,
    texts: Vec<(Pos2, Color32, Arc<Galley>)>,
}

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                let rect = mesh.vertices.iter().fold(Rect::NOTHING, |r, v| {
                    r.union(Rect::from_min_max(v.pos, v.pos))
                });
                out.images.push((mesh.texture_id, rect));
            }
            Shape::Rect(r) => out.fills.push((r.rect, r.fill)),
            Shape::Text(text) => {
                out.texts
                    .push((text.pos, text.fallback_color, text.galley.clone()));
            }
            _ => {}
        }
    }
    let mut out = Drawn::default();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out
}

impl Drawn {
    /// La caja de la pieza `name` (`state`): las partes del 9-slice juntas.
    fn piece(&self, skin: &Skin, name: &str, state: WidgetState) -> Option<Rect> {
        let id = skin.piece(name, state).unwrap().texture.id();
        self.images
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(b))
    }
}

/// La columna donde empieza la tinta de un texto y la fila de arriba de sus mayúsculas.
fn ink_at(at: Pos2, galley: &Galley) -> (f32, f32) {
    let row = &galley.rows[0];
    let glyph = &row.glyphs[0];
    (
        at.x + glyph.pos.x + glyph.uv_rect.offset.x,
        at.y + row.pos.y + glyph.pos.y - tab_bar::CAP_HEIGHT,
    )
}

const AT: Pos2 = pos2(20.0, 30.0);

fn rect(width: f32) -> Rect {
    Rect::from_min_size(AT, vec2(width, HEIGHT))
}

#[test]
fn the_field_is_17_high_with_the_text_5_px_in_and_its_caps_on_row_5() {
    let (ctx, skin) = pixel_ctx();
    let colors = *skin.colors();
    for (state, piece, color) in [
        (InputState::Normal, WidgetState::Normal, colors.field_text),
        (InputState::Focused, WidgetState::Normal, colors.text),
        (
            InputState::Disabled,
            WidgetState::Disabled,
            colors.field_text_disabled,
        ),
    ] {
        let shapes = frame(&ctx, vec![], |ui| {
            let shapes = input_shapes(
                ui,
                &skin,
                rect(109.0),
                "Input Placeholder",
                "",
                state,
                None,
                Affixes::NONE,
            );
            ui.painter().extend(shapes);
        });
        let d = drawn(&shapes);
        assert_eq!(d.piece(&skin, FIELD, piece), Some(rect(109.0)), "{state:?}");
        let (at, c, galley) = d.texts.last().unwrap();
        assert_eq!(*c, color, "{state:?}");
        // Como en el mockup: la tinta en x 330 de un campo que empieza en 325, las mayúsculas
        // en la fila 5 (y 143 de 138).
        assert_eq!(ink_at(*at, galley), (AT.x + 5.0, AT.y + 5.0), "{state:?}");
    }
    // Los colores del mockup.
    assert_eq!(colors.field_text, Color32::from_rgb(0x59, 0x4d, 0x4d));
    assert_eq!(
        colors.field_text_disabled,
        Color32::from_rgb(0xbd, 0xa4, 0x99)
    );
    assert_eq!(colors.text_selection, Color32::from_rgb(0xe2, 0x72, 0x85));
    assert_eq!(colors.text_selected, Color32::WHITE);
}

#[test]
fn the_selection_is_a_pink_block_of_rows_3_to_13_from_2_px_before_the_ink_with_white_text() {
    let (ctx, skin) = pixel_ctx();
    let colors = *skin.colors();
    let text = "Input Placeholder";
    let mut ink = 0.0;
    let shapes = frame(&ctx, vec![], |ui| {
        ink = super::ink_width(ui, text);
        let shapes = input_shapes(
            ui,
            &skin,
            rect(109.0),
            text,
            "",
            InputState::Focused,
            Some(0..text.chars().count()),
            Affixes::NONE,
        );
        ui.painter().extend(shapes);
    });
    let d = drawn(&shapes);
    let blocks: Vec<Rect> = d
        .fills
        .iter()
        .filter(|(_, c)| *c == colors.text_selection)
        .map(|(r, _)| *r)
        .collect();
    // En el mockup (y 114): el bloque de la x 328 a la 406 y de la fila 3 a la 13; la tinta, de
    // la 330 a la 405.
    let left = AT.x + 5.0;
    assert_eq!(
        blocks,
        vec![Rect::from_min_max(
            pos2(left - 2.0, AT.y + 3.0),
            pos2(left + ink + 1.0, AT.y + 14.0)
        )]
    );
    // El texto elegido, blanco (en la galería, que manda sobre el color de la forma).
    let (_, _, galley) = d.texts.last().unwrap();
    let colors_of: Vec<Color32> = galley
        .job
        .sections
        .iter()
        .filter(|s| !s.byte_range.is_empty())
        .map(|s| s.format.color)
        .collect();
    assert_eq!(colors_of, vec![colors.text_selected]);
}

#[test]
fn the_unit_and_prefix_boxes_are_3_px_around_their_ink_against_the_outline() {
    let (ctx, skin) = pixel_ctx();
    let colors = *skin.colors();
    let mut widths = (0.0, 0.0);
    let mut area = Rect::NOTHING;
    let shapes = frame(&ctx, vec![], |ui| {
        widths = (box_width(ui, "px"), box_width(ui, "X"));
        area = text_area(ui, rect(42.0), Affixes::suffix("px"));
        let mut shapes = frame_shapes(
            ui,
            &skin,
            rect(42.0),
            InputState::Normal,
            Affixes::suffix("px"),
        );
        let below = Rect::from_min_size(AT + vec2(0.0, 30.0), vec2(42.0, HEIGHT));
        shapes.extend(frame_shapes(
            ui,
            &skin,
            below,
            InputState::Normal,
            Affixes::prefix("X"),
        ));
        ui.painter().extend(shapes);
    });
    // El mockup: la caja del "px", 16 de ancho (x 388..403) y la de la "X", 11 (x 364..374).
    assert_eq!(widths, (16.0, 11.0));
    assert_eq!(
        area.x_range(),
        egui::Rangef::new(AT.x + 1.0, AT.x + 42.0 - 17.0)
    );
    let d = drawn(&shapes);
    // Las piezas de las cajas traen el borde de su lado.
    assert_eq!(
        d.piece(&skin, UNIT, WidgetState::Normal),
        Some(Rect::from_min_size(
            pos2(AT.x + 42.0 - 17.0, AT.y),
            vec2(17.0, HEIGHT)
        ))
    );
    assert_eq!(
        d.piece(&skin, PREFIX, WidgetState::Normal),
        Some(Rect::from_min_size(
            AT + vec2(0.0, 30.0),
            vec2(12.0, HEIGHT)
        ))
    );
    // Su texto centrado en la caja, en la fila de las mayúsculas, en `field_text`.
    let inks: Vec<(f32, f32)> = d
        .texts
        .iter()
        .inspect(|t| assert_eq!(t.1, colors.field_text))
        .map(|(at, _, g)| ink_at(*at, g))
        .collect();
    assert_eq!(inks[0].0, AT.x + 42.0 - 17.0 + 3.0, "px");
    // La X de la fuente mide 4: centrada en la caja de 11 (el mínimo), 3 px adentro.
    assert_eq!(inks[1], (AT.x + 1.0 + 3.0, AT.y + 30.0 + 5.0), "X");
}

#[test]
fn a_real_field_takes_typing_and_shows_focus_and_disabled() {
    let (ctx, skin) = pixel_ctx();
    let mut text = String::from("ab");
    let run = |events: Vec<Event>, enabled: bool, text: &mut String| {
        let mut field = Rect::NOTHING;
        let shapes = frame(&ctx, events, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                field = text_input(ui, &skin, text, 80.0, "Name", "Hint").rect;
            });
        });
        (shapes, field)
    };
    let (shapes, field) = run(vec![], true, &mut text);
    let d = drawn(&shapes);
    assert_eq!(d.texts.last().unwrap().1, skin.colors().field_text);
    let at = field.center();
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run(vec![Event::PointerMoved(at), click(true)], true, &mut text);
    run(vec![click(false)], true, &mut text);
    run(
        vec![
            Event::Key {
                key: Key::End,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Text("c".into()),
        ],
        true,
        &mut text,
    );
    assert_eq!(text, "abc");
    let (shapes, _) = run(vec![], true, &mut text);
    let d = drawn(&shapes);
    assert_eq!(d.texts.last().unwrap().1, skin.colors().text, "focused");
    // El cursor de escribir, en las filas de la selección.
    let caret = d
        .fills
        .iter()
        .find(|(r, c)| *c == skin.colors().outline && r.width() == 1.0);
    if let Some((r, _)) = caret {
        assert_eq!(r.y_range(), selection_rows(field.top()));
    }
    let (shapes, _) = run(vec![], false, &mut text);
    let d = drawn(&shapes);
    assert!(d.piece(&skin, FIELD, WidgetState::Disabled).is_some());
    assert_eq!(d.texts.last().unwrap().1, skin.colors().field_text_disabled);
}

#[test]
fn a_number_field_drags_and_draws_its_unit_box() {
    let (ctx, skin) = pixel_ctx();
    let mut value = 10;
    let mut field = Rect::NOTHING;
    let mut run = |events: Vec<Event>, value: &mut i32| {
        frame(&ctx, events, |ui| {
            let options = NumberOptions {
                affixes: Affixes::suffix("px"),
                ..Default::default()
            };
            let r = number_input(ui, &skin, 42.0, "n", value, 0..=100, options);
            field = r.rect;
        })
    };
    let shapes = run(vec![], &mut value);
    let d = drawn(&shapes);
    assert!(d.piece(&skin, UNIT, WidgetState::Normal).is_some());
    let whole = d.piece(&skin, FIELD, WidgetState::Normal).unwrap();
    assert_eq!(whole.size(), vec2(42.0, HEIGHT));
    // El número, en la fila 5 y 5 px adentro.
    let (at, color, galley) = d.texts.iter().find(|t| t.2.text() == "10").unwrap();
    assert_eq!(*color, skin.colors().field_text);
    assert_eq!(ink_at(*at, galley), (whole.left() + 5.0, whole.top() + 5.0));
    // Arrastrar a la derecha lo agranda.
    let from = whole.center();
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    run(
        vec![Event::PointerMoved(from), press(from, true)],
        &mut value,
    );
    for step in 1..=3 {
        run(
            vec![Event::PointerMoved(from + vec2(10.0 * step as f32, 0.0))],
            &mut value,
        );
    }
    run(vec![press(from + vec2(30.0, 0.0), false)], &mut value);
    assert!(value > 10, "{value}");
    let _ = field;
}
