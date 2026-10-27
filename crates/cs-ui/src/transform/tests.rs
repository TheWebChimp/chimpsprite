use std::f32::consts::{FRAC_PI_2, PI};

use cs_commands::Command;
use cs_core::{ColorMode, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};

use super::*;
use crate::editor::ToolKind;
use crate::transform_box::{Handle, TransformBox, TransformHit};

fn red() -> PixelValue {
    PixelValue::Rgba(Rgba::rgb(255, 0, 0))
}

/// 8×8 con un punto rojo en (1, 1), la selección (0, 0)–(2, 2) y una herramienta de selección.
fn editor() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, red())],
    });
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
    e.tool = ToolKind::RectSelect;
    e
}

fn px(e: &Editor, x: i32, y: i32) -> PixelValue {
    e.sprite().pixel(LayerId(1), 0, x, y)
}

/// `editor()` con el punto rojo en `at` (dentro de la selección de 3×3) en vez de en (1, 1).
fn editor_with_dot(at: (i32, i32)) -> Editor {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, t), (at.0, at.1, red())],
    });
    e
}

#[test]
fn flipping_a_selection_leaves_it_floating_and_flipped_in_its_box() {
    let mut e = editor_with_dot((0, 1));
    let t = e.sprite().color_mode.transparent();
    assert!(e.flip(true));
    assert!(e.is_floating(), "with a selection tool it keeps floating");
    assert_eq!(e.transform_box().map(|b| (b.left(), b.top())), Some((0, 0)));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 2, 1), red());
    assert_eq!(px(&e, 0, 1), t);
    // Un solo paso de undo.
    e.undo();
    assert_eq!(px(&e, 0, 1), red());
    assert_eq!(px(&e, 2, 1), t);
}

#[test]
fn flipping_vertically_mirrors_top_and_bottom() {
    let mut e = editor_with_dot((1, 0));
    assert!(e.flip(false));
    e.commit_floating();
    assert_eq!(px(&e, 1, 2), red());
    assert_eq!(px(&e, 1, 0), e.sprite().color_mode.transparent());
}

#[test]
fn flipping_twice_gets_back_where_it_was() {
    let mut e = editor_with_dot((0, 1));
    e.flip(true);
    e.flip(true);
    e.commit_floating();
    assert_eq!(px(&e, 0, 1), red());
}

#[test]
fn with_a_painting_tool_the_flip_applies_right_away() {
    let mut e = editor_with_dot((0, 1));
    e.tool = ToolKind::Pencil;
    assert!(e.flip(true));
    assert!(!e.is_floating());
    assert_eq!(px(&e, 2, 1), red());
    assert!(e.selection.is_some(), "the selection stays");
}

#[test]
fn without_a_selection_the_whole_layer_flips() {
    let mut e = editor();
    e.set_selection(None);
    assert!(e.flip(true));
    assert_eq!(px(&e, 6, 1), red());
    assert_eq!(px(&e, 1, 1), e.sprite().color_mode.transparent());
    e.undo();
    assert_eq!(px(&e, 1, 1), red());
    assert!(e.flip(false));
    assert_eq!(px(&e, 1, 6), red());
}

#[test]
fn a_locked_layer_does_not_flip() {
    let mut e = editor();
    e.set_selection(None);
    e.toggle_locked(LayerId(1));
    assert!(!e.flip(true));
    assert_eq!(px(&e, 1, 1), red());
}

/// Lo que muestra el lienzo (con lo flotante aplicado).
fn shown(e: &Editor, x: i32, y: i32) -> PixelValue {
    e.view().document.artboards[0]
        .sprite
        .pixel(LayerId(1), 0, x, y)
}

fn place(e: &mut Editor, bbox: TransformBox) {
    e.floating.as_mut().expect("floating").bbox = bbox;
    e.refresh_floating(false);
}

fn moved(e: &mut Editor, dx: i32, dy: i32) {
    let b = e.transform_box().unwrap().translated(dx, dy);
    place(e, b);
}

