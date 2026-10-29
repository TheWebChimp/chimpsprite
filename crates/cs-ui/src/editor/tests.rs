use super::*;
use crate::tile_ops::test_util::{t, tile_at, tile_editor};
use cs_commands::ops;

fn editor() -> Editor {
    Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba))
}

/// Clic en coordenadas locales del artboard activo.
fn click(e: &mut Editor, p: Point, secondary: bool, alt: bool) {
    let (x, y) = e.artboard().position;
    let world = ((x + p.0) as f32 + 0.5, (y + p.1) as f32 + 0.5);
    e.pointer_press(world, secondary, alt, 0.1);
    e.pointer_release();
}

/// Arrastre en coordenadas locales (centro de píxel) con Shift/Alt al presionar.
fn drag_local(e: &mut Editor, a: Point, b: Point, shift: bool, alt: bool) {
    let at = |p: Point| (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
    e.options.constrain = shift;
    e.pointer_press(at(a), false, alt, 0.1);
    e.pointer_drag(at(b));
    e.pointer_release();
    e.options.constrain = false;
}

#[test]
fn sprite_menu_crop_trim_rotate_and_duplicate() {
    let mut e = editor();
    let red = PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    let layer = e.layer;
    e.execute(Command::DrawPixels {
        layer,
        frame: 0,
        pixels: vec![(2, 3, red), (5, 4, red)],
    });
    let start = e.artboard().position;
    assert!(e.trim_canvas());
    let s = e.sprite();
    assert_eq!((s.width, s.height), (4, 2));
    assert_eq!(s.pixel(layer, 0, 0, 0), red);
    assert_eq!(
        e.artboard().position,
        (start.0 + 2, start.1 + 3),
        "stays in the world"
    );
    assert!(!e.trim_canvas(), "nothing left to trim");
    assert!(e.rotate_canvas(sprite_ops::Rotation::Clockwise));
    assert_eq!((e.sprite().width, e.sprite().height), (2, 4));
    e.selection = Some(cs_core::Mask::from_points(2, 4, [(0, 0), (1, 1)]));
    assert!(e.crop_to_selection());
    assert_eq!((e.sprite().width, e.sprite().height), (2, 2));
    let before = e.document.artboards.len();
    assert!(e.duplicate_artboard("Copy".into()));
    assert_eq!(e.document.artboards.len(), before + 1);
    assert_eq!(e.artboard().name, "Copy");
    assert_eq!((e.sprite().width, e.sprite().height), (2, 2));
    e.undo();
    e.undo();
    e.undo();
    e.undo();
    assert_eq!(
        (e.sprite().width, e.sprite().height),
        (8, 8),
        "every step undoes"
    );
}

#[test]
fn mirror_selection_selects_and_moves_like_a_mirror() {
    let mut e = editor(); // 8×8: el eje vertical por defecto pasa por x = 4.
    let red = PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    let layer = e.layer;
    e.execute(Command::DrawPixels {
        layer,
        frame: 0,
        pixels: vec![(1, 1, red), (6, 1, red)],
    });
    e.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..Default::default()
        },
        center2: None,
    });
    e.options.mirror_selection = true;
    e.set_tool(ToolKind::RectSelect);
    // Seleccionar a la izquierda también selecciona el reflejo a la derecha.
    drag_local(&mut e, (0, 0), (2, 2), false, false);
    let sel = e.selection.clone().unwrap();
    assert!(sel.contains(1, 1) && sel.contains(6, 1), "both sides");
    assert!(!sel.contains(4, 1), "not the middle");
    // Moverlo 1 a la derecha: la mitad de la izquierda va a la derecha y la de la derecha a
    // la izquierda (las dos se acercan al eje, como en un espejo).
    assert!(e.nudge(1, 0));
    e.commit_floating();
    let s = e.sprite();
    assert_eq!(s.pixel(layer, 0, 2, 1), red, "the left half moved right");
    assert_eq!(s.pixel(layer, 0, 5, 1), red, "the right half moved left");
    assert_ne!(s.pixel(layer, 0, 1, 1), red);
    assert_ne!(s.pixel(layer, 0, 6, 1), red);
    let sel = e.selection.clone().unwrap();
    assert!(
        sel.contains(2, 1) && sel.contains(5, 1),
        "the selection followed"
    );
    // Sin la opción, mover es mover todo igual.
    e.options.mirror_selection = false;
    assert!(e.nudge(1, 0));
    e.commit_floating();
    assert_eq!(e.sprite().pixel(layer, 0, 3, 1), red);
    assert_eq!(e.sprite().pixel(layer, 0, 6, 1), red);
}

#[test]
fn selection_modes_from_modifiers() {
    let mut e = editor();
    e.tool = ToolKind::RectSelect;
    drag_local(&mut e, (0, 0), (3, 3), false, false);
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 16);
    drag_local(&mut e, (4, 0), (5, 0), true, false); // Shift: add
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 18);
    drag_local(&mut e, (0, 0), (1, 1), false, true); // Alt: subtract (no cuentagotas)
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 14);
    drag_local(&mut e, (2, 2), (7, 7), true, true); // Shift+Alt: intersect
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
    // Modo elegido en las opciones.
    e.options.selection_mode = SelectionMode::Add;
    drag_local(&mut e, (7, 7), (7, 7), false, false); // clic: no cambia en Add
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
    e.options.selection_mode = SelectionMode::Replace;
    drag_local(&mut e, (7, 7), (7, 7), false, false);
    assert!(e.selection.is_none(), "click deselects in Replace");
    e.reselect();
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
    e.invert_selection();
    assert_eq!(e.selection.as_ref().unwrap().points().count(), 60);
    e.deselect();
    e.invert_selection();
    assert_eq!(
        e.selection.as_ref().unwrap().points().count(),
        64,
        "inverse of nothing = all"
    );
}

