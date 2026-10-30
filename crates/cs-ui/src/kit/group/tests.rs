use eframe::egui::{Color32, Event, Galley, TextureId};
use std::sync::Arc;

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Lo dibujado: las imágenes (textura, rect en pantalla y esquina de arriba de la fuente) y los
/// textos (posición, color y galería), en orden.
#[derive(Default)]
struct Drawn {
    images: Vec<(TextureId, Rect, Pos2)>,
    texts: Vec<(Pos2, Color32, Arc<Galley>)>,
}

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
                let rect = mesh.vertices.iter().fold(Rect::NOTHING, |r, v| {
                    r.union(Rect::from_min_max(v.pos, v.pos))
                });
                let uv = mesh
                    .vertices
                    .iter()
                    .fold(pos2(1.0, 1.0), |m, v| m.min(v.uv));
                out.images.push((mesh.texture_id, rect, uv));
            }
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
    /// Los rects de la pieza `name` (`state`) que se dibujaron: las partes de un 9-slice van
    /// seguidas y la primera es la esquina de arriba a la izquierda (o la pieza entera).
    fn boxes(&self, skin: &Skin, name: &str, state: WidgetState) -> Vec<Rect> {
        let id = skin.piece(name, state).unwrap().texture.id();
        let mut out: Vec<Rect> = Vec::new();
        let mut open = false;
        for (tex, rect, uv) in &self.images {
            if *tex != id {
                open = false;
                continue;
            }
            match out.last_mut() {
                Some(last) if open && *uv != Pos2::ZERO && name != SEP => *last = last.union(*rect),
                _ => out.push(*rect),
            }
            open = true;
        }
        out
    }
}

/// La columna donde empieza la tinta y la fila de arriba de las mayúsculas del texto `i`.
fn ink_at(text: &(Pos2, Color32, Arc<Galley>)) -> (f32, f32) {
    let (at, _, galley) = text;
    let row = &galley.rows[0];
    let glyph = &row.glyphs[0];
    (
        at.x + glyph.pos.x + glyph.uv_rect.offset.x,
        at.y + row.pos.y + glyph.pos.y - crate::tab_bar::CAP_HEIGHT,
    )
}

const AT: Pos2 = pos2(20.0, 30.0);

fn looks(on: &[bool]) -> Vec<SegmentLook> {
    on.iter()
        .map(|&on| SegmentLook {
            state: ButtonState::Normal,
            on,
        })
        .collect()
}

#[test]
fn the_group_is_17_high_with_one_separator_column_between_segments_like_the_mockup() {
    let (ctx, skin) = pixel_ctx();
    let items = [GroupItem::text("Button"), GroupItem::text("Button")];
    let dots = [GroupItem::with_dot("Button"), GroupItem::with_dot("Button")];
    let mut out = (0.0, 0.0, 0.0, vec![], vec![]);
    let shapes = frame(&ctx, vec![], |ui| {
        out = (
            segment_width(ui, &items[0]),
            width(ui, &items),
            width(ui, &dots),
            segment_rects(ui, &items, AT),
            segment_rects(ui, &dots, AT),
        );
        ui.painter()
            .extend(group_shapes(ui, &skin, AT, &items, &looks(&[false, false])));
    });
    let (segment, plain, with_dots, rects, dot_rects) = out;
    // El mockup: "Button" (27 de tinta) en segmentos de 1 + 4 + 27 + 4 + 1; el grupo, x 210..286
    // (77) sin puntos y x 309..403 (95) con puntos de 6 y 3 de aire.
    assert_eq!(segment, 37.0);
    assert_eq!(plain, 77.0);
    assert_eq!(with_dots, 95.0);
    assert_eq!(
        rects[0],
        Rect::from_min_size(AT, vec2(39.0, button::HEIGHT))
    );
    assert_eq!(button::HEIGHT, 17.0);
    // Los vecinos comparten la columna del separador (la x 248 del mockup, 38 después del borde).
    assert_eq!(rects[1].left(), rects[0].right() - 1.0);
    assert_eq!(rects[1].left(), AT.x + 38.0);
    assert_eq!(rects[1].right(), AT.x + plain);
    assert_eq!(dot_rects[1].left(), AT.x + 47.0, "x 356");
    let d = drawn(&shapes);
    // Las caras: la pieza `button` en cada segmento; el separador, una columna de 17.
    assert_eq!(
        d.boxes(&skin, "button", WidgetState::Normal),
        rects,
        "one face per segment"
    );
    let seps = d.boxes(&skin, SEP, WidgetState::Normal);
    assert_eq!(
        seps,
        vec![Rect::from_min_size(
            pos2(rects[1].left(), AT.y),
            vec2(1.0, 17.0)
        )]
    );
    // Los textos: la tinta 1 + 1 + 4 adentro y las mayúsculas en la fila 4, como el botón.
    let inks: Vec<(f32, f32)> = d
        .texts
        .iter()
        .filter(|t| t.1 == skin.colors().text)
        .map(ink_at)
        .collect();
    assert_eq!(
        inks,
        vec![(AT.x + 6.0, AT.y + 4.0), (AT.x + 44.0, AT.y + 4.0)]
    );
}