#[test]
fn escape_cancels_the_gesture_then_applies_the_floating_content_then_deselects() {
    let mut e = editor();
    // Un lazo poligonal a medias: Esc lo descarta y la selección sigue.
    e.set_tool(ToolKind::PolygonLasso);
    e.pointer_press((6.5, 6.5), false, false, 0.1);
    e.pointer_release();
    assert!(e.is_pending());
    e.escape();
    assert!(!e.is_pending());
    assert!(e.selection.is_some());
    // Flotando: Esc lo deja donde se movió, sin deseleccionar.
    e.set_tool(ToolKind::RectSelect);
    assert!(e.lift());
    moved(&mut e, 2, 2);
    e.escape();
    assert!(!e.is_floating());
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((2, 2), (4, 4))));
    assert_eq!(px(&e, 3, 3), red());
    // Sin nada más: Esc deselecciona, como Deselect.
    e.escape();
    assert!(e.selection.is_none());
    assert!(e.can_reselect());
}

#[test]
fn lifting_and_moving_only_changes_the_view_until_commit() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    moved(&mut e, 3, 2);
    assert_eq!(px(&e, 1, 1), red(), "the layer is untouched");
    assert_eq!(shown(&e, 1, 1), t);
    assert_eq!(shown(&e, 4, 3), red());
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((3, 2), (5, 4))));
    let version = e.doc_version;
    assert!(e.commit_floating());
    assert_eq!(e.doc_version, version + 1, "a single command");
    assert_eq!(px(&e, 4, 3), red());
    assert_eq!(px(&e, 1, 1), t);
    e.undo();
    assert_eq!(px(&e, 1, 1), red());
    assert_eq!(px(&e, 4, 3), t);
}

#[test]
fn cancel_leaves_no_trace_and_restores_the_selection() {
    let mut e = editor();
    let before = e.selection.clone();
    let version = e.doc_version;
    assert!(e.lift());
    moved(&mut e, 1, 1);
    assert!(e.cancel_floating());
    assert!(!e.is_floating());
    assert_eq!(e.doc_version, version);
    assert_eq!(e.selection, before);
    assert_eq!(shown(&e, 1, 1), red());
    assert!(!e.cancel_floating(), "nothing left to cancel");
}

#[test]
fn scaling_doubles_pixels_and_commit_selects_the_result() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    place(&mut e, TransformBox::new(0, 0, 6, 6));
    assert!(e.commit_floating());
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        assert_eq!(px(&e, x, y), red(), "({x}, {y})");
    }
    assert_eq!(px(&e, 1, 1), t);
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((0, 0), (5, 5))));
}

#[test]
fn an_untouched_lift_commits_nothing() {
    let mut e = editor();
    let version = e.doc_version;
    assert!(e.lift());
    assert!(e.commit_floating());
    assert_eq!(e.doc_version, version, "no empty undo step");
}

#[test]
fn changing_layer_commits_and_locking_the_layer_cancels() {
    let mut e = editor();
    e.add_layer(LayerKind::Image);
    let top = e.layer;
    e.select_layer(LayerId(1));
    assert!(e.lift());
    moved(&mut e, 4, 0);
    e.select_layer(top);
    assert!(!e.is_floating());
    assert_eq!(
        px(&e, 5, 1),
        red(),
        "committed on the layer it was lifted from"
    );

    e.select_layer(LayerId(1));
    e.set_selection(Some(Mask::from_rect(8, 8, (4, 0), (6, 2))));
    assert!(e.lift());
    moved(&mut e, 0, 3);
    // Bloquear con el comando (un cambio de propiedades) confirma antes.
    e.toggle_locked(LayerId(1));
    assert!(!e.is_floating());
    assert_eq!(px(&e, 5, 4), red(), "committed before locking");
    e.toggle_locked(LayerId(1));

    // Bloqueada por un camino que no confirma: el frame siguiente cancela.
    e.set_selection(Some(Mask::from_rect(8, 8, (4, 3), (6, 5))));
    assert!(e.lift());
    moved(&mut e, 0, -3);
    let lock = cs_commands::DocCommand::OnArtboard {
        id: e.active,
        cmd: Command::SetLayerProps {
            id: LayerId(1),
            props: cs_commands::LayerProps {
                locked: Some(true),
                ..Default::default()
            },
        },
    };
    e.history.execute(&mut e.document, lock).unwrap();
    e.sync_floating();
    assert!(!e.is_floating());
    assert_eq!(px(&e, 5, 4), red(), "nothing moved");
    assert!(!e.lift(), "a locked layer cannot be lifted");
}