#[test]
fn polygon_lasso_by_clicks_enter_and_escape() {
    let mut e = editor();
    e.set_tool(ToolKind::PolygonLasso);
    for p in [(0, 0), (6, 0), (6, 6)] {
        click(&mut e, p, false, false);
    }
    assert!(e.is_pending() && !e.is_painting());
    e.pointer_hover((0.5, 6.5));
    assert!(e.view().selection.is_some(), "preview while pending");
    e.finish_gesture();
    assert!(!e.is_pending());
    assert!(e.selection.as_ref().unwrap().contains(5, 1));
    click(&mut e, (1, 1), false, false);
    e.cancel_gesture();
    assert!(!e.is_pending());
    // Cambiar de herramienta descarta el gesto a medias.
    click(&mut e, (1, 1), false, false);
    e.set_tool(ToolKind::Pencil);
    e.set_tool(ToolKind::PolygonLasso);
    assert!(!e.is_pending());
}

#[test]
fn text_draft_previews_commits_and_cancels() {
    let mut e = editor();
    e.set_tool(ToolKind::Text);
    click(&mut e, (1, 0), false, false);
    e.text.as_mut().unwrap().text = "I".into();
    assert!(!e.history.can_undo());
    let preview = e.view().document.artboards[0].sprite.clone();
    assert!(preview.cels.contains_key(&(LayerId(1), 0)), "preview draws");
    click(&mut e, (2, 0), false, false); // mueve el texto
    e.commit_text();
    assert!(e.text.is_none());
    let px = e.sprite().pixel(LayerId(1), 0, 4, 3);
    assert_eq!(px, e.primary, "the I stem at x = 2 + 2");
    e.undo();
    assert!(e.sprite().cels.is_empty());
    click(&mut e, (0, 0), false, false);
    e.cancel_gesture();
    assert!(e.text.is_none());
}

#[test]
fn spray_keeps_painting_while_held_and_gradient_fills() {
    let mut e = editor();
    e.set_tool(ToolKind::Spray);
    e.options.spray_width = 4;
    e.pointer_press((4.5, 4.5), false, false, 0.1);
    e.tool_tick(1000.0);
    e.pointer_release();
    let painted = (0..8)
        .flat_map(|y| (0..8).map(move |x| (x, y)))
        .filter(|&(x, y)| e.sprite().pixel(LayerId(1), 0, x, y) == e.primary)
        .count();
    assert!(painted > 3, "{painted}");

    let mut e = editor();
    e.set_tool(ToolKind::Gradient);
    e.options.dither = Dither::Bayer2;
    drag_local(&mut e, (0, 0), (7, 0), false, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 0, 5), e.primary);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 7, 5), e.secondary);
}

fn two_boards() -> Editor {
    let mut e = editor();
    e.add_artboard(4, 4);
    e
}

#[test]
fn execute_undo_redo_bump_revision() {
    let mut e = editor();
    let r0 = e.revision;
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, e.primary)],
    });
    assert!(e.revision > r0 && e.history.can_undo());
    e.undo();
    assert!(e.sprite().cels.is_empty());
    e.redo();
    assert!(!e.sprite().cels.is_empty());
}

#[test]
fn the_other_button_cancels_the_stroke() {
    let mut e = editor();
    // Una herramienta que pinta con los dos botones (con el lápiz, el derecho es cuentagotas).
    e.set_tool(ToolKind::Line);
    let at = |p: Point| (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
    for secondary in [false, true] {
        e.pointer_press(at((1, 1)), secondary, false, 0.1);
        e.pointer_drag(at((5, 1)));
        assert!(
            !e.cancel_with_other_button(secondary),
            "the same button does nothing"
        );
        assert!(e.cancel_with_other_button(!secondary));
        assert!(!e.is_painting());
        e.pointer_release();
        assert!(e.sprite().cels.is_empty(), "nothing painted");
        assert!(!e.history.can_undo(), "nothing to undo");
    }
    // Sin trazo, el otro botón no corta nada.
    assert!(!e.cancel_with_other_button(true));
}

#[test]
fn stroke_paints_the_edge_of_the_selection_with_the_primary_color() {
    let mut e = editor();
    e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (5, 4))));
    e.stroke_selection();
    let t = e.sprite().color_mode.transparent();
    let at = |e: &Editor, x, y| e.sprite().pixel(LayerId(1), 0, x, y);
    for (x, y) in [
        (1, 1),
        (5, 1),
        (1, 4),
        (5, 4),
        (3, 1),
        (1, 2),
        (5, 3),
        (3, 4),
    ] {
        assert_eq!(at(&e, x, y), e.primary, "edge ({x}, {y})");
    }
    for (x, y) in [(2, 2), (4, 3), (0, 0), (6, 4)] {
        assert_eq!(at(&e, x, y), t, "not the edge ({x}, {y})");
    }
    e.undo();
    assert_eq!(at(&e, 1, 1), t, "one undo step");
    // Sin selección no hace nada.
    e.deselect();
    e.stroke_selection();
    assert!(!e.history.can_undo() || at(&e, 1, 1) == t);
}

