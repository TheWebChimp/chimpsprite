//! Piezas de la UI en pixel art dibujadas con el skin: diálogos y botones. Si el skin no trae una
//! pieza, se dibuja un respaldo plano con sus colores (fondo `panel`, borde `outline` de 1 px, sin
//! redondeo).

use eframe::egui::{
    self, Align2, Color32, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, TextStyle,
    Ui, Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::skin::{IRect, Piece, Skin, WidgetState, nine_slice};

/// Marco de los diálogos: la ventana de barra rosa del mockup de "Unsaved changes".
const DIALOG_PIECE: &str = "window";
const BUTTON_PIECE: &str = "button";
/// Botón de cerrar de la barra de título.
const CLOSE_PIECE: &str = "window.close";
/// Margen entre el área del centro del 9-slice y lo de adentro (contenido o texto).
const PADDING: f32 = 4.0;
/// Espacio entre botones de una fila.
const BUTTON_GAP: f32 = 4.0;
/// Bordes del diálogo de respaldo (izquierda, arriba, derecha, abajo); arriba va la barra de
/// título.
const FALLBACK_DIALOG: [f32; 4] = [1.0, 13.0, 1.0, 1.0];
/// Bordes y tamaño mínimo del botón de respaldo (el alto del botón del mockup).
const FALLBACK_BUTTON: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const FALLBACK_BUTTON_SIZE: Vec2 = Vec2::new(32.0, 18.0);
/// Tamaño del botón de cerrar de respaldo (el de la pieza del skin).
const FALLBACK_CLOSE_SIZE: Vec2 = Vec2::new(9.0, 9.0);
/// Marco de los botones de herramienta; `.active` cuando su grupo tiene la herramienta activa.
const SLOT_PIECE: &str = "tool.slot";
/// Tamaño del botón de herramienta de respaldo (el del marco del mockup).
const FALLBACK_SLOT_SIZE: Vec2 = Vec2::new(23.0, 23.0);
/// Lado de la marca de variantes, en píxeles del skin.
const MORE_MARK: f32 = 3.0;
/// Margen de la barra de íconos a cada lado del marco, en puntos.
pub const TOOLBAR_MARGIN: i8 = 4;
/// Marco de los botones de prender y apagar de la barra de opciones (los ejes de simetría);
/// `.active` cuando están prendidos.
const TOGGLE_PIECE: &str = "sym.slot";
/// Tamaño del botón de prender y apagar de respaldo (el de la pieza del mockup).
const FALLBACK_TOGGLE_SIZE: Vec2 = Vec2::new(13.0, 14.0);
/// Esquina del ícono dentro de `sym.slot`: el interior del cuadro de 11×11 empieza en (2,2).
const TOGGLE_ICON_OFFSET: Vec2 = Vec2::new(2.0, 2.0);
/// Lado del interior de `sym.slot`, donde va el ícono.
const TOGGLE_ICON_SIZE: Vec2 = Vec2::new(9.0, 9.0);

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
///
/// Con `closable`, a la derecha de la barra va el botón de cerrar (`window.close`, o
/// `window.close.hover` con el puntero encima; si el skin no lo trae, una × de líneas). Si se
/// hace clic en él, [`DialogOutput::closed`] es `true`; qué hacer con eso lo decide quien llama.
pub fn pixel_dialog<R>(
    ctx: &egui::Context,
    skin: &Skin,
    id: Id,
    title: &str,
    closable: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> DialogOutput<R> {
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
            let closed = closable && close_button(ui, skin, id.with("close"), bar);
            DialogOutput {
                inner: inner.inner,
                closed,
            }
        })
        .inner
}

/// Lo que devuelve [`pixel_dialog`]: lo que devolvió el contenido y si se hizo clic en el botón
/// de cerrar de la barra de título.
pub struct DialogOutput<R> {
    pub inner: R,
    pub closed: bool,
}

/// Botón de cerrar a la derecha de la barra de título `bar`, centrado en vertical y con el mismo
/// margen a la derecha que arriba. Devuelve si se hizo clic.
fn close_button(ui: &mut Ui, skin: &Skin, id: Id, bar: Rect) -> bool {
    let size = skin
        .piece(CLOSE_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_CLOSE_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        });
    let inset = ((bar.height() - size.y) / 2.0).floor().max(0.0);
    let ppp = ui.ctx().pixels_per_point();
    let rect = Rect::from_min_size(pos2(bar.max.x - inset - size.x, bar.min.y + inset), size)
        .round_to_pixels(ppp);
    let response = ui.interact(rect, id, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "Close"));
    let state = if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let shapes = match skin.piece(CLOSE_PIECE, state) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => close_cross(rect, skin.colors().text, ppp),
    };
    ui.painter().extend(shapes);
    response.clicked()
}

