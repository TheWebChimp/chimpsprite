//! Lo que el lienzo dibuja sobre un artboard de fuente (no son pixeles, no se exportan): la letra
//! fantasma detrás, las guías de las medidas y las líneas del borde izquierdo y del ancho.

use eframe::egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke};

use cs_core::FontInfo;

pub const GUIDE_RED: Color32 = Color32::from_rgb(230, 40, 60);
pub const GUIDE_DIM: Color32 = Color32::from_rgba_unmultiplied_const(230, 40, 60, 90);
const GHOST: Color32 = Color32::from_rgba_premultiplied(60, 60, 60, 60);

/// La x en pantalla de la línea del ancho.
pub fn advance_x(pixels: Rect, advance: u32, scale: f32) -> f32 {
    pixels.left() + advance as f32 * scale
}

/// Una línea horizontal en la fila `row` (su borde de arriba) de lado a lado.
fn row_line(pixels: Rect, row: u32, scale: f32, stroke: Stroke, dashed: bool) -> Vec<Shape> {
    let y = pixels.top() + row as f32 * scale;
    let (a, b) = (Pos2::new(pixels.left(), y), Pos2::new(pixels.right(), y));
    if dashed {
        Shape::dashed_line(&[a, b], stroke, 4.0, 3.0)
    } else {
        vec![Shape::line_segment([a, b], stroke)]
    }
}

/// La letra fantasma (detrás de los pixeles): `glyph` en gris, de la altura de las mayúsculas,
/// apoyada en la baseline y desde el borde izquierdo.
pub fn ghost(
    painter: &Painter,
    pixels: Rect,
    font: &FontInfo,
    glyph: char,
    scale: f32,
) -> Option<Shape> {
    if !font.ghost.enabled || font.metrics.cap_height == 0 {
        return None;
    }
    let family = if font.ghost.monospace {
        FontFamily::Monospace
    } else {
        FontFamily::Proportional
    };
    // La altura de las mayúsculas de las fuentes de egui es ~0.7 del tamaño.
    let size = font.metrics.cap_height as f32 * scale / 0.7;
    let galley = painter.layout_no_wrap(glyph.to_string(), FontId::new(size, family), GHOST);
    // La baseline del texto dentro del galley (la de su primer carácter).
    let text_baseline = galley
        .rows
        .first()
        .and_then(|r| r.glyphs.first().map(|g| r.pos.y + g.pos.y))
        .unwrap_or(size * 0.8);
    let baseline = pixels.top() + font.metrics.baseline as f32 * scale;
    let at = Pos2::new(pixels.left(), baseline - text_baseline);
    Some(Shape::galley(at, galley, GHOST))
}

/// Las guías y las líneas del borde izquierdo y del ancho (encima de los pixeles).
pub fn guides(pixels: Rect, font: &FontInfo, advance: u32, scale: f32) -> Vec<Shape> {
    let m = font.metrics;
    let strong = Stroke::new(1.0, GUIDE_RED);
    let dim = Stroke::new(1.0, GUIDE_DIM);
    let mut shapes = Vec::new();
    shapes.extend(row_line(pixels, m.baseline, scale, strong, false));
    shapes.extend(row_line(
        pixels,
        m.baseline.saturating_sub(m.cap_height),
        scale,
        dim,
        true,
    ));
    shapes.extend(row_line(
        pixels,
        m.baseline.saturating_sub(m.x_height),
        scale,
        dim,
        true,
    ));
    shapes.extend(row_line(pixels, m.baseline + m.descent, scale, dim, true));
    let left = pixels.left();
    shapes.push(Shape::line_segment(
        [
            Pos2::new(left, pixels.top()),
            Pos2::new(left, pixels.bottom()),
        ],
        strong,
    ));
    let x = advance_x(pixels, advance, scale);
    shapes.push(Shape::line_segment(
        [Pos2::new(x, pixels.top()), Pos2::new(x, pixels.bottom())],
        Stroke::new(2.0, GUIDE_RED),
    ));
    shapes
}

/// El rect de golpe de la línea del ancho: 6 puntos de ancho, de arriba abajo del artboard.
pub fn advance_hit(pixels: Rect, advance: u32, scale: f32) -> Rect {
    let x = advance_x(pixels, advance, scale);
    Rect::from_center_size(
        Pos2::new(x, pixels.center().y),
        eframe::egui::vec2(6.0, pixels.height()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_advance_line_sits_advance_pixels_from_the_left_edge() {
        let pixels = Rect::from_min_size(Pos2::new(10.0, 20.0), eframe::egui::vec2(64.0, 64.0));
        assert_eq!(advance_x(pixels, 3, 4.0), 22.0);
        let hit = advance_hit(pixels, 3, 4.0);
        assert_eq!(hit.center().x, 22.0);
        assert_eq!(hit.width(), 6.0);
        assert_eq!(hit.height(), 64.0);
    }
}
