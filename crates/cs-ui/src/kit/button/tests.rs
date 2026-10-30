use eframe::egui::{Event, Pos2, RawInput, TextureId, pos2};

use super::*;

/// Un contexto con la fuente pixel y el estilo de Pixel UI, y el skin embebido. Las fuentes rigen
/// desde el segundo frame.
pub(crate) fn pixel_ctx() -> (egui::Context, Skin) {
    let ctx = egui::Context::default();
    ctx.set_fonts(crate::appearance::font_definitions(true));
    let skin = Skin::embedded(&ctx);
    crate::appearance::apply_style(&ctx, Some(skin.colors()));
    frame(&ctx, vec![], |_| {});
    (ctx, skin)
}

/// Un frame; devuelve lo dibujado. Las texturas no se suben a ninguna GPU.
pub(crate) fn frame(
    ctx: &egui::Context,
    events: Vec<Event>,
    mut run: impl FnMut(&mut Ui),
) -> Vec<Shape> {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| run(ui));
    output.textures_delta.clear();
    output.shapes.into_iter().map(|c| c.shape).collect()
}

/// Lo que dibujan las formas: las piezas (su textura) y los textos (su posición, color y
/// galería).
#[derive(Default)]
struct Drawn {
    textures: Vec<TextureId>,
    texts: Vec<(Pos2, Color32, Arc<Galley>)>,
}

fn drawn(shapes: &[Shape]) -> Drawn {
    fn walk(shape: &Shape, out: &mut Drawn) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
                out.textures.push(mesh.texture_id);
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
    // Las partes del 9-slice, una vez por pieza.
    out.textures.dedup();
    out
}

/// La columna donde empieza la tinta del texto y la fila de su línea base.
fn ink_left_and_baseline(at: Pos2, galley: &Galley) -> (f32, f32) {
    let row = &galley.rows[0];
    let glyph = &row.glyphs[0];
    (
        at.x + glyph.pos.x + glyph.uv_rect.offset.x,
        at.y + row.pos.y + glyph.pos.y,
    )
}

fn texture(skin: &Skin, state: WidgetState) -> TextureId {
    skin.piece(PIECE, state).unwrap().texture.id()
}

#[test]
fn the_button_is_17_high_and_its_text_box_matches_the_mockup() {
    let (ctx, skin) = pixel_ctx();
    let mut rect = Rect::NOTHING;
    let mut ink = 0.0;
    let shapes = frame(&ctx, vec![], |ui| {
        ink = ink_width(ui, "Button");
        rect = button(ui, &skin, "Button").rect;
    });
    // El "Button" del mockup: 27 de tinta en un botón de 39.
    assert_eq!(ink, 27.0);
    assert_eq!(rect.size(), vec2(ink + 12.0, HEIGHT));
    let d = drawn(&shapes);
    assert_eq!(d.textures, vec![texture(&skin, WidgetState::Normal)]);
    let (at, color, galley) = d.texts.last().unwrap();
    assert_eq!(*color, skin.colors().text);
    let (left, baseline) = ink_left_and_baseline(*at, galley);
    assert_eq!(left, rect.left() + 6.0, "1 + 1 + 4 to the ink");
    // Las mayúsculas, 4 filas debajo del borde de arriba (6 de alto, hasta la línea base).
    assert_eq!(baseline - tab_bar::CAP_HEIGHT, rect.top() + CAPS_TOP);
    assert_eq!(CAPS_TOP, 4.0);
    // Más ancho, el texto centrado.
    let shapes = frame(&ctx, vec![], |ui| {
        rect = button_sized(ui, &skin, "Button", 60.0).rect;
    });
    assert_eq!(rect.width(), 60.0);
    let d = drawn(&shapes);
    let (at, _, galley) = d.texts.last().unwrap();
    let (left, _) = ink_left_and_baseline(*at, galley);
    assert_eq!(left, rect.left() + ((60.0 - ink) / 2.0).floor());
}

#[test]
fn the_text_box_fits_the_tallest_accent_and_the_lowest_descender_with_its_air() {
    // La fuente pixel a 16: un píxel de su grilla por punto.
    let font =
        fontdue::Font::from_bytes(crate::appearance::PIXEL_FONT, Default::default()).unwrap();
    let top = |c: char| {
        let m = font.metrics(c, crate::appearance::PIXEL_FONT_SIZE);
        m.ymin as f32 + m.height as f32
    };
    let bottom = |c: char| font.metrics(c, crate::appearance::PIXEL_FONT_SIZE).ymin as f32;
    assert_eq!(top('H'), tab_bar::CAP_HEIGHT);
    let chars: Vec<char> = (' '..='ž')
        .filter(|c| !c.is_control() && font.has_glyph(*c))
        .collect();
    let tallest = chars
        .iter()
        .filter(|c| c.is_uppercase())
        .map(|&c| top(c))
        .fold(0.0, f32::max);
    let lowest = chars.iter().map(|&c| bottom(c)).fold(0.0, f32::min);
    assert_eq!(tallest, tab_bar::CAP_HEIGHT + ACCENT_ABOVE_CAPS, "É, Ñ…");
    assert_eq!(-lowest, DESCENT, "g, p, y…");
    // En filas de la pieza: el acento en la 3, las mayúsculas en la 4, la línea base debajo de
    // la 9, el descendente hasta la 11 y 2 filas de aire antes del bisel (14).
    let baseline = CAPS_TOP + tab_bar::CAP_HEIGHT;
    assert_eq!(baseline - tallest, TOP_ROWS + AIR_TOP);
    assert_eq!(baseline - tallest, 3.0);
    assert_eq!(CAPS_TOP, 4.0);
    assert_eq!(baseline, 10.0);
    assert_eq!(baseline - lowest - 1.0, 11.0, "the last descender row");
    assert_eq!(baseline - lowest + AIR_BOTTOM, HEIGHT - BOTTOM_ROWS);
}

