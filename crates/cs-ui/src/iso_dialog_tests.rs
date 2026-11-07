use cs_core::{ColorMode, LayerId, Mask, Palette, PixelValue, Sprite};
use cs_raster::IsoPart;

use super::*;
use crate::clipboard::Clip;
use crate::editor::{Editor, ToolKind};

const RED: Rgba = Rgba::rgb(200, 40, 40);

fn dialog(settings: IsoSettings) -> IsoDialog {
    IsoDialog::new(settings, RED, &[], false)
}

/// La caja de 2 × 2 × 2 con esquina de 2 px (6 × 6).
fn small_box() -> IsoSettings {
    IsoSettings {
        shape: IsoBox {
            left: 2,
            right: 2,
            height: 2,
            corner: IsoCorner::Two,
        },
        ..Default::default()
    }
}

/// Coloca lo del diálogo en `e` como lo hace la app.
fn place(e: &mut Editor, d: &IsoDialog) -> bool {
    let image = d.image();
    let origin = e.centered_origin((image.width, image.height));
    let Some(clip) = Clip::from_shape(origin, (image.width, image.height), &image.pixels) else {
        return false;
    };
    e.place_floating(&clip, d.label())
}

#[test]
fn shade_from_follows_the_primary_and_a_face_by_hand_turns_it_off() {
    let d = dialog(IsoSettings::default());
    assert_eq!(d.settings.base, RED);
    assert_eq!(d.settings.colors, IsoColors::from_base(RED));
    let mut s = d.settings;
    s.set_color(IsoPart::Top, Rgba::rgb(1, 2, 3));
    assert!(!s.shade);
    assert_eq!(s.colors.top, Rgba::rgb(1, 2, 3));
    // Otro base sin Shade from no toca los colores; prenderlo los vuelve a sacar del base.
    s.set_base(Rgba::rgb(0, 0, 200));
    assert_eq!(s.colors.top, Rgba::rgb(1, 2, 3));
    s.set_shade(true);
    assert_eq!(s.colors, IsoColors::from_base(Rgba::rgb(0, 0, 200)));
    // Sin Shade from, abrir de nuevo conserva los colores a mano.
    s.set_color(IsoPart::Right, Rgba::rgb(9, 9, 9));
    let again = dialog(s.clone());
    assert_eq!(again.settings.colors, s.colors);
}

#[test]
fn a_black_or_transparent_primary_keeps_the_last_base() {
    let last = IsoSettings::default();
    for primary in [Rgba::rgb(0, 0, 0), Rgba::rgb(20, 30, 10), Rgba::TRANSPARENT] {
        let d = IsoDialog::new(last.clone(), primary, &[], false);
        assert_eq!(d.settings.base, last.base, "{primary:?}");
    }
    let d = IsoDialog::new(last.clone(), Rgba::rgb(60, 20, 20), &[], false);
    assert_eq!(d.settings.base, Rgba::rgb(60, 20, 20));
}

#[test]
fn the_corner_is_shared_by_the_box_and_the_blocks() {
    let mut s = IsoSettings::default();
    s.set_corner(IsoCorner::Four);
    assert_eq!(
        (s.shape.corner, s.blocks.corner),
        (IsoCorner::Four, IsoCorner::Four)
    );
    assert_eq!(s.corner(), IsoCorner::Four);
}

#[test]
fn place_floats_the_box_centered_with_move_and_one_undo_step() {
    let mut e = Editor::with_sprite(Sprite::new(32, 32, ColorMode::Rgba));
    let d = dialog(small_box());
    assert!(place(&mut e, &d));
    assert!(e.is_floating());
    assert_eq!(e.tool, ToolKind::Move);
    // 6×6 centrado en 32×32: esquina en (13, 13). La selección es la forma de la caja.
    let sel = e.selection.as_ref().unwrap();
    assert_eq!(sel.bounds(), Some(((13, 13), (18, 18))));
    assert!(!sel.contains(13, 13), "the corner of the box is empty");
    assert!(e.commit_floating());
    let c = d.colors();
    let at = |e: &Editor, x, y| e.sprite().pixel(LayerId(1), 0, 13 + x, 13 + y);
    assert_eq!(at(&e, 2, 1), PixelValue::Rgba(c.top));
    assert_eq!(at(&e, 1, 3), PixelValue::Rgba(c.left));
    assert_eq!(at(&e, 2, 0), PixelValue::Rgba(c.outline));
    assert_eq!(at(&e, 0, 0), PixelValue::Rgba(Rgba::TRANSPARENT));
    assert_eq!(e.history.done().last(), Some("Iso Box"));
    assert_eq!(e.history.done_count(), 1);
    e.undo();
    assert_eq!(at(&e, 2, 1), PixelValue::Rgba(Rgba::TRANSPARENT));
}

#[test]
fn with_a_selection_it_is_centered_on_it_and_nothing_below_is_erased() {
    let mut e = Editor::with_sprite(Sprite::new(32, 32, ColorMode::Rgba));
    let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
    e.execute(cs_commands::Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(2, 2, blue)],
    });
    e.set_selection(Some(Mask::from_rect(32, 32, (0, 0), (10, 10))));
    let d = dialog(small_box());
    assert_eq!(e.centered_origin((6, 6)), (2, 2));
    assert!(place(&mut e, &d));
    assert!(e.commit_floating());
    // El rincón vacío de la caja (2, 2) no borra lo de debajo.
    assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), blue);
}

#[test]
fn indexed_sprites_get_palette_indices() {
    let mut sprite = Sprite::new(
        32,
        32,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    );
    let mut colors = vec![Rgba::TRANSPARENT];
    colors.extend_from_slice(Palette::db32().colors());
    sprite.palette = Palette::new(colors);
    let mut e = Editor::with_sprite(sprite);
    let palette: Vec<Rgba> = e.sprite().palette.colors().to_vec();
    let d = IsoDialog::new(IsoSettings::default(), RED, &palette, true);
    for c in [d.colors().top, d.colors().left, d.colors().right] {
        assert!(palette[1..].contains(&c), "{c:?}");
    }
    assert!(place(&mut e, &d));
    assert!(e.commit_floating());
    let sprite = e.sprite();
    let painted = (0..32)
        .flat_map(|y| (0..32).map(move |x| (x, y)))
        .map(|(x, y)| sprite.pixel(LayerId(1), 0, x, y))
        .filter(|v| *v != PixelValue::Index(0))
        .count();
    assert!(painted > 0);
    assert!((0..32).all(|y| {
        (0..32).all(|x| matches!(sprite.pixel(LayerId(1), 0, x, y), PixelValue::Index(_)))
    }));
}

#[test]
fn blocks_stack_with_clicks_and_an_empty_grid_places_nothing() {
    let mut s = IsoSettings {
        mode: IsoMode::Blocks,
        ..Default::default()
    };
    s.blocks.resize(2, 1);
    s.blocks.heights = vec![0, 0];
    s.bump(0, 0, 1);
    s.bump(0, 0, 1);
    s.bump(1, 0, -1);
    assert_eq!(s.blocks.heights, vec![2, 0]);
    for _ in 0..40 {
        s.bump(1, 0, 1);
    }
    assert_eq!(s.blocks.height_at(1, 0), MAX_STACK);
    let d = dialog(s.clone());
    assert_eq!(d.label(), "Iso Blocks");
    assert!(d.image().width > 0);
    let mut empty = s;
    empty.blocks.heights = vec![0, 0];
    let d = dialog(empty);
    assert_eq!(d.image().width, 0);
    let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
    assert!(!place(&mut e, &d));
    assert!(!e.is_floating());
}
