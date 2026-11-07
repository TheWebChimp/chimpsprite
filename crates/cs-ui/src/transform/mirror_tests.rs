//! Selección espejo: transformar lo primario y sus reflejos.

use cs_commands::Command;
use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite, SymmetryAxes};

use super::*;
use crate::editor::ToolKind;
use crate::transform_box::{Handle, TransformHit};

const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
const BLUE: PixelValue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
const GREEN: PixelValue = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
const YELLOW: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 255, 0));

const V: SymmetryAxes = SymmetryAxes {
    vertical: true,
    ..SymmetryAxes::NONE
};

/// Un lienzo `w`×`h` con `pixels`, los ejes `axes`, la selección espejo prendida y Rect Select.
fn editor(w: u32, h: u32, pixels: &[(i32, i32, PixelValue)], axes: SymmetryAxes) -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(w, h, ColorMode::Rgba));
    e.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: pixels.to_vec(),
    });
    e.set_symmetry(cs_core::Symmetry {
        axes,
        center2: None,
    });
    e.options.mirror_selection = true;
    e.set_tool(ToolKind::RectSelect);
    e
}

fn px(e: &Editor, x: i32, y: i32) -> PixelValue {
    e.sprite().pixel(LayerId(1), 0, x, y)
}

fn clear(e: &Editor) -> PixelValue {
    e.sprite().color_mode.transparent()
}

/// Centro del píxel `p` (el artboard está en el origen).
fn at(p: (i32, i32)) -> (f32, f32) {
    (p.0 as f32 + 0.5, p.1 as f32 + 0.5)
}

/// Selecciona arrastrando de `a` a `b` (se refleja en los ejes).
fn select(e: &mut Editor, a: (i32, i32), b: (i32, i32)) {
    e.pointer_press(at(a), false, false, 0.1);
    e.pointer_drag(at(b));
    e.pointer_release();
}

/// Arrastra lo seleccionado (o un handle) de `from` a `to`, en coordenadas de mundo.
fn drag(e: &mut Editor, hit: TransformHit, from: (f32, f32), to: (f32, f32)) {
    e.press_transform(hit, from);
    e.drag_transform(to);
    e.release_transform();
}

#[test]
fn nudging_moves_each_side_like_a_mirror_with_its_own_pixels() {
    // 8×8, eje entre 3 y 4: ojo rojo a la izquierda, azul a la derecha.
    let mut e = editor(8, 8, &[(1, 1, RED), (6, 1, BLUE)], V);
    select(&mut e, (0, 0), (2, 2));
    let sel = e.selection.clone().unwrap();
    assert!(sel.contains(1, 1) && sel.contains(6, 1), "both sides");
    let b = e.transform_box().unwrap();
    assert_eq!(
        (b.left(), b.top(), b.width(), b.height()),
        (0, 0, 3, 3),
        "the primary only"
    );
    assert!(e.nudge(1, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 2, 1), RED, "the left eye moved right");
    assert_eq!(
        px(&e, 5, 1),
        BLUE,
        "the right eye moved left, keeping its color"
    );
    assert_eq!(px(&e, 1, 1), clear(&e));
    assert_eq!(px(&e, 6, 1), clear(&e));
    let sel = e.selection.clone().unwrap();
    assert!(
        sel.contains(2, 1) && sel.contains(5, 1),
        "the selection followed"
    );
    // Un solo paso de undo.
    e.undo();
    assert_eq!(px(&e, 1, 1), RED);
    assert_eq!(px(&e, 6, 1), BLUE);
    assert_eq!(px(&e, 2, 1), clear(&e));
}

#[test]
fn scaling_the_box_scales_the_reflection_toward_its_side() {
    let mut pixels = Vec::new();
    for y in 0..2 {
        pixels.extend([(0, y, RED), (1, y, RED), (6, y, BLUE), (7, y, BLUE)]);
    }
    let mut e = editor(8, 8, &pixels, V);
    select(&mut e, (0, 0), (1, 1));
    // El handle derecho de la caja (x = 2) hasta x = 4: el doble de ancho.
    drag(
        &mut e,
        TransformHit::Handle(Handle::E),
        (2.0, 1.0),
        (4.0, 1.0),
    );
    assert!(e.commit_floating());
    for x in 0..4 {
        assert_eq!(px(&e, x, 0), RED, "x = {x}");
        assert_eq!(px(&e, 7 - x, 1), BLUE, "x = {}", 7 - x);
    }
}

