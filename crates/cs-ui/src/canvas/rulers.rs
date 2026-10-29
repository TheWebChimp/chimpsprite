//! Las reglas de arriba y de la izquierda del lienzo (las de egui y las de Pixel UI).

use super::*;

impl CanvasView {
    /// Grosor de las reglas: el del mockup con Pixel UI; si no, el de siempre.
    pub(super) fn ruler_px(&self) -> f32 {
        if self.pixel_rulers.is_some() {
            pixel_rulers::RULER_PX
        } else {
            RULER_PX
        }
    }

    /// Reglas superior e izquierda (sin la esquina), de `px` de grosor.
    pub(super) fn ruler_rects(rect: Rect, px: f32) -> (Rect, Rect) {
        let top = Rect::from_min_max(
            rect.min + Vec2::new(px, 0.0),
            Pos2::new(rect.right(), rect.top() + px),
        );
        let left = Rect::from_min_max(
            rect.min + Vec2::new(0.0, px),
            Pos2::new(rect.left() + px, rect.bottom()),
        );
        (top, left)
    }

    /// Reglas en píxeles del artboard activo (origen en su esquina superior izquierda).
    pub(super) fn draw_rulers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        pointer: Option<Pos2>,
    ) {
        let (top, left) = Self::ruler_rects(rect, RULER_PX);
        let corner = Rect::from_min_size(rect.min, Vec2::splat(RULER_PX));
        for r in [top, left, corner] {
            painter.rect_filled(r, 0.0, theme::RULER_BG);
        }
        painter.hline(
            rect.x_range(),
            top.bottom(),
            Stroke::new(1.0, theme::CANVAS_BORDER),
        );
        painter.vline(
            left.right(),
            rect.y_range(),
            Stroke::new(1.0, theme::CANVAS_BORDER),
        );

        let origin = editor.artboard().position;
        let major = nice_step(self.zoom, 50.0);
        let minor = minor_step(major, self.zoom);
        let font = FontId::monospace(9.0);
        let tick = Stroke::new(1.0, theme::RULER_TICK);
        // Rango local visible en cada eje.
        let (wx0, wy0) = self.to_world(rect, rect.min);
        let (wx1, wy1) = self.to_world(rect, rect.max);
        let span = |w0: f32, w1: f32, o: i32| {
            let a = ((w0 - o as f32) / minor as f32).floor() as i32 - 1;
            let b = ((w1 - o as f32) / minor as f32).ceil() as i32 + 1;
            a..=b
        };
        for k in span(wx0, wx1, origin.0) {
            let u = k * minor;
            let x = self.to_screen(rect, (origin.0 + u, 0)).x;
            if x < top.left() || x > top.right() {
                continue;
            }
            let is_major = u.rem_euclid(major) == 0;
            let h = if is_major { RULER_PX } else { RULER_PX * 0.35 };
            painter.vline(x, (top.bottom() - h)..=top.bottom(), tick);
            if is_major {
                painter.text(
                    Pos2::new(x + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    u.to_string(),
                    font.clone(),
                    theme::LABEL,
                );
            }
        }
        for k in span(wy0, wy1, origin.1) {
            let u = k * minor;
            let y = self.to_screen(rect, (0, origin.1 + u)).y;
            if y < left.top() || y > left.bottom() {
                continue;
            }
            let is_major = u.rem_euclid(major) == 0;
            let w = if is_major { RULER_PX } else { RULER_PX * 0.35 };
            painter.hline((left.right() - w)..=left.right(), y, tick);
            if is_major {
                painter.text(
                    Pos2::new(left.left() + 1.0, y + 2.0),
                    Align2::LEFT_TOP,
                    u.to_string(),
                    font.clone(),
                    theme::LABEL,
                );
            }
        }
        // Posición del cursor.
        if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
            let mark = Stroke::new(1.0, theme::ACCENT);
            painter.vline(p.x, top.y_range(), mark);
            painter.hline(left.x_range(), p.y, mark);
        }
    }

    /// Las reglas con Pixel UI, como en el mockup (ver `pixel_rulers`): la misma escala que las
    /// de siempre, con marcas grandes cada `MAJOR_MIN_PX` como mínimo. Cada marca va en la
    /// columna (o fila) de píxeles justo antes de su borde: el 0 del mockup cae sobre la línea
    /// oscura y el lienzo empieza al lado.
    pub(super) fn draw_pixel_rulers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        pointer: Option<Pos2>,
        look: &PixelRulers,
    ) {
        painter.extend(look.frame_shapes(rect));
        let origin = editor.artboard().position;
        let major = nice_step(self.zoom, pixel_rulers::MAJOR_MIN_PX);
        let minor = minor_step(major, self.zoom);
        let (wx0, wy0) = self.to_world(rect, rect.min);
        let (wx1, wy1) = self.to_world(rect, rect.max);
        // Cada regla recorta lo suyo: las marcas no pisan la esquina ni el lienzo.
        let clip = |strip: Rect| painter.with_clip_rect(strip.intersect(painter.clip_rect()));
        let top = clip(PixelRulers::top_strip(rect));
        let screen_x = |x: i32| self.to_screen(rect, (x, 0)).x.round() - 1.0;
        for (u, x, is_major) in ruler_ticks(wx0, wx1, origin.0, major, minor, screen_x) {
            top.add(look.top_tick(x, rect.top(), is_major));
            if is_major {
                top.extend(look.top_label(u, x, rect.top()));
            }
        }
        let left = clip(PixelRulers::left_strip(rect));
        let screen_y = |y: i32| self.to_screen(rect, (0, y)).y.round() - 1.0;
        for (u, y, is_major) in ruler_ticks(wy0, wy1, origin.1, major, minor, screen_y) {
            left.add(look.left_tick(y, rect.left(), is_major));
            if is_major {
                left.extend(look.left_label(u, y, rect.left()));
            }
        }
        if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
            painter.add(look.pointer_shapes(rect, p));
        }
    }
}