#[test]
fn secondary_button_paints_secondary_color() {
    let mut e = editor();
    // Con el lápiz el derecho es el cuentagotas; con las demás, pinta el secundario.
    e.set_tool(ToolKind::Line);
    click(&mut e, (1, 1), true, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), e.secondary);
}

#[test]
fn painting_is_clipped_to_selection() {
    let mut e = editor();
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
    e.tool = ToolKind::Bucket;
    e.options.contiguous = false;
    click(&mut e, (0, 0), false, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), e.primary);
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 2, 2),
        e.sprite().color_mode.transparent()
    );
}

#[test]
fn alt_click_picks_color_and_restores_tool() {
    let mut e = editor();
    let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(3, 3, green)],
    });
    click(&mut e, (3, 3), false, true);
    assert_eq!(e.primary, green);
    assert_eq!(e.active_tool(), ToolKind::Pencil);
    e.tool = ToolKind::Eyedropper;
    click(&mut e, (0, 0), true, false);
    assert_eq!(e.secondary, e.sprite().color_mode.transparent());
}

#[test]
fn the_eyedropper_switches_the_color_editor_to_the_color_it_picks() {
    let mut e = editor();
    e.tool = ToolKind::Eyedropper;
    e.editing_secondary = true;
    e.palette_entry = [Some(4), Some(5)];
    click(&mut e, (0, 0), false, false);
    assert!(!e.editing_secondary, "left: the primary");
    assert_eq!(e.palette_entry[0], None, "no swatch was picked");
    click(&mut e, (0, 0), true, false);
    assert!(e.editing_secondary, "right: the secondary");
    assert_eq!(e.palette_entry[1], None);
}

#[test]
fn delete_clears_selected_content() {
    let mut e = editor();
    e.tool = ToolKind::Bucket;
    click(&mut e, (0, 0), false, false);
    e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (3, 3))));
    e.clear_selection_content();
    let t = e.sprite().color_mode.transparent();
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), t);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 0, 0), e.primary);
}

#[test]
fn delete_in_tiles_mode_clears_the_selected_cells_in_one_undo_step() {
    use crate::tile_ops::test_util::{t, tile_at, tile_editor};
    let mut e = tile_editor();
    let cmd = cs_commands::ops::set_tiles(
        e.sprite(),
        e.layer,
        0,
        &[(0, 0, t(1)), (1, 0, t(2)), (3, 3, t(1))],
    )
    .unwrap();
    e.execute(cmd);
    // Toca las celdas (0, 0) y (1, 0).
    e.set_selection(Some(Mask::from_rect(16, 16, (3, 1), (5, 1))));
    e.clear_selection_content();
    assert_eq!(tile_at(&e, (0, 0)), cs_core::Tile::EMPTY);
    assert_eq!(tile_at(&e, (1, 0)), cs_core::Tile::EMPTY);
    assert_eq!(tile_at(&e, (3, 3)), t(1), "outside the selection");
    e.undo();
    assert_eq!(tile_at(&e, (0, 0)), t(1));
    assert_eq!(tile_at(&e, (1, 0)), t(2));
}

#[test]
fn delete_does_nothing_on_a_locked_tilemap_layer() {
    use crate::tile_ops::test_util::{t, tile_at, tile_editor};
    let mut e = tile_editor();
    let cmd = cs_commands::ops::set_tiles(e.sprite(), e.layer, 0, &[(0, 0, t(1))]).unwrap();
    e.execute(cmd);
    e.execute(Command::SetLayerProps {
        id: e.layer,
        props: cs_commands::LayerProps {
            locked: Some(true),
            ..Default::default()
        },
    });
    e.set_selection(Some(Mask::all(16, 16)));
    let version = e.doc_version;
    e.clear_selection_content();
    assert_eq!(tile_at(&e, (0, 0)), t(1), "the locked layer is untouched");
    assert_eq!(e.doc_version, version, "no command ran");
    e.undo();
    assert!(
        !e.sprite().layer(e.layer).unwrap().locked,
        "the last undo step was the lock, not a clear"
    );
}

#[test]
fn preview_shows_stroke_without_touching_document() {
    let mut e = editor();
    e.pointer_press((0.5, 0.5), false, false, 0.1);
    let view = e.view();
    let preview = &view.document.artboards[0].sprite;
    assert_eq!(preview.pixel(LayerId(1), 0, 0, 0), e.primary);
    drop(view);
    assert!(e.sprite().cels.is_empty());
    e.pointer_release();
    assert!(!e.sprite().cels.is_empty());
}

#[test]
fn drawing_only_touches_active_artboard_and_click_activates() {
    let mut e = two_boards();
    let second = e.active;
    click(&mut e, (0, 0), false, false);
    assert!(e.document.artboards[0].sprite.cels.is_empty());
    assert!(!e.document.artboards[1].sprite.cels.is_empty());
    // Clic sobre el primer artboard: lo activa y pinta ahí.
    e.pointer_press((0.5, 0.5), false, false, 0.1);
    e.pointer_release();
    assert_ne!(e.active, second);
    assert!(!e.document.artboards[0].sprite.cels.is_empty());
}

