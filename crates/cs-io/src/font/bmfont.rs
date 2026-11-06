//! Exportar una fuente pixel a BMFont (el `.fnt` de texto de AngelCode y su atlas `.png`), lo
//! que leen los motores de juegos. Cada glifo va recortado a sus pixeles, en estantes con 1 pixel
//! de aire; la tinta de 1 bit sale blanca, para teñirla en el motor.

use std::fmt::Write as _;

use cs_core::{Artboard, Rgba};

use crate::export::Bitmap;
use crate::{IoError, Result};

/// El aire entre glifos del atlas.
const SPACING: u32 = 1;
/// La tinta de 1 bit en el atlas.
const WHITE: Rgba = Rgba::new(255, 255, 255, 255);

/// Lo que se escribe: el `.fnt` y el `.png` al que apunta.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BmFont {
    pub fnt: String,
    pub png: Vec<u8>,
}

/// Un glifo en el atlas.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Placed {
    ch: char,
    /// Su lugar en el atlas y su tamaño (0×0 si está vacío).
    at: (u32, u32),
    size: (u32, u32),
    /// Su esquina en la celda.
    offset: (u32, u32),
    advance: u32,
    pixels: Vec<(u32, u32, Rgba)>,
}

/// El BMFont de la fuente del artboard `artboard`; el `.fnt` apunta a `png_file`.
pub fn export_bmfont(artboard: &Artboard, png_file: &str) -> Result<BmFont> {
    let font = artboard
        .font
        .as_ref()
        .ok_or_else(|| IoError::Unsupported("the artboard is not a font".into()))?;
    let view = artboard
        .font_view()
        .ok_or_else(|| IoError::Unsupported("the artboard is not a font".into()))?;
    let mut glyphs: Vec<Placed> = view
        .glyphs
        .iter()
        .filter(|(ch, _)| **ch != ' ')
        .map(|(&ch, g)| {
            let x0 = g.pixels.iter().map(|p| p.0).min().unwrap_or(0);
            let y0 = g.pixels.iter().map(|p| p.1).min().unwrap_or(0);
            let x1 = g.pixels.iter().map(|p| p.0 + 1).max().unwrap_or(0);
            let y1 = g.pixels.iter().map(|p| p.1 + 1).max().unwrap_or(0);
            Placed {
                ch,
                at: (0, 0),
                size: (x1.saturating_sub(x0), y1.saturating_sub(y0)),
                offset: if g.pixels.is_empty() {
                    (0, 0)
                } else {
                    (x0, y0)
                },
                advance: g.advance,
                pixels: g
                    .pixels
                    .iter()
                    .map(|&(x, y, c)| (x - x0, y - y0, c.unwrap_or(WHITE)))
                    .collect(),
            }
        })
        .collect();
    let (width, height) = pack(&mut glyphs);
    let mut atlas = Bitmap::new(width, height);
    for g in &glyphs {
        for &(x, y, c) in &g.pixels {
            atlas.pixels[((g.at.1 + y) * width + g.at.0 + x) as usize] = c;
        }
    }
    let (_, cell_h) = view.cell;
    let face = font.name.replace('"', "'");
    let file = png_file.replace('"', "'");
    let mut fnt = String::new();
    // `write!` a un `String` no falla.
    let _ = writeln!(
        fnt,
        "info face=\"{face}\" size={cell_h} bold=0 italic=0 charset=\"\" unicode=1 \
         stretchH=100 smooth=0 aa=1 padding=0,0,0,0 spacing={SPACING},{SPACING} outline=0"
    );
    let _ = writeln!(
        fnt,
        "common lineHeight={cell_h} base={} scaleW={width} scaleH={height} pages=1 packed=0 \
         alphaChnl=0 redChnl=0 greenChnl=0 blueChnl=0",
        view.metrics.baseline.min(cell_h)
    );
    let _ = writeln!(fnt, "page id=0 file=\"{file}\"");
    glyphs.sort_by_key(|g| g.ch);
    let _ = writeln!(fnt, "chars count={}", glyphs.len() + 1);
    let mut line = |id: u32, g: Option<&Placed>, advance: u32| {
        let (at, size, offset) = g.map_or(((0, 0), (0, 0), (0, 0)), |g| (g.at, g.size, g.offset));
        let _ = writeln!(
            fnt,
            "char id={id} x={} y={} width={} height={} xoffset={} yoffset={} xadvance={advance} \
             page=0 chnl=15",
            at.0, at.1, size.0, size.1, offset.0, offset.1
        );
    };
    line(' ' as u32, None, view.space_width);
    for g in &glyphs {
        line(g.ch as u32, Some(g), g.advance);
    }
    let _ = writeln!(fnt, "kernings count=0");
    Ok(BmFont {
        fnt,
        png: atlas.to_png()?,
    })
}