#[test]
fn changing_tool_commits_and_undo_while_floating_only_cancels() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    moved(&mut e, 2, 0);
    e.undo();
    assert!(!e.is_floating());
    assert_eq!(px(&e, 1, 1), red(), "the drawing is still there");
    assert!(e.lift());
    moved(&mut e, 2, 0);
    e.set_tool(ToolKind::Pencil);
    assert!(!e.is_floating());
    assert_eq!(px(&e, 3, 1), red());
    e.undo();
    e.undo();
    assert_eq!(
        px(&e, 1, 1),
        t,
        "the next undos undo the move and the drawing"
    );
}

#[test]
fn dragging_a_corner_handle_scales_and_shift_keeps_the_proportion() {
    let mut e = editor();
    e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
    e.pointer_drag((6.0, 4.0));
    e.pointer_release();
    assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 4)));
    assert!(e.cancel_floating());
    e.options.constrain = true;
    e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
    e.pointer_drag((6.0, 4.0));
    e.pointer_release();
    assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 6)));
}

#[test]
fn dragging_around_the_center_rotates_and_shift_snaps_to_15_degrees() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    // 4×4 con el rojo en su esquina superior izquierda; centro (3, 3).
    e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
    e.press_transform(TransformHit::Rotate, (6.0, 3.0));
    e.pointer_drag((3.0, 6.0)); // de la derecha a abajo: +90°
    assert_eq!(
        e.floating().unwrap().rendered_with,
        RotationAlgorithm::Fast,
        "Fast while dragging"
    );
    e.pointer_release();
    assert_eq!(
        e.floating().unwrap().rendered_with,
        RotationAlgorithm::RotSprite,
        "the chosen rotation on release"
    );
    let b = e.transform_box().unwrap();
    assert!((b.angle - FRAC_PI_2).abs() < 1e-4, "{}", b.angle);
    assert!(e.commit_floating());
    assert_eq!(
        px(&e, 4, 1),
        red(),
        "the top-left corner went to the top-right"
    );
    assert_eq!(px(&e, 1, 1), t);
    e.options.constrain = true;
    e.press_transform(TransformHit::Rotate, (6.0, 3.0));
    e.pointer_drag((3.0 + 3.0 * 0.9, 3.0 + 3.0 * 0.44)); // unos 26°
    e.pointer_release();
    let b = e.transform_box().unwrap();
    assert!((b.angle - PI / 6.0).abs() < 1e-4, "{}", b.angle);
}

#[test]
fn moving_a_rotated_block_shifts_the_same_result() {
    let mut e = editor();
    e.press_transform(TransformHit::Rotate, (4.0, 1.5));
    e.pointer_drag((1.5 + 2.5 * 0.8, 1.5 + 2.5 * 0.6)); // unos 37°
    e.pointer_release();
    let before = e.floating().unwrap().clone();
    assert_eq!(before.rendered_with, RotationAlgorithm::RotSprite);
    e.press_transform(TransformHit::Move, (1.5, 1.5));
    e.pointer_drag((3.5, 2.5));
    let after = e.floating().unwrap();
    assert_eq!(
        after.rendered_with,
        RotationAlgorithm::RotSprite,
        "not Fast"
    );
    assert_eq!(after.rendered, before.rendered, "the same pixels");
    assert_eq!(after.at, (before.at.0 + 2, before.at.1 + 1));
    e.pointer_release();
}

#[test]
fn status_shows_size_angle_and_keys_while_floating() {
    let mut e = editor();
    assert_eq!(e.floating_status(), None);
    assert!(e.lift());
    place(
        &mut e,
        TransformBox {
            x0: 6,
            y0: 0,
            x1: 0,
            y1: 3,
            angle: FRAC_PI_2,
        },
    );
    assert_eq!(
        e.floating_status().as_deref(),
        Some("6×3  90°  Enter to apply · Esc to cancel")
    );
}

#[test]
fn changing_the_rotation_option_recomputes_the_floating_content() {
    let mut e = editor();
    // Lift, rotate about 30 degrees by dragging and release: renders with RotSprite.
    e.press_transform(TransformHit::Rotate, (4.0, 1.5));
    e.pointer_drag((1.5 + 2.5 * 0.866, 1.5 + 2.5 * 0.5));
    e.pointer_release();
    assert_eq!(
        e.floating().unwrap().rendered_with,
        RotationAlgorithm::RotSprite
    );
    // Switching the option re-renders the floating content with the new algorithm.
    e.options.rotation = RotationAlgorithm::Fast;
    e.refresh_floating(false);
    assert_eq!(e.floating().unwrap().rendered_with, RotationAlgorithm::Fast);
}

