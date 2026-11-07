use std::sync::Arc;

use cs_core::{Cel, ColorMode, Frame, Image, ImageId, PixelBuffer, PixelValue, Rgba, Sprite};

use crate::ChimpApp;
use crate::canvas::ArtboardRequest;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::view::ViewSettings;

const COLORS: [Rgba; 3] = [
    Rgba::rgb(255, 0, 0),
    Rgba::rgb(0, 255, 0),
    Rgba::rgb(0, 0, 255),
];

/// Un sprite de 2×2 con un frame lleno de cada color de `COLORS`, de 100, 200 y 300 ms.
fn animated() -> Sprite {
    let mut s = Sprite::new(2, 2, ColorMode::Rgba);
    s.frames = (1..=3)
        .map(|i| Frame {
            duration_ms: 100 * i,
        })
        .collect();
    let layer = s.layers[0].id;
    for (f, c) in COLORS.into_iter().enumerate() {
        let id = ImageId(f as u32 + 1);
        let image = Image::from_buffer(2, 2, PixelBuffer::Rgba(vec![c; 4])).unwrap();
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

/// La app con una pestaña cuyo artboard activo es "Hero" (el de `animated`).
fn app() -> ChimpApp {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), ViewSettings::default(), palettes);
    app.new_document();
    assert!(
        app.doc
            .editor
            .add_artboard_with_sprite("Hero".into(), animated())
    );
    app
}

fn pixel(s: &Sprite, frame: usize, x: i32, y: i32) -> Rgba {
    match s.pixel(s.layers[0].id, frame, x, y) {
        PixelValue::Rgba(c) => c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn expand_adds_a_sheet_artboard_in_one_undo_step() {
    let mut app = app();
    let hero = app.doc.editor.active;
    let count = app.doc.editor.document.artboards.len();
    app.doc.canvas.artboard_request = Some(ArtboardRequest::ExpandSheet);
    app.artboard_request();
    let dialog = app.expand_dialog.as_ref().expect("the dialog");
    let sheet = dialog.sheet().unwrap();
    app.expand_dialog = None;
    app.doc
        .editor
        .add_artboard_with_sprite(format!("{} Sheet", "Hero"), sheet);
    let e = &app.doc.editor;
    assert_eq!(e.document.artboards.len(), count + 1);
    assert_ne!(e.active, hero, "the sheet is active");
    assert_eq!(e.artboard().name, "Hero Sheet");
    let s = e.sprite();
    // 3 frames: 2 columnas × 2 filas de 2×2, por filas.
    assert_eq!((s.width, s.height, s.frames.len()), (4, 4, 1));
    assert_eq!(pixel(s, 0, 0, 0), COLORS[0]);
    assert_eq!(pixel(s, 0, 2, 0), COLORS[1]);
    assert_eq!(pixel(s, 0, 0, 2), COLORS[2]);
    assert_eq!(s.grid.size, (2, 2), "the grid shows the cells");
    app.doc.editor.undo();
    assert_eq!(app.doc.editor.document.artboards.len(), count);
}

#[test]
fn slice_parts_the_shown_frame_into_an_animated_artboard() {
    let mut app = app();
    // La hoja de Expand se vuelve a partir en frames: los mismos píxeles.
    app.doc.canvas.artboard_request = Some(ArtboardRequest::ExpandSheet);
    app.artboard_request();
    let sheet = app.expand_dialog.take().unwrap().sheet().unwrap();
    app.doc
        .editor
        .add_artboard_with_sprite("Hero Sheet".into(), sheet);
    let count = app.doc.editor.document.artboards.len();
    app.doc.canvas.artboard_request = Some(ArtboardRequest::SliceFrames);
    app.artboard_request();
    let dialog = app.sheet_dialog.as_mut().expect("the dialog");
    assert_eq!(dialog.grid.cell, (2, 2), "the cells of the grid");
    dialog.duration_ms = 80;
    let sprite = dialog.sprite().unwrap();
    app.sheet_dialog = None;
    app.add_sliced_artboard("Hero Sheet", sprite);
    let e = &app.doc.editor;
    assert_eq!(e.document.artboards.len(), count + 1);
    assert_eq!(e.artboard().name, "Hero Sheet Frames");
    let s = e.sprite();
    // La celda vacía del final no se agrega.
    assert_eq!((s.width, s.height, s.frames.len()), (2, 2, 3));
    for (f, c) in COLORS.into_iter().enumerate() {
        assert_eq!(pixel(s, f, 1, 1), c, "frame {f}");
        assert_eq!(s.frames[f].duration_ms, 80);
    }
    assert_eq!(app.tabs.docs(&app.doc).len(), 1, "in the same tab");
}

#[test]
fn fonts_and_single_frame_artboards_do_not_expand() {
    let mut app = app();
    // El Untitled de un solo frame.
    let first = app.doc.editor.document.artboards[0].id;
    app.doc.editor.activate(first);
    app.doc.canvas.artboard_request = Some(ArtboardRequest::ExpandSheet);
    app.artboard_request();
    assert!(app.expand_dialog.is_none());
    // Slice sí: es una hoja de un frame.
    app.doc.canvas.artboard_request = Some(ArtboardRequest::SliceFrames);
    app.artboard_request();
    assert!(app.sheet_dialog.is_some());
}
