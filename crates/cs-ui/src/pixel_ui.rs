//! Piezas de la UI en pixel art dibujadas con el skin: diálogos y botones. Si el skin no trae una
//! pieza, se dibuja un respaldo plano con sus colores (fondo `panel`, borde `outline` de 1 px, sin
//! redondeo).

use eframe::egui::{
    self, Align2, Color32, Id, Mesh, Pos2, Rangef, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::skin::{IRect, Piece, Skin, SkinColors, WidgetState, nine_slice};

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
/// Marcos de los botones de herramienta, los tramos de la columna del mockup: el primero
/// (redondeado arriba), los del medio y el último (redondeado abajo). `.active` cuando su grupo
/// tiene la herramienta activa.
const SLOT_PIECE: &str = "tool.slot";
const FIRST_SLOT_PIECE: &str = "tool.slot.first";
const LAST_SLOT_PIECE: &str = "tool.slot.last";
/// Tamaño del botón de herramienta de respaldo (el del marco del mockup).
const FALLBACK_SLOT_SIZE: Vec2 = Vec2::new(23.0, 23.0);
/// Caja redondeada alrededor de la columna de botones, en 9-slice: se estira en vertical.
const BOX_PIECE: &str = "toolbar.box";
/// Línea y aire de la caja de respaldo a cada lado de los botones: 1 px de cada uno.
const FALLBACK_BOX_INSET: f32 = 2.0;
/// Lado de la marca de variantes, en píxeles del skin.
const MORE_MARK: f32 = 3.0;
/// Margen de la barra de íconos a cada lado de la caja, en puntos: 5 px de fondo en el mockup.
pub const TOOLBAR_MARGIN: i8 = 5;
/// Margen de arriba de la barra de íconos: 5 px de fondo bajo la barra de opciones, igual que a
/// los lados.
pub const TOOLBAR_TOP_MARGIN: i8 = 5;
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
pub(crate) fn piece_shapes(piece: &Piece, rect: Rect, pixels_per_point: f32) -> Vec<Shape> {
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

/// Qué tramo de la columna es un botón: los botones van pegados y solo las puntas son
/// redondeadas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotPos {
    /// El de arriba: `tool.slot.first`.
    First,
    /// Los del medio: `tool.slot`.
    Middle,
    /// El de abajo, con el borde de cierre: `tool.slot.last`.
    Last,
}

impl SlotPos {
    /// El tramo del botón `index` en una columna de `len` botones.
    pub fn of(index: usize, len: usize) -> SlotPos {
        if index == 0 {
            SlotPos::First
        } else if index + 1 >= len {
            SlotPos::Last
        } else {
            SlotPos::Middle
        }
    }

    fn piece_name(self) -> &'static str {
        match self {
            SlotPos::First => FIRST_SLOT_PIECE,
            SlotPos::Middle => SLOT_PIECE,
            SlotPos::Last => LAST_SLOT_PIECE,
        }
    }
}

/// El marco del tramo `slot` en `state`; si el skin no trae ese tramo, el del medio.
fn slot_piece(skin: &Skin, slot: SlotPos, state: WidgetState) -> Option<&Piece> {
    skin.piece(slot.piece_name(), state)
        .or_else(|| skin.piece(SLOT_PIECE, state))
}

/// Tamaño del marco del tramo `slot`: el de su pieza o el de respaldo.
fn slot_size(skin: &Skin, slot: SlotPos) -> Vec2 {
    slot_piece(skin, slot, WidgetState::Normal).map_or(FALLBACK_SLOT_SIZE, |p| {
        vec2(p.size[0] as f32, p.size[1] as f32)
    })
}

/// Ancho de la caja de la barra: el de `toolbar.box` o, sin ella, el botón con la línea y el
/// aire de respaldo a cada lado.
fn box_width(skin: &Skin) -> f32 {
    skin.piece(BOX_PIECE, WidgetState::Normal).map_or(
        slot_size(skin, SlotPos::Middle).x + 2.0 * FALLBACK_BOX_INSET,
        |p| p.size[0] as f32,
    )
}

