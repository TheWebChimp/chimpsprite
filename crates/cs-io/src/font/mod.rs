//! Las fuentes pixel hacia y desde otros formatos: TTF (con COLR para las de color), BMFont y el
//! JSON de BitFontMaker2.

pub mod bmfont;
pub mod outline;
pub mod ttf;
pub mod ttf_import;

/// Una fuente de prueba de celda 8×10 (baseline 8): una `A` con hueco (en color, de dos
/// colores), una `g` con descendente, una `i` de ancho manual, un `€` y un carácter fuera del BMP.
#[cfg(test)]
pub(crate) fn sample_font(mode: cs_core::FontMode) -> cs_core::Artboard {
    use cs_core::{FontBuild, FontMetrics, GlyphBuild, Rgba, build_font};
    let red = Some(Rgba::new(220, 30, 40, 255));
    let blue = Some(Rgba::new(20, 40, 200, 255));
    let art = |rows: &[&str], top: u32| -> Vec<(u32, u32, Option<Rgba>)> {
        let mut out = Vec::new();
        for (dy, row) in rows.iter().enumerate() {
            for (x, c) in row.chars().enumerate() {
                let color = match c {
                    'X' => red,
                    'O' => blue,
                    _ => continue,
                };
                out.push((x as u32, top + dy as u32, color));
            }
        }
        out
    };
    let glyph = |ch, advance, pixels| GlyphBuild {
        ch,
        advance,
        pixels,
    };
    let a = art(
        &[
            ".XXX.", "X...X", "X...X", "OOOOO", "X...X", "X...X", "X...X",
        ],
        1,
    );
    let g = art(&[".XXX", "X..X", "X..X", ".XXX", "...X", "XXX."], 3);
    let i = art(&["X", ".", "X", "X", "X", "X"], 2);
    let euro = art(&["..XX", ".X..", "XXX.", ".X..", "XXX.", ".X..", "..XX"], 1);
    let smile = art(&["X.X", "...", "XXX"], 4);
    let (font, sprite) = build_font(FontBuild {
        name: "Sample".into(),
        mode,
        cell: (8, 10),
        metrics: FontMetrics {
            baseline: 8,
            cap_height: 7,
            x_height: 5,
            descent: 2,
        },
        letter_spacing: 1,
        space_width: 3,
        monospace: false,
        glyphs: vec![
            glyph('A', None, a),
            glyph('g', None, g),
            glyph('i', Some(3), i),
            glyph('€', None, euro),
            glyph('\u{1F600}', None, smile),
            glyph('z', None, Vec::new()),
        ],
    });
    cs_core::Artboard {
        model: None,
        id: cs_core::ArtboardId(7),
        name: "Sample".into(),
        position: (0, 0),
        sprite,
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: true,
        font: Some(font),
    }
}
