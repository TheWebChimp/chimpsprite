//! Importar el JSON de BitFontMaker2 (pentacom.jp). Su formato, leído del código de la página
//! (`savetofile` y `loadData`):
//!
//! ```json
//! {"65":[0,0,…16 enteros…], "name":"MyFont", "copy":"", "letterspace":"64",
//!  "basefont_size":"512", "basefont_left":"62", "basefont_top":"0", "basefont":"Arial",
//!  "monospace":true, "monospacewidth":"8", "ascender":"12", "descender":"4",
//!  "linegap":"1", "wordspacing":"5"}
//! ```
//!
//! Las claves numéricas (> 30) son el código Unicode en decimal y su valor, las 16 filas de
//! arriba a abajo: el bit `1 << x` es la columna `x` desde la izquierda. La cuadrícula es de
//! 16×16, con la baseline en la fila 12 y el borde izquierdo de las letras en la columna 2. El
//! espaciado va en sus unidades (64 por pixel); lo demás, en pixeles. Los valores pueden venir
//! como texto o como número.

use cs_core::{FontBuild, FontMetrics, FontMode, GlyphBuild};
use serde_json::Value;

use crate::{IoError, Result};

/// El lado de la cuadrícula de BitFontMaker2.
const GRID: u32 = 16;
/// Su baseline y su margen izquierdo.
const BASELINE: u32 = 12;
const LEFT_MARGIN: u32 = 2;
/// Sus unidades por pixel (las del espaciado).
const UNITS: f64 = 64.0;

/// Un número que puede venir como número o como texto.
fn number(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(*b as u8 as f64),
        _ => None,
    }
}

