//! Tests del editor para la toolbar y las herramientas Hand, Contour y Blur (aparte de
//! `editor.rs`, que ya es muy grande).

use cs_core::{ColorMode, LayerId, PixelValue, Sprite};

use crate::editor::{Editor, ToolKind};

/// Editor de 8×8 RGBA con `tool` elegida.
pub(crate) fn editor(tool: ToolKind) -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    e.set_tool(tool);
    e
}

/// Centro del píxel local `p` (el artboard está en el origen).
pub(crate) fn at(p: (i32, i32)) -> (f32, f32) {
    (p.0 as f32 + 0.5, p.1 as f32 + 0.5)
}

/// Trazo por los píxeles `points` con el botón principal o el secundario.
pub(crate) fn stroke(e: &mut Editor, points: &[(i32, i32)], secondary: bool) {
    e.pointer_press(at(points[0]), secondary, false, 0.1);
    for &p in &points[1..] {
        e.pointer_drag(at(p));
    }
    e.pointer_release();
}

pub(crate) fn px(e: &Editor, x: i32, y: i32) -> PixelValue {
    e.sprite().pixel(LayerId(1), 0, x, y)
}

#[test]
fn thirteen_groups_in_the_toolbar_order() {
    use ToolKind::*;
    let first: Vec<ToolKind> = ToolKind::GROUPS.iter().map(|g| g[0]).collect();
    assert_eq!(
        first,
        vec![
            RectSelect, Hand, Pencil, Eraser, Eyedropper, Bucket, Line, Rectangle, Text, Contour,
            Blur, Move, Artboard
        ]
    );
    assert_eq!(ToolKind::GROUPS[5], &[Bucket, Gradient, Jumble]);
    assert_eq!(ToolKind::GROUPS[11], &[Move, Slice]);
    assert_eq!(
        ToolKind::GROUPS[12],
        &[Artboard],
        "Artboard has its own group"
    );
    // Cada herramienta está en un solo grupo.
    let all: Vec<ToolKind> = ToolKind::GROUPS
        .iter()
        .flat_map(|g| g.iter().copied())
        .collect();
    assert_eq!(all.len(), 22);
    for (i, kind) in all.iter().enumerate() {
        assert!(!all[..i].contains(kind), "{kind:?} twice");
    }
}

#[test]
fn new_shortcuts_and_every_shortcut_round_trips() {
    assert_eq!(ToolKind::from_key('H', false), Some(ToolKind::Hand));
    assert_eq!(ToolKind::from_key('D', false), Some(ToolKind::Contour));
    assert_eq!(ToolKind::from_key('R', false), Some(ToolKind::Blur));
    assert_eq!(ToolKind::from_key('R', true), Some(ToolKind::Jumble));
    for &kind in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
        let s = kind.shortcut();
        let (key, shift) = match s.strip_prefix("Shift+") {
            Some(k) => (k, true),
            None => (s, false),
        };
        let key = key.chars().next().unwrap();
        assert_eq!(ToolKind::from_key(key, shift), Some(kind), "{s}");
    }
    assert_eq!(ToolKind::Hand.label(), "Hand");
    assert_eq!(ToolKind::Contour.label(), "Contour");
    assert_eq!(ToolKind::Blur.label(), "Blur");
    assert!(
        !ToolKind::Hand.blocked_in_tiles(),
        "Hand works in Tiles mode"
    );
    // En el editor Hand no es una herramienta de sprite: un arrastre no pinta nada.
    let mut e = editor(ToolKind::Hand);
    stroke(&mut e, &[(1, 1), (3, 3)], false);
    assert_eq!(px(&e, 1, 1), e.sprite().color_mode.transparent());
    assert!(!e.history.can_undo(), "Hand never paints");
}

mod contour {
    use cs_core::{Mask, Symmetry, SymmetryMode};

    use super::*;

    #[test]
    fn fills_with_the_button_color_in_one_undo_step() {
        let mut e = editor(ToolKind::Contour);
        let t = e.sprite().color_mode.transparent();
        stroke(&mut e, &[(0, 0), (0, 4), (4, 4)], false);
        assert_eq!(px(&e, 0, 0), e.primary);
        assert_eq!(px(&e, 1, 3), e.primary, "inside the triangle");
        assert_eq!(px(&e, 3, 1), t, "outside the triangle");
        e.undo();
        assert!(e.sprite().cels.is_empty(), "the whole stroke was one step");
        assert!(!e.history.can_undo());
        stroke(&mut e, &[(0, 0), (0, 4), (4, 4)], true);
        assert_eq!(px(&e, 1, 3), e.secondary, "right button: secondary color");
    }

    #[test]
    fn is_clipped_to_the_selection() {
        let mut e = editor(ToolKind::Contour);
        let t = e.sprite().color_mode.transparent();
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 7))));
        stroke(&mut e, &[(0, 0), (0, 4), (4, 4)], false);
        assert_eq!(px(&e, 1, 3), e.primary);
        assert_eq!(px(&e, 2, 3), t, "inside the triangle but not selected");
    }

    #[test]
    fn a_click_paints_one_pixel_mirrored() {
        let mut e = editor(ToolKind::Contour);
        e.set_symmetry(Symmetry {
            mode: SymmetryMode::Horizontal,
            center2: None,
        });
        stroke(&mut e, &[(1, 1)], false);
        let painted: Vec<(i32, i32)> = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| px(&e, x, y) == e.primary)
            .collect();
        assert_eq!(painted, vec![(1, 1), (6, 1)]);
    }

    #[test]
    fn does_nothing_in_tiles_mode() {
        let mut e = crate::tile_ops::test_util::tile_editor();
        e.set_tool(ToolKind::Contour);
        let version = e.doc_version;
        stroke(&mut e, &[(1, 1), (1, 6), (6, 6)], false);
        assert_eq!(e.doc_version, version);
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Not available in Tiles mode")
        );
    }

    #[test]
    fn does_nothing_on_a_locked_layer() {
        let mut e = editor(ToolKind::Contour);
        e.toggle_locked(LayerId(1));
        let version = e.doc_version;
        let t = e.sprite().color_mode.transparent();
        stroke(&mut e, &[(0, 0), (0, 4), (4, 4)], false);
        assert_eq!(px(&e, 1, 3), t);
        assert_eq!(e.doc_version, version);
    }
}