#[test]
fn a_handle_stops_at_the_size_limit() {
    let mut e = editor();
    e.press_transform(TransformHit::Handle(Handle::E), (3.0, 1.5));
    e.pointer_drag((5000.0, 1.5));
    assert_eq!(
        e.transform_box(),
        Some(TransformBox::new(0, 0, 3, 3)),
        "too wide: keeps the last valid size"
    );
    e.pointer_drag((100.0, 1.5));
    assert_eq!(e.transform_box().unwrap().width(), 100);
    e.pointer_release();
}

fn pxf(e: &Editor, frame: usize, x: i32, y: i32) -> PixelValue {
    e.sprite().pixel(LayerId(1), frame, x, y)
}

/// Tres frames con el punto rojo en (1, 1) (copias independientes) y el frame 0 activo.
fn three_frames() -> Editor {
    let mut e = editor();
    e.duplicate_frame();
    e.duplicate_frame();
    e.set_frame(0);
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
    e
}

#[test]
fn removing_the_frame_while_floating_commits_on_the_lifted_frame_first() {
    let mut e = three_frames();
    let t = e.sprite().color_mode.transparent();
    e.set_frame(1);
    assert!(e.lift());
    moved(&mut e, 3, 3);
    e.remove_frame();
    e.sync_floating();
    e.commit_floating();
    assert!(!e.is_floating());
    assert_eq!(e.frame_count(), 2);
    for f in 0..2 {
        assert_eq!(pxf(&e, f, 1, 1), red(), "frame {f} untouched");
        assert_eq!(pxf(&e, f, 4, 4), t, "frame {f} untouched");
    }
    e.undo();
    assert_eq!(e.frame_count(), 3);
    assert_eq!(pxf(&e, 1, 4, 4), red(), "the move was applied on its frame");
    assert_eq!(pxf(&e, 1, 1, 1), t);
}

#[test]
fn moving_the_frame_while_floating_keeps_the_content_on_the_lifted_frame() {
    let mut e = three_frames();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    moved(&mut e, 3, 3);
    e.move_frame(0, 2);
    e.sync_floating();
    e.commit_floating();
    assert!(!e.is_floating());
    // El frame levantado ahora es el 2.
    assert_eq!(pxf(&e, 2, 4, 4), red());
    assert_eq!(pxf(&e, 2, 1, 1), t);
    for f in 0..2 {
        assert_eq!(pxf(&e, f, 1, 1), red(), "frame {f} untouched");
        assert_eq!(pxf(&e, f, 4, 4), t, "frame {f} untouched");
    }
}

#[test]
fn a_palette_change_while_floating_commits_first() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    moved(&mut e, 3, 3);
    let mut colors = e.sprite().palette.colors().to_vec();
    colors.reverse();
    assert!(e.apply_palette(cs_core::Palette::new(colors)));
    assert!(!e.is_floating());
    assert_eq!(px(&e, 4, 4), red());
    assert_eq!(px(&e, 1, 1), t);
    e.undo(); // la paleta
    e.undo(); // el movimiento
    assert_eq!(px(&e, 1, 1), red());
    assert_eq!(px(&e, 4, 4), t);
}

#[test]
fn the_preview_is_cached_until_the_box_or_the_algorithm_changes() {
    let mut e = editor();
    assert!(e.lift());
    moved(&mut e, 1, 0);
    let builds = |e: &Editor| e.floating().unwrap().builds.get();
    let start = builds(&e);
    e.view();
    e.view();
    assert_eq!(builds(&e), start + 1, "an idle frame reuses the preview");
    let b = e.transform_box().unwrap().translated(1, 1);
    assert!(e.set_floating_box(b, false));
    e.view();
    e.view();
    assert_eq!(builds(&e), start + 2, "moving rebuilds it once");
    assert_eq!(shown(&e, 3, 2), red());
    e.options.rotation = RotationAlgorithm::Fast;
    e.refresh_floating(false);
    e.view();
    assert_eq!(builds(&e), start + 3, "another algorithm rebuilds it");
}