#[test]
fn each_state_has_its_piece_color_and_shadow_and_only_pressed_moves_the_text_down() {
    let (ctx, skin) = pixel_ctx();
    let colors = *skin.colors();
    let rect = Rect::from_min_size(pos2(20.0, 30.0), vec2(39.0, HEIGHT));
    let mut texts = Vec::new();
    for state in ButtonState::ALL {
        let shapes = frame(&ctx, vec![], |ui| {
            ui.painter()
                .extend(button_shapes(&skin, rect, "Button", state, ui));
        });
        let d = drawn(&shapes);
        let piece = match state {
            ButtonState::Normal => WidgetState::Normal,
            ButtonState::Hover => WidgetState::Hover,
            ButtonState::Pressed => WidgetState::Down,
            ButtonState::Disabled => WidgetState::Disabled,
        };
        assert_eq!(d.textures, vec![texture(&skin, piece)], "{state:?}");
        let color = if state == ButtonState::Disabled {
            colors.button_text_disabled
        } else {
            colors.text
        };
        let (text, shadow) = match state {
            // Con el puntero encima, sin sombra.
            ButtonState::Hover => {
                assert_eq!(d.texts.len(), 1);
                (d.texts[0].clone(), None)
            }
            // La sombra primero, 1 px abajo (como en el mockup), y el texto encima.
            _ => {
                assert_eq!(d.texts.len(), 2, "{state:?}");
                (d.texts[1].clone(), Some(d.texts[0].clone()))
            }
        };
        assert_eq!(text.1, color, "{state:?}");
        if let Some(shadow) = shadow {
            assert_eq!(shadow.0, text.0 + vec2(0.0, 1.0), "{state:?}");
            assert_eq!(shadow.1, Color32::from_rgb(221, 206, 191), "{state:?}");
        }
        texts.push(text.0);
    }
    // Presionado, el texto baja con la cara del botón; en los demás, en el mismo lugar.
    let normal = texts[0];
    for (state, at) in ButtonState::ALL.iter().zip(&texts) {
        let drop = if *state == ButtonState::Pressed {
            PRESS_DROP
        } else {
            0.0
        };
        assert_eq!(*at, normal + vec2(0.0, drop), "{state:?}");
    }
    assert_eq!(PRESS_DROP, 1.0);
    // Las piezas son distintas.
    let mut ids: Vec<_> = [
        WidgetState::Normal,
        WidgetState::Hover,
        WidgetState::Down,
        WidgetState::Disabled,
    ]
    .map(|s| texture(&skin, s))
    .to_vec();
    ids.dedup();
    assert_eq!(ids.len(), 4);
}

/// Un frame con un botón real ("Button"): las piezas dibujadas, su rect y si se hizo clic.
fn real(
    ctx: &egui::Context,
    skin: &Skin,
    events: Vec<Event>,
    enabled: bool,
) -> (Vec<TextureId>, Rect, bool) {
    let mut out = (Rect::NOTHING, false);
    let shapes = frame(ctx, events, |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let r = button(ui, skin, "Button");
            out = (r.rect, r.clicked());
        });
    });
    (drawn(&shapes).textures, out.0, out.1)
}

#[test]
fn a_real_button_shows_hover_and_pressed_and_disabled_from_the_ui() {
    let (ctx, skin) = pixel_ctx();
    let (textures, rect, _) = real(&ctx, &skin, vec![], true);
    assert_eq!(textures, vec![texture(&skin, WidgetState::Normal)]);
    let at = rect.center();
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (textures, _, _) = real(&ctx, &skin, vec![Event::PointerMoved(at)], true);
    assert_eq!(textures, vec![texture(&skin, WidgetState::Hover)]);
    let (textures, _, _) = real(&ctx, &skin, vec![press(true)], true);
    assert_eq!(textures, vec![texture(&skin, WidgetState::Down)]);
    let (_, _, clicked) = real(&ctx, &skin, vec![press(false)], true);
    assert!(clicked);
    let (textures, _, _) = real(&ctx, &skin, vec![], false);
    assert_eq!(textures, vec![texture(&skin, WidgetState::Disabled)]);
}
