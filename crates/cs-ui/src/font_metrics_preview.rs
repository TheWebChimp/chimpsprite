//! La vista previa de métricas de los ajustes de FontForge (el recuadro gris del mockup, a la
//! derecha de los steppers): un glifo a escala entera sobre el damero del lienzo, con las guías
//! del lienzo (`font_overlay`): baseline sólida, cap height, x-height y descent punteadas, el
//! borde izquierdo y el ancho de avance. Con el puntero sobre la fila de un número, su línea se
//! resalta; Letter spacing y Space width, que en el lienzo no tienen guía, solo entonces.
//!
//! [`layout`] es la lógica (escala, celda y líneas, sin pintar); [`paint`] la dibuja.

use cs_core::{FontMetrics, PixelFontView};
use eframe::egui::{Color32, Painter, Rect, Vec2, pos2, vec2};

use crate::font_overlay::{GUIDE_DIM, GUIDE_RED};
use crate::theme::{CHECKER_DARK, CHECKER_LIGHT};

/// Los números de los ajustes, en el orden de sus filas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    Baseline,
    XHeight,
    LetterSpacing,
    CapHeight,
    Descent,
    SpaceWidth,
}

impl Metric {
    pub const ALL: [Metric; 6] = [
        Metric::Baseline,
        Metric::XHeight,
        Metric::LetterSpacing,
        Metric::CapHeight,
        Metric::Descent,
        Metric::SpaceWidth,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Metric::Baseline => "Baseline",
            Metric::XHeight => "x-height",
            Metric::LetterSpacing => "Letter spacing",
            Metric::CapHeight => "Cap height",
            Metric::Descent => "Descent",
            Metric::SpaceWidth => "Space width",
        }
    }
}

/// El aire mínimo entre la celda y el borde del recuadro.
pub const PAD: f32 = 2.0;
/// El grosor de una línea resaltada (y el de la del ancho, como en el lienzo).
pub const STRONG_W: f32 = 2.0;
/// Las celdas del damero, en pixeles de la fuente (las del lienzo).
const CHECKER_CELL: u32 = 16;
/// Los trazos y los huecos de las líneas punteadas.
const DASH: f32 = 3.0;
const GAP: f32 = 2.0;
/// La tinta de los glifos de 1 bit (la del Test Drive).
const INK: Color32 = crate::font_panel::TEST_INK;

/// Las letras de muestra, si el glifo actual está vacío.
const SAMPLES: [char; 6] = ['H', 'x', 'A', 'a', 'n', 'o'];

/// Lo que se mide: la celda, las medidas, el avance del glifo y los dos espaciados.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbers {
    pub cell: (u32, u32),
    pub metrics: FontMetrics,
    pub advance: u32,
    pub letter_spacing: u32,
    pub space_width: u32,
}

/// Una línea: su rect (de 1 o de [`STRONG_W`] de grueso), su color, si va punteada y de qué
/// número es (`None`: el borde izquierdo y el ancho, que no se resaltan).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub metric: Option<Metric>,
    pub rect: Rect,
    pub color: Color32,
    pub dashed: bool,
}

/// Dónde va todo dentro del recuadro.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Puntos por pixel de la fuente (entero).
    pub scale: f32,
    /// La celda del glifo, centrada y en puntos enteros.
    pub cell: Rect,
    /// Las líneas, en el orden en que se pintan (la resaltada al final).
    pub lines: Vec<Line>,
}

/// El glifo que se muestra: el actual si tiene pixeles; si no, la primera letra de muestra
/// dibujada, o la primera dibujada de la fuente; si no hay ninguna, el actual (vacío).
pub fn preview_char(view: &PixelFontView, current: Option<char>) -> Option<char> {
    let inked = |c: &char| view.glyphs.get(c).is_some_and(|g| !g.pixels.is_empty());
    current
        .filter(inked)
        .or_else(|| SAMPLES.into_iter().find(inked))
        .or_else(|| view.glyphs.keys().copied().find(inked))
        .or(current)
}

