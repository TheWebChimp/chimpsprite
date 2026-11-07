use cs_core::{ColorMode, Frame, Sprite, Tag};
use cs_io::spritesheet::SheetOrder;

use super::*;

/// Un sprite de 4×2 con `n` frames de 100 ms y un tag "walk" de los frames 2 a 4.
fn sprite(n: usize) -> Sprite {
    let mut s = Sprite::new(4, 2, ColorMode::Rgba);
    s.frames = vec![Frame { duration_ms: 100 }; n];
    s.tags.push(Tag {
        name: "walk".into(),
        from: 1,
        to: 3,
        direction: Default::default(),
        color: cs_core::Rgba::rgb(0, 0, 0),
        repeat: 0,
    });
    s
}

#[test]
fn by_default_all_frames_go_to_an_almost_square_sheet() {
    let d = ExpandDialog::new("Hero".into(), sprite(5));
    assert_eq!(d.frame_list(), vec![0, 1, 2, 3, 4]);
    let s = d.settings();
    assert_eq!((s.columns, s.rows, s.order), (3, 2, SheetOrder::ByRows));
    assert_eq!(d.sheet_size(), (12, 4));
    let sheet = d.sheet().unwrap();
    assert_eq!((sheet.width, sheet.height, sheet.frames.len()), (12, 4, 1));
}

#[test]
fn a_tag_or_a_range_picks_the_frames() {
    let mut d = ExpandDialog::new("Hero".into(), sprite(6));
    d.frames = ExpandFrames::Tag(0);
    assert_eq!(d.frame_list(), vec![1, 2, 3]);
    d.frames = ExpandFrames::Range;
    d.range = (4, 5);
    assert_eq!(d.frame_list(), vec![4, 5]);
    // Un rango que se sale queda dentro de los frames.
    d.range = (3, 40);
    assert_eq!(d.frame_list(), vec![3, 4, 5]);
}

#[test]
fn fixed_columns_or_rows_compute_the_other_side() {
    let mut d = ExpandDialog::new("Hero".into(), sprite(5));
    d.shape = SheetShape::Columns;
    d.count = 5;
    assert_eq!((d.settings().columns, d.settings().rows), (5, 1));
    d.shape = SheetShape::Rows;
    d.count = 2;
    assert_eq!((d.settings().columns, d.settings().rows), (3, 2));
    // Más que frames no tiene sentido: se acota.
    d.count = 9;
    assert_eq!((d.settings().columns, d.settings().rows), (1, 5));
    d.spacing = 1;
    d.border = 2;
    assert_eq!(d.sheet_size(), (2 * 2 + 4, 2 * 2 + 5 * 2 + 4));
}

#[test]
fn a_sheet_wider_than_16384_is_refused() {
    let mut d = ExpandDialog::new("Hero".into(), sprite(5));
    d.shape = SheetShape::Columns;
    d.count = 5;
    d.spacing = 5000;
    assert!(d.sheet().unwrap_err().contains("too big"));
}