/// Respaldo del botón de cerrar: una × de dos líneas de 1 px, sin glifo.
fn close_cross(rect: Rect, color: Color32, pixels_per_point: f32) -> Vec<Shape> {
    let r = rect.shrink(2.0).round_to_pixels(pixels_per_point);
    let stroke = Stroke::new(1.0, color);
    vec![
        Shape::line_segment([r.left_top(), r.right_bottom()], stroke),
        Shape::line_segment([r.right_top(), r.left_bottom()], stroke),
    ]
}

/// Tamaño del botón `label` ajustado al texto: 4 px de margen a cada lado del área del centro,
/// sin bajar del tamaño de la pieza.
fn button_size(ui: &Ui, skin: &Skin, label: &str) -> Vec2 {
    let font = TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, skin.colors().text)
        .size();
    let base = skin.piece(BUTTON_PIECE, WidgetState::Normal);
    let [left, _, right, _] = base.map_or(FALLBACK_BUTTON, borders);
    let min = base.map_or(FALLBACK_BUTTON_SIZE, |p| {
        vec2(p.size[0] as f32, p.size[1] as f32)
    });
    vec2(
        (left + PADDING + text.x + PADDING + right)
            .ceil()
            .max(min.x),
        min.y,
    )
}

/// Botón con `button` en reposo, `button.hover` con el puntero encima y `button.down` mientras
/// se presiona. Presionado, el texto baja 1 px.
///
/// El ancho se ajusta al texto, con 4 px de margen a cada lado del área del centro, sin bajar
/// del tamaño de la pieza. Devuelve un `Response` normal: clic, foco y teclado (Enter o Espacio
/// con foco).
// Para botones sueltos; por ahora los diálogos usan filas de `pixel_buttons`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn pixel_button(ui: &mut Ui, skin: &Skin, label: &str) -> Response {
    let size = button_size(ui, skin, label);
    button_of_size(ui, skin, label, size)
}

/// Una fila de botones que ocupa todo el ancho disponible: el ancho se reparte en partes
/// iguales, con 4 px entre botón y botón, y los píxeles que sobran van a los últimos (uno a cada
/// uno, así los anchos son enteros y no difieren en más de 1 px). Ningún botón
/// queda más angosto que su texto: si no entran, la fila se ensancha. Devuelve las respuestas en
/// el orden de `labels`.
///
/// En la pasada de medición de un `Area` la fila mide lo justo, para no ensanchar el diálogo.
pub fn pixel_buttons(ui: &mut Ui, skin: &Skin, labels: &[&str]) -> Vec<Response> {
    let n = labels.len();
    if n == 0 {
        return Vec::new();
    }
    let sizes: Vec<Vec2> = labels.iter().map(|l| button_size(ui, skin, l)).collect();
    let widest = sizes.iter().map(|s| s.x).fold(0.0, f32::max);
    let gaps = BUTTON_GAP * (n - 1) as f32;
    let needed = widest * n as f32 + gaps;
    let total = if ui.is_sizing_pass() {
        needed
    } else {
        ui.available_width().floor().max(needed)
    };
    let each = ((total - gaps) / n as f32).floor();
    // Los píxeles que sobran (menos que `n`), uno por botón empezando por el último.
    let extra = (total - gaps - each * n as f32) as usize;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = BUTTON_GAP;
        labels
            .iter()
            .zip(&sizes)
            .enumerate()
            .map(|(i, (label, size))| {
                let width = if i + extra >= n { each + 1.0 } else { each };
                button_of_size(ui, skin, label, vec2(width, size.y))
            })
            .collect()
    })
    .inner
}

/// Dónde va el texto de un botón: centrado en `inner` y, presionado, 1 px más abajo. Alineado a
/// píxeles de pantalla.
fn label_pos(inner: Rect, size: Vec2, state: WidgetState, pixels_per_point: f32) -> Pos2 {
    let down = if state == WidgetState::Down { 1.0 } else { 0.0 };
    (inner.center() - size / 2.0 + vec2(0.0, down)).round_to_pixels(pixels_per_point)
}