#[test]
fn undo_is_chronological_across_artboards() {
    let mut e = two_boards();
    click(&mut e, (0, 0), false, false);
    e.activate(e.document.artboards[0].id);
    click(&mut e, (0, 0), false, false);
    e.undo();
    assert!(e.document.artboards[0].sprite.cels.is_empty());
    assert!(!e.document.artboards[1].sprite.cels.is_empty());
    e.undo();
    assert!(e.document.artboards[1].sprite.cels.is_empty());
    e.undo();
    assert_eq!(e.document.artboards.len(), 1);
    assert_eq!(e.active, e.document.artboards[0].id);
}

#[test]
fn activating_drops_selection_and_remembers_layer() {
    let mut e = two_boards();
    let (first, second) = (e.document.artboards[0].id, e.document.artboards[1].id);
    e.execute(Command::AddLayer {
        index: 1,
        layer: cs_core::Layer::new_image(LayerId(2), "top"),
    });
    e.layer = LayerId(2);
    e.select_all();
    e.activate(first);
    assert!(e.selection.is_none());
    assert_eq!(e.layer, LayerId(1));
    e.activate(second);
    assert_eq!(e.layer, LayerId(2));
}

#[test]
fn cannot_remove_last_artboard() {
    let mut e = editor();
    e.remove_active_artboard();
    assert_eq!(e.document.artboards.len(), 1);
}

#[test]
fn artboard_tool_creates_and_activates() {
    let mut e = editor();
    e.tool = ToolKind::Artboard;
    e.pointer_press((20.0, 0.0), false, false, 0.1);
    e.pointer_drag((29.0, 9.0));
    e.pointer_release();
    assert_eq!(e.document.artboards.len(), 2);
    assert_eq!(e.active, e.document.artboards[1].id);
    assert_eq!((e.sprite().width, e.sprite().height), (10, 10));
}

#[test]
fn dragging_label_moves_artboard_with_any_tool() {
    let mut e = two_boards();
    let first = e.document.artboards[0].id;
    e.tool = ToolKind::Pencil;
    e.press_artboard_label(first, (1.0, -2.0));
    assert!(e.is_painting());
    e.pointer_drag((6.0, -2.0));
    e.pointer_release();
    assert_eq!(e.active, first);
    assert_eq!(e.artboard().position, (5, 0));
    assert!(e.sprite().cels.is_empty(), "no debe pintar");
    e.undo();
    assert_eq!(e.artboard().position, (0, 0));
}

#[test]
fn shape_tools_snap_to_guides_but_pencil_does_not() {
    use cs_core::{Axis, Guide};
    let mut e = editor();
    e.set_guides(vec![Guide {
        axis: Axis::Vertical,
        position: 4,
    }]);
    e.snap = Some(crate::guides::SnapContext {
        guides: e.guides().to_vec(),
        grid: None,
        tolerance: 1.0,
    });
    e.tool = ToolKind::RectSelect;
    e.pointer_press((0.5, 0.5), false, false, 0.1);
    e.pointer_drag((4.3, 2.5)); // cerca de x = 4 → píxel 4 (lado del puntero)
    e.pointer_release();
    assert_eq!(
        e.selection.as_ref().unwrap().bounds(),
        Some(((0, 0), (4, 2)))
    );
    e.deselect();
    e.tool = ToolKind::Pencil;
    e.pointer_press((3.6, 1.5), false, false, 0.1);
    e.pointer_release();
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 3, 1),
        e.primary,
        "pencil ignores snap"
    );
    e.undo();
    e.undo();
    assert!(e.guides().is_empty(), "guides are undoable");
}

#[test]
fn moving_the_selection_snaps_to_guides_with_any_selection_tool_and_move() {
    use cs_core::{Axis, Guide};
    // Lasso no pega sus trazos, pero mover lo seleccionado sí (como Move).
    for tool in [ToolKind::RectSelect, ToolKind::Lasso, ToolKind::Move] {
        let mut e = editor();
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        e.set_guides(vec![Guide {
            axis: Axis::Vertical,
            position: 4,
        }]);
        e.snap = Some(crate::guides::SnapContext {
            guides: e.guides().to_vec(),
            grid: None,
            tolerance: 2.0,
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.set_tool(tool);
        // Sin pegar el puntero caería en x = 5 (4 px); la guía en x = 4 lo deja en 3 px.
        e.pointer_press((1.5, 1.5), false, false, 0.1);
        e.pointer_drag((5.5, 1.5));
        e.pointer_release();
        assert!(e.is_floating(), "{tool:?}");
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((3, 0), (5, 2))), "{tool:?}");
        assert!(e.commit_floating());
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 4, 1), red, "{tool:?}");
    }
}

/// Editor 16×16 con un punto rojo en (3, 1), la selección (2, 0)–(4, 2), una guía vertical en
/// x = 4 que pega a 2 px y RectSelect.
fn guided_selection() -> Editor {
    use cs_core::{Axis, Guide};
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(3, 1, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
    });
    e.set_guides(vec![Guide {
        axis: Axis::Vertical,
        position: 4,
    }]);
    e.snap = Some(crate::guides::SnapContext {
        guides: e.guides().to_vec(),
        grid: None,
        tolerance: 2.0,
    });
    e.set_selection(Some(Mask::from_rect(16, 16, (2, 0), (4, 2))));
    e.set_tool(ToolKind::RectSelect);
    e
}