/// La escala entera más grande con la que la celda cabe en `area` con [`PAD`] de aire (mínimo
/// 1), la celda centrada y las líneas. Las de arriba (cap height, x-height) van por encima del
/// borde de su fila y las de abajo (baseline, descent) por debajo, para no tapar la tinta; el
/// borde izquierdo, a la izquierda de la celda; el ancho y los espaciados, a la derecha de su
/// columna.
pub fn layout(area: Rect, n: &Numbers, hovered: Option<Metric>) -> Layout {
    let (w, h) = (n.cell.0.max(1) as f32, n.cell.1.max(1) as f32);
    let fit = ((area.width() - 2.0 * PAD) / w).min((area.height() - 2.0 * PAD) / h);
    let scale = fit.floor().max(1.0);
    let size = vec2(w, h) * scale;
    let min = area.center() - size / 2.0;
    let cell = Rect::from_min_size(pos2(min.x.floor(), min.y.floor()), size);
    let m = n.metrics;
    let row_y = |row: u32| cell.top() + row as f32 * scale;
    let col_x = |col: u32| cell.left() + col as f32 * scale;
    let on = |metric: Metric| hovered == Some(metric);
    let mut lines = Vec::new();
    // Una línea de un número: tenue y punteada (o la baseline, roja y sólida); resaltada, roja,
    // sólida y gruesa.
    let mut add = |metric: Option<Metric>, rect: Rect, color: Color32, dashed: bool| {
        lines.push(Line {
            metric,
            rect,
            color,
            dashed,
        });
    };
    let horizontal = |y: f32, below: bool, thick: f32| {
        let (top, bottom) = if below {
            (y, y + thick)
        } else {
            (y - thick, y)
        };
        Rect::from_x_y_ranges(cell.x_range(), top..=bottom)
    };
    let vertical = |x: f32, right: bool, thick: f32| {
        let (left, r) = if right {
            (x, x + thick)
        } else {
            (x - thick, x)
        };
        Rect::from_x_y_ranges(left..=r, cell.y_range())
    };
    let rows = [
        (Metric::Baseline, m.baseline, true),
        (
            Metric::CapHeight,
            m.baseline.saturating_sub(m.cap_height),
            false,
        ),
        (
            Metric::XHeight,
            m.baseline.saturating_sub(m.x_height),
            false,
        ),
        (Metric::Descent, m.baseline + m.descent, true),
    ];
    for (metric, row, below) in rows {
        let (thick, color, dashed) = match (on(metric), metric) {
            (true, _) => (STRONG_W, GUIDE_RED, false),
            (false, Metric::Baseline) => (1.0, GUIDE_RED, false),
            (false, _) => (1.0, GUIDE_DIM, true),
        };
        add(
            Some(metric),
            horizontal(row_y(row), below, thick),
            color,
            dashed,
        );
    }
    add(None, vertical(cell.left(), false, 1.0), GUIDE_RED, false);
    add(
        None,
        vertical(col_x(n.advance), true, STRONG_W),
        GUIDE_RED,
        false,
    );
    if on(Metric::LetterSpacing) {
        let x = col_x(n.advance + n.letter_spacing);
        add(
            Some(Metric::LetterSpacing),
            vertical(x, true, STRONG_W),
            GUIDE_RED,
            false,
        );
    }
    if on(Metric::SpaceWidth) {
        let x = col_x(n.space_width);
        add(
            Some(Metric::SpaceWidth),
            vertical(x, true, STRONG_W),
            GUIDE_RED,
            false,
        );
    }
    // La resaltada, encima de todo.
    lines.sort_by_key(|l| l.metric.is_some() && l.metric == hovered);
    Layout { scale, cell, lines }
}

