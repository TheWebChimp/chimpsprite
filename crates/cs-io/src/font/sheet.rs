//! Glifos desde una imagen: una hoja con las letras en cuadrícula (como Import Sprite Sheet) se
//! parte en celdas que toman, en orden de lectura, los caracteres de un texto. Cada pixel se
//! decide por los colores de tinta y de fondo.

use std::collections::BTreeMap;

use cs_core::{GlyphBuild, Rgba};

use crate::export::Bitmap;
use crate::import::SheetGrid;

/// Lo que se parece al fondo (distancia en RGB) cuenta como fondo.
pub const PAPER_TOLERANCE: u32 = 48;
/// Con menos alfa, transparente.
const OPAQUE: u8 = 128;

/// Cómo leer una hoja de glifos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphSheet {
    pub grid: SheetGrid,
    /// Los caracteres de las celdas en orden (los saltos de línea no cuentan; un espacio salta
    /// una celda).
    pub chars: String,
    pub ink: Rgba,
    /// El fondo; transparente: solo el alfa decide.
    pub paper: Rgba,
    /// Con los colores de la hoja (fuente de color); si no, 1 bit.
    pub color: bool,
    /// La fila de la baseline dentro de la celda de la hoja.
    pub baseline: u32,
    /// La de la fuente: los glifos se recorren para que caigan en ella.
    pub target_baseline: u32,
}

/// Los caracteres de las celdas, en orden: `None` es una celda que se salta.
pub fn sheet_chars(text: &str) -> Vec<Option<char>> {
    text.chars()
        .filter(|c| *c != '\n' && *c != '\r')
        .map(|c| (!c.is_whitespace()).then_some(c))
        .collect()
}

fn distance(a: Rgba, b: Rgba) -> u32 {
    let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2) as u32;
    (d(a.r, b.r) + d(a.g, b.g) + d(a.b, b.b)).isqrt()
}

/// El fondo (el pixel de arriba a la izquierda) y la tinta (el color opaco más común que no se
/// parece al fondo; negro si no hay).
pub fn sheet_colors(sheet: &Bitmap) -> (Rgba, Rgba) {
    let paper = sheet.pixels.first().copied().unwrap_or(Rgba::TRANSPARENT);
    let mut counts: BTreeMap<(u8, u8, u8), usize> = BTreeMap::new();
    for &p in &sheet.pixels {
        if p.a >= OPAQUE && !is_paper(p, paper) {
            *counts.entry((p.r, p.g, p.b)).or_default() += 1;
        }
    }
    let ink = counts
        .into_iter()
        .max_by_key(|&(_, n)| n)
        .map_or(Rgba::new(0, 0, 0, 255), |((r, g, b), _)| {
            Rgba::new(r, g, b, 255)
        });
    (paper, ink)
}

fn is_paper(p: Rgba, paper: Rgba) -> bool {
    p.a < OPAQUE || (paper.a >= OPAQUE && distance(p, paper) <= PAPER_TOLERANCE)
}

