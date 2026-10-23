//! Piezas de la UI en pixel art dibujadas con el skin: diálogos y botones. Si el skin no trae una
//! pieza, se dibuja un respaldo plano con sus colores (fondo `panel`, borde `outline` de 1 px, sin
//! redondeo).

use eframe::egui::{
    self, Align2, Color32, Id, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle, Ui,
    Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::skin::{IRect, Piece, Skin, WidgetState, nine_slice};

/// Marco de los diálogos: la ventana de barra rosa del mockup de "Unsaved changes".
const DIALOG_PIECE: &str = "window";
const BUTTON_PIECE: &str = "button";
/// Margen entre el área del centro del 9-slice y lo de adentro (contenido o texto).
const PADDING: f32 = 4.0;
/// Bordes del diálogo de respaldo (izquierda, arriba, derecha, abajo); arriba va la barra de
/// título.
const FALLBACK_DIALOG: [f32; 4] = [1.0, 13.0, 1.0, 1.0];
/// Bordes y tamaño mínimo del botón de respaldo (el alto del botón del mockup).
const FALLBACK_BUTTON: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const FALLBACK_BUTTON_SIZE: Vec2 = Vec2::new(32.0, 18.0);

/// Bordes del 9-slice de la pieza en puntos (izquierda, arriba, derecha, abajo). Con la escala
/// entera de Pixel UI, un punto es un píxel del skin.
fn borders(piece: &Piece) -> [f32; 4] {
    let [w, h] = piece.size;
    match piece.center {
        Some(c) => [
            c.x as f32,
            c.y as f32,
            (w - c.x - c.w) as f32,
            (h - c.y - c.h) as f32,
        ],
        None => [0.0; 4],
    }
}

/// Formas que dibujan `piece` estirada sobre `rect` (en 9-slice si tiene centro), alineadas a
/// píxeles de pantalla.
fn piece_shapes(piece: &Piece, rect: Rect, pixels_per_point: f32) -> Vec<Shape> {
    let ppp = pixels_per_point;
    let min = (rect.min.to_vec2() * ppp).round();
    let max = (rect.max.to_vec2() * ppp).round();
    let dest = IRect::new(
        min.x as i32,
        min.y as i32,
        (max.x - min.x) as i32,
        (max.y - min.y) as i32,
    );
    let [w, h] = piece.size;
    let parts: Vec<(IRect, IRect)> = match piece.center {
        Some(center) => nine_slice(dest, piece.size, center, ppp.round().max(1.0) as u32).to_vec(),
        None => vec![(IRect::new(0, 0, w as i32, h as i32), dest)],
    };
    let (w, h) = (w as f32, h as f32);
    parts
        .into_iter()
        .filter(|(src, dst)| !src.is_empty() && !dst.is_empty())
        .map(|(src, dst)| {
            let screen = Rect::from_min_size(
                pos2(dst.x as f32 / ppp, dst.y as f32 / ppp),
                vec2(dst.w as f32 / ppp, dst.h as f32 / ppp),
            );
            let uv = Rect::from_min_size(
                pos2(src.x as f32 / w, src.y as f32 / h),
                vec2(src.w as f32 / w, src.h as f32 / h),
            );
            Shape::image(piece.texture.id(), screen, uv, Color32::WHITE)
        })
        .collect()
}

/// Respaldo plano: fondo `panel`, borde `outline` de 1 px y, si `title_bar > 0`, la barra de
/// título en `accent`. Alineado a píxeles de pantalla, como las piezas.
fn flat_shapes(skin: &Skin, rect: Rect, title_bar: f32, pixels_per_point: f32) -> Vec<Shape> {
    let colors = skin.colors();
    let rect = rect.round_to_pixels(pixels_per_point);
    let mut shapes = vec![Shape::rect_filled(rect, 0.0, colors.panel)];
    if title_bar > 0.0 {
        let bar = Rect::from_min_size(rect.min, vec2(rect.width(), title_bar))
            .round_to_pixels(pixels_per_point);
        shapes.push(Shape::rect_filled(bar, 0.0, colors.accent));
    }
    shapes.push(Shape::rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, colors.outline),
        StrokeKind::Inside,
    ));
    shapes
}