#[test]
fn a_drag_held_still_by_snapping_is_not_a_click() {
    let mut e = guided_selection();
    let before = e.selection.clone();
    let version = e.doc_version;
    // Presionar y arrastrar un píxel cerca de la guía: el ajuste deja la caja quieta.
    e.pointer_press((4.5, 1.5), false, false, 0.1);
    e.pointer_drag((5.5, 1.5));
    e.pointer_release();
    assert_eq!(e.selection, before, "not deselected");
    assert!(!e.is_floating(), "nothing lifted");
    assert_eq!(e.doc_version, version);
}

#[test]
fn moving_the_selection_measures_from_the_snapped_press_point() {
    let mut e = guided_selection();
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    // El punto presionado (x = 2.5, píxel 2) pega a la guía y cae en el píxel 3; el soltado
    // (x = 7.5) queda lejos: se mueve 4 px, no 5.
    e.pointer_press((2.5, 1.5), false, false, 0.1);
    e.pointer_drag((7.5, 1.5));
    e.pointer_release();
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((6, 0), (8, 2))));
    assert!(e.commit_floating());
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 7, 1), red);
}

#[test]
fn symmetric_strokes_are_one_undo_step_and_respect_selection() {
    use cs_core::{Symmetry, SymmetryAxes};
    let mut e = editor(); // 8×8
    e.set_symmetry(Symmetry {
        axes: SymmetryAxes {
            vertical: true,
            ..SymmetryAxes::NONE
        },
        center2: None,
    });
    click(&mut e, (1, 1), false, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 6, 1), e.primary);
    e.undo();
    assert!(e.sprite().cels.is_empty(), "both sides undone together");
    // Con selección solo en la mitad izquierda, el reflejo queda fuera.
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (3, 7))));
    click(&mut e, (2, 2), false, false);
    let t = e.sprite().color_mode.transparent();
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), e.primary);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 2), t);
}

#[test]
fn move_tool_picks_the_layer_under_the_pointer() {
    let mut e = editor();
    let bottom = LayerId(1);
    e.execute(Command::DrawPixels {
        layer: bottom,
        frame: 0,
        pixels: vec![(1, 1, e.primary)],
    });
    e.execute(Command::AddLayer {
        index: 1,
        layer: cs_core::Layer::new_image(LayerId(2), "top"),
    });
    e.execute(Command::DrawPixels {
        layer: LayerId(2),
        frame: 0,
        pixels: vec![(5, 5, e.primary)],
    });
    e.layer = bottom;
    e.tool = ToolKind::Move;
    // Clic sobre lo de la capa de arriba: cambia a ella y la mueve.
    e.pointer_press((5.5, 5.5), false, false, 0.1);
    assert_eq!(e.layer, LayerId(2));
    e.pointer_drag((6.5, 5.5));
    e.pointer_release();
    assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (1, 0));
    assert_eq!(e.sprite().cel(bottom, 0).unwrap().position, (0, 0));
    // Clic en vacío: se queda en la capa actual.
    e.pointer_press((7.5, 0.5), false, false, 0.1);
    e.pointer_release();
    assert_eq!(e.layer, LayerId(2));
    // Las capas bloqueadas u ocultas no se eligen.
    e.toggle_locked(LayerId(2));
    e.pointer_press((1.5, 1.5), false, false, 0.1);
    e.pointer_release();
    assert_eq!(e.layer, bottom);
    e.toggle_visible(bottom);
    e.layer = LayerId(2);
    assert_eq!(e.layer_at((1, 1)), None);
}

#[test]
fn dragging_inside_the_selection_with_a_selection_tool_moves_its_content() {
    let mut e = editor();
    let red = e.primary;
    let t = e.sprite().color_mode.transparent();
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, red)],
    });
    e.set_tool(ToolKind::RectSelect);
    drag_local(&mut e, (0, 0), (2, 2), false, false);
    // Arrastrar desde dentro: el contenido flota y se mueve, sin cambiar de herramienta.
    drag_local(&mut e, (1, 1), (4, 3), false, false);
    assert_eq!(e.tool, ToolKind::RectSelect);
    assert_eq!(e.active_tool(), ToolKind::RectSelect);
    assert!(e.is_floating(), "it floats until it is applied");
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 1, 1),
        red,
        "not stamped yet"
    );
    assert!(e.commit_floating());
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), t);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 4, 3), red);
    let bounds = e.selection.as_ref().and_then(|m| m.bounds());
    assert_eq!(bounds, Some(((3, 2), (5, 4))));
    e.undo();
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
    // Con Shift (Add) dentro de la selección se selecciona, no se mueve.
    drag_local(&mut e, (4, 3), (6, 6), true, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
    assert!(e.selection.as_ref().unwrap().contains(6, 6));
    assert!(!e.is_floating());
    // Un clic sin arrastrar dentro de la selección deselecciona, como siempre.
    click(&mut e, (4, 4), false, false);
    assert!(e.selection.is_none());
}

#[test]
fn moving_the_selection_twice_only_carries_what_was_selected_first() {
    // Bug del 2026-10-23: cada arrastre estampaba en la capa y el siguiente levantaba también
    // el dibujo que había quedado dentro de la selección.
    let mut e = editor();
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
    let t = e.sprite().color_mode.transparent();
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, red), (5, 1, blue)],
    });
    e.set_tool(ToolKind::RectSelect);
    drag_local(&mut e, (0, 0), (2, 2), false, false);
    drag_local(&mut e, (1, 1), (5, 1), false, false); // el rojo pasa encima del azul
    let shown = e.view().document.artboards[0]
        .sprite
        .pixel(LayerId(1), 0, 5, 1);
    assert_eq!(shown, red, "the floating content is drawn on top");
    drag_local(&mut e, (5, 1), (5, 5), false, false); // y se sigue moviendo
    assert!(e.commit_floating());
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 5), red);
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 5, 1),
        blue,
        "the drawing below stays"
    );
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), t);
    e.undo();
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 1, 1),
        red,
        "a single undo step"
    );
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 5), t);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 1), blue);
}

