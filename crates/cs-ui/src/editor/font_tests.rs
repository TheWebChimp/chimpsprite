use cs_core::{ColorMode, FontMode, ONE_BIT_INK, PixelValue, Sprite};

use super::Editor;

fn editor_with_font() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
    e
}

#[test]
fn a_new_font_is_active_with_its_first_glyph() {
    let e = editor_with_font();
    assert_eq!(e.font().unwrap().name, "Mine");
    assert_eq!(e.current_glyph(), Some('!'));
    assert_eq!(e.document.artboards.len(), 2);
}

#[test]
fn going_to_a_glyph_is_changing_the_frame() {
    let mut e = editor_with_font();
    assert!(e.go_to_glyph('A'));
    assert_eq!(e.current_glyph(), Some('A'));
    assert!(!e.go_to_glyph('á'));
    e.next_frame();
    assert_eq!(e.current_glyph(), Some('B'));
}

#[test]
fn one_bit_fonts_paint_with_the_ink_whatever_the_color() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    e.primary = PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    assert_eq!(e.color(), ONE_BIT_INK);
}

#[test]
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