/// Distancia de la caja a los botones, a cada lado: lo que sobra del ancho de la caja, repartido
/// (1 px de línea y 1 de aire en el mockup). Arriba y abajo es la misma.
fn box_inset(skin: &Skin) -> f32 {
    ((box_width(skin) - slot_size(skin, SlotPos::Middle).x) / 2.0)
        .floor()
        .max(0.0)
}

/// Ancho de la barra de íconos sin la línea que la separa del lienzo: la caja `toolbar.box` más
/// `TOOLBAR_MARGIN` a cada lado. Sale de las piezas, así sigue al skin.
pub fn toolbar_width(skin: &Skin) -> f32 {
    box_width(skin) + 2.0 * f32::from(TOOLBAR_MARGIN)
}

/// La caja de la barra de íconos alrededor de lo que agregue `add_contents` (la columna de
/// botones), con la línea y el aire de la pieza a cada lado. La pieza se estira en vertical
/// hasta el alto de la columna; sin ella, un borde `outline` de 1 px. Devuelve el rect de la
/// caja.
pub fn toolbar_box(ui: &mut Ui, skin: &Skin, add_contents: impl FnOnce(&mut Ui)) -> Rect {
    let inset = box_inset(skin);
    // La caja se pinta detrás, cuando ya se sabe el alto de la columna.
    let background = ui.painter().add(Shape::Noop);
    let margin = egui::Margin::same(inset.min(f32::from(i8::MAX)) as i8);
    let inner = egui::Frame::NONE
        .inner_margin(margin)
        .show(ui, add_contents);
    let rect = Rect::from_min_size(
        inner.response.rect.min,
        vec2(box_width(skin), inner.response.rect.height()),
    );
    let ppp = ui.ctx().pixels_per_point();
    let shapes = match skin.piece(BOX_PIECE, WidgetState::Normal) {
        Some(piece) => piece_shapes(piece, rect, ppp),
        None => vec![Shape::rect_stroke(
            rect.round_to_pixels(ppp),
            0.0,
            Stroke::new(1.0, skin.colors().outline),
            StrokeKind::Inside,
        )],
    };
    ui.painter().set(background, Shape::Vec(shapes));
    rect
}

