use std::sync::Arc;

use cs_core::{Cel, ColorMode, Frame, Image, ImageId, PixelBuffer, PixelValue, Rgba, Sprite};

use super::*;
use crate::import::{SheetGrid, sprite_from_sheet_ordered};

const RED: Rgba = Rgba::rgb(255, 0, 0);
const GREEN: Rgba = Rgba::rgb(0, 255, 0);
const BLUE: Rgba = Rgba::rgb(0, 0, 255);
const WHITE: Rgba = Rgba::rgb(255, 255, 255);
const T: Rgba = Rgba::TRANSPARENT;

/// Un sprite de 2×1 con un frame lleno de cada color, de 100, 200, 300… ms.
fn frames(colors: &[Rgba]) -> Sprite {
    let mut s = Sprite::new(2, 1, ColorMode::Rgba);
    s.frames = (0..colors.len())
        .map(|i| Frame {
            duration_ms: 100 * (i as u16 + 1),
        })
        .collect();
    let layer = s.layers[0].id;
    for (f, &c) in colors.iter().enumerate() {
        let id = ImageId(f as u32 + 1);
        let image = Image::from_buffer(2, 1, PixelBuffer::Rgba(vec![c, c])).unwrap();
        s.images.insert(id, Arc::new(image));
        s.cels.insert(
            (layer, f),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }
    s
}

fn rgba(s: &Sprite, x: i32, y: i32) -> Rgba {
    match s.pixel(s.layers[0].id, 0, x, y) {
        PixelValue::Rgba(c) => c,
        PixelValue::Index(i) => s.palette.get(i).unwrap(),
    }
}

#[test]
fn auto_columns_is_the_ceiling_of_the_square_root() {
    let cases = [
        (0, 1),
        (1, 1),
        (2, 2),
        (4, 2),
        (5, 3),
        (9, 3),
        (10, 4),
        (16, 4),
        (17, 5),
    ];
    for (n, cols) in cases {
        assert_eq!(auto_columns(n), cols, "{n} frames");
    }
    let s = ExpandSettings::auto(5);
    assert_eq!((s.columns, s.rows), (3, 2));
    assert_eq!(ExpandSettings::auto(10).rows, 3);
    assert_eq!(fit(7, 2), 4);
    assert_eq!(fit(7, 0), 7, "zero counts as one");
}

#[test]
fn cells_go_by_rows_or_by_columns() {
    let order = |o: SheetOrder| (0..5).map(|i| o.cell(i, 3, 2)).collect::<Vec<_>>();
    assert_eq!(
        order(SheetOrder::ByRows),
        vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1)]
    );
    assert_eq!(
        order(SheetOrder::ByColumns),
        vec![(0, 0), (0, 1), (1, 0), (1, 1), (2, 0)]
    );
}

#[test]
fn spacing_and_border_grow_the_sheet_and_move_the_cells() {
    let s = ExpandSettings {
        spacing: 1,
        border: 2,
        ..ExpandSettings::auto(5)
    };
    // 3 × 2 celdas de 2×1: 2 + 2+1+2+1+2 + 2 de ancho, 2 + 1+1+1 + 2 de alto.
    assert_eq!(s.sheet_size((2, 1)), (12, 7));
    assert_eq!(s.cell_origin(0, (2, 1)), (2, 2));
    assert_eq!(s.cell_origin(4, (2, 1)), (5, 4));
}

#[test]
fn expanding_places_each_frame_in_its_cell_flattened() {
    let sprite = frames(&[RED, GREEN, BLUE]);
    let sheet = expand_frames(&sprite, &[0, 1, 2], ExpandSettings::auto(3)).expect("sheet");
    // 2 × 2 celdas de 2×1: la última vacía.
    assert_eq!((sheet.width, sheet.height), (4, 2));
    assert_eq!(sheet.frames.len(), 1);
    assert_eq!(sheet.layers.len(), 1);
    assert_eq!(rgba(&sheet, 0, 0), RED);
    assert_eq!(rgba(&sheet, 3, 0), GREEN);
    assert_eq!(rgba(&sheet, 1, 1), BLUE);
    assert_eq!(rgba(&sheet, 2, 1), T);
    // Sin aire la grilla es la celda.
    assert_eq!(sheet.grid.size, (2, 1));
    assert_eq!(sheet.palette, sprite.palette);
    assert_eq!(sheet.color_mode, ColorMode::Rgba);
}