#[test]
fn the_move_tool_floats_the_selection_and_a_click_outside_applies_it() {
    let mut e = editor();
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, red)],
    });
    e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
    e.set_tool(ToolKind::Move);
    drag_local(&mut e, (6, 6), (7, 6), false, false); // desde fuera de la selección
    assert!(e.is_floating());
    assert_eq!(
        e.sprite().pixel(LayerId(1), 0, 1, 1),
        red,
        "not stamped yet"
    );
    click(&mut e, (6, 0), false, false); // fuera de lo flotante
    assert!(!e.is_floating());
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 1), red);
}

#[test]
fn painting_with_the_mask_color_erases() {
    let mut e = editor();
    let red = e.primary;
    click(&mut e, (2, 2), false, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), red);
    e.secondary = e.mask_value();
    assert!(e.is_mask(e.secondary) && !e.is_mask(e.primary));
    e.set_tool(ToolKind::Line);
    click(&mut e, (2, 2), true, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), e.mask_value());

    let indexed = Editor::with_sprite(Sprite::new(
        4,
        4,
        ColorMode::Indexed {
            transparent_index: 3,
        },
    ));
    assert_eq!(indexed.mask_value(), PixelValue::Index(3));
}

#[test]
fn indexed_sprite_uses_palette_indices() {
    let e = Editor::with_sprite(Sprite::new(
        4,
        4,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    ));
    // El negro de DB32 es el 0, que aquí es el transparente: el negro siguiente (el 1, casi
    // negro) y la máscara de secundario.
    assert_eq!(e.primary, PixelValue::Index(1));
    assert_eq!(e.secondary, PixelValue::Index(0));
}

#[test]
fn tiles_mode_pencil_paints_the_brush_tile_with_flips_and_previews_it() {
    let mut e = tile_editor();
    assert!(e.tiles_mode());
    assert_eq!(e.brush_tile(), t(1), "the brush starts on tile 1");
    let flipped = Tile::new(2, true, false, false);
    e.set_brush_tile(flipped);
    e.pointer_press((5.5, 9.5), false, false, 0.1); // celda (1, 2)
    let preview = e.view().document.artboards[0].sprite.clone();
    let shown = TileProxy::build(&preview, LayerId(2), 0, None).unwrap();
    assert_eq!(shown.tile_at((1, 2)), flipped, "the preview shows the tile");
    assert!(e.sprite().cel(LayerId(2), 0).is_none(), "preview only");
    e.pointer_release();
    assert_eq!(tile_at(&e, (1, 2)), flipped);
    assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
    e.undo();
    assert!(e.sprite().cel(LayerId(2), 0).is_none());
}

#[test]
fn tiles_mode_shapes_are_exact_in_cells() {
    let mut e = tile_editor();
    e.set_tool(ToolKind::Line);
    drag_local(&mut e, (1, 1), (13, 1), false, false);
    for cx in 0..4 {
        assert_eq!(tile_at(&e, (cx, 0)), t(1));
    }
    assert_eq!(tile_at(&e, (0, 1)), Tile::EMPTY);
    e.undo();
    e.set_tool(ToolKind::Rectangle);
    e.options.filled = true;
    drag_local(&mut e, (1, 1), (9, 9), false, false);
    let proxy = TileProxy::build(e.sprite(), LayerId(2), 0, None).unwrap();
    let filled: Vec<Point> = (0..4)
        .flat_map(|y| (0..4).map(move |x| (x, y)))
        .filter(|&c| proxy.tile_at(c) == t(1))
        .collect();
    assert_eq!(
        filled,
        vec![
            (0, 0),
            (1, 0),
            (2, 0),
            (0, 1),
            (1, 1),
            (2, 1),
            (0, 2),
            (1, 2),
            (2, 2)
        ]
    );
}

#[test]
fn tiles_mode_bucket_fills_equal_tiles_contiguous_or_not() {
    let mut e = tile_editor();
    let flipped = Tile::new(1, true, false, false);
    let cells = [
        (0, 0, t(1)),
        (1, 0, t(1)),
        (2, 0, t(2)),
        (3, 0, t(1)),
        (0, 1, flipped),
    ];
    let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &cells).unwrap();
    e.execute(cmd);
    let row = |e: &Editor| (0..4).map(|x| tile_at(e, (x, 0))).collect::<Vec<_>>();
    e.set_tool(ToolKind::Bucket);
    e.set_brush_tile(t(2));
    click(&mut e, (5, 1), false, false); // celda (1, 0)
    assert_eq!(row(&e), vec![t(2), t(2), t(2), t(1)]);
    assert_eq!(
        tile_at(&e, (0, 1)),
        flipped,
        "flips count: not the same tile"
    );
    e.undo();
    e.options.contiguous = false;
    click(&mut e, (5, 1), false, false);
    assert_eq!(row(&e), vec![t(2); 4]);
    assert_eq!(tile_at(&e, (0, 1)), flipped);
    assert_eq!(tile_at(&e, (1, 1)), Tile::EMPTY);
}

