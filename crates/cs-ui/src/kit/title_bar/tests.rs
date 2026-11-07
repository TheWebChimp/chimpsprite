use super::*;
use crate::kit::button::tests::{frame, pixel_ctx};

/// Los puntos de color dibujados (los íconos), con su color.
fn dots_of(shapes: &[Shape]) -> Vec<(Pos2, Color32)> {
    shapes
        .iter()
        .filter_map(|s| match s {
            Shape::Rect(r) if r.rect.size() == Vec2::splat(1.0) => Some((r.rect.min, r.fill)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_bar_has_the_menu_the_title_and_the_close_mark_like_the_mockup() {
    let (ctx, skin) = pixel_ctx();
    let look = TitleLook::new(&skin);
    let rect = Rect::from_min_size(pos2(10.0, 10.0), vec2(173.0, HEIGHT));
    let mut shapes = vec![];
    frame(&ctx, vec![], |ui| {
        let state = TitleState {
            focused: true,
            menu: true,
            close: Some(false),
            play: None,
        };
        shapes = bar_shapes(ui.painter(), &look, rect, "Un título", state, 1.0);
    });
    let dots = dots_of(&shapes);
    let ink = Color32::from_rgb(100, 54, 75);
    // ≡: 3 rayas de 6 desde la columna 4, filas 5, 7 y 9.
    assert!(dots.contains(&(pos2(14.0, 15.0), ink)));
    assert!(dots.contains(&(pos2(19.0, 19.0), ink)));
    // ○: 4 px antes del borde derecho, desde la fila 5.
    let right = dots.iter().map(|(p, _)| p.x).fold(f32::MIN, f32::max);
    assert_eq!(right + 1.0, rect.right() - CLOSE_RIGHT);
    // El título, 2 px después del ≡, en `color.text`.
    let text = shapes.iter().find_map(|s| match s {
        Shape::Text(t) => Some(t.clone()),
        _ => None,
    });
    let text = text.expect("the title");
    assert_eq!(text.fallback_color, skin.colors().text);
    let glyph = &text.galley.rows[0].glyphs[0];
    assert_eq!(
        text.pos.x + glyph.pos.x + glyph.uv_rect.offset.x,
        rect.left() + 12.0
    );
}

#[test]
fn the_orange_font_bar_dims_to_its_own_dark_row() {
    let (ctx, skin) = pixel_ctx();
    let look = TitleLook::with_bar(&skin, "title.bar.font");
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, HEIGHT));
    let orange = skin.piece("title.bar.font", WidgetState::Normal).unwrap();
    let mut shapes = vec![];
    frame(&ctx, vec![], |ui| {
        let state = TitleState {
            focused: false,
            menu: true,
            close: Some(false),
            play: None,
        };
        shapes = bar_shapes(ui.painter(), &look, rect, "Mine", state, 1.0);
    });
    assert!(shapes.iter().any(|s| matches!(
        s,
        Shape::Mesh(m) if m.texture_id == orange.texture.id()
    )));
    let dim = Color32::from_rgb(212, 102, 54);
    assert!(dots_of(&shapes).iter().all(|(_, c)| *c == dim));
    assert_eq!(look.text_color(false), dim);
    // Con el foco, como la rosa.
    assert_eq!(look.text_color(true), skin.colors().text);
    // Sin la pieza, la rosa.
    let pink = TitleLook::with_bar(&skin, "no.such.bar");
    assert_eq!(pink.text_color(false), Color32::from_rgb(178, 82, 102));
}

#[test]
fn without_focus_everything_takes_the_dark_row_color_and_hover_turns_the_mark_into_an_x() {
    let (ctx, skin) = pixel_ctx();
    let look = TitleLook::new(&skin);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, HEIGHT));
    let dim = Color32::from_rgb(178, 82, 102);
    let mut shapes = vec![];
    frame(&ctx, vec![], |ui| {
        let state = TitleState {
            focused: false,
            menu: true,
            close: Some(true),
            play: None,
        };
        shapes = bar_shapes(ui.painter(), &look, rect, "Doc", state, 1.0);
    });
    assert!(dots_of(&shapes).iter().all(|(_, c)| *c == dim));
    let text = shapes.iter().find_map(|s| match s {
        Shape::Text(t) => Some(t.fallback_color),
        _ => None,
    });
    assert_eq!(text, Some(dim));
    // La ×: su esquina de arriba a la izquierda tiene tinta (el ○ no).
    let x_left = rect.right() - CLOSE_RIGHT - 5.0;
    assert!(
        dots_of(&shapes)
            .iter()
            .any(|(p, _)| *p == pos2(x_left, ICON_TOP))
    );
}

#[test]
fn the_play_mark_goes_four_pixels_before_the_close_mark_and_turns_into_a_pause() {
    let (ctx, skin) = pixel_ctx();
    let look = TitleLook::new(&skin);
    let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, HEIGHT));
    let parts = layout_with_play(rect, true, true, true);
    let (play, close) = (parts.play.unwrap(), parts.close.unwrap());
    // Los 9×9 que se tocan quedan pegados sin pisarse; el título termina 3 antes del ▶.
    assert_eq!(play.right(), close.left());
    assert_eq!(parts.text.max, play.left() + 2.0 - CLOSE_GAP);
    assert_eq!(layout(rect, true, true).play, None);
    let dots_with = |playing: bool| {
        let mut shapes = vec![];
        frame(&ctx, vec![], |ui| {
            let state = TitleState {
                focused: true,
                menu: false,
                close: None,
                play: Some(playing),
            };
            shapes = bar_shapes(ui.painter(), &look, rect, "", state, 1.0);
        });
        dots_of(&shapes)
    };
    let ink = Color32::from_rgb(100, 54, 75);
    // Sin ○, el ▶ va en su lugar: 4 px antes del borde, desde la fila 5.
    let play = dots_with(false);
    assert_eq!(play.len(), 13, "the triangle");
    assert!(play.contains(&(pos2(91.0, 5.0), ink)));
    assert!(play.contains(&(pos2(95.0, 7.0), ink)));
    let pause = dots_with(true);
    assert_eq!(pause.len(), 20, "two bars");
    assert!(!pause.iter().any(|(p, _)| p.x == 93.0));
}