#[test]
fn commit_after_a_canvas_resize_cancels_without_a_wrong_size_selection() {
    let mut e = editor();
    assert!(e.lift());
    moved(&mut e, 1, 1);
    let version = e.doc_version;
    // Un cambio de tamaño que no pasa por `execute_doc` (no confirma antes).
    let resize = cs_commands::DocCommand::OnArtboard {
        id: e.active,
        cmd: Command::ResizeCanvas {
            width: 16,
            height: 16,
            offset: (0, 0),
        },
    };
    e.history.execute(&mut e.document, resize).unwrap();
    assert!(e.commit_floating());
    assert!(!e.is_floating());
    assert_eq!(e.doc_version, version, "nothing applied");
    assert!(
        e.selection
            .as_ref()
            .is_none_or(|m| (m.width(), m.height()) == (16, 16)),
        "no selection of the old size"
    );
    assert_eq!(px(&e, 1, 1), red());
}

#[test]
fn delete_while_floating_a_paste_discards_it_without_erasing_below() {
    let mut e = editor();
    let version = e.doc_version;
    // Un pegado transparente encima del punto rojo.
    let clip = crate::clipboard::Clip::from_rgba(3, 3, &[0; 3 * 3 * 4]).unwrap();
    e.paste(&clip);
    assert!(e.is_floating());
    e.clear_selection_content();
    assert!(!e.is_floating());
    assert_eq!(px(&e, 1, 1), red(), "the artwork under the paste stays");
    assert_eq!(e.doc_version, version, "no undo step");
}

#[test]
fn delete_while_floating_lifted_content_only_leaves_the_hole_in_one_step() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    assert!(e.lift());
    moved(&mut e, 3, 3);
    let version = e.doc_version;
    e.clear_selection_content();
    assert!(!e.is_floating());
    assert_eq!(e.doc_version, version + 1, "one undo step");
    assert_eq!(px(&e, 1, 1), t, "the lifted pixels are gone");
    assert_eq!(px(&e, 4, 4), t, "nothing stamped");
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((0, 0), (2, 2))), "the selection is the hole");
    e.undo();
    assert_eq!(px(&e, 1, 1), red());
}

#[test]
fn indexed_lift_move_and_commit_keep_the_indices() {
    let mut e = Editor::with_sprite(Sprite::new(
        8,
        8,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    ));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, PixelValue::Index(5)), (2, 1, PixelValue::Index(9))],
    });
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
    e.tool = ToolKind::RectSelect;
    assert!(e.lift());
    moved(&mut e, 3, 2);
    assert!(e.commit_floating());
    assert_eq!(px(&e, 4, 3), PixelValue::Index(5));
    assert_eq!(px(&e, 5, 3), PixelValue::Index(9));
    assert_eq!(px(&e, 1, 1), PixelValue::Index(0));
}

#[test]
fn undo_of_a_move_whose_hole_and_stamp_overlap_restores_everything() {
    let mut e = editor();
    let t = e.sprite().color_mode.transparent();
    let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, blue), (2, 2, blue)],
    });
    let before: Vec<PixelValue> = (0..8)
        .flat_map(|y| (0..8).map(move |x| (x, y)))
        .map(|(x, y)| px(&e, x, y))
        .collect();
    assert!(e.lift());
    moved(&mut e, 1, 0); // la mitad del bloque cae sobre su propio hueco
    assert!(e.commit_floating());
    assert_eq!(px(&e, 0, 0), t);
    assert_eq!(px(&e, 1, 0), blue);
    assert_eq!(px(&e, 2, 1), red());
    assert_eq!(px(&e, 3, 2), blue);
    e.undo();
    let after: Vec<PixelValue> = (0..8)
        .flat_map(|y| (0..8).map(move |x| (x, y)))
        .map(|(x, y)| px(&e, x, y))
        .collect();
    assert_eq!(after, before);
}

#[test]
fn copy_while_floating_applies_it_first() {
    let mut e = editor();
    assert!(e.lift());
    moved(&mut e, 3, 3);
    let clip = e.copy().expect("something selected");
    assert!(!e.is_floating(), "copy confirms first");
    assert_eq!(px(&e, 4, 4), red());
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
    e.paste(&clip);
    assert_eq!(
        e.transform_box(),
        Some(TransformBox::new(3, 3, 3, 3)),
        "copied from where it was moved"
    );
}

#[test]
fn handles_show_only_where_the_clipboard_works() {
    let mut e = editor();
    assert!(e.shows_handles());
    e.set_tool(ToolKind::Pencil);
    assert!(!e.shows_handles(), "only selection tools and Move");
    e.set_tool(ToolKind::Move);
    assert!(e.shows_handles());
    e.toggle_locked(LayerId(1));
    assert!(!e.shows_handles(), "not on a locked layer");
}
