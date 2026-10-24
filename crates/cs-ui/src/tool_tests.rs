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
    // Cada grupo por su contenido, no por su posición.
    let group_of = |kind: ToolKind| {
        *ToolKind::GROUPS
            .iter()
            .find(|g| g.contains(&kind))
            .unwrap_or_else(|| panic!("{kind:?} in no group"))
    };
    assert_eq!(group_of(Jumble), &[Bucket, Gradient, Jumble]);
    assert_eq!(group_of(Slice), &[Move, Slice]);
    assert_eq!(
        group_of(Artboard),
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

#[test]
fn tool_options_saved_without_the_brush_shape_read_as_round() {
    use crate::editor::ToolOptions;
    use cs_tools::BrushShape;
    let old: ToolOptions =
        serde_json::from_str(r#"{"brush_size":4,"pixel_perfect":false}"#).unwrap();
    assert_eq!(old.brush_shape, BrushShape::Round);
    assert_eq!(old.brush_size, 4);
    assert!(!old.pixel_perfect);
    let square = ToolOptions {
        brush_shape: BrushShape::Square,
        constrain: true,
        ..Default::default()
    };
    let back: ToolOptions = serde_json::from_str(&serde_json::to_string(&square).unwrap()).unwrap();
    assert_eq!(back.brush_shape, BrushShape::Square);
    assert!(!back.constrain, "Shift is not saved");
}

#[test]
fn saved_brush_size_is_clamped_and_the_selection_mode_is_not_saved() {
    use crate::editor::{MAX_BRUSH_SIZE, SelectionMode, ToolOptions};
    let read = |json: &str| serde_json::from_str::<ToolOptions>(json).unwrap();
    assert_eq!(read(r#"{"brush_size":0}"#).brush_size, 1);
    assert_eq!(read(r#"{"brush_size":4000}"#).brush_size, MAX_BRUSH_SIZE);
    assert_eq!(read(r#"{"brush_size":7}"#).brush_size, 7);
    assert_eq!(
        read(r#"{"selection_mode":"Subtract"}"#).selection_mode,
        SelectionMode::Replace,
        "an old saved mode is ignored"
    );
    let add = ToolOptions {
        selection_mode: SelectionMode::Add,
        ..Default::default()
    };
    let json = serde_json::to_string(&add).unwrap();
    assert!(!json.contains("selection_mode"), "{json}");
    assert_eq!(read(&json).selection_mode, SelectionMode::Replace);
}

#[test]
fn a_group_remembers_the_last_tool_chosen_in_it() {
    let marquee = ToolKind::RectSelect.group();
    let mut e = editor(ToolKind::Pencil);
    assert_eq!(e.group_tool(marquee), ToolKind::RectSelect, "the first one");
    e.set_tool(ToolKind::Lasso);
    e.set_tool(ToolKind::Pencil);
    assert_eq!(e.group_tool(marquee), ToolKind::Lasso);
    // La activa manda en su grupo aunque se haya recordado otra.
    let pencil = ToolKind::Pencil.group();
    e.options.group_choice.remember(ToolKind::Spray);
    assert_eq!(e.group_tool(pencil), ToolKind::Pencil);
}

#[test]
fn the_group_choice_is_saved_with_the_tool_options() {
    use crate::editor::{GroupChoice, ToolOptions};
    let mut options = ToolOptions::default();
    options.group_choice.remember(ToolKind::Lasso);
    options.group_choice.remember(ToolKind::Slice);
    let json = serde_json::to_string(&options).unwrap();
    assert!(json.contains(r#""Lasso""#), "{json}");
    let back: ToolOptions = serde_json::from_str(&json).unwrap();
    assert_eq!(back.group_choice, options.group_choice);

    // Un ajuste de antes no la trae: cada grupo con su primera herramienta.
    let old: ToolOptions = serde_json::from_str(r#"{"brush_size":4}"#).unwrap();
    assert_eq!(old.group_choice, GroupChoice::default());
    assert_eq!(old.group_choice.get(0), ToolKind::RectSelect);

    // Nombres desconocidos, de otro grupo o que faltan: la primera del grupo.
    let odd: ToolOptions =
        serde_json::from_str(r#"{"group_choice":["Teleport","Pencil","Spray"]}"#).unwrap();
    assert_eq!(odd.group_choice.get(0), ToolKind::RectSelect, "unknown");
    assert_eq!(
        odd.group_choice.get(1),
        ToolKind::Hand,
        "Pencil is not a Hand tool"
    );
    assert_eq!(odd.group_choice.get(2), ToolKind::Spray);
    assert_eq!(odd.group_choice.get(5), ToolKind::Bucket, "missing");
}

#[test]
fn the_editor_paints_with_the_chosen_brush_shape() {
    use cs_tools::BrushShape;
    let mut e = editor(ToolKind::Pencil);
    e.options.brush_size = 3;
    assert_eq!(e.options.brush_shape, BrushShape::Round, "default");
    stroke(&mut e, &[(3, 3)], false);
    let t = e.sprite().color_mode.transparent();
    assert_ne!(px(&e, 3, 2), t, "top of the cross");
    assert_eq!(px(&e, 2, 2), t, "no corner");
    e.options.brush_shape = BrushShape::Square;
    stroke(&mut e, &[(6, 6)], false);
    assert_ne!(px(&e, 5, 5), t, "the square has corners");
}

mod contour {
    use cs_core::{Mask, Symmetry, SymmetryAxes};

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
            axes: SymmetryAxes {
                vertical: true,
                ..SymmetryAxes::NONE
            },
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

mod blur {
    use cs_commands::{Command, LayerProps};
    use cs_core::{Mask, Rgba, Symmetry, SymmetryAxes};

    use super::*;

    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    /// Rojo con un noveno del alfa.
    fn soft() -> PixelValue {
        PixelValue::Rgba(Rgba::new(255, 0, 0, 28))
    }

    fn dot(e: &mut Editor, p: (i32, i32)) {
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(p.0, p.1, red())],
        });
    }

    fn all(e: &Editor) -> Vec<PixelValue> {
        (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .map(|(x, y)| px(e, x, y))
            .collect()
    }

    #[test]
    fn only_changes_selected_pixels() {
        let mut e = editor(ToolKind::Blur);
        e.options.brush_size = 3;
        dot(&mut e, (4, 4));
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (3, 7))));
        stroke(&mut e, &[(4, 4)], false);
        assert_eq!(px(&e, 3, 4), soft());
        assert_eq!(px(&e, 4, 4), red(), "not selected");
        assert_eq!(px(&e, 5, 4), e.sprite().color_mode.transparent());
    }

    #[test]
    fn a_stroke_is_one_undo_step() {
        let mut e = editor(ToolKind::Blur);
        e.options.brush_size = 3;
        dot(&mut e, (4, 4));
        let before = all(&e);
        stroke(&mut e, &[(4, 4), (4, 5), (4, 6)], false);
        assert_ne!(all(&e), before);
        e.undo();
        assert_eq!(all(&e), before, "one undo brings back the dot");
    }

    #[test]
    fn does_nothing_on_a_locked_layer() {
        let mut e = editor(ToolKind::Blur);
        dot(&mut e, (4, 4));
        e.execute(Command::SetLayerProps {
            id: LayerId(1),
            props: LayerProps {
                locked: Some(true),
                ..Default::default()
            },
        });
        let version = e.doc_version;
        stroke(&mut e, &[(4, 4), (5, 4)], false);
        assert_eq!(e.doc_version, version);
        assert_eq!(px(&e, 4, 4), red());
    }

    #[test]
    fn ignores_symmetry() {
        let mut e = editor(ToolKind::Blur);
        dot(&mut e, (1, 4));
        e.set_symmetry(Symmetry {
            axes: SymmetryAxes {
                vertical: true,
                ..SymmetryAxes::NONE
            },
            center2: None,
        });
        stroke(&mut e, &[(1, 4)], false);
        assert_eq!(px(&e, 1, 4), soft());
        assert_eq!(
            px(&e, 6, 4),
            e.sprite().color_mode.transparent(),
            "no mirrored copy"
        );
    }

    #[test]
    fn uses_the_brush_and_is_blocked_in_tiles_mode() {
        assert!(ToolKind::Blur.uses_brush());
        let mut e = crate::tile_ops::test_util::tile_editor();
        e.set_tool(ToolKind::Blur);
        let version = e.doc_version;
        stroke(&mut e, &[(1, 1), (6, 6)], false);
        assert_eq!(e.doc_version, version);
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Not available in Tiles mode")
        );
    }
}

/// Blur y Jumble promedian o mezclan colores de la capa: editando una máscara no actúan.
mod on_a_mask {
    use cs_core::{Rgba, Sprite};

    use super::*;
    use crate::mask_ops::NewMask;

    /// Editor con un punto rojo y una máscara a medias (izquierda visible, derecha oculta), en
    /// edición.
    fn masked(tool: ToolKind) -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.primary = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        stroke(&mut e, &[(4, 4)], false);
        e.set_selection(Some(cs_core::Mask::from_rect(8, 8, (0, 0), (3, 7))));
        e.add_mask(LayerId(1), NewMask::FromSelection);
        assert!(e.editing_mask());
        e.set_tool(tool);
        e
    }

    fn mask(e: &Editor) -> cs_core::LayerMask {
        e.sprite().layers[0].mask.clone().unwrap()
    }

    #[test]
    fn blur_and_jumble_do_nothing_and_say_so() {
        for tool in [ToolKind::Blur, ToolKind::Jumble] {
            let mut e = masked(tool);
            e.options.brush_size = 3;
            let before = mask(&e);
            let version = e.doc_version;
            stroke(&mut e, &[(3, 4), (4, 4), (5, 4)], false);
            assert_eq!(mask(&e), before, "{tool:?} left the mask alone");
            assert_eq!(e.doc_version, version, "{tool:?}");
            assert!(e.tool_blocked(), "{tool:?}: no brush outline");
            assert_eq!(
                e.mask_status(),
                Some("Not available while editing a mask"),
                "{tool:?}"
            );
        }
    }

    #[test]
    fn the_pencil_still_paints_the_mask() {
        let mut e = masked(ToolKind::Pencil);
        assert!(!e.tool_blocked());
        assert_eq!(e.mask_status(), None);
        e.primary = PixelValue::Rgba(Rgba::rgb(0, 0, 0));
        stroke(&mut e, &[(1, 1)], false);
        assert_eq!(mask(&e).get(1, 1), 0, "black hides");
        // Fuera de la edición de máscara Blur vuelve a actuar.
        e.set_tool(ToolKind::Blur);
        e.select_layer(LayerId(1));
        assert!(!e.tool_blocked());
        assert_eq!(e.mask_status(), None);
    }

    #[test]
    fn spray_blur_and_jumble_are_blocked_in_tiles_mode_and_the_pencil_is_not() {
        let mut e = crate::tile_ops::test_util::tile_editor();
        for tool in [ToolKind::Spray, ToolKind::Blur, ToolKind::Jumble] {
            e.set_tool(tool);
            assert!(e.tool_blocked(), "{tool:?}");
        }
        e.set_tool(ToolKind::Pencil);
        assert!(!e.tool_blocked());
    }

    /// Editor con un punto rojo, Blur de 3 px y una máscara que muestra todo, sin editarla.
    fn blur_beside_a_mask() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.primary = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        stroke(&mut e, &[(4, 4)], false);
        e.add_mask(LayerId(1), NewMask::RevealAll);
        e.set_edit_mask(false);
        assert!(!e.editing_mask());
        e.set_tool(ToolKind::Blur);
        e.options.brush_size = 3;
        e
    }

    #[test]
    fn entering_the_mask_mid_stroke_cancels_the_blur_stroke() {
        let mut e = blur_beside_a_mask();
        let version = e.doc_version;
        e.pointer_press(at((3, 4)), false, false, 0.1);
        assert!(e.is_painting());
        e.set_edit_mask(true);
        assert!(!e.is_painting(), "the stroke is cancelled");
        let before = mask(&e);
        e.pointer_drag(at((5, 4)));
        e.pointer_release();
        assert_eq!(mask(&e), before, "nothing reached the mask");
        assert_eq!(e.doc_version, version, "nothing was applied");
    }

    #[test]
    fn a_stroke_left_blocked_by_the_mask_is_cancelled_on_the_next_drag() {
        let mut e = blur_beside_a_mask();
        e.pointer_press(at((3, 4)), false, false, 0.1);
        // Sin pasar por `set_edit_mask`.
        e.edit_mask = true;
        e.pointer_drag(at((5, 4)));
        assert!(!e.is_painting());
    }

    #[test]
    fn switching_to_a_tilemap_layer_mid_stroke_cancels_the_stroke() {
        let mut e = crate::tile_ops::test_util::tile_editor();
        e.select_layer(LayerId(1));
        assert!(!e.tiles_mode());
        e.set_tool(ToolKind::Blur);
        e.pointer_press(at((3, 4)), false, false, 0.1);
        assert!(e.is_painting());
        e.select_layer(LayerId(2));
        assert!(e.tiles_mode());
        assert!(!e.is_painting(), "the stroke is cancelled");
    }
}