#[test]
fn flip_flips_each_side_in_place() {
    let pixels = [(0, 0, RED), (1, 0, GREEN), (6, 0, YELLOW), (7, 0, BLUE)];
    let mut e = editor(8, 8, &pixels, V);
    select(&mut e, (0, 0), (1, 0));
    assert!(e.flip(true));
    assert!(e.commit_floating());
    assert_eq!([px(&e, 0, 0), px(&e, 1, 0)], [GREEN, RED]);
    assert_eq!([px(&e, 6, 0), px(&e, 7, 0)], [BLUE, YELLOW]);
}

#[test]
fn a_piece_across_the_axis_only_moves_along_it() {
    let mouth: Vec<_> = (2..=5).map(|x| (x, 4, RED)).collect();
    let mut e = editor(8, 8, &mouth, V);
    select(&mut e, (2, 4), (5, 4));
    assert!(e.nudge(1, 1));
    assert!(e.commit_floating());
    for x in 2..=5 {
        assert_eq!(px(&e, x, 5), RED, "x = {x}: it went down, not right");
        assert_eq!(px(&e, x, 4), clear(&e));
    }
    assert_eq!(px(&e, 6, 5), clear(&e));
}

#[test]
fn widening_a_piece_across_the_axis_keeps_it_centered() {
    let mouth: Vec<_> = (3..=4).map(|x| (x, 4, RED)).collect();
    let mut e = editor(8, 8, &mouth, V);
    select(&mut e, (3, 4), (4, 4));
    // El handle derecho (x = 5) hasta x = 7: de 2 a 4 de ancho, centrado en el eje.
    drag(
        &mut e,
        TransformHit::Handle(Handle::E),
        (5.0, 4.5),
        (7.0, 4.5),
    );
    assert!(e.commit_floating());
    for x in 2..=5 {
        assert_eq!(px(&e, x, 4), RED, "x = {x}");
    }
    assert_eq!(px(&e, 6, 4), clear(&e));
    assert_eq!(px(&e, 1, 4), clear(&e));
}

#[test]
fn with_both_axes_the_mark_and_the_mouth_move_apart() {
    let axes = SymmetryAxes {
        vertical: true,
        horizontal: true,
        ..SymmetryAxes::NONE
    };
    let pixels = [(3, 2, RED), (4, 2, RED), (3, 5, BLUE), (4, 5, BLUE)];
    let mut e = editor(8, 8, &pixels, axes);
    select(&mut e, (3, 2), (4, 2));
    assert!(e.selection.as_ref().unwrap().contains(3, 5), "the twin");
    assert!(e.nudge(0, -1));
    assert!(e.commit_floating());
    assert_eq!([px(&e, 3, 1), px(&e, 4, 1)], [RED, RED]);
    assert_eq!([px(&e, 3, 6), px(&e, 4, 6)], [BLUE, BLUE]);
    assert_eq!(px(&e, 3, 2), clear(&e));
    assert_eq!(px(&e, 3, 5), clear(&e));
}

#[test]
fn a_diagonal_moves_the_reflection_across_it() {
    let axes = SymmetryAxes {
        diagonal_down: true,
        ..SymmetryAxes::NONE
    };
    let mut e = editor(8, 8, &[(1, 5, RED), (5, 1, BLUE)], axes);
    select(&mut e, (1, 5), (2, 5));
    assert!(e.nudge(1, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 2, 5), RED);
    assert_eq!(px(&e, 5, 2), BLUE, "→ becomes ↓ across ╲");
}

#[test]
fn grabbing_the_reflection_drags_it_under_the_pointer() {
    let mut e = editor(8, 8, &[(1, 1, RED), (6, 1, BLUE)], V);
    select(&mut e, (0, 0), (2, 2));
    // Agarrar el ojo derecho (sin levantar todavía): pasa a ser lo primario.
    drag(&mut e, TransformHit::Move, at((6, 1)), at((5, 1)));
    assert_eq!(e.transform_box().map(|b| b.left()), Some(4));
    // Ya flotando, agarrar el izquierdo y llevarlo a la izquierda: vuelven los dos.
    drag(&mut e, TransformHit::Move, at((2, 1)), at((1, 1)));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 1, 1), RED);
    assert_eq!(px(&e, 6, 1), BLUE);
    assert_eq!(px(&e, 2, 1), clear(&e));
}