#[test]
fn tiles_mode_eyedropper_picks_the_tile_and_its_flips() {
    let mut e = tile_editor();
    let flipped = Tile::new(2, false, true, false);
    let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &[(2, 0, flipped)]).unwrap();
    e.execute(cmd);
    e.set_tool(ToolKind::Eyedropper);
    click(&mut e, (9, 1), false, false);
    assert_eq!(e.brush_tile(), flipped);
    click(&mut e, (1, 13), false, false);
    assert_eq!(
        e.brush_tile(),
        Tile::EMPTY,
        "clicking an empty cell picks the empty tile"
    );
}

#[test]
fn tiles_mode_eraser_leaves_the_empty_tile() {
    let mut e = tile_editor();
    click(&mut e, (1, 1), false, false);
    assert_eq!(tile_at(&e, (0, 0)), t(1));
    e.set_tool(ToolKind::Eraser);
    click(&mut e, (2, 2), false, false);
    assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
}

#[test]
fn tiles_mode_marquee_selects_whole_cells_and_limits_painting() {
    let mut e = tile_editor();
    e.set_tool(ToolKind::RectSelect);
    drag_local(&mut e, (1, 1), (5, 5), false, false);
    let sel = e.selection.clone().unwrap();
    assert_eq!(sel.bounds(), Some(((0, 0), (7, 7))));
    assert_eq!(sel.points().count(), 64);
    e.set_tool(ToolKind::Pencil);
    drag_local(&mut e, (1, 1), (13, 1), false, false);
    assert_eq!(tile_at(&e, (1, 0)), t(1));
    assert_eq!(tile_at(&e, (2, 0)), Tile::EMPTY, "outside the selection");
}

#[test]
fn tiles_mode_move_steps_by_whole_tiles() {
    let mut e = tile_editor();
    click(&mut e, (1, 1), false, false); // tile 1 en la celda (0, 0)
    e.set_tool(ToolKind::Move);
    drag_local(&mut e, (1, 1), (2, 1), false, false); // 1 px: menos de medio tile
    assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (0, 0));
    drag_local(&mut e, (1, 1), (4, 2), false, false); // 3 px → 1 tile; 1 px → 0
    assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (4, 0));
    assert_eq!(tile_at(&e, (1, 0)), t(1));
    assert_eq!(e.layer, LayerId(2), "Move keeps the tilemap layer");
}

#[test]
fn tiles_mode_blocks_spray_and_text() {
    let mut e = tile_editor();
    e.set_tool(ToolKind::Spray);
    e.pointer_press((4.5, 4.5), false, false, 0.1);
    e.tool_tick(1000.0);
    e.pointer_release();
    assert_eq!(
        e.tiles_status().as_deref(),
        Some("Not available in Tiles mode")
    );
    e.set_tool(ToolKind::Text);
    click(&mut e, (1, 1), false, false);
    assert!(e.text.is_none());
    assert!(!e.history.can_undo(), "no commands");
    e.set_tool(ToolKind::Pencil);
    assert_eq!(
        e.tiles_status().as_deref(),
        Some("Tiles mode: painting with tile 1")
    );
    // Numeración del usuario: índice + base_index - 1.
    let mut ts = e.sprite().tilesets[0].clone();
    ts.base_index = 5;
    e.execute(Command::SetTileset {
        index: 0,
        tileset: Some(ts),
    });
    e.set_brush_tile(t(2));
    assert_eq!(
        e.tiles_status().as_deref(),
        Some("Tiles mode: painting with tile 6")
    );
}

#[test]
fn tiles_mode_reports_the_grid_limit() {
    use std::sync::Arc;

    use cs_core::{Image, Tileset};
    let mut s = Sprite::new(5000, 1, ColorMode::Rgba);
    let mut ts = Tileset::new("t", (1, 1), ColorMode::Rgba);
    ts.tiles.push(Arc::new(Image::new(1, 1, ColorMode::Rgba)));
    s.tilesets.push(ts);
    let add = ops::new_tilemap_layer(&s, Some(LayerId(1)), 0);
    add.apply(&mut s).unwrap();
    let mut e = Editor::with_sprite(s);
    e.select_layer(LayerId(2));
    e.set_tool(ToolKind::Line);
    // 5000 celdas de ancho: más que MAX_TILEMAP_SIDE (4096).
    drag_local(&mut e, (0, 0), (4999, 0), false, false);
    assert!(e.sprite().cel(LayerId(2), 0).is_none());
    assert_eq!(
        e.tiles_status().as_deref(),
        Some("Tiles mode: the tilemap would exceed its size limit")
    );
}

#[test]
fn tiles_mode_paints_the_partial_edge_cell_of_an_unaligned_cel_with_a_selection() {
    let mut e = tile_editor();
    // Un tile en la celda (1, 1) y el cel corrido 2 px: offset (2, 2), la celda -1 se ve en
    // los píxeles [0, 2) del borde.
    let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &[(1, 1, t(2))]).unwrap();
    e.execute(cmd);
    let mut cel = *e.sprite().cel(LayerId(2), 0).unwrap();
    cel.position = (cel.position.0 + 2, cel.position.1 + 2);
    e.execute(Command::SetCel {
        layer: LayerId(2),
        frame: 0,
        cel: Some(cel),
    });
    e.select_all();
    click(&mut e, (1, 1), false, false);
    assert_eq!(tile_at(&e, (-1, -1)), t(1), "the edge cell is painted");
    assert_eq!(tile_at(&e, (1, 1)), t(2));
    // El puntero sobre un tile corrido cae en su celda.
    click(&mut e, (6, 6), false, false);
    assert_eq!(tile_at(&e, (1, 1)), t(1));
}