/// Un botón de `size` exacto, dibujado con la pieza del estado en que está.
fn button_of_size(ui: &mut Ui, skin: &Skin, label: &str, size: Vec2) -> Response {
    let colors = *skin.colors();
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, colors.text);
    let base = skin.piece(BUTTON_PIECE, WidgetState::Normal);
    let [left, top, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
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
        let at = label_pos(inner, galley.size(), state, ppp);
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

/// Tamaño del marco de los botones de herramienta: el de la pieza `tool.slot` o el de respaldo.
fn slot_size(skin: &Skin) -> Vec2 {
    skin.piece(SLOT_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_SLOT_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        })
}

/// Ancho de la barra de íconos sin la línea que la separa del lienzo: el marco `tool.slot` más
/// `TOOLBAR_MARGIN` a cada lado. Sale de la pieza, así sigue al skin.
pub fn toolbar_width(skin: &Skin) -> f32 {
    slot_size(skin).x + 2.0 * f32::from(TOOLBAR_MARGIN)
}

/// Botón de herramienta de la barra de íconos: el marco `tool.slot` (`tool.slot.active` con
/// `selected`) a tamaño de pieza y el ícono `icon` centrado en él. Presionado, el ícono baja 1 px.
/// Con `more`, un triangulito de 3×3 en la esquina de abajo a la derecha del relleno marca que
/// hay variantes.
///
/// Sin el ícono se dibuja la inicial de `label`; sin el marco, el respaldo plano (con un borde
/// `accent` si está elegido). `label` es también el nombre accesible.
///
/// `id` es el del botón (y de su lista de variantes): fijo por grupo, no depende del orden de los
/// widgets de la barra.
pub fn tool_button(
    ui: &mut Ui,
    id: egui::Id,
    skin: &Skin,
    icon: &str,
    label: &str,
    selected: bool,
    more: bool,
) -> Response {
    let base = skin.piece(SLOT_PIECE, WidgetState::Normal);
    let size = slot_size(skin);
    let (_, space) = ui.allocate_space(size);
    let response = ui.interact(space, id, Sense::click());
    let rect = egui::Align2::CENTER_CENTER.align_size_within_rect(size, space);
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, selected, label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let pressed = response.is_pointer_button_down_on();
    let state = if selected {
        WidgetState::Active
    } else if pressed {
        WidgetState::Down
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = match skin.piece(SLOT_PIECE, state) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => {
            let mut flat = flat_shapes(skin, rect, 0.0, ppp);
            if selected {
                flat.push(Shape::rect_stroke(
                    rect.shrink(1.0).round_to_pixels(ppp),
                    0.0,
                    Stroke::new(1.0, colors.accent),
                    StrokeKind::Inside,
                ));
            }
            flat
        }
    };
    let icon_state = if pressed {
        WidgetState::Down
    } else {
        WidgetState::Normal
    };
    match skin.piece(icon, WidgetState::Normal) {
        Some(piece) => {
            let icon_size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = label_pos(rect, icon_size, icon_state, ppp);
            shapes.extend(piece_shapes(piece, Rect::from_min_size(at, icon_size), ppp));
        }
        None => {
            let initial: String = label.chars().take(1).collect();
            let font = TextStyle::Button.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(initial, font, colors.text);
            let at = label_pos(rect, galley.size(), icon_state, ppp);
            shapes.push(Shape::galley(at, galley, colors.text));
        }
    }
    if more {
        let [_, _, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
        let corner = rect.max - vec2(right, bottom);
        shapes.extend(
            more_mark(corner, ppp)
                .into_iter()
                .map(|r| Shape::rect_filled(r, 0.0, colors.text_dim)),
        );
    }
    ui.painter().extend(shapes);
    response
}

/// Botón de prender y apagar de la barra de opciones: el marco `sym.slot` (`sym.slot.active`
/// con `on`) a tamaño de pieza y el ícono `icon` en su interior.
///
/// - Sin el marco: el respaldo plano, con un borde `accent` si está prendido.
/// - Sin el ícono: lo que dibuje `fallback` en el interior (rect y color del texto).
/// - `label` es el nombre accesible.
pub fn toggle_button(
    ui: &mut Ui,
    skin: &Skin,
    icon: &str,
    label: &str,
    on: bool,
    fallback: impl FnOnce(Rect, Color32) -> Vec<Shape>,
) -> Response {
    let size = skin
        .piece(TOGGLE_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_TOGGLE_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        });
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, label)
    });
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let state = if on {
        WidgetState::Active
    } else {
        WidgetState::Normal
    };
    let mut shapes = match skin.piece(TOGGLE_PIECE, state) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => {
            let mut flat = flat_shapes(skin, rect, 0.0, ppp);
            if on {
                flat.push(Shape::rect_stroke(
                    rect.shrink(1.0).round_to_pixels(ppp),
                    0.0,
                    Stroke::new(1.0, colors.accent),
                    StrokeKind::Inside,
                ));
            }
            flat
        }
    };
    let inner =
        Rect::from_min_size(rect.min + TOGGLE_ICON_OFFSET, TOGGLE_ICON_SIZE).round_to_pixels(ppp);
    match skin.piece(icon, WidgetState::Normal) {
        Some(piece) => {
            let icon_size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = Rect::from_min_size(inner.min, icon_size);
            shapes.extend(piece_shapes(piece, at, ppp));
        }
        None => shapes.extend(fallback(inner, colors.text)),
    }
    ui.painter().extend(shapes);
    response
}

