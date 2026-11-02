use cs_core::{ColorMode, FontMode, ONE_BIT_INK, PixelValue, Sprite};

use super::Editor;

fn editor_with_font() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
    e
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn a_new_font_is_active_with_its_first_glyph() {
    let e = editor_with_font();
    assert_eq!(e.font().unwrap().name, "Mine");
    assert_eq!(e.current_glyph(), Some('!'));
    assert_eq!(e.document.artboards.len(), 2);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn going_to_a_glyph_is_changing_the_frame() {
    let mut e = editor_with_font();
    assert!(e.go_to_glyph('A'));
    assert_eq!(e.current_glyph(), Some('A'));
    assert!(!e.go_to_glyph('á'));
    e.next_frame();
    assert_eq!(e.current_glyph(), Some('B'));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn one_bit_fonts_paint_with_the_ink_whatever_the_color() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    e.primary = PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    assert_eq!(e.color(), ONE_BIT_INK);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn glyphs_are_added_removed_and_their_advance_set_with_undo() {
    let mut e = editor_with_font();
    assert!(e.add_glyphs(&['á']));
    assert!(e.go_to_glyph('á'));
    assert!(e.set_glyph_advance('á', Some(5)));
    assert_eq!(e.glyph_advance(), Some(5));
    e.undo();
    assert_eq!(e.glyph_advance(), Some(4), "empty: half the 8 px cell");
    assert!(e.remove_glyph('á'));
    assert_eq!(e.font().unwrap().index_of('á'), None);
    e.undo();
    assert!(e.font().unwrap().index_of('á').is_some());
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn shift_copy_paste_and_clear_a_glyph() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    let layer = e.layer;
    let frame = e.frame;
    e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(1, 1, ONE_BIT_INK)],
    });
    assert!(e.shift_glyph(1, 0));
    assert_eq!(e.sprite().pixel(layer, frame, 2, 1), ONE_BIT_INK);
    e.copy_glyph();
    e.go_to_glyph('B');
    assert!(e.paste_glyph());
    assert_eq!(e.sprite().pixel(layer, e.frame, 2, 1), ONE_BIT_INK);
    assert!(e.clear_glyph());
    assert_ne!(e.sprite().pixel(layer, e.frame, 2, 1), ONE_BIT_INK);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn glyph_steps_have_glyph_names_in_the_history() {
    let mut e = editor_with_font();
    e.add_glyphs(&['á']);
    assert_eq!(e.history.done().last(), Some("Add Glyph"));
    e.remove_glyph('á');
    assert_eq!(e.history.done().last(), Some("Remove Glyph"));
    e.set_glyph_advance('A', Some(3));
    assert_eq!(e.history.done().last(), Some("Font"));
    let mut info = e.font().unwrap().clone();
    info.letter_spacing = 2;
    e.set_font_info(info);
    assert_eq!(e.history.done().last(), Some("Font"));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_text_font_of_a_deleted_font_artboard_falls_back_to_the_pixel_font() {
    use cs_tools::text::FontChoice;
    let mut e = editor_with_font();
    let id = e.active;
    // Una fuente del documento que se llama como una integrada no es la integrada.
    let hack = FontChoice::Document {
        artboard: id.0,
        name: "Hack".into(),
    };
    assert_ne!(hack, FontChoice::Hack);
    assert_eq!(
        hack,
        FontChoice::Document {
            artboard: id.0,
            name: "Renamed".into(),
        }
    );
    e.text_style.font = hack;
    e.text_style.size = 2;
    e.sync_text_font();
    assert_eq!(
        e.text_style.font.name(),
        "Mine",
        "the name follows the font"
    );
    e.remove_active_artboard();
    e.sync_text_font();
    assert_eq!(e.text_style.font, FontChoice::Pixel);
}

/// Pinta `value` en (1, 1) del glifo actual.
fn paint(e: &mut Editor, value: PixelValue) {
    let (layer, frame) = (e.layer, e.frame);
    assert!(e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(1, 1, value)],
    }));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn a_glyph_pastes_only_into_a_font_of_its_color_mode_and_keeps_its_opacity() {
    let mut e = editor_with_font();
    let one_bit = e.active;
    assert!(e.new_font("Colors", (8, 8), FontMode::Color));
    e.go_to_glyph('A');
    paint(&mut e, PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0)));
    let (layer, frame) = (e.layer, e.frame);
    let mut cel = *e.sprite().cel(layer, frame).unwrap();
    cel.opacity = 128;
    assert!(e.execute(cs_commands::Command::SetCel {
        layer,
        frame,
        cel: Some(cel),
    }));
    e.copy_glyph();
    // En la misma fuente (color): con su opacidad.
    e.go_to_glyph('B');
    assert!(e.paste_glyph());
    assert_eq!(e.sprite().cel(e.layer, e.frame).unwrap().opacity, 128);
    // En la de 1 bit: no.
    e.activate(one_bit);
    e.go_to_glyph('A');
    let before = e.document.clone();
    assert!(!e.paste_glyph());
    assert_eq!(e.document, before);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_text_tool_writes_with_the_ink_on_a_one_bit_font() {
    let mut e = editor_with_font();
    let one_bit = e.active;
    // Una fuente de color con su 'A' en rojo, como fuente del texto.
    assert!(e.new_font("Colors", (8, 8), FontMode::Color));
    let colors = e.active;
    e.go_to_glyph('A');
    paint(&mut e, PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0)));
    e.activate(one_bit);
    e.go_to_glyph('B');
    e.primary = PixelValue::Index(7);
    for font in [
        cs_tools::text::FontChoice::Pixel,
        cs_tools::text::FontChoice::Document {
            artboard: colors.0,
            name: "Colors".into(),
        },
    ] {
        e.text_style.font = font;
        e.text_style.size = 1;
        e.text = Some(crate::editor::TextDraft {
            origin: (0, 0),
            text: "A".into(),
        });
        let Some(cs_commands::Command::DrawPixels { pixels, .. }) = e.text_command() else {
            panic!("a draw command");
        };
        assert!(!pixels.is_empty());
        assert!(pixels.iter().all(|p| p.2 == ONE_BIT_INK), "{pixels:?}");
    }
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn renaming_a_font_artboard_renames_the_font_in_one_step() {
    let mut e = editor_with_font();
    let id = e.active;
    let steps = e.history.done_count();
    e.rename_active_artboard("Other".into());
    assert_eq!(e.artboard().name, "Other");
    assert_eq!(e.font().unwrap().name, "Other");
    assert_eq!(e.history.done_count(), steps + 1);
    e.undo();
    assert_eq!(
        (e.artboard().name.as_str(), e.font().unwrap().name.as_str()),
        ("Mine", "Mine")
    );
    // El campo Name del panel Font.
    let mut info = e.font().unwrap().clone();
    info.name = "Third".into();
    assert!(e.set_font_info(info));
    assert_eq!(e.artboard().name, "Third");
    e.undo();
    assert_eq!(e.artboard().name, "Mine");
    // Desde la Library (o cualquier artboard por id).
    e.rename_artboard(id, "Lib".into());
    assert_eq!(e.font().unwrap().name, "Lib");
}

