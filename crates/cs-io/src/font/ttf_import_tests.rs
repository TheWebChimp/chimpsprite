use cs_core::{Artboard, FontMode, build_font};

use super::*;
use crate::font::sample_font;
use crate::font::ttf::{TtfOptions, export_ttf};

fn exported(mode: FontMode) -> Vec<u8> {
    let options = TtfOptions {
        family: String::new(),
        color: mode == FontMode::Color,
    };
    export_ttf(&sample_font(mode), &options).unwrap()
}

fn options(px: u32, color: bool) -> TtfImport {
    TtfImport {
        name: String::new(),
        px,
        threshold: 50,
        chars: CharRange::All,
        color,
    }
}

/// El artboard de lo importado.
fn imported(bytes: &[u8], options: &TtfImport) -> Artboard {
    let (font, sprite) = build_font(import_ttf(bytes, options).unwrap());
    Artboard {
        font: Some(font),
        sprite,
        ..sample_font(FontMode::OneBit)
    }
}

#[test]
fn the_info_of_our_ttf_suggests_its_cell_height() {
    let info = ttf_info(&exported(FontMode::OneBit)).unwrap();
    assert_eq!(info.family, "Sample");
    assert_eq!(info.units_per_em, 640);
    assert!(info.pixel_grid);
    assert_eq!(info.suggested_px, 10);
    assert!(!info.color);
    assert_eq!(info.chars, 6);
    assert_eq!(info.sample, "Agiz€\u{1F600}");
    assert!(ttf_info(&exported(FontMode::Color)).unwrap().color);
    assert!(ttf_info(b"not a font").is_err());
}

#[test]
fn export_then_import_gives_the_same_pixels_and_advances() {
    for mode in [FontMode::OneBit, FontMode::Color] {
        let original = sample_font(mode);
        let bytes = exported(mode);
        let back = imported(&bytes, &options(10, mode == FontMode::Color));
        let (a, b) = (original.font_view().unwrap(), back.font_view().unwrap());
        assert_eq!(b.cell, a.cell, "{mode:?}");
        assert_eq!(b.metrics, a.metrics, "{mode:?}");
        assert_eq!(b.space_width, a.space_width);
        assert_eq!(b.glyphs, a.glyphs, "{mode:?}");
        let font = back.font.as_ref().unwrap();
        assert_eq!(font.name, "Sample");
        assert_eq!(font.mode, mode);
        assert!(!font.monospace);
        // La i sigue manual; la A, automática.
        let adv = |c| font.glyphs[font.index_of(c).unwrap()].advance;
        assert_eq!((adv('A'), adv('i')), (None, Some(3)));
    }
}

#[test]
fn a_monospace_font_comes_back_monospace() {
    let mut artboard = sample_font(FontMode::OneBit);
    artboard.font.as_mut().unwrap().monospace = true;
    let options_out = TtfOptions {
        family: String::new(),
        color: false,
    };
    let bytes = export_ttf(&artboard, &options_out).unwrap();
    let back = imported(&bytes, &options(10, false));
    let font = back.font.as_ref().unwrap();
    assert!(font.monospace);
    assert!(font.glyphs.iter().all(|g| g.advance.is_none()));
}

#[test]
fn twice_the_size_doubles_every_pixel() {
    let bytes = exported(FontMode::OneBit);
    let back = imported(&bytes, &options(20, false));
    let view = back.font_view().unwrap();
    let small = sample_font(FontMode::OneBit).font_view().unwrap();
    assert_eq!(view.cell, (16, 20));
    assert_eq!(view.metrics.baseline, 16);
    for (ch, g) in &small.glyphs {
        let big = &view.glyphs[ch];
        assert_eq!(big.pixels.len(), g.pixels.len() * 4, "{ch}");
        assert_eq!(big.advance, g.advance * 2, "{ch}");
    }
}

#[test]
fn a_higher_threshold_lights_fewer_pixels() {
    let bytes = exported(FontMode::OneBit);
    let count = |threshold| {
        let o = TtfImport {
            threshold,
            ..options(15, false)
        };
        let view = imported(&bytes, &o).font_view().unwrap();
        view.glyphs.values().map(|g| g.pixels.len()).sum::<usize>()
    };
    let (low, mid, high) = (count(1), count(50), count(100));
    assert!(low > mid && mid > high, "{low} {mid} {high}");
}