#[test]
fn the_custom_brush_colors_are_converted_once_per_brush_and_document() {
    let mut e = editor();
    let red = Rgba::rgb(255, 0, 0);
    cs_raster::set_custom_brush(vec![((0, 0), red), ((1, 0), red)]);
    e.options.brush_shape = BrushShape::Custom;
    let image = |e: &Editor| e.context().brush_image.expect("with its colors");
    let first = image(&e);
    assert_eq!(first[0], ((0, 0), PixelValue::Rgba(red)));
    assert!(
        std::rc::Rc::ptr_eq(&first, &image(&e)),
        "the same, not rebuilt"
    );
    // Otro pincel: se rearma.
    cs_raster::set_custom_brush(vec![((0, 0), red)]);
    let second = image(&e);
    assert!(!std::rc::Rc::ptr_eq(&first, &second));
    assert_eq!(second.len(), 1);
    // Un cambio al documento (podría ser la paleta): también.
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(0, 0, PixelValue::Rgba(red))],
    });
    assert!(!std::rc::Rc::ptr_eq(&second, &image(&e)));
    // Sin sus colores, nada.
    e.options.brush_colors = false;
    assert!(e.context().brush_image.is_none());
}

#[test]
fn lock_alpha_paints_only_over_what_is_painted_keeping_its_alpha() {
    let mut e = editor();
    let half = PixelValue::Rgba(Rgba::new(255, 0, 0, 128));
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(1, 1, half)],
    });
    e.set_tool(ToolKind::Pencil);
    e.options.ink = Ink::LockAlpha;
    e.primary = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
    click(&mut e, (1, 1), false, false);
    click(&mut e, (2, 2), false, false);
    let s = e.sprite();
    assert_eq!(
        s.pixel(e.layer, 0, 1, 1),
        PixelValue::Rgba(Rgba::new(0, 0, 255, 128))
    );
    assert_eq!(
        s.pixel(e.layer, 0, 2, 2),
        s.color_mode.transparent(),
        "nothing there"
    );
}

#[test]
fn shading_moves_each_pixel_along_the_palette_range() {
    let mut e = editor();
    let value = |e: &Editor, i: u8| e.palette_value(i);
    let start = value(&e, 3);
    e.execute(Command::DrawPixels {
        layer: e.layer,
        frame: 0,
        pixels: vec![(1, 1, start), (2, 2, start)],
    });
    e.palette_selection = Some((2, 5));
    e.set_tool(ToolKind::Pencil);
    e.options.ink = Ink::Shading;
    click(&mut e, (1, 1), false, false);
    assert_eq!(
        e.sprite().pixel(e.layer, 0, 1, 1),
        value(&e, 4),
        "one step right"
    );
    click(&mut e, (2, 2), true, false);
    assert_eq!(
        e.sprite().pixel(e.layer, 0, 2, 2),
        value(&e, 2),
        "right-click: one back"
    );
    // En la punta de la rampa se queda; fuera de ella, nada.
    click(&mut e, (2, 2), true, false);
    assert_eq!(e.sprite().pixel(e.layer, 0, 2, 2), value(&e, 2));
    click(&mut e, (5, 5), false, false);
    let s = e.sprite();
    assert_eq!(s.pixel(e.layer, 0, 5, 5), s.color_mode.transparent());
}

#[test]
fn in_tiled_mode_painting_on_a_copy_paints_the_sprite() {
    let mut e = editor();
    e.set_tool(ToolKind::Pencil);
    e.options.pixel_perfect = false;
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    e.primary = red;
    let t = e.sprite().color_mode.transparent();
    // Sin Tiled Mode, fuera del sprite no pasa nada.
    click(&mut e, (9, 1), false, false);
    assert_eq!(e.sprite().pixel(e.layer, 0, 1, 1), t);
    // A lo ancho: x = 9 en un sprite de 8 es la x = 1.
    e.tiled = TiledMode::X;
    click(&mut e, (9, 1), false, false);
    assert_eq!(e.sprite().pixel(e.layer, 0, 1, 1), red);
    // Solo a lo ancho: lo de abajo no se envuelve.
    click(&mut e, (2, 10), false, false);
    assert_eq!(e.sprite().pixel(e.layer, 0, 2, 2), t);
    e.tiled = TiledMode::Both;
    click(&mut e, (-1, -1), false, false);
    assert_eq!(e.sprite().pixel(e.layer, 0, 7, 7), red);
}

#[test]
fn the_right_button_of_the_pencil_picks_the_color_under_it() {
    let mut e = editor();
    let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(3, 3, green)],
    });
    e.set_tool(ToolKind::Pencil);
    let secondary = e.secondary;
    click(&mut e, (3, 3), true, false);
    assert_eq!(e.primary, green, "the primary is the color under it");
    assert_eq!(e.secondary, secondary, "the secondary stays");
    assert_eq!(e.tool, ToolKind::Pencil, "still the pencil");
    // Y se sigue pintando con él.
    click(&mut e, (5, 5), false, false);
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 5), green);
}
