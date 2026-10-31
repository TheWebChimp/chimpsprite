use eframe::egui::{Event, TextureId};

use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// La textura de la pieza dibujada y los rects de color liso, en orden.
fn drawn(shapes: &[Shape]) -> (Vec<TextureId>, Vec<(Rect, Color32)>) {
    fn walk(shape: &Shape, out: &mut (Vec<TextureId>, Vec<(Rect, Color32)>)) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                out.0.push(mesh.texture_id)
            }
            Shape::Rect(r) => out.1.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut out = Default::default();
    shapes.iter().for_each(|s| walk(s, &mut out));
    out.0.dedup();
    out
}

fn id(skin: &Skin, state: WidgetState) -> TextureId {
    skin.piece(PIECE, state).unwrap().texture.id()
}

#[test]
fn the_frame_is_the_mockup_one_with_a_clear_inside_and_translucent_gloss_and_shade() {
    let data = crate::skin::decode(crate::skin::EMBEDDED).unwrap();
    let piece = &data.pieces[PIECE];
    assert_eq!(piece.size, [16, 17]);
    let at = |x: u32, y: u32| {
        let i = ((y * piece.size[0] + x) * 4) as usize;
        [
            piece.rgba[i],
            piece.rgba[i + 1],
            piece.rgba[i + 2],
            piece.rgba[i + 3],
        ]
    };
    assert_eq!(at(0, 5), [89, 77, 77, 255], "the border");
    assert_eq!(at(5, 5)[3], 0, "the color shows through");
    assert_eq!(at(13, 2), [255, 255, 255, 255], "the gloss");
    assert_eq!(at(11, 2), [255, 255, 255, 87], "the soft gloss");
    assert_eq!(at(5, 14), [0, 0, 0, 97], "the shade row");
    assert_eq!(at(5, 16), [241, 235, 219, 255], "the light below");
    // Sobre el amarillo del mockup, el mismo amarillo oscuro de su fila de sombra.
    let over = |c: u8| (c as f32 * (1.0 - 97.0 / 255.0)).round() as u8;
    assert_eq!([over(255), over(214), over(120)], [158, 133, 74]);
}

#[test]
fn the_button_paints_the_color_inside_the_frame_and_a_click_opens_the_picker() {
    let (ctx, skin) = pixel_ctx();
    let run = |events: Vec<Event>, color: &mut Rgba| {
        let mut response = None;
        let shapes = frame(&ctx, events, |ui| {
            response = Some(color_edit(ui, &skin, color, false, "Light"));
        });
        (shapes, response.unwrap())
    };
    let mut color = Rgba::rgb(255, 214, 120);
    let (shapes, response) = run(vec![], &mut color);
    let rect = response.rect;
    assert_eq!(rect.size(), vec2(16.0, 17.0));
    let (textures, fills) = drawn(&shapes);
    assert_eq!(textures, vec![id(&skin, WidgetState::Normal)]);
    let inner = Rect::from_min_max(rect.min + vec2(1.0, 1.0), rect.max - vec2(1.0, 2.0));
    assert_eq!(fills, vec![(inner, Color32::from_rgb(255, 214, 120))]);
    let popup = egui::Popup::default_response_id(&response);
    assert!(!egui::Popup::is_id_open(&ctx, popup));
    let at = rect.center();
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(vec![Event::PointerMoved(at), button(true)], &mut color);
    run(vec![button(false)], &mut color);
    run(vec![], &mut color);
    assert!(egui::Popup::is_id_open(&ctx, popup), "the picker is open");
}

#[test]
fn with_alpha_the_checker_shows_behind_and_disabled_uses_its_frame() {
    let (ctx, skin) = pixel_ctx();
    let shapes = frame(&ctx, vec![], |ui| {
        ui.add_enabled_ui(false, |ui| {
            swatch_button(ui, &skin, Rgba::new(255, 0, 0, 128), "Tint");
        });
    });
    let (textures, fills) = drawn(&shapes);
    assert_eq!(textures, vec![id(&skin, WidgetState::Disabled)]);
    let dark = Color32::from_rgb(197, 199, 221);
    assert!(fills.iter().any(|(_, c)| *c == dark), "the checker");
    // El color, encima del damero.
    assert_eq!(
        fills.last().unwrap().1,
        Color32::from_rgba_unmultiplied(255, 0, 0, 128)
    );
}

#[test]
fn the_copy_button_of_the_picker_copies_the_hex() {
    let (ctx, skin) = pixel_ctx();
    let mut color = Rgba::new(226, 114, 133, 128);
    let copied = std::cell::RefCell::new(Vec::new());
    let button_id = std::cell::Cell::new(Id::NULL);
    let run = |events: Vec<Event>, color: &mut Rgba| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 300.0))),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            button_id.set(color_edit(ui, &skin, color, true, "Tint").id);
        });
        out.textures_delta.clear();
        for c in out.platform_output.commands {
            if let egui::OutputCommand::CopyText(t) = c {
                copied.borrow_mut().push(t);
            }
        }
    };
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(vec![], &mut color);
    let at = pos2(8.0, 8.0);
    run(vec![Event::PointerMoved(at), press(at, true)], &mut color);
    run(vec![press(at, false)], &mut color);
    run(vec![], &mut color);
    run(vec![], &mut color);
    let copy = ctx
        .read_response(button_id.get().with("picker").with("copy"))
        .expect("the copy button")
        .rect
        .center();
    run(vec![Event::PointerMoved(copy)], &mut color);
    run(vec![press(copy, true)], &mut color);
    run(vec![press(copy, false)], &mut color);
    run(vec![], &mut color);
    assert_eq!(copied.into_inner(), vec!["#e2728580".to_owned()]);
    // Y lo avisa: el globo dice qué se copió.
    let said = ctx.memory(|m| {
        m.data
            .get_temp::<(f64, String)>(button_id.get().with("picker").with("copy").with("copied"))
    });
    assert_eq!(said.map(|(_, hex)| hex), Some("#e2728580".to_owned()));
}