/// Botón de herramienta de la barra de íconos: el marco del tramo `slot` de la columna
/// (`tool.slot.first`, `tool.slot` o `tool.slot.last`; con `selected`, su `.active`) a tamaño
/// de pieza y el ícono `icon` centrado en el relleno (el centro del 9-slice). Presionado, el
/// ícono baja 1 px. Con `more`, un triangulito de 3×3 en la esquina de abajo a la derecha del
/// relleno marca que hay variantes.
///
/// Sin el marco del tramo se usa el del medio. Sin el ícono se dibuja la inicial de `label`; sin
/// ningún marco, el respaldo plano (con un borde `accent` si está elegido). `label` es también el
/// nombre accesible.
///
/// `id` es el del botón (y de su lista de variantes): fijo por grupo, no depende del orden de los
/// widgets de la barra.
#[allow(clippy::too_many_arguments)]
pub fn tool_button(
    ui: &mut Ui,
    id: egui::Id,
    skin: &Skin,
    slot: SlotPos,
    icon: &str,
    label: &str,
    selected: bool,
    more: bool,
) -> Response {
    let base = slot_piece(skin, slot, WidgetState::Normal);
    let size = slot_size(skin, slot);
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
    let mut shapes = match slot_piece(skin, slot, state) {
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
    // El relleno: el centro del 9-slice del marco (o 1 px adentro del respaldo). El ícono de 17
    // queda a 1 px de su borde, como en el mockup.
    let [left, top, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
    let fill = Rect::from_min_max(rect.min + vec2(left, top), rect.max - vec2(right, bottom));
    match skin.piece(icon, WidgetState::Normal) {
        Some(piece) => {
            let icon_size = vec2(piece.size[0] as f32, piece.size[1] as f32);
            let at = label_pos(fill, icon_size, icon_state, ppp);
            shapes.extend(piece_shapes(piece, Rect::from_min_size(at, icon_size), ppp));
        }
        None => {
            let initial: String = label.chars().take(1).collect();
            let font = TextStyle::Button.resolve(ui.style());
            let galley = ui.painter().layout_no_wrap(initial, font, colors.text);
            let at = label_pos(fill, galley.size(), icon_state, ppp);
            shapes.push(Shape::galley(at, galley, colors.text));
        }
    }
    if more {
        let corner = fill.max;
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

/// Grosor de un separador grabado, en puntos: la línea oscura y el brillo, de 1 px de skin cada
/// una.
pub const ETCHED_SIZE: f32 = 2.0;

/// Separador grabado horizontal del mockup: la línea oscura (`canvas`) en la fila `top` y el
/// brillo (`highlight`) en la de abajo, a lo largo de `x`. Alineado a píxeles físicos y como
/// malla: sin el difuminado de los trazos de egui.
pub fn etched_hline(x: Rangef, top: f32, colors: &SkinColors, pixels_per_point: f32) -> Shape {
    let dark = Rect::from_x_y_ranges(x, top..=top + 1.0);
    etched(
        dark,
        dark.translate(vec2(0.0, 1.0)),
        colors,
        pixels_per_point,
    )
}

/// Separador grabado vertical del mockup: la línea oscura (`canvas`) en la columna `left` y el
/// brillo (`highlight`) en la de la derecha, a lo largo de `y`. Como [`etched_hline`].
pub fn etched_vline(left: f32, y: Rangef, colors: &SkinColors, pixels_per_point: f32) -> Shape {
    let dark = Rect::from_x_y_ranges(left..=left + 1.0, y);
    etched(
        dark,
        dark.translate(vec2(1.0, 0.0)),
        colors,
        pixels_per_point,
    )
}

/// Las dos líneas de un separador grabado, cada una redondeada a píxeles físicos.
fn etched(dark: Rect, light: Rect, colors: &SkinColors, pixels_per_point: f32) -> Shape {
    let mut mesh = Mesh::default();
    mesh.add_colored_rect(dark.round_to_pixels(pixels_per_point), colors.canvas);
    mesh.add_colored_rect(light.round_to_pixels(pixels_per_point), colors.highlight);
    Shape::mesh(mesh)
}

/// Los rects de color liso de una forma (las mallas, de a cuatro vértices), para los tests.
#[cfg(test)]
pub(crate) fn colored_rects(shape: &Shape) -> Vec<(Color32, Rect)> {
    match shape {
        Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::default() => mesh
            .vertices
            .chunks(4)
            .map(|quad| {
                let rect = Rect::from_points(&quad.iter().map(|v| v.pos).collect::<Vec<_>>());
                (quad[0].color, rect)
            })
            .collect(),
        Shape::Vec(shapes) => shapes.iter().flat_map(colored_rects).collect(),
        _ => Vec::new(),
    }
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

    /// Un frame con un botón de herramienta (Lasso, con variantes) en el tramo `slot` de la
    /// columna: su rectángulo, si se hizo clic y las texturas que se dibujaron con su rect.
    fn tool(
        ctx: &egui::Context,
        skin: &Skin,
        slot: SlotPos,
        selected: bool,
        events: Vec<Event>,
    ) -> (Rect, bool, Vec<(egui::TextureId, Rect)>) {
        let mut out = (Rect::NOTHING, false);
        let input = RawInput { events, ..screen() };
        let textures = run(ctx, input, |ui| {
            let r = tool_button(
                ui,
                egui::Id::new("lasso"),
                skin,
                slot,
                "icon.lasso",
                "Lasso",
                selected,
                true,
            );
            out = (r.rect, r.clicked());
        });
        (out.0, out.1, textures)
    }

    /// Un frame con `add` en un panel central; devuelve las texturas dibujadas con su rect.
    fn run(
        ctx: &egui::Context,
        input: RawInput,
        mut add: impl FnMut(&mut Ui),
    ) -> Vec<(egui::TextureId, Rect)> {
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| add(ui));
        });
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
        let mut textures = Vec::new();
        for clipped in &output.shapes {
            collect_meshes(&clipped.shape, &mut textures);
        }
        textures
    }

    fn collect_meshes(shape: &Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_meshes(s, out)),
            Shape::Mesh(mesh) => out.push((mesh.texture_id, mesh.calc_bounds())),
            _ => {}
        }
    }

    fn collect_textures(shape: &Shape, out: &mut Vec<egui::TextureId>) {
        let mut meshes = Vec::new();
        collect_meshes(shape, &mut meshes);
        out.extend(meshes.into_iter().map(|(t, _)| t));
    }

    /// Rect de lo dibujado con la textura `texture` (la unión de sus partes).
    fn drawn(textures: &[(egui::TextureId, Rect)], texture: egui::TextureId) -> Option<Rect> {
        textures
            .iter()
            .filter(|(t, _)| *t == texture)
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(b))
    }

    /// Cuáles de los seis marcos de la columna se dibujaron.
    fn drawn_slots(skin: &Skin, textures: &[(egui::TextureId, Rect)]) -> Vec<&'static str> {
        [
            "tool.slot.first",
            "tool.slot",
            "tool.slot.last",
            "tool.slot.first.active",
            "tool.slot.active",
            "tool.slot.last.active",
        ]
        .into_iter()
        .filter(|name| {
            let id = skin.piece(name, WidgetState::Normal).unwrap().texture.id();
            textures.iter().any(|(t, _)| *t == id)
        })
        .collect()
    }

    #[test]
    fn a_tool_button_is_its_column_slot_with_the_icon_and_the_active_slot_when_selected() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let id = |name: &str| skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        for (slot, name, active, height) in [
            (
                SlotPos::First,
                "tool.slot.first",
                "tool.slot.first.active",
                22.0,
            ),
            (SlotPos::Middle, "tool.slot", "tool.slot.active", 22.0),
            (
                SlotPos::Last,
                "tool.slot.last",
                "tool.slot.last.active",
                23.0,
            ),
        ] {
            let (rect, _, textures) = tool(&ctx, &skin, slot, false, vec![]);
            assert_eq!(rect.size(), vec2(23.0, height), "{name}: the piece size");
            assert_eq!(drawn_slots(&skin, &textures), [name], "{slot:?}");
            assert_eq!(drawn(&textures, id(name)), Some(rect), "{name}");
            // El ícono de 17 va a (3,3) del tramo: 1 px adentro del relleno, como en el mockup.
            let icon = drawn(&textures, id("icon.lasso")).expect("the icon");
            assert_eq!(icon.min - rect.min, vec2(3.0, 3.0), "{name}");
            let (_, _, textures) = tool(&ctx, &skin, slot, true, vec![]);
            assert_eq!(drawn_slots(&skin, &textures), [active], "{slot:?} selected");
        }
    }

    #[test]
    fn without_first_and_last_pieces_the_ends_use_the_middle_slot() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap().without(&[
            "tool.slot.first",
            "tool.slot.first.active",
            "tool.slot.last",
            "tool.slot.last.active",
        ]);
        let middle = skin.piece("tool.slot", WidgetState::Normal).unwrap();
        for slot in [SlotPos::First, SlotPos::Last] {
            let (rect, _, textures) = tool(&ctx, &skin, slot, false, vec![]);
            assert_eq!(rect.size(), vec2(23.0, 22.0), "{slot:?}");
            assert_eq!(drawn(&textures, middle.texture.id()), Some(rect));
        }
    }

    #[test]
    fn a_tool_button_clicks_with_the_skin_and_with_the_flat_fallback() {
        let ctx = egui::Context::default();
        let skins = [
            Skin::load(&ctx, crate::skin::EMBEDDED).unwrap(),
            Skin::fallback(),
        ];
        // Con el skin, el tramo del medio mide 23×22; el respaldo, 23×23.
        for (skin, size) in skins.iter().zip([vec2(23.0, 22.0), vec2(23.0, 23.0)]) {
            let middle = SlotPos::Middle;
            let (rect, _, _) = tool(&ctx, skin, middle, false, vec![]);
            assert_eq!(rect.size(), size);
            let at = rect.center();
            tool(&ctx, skin, middle, false, vec![Event::PointerMoved(at)]);
            tool(&ctx, skin, middle, false, vec![press(at, true)]);
            let (_, clicked, _) = tool(&ctx, skin, middle, false, vec![press(at, false)]);
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

    /// Los colores del skin embebido.
    fn mockup_colors() -> SkinColors {
        let ctx = egui::Context::default();
        *Skin::load(&ctx, crate::skin::EMBEDDED).unwrap().colors()
    }

    /// Los píxeles físicos de un rect a `ppp`.
    fn physical(r: Rect, ppp: f32) -> [f32; 4] {
        [r.min.x * ppp, r.min.y * ppp, r.max.x * ppp, r.max.y * ppp]
    }

    #[test]
    fn the_etched_lines_are_one_dark_and_one_light_pixel_of_the_mockup() {
        let colors = mockup_colors();
        let dark = Color32::from_rgb(189, 164, 153);
        let light = Color32::from_rgb(241, 235, 219);
        assert_eq!((colors.canvas, colors.highlight), (dark, light));
        for ppp in [1.0, 2.0, 3.0] {
            // Horizontal: la oscura en la fila 69 y el brillo debajo, en la 70.
            let h = colored_rects(&etched_hline(Rangef::new(28.0, 978.0), 69.0, &colors, ppp));
            assert_eq!(
                h,
                vec![
                    (
                        dark,
                        Rect::from_min_max(pos2(28.0, 69.0), pos2(978.0, 70.0))
                    ),
                    (
                        light,
                        Rect::from_min_max(pos2(28.0, 70.0), pos2(978.0, 71.0))
                    ),
                ],
                "at {ppp}"
            );
            // Vertical: la oscura en la columna 852 y el brillo a la derecha, en la 853.
            let v = colored_rects(&etched_vline(852.0, Rangef::new(70.0, 88.0), &colors, ppp));
            assert_eq!(
                v,
                vec![
                    (
                        dark,
                        Rect::from_min_max(pos2(852.0, 70.0), pos2(853.0, 88.0))
                    ),
                    (
                        light,
                        Rect::from_min_max(pos2(853.0, 70.0), pos2(854.0, 88.0))
                    ),
                ],
                "at {ppp}"
            );
            // Cada línea mide `ppp` píxeles físicos.
            let [_, top, _, bottom] = physical(h[0].1, ppp);
            assert_eq!(bottom - top, ppp);
        }
    }

    #[test]
    fn the_etched_lines_snap_to_physical_pixels_and_are_not_blurred() {
        let colors = mockup_colors();
        for ppp in [1.0, 2.0, 3.0] {
            let shapes = [
                etched_hline(Rangef::new(10.3, 50.6), 20.4, &colors, ppp),
                etched_vline(30.7, Rangef::new(5.2, 40.1), &colors, ppp),
            ];
            for shape in &shapes {
                let rects = colored_rects(shape);
                assert_eq!(rects.len(), 2, "{shape:?}");
                for (_, r) in rects {
                    for v in physical(r, ppp) {
                        assert_eq!(v, v.round(), "{r:?} at {ppp}");
                    }
                }
            }
            // Teselado: solo los dos colores, sin vértices semitransparentes de difuminado.
            let mut tessellator = egui::epaint::Tessellator::new(
                ppp,
                egui::epaint::TessellationOptions::default(),
                [1, 1],
                Vec::new(),
            );
            let clipped: Vec<egui::epaint::ClippedShape> = shapes
                .iter()
                .map(|s| egui::epaint::ClippedShape {
                    clip_rect: Rect::EVERYTHING,
                    shape: s.clone(),
                })
                .collect();
            for primitive in tessellator.tessellate_shapes(clipped) {
                let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                    panic!("only meshes");
                };
                for v in mesh.vertices {
                    assert!(
                        v.color == colors.canvas || v.color == colors.highlight,
                        "{:?} at {ppp}",
                        v.color
                    );
                }
            }
        }
    }

    #[test]
    fn the_toolbar_is_5_points_the_box_and_5_points() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let frame = skin.piece(BOX_PIECE, WidgetState::Normal).unwrap().size[0] as f32;
        assert_eq!(frame, 27.0);
        assert_eq!(toolbar_width(&skin), 5.0 + frame + 5.0);
        assert_eq!(toolbar_width(&skin), 37.0, "the mockup");
        // El respaldo: el botón de 23 con 1 px de aire y 1 de línea a cada lado.
        assert_eq!(toolbar_width(&Skin::fallback()), 37.0, "the fallback");
    }

    /// Un frame con dos botones dentro de la caja de la barra: el rect de la caja (lo que
    /// devuelve `toolbar_box`), el de cada botón y las texturas dibujadas.
    fn boxed(ctx: &egui::Context, skin: &Skin) -> (Rect, [Rect; 2], Vec<(egui::TextureId, Rect)>) {
        let mut out = (Rect::NOTHING, [Rect::NOTHING; 2]);
        let textures = run(ctx, screen(), |ui| {
            let mut buttons = [Rect::NOTHING; 2];
            let frame = toolbar_box(ui, skin, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, slot) in [SlotPos::First, SlotPos::Last].into_iter().enumerate() {
                    let id = egui::Id::new(("boxed", i));
                    let r = tool_button(ui, id, skin, slot, "icon.pencil", "Pencil", false, false);
                    buttons[i] = r.rect;
                }
            });
            out = (frame, buttons);
        });
        (out.0, out.1, textures)
    }

    #[test]
    fn the_toolbar_box_goes_around_the_buttons_with_one_pixel_of_air() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let (frame, [first, last], textures) = boxed(&ctx, &skin);
        let id = skin
            .piece(BOX_PIECE, WidgetState::Normal)
            .unwrap()
            .texture
            .id();
        assert_eq!(
            drawn(&textures, id),
            Some(frame),
            "the box piece, stretched"
        );
        // 1 px de línea y 1 de aire: los botones empiezan 2 px adentro, en los dos ejes.
        assert_eq!(
            first.min - frame.min,
            vec2(2.0, 2.0),
            "{first:?} in {frame:?}"
        );
        assert_eq!(
            frame.max - last.max,
            vec2(2.0, 2.0),
            "{last:?} in {frame:?}"
        );
        assert_eq!(frame.size(), vec2(27.0, 22.0 + 23.0 + 4.0));
        assert_eq!(last.min.y, first.max.y, "no gap");
        // Sin la pieza, la caja de respaldo mide lo mismo y los botones van igual.
        let fallback = Skin::fallback();
        let (frame, [first, last], _) = boxed(&ctx, &fallback);
        assert_eq!(frame.width(), 27.0);
        assert_eq!(first.min - frame.min, vec2(2.0, 2.0));
        assert_eq!(frame.max - last.max, vec2(2.0, 2.0));
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