#[test]
fn the_separator_piece_has_the_mockup_column() {
    let (_, skin) = pixel_ctx();
    let piece = skin.piece(SEP, WidgetState::Normal).unwrap();
    assert_eq!(piece.size, [1, 17]);
    let px = |y: usize| -> [u8; 4] { piece.rgba[y * 4..y * 4 + 4].try_into().unwrap() };
    let outline = [0x59, 0x4d, 0x4d, 255];
    assert_eq!(px(0), outline);
    for y in 1..=13 {
        assert_eq!(px(y), [0xbd, 0xa4, 0x99, 255], "row {y}");
    }
    // En la fila del bisel, el contorno (no el bisel `#886e6a`); después, el de abajo y la luz.
    assert_eq!(px(14), outline);
    assert_eq!(px(15), outline);
    assert_eq!(px(16), [0xf1, 0xeb, 0xdb, 255]);
    for name in [DOT_ON, DOT_OFF] {
        assert_eq!(skin.piece(name, WidgetState::Normal).unwrap().size, [6, 7]);
    }
    assert!(skin.piece(SEP, WidgetState::Disabled).unwrap().texture.id() != piece.texture.id());
}

#[test]
fn the_active_segment_drops_one_pixel_like_the_pressed_button_with_its_dot_and_text() {
    let (ctx, skin) = pixel_ctx();
    let items = [GroupItem::with_dot("Button"), GroupItem::with_dot("Button")];
    let mut rects = vec![];
    let shapes = frame(&ctx, vec![], |ui| {
        rects = segment_rects(ui, &items, AT);
        ui.painter()
            .extend(group_shapes(ui, &skin, AT, &items, &looks(&[true, false])));
    });
    let d = drawn(&shapes);
    // El elegido, con la pieza del presionado (su fila de arriba es transparente y no tiene el
    // bisel oscuro); el otro, la normal.
    assert_eq!(d.boxes(&skin, "button", WidgetState::Down), vec![rects[0]]);
    assert_eq!(
        d.boxes(&skin, "button", WidgetState::Normal),
        vec![rects[1]]
    );
    // El separador entero: arriba sigue el contorno del de la derecha (como en el mockup).
    let sep_id = skin.piece(SEP, WidgetState::Normal).unwrap().texture.id();
    let sep = d.images.iter().find(|i| i.0 == sep_id).unwrap();
    assert_eq!((sep.1.top(), sep.2.y), (AT.y, 0.0));
    // El punto verde 1 px más abajo que el morado, los dos 5 px adentro de su segmento.
    let on = d.boxes(&skin, DOT_ON, WidgetState::Normal);
    let off = d.boxes(&skin, DOT_OFF, WidgetState::Normal);
    assert_eq!(
        on,
        vec![Rect::from_min_size(
            pos2(AT.x + 6.0, AT.y + 5.0),
            vec2(6.0, 7.0)
        )]
    );
    assert_eq!(
        off,
        vec![Rect::from_min_size(
            pos2(rects[1].left() + 6.0, AT.y + 4.0),
            vec2(6.0, 7.0)
        )]
    );
    // El texto 3 px después del punto, con la sombra; el del elegido, 1 px más abajo.
    let texts: Vec<_> = d
        .texts
        .iter()
        .filter(|t| t.1 == skin.colors().text)
        .collect();
    let (left0, top0) = ink_at(texts[0]);
    let (left1, top1) = ink_at(texts[1]);
    assert_eq!(left0, AT.x + 15.0);
    assert_eq!(left1, rects[1].left() + 15.0);
    assert_eq!(top0, AT.y + button::CAPS_TOP + button::PRESS_DROP);
    assert_eq!(top1, AT.y + button::CAPS_TOP);
    assert_eq!(d.texts.len(), 4, "two shadows");
    // Los dos elegidos (no pasa con radios): el separador sin la fila de arriba.
    let shapes = frame(&ctx, vec![], |ui| {
        ui.painter()
            .extend(group_shapes(ui, &skin, AT, &items, &looks(&[true, true])));
    });
    let d = drawn(&shapes);
    let sep = d.images.iter().find(|i| i.0 == sep_id).unwrap();
    assert_eq!(sep.1.top(), AT.y + 1.0);
    assert_eq!(sep.2.y, 1.0 / 17.0);
}