#[test]
fn ranges_limit_the_chars_and_the_color_option_needs_colr() {
    let bytes = exported(FontMode::Color);
    let ascii = TtfImport {
        chars: CharRange::Ascii,
        ..options(10, true)
    };
    let build = import_ttf(&bytes, &ascii).unwrap();
    let chars: Vec<char> = build.glyphs.iter().map(|g| g.ch).collect();
    assert_eq!(chars, vec!['A', 'g', 'i', 'z']);
    let latin = TtfImport {
        chars: CharRange::Latin,
        ..options(10, true)
    };
    assert_eq!(import_ttf(&bytes, &latin).unwrap().glyphs.len(), 4);
    // Sin COLR, aunque se pida color, es de 1 bit.
    let plain = import_ttf(&exported(FontMode::OneBit), &options(10, true)).unwrap();
    assert_eq!(plain.mode, FontMode::OneBit);
    // Con color, pero sin pedirlo, también.
    let gray = import_ttf(&bytes, &options(10, false)).unwrap();
    assert_eq!(gray.mode, FontMode::OneBit);
    assert!(
        gray.glyphs
            .iter()
            .flat_map(|g| &g.pixels)
            .all(|p| p.2.is_none())
    );
}

#[test]
fn some_chars_can_be_rasterized_for_a_preview() {
    let bytes = exported(FontMode::OneBit);
    let build = import_chars(&bytes, &['A', ' ', 'Q', 'i'], &options(10, false)).unwrap();
    let chars: Vec<char> = build.glyphs.iter().map(|g| g.ch).collect();
    assert_eq!(chars, vec!['A', 'i']);
}

#[test]
fn the_common_grid_of_the_coordinates() {
    assert_eq!(grid_of(&[64.0, 128.0, -192.0, 0.0]), Some(64));
    assert_eq!(grid_of(&[64.0, 96.0]), Some(32));
    assert_eq!(grid_of(&[64.5]), None);
    assert_eq!(grid_of(&[0.0]), None);
}

#[test]
fn a_real_outline_font_is_rasterized_into_its_cell() {
    for bytes in [
        epaint_default_fonts::HACK_REGULAR,
        epaint_default_fonts::UBUNTU_LIGHT,
    ] {
        let info = ttf_info(bytes).unwrap();
        assert!(!info.pixel_grid, "{}", info.family);
        assert_eq!(info.suggested_px, 16);
        let o = TtfImport {
            chars: CharRange::Ascii,
            ..options(16, false)
        };
        let build = import_ttf(bytes, &o).unwrap();
        assert_eq!(build.glyphs.len(), 94, "{}", info.family);
        let (w, h) = build.cell;
        assert!((16..=24).contains(&h), "{} {h}", info.family);
        let pixels = |c| {
            build
                .glyphs
                .iter()
                .find(|g| g.ch == c)
                .unwrap()
                .pixels
                .len()
        };
        assert!(pixels('H') > 10 && pixels('W') > pixels('.'));
        assert!(
            build
                .glyphs
                .iter()
                .flat_map(|g| &g.pixels)
                .all(|p| p.0 < w && p.1 < h)
        );
        // La H se apoya en la baseline.
        let h_glyph = build.glyphs.iter().find(|g| g.ch == 'H').unwrap();
        let bottom = h_glyph.pixels.iter().map(|p| p.1).max().unwrap();
        assert_eq!(bottom + 1, build.metrics.baseline, "{}", info.family);
    }
}

#[test]
fn rebasing_moves_the_glyphs_onto_another_baseline() {
    // La muestra tiene la baseline en 8: a una de 10 baja 2, a una de 3 sube 5 (y se pierde lo de
    // arriba).
    let build = import_ttf(&exported(FontMode::OneBit), &options(10, false)).unwrap();
    assert_eq!(build.metrics.baseline, 8);
    let a = |glyphs: &[GlyphBuild]| glyphs.iter().find(|g| g.ch == 'A').unwrap().pixels.clone();
    let original = a(&build.glyphs);
    let down = a(&rebase(&build, 10));
    assert_eq!(down.len(), original.len());
    assert!(
        original
            .iter()
            .zip(&down)
            .all(|(o, d)| d.1 == o.1 + 2 && d.0 == o.0)
    );
    let up = a(&rebase(&build, 3));
    assert!(up.len() < original.len() && up.iter().all(|p| p.1 <= 2));
}
