//! Lo que el lienzo dibuja sobre un artboard de fuente (no son pixeles, no se exportan): la letra
//! fantasma detrás, los componentes de un glifo compuesto (en azul translúcido), las guías de
//! las medidas y las líneas del borde izquierdo y del ancho.

use eframe::egui::{Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use cs_core::{ComposedPixels, FontInfo, Sprite};

pub const GUIDE_RED: Color32 = Color32::from_rgb(230, 40, 60);
pub const GUIDE_DIM: Color32 = Color32::from_rgba_unmultiplied_const(230, 40, 60, 90);
const GHOST: Color32 = Color32::from_rgba_premultiplied(60, 60, 60, 60);

/// Los componentes de un compuesto (debajo de los pixeles propios) y el elegido, más fuerte.
const COMPONENT: Color32 = Color32::from_rgba_unmultiplied_const(60, 110, 235, 90);
const COMPONENT_SELECTED: Color32 = Color32::from_rgba_unmultiplied_const(60, 110, 235, 170);
const COMPONENT_BOX: Color32 = Color32::from_rgb(60, 110, 235);

/// Lo que se ve del componente `i` del glifo `index` en la celda, con `offset` en lugar del suyo
/// si se da (el que se está arrastrando).
pub fn component_pixels(
    font: &FontInfo,
    sprite: &Sprite,
    index: usize,
    i: usize,
    offset: Option<(i32, i32)>,
) -> ComposedPixels {
    let Some(c) = font.glyphs.get(index).and_then(|g| g.components.get(i)) else {
        return ComposedPixels::new();
    };
    let Some(j) = font.index_of(c.ch).filter(|&j| j != index) else {
        return ComposedPixels::new();
    };
    let (dx, dy) = offset.unwrap_or(c.offset);
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    font.composed(sprite, j)
        .into_iter()
        .filter_map(|((y, x), v)| {
            let (x, y) = (x as i32 + dx, y as i32 + dy);
            ((0..w).contains(&x) && (0..h).contains(&y)).then_some(((y as u32, x as u32), v))
        })
        .collect()
}

/// El componente del glifo `index` que tiene un pixel en `at` (el de más arriba, el último); si
/// ninguno, el primero cuya caja lo contiene.
pub fn component_at(
    font: &FontInfo,
    sprite: &Sprite,
    index: usize,
    at: (i32, i32),
) -> Option<usize> {
    let count = font.glyphs.get(index)?.components.len();
    let layers: Vec<ComposedPixels> = (0..count)
        .map(|i| component_pixels(font, sprite, index, i, None))
        .collect();
    let key = (u32::try_from(at.1).ok()?, u32::try_from(at.0).ok()?);
    (0..count)
        .rev()
        .find(|&i| layers[i].contains_key(&key))
        .or_else(|| {
            (0..count).rev().find(|&i| {
                cs_core::bounds_of(&layers[i])
                    .is_some_and(|b| (b.x0..b.x1).contains(&at.0) && (b.y0..b.y1).contains(&at.1))
            })
        })
}

/// Los componentes del glifo `index` como fantasma: cada pixel en azul translúcido; el
/// `selected`, más fuerte y con su caja punteada. `moved` es el que se arrastra y a dónde.
pub fn components(
    pixels: Rect,
    font: &FontInfo,
    sprite: &Sprite,
    index: usize,
    selected: Option<usize>,
    moved: Option<(usize, (i32, i32))>,
    scale: f32,
) -> Vec<Shape> {
    let count = font.glyphs.get(index).map_or(0, |g| g.components.len());
    let mut mesh = eframe::egui::Mesh::default();
    let mut boxes = Vec::new();
    for i in 0..count {
        let offset = moved.filter(|(m, _)| *m == i).map(|(_, o)| o);
        let layer = component_pixels(font, sprite, index, i, offset);
        let on = selected == Some(i);
        let color = if on { COMPONENT_SELECTED } else { COMPONENT };
        for &(y, x) in layer.keys() {
            let min = pixels.min + Vec2::new(x as f32, y as f32) * scale;
            mesh.add_colored_rect(Rect::from_min_size(min, Vec2::splat(scale)), color);
        }
        if on && let Some(b) = cs_core::bounds_of(&layer) {
            let min = pixels.min + Vec2::new(b.x0 as f32, b.y0 as f32) * scale;
            let size = Vec2::new(b.width() as f32, b.height() as f32) * scale;
            boxes.push(Rect::from_min_size(min, size));
        }
    }
    let mut shapes = vec![Shape::mesh(mesh)];
    let stroke = Stroke::new(1.0, COMPONENT_BOX);
    for r in boxes {
        let corners = [
            r.left_top(),
            r.right_top(),
            r.right_bottom(),
            r.left_bottom(),
            r.left_top(),
        ];
        shapes.extend(Shape::dashed_line(&corners, stroke, 3.0, 2.0));
    }
    shapes
}

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

/// El rect de golpe de la línea del ancho: hasta 6 puntos de ancho (con poco zoom, como mucho
/// una columna de píxeles, mínimo 2), de arriba abajo del artboard.
pub fn advance_hit(pixels: Rect, advance: u32, scale: f32) -> Rect {
    let x = advance_x(pixels, advance, scale);
    let width = 6.0_f32.min(scale.max(2.0));
    Rect::from_center_size(
        Pos2::new(x, pixels.center().y),
        eframe::egui::vec2(width, pixels.height()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{FontMode, GlyphComponent, GlyphInfo, ONE_BIT_INK, font_sprite};

    #[test]
    fn a_component_is_found_by_its_pixels_or_its_box() {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        font.glyphs.push(GlyphInfo {
            components: vec![
                GlyphComponent {
                    ch: 'a',
                    offset: (0, 0),
                },
                GlyphComponent {
                    ch: 'b',
                    offset: (2, 0),
                },
            ],
            ..GlyphInfo::new('é')
        });
        let mut s = font_sprite((8, 8), FontMode::OneBit, font.glyphs.len());
        let layer = s.layers[0].id;
        for (ch, points) in [('a', [(0, 4), (2, 6)]), ('b', [(0, 0), (0, 1)])] {
            let frame = font.index_of(ch).unwrap();
            let mut image = cs_core::Image::new(8, 8, s.color_mode);
            for (x, y) in points {
                image.set(x, y, ONE_BIT_INK).unwrap();
            }
            let id = s.next_image_id();
            s.images.insert(id, std::sync::Arc::new(image));
            let cel = cs_core::Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            };
            s.cels.insert((layer, frame), cel);
        }
        let index = font.glyphs.len() - 1;
        assert_eq!(component_at(&font, &s, index, (2, 1)), Some(1), "b's pixel");
        assert_eq!(
            component_at(&font, &s, index, (1, 5)),
            Some(0),
            "inside a's box"
        );
        assert_eq!(component_at(&font, &s, index, (7, 7)), None);
        assert_eq!(component_at(&font, &s, index, (-1, 0)), None);
        let moved = component_pixels(&font, &s, index, 1, Some((7, 7)));
        assert_eq!(moved.len(), 1, "one pixel fell outside");
        let pixels = Rect::from_min_size(Pos2::ZERO, eframe::egui::vec2(64.0, 64.0));
        let shapes = components(pixels, &font, &s, index, Some(1), None, 8.0);
        assert!(shapes.len() > 1, "the mesh and the dashed box");
    }

    #[test]
    fn the_advance_line_sits_advance_pixels_from_the_left_edge() {
        let pixels = Rect::from_min_size(Pos2::new(10.0, 20.0), eframe::egui::vec2(64.0, 64.0));
        assert_eq!(advance_x(pixels, 3, 4.0), 22.0);
        let hit = advance_hit(pixels, 3, 4.0);
        assert_eq!(hit.center().x, 22.0);
        assert_eq!(hit.width(), 4.0, "never wider than one pixel column");
        assert_eq!(hit.height(), 64.0);
        assert_eq!(advance_hit(pixels, 3, 16.0).width(), 6.0);
        assert_eq!(advance_hit(pixels, 3, 1.0).width(), 2.0);
    }
}