/// Los glifos de la hoja: uno por celda con carácter y algo de tinta, sus pixeles recorridos en
/// y para que la baseline de la celda caiga en la de la fuente (lo de arriba de la fila 0 se
/// pierde).
pub fn glyphs_from_sheet(sheet: &Bitmap, options: &GlyphSheet) -> Vec<GlyphBuild> {
    let g = options.grid;
    let (cols, rows) = g.count((sheet.width, sheet.height));
    let (w, h) = (g.cell.0.max(1), g.cell.1.max(1));
    let dy = options.target_baseline as i64 - options.baseline as i64;
    let cells = (0..rows).flat_map(|row| (0..cols).map(move |col| (col, row)));
    let mut out = Vec::new();
    for ((col, row), ch) in cells.zip(sheet_chars(&options.chars)) {
        let Some(ch) = ch else {
            continue;
        };
        let x0 = g.offset.0 + col * (w + g.padding.0);
        let y0 = g.offset.1 + row * (h + g.padding.1);
        let mut pixels = Vec::new();
        for y in 0..h {
            let ty = y as i64 + dy;
            if ty < 0 {
                continue;
            }
            for x in 0..w {
                let p = sheet.pixels[((y0 + y) * sheet.width + x0 + x) as usize];
                if is_paper(p, options.paper) {
                    continue;
                }
                let color = if options.color {
                    Some(p)
                } else if options.paper.a < OPAQUE
                    || distance(p, options.ink) < distance(p, options.paper)
                {
                    None
                } else {
                    continue;
                };
                pixels.push((x, ty as u32, color));
            }
        }
        if !pixels.is_empty() {
            out.push(GlyphBuild {
                ch,
                advance: None,
                pixels,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgba = Rgba::new(255, 255, 255, 255);
    const BLACK: Rgba = Rgba::new(0, 0, 0, 255);
    const GRAY: Rgba = Rgba::new(90, 90, 90, 255);
    const RED: Rgba = Rgba::new(220, 20, 20, 255);

    /// Una hoja blanca de 3 celdas de 4×4 en una fila, con 1 de aire, y unos pixeles.
    fn sheet(points: &[(u32, u32, Rgba)]) -> Bitmap {
        let mut b = Bitmap::new(14, 4);
        b.pixels.fill(WHITE);
        for &(x, y, c) in points {
            b.pixels[(y * 14 + x) as usize] = c;
        }
        b
    }

    fn options(chars: &str) -> GlyphSheet {
        GlyphSheet {
            grid: SheetGrid {
                cell: (4, 4),
                padding: (1, 0),
                offset: (0, 0),
            },
            chars: chars.into(),
            ink: BLACK,
            paper: WHITE,
            color: false,
            baseline: 3,
            target_baseline: 3,
        }
    }

    #[test]
    fn cells_take_the_chars_in_order_and_spaces_skip_one() {
        let b = sheet(&[(1, 1, BLACK), (6, 2, BLACK), (11, 0, BLACK)]);
        let glyphs = glyphs_from_sheet(&b, &options("A\n C"));
        let chars: Vec<char> = glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, vec!['A', 'C']);
        assert_eq!(glyphs[0].pixels, vec![(1, 1, None)]);
        assert_eq!(glyphs[1].pixels, vec![(1, 0, None)]);
        assert_eq!(
            sheet_chars("a b\nc"),
            vec![Some('a'), None, Some('b'), Some('c')]
        );
    }

    #[test]
    fn one_bit_takes_what_is_nearer_the_ink_and_color_keeps_the_colors() {
        let light = Rgba::new(200, 200, 200, 255);
        let b = sheet(&[(0, 0, GRAY), (1, 0, light), (2, 0, RED), (3, 0, WHITE)]);
        let glyphs = glyphs_from_sheet(&b, &options("A"));
        assert_eq!(glyphs[0].pixels, vec![(0, 0, None), (2, 0, None)]);
        let color = GlyphSheet {
            color: true,
            ..options("A")
        };
        let glyphs = glyphs_from_sheet(&b, &color);
        assert_eq!(
            glyphs[0].pixels,
            vec![(0, 0, Some(GRAY)), (1, 0, Some(light)), (2, 0, Some(RED))]
        );
    }

    #[test]
    fn the_sheet_baseline_lands_on_the_fonts() {
        let b = sheet(&[(0, 0, BLACK), (0, 3, BLACK)]);
        let up = GlyphSheet {
            target_baseline: 2,
            ..options("A")
        };
        assert_eq!(glyphs_from_sheet(&b, &up)[0].pixels, vec![(0, 2, None)]);
        let down = GlyphSheet {
            target_baseline: 5,
            ..options("A")
        };
        assert_eq!(
            glyphs_from_sheet(&b, &down)[0].pixels,
            vec![(0, 2, None), (0, 5, None)]
        );
    }

    #[test]
    fn the_default_colors_are_the_corner_and_the_most_common_other() {
        let b = sheet(&[(1, 1, RED), (2, 1, GRAY), (3, 1, GRAY)]);
        assert_eq!(sheet_colors(&b), (WHITE, GRAY));
        let mut clear = Bitmap::new(2, 1);
        clear.pixels[1] = RED;
        assert_eq!(sheet_colors(&clear), (Rgba::TRANSPARENT, RED));
        let glyphs = glyphs_from_sheet(
            &clear,
            &GlyphSheet {
                grid: SheetGrid {
                    cell: (2, 1),
                    padding: (0, 0),
                    offset: (0, 0),
                },
                paper: Rgba::TRANSPARENT,
                baseline: 0,
                target_baseline: 0,
                ..options("x")
            },
        );
        assert_eq!(glyphs[0].pixels, vec![(1, 0, None)], "transparent paper");
    }
}