#[test]
fn delete_clears_every_piece() {
    let mut e = editor(8, 8, &[(1, 1, RED), (6, 1, BLUE)], V);
    select(&mut e, (0, 0), (2, 2));
    assert!(e.lift());
    assert!(e.delete_floating());
    assert_eq!(px(&e, 1, 1), clear(&e));
    assert_eq!(px(&e, 6, 1), clear(&e));
}

#[test]
fn an_asymmetric_selection_moves_normally() {
    let mut e = editor(8, 8, &[(1, 1, RED), (6, 1, BLUE)], V);
    e.set_selection(Some(cs_core::Mask::from_rect(8, 8, (0, 0), (2, 2))));
    assert!(e.nudge(1, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 2, 1), RED);
    assert_eq!(px(&e, 6, 1), BLUE, "not part of the selection");
}

#[test]
fn where_the_halves_overlap_the_primary_wins_and_clear_never_covers() {
    // Ancho 7, eje en la columna 3: el rojo y el azul caen los dos ahí.
    let mut e = editor(7, 7, &[(2, 0, RED), (4, 0, BLUE)], V);
    select(&mut e, (1, 0), (2, 0));
    assert!(e.nudge(1, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 3, 0), RED, "the primary on top");
    // Lo transparente de lo primario no tapa lo opaco del reflejo.
    let mut e = editor(8, 8, &[(1, 0, RED), (6, 0, BLUE)], V);
    select(&mut e, (1, 0), (2, 0));
    assert!(e.nudge(2, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 3, 0), RED);
    assert_eq!(px(&e, 4, 0), BLUE, "the primary's clear pixel lands here");
}

#[test]
fn rotating_turns_the_reflection_the_other_way() {
    // Una L arriba a la izquierda y su reflejo.
    let pixels = [
        (0, 0, RED),
        (0, 1, RED),
        (1, 1, RED),
        (7, 0, BLUE),
        (7, 1, BLUE),
        (6, 1, BLUE),
    ];
    let mut e = editor(8, 8, &pixels, V);
    select(&mut e, (0, 0), (1, 1));
    assert!(e.rotate_content(crate::edit_ops::Turn::Clockwise));
    assert!(e.commit_floating());
    // Girar 90° a la derecha la L (0,0),(0,1),(1,1) en su caja de 2×2: (0,0),(1,0),(0,1).
    assert_eq!([px(&e, 0, 0), px(&e, 1, 0), px(&e, 0, 1)], [RED; 3]);
    assert_eq!(px(&e, 1, 1), clear(&e));
    // El reflejo gira al revés: el espejo de lo de arriba.
    assert_eq!([px(&e, 7, 0), px(&e, 6, 0), px(&e, 7, 1)], [BLUE; 3]);
    assert_eq!(px(&e, 6, 1), clear(&e));
}

#[test]
fn the_box_is_the_primary_and_the_mouth_on_the_axis_stays() {
    // 12×8, eje entre 5 y 6: ojos en (2,1) y (9,1), boca (5,6)-(6,6).
    let pixels = [(2, 1, RED), (9, 1, BLUE), (5, 6, GREEN), (6, 6, GREEN)];
    let mut e = editor(12, 8, &pixels, V);
    let points = [(2, 1), (9, 1), (5, 6), (6, 6)];
    e.set_selection(Some(cs_core::Mask::from_points(12, 8, points)));
    e.mirror_anchor = Some((5, 3));
    let b = e.transform_box().unwrap();
    assert_eq!(
        (b.left(), b.top(), b.width(), b.height()),
        (2, 1, 1, 1),
        "the left eye"
    );
    assert!(e.nudge(1, 0));
    assert!(e.commit_floating());
    assert_eq!(px(&e, 3, 1), RED);
    assert_eq!(px(&e, 8, 1), BLUE);
    assert_eq!(
        [px(&e, 5, 6), px(&e, 6, 6)],
        [GREEN, GREEN],
        "the mouth stays centered"
    );
}