#[test]
fn hover_and_disabled_use_the_button_pieces_per_segment() {
    let (ctx, skin) = pixel_ctx();
    let items = [GroupItem::text("One"), GroupItem::text("Two")];
    let shapes = frame(&ctx, vec![], |ui| {
        let looks = [
            SegmentLook {
                state: ButtonState::Hover,
                on: false,
            },
            SegmentLook {
                state: ButtonState::Normal,
                on: false,
            },
        ];
        ui.painter()
            .extend(group_shapes(ui, &skin, AT, &items, &looks));
    });
    let d = drawn(&shapes);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Hover).len(), 1);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Normal).len(), 1);
    // Con el puntero encima, sin sombra: 1 + 2 textos.
    assert_eq!(d.texts.len(), 3);
    let shapes = frame(&ctx, vec![], |ui| {
        let looks = [SegmentLook {
            state: ButtonState::Disabled,
            on: true,
        }; 2];
        ui.painter()
            .extend(group_shapes(ui, &skin, AT, &items, &looks));
    });
    let d = drawn(&shapes);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Disabled).len(), 2);
    assert_eq!(d.boxes(&skin, SEP, WidgetState::Disabled).len(), 1);
    let colors = skin.colors();
    let inks: Vec<Color32> = d
        .texts
        .iter()
        .map(|t| t.1)
        .filter(|c| *c != colors.background)
        .collect();
    assert_eq!(inks, vec![colors.button_text_disabled; 2]);
}

#[test]
fn a_real_group_reports_the_clicked_segment_and_disables_as_a_whole() {
    let (ctx, skin) = pixel_ctx();
    let items = [GroupItem::text("One"), GroupItem::text("Two")];
    let mut picked = None;
    let mut rects = vec![];
    let mut run = |events: Vec<Event>, enabled: bool| {
        let shapes = frame(&ctx, events, |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                let at = ui.cursor().min.round();
                rects = segment_rects(ui, &items, at);
                picked = button_group(ui, &skin, &items, &|i| i == 0);
            });
        });
        (drawn(&shapes), picked, rects.clone())
    };
    let (d, _, rects) = run(vec![], true);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Down), vec![rects[0]]);
    let at = rects[1].center();
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (d, _, _) = run(vec![Event::PointerMoved(at)], true);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Hover), vec![rects[1]]);
    let (d, _, _) = run(vec![press(true)], true);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Down).len(), 2);
    let (_, picked, _) = run(vec![press(false)], true);
    assert_eq!(picked, Some(1));
    let (d, picked, _) = run(vec![], false);
    assert_eq!(picked, None);
    assert_eq!(d.boxes(&skin, "button", WidgetState::Disabled).len(), 2);
}