/// Pinta la vista previa en `area` (lo de adentro del marco): el gris del lienzo, el damero en
/// la celda, el glifo `ch` de `view` y las líneas.
pub fn paint(
    painter: &Painter,
    area: Rect,
    view: &PixelFontView,
    ch: Option<char>,
    n: &Numbers,
    hovered: Option<Metric>,
) {
    let painter = painter.with_clip_rect(area.intersect(painter.clip_rect()));
    painter.rect_filled(area, 0.0, CHECKER_LIGHT);
    let l = layout(area, n, hovered);
    // El damero del lienzo: celdas de 16 pixeles, la de arriba a la izquierda clara.
    let (w, h) = n.cell;
    for cy in (0..h).step_by(CHECKER_CELL as usize) {
        for cx in (0..w).step_by(CHECKER_CELL as usize) {
            if ((cx + cy) / CHECKER_CELL).is_multiple_of(2) {
                continue;
            }
            let min = l.cell.min + vec2(cx as f32, cy as f32) * l.scale;
            let rect = Rect::from_min_size(min, Vec2::splat(CHECKER_CELL as f32 * l.scale));
            painter.rect_filled(rect.intersect(l.cell), 0.0, CHECKER_DARK);
        }
    }
    let glyph = ch.and_then(|c| view.glyphs.get(&c));
    for &(x, y, c) in glyph.map_or(&[][..], |g| &g.pixels[..]) {
        let color = c.map_or(INK, |c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
        let min = l.cell.min + vec2(x as f32, y as f32) * l.scale;
        painter.rect_filled(Rect::from_min_size(min, Vec2::splat(l.scale)), 0.0, color);
    }
    for line in &l.lines {
        if !line.dashed {
            painter.rect_filled(line.rect, 0.0, line.color);
            continue;
        }
        // De DASH en DASH a lo largo (las punteadas son horizontales).
        let mut x = line.rect.left();
        while x < line.rect.right() {
            let right = (x + DASH).min(line.rect.right());
            let dash = Rect::from_x_y_ranges(x..=right, line.rect.y_range());
            painter.rect_filled(dash, 0.0, line.color);
            x += DASH + GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::GlyphPixels;
    use eframe::egui::Pos2;

    /// La fuente nueva de 16×16 del mockup: baseline 12, x-height 6, cap height 10, descent 4.
    fn numbers() -> Numbers {
        Numbers {
            cell: (16, 16),
            metrics: FontMetrics {
                baseline: 12,
                cap_height: 10,
                x_height: 6,
                descent: 4,
            },
            advance: 9,
            letter_spacing: 1,
            space_width: 3,
        }
    }

    /// Lo de adentro del recuadro gris del mockup (166×121 con su marco), en (10, 20).
    fn area() -> Rect {
        Rect::from_min_size(Pos2::new(10.0, 20.0), vec2(164.0, 119.0))
    }

    fn line(l: &Layout, metric: Metric) -> Line {
        *l.lines
            .iter()
            .find(|x| x.metric == Some(metric))
            .unwrap_or_else(|| panic!("the {metric:?} line"))
    }

    #[test]
    fn the_cell_is_scaled_by_a_whole_number_and_centered() {
        let l = layout(area(), &numbers(), None);
        // 119 − 2·2 = 115 de alto: caben 7 puntos por pixel (112).
        assert_eq!(l.scale, 7.0);
        assert_eq!(l.cell.size(), vec2(112.0, 112.0));
        // Centrada: (164 − 112) / 2 = 26 y (119 − 112) / 2 = 3.5, hacia abajo.
        assert_eq!(l.cell.min, Pos2::new(36.0, 23.0));
        // Una celda que no cabe ni a 1: a 1, centrada igual.
        let big = Numbers {
            cell: (400, 300),
            ..numbers()
        };
        assert_eq!(layout(area(), &big, None).scale, 1.0);
        // Una celda ancha: manda el ancho.
        let wide = Numbers {
            cell: (32, 8),
            ..numbers()
        };
        assert_eq!(layout(area(), &wide, None).scale, 5.0);
    }

    #[test]
    fn the_lines_sit_on_the_metric_rows_like_the_canvas_guides() {
        let l = layout(area(), &numbers(), None);
        let top = l.cell.top();
        // La baseline (fila 12), roja y sólida, debajo del borde: no tapa la tinta de arriba.
        let base = line(&l, Metric::Baseline);
        assert_eq!(base.rect.y_range(), (top + 84.0..=top + 85.0));
        assert_eq!(base.rect.x_range(), l.cell.x_range());
        assert_eq!((base.color, base.dashed), (GUIDE_RED, false));
        // Cap height (fila 2) y x-height (fila 6), tenues, punteadas y por encima del borde.
        let cap = line(&l, Metric::CapHeight);
        assert_eq!(cap.rect.y_range(), (top + 13.0..=top + 14.0));
        assert_eq!((cap.color, cap.dashed), (GUIDE_DIM, true));
        let x = line(&l, Metric::XHeight);
        assert_eq!(x.rect.y_range(), (top + 41.0..=top + 42.0));
        // Descent: 4 debajo de la baseline, en el borde de abajo de la celda.
        let descent = line(&l, Metric::Descent);
        assert_eq!(descent.rect.top(), l.cell.bottom());
        // El borde izquierdo y el ancho (9 pixeles), sin número.
        let plain: Vec<Rect> = l
            .lines
            .iter()
            .filter(|x| x.metric.is_none())
            .map(|x| x.rect)
            .collect();
        assert_eq!(plain.len(), 2);
        assert_eq!(plain[0].right(), l.cell.left());
        assert_eq!(plain[1].left(), l.cell.left() + 63.0);
        assert_eq!(plain[1].width(), STRONG_W);
        // Los espaciados no tienen guía si no se resaltan.
        assert!(
            l.lines
                .iter()
                .all(|x| !matches!(x.metric, Some(Metric::LetterSpacing | Metric::SpaceWidth)))
        );
    }

    #[test]
    fn the_hovered_number_draws_its_line_strong_and_last() {
        let l = layout(area(), &numbers(), Some(Metric::CapHeight));
        let cap = line(&l, Metric::CapHeight);
        assert_eq!((cap.color, cap.dashed), (GUIDE_RED, false));
        assert_eq!(cap.rect.height(), STRONG_W);
        // Sigue por encima del borde de su fila.
        assert_eq!(cap.rect.bottom(), l.cell.top() + 14.0);
        assert_eq!(l.lines.last(), Some(&cap));
        // Las demás, igual que sin resaltar.
        let x = line(&l, Metric::XHeight);
        assert_eq!((x.color, x.dashed), (GUIDE_DIM, true));
        // Letter spacing: la columna después del avance más el espaciado (9 + 1).
        let l = layout(area(), &numbers(), Some(Metric::LetterSpacing));
        let spacing = line(&l, Metric::LetterSpacing);
        assert_eq!(spacing.rect.left(), l.cell.left() + 70.0);
        assert_eq!(spacing.rect.y_range(), l.cell.y_range());
        // Space width: 3 desde el borde izquierdo.
        let l = layout(area(), &numbers(), Some(Metric::SpaceWidth));
        assert_eq!(
            line(&l, Metric::SpaceWidth).rect.left(),
            l.cell.left() + 21.0
        );
    }

    #[test]
    fn an_empty_glyph_shows_a_drawn_sample_letter() {
        let glyph =
            |pixels: Vec<(u32, u32, Option<cs_core::Rgba>)>| GlyphPixels { pixels, advance: 4 };
        let mut view = PixelFontView {
            cell: (8, 8),
            metrics: FontMetrics::for_cell(8),
            space_width: 3,
            missing_width: 4,
            glyphs: Default::default(),
            kerning: Default::default(),
        };
        assert_eq!(preview_char(&view, None), None);
        view.glyphs.insert('B', glyph(vec![]));
        assert_eq!(preview_char(&view, Some('B')), Some('B'), "nothing drawn");
        view.glyphs.insert('Q', glyph(vec![(0, 0, None)]));
        assert_eq!(preview_char(&view, Some('B')), Some('Q'), "the first drawn");
        view.glyphs.insert('x', glyph(vec![(1, 1, None)]));
        assert_eq!(preview_char(&view, Some('B')), Some('x'), "a sample first");
        view.glyphs.insert('H', glyph(vec![(1, 1, None)]));
        assert_eq!(preview_char(&view, Some('B')), Some('H'));
        view.glyphs.insert('B', glyph(vec![(2, 2, None)]));
        assert_eq!(preview_char(&view, Some('B')), Some('B'), "the current one");
    }
}