/// Triangulito de 3×3 px que marca un grupo con variantes, con el ángulo recto abajo a la
/// derecha, en `corner`: tres filas de 1, 2 y 3 px, alineadas a píxeles de pantalla.
fn more_mark(corner: Pos2, pixels_per_point: f32) -> [Rect; 3] {
    let top = corner.y - MORE_MARK;
    [1.0, 2.0, 3.0].map(|w| {
        let y = top + w - 1.0;
        Rect::from_min_max(pos2(corner.x - w, y), pos2(corner.x, y + 1.0))
            .round_to_pixels(pixels_per_point)
    })
}

#[cfg(test)]
mod tests {
    use egui::{Event, PointerButton, RawInput};

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
        assert_eq!(short.size(), vec2(49.0, 17.0), "the piece size");
        let (long, _) = button(&ctx, &skin, "A much longer button label", vec![]);
        assert!(long.width() > 49.0, "{long:?}");
        assert_eq!(long.height(), 17.0);
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
                pixel_dialog(ui.ctx(), &skin, id, "Unsaved changes", false, |ui| {
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

    #[test]
    fn a_row_of_buttons_fills_the_content_width_evenly() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            let id = Id::new((
                "row",
                skin.piece(BUTTON_PIECE, WidgetState::Normal).is_some(),
            ));
            let mut content = Rect::NOTHING;
            let mut rects = Vec::new();
            for _ in 0..3 {
                frame(&ctx, screen(), |ui| {
                    pixel_dialog(ui.ctx(), skin, id, "Unsaved changes", true, |ui| {
                        content = ui.max_rect();
                        ui.label("A long question that is wider than the buttons, surely?");
                        rects = pixel_buttons(ui, skin, &["Save", "Discard", "Cancel"])
                            .iter()
                            .map(|r| r.rect)
                            .collect();
                    });
                });
            }
            assert_eq!(rects.len(), 3);
            assert_eq!(rects[0].min.x, content.min.x, "{rects:?} in {content:?}");
            assert!(
                (rects[2].max.x - content.max.x).abs() < 1.0,
                "{rects:?} in {content:?}"
            );
            for pair in rects.windows(2) {
                assert_eq!(pair[1].min.x - pair[0].max.x, BUTTON_GAP, "{rects:?}");
            }
            let widths: Vec<f32> = rects.iter().map(|r| r.width()).collect();
            for w in &widths {
                assert_eq!(w.fract(), 0.0, "whole pixels: {widths:?}");
                assert!((w - widths[0]).abs() <= 1.0, "{widths:?}");
            }
        }
    }