/// La fuente del JSON `json`, lista para [`cs_core::build_font`].
pub fn import_bitfontmaker(json: &str) -> Result<FontBuild> {
    let value: Value = serde_json::from_str(json.trim().trim_start_matches('\u{feff}'))?;
    let Value::Object(map) = value else {
        return Err(IoError::Unsupported("not BitFontMaker2 data".into()));
    };
    let mut glyphs: Vec<GlyphBuild> = Vec::new();
    for (key, rows) in &map {
        let Some(ch) = key
            .parse::<u32>()
            .ok()
            .filter(|cp| *cp > 30)
            .and_then(char::from_u32)
        else {
            continue;
        };
        let Some(rows) = rows.as_array().filter(|r| r.len() == GRID as usize) else {
            continue;
        };
        let mut pixels = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            let bits = number(Some(row)).unwrap_or(0.0) as u64;
            for x in 0..GRID {
                if bits & (1 << x) != 0 {
                    pixels.push((x, y as u32, None));
                }
            }
        }
        glyphs.push(GlyphBuild {
            ch,
            advance: None,
            pixels,
        });
    }
    if glyphs.is_empty() {
        return Err(IoError::Unsupported(
            "no BitFontMaker2 glyphs in the data".into(),
        ));
    }
    // El margen izquierdo de BitFontMaker2 no es parte de la letra: se quita lo que se pueda sin
    // perder pixeles.
    let left = glyphs
        .iter()
        .flat_map(|g| g.pixels.iter().map(|p| p.0))
        .min()
        .unwrap_or(0);
    let shift = left.min(LEFT_MARGIN);
    for g in &mut glyphs {
        for p in &mut g.pixels {
            p.0 -= shift;
        }
    }
    let monospace = number(map.get("monospace")).is_some_and(|v| v != 0.0)
        || map.get("monospace").and_then(Value::as_str) == Some("true");
    if monospace {
        let width = number(map.get("monospacewidth")).unwrap_or(8.0).max(1.0) as u32;
        let advance = width + (LEFT_MARGIN - shift);
        for g in &mut glyphs {
            g.advance = Some(advance);
        }
    }
    let baseline = number(map.get("ascender"))
        .map(|a| a.round() as u32)
        .filter(|a| (1..=GRID).contains(a))
        .unwrap_or(BASELINE);
    let top_of = |ch: char| {
        glyphs
            .iter()
            .find(|g| g.ch == ch)
            .and_then(|g| g.pixels.iter().map(|p| p.1).min())
            .map(|top| baseline.saturating_sub(top))
    };
    let initial = FontMetrics::for_cell(GRID);
    let metrics = FontMetrics {
        baseline,
        cap_height: top_of('H').unwrap_or(initial.cap_height),
        x_height: top_of('x').unwrap_or(initial.x_height),
        descent: GRID - baseline,
    };
    let letter_spacing =
        number(map.get("letterspace")).map_or(1, |units| (units / UNITS).round().max(0.0) as u32);
    let space_width = number(map.get("wordspacing"))
        .map_or(5, |w| w.round().max(0.0) as u32)
        .min(GRID);
    let name = map
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or("BitFontMaker2")
        .to_owned();
    Ok(FontBuild {
        name,
        mode: FontMode::OneBit,
        cell: (GRID, GRID),
        metrics,
        letter_spacing,
        space_width,
        monospace: false,
        kerning: Vec::new(),
        style: Default::default(),
        glyphs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Las filas de BitFontMaker2 de un dibujo (`X` prendido) desde la fila `top`, a partir de
    /// la columna `left`.
    fn rows(art: &[&str], top: usize, left: u32) -> Vec<u64> {
        let mut out = vec![0u64; 16];
        for (dy, line) in art.iter().enumerate() {
            for (x, c) in line.chars().enumerate() {
                if c == 'X' {
                    out[top + dy] |= 1 << (x as u32 + left);
                }
            }
        }
        out
    }

    #[test]
    fn glyph_bits_go_left_to_right_and_lose_the_left_margin() {
        let h = rows(&["X..X", "X..X", "XXXX", "X..X", "X..X"], 7, 2);
        let x = rows(&["X.X", ".X.", "X.X"], 9, 2);
        let json = serde_json::json!({
            "72": h, "120": x, "name": "Tiny", "copy": "me", "letterspace": "128",
            "basefont_size": "512", "basefont": "Arial", "wordspacing": 4,
        })
        .to_string();
        let build = import_bitfontmaker(&json).unwrap();
        assert_eq!(build.name, "Tiny");
        assert_eq!(build.cell, (16, 16));
        assert_eq!((build.letter_spacing, build.space_width), (2, 4));
        assert_eq!(build.metrics.baseline, 12);
        assert_eq!((build.metrics.cap_height, build.metrics.x_height), (5, 3));
        assert_eq!(build.metrics.descent, 4);
        let h = &build.glyphs.iter().find(|g| g.ch == 'H').unwrap().pixels;
        assert!(h.contains(&(0, 7, None)) && h.contains(&(3, 7, None)));
        assert!(h.contains(&(1, 9, None)));
        assert_eq!(h.len(), 12);
        assert!(build.glyphs.iter().all(|g| g.advance.is_none()));
    }

    #[test]
    fn pixels_in_the_margin_are_kept() {
        let json = serde_json::json!({
            "65": rows(&["XX"], 3, 1),
            "66": rows(&["X"], 3, 4),
        })
        .to_string();
        let build = import_bitfontmaker(&json).unwrap();
        let a = &build.glyphs.iter().find(|g| g.ch == 'A').unwrap().pixels;
        assert_eq!(a, &vec![(0, 3, None), (1, 3, None)], "shifted only by 1");
        assert_eq!(build.name, "BitFontMaker2");
        assert_eq!((build.letter_spacing, build.space_width), (1, 5));
    }

    #[test]
    fn monospace_strings_and_the_ascender() {
        let rows: Vec<Value> = rows(&["X"], 2, 2)
            .into_iter()
            .map(|r| Value::String(r.to_string()))
            .collect();
        let json = serde_json::json!({
            "97": rows, "monospace": true, "monospacewidth": "6", "ascender": "10",
            "descender": "6", "letterspace": 0,
        })
        .to_string();
        let build = import_bitfontmaker(&json).unwrap();
        assert_eq!(build.glyphs[0].advance, Some(6));
        assert_eq!(build.glyphs[0].pixels, vec![(0, 2, None)]);
        assert_eq!(build.metrics.baseline, 10);
        assert_eq!(build.metrics.descent, 6);
        assert_eq!(build.letter_spacing, 0);
    }

    #[test]
    fn what_is_not_bitfontmaker_data_is_refused() {
        assert!(import_bitfontmaker("[1, 2]").is_err());
        assert!(import_bitfontmaker("{\"name\": \"x\"}").is_err());
        assert!(
            import_bitfontmaker("{\"65\": [1, 2]}").is_err(),
            "not 16 rows"
        );
        assert!(import_bitfontmaker("nope").is_err());
    }
}