/// Diálogo fijo en el centro de la pantalla, con el marco del skin en 9-slice. El título va
/// centrado en la barra de arriba y el contenido en el área del centro, con 4 px de margen. El
/// texto de adentro usa `color.text`.
pub fn pixel_dialog<R>(
    ctx: &egui::Context,
    skin: &Skin,
    id: Id,
    title: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let piece = skin.piece(DIALOG_PIECE, WidgetState::Normal);
    let [left, top, right, bottom] = piece.map_or(FALLBACK_DIALOG, borders);
    let side = |v: f32| (v + PADDING).min(i8::MAX as f32) as i8;
    let margin = egui::Margin {
        left: side(left),
        right: side(right),
        top: side(top),
        bottom: side(bottom),
    };
    let text = skin.colors().text;
    egui::Area::new(id)
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // El marco se pinta detrás, cuando ya se sabe el tamaño del contenido.
            let background = ui.painter().add(Shape::Noop);
            let font = TextStyle::Body.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(title.to_owned(), font, text);
            let inner = egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
                ui.visuals_mut().override_text_color = Some(text);
                ui.set_min_width(galley.size().x);
                add_contents(ui)
            });
            let rect = inner.response.rect;
            let ppp = ui.ctx().pixels_per_point();
            let mut shapes = match piece {
                Some(p) => piece_shapes(p, rect, ppp),
                None => flat_shapes(skin, rect, top, ppp),
            };
            let bar = Rect::from_min_size(rect.min, vec2(rect.width(), top));
            let at = (bar.center() - galley.size() / 2.0).round_to_pixels(ppp);
            shapes.push(Shape::galley(at, galley, text));
            ui.painter().set(background, Shape::Vec(shapes));
            inner.inner
        })
        .inner
}