/// Acomoda los glifos en estantes (los más altos primero) dentro de un ancho de potencia de 2,
/// el menor que deja el atlas más o menos cuadrado; devuelve el tamaño del atlas (también
/// potencias de 2).
fn pack(glyphs: &mut [Placed]) -> (u32, u32) {
    let area: u32 = glyphs
        .iter()
        .map(|g| (g.size.0 + SPACING) * (g.size.1 + SPACING))
        .sum();
    let widest = glyphs.iter().map(|g| g.size.0).max().unwrap_or(0);
    let width = ((area as f64).sqrt().ceil() as u32)
        .max(widest + SPACING)
        .max(1)
        .next_power_of_two();
    glyphs.sort_by(|a, b| b.size.1.cmp(&a.size.1).then(a.ch.cmp(&b.ch)));
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    for g in glyphs.iter_mut() {
        if g.size.0 == 0 || g.size.1 == 0 {
            continue;
        }
        if x + g.size.0 > width {
            x = 0;
            y += shelf + SPACING;
            shelf = 0;
        }
        g.at = (x, y);
        x += g.size.0 + SPACING;
        shelf = shelf.max(g.size.1);
    }
    (width, (y + shelf).max(1).next_power_of_two())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::sample_font;
    use cs_core::FontMode;

    /// Los valores de una línea `char id=…` del `.fnt`.
    fn char_line(fnt: &str, id: u32) -> std::collections::BTreeMap<String, i64> {
        let line = fnt
            .lines()
            .find(|l| l.starts_with(&format!("char id={id} ")))
            .unwrap_or_else(|| panic!("no char {id}"));
        line.split_whitespace()
            .skip(1)
            .map(|kv| {
                let (k, v) = kv.split_once('=').unwrap();
                (k.to_owned(), v.parse().unwrap())
            })
            .collect()
    }

    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(std::io::Cursor::new(png));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        (info.width, info.height, buf)
    }

    #[test]
    fn the_fnt_has_the_header_the_space_and_every_glyph() {
        let bm = export_bmfont(&sample_font(FontMode::OneBit), "sample.png").unwrap();
        let lines: Vec<&str> = bm.fnt.lines().collect();
        assert!(lines[0].starts_with("info face=\"Sample\" size=10 "));
        assert!(lines[1].starts_with("common lineHeight=10 base=8 "));
        assert_eq!(lines[2], "page id=0 file=\"sample.png\"");
        assert_eq!(lines[3], "chars count=7");
        assert_eq!(*lines.last().unwrap(), "kernings count=0");
        let space = char_line(&bm.fnt, 32);
        assert_eq!((space["width"], space["xadvance"]), (0, 3));
        let a = char_line(&bm.fnt, 'A' as u32);
        assert_eq!((a["width"], a["height"]), (5, 7));
        assert_eq!((a["xoffset"], a["yoffset"], a["xadvance"]), (0, 1, 6));
        let i = char_line(&bm.fnt, 'i' as u32);
        assert_eq!((i["width"], i["xadvance"]), (1, 3));
        let z = char_line(&bm.fnt, 'z' as u32);
        assert_eq!((z["width"], z["height"], z["xadvance"]), (0, 0, 4));
        assert!(bm.fnt.contains(&format!("char id={} ", 0x1F600)));
    }

    #[test]
    fn the_atlas_has_each_glyph_in_its_place_in_white_or_its_colors() {
        let artboard = sample_font(FontMode::OneBit);
        let view = artboard.font_view().unwrap();
        let bm = export_bmfont(&artboard, "s.png").unwrap();
        let (w, h, rgba) = decode(&bm.png);
        assert!(w.is_power_of_two() && h.is_power_of_two());
        let at = |x: i64, y: i64| {
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]]
        };
        for (ch, glyph) in &view.glyphs {
            let c = char_line(&bm.fnt, *ch as u32);
            for &(x, y, _) in &glyph.pixels {
                let (ax, ay) = (
                    c["x"] + x as i64 - c["xoffset"],
                    c["y"] + y as i64 - c["yoffset"],
                );
                assert_eq!(at(ax, ay), [255, 255, 255, 255], "{ch}");
            }
        }
        // La tinta es todo lo que hay.
        let ink: usize = view.glyphs.values().map(|g| g.pixels.len()).sum();
        assert_eq!(rgba.chunks(4).filter(|p| p[3] > 0).count(), ink);
        // En color, sus colores.
        let bm = export_bmfont(&sample_font(FontMode::Color), "s.png").unwrap();
        let (w, _, rgba) = decode(&bm.png);
        let a = char_line(&bm.fnt, 'A' as u32);
        let i = (((a["y"] + 3) as u32 * w + a["x"] as u32) * 4) as usize;
        assert_eq!(&rgba[i..i + 4], &[20, 40, 200, 255], "the blue bar");
    }

    #[test]
    fn glyphs_do_not_overlap_in_the_atlas() {
        let mut glyphs: Vec<Placed> = (0..40)
            .map(|i| Placed {
                ch: char::from_u32(0x41 + i).unwrap(),
                at: (0, 0),
                size: (1 + i % 7, 1 + i % 5),
                offset: (0, 0),
                advance: 1,
                pixels: Vec::new(),
            })
            .collect();
        let (w, h) = pack(&mut glyphs);
        for (i, a) in glyphs.iter().enumerate() {
            assert!(a.at.0 + a.size.0 <= w && a.at.1 + a.size.1 <= h);
            for b in &glyphs[i + 1..] {
                let apart = a.at.0 + a.size.0 + SPACING <= b.at.0
                    || b.at.0 + b.size.0 + SPACING <= a.at.0
                    || a.at.1 + a.size.1 + SPACING <= b.at.1
                    || b.at.1 + b.size.1 + SPACING <= a.at.1;
                assert!(apart, "{} and {}", a.ch, b.ch);
            }
        }
    }
}