#[test]
fn expanding_a_range_by_columns_with_spacing() {
    let sprite = frames(&[RED, GREEN, BLUE, WHITE]);
    let settings = ExpandSettings {
        columns: 1,
        rows: 2,
        spacing: 1,
        border: 0,
        order: SheetOrder::ByColumns,
    };
    let sheet = expand_frames(&sprite, &[2, 3], settings).expect("sheet");
    assert_eq!((sheet.width, sheet.height), (2, 3));
    assert_eq!(rgba(&sheet, 0, 0), BLUE);
    assert_eq!(rgba(&sheet, 0, 1), T, "the spacing row");
    assert_eq!(rgba(&sheet, 1, 2), WHITE);
    assert_eq!(
        sheet.frames[0].duration_ms, 300,
        "the first frame's duration"
    );
    assert_eq!(
        sheet.grid,
        cs_core::GridSettings::default(),
        "spacing: no grid"
    );
    // Lo que no entra, frames que no existen o ninguno: nada.
    assert!(expand_frames(&sprite, &[0, 1, 2], settings).is_none());
    assert!(expand_frames(&sprite, &[9], settings).is_none());
    assert!(expand_frames(&sprite, &[], settings).is_none());
}

#[test]
fn an_indexed_sprite_gives_an_indexed_sheet_when_its_colors_are_in_the_palette() {
    let mut sprite = frames(&[RED, BLUE]);
    sprite.palette = cs_core::Palette::new(vec![T, RED, BLUE]);
    sprite.color_mode = ColorMode::Indexed {
        transparent_index: 0,
    };
    for (i, image) in sprite.images.values_mut().enumerate() {
        let v = i as u8 + 1;
        *image = Arc::new(Image::from_buffer(2, 1, PixelBuffer::Indexed(vec![v, v])).unwrap());
    }
    let sheet = expand_frames(&sprite, &[0, 1], ExpandSettings::auto(2)).unwrap();
    assert_eq!(sheet.color_mode, sprite.color_mode);
    assert_eq!(sheet.layers.len(), 1);
    assert_eq!(
        sheet.pixel(sheet.layers[0].id, 0, 0, 0),
        PixelValue::Index(1)
    );
    assert_eq!(
        sheet.pixel(sheet.layers[0].id, 0, 2, 0),
        PixelValue::Index(2)
    );
    assert_eq!(
        sheet.pixel(sheet.layers[0].id, 0, 0, 1),
        PixelValue::Index(0)
    );
    // Un color que no está (un cel a media opacidad): queda RGBA.
    let layer = sprite.layers[0].id;
    sprite.cels.get_mut(&(layer, 1)).unwrap().opacity = 128;
    let sheet = expand_frames(&sprite, &[0, 1], ExpandSettings::auto(2)).unwrap();
    assert_eq!(sheet.color_mode, ColorMode::Rgba);
    assert_eq!(rgba(&sheet, 0, 0), RED);
}

#[test]
fn slicing_an_expanded_sheet_gives_the_frames_back() {
    let sprite = frames(&[RED, GREEN, BLUE, WHITE, RED]);
    for order in [SheetOrder::ByRows, SheetOrder::ByColumns] {
        let settings = ExpandSettings {
            spacing: 1,
            border: 1,
            order,
            ..ExpandSettings::auto(5)
        };
        let sheet = expand_frames(&sprite, &[0, 1, 2, 3, 4], settings).unwrap();
        let bitmap = crate::export::Bitmap::from_frame(&sheet, 0, 1);
        let grid = SheetGrid::from_counts(
            (sheet.width, sheet.height),
            (settings.columns, settings.rows),
            (1, 1),
            (1, 1),
        );
        assert_eq!(grid.cell, (2, 1), "{order:?}");
        let back = sprite_from_sheet_ordered(&bitmap, grid, order, 80).unwrap();
        assert_eq!(
            back.frames.len(),
            5,
            "the empty sixth cell is dropped, {order:?}"
        );
        assert!(back.frames.iter().all(|f| f.duration_ms == 80));
        for f in 0..5 {
            assert_eq!(
                back.pixel(back.layers[0].id, f, 1, 0),
                sprite.pixel(sprite.layers[0].id, f, 1, 0),
                "frame {f}, {order:?}"
            );
        }
    }
}

#[test]
fn a_grid_from_counts_splits_what_is_left_after_the_offset_and_padding() {
    let g = SheetGrid::from_counts((64, 32), (4, 2), (0, 0), (0, 0));
    assert_eq!(g.cell, (16, 16));
    let g = SheetGrid::from_counts((65, 33), (4, 2), (1, 1), (2, 2));
    // (65 − 2 − 3) / 4 = 15; (33 − 2 − 1) / 2 = 15.
    assert_eq!(g.cell, (15, 15));
    assert_eq!(g.count((65, 33)), (4, 2));
    let g = SheetGrid::from_counts((3, 3), (8, 0), (0, 0), (0, 0));
    assert_eq!(g.cell, (1, 3), "at least one pixel; zero rows count as one");
}