/// Que la fuente siga con un glifo por frame, en orden, una capa y sus 8×8.
fn assert_intact(e: &Editor) {
    let font = e.font().unwrap();
    assert_eq!(font.check(e.sprite()), Ok(()));
    assert_eq!((e.sprite().width, e.sprite().height), (8, 8));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn frame_actions_do_nothing_on_a_font() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    let before = e.document.clone();
    // Alt+N (New Frame), New Empty Frame, Delete Frame.
    e.duplicate_frame();
    e.insert_empty_frame();
    e.remove_frame();
    e.move_frame(e.frame, 0);
    e.frame_range = Some((0, 3));
    assert!(!e.reverse_frames());
    assert_intact(&e);
    assert_eq!(e.document, before);
    // Cortar y pegar frames del timeline.
    e.frame_range = Some((0, 1));
    e.frames_picked = true;
    assert!(e.cut_timeline().is_none());
    let clip = e.copy_timeline().unwrap();
    e.paste_timeline(&clip);
    assert_intact(&e);
    assert_eq!(e.document, before);
    assert_eq!(e.current_glyph(), Some('A'));
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn play_does_nothing_on_a_font() {
    let mut e = editor_with_font();
    e.toggle_play();
    assert!(!e.playing);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_sprite_of_a_font_keeps_its_color_mode_and_size() {
    let mut e = editor_with_font();
    let before = e.document.clone();
    assert!(!e.set_color_mode(cs_commands::sprite_ops::ColorTarget::ALL[0]));
    assert!(!e.set_color_mode(cs_commands::sprite_ops::ColorTarget::ALL[1]));
    assert!(!e.scale_sprite(16, 16));
    assert!(!e.rotate_canvas(cs_commands::sprite_ops::Rotation::Clockwise));
    e.selection = Some(cs_core::Mask::from_rect(8, 8, (0, 0), (3, 3)));
    assert!(!e.crop_to_selection());
    assert!(!e.can_resize_sprite());
    assert_intact(&e);
    assert_eq!(e.document, before);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn layers_cannot_be_added_or_removed_on_a_font() {
    let mut e = editor_with_font();
    let before = e.document.clone();
    e.add_layer(cs_core::LayerKind::Image);
    e.add_layer(cs_core::LayerKind::Group);
    assert!(!e.duplicate_layer());
    e.remove_layer();
    assert_intact(&e);
    assert_eq!(e.document, before);
    assert_eq!(e.sprite().layers.len(), 1);
}

#[test]
#[ignore = "cad-font-tabs: workspace (Task 4/5)"]
fn the_text_tool_writes_with_a_document_font() {
    let mut e = editor_with_font();
    let font_id = e.active;
    e.go_to_glyph('A');
    let (layer, frame) = (e.layer, e.frame);
    e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(0, 0, ONE_BIT_INK)],
    });
    assert_eq!(e.document_fonts(), vec![(font_id, "Mine".to_owned())]);
    e.activate(cs_core::ArtboardId(1));
    e.text_style.font = cs_tools::text::FontChoice::Document {
        artboard: font_id.0,
        name: "Mine".into(),
    };
    e.primary = PixelValue::Rgba(cs_core::Rgba::rgb(0, 255, 0));
    e.text = Some(crate::editor::TextDraft {
        origin: (2, 3),
        text: "A".into(),
    });
    let Some(cs_commands::Command::DrawPixels { pixels, .. }) = e.text_command() else {
        panic!("a draw command");
    };
    assert_eq!(pixels, vec![(2, 3, e.primary)]);
}