    /// Un frame con un botón de herramienta (Lasso, con variantes): su rectángulo, si se hizo
    /// clic y las texturas que se dibujaron.
    fn tool(
        ctx: &egui::Context,
        skin: &Skin,
        selected: bool,
        events: Vec<Event>,
    ) -> (Rect, bool, Vec<egui::TextureId>) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = tool_button(
                    ui,
                    egui::Id::new("lasso"),
                    skin,
                    "icon.lasso",
                    "Lasso",
                    selected,
                    true,
                );
                out = (r.rect, r.clicked());
            });
        });
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
        let mut textures = Vec::new();
        for clipped in &output.shapes {
            collect_textures(&clipped.shape, &mut textures);
        }
        (out.0, out.1, textures)
    }

    fn collect_textures(shape: &Shape, out: &mut Vec<egui::TextureId>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_textures(s, out)),
            Shape::Mesh(mesh) => out.push(mesh.texture_id),
            _ => {}
        }
    }

    #[test]
    fn a_tool_button_is_the_slot_with_its_icon_and_the_active_slot_when_selected() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        let (rect, _, textures) = tool(&ctx, &skin, false, vec![]);
        assert_eq!(rect.size(), vec2(23.0, 23.0), "the slot size");
        assert!(textures.contains(&id("tool.slot")));
        assert!(textures.contains(&id("icon.lasso")));
        let (_, _, textures) = tool(&ctx, &skin, true, vec![]);
        assert!(textures.contains(&id("tool.slot.active")));
        assert!(!textures.contains(&id("tool.slot")));
    }

    #[test]
    fn a_tool_button_clicks_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            let (rect, _, _) = tool(&ctx, skin, false, vec![]);
            assert_eq!(rect.size(), vec2(23.0, 23.0));
            let at = rect.center();
            tool(&ctx, skin, false, vec![Event::PointerMoved(at)]);
            tool(&ctx, skin, false, vec![press(at, true)]);
            let (_, clicked, _) = tool(&ctx, skin, false, vec![press(at, false)]);
            assert!(clicked);
        }
    }

    /// Un frame con un botón de prender y apagar: su rect, si se hizo clic y las texturas
    /// dibujadas.
    fn toggle(
        ctx: &egui::Context,
        skin: &Skin,
        icon: &str,
        on: bool,
        events: Vec<Event>,
    ) -> (Rect, bool, Vec<egui::TextureId>) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let r = toggle_button(ui, skin, icon, "Vertical axis", on, |r, color| {
                    vec![Shape::rect_filled(r, 0.0, color)]
                });
                out = (r.rect, r.clicked());
            });
        });
        output.textures_delta.clear();
        let mut textures = Vec::new();
        for clipped in &output.shapes {
            collect_textures(&clipped.shape, &mut textures);
        }
        (out.0, out.1, textures)
    }

    #[test]
    fn a_toggle_is_the_symmetry_slot_with_its_icon_and_the_active_slot_when_on() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        let (rect, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", false, vec![]);
        assert_eq!(rect.size(), vec2(13.0, 14.0), "the piece size");
        assert!(textures.contains(&id("sym.slot")));
        assert!(textures.contains(&id("icon.sym_vertical")));
        let (_, _, textures) = toggle(&ctx, &skin, "icon.sym_vertical", true, vec![]);
        assert!(textures.contains(&id("sym.slot.active")));
        assert!(!textures.contains(&id("sym.slot")));
    }

    #[test]
    fn a_toggle_clicks_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        for skin in &skins {
            // Sin el ícono (`icon.nope`), dibuja el respaldo.
            let (rect, _, _) = toggle(&ctx, skin, "icon.nope", false, vec![]);
            assert_eq!(rect.size(), vec2(13.0, 14.0));
            let at = rect.center();
            toggle(
                &ctx,
                skin,
                "icon.nope",
                false,
                vec![Event::PointerMoved(at)],
            );
            toggle(&ctx, skin, "icon.nope", false, vec![press(at, true)]);
            let (_, clicked, _) = toggle(&ctx, skin, "icon.nope", false, vec![press(at, false)]);
            assert!(clicked);
        }
    }

    #[test]
    fn the_toolbar_is_as_wide_as_the_slot_plus_the_margin() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let slot = skin.piece(SLOT_PIECE, WidgetState::Normal).unwrap().size[0] as f32;
        assert_eq!(toolbar_width(&skin), slot + 8.0);
        assert_eq!(
            toolbar_width(&Skin::fallback()),
            23.0 + 8.0,
            "the fallback slot"
        );
    }

    #[test]
    fn the_variants_mark_is_a_three_pixel_triangle_in_the_corner() {
        for ppp in [1.0, 2.0, 3.0] {
            let rows = more_mark(pos2(21.0, 21.0), ppp);
            assert_eq!(
                rows,
                [
                    Rect::from_min_max(pos2(20.0, 18.0), pos2(21.0, 19.0)),
                    Rect::from_min_max(pos2(19.0, 19.0), pos2(21.0, 20.0)),
                    Rect::from_min_max(pos2(18.0, 20.0), pos2(21.0, 21.0)),
                ],
                "at {ppp}"
            );
        }
    }

    #[test]
    fn a_pressed_label_goes_one_point_down() {
        let inner = Rect::from_min_size(pos2(10.0, 20.0), vec2(43.0, 10.0));
        let size = vec2(20.3, 7.0);
        for ppp in [1.0, 2.0, 3.0] {
            let normal = label_pos(inner, size, WidgetState::Normal, ppp);
            let hover = label_pos(inner, size, WidgetState::Hover, ppp);
            let down = label_pos(inner, size, WidgetState::Down, ppp);
            assert_eq!(hover, normal);
            assert_eq!(down - normal, vec2(0.0, 1.0), "at {ppp}");
        }
    }
}