/// Botón con `button` en reposo, `button.hover` con el puntero encima y `button.down` mientras
/// se presiona.
///
/// El ancho se ajusta al texto, con 4 px de margen a cada lado del área del centro, sin bajar
/// del tamaño de la pieza. Devuelve un `Response` normal: clic, foco y teclado (Enter o Espacio
/// con foco).
pub fn pixel_button(ui: &mut Ui, skin: &Skin, label: &str) -> Response {
    let colors = *skin.colors();
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, colors.text);
    let base = skin.piece(BUTTON_PIECE, WidgetState::Normal);
    let [left, top, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
    let min = base.map_or(FALLBACK_BUTTON_SIZE, |p| {
        vec2(p.size[0] as f32, p.size[1] as f32)
    });
    let width = (left + PADDING + galley.size().x + PADDING + right)
        .ceil()
        .max(min.x);
    let (rect, response) = ui.allocate_exact_size(vec2(width, min.y), Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if ui.is_rect_visible(rect) {
        let state = if response.is_pointer_button_down_on() {
            WidgetState::Down
        } else if response.hovered() {
            WidgetState::Hover
        } else {
            WidgetState::Normal
        };
        let painter = ui.painter();
        let ppp = ui.ctx().pixels_per_point();
        match skin.piece(BUTTON_PIECE, state) {
            Some(piece) => painter.extend(piece_shapes(piece, rect, ppp)),
            None => painter.extend(flat_shapes(skin, rect, 0.0, ppp)),
        }
        let inner = Rect::from_min_max(rect.min + vec2(left, top), rect.max - vec2(right, bottom));
        let at = (inner.center() - galley.size() / 2.0).round_to_pixels(ppp);
        painter.galley(at, galley, colors.text);
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(1.0).round_to_pixels(ppp),
                0.0,
                Stroke::new(1.0, colors.accent),
                StrokeKind::Outside,
            );
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use egui::{Event, PointerButton, Pos2, RawInput};

    use super::*;

    fn screen() -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        }
    }

    /// Un frame; las texturas (fuentes y skin) no se suben a ninguna GPU.
    fn frame(ctx: &egui::Context, input: RawInput, run_ui: impl FnMut(&mut Ui)) {
        ctx.run_ui(input, run_ui).textures_delta.clear();
    }

    fn press(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    /// Un frame con un botón en un panel central: su rectángulo y si se hizo clic.
    fn button(ctx: &egui::Context, skin: &Skin, label: &str, events: Vec<Event>) -> (Rect, bool) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        frame(ctx, input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = pixel_button(ui, skin, label);
                out = (r.rect, r.clicked());
            });
        });
        out
    }

    #[test]
    fn a_button_takes_the_piece_size_and_grows_with_the_text() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let (short, _) = button(&ctx, &skin, "OK", vec![]);
        assert_eq!(short.size(), vec2(49.0, 18.0), "the piece size");
        let (long, _) = button(&ctx, &skin, "A much longer button label", vec![]);
        assert!(long.width() > 49.0, "{long:?}");
        assert_eq!(long.height(), 18.0);
    }

    #[test]
    fn clicking_works_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            let (rect, _) = button(&ctx, skin, "Cancel", vec![]);
            let at = rect.center();
            button(&ctx, skin, "Cancel", vec![Event::PointerMoved(at)]);
            button(&ctx, skin, "Cancel", vec![press(at, true)]);
            let (_, clicked) = button(&ctx, skin, "Cancel", vec![press(at, false)]);
            assert!(clicked);
        }
    }

    #[test]
    fn the_dialog_is_centered_with_its_content_inside_the_frame() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = Id::new("test_dialog");
        let mut content = Rect::NOTHING;
        // El área se mide en el primer frame.
        for _ in 0..3 {
            frame(&ctx, screen(), |ui| {
                pixel_dialog(ui.ctx(), &skin, id, "Unsaved changes", |ui| {
                    content = ui.max_rect();
                    ui.label("Save changes before closing?");
                });
            });
        }
        let frame = ctx
            .memory(|m| m.area_rect(id))
            .expect("the dialog is shown");
        // `window`: bordes de 3 px y barra de título de 13, más 4 px de margen.
        assert_eq!(content.min - frame.min, vec2(7.0, 17.0));
        assert!(
            frame.center().distance(Pos2::new(400.0, 300.0)) <= 1.0,
            "{frame:?}"
        );
    }

    /// Que `v` caiga en un píxel físico entero a `ppp`.
    fn on_pixels(v: f32, ppp: f32) -> bool {
        ((v * ppp) - (v * ppp).round()).abs() < 1e-3
    }

    fn assert_on_pixels(shapes: &[Shape], ppp: f32) {
        for shape in shapes {
            let r = shape.visual_bounding_rect();
            for v in [r.min.x, r.min.y, r.max.x, r.max.y] {
                assert!(on_pixels(v, ppp), "{r:?} at {ppp}");
            }
        }
    }

    #[test]
    fn pieces_and_the_flat_fallback_land_on_whole_physical_pixels() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let window = skin.piece(DIALOG_PIECE, WidgetState::Normal).unwrap();
        let rect = Rect::from_min_size(pos2(10.3, 20.7), vec2(100.4, 60.2));
        for ppp in [1.0, 2.0, 3.0] {
            let shapes = piece_shapes(window, rect, ppp);
            assert_eq!(shapes.len(), 9);
            assert_on_pixels(&shapes, ppp);
            let flat: Vec<Shape> = flat_shapes(&skin, rect, 13.0, ppp)
                .into_iter()
                .filter(|s| matches!(s, Shape::Rect(r) if r.fill != Color32::TRANSPARENT))
                .collect();
            assert_eq!(flat.len(), 2, "background and title bar");
            assert_on_pixels(&flat, ppp);
        }
    }

    #[test]
    fn parts_with_an_empty_source_are_not_drawn() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let window = skin.piece(DIALOG_PIECE, WidgetState::Normal).unwrap();
        // Centro de 0×0: el centro y los cuatro bordes no tienen de dónde copiar.
        let piece = Piece {
            texture: window.texture.clone(),
            size: window.size,
            center: Some(crate::skin::Rect9::new(3, 13, 0, 0, window.size).unwrap()),
        };
        let rect = Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0));
        assert_eq!(piece_shapes(&piece, rect, 1.0).len(), 4, "only the corners");
    }
}
