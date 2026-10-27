//! Piezas de la UI en pixel art dibujadas con el skin: diálogos y botones. Si el skin no trae una
//! pieza, se dibuja un respaldo plano con sus colores (fondo `panel`, borde `outline` de 1 px, sin
//! redondeo).

use eframe::egui::{
    self, Align2, Color32, Id, Mesh, Pos2, Rangef, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;
use std::ops::Range;

use crate::skin::{IRect, Piece, Skin, SkinColors, WidgetState, nine_slice};
use crate::tab_bar;

/// Marco de los diálogos: la ventana de barra rosa del mockup de "Unsaved changes".
const DIALOG_PIECE: &str = "window";
const BUTTON_PIECE: &str = "button";
/// Botón de cerrar de la barra de título.
const CLOSE_PIECE: &str = "window.close";
/// Margen entre el área del centro del 9-slice y lo de adentro (contenido o texto).
pub(crate) const PADDING: f32 = 4.0;
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
/// En la memoria de egui, con el id del diálogo: la posición a la que se lo arrastró y la pasada
/// en que se mostró por última vez.
const MOVED_KEY: &str = "pixel_dialog_moved";
/// En la memoria de egui, con el id del diálogo: dónde estaba al empezar a arrastrar la barra.
const DRAG_START_KEY: &str = "pixel_dialog_drag_start";
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
const FALLBACK_TOGGLE_SIZE: Vec2 = Vec2::new(15.0, 16.0);
/// Esquina del ícono dentro de `sym.slot`: el interior del cuadro de 13×13 empieza en (2,2).
const TOGGLE_ICON_OFFSET: Vec2 = Vec2::new(2.0, 2.0);
/// Lado del interior de `sym.slot`, donde va el ícono.
const TOGGLE_ICON_SIZE: Vec2 = Vec2::new(11.0, 11.0);

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
    flat_frame(skin.colors(), rect, title_bar, pixels_per_point)
}

/// [`flat_shapes`] con los colores sueltos (para quien guarda solo los colores del skin).
pub(crate) fn flat_frame(
    colors: &SkinColors,
    rect: Rect,
    title_bar: f32,
    pixels_per_point: f32,
) -> Vec<Shape> {
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

/// Diálogo con el marco del skin en 9-slice, anclado en `anchor` (alineación y desfase). El título va centrado en
/// la barra de arriba y el contenido en el área del centro, con 4 px de margen. El texto de
/// adentro usa `color.text`.
///
/// Se mueve arrastrando la barra de título (no el cuerpo ni el botón de cerrar) y no sale de la
/// pantalla. Queda donde se lo dejó mientras siga abierto; al abrirlo de nuevo, vuelve al centro.
///
/// Con `closable`, a la derecha de la barra va el botón de cerrar (`window.close`, o
/// `window.close.hover` con el puntero encima; si el skin no lo trae, una × de líneas). Si se
/// hace clic en él, [`DialogOutput::closed`] es `true`; qué hacer con eso lo decide quien llama.
pub fn pixel_dialog_at<R>(
    ctx: &egui::Context,
    skin: &Skin,
    id: Id,
    title: &str,
    closable: bool,
    anchor: (Align2, Vec2),
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> DialogOutput<R> {
    pixel_dialog_with_bar(
        ctx,
        skin,
        id,
        title,
        closable,
        anchor,
        add_contents,
        |_, _| {},
    )
}

/// Como [`pixel_dialog_at`], y `bar_contents` dibuja en la barra de título (la recibe), encima
/// de ella: los controles de la ventana sin botón, como el play del preview.
#[allow(clippy::too_many_arguments)]
pub fn pixel_dialog_with_bar<R>(
    ctx: &egui::Context,
    skin: &Skin,
    id: Id,
    title: &str,
    closable: bool,
    anchor: (Align2, Vec2),
    add_contents: impl FnOnce(&mut Ui) -> R,
    bar_contents: impl FnOnce(&mut Ui, Rect),
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
    // Abre en su lugar (el ancla) y, si se lo arrastró de la barra de título, sigue donde quedó.
    // Un `Area` anclado no se mueve aunque sea `movable`: la posición movida va en la memoria.
    let moved = moved_dialog_pos(ctx, id);
    let area = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .movable(false);
    let area = match moved {
        Some(pos) => area.current_pos(pos),
        None => area.anchor(anchor.0, anchor.1),
    };
    area.show(ctx, |ui| {
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
        // Las mayúsculas y lo que baja de la línea (p, y, g) centrados entre el borde de arriba y
        // la línea de abajo de la barra: nada se encima en la línea.
        let room = top - 2.0;
        let ink = crate::tab_bar::CAP_HEIGHT + TITLE_DESCENT;
        let caps = bar.top() + 1.0 + ((room - ink) / 2.0).round();
        let left = (bar.center().x - galley.size().x / 2.0).round();
        let at = crate::tab_bar::text_pos(&galley, left, caps, ppp);
        shapes.push(Shape::galley(at, galley, text));
        ui.painter().set(background, Shape::Vec(shapes));
        let close = closable.then(|| close_rect(skin, bar, ppp));
        drag_title_bar(ui, id, rect, bar, close);
        let closed = close.is_some_and(|r| close_button(ui, skin, id.with("close"), r));
        bar_contents(ui, bar);
        DialogOutput {
            inner: inner.inner,
            closed,
        }
    })
    .inner
}

/// Lo que bajan de la línea las letras con descendente en el título (p, y, g).
const TITLE_DESCENT: f32 = 2.0;

/// Lo que devuelve [`pixel_dialog_at`]: lo que devolvió el contenido y si se hizo clic en el botón
/// de cerrar de la barra de título.
pub struct DialogOutput<R> {
    pub inner: R,
    pub closed: bool,
}

/// La posición a la que se arrastró el diálogo `id`, si sigue abierto desde entonces. Solo vale
/// si se mostró en la pasada anterior: al cerrarlo se olvida, y al abrirlo de nuevo vuelve a su
/// lugar de siempre.
fn moved_dialog_pos(ctx: &egui::Context, id: Id) -> Option<Pos2> {
    let key = id.with(MOVED_KEY);
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let kept = d
            .get_temp::<(Pos2, u64)>(key)
            .filter(|&(_, last)| last + 1 >= pass)
            .map(|(pos, _)| pos);
        if let Some(pos) = kept {
            d.insert_temp(key, (pos, pass));
        } else {
            d.remove::<(Pos2, u64)>(key);
        }
        kept
    })
}

/// Mover el diálogo `id` (que ocupa `rect`) arrastrando su barra de título `bar`. Lo que empieza
/// en el botón de cerrar (`close`) no lo mueve. Queda entero dentro de la pantalla; la posición
/// nueva rige desde el frame siguiente.
fn drag_title_bar(ui: &Ui, id: Id, rect: Rect, bar: Rect, close: Option<Rect>) {
    let ctx = ui.ctx();
    let start_key = id.with(DRAG_START_KEY);
    let response = ui.interact(bar, id.with("title_bar"), Sense::drag());
    if response.drag_started() {
        let origin = ctx.input(|i| i.pointer.press_origin());
        let on_close = origin.zip(close).is_some_and(|(p, c)| c.contains(p));
        if !on_close {
            ctx.data_mut(|d| d.insert_temp(start_key, rect.min));
        }
    }
    let start = ctx.data(|d| d.get_temp::<Pos2>(start_key));
    if let Some(start) = start
        && response.dragged()
    {
        let screen = ctx.content_rect();
        let max = (screen.max - rect.size()).max(screen.min);
        let to = (start + response.total_drag_delta().unwrap_or_default()).clamp(screen.min, max);
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|d| d.insert_temp(id.with(MOVED_KEY), (to, pass)));
        ctx.request_repaint();
    }
    if start.is_some() && !response.dragged() {
        ctx.data_mut(|d| d.remove::<Pos2>(start_key));
    }
}

/// El botón de cerrar de la barra de título `bar`: a la derecha, centrado en vertical y con el
/// mismo margen a la derecha que arriba.
fn close_rect(skin: &Skin, bar: Rect, pixels_per_point: f32) -> Rect {
    let size = skin
        .piece(CLOSE_PIECE, WidgetState::Normal)
        .map_or(FALLBACK_CLOSE_SIZE, |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        });
    let inset = ((bar.height() - size.y) / 2.0).floor().max(0.0);
    Rect::from_min_size(pos2(bar.max.x - inset - size.x, bar.min.y + inset), size)
        .round_to_pixels(pixels_per_point)
}

/// Botón de cerrar en `rect` (ver [`close_rect`]). Devuelve si se hizo clic.
fn close_button(ui: &mut Ui, skin: &Skin, id: Id, rect: Rect) -> bool {
    let ppp = ui.ctx().pixels_per_point();
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
/// El texto de un botón como se muestra: "…" con los puntos de la fuente pixel (si no lo tiene,
/// el de la fuente de respaldo no es pixel art), como en los menús.
fn shown_label(label: &str) -> String {
    label.replace('…', tab_bar::ellipsis())
}

fn button_size(ui: &Ui, skin: &Skin, label: &str) -> Vec2 {
    let font = TextStyle::Button.resolve(ui.style());
    let text = ui
        .painter()
        .layout_no_wrap(shown_label(label), font, skin.colors().text)
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

/// Un botón con `button` del skin de `size` exacto (en los diálogos con su lugar del diseño).
pub fn pixel_button_sized(ui: &mut Ui, skin: &Skin, label: &str, size: Vec2) -> Response {
    button_of_size(ui, skin, label, size)
}

/// Un botón de `size` exacto, dibujado con la pieza del estado en que está.
/// Un botón de `size` (el cuerpo; debajo va la luz: ocupa 1 fila más), con la pieza del estado
/// en que está y el texto con las mayúsculas en [`CONTROL_CAPS`].
fn button_of_size(ui: &mut Ui, skin: &Skin, label: &str, size: Vec2) -> Response {
    let colors = *skin.colors();
    let font = TextStyle::Button.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(shown_label(label), font, colors.text);
    let base = skin.piece(BUTTON_PIECE, WidgetState::Normal);
    let [left, top, right, bottom] = base.map_or(FALLBACK_BUTTON, borders);
    let (slot, response) = ui.allocate_exact_size(
        vec2(size.x, size.y + CONTROL_HEIGHT - CONTROL_BODY),
        Sense::click(),
    );
    let rect = Rect::from_min_size(slot.min, size);
    under_light(ui, rect);
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
        // Deshabilitado, como en el diseño: el mismo botón (sin la transparencia de egui) con el
        // texto en `button_text_disabled`.
        let mut painter = ui.painter().clone();
        let text_color = if enabled {
            colors.text
        } else {
            painter.set_opacity(1.0);
            colors.button_text_disabled
        };
        let ppp = ui.ctx().pixels_per_point();
        match skin.piece(BUTTON_PIECE, state) {
            Some(piece) => painter.extend(piece_shapes(piece, rect, ppp)),
            None => painter.extend(flat_shapes(skin, rect, 0.0, ppp)),
        }
        let inner = Rect::from_min_max(rect.min + vec2(left, top), rect.max - vec2(right, bottom));
        let at = label_pos(inner, galley.size(), state, ppp);
        // A lo alto, las mayúsculas en su fila (como en los campos).
        let down = if state == WidgetState::Down { 1.0 } else { 0.0 };
        let at = crate::tab_bar::text_pos(&galley, at.x, rect.top() + CONTROL_CAPS + down, ppp);
        painter.galley(at, galley, text_color);
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

/// Un botón elegido como en la barra de herramientas: no se hunde; la pieza de siempre con su
/// relleno liso y 1 px rosa por dentro del borde (sin el brillo ni el bisel). Abajo cubre también
/// la luz; con `bottom_line`, hasta la línea de abajo de la pieza.
pub(crate) fn selected_shapes(
    piece: &Piece,
    rect: Rect,
    bottom_line: bool,
    colors: &SkinColors,
    ppp: f32,
) -> Vec<Shape> {
    let mut shapes = piece_shapes(piece, rect, ppp);
    let [left, top, right, bottom] = borders(piece);
    let below = if bottom_line { bottom.min(1.0) } else { 0.0 };
    let inner = Rect::from_min_max(
        rect.min + vec2(left.min(1.0), top.min(1.0)),
        rect.max - vec2(right.min(1.0), below),
    );
    // En una malla, como el resto del pixel art (sin el difuminado de los rects de egui).
    let mut mesh = egui::Mesh::default();
    mesh.add_colored_rect(inner.round_to_pixels(ppp), colors.panel);
    for r in [
        Rect::from_min_max(inner.min, pos2(inner.right(), inner.top() + 1.0)),
        Rect::from_min_max(pos2(inner.left(), inner.bottom() - 1.0), inner.max),
        Rect::from_min_max(inner.min, pos2(inner.left() + 1.0, inner.bottom())),
        Rect::from_min_max(pos2(inner.right() - 1.0, inner.top()), inner.max),
    ] {
        mesh.add_colored_rect(r.round_to_pixels(ppp), colors.timeline_selection);
    }
    shapes.push(Shape::mesh(mesh));
    shapes
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
    // La elegida no se hunde: el botón de siempre con su relleno liso y 1 px rosa por dentro
    // del borde (abajo).
    let state = if pressed {
        WidgetState::Down
    } else if response.hovered() {
        WidgetState::Hover
    } else {
        WidgetState::Normal
    };
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = match slot_piece(skin, slot, state) {
        // Los tramos de arriba y del medio terminan en la luz (la línea es la de arriba del
        // siguiente); el último trae su línea de abajo.
        Some(piece) if selected && !pressed => {
            selected_shapes(piece, rect, slot == SlotPos::Last, &colors, ppp)
        }
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
    // Centrado en la fila puede caer en medio punto (16 en 19): se dibuja en puntos enteros, que
    // con la escala entera de Pixel UI son píxeles del skin enteros.
    let rect = Rect::from_min_size(rect.min.round(), size);
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

/// Globo de los tooltips de la barra de íconos: el del mockup (frame 2, junto a la barra), con
/// la cola a la izquierda. Su 9-slice deja la cola fija: las filas de la cola van en el borde de
/// arriba y sus columnas en el de la izquierda, así el globo crece a la derecha y hacia abajo.
const BUBBLE_PIECE: &str = "tooltip.bubble";
/// Ancho de la cola: el cuerpo empieza en la columna 8 de la pieza.
const BUBBLE_TAIL: f32 = 8.0;
/// Fila de la punta de la cola, contada desde arriba del cuerpo.
const BUBBLE_TIP_ROW: f32 = 8.0;
/// Aire entre el botón y la punta de la cola: en el mockup el botón termina en la x 55 y la cola
/// empieza en la 62.
const BUBBLE_GAP: f32 = 6.0;
/// Aire del cuerpo alrededor del texto. En el mockup, una línea de mayúsculas (7 px) centrada en
/// las 19 filas de relleno del cuerpo deja 6 arriba y 6 abajo; a los lados va el mismo.
const BUBBLE_PADDING: f32 = 6.0;
/// La fila de sombra debajo del relleno del cuerpo.
const BUBBLE_SHADOW: f32 = 1.0;
/// Lo más ancho que se hace el texto del globo (después parte en líneas) y cuántas líneas
/// muestra como mucho (si no entra, termina en puntos suspensivos).
const BUBBLE_MAX_WIDTH: f32 = 200.0;
const BUBBLE_MAX_ROWS: usize = 3;

/// El texto del globo: en líneas de hasta `BUBBLE_MAX_WIDTH` y, si pasa de `BUBBLE_MAX_ROWS`,
/// recortado con los puntos suspensivos de la fuente pixel.
fn bubble_galley(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
) -> std::sync::Arc<egui::Galley> {
    let layout = |text: String| {
        let mut job = egui::text::LayoutJob::single_section(
            text,
            egui::TextFormat::simple(font.clone(), color),
        );
        job.wrap.max_width = BUBBLE_MAX_WIDTH;
        ui.painter().layout_job(job)
    };
    let galley = layout(text.to_owned());
    if galley.rows.len() <= BUBBLE_MAX_ROWS {
        return galley;
    }
    // Lo que entra en las primeras líneas, y de ahí se quita de a palabra hasta que entre con
    // los puntos.
    let shown: usize = galley.rows[..BUBBLE_MAX_ROWS]
        .iter()
        .map(|r| r.char_count_including_newline().0)
        .sum();
    let mut words: Vec<&str> = text
        .char_indices()
        .nth(shown)
        .map_or(text, |(i, _)| &text[..i])
        .split_whitespace()
        .collect();
    while !words.is_empty() {
        let cut = format!("{}{}", words.join(" "), tab_bar::ellipsis());
        let galley = layout(cut);
        if galley.rows.len() <= BUBBLE_MAX_ROWS {
            return galley;
        }
        words.pop();
    }
    layout(tab_bar::ellipsis().to_owned())
}

/// Alto del texto de `galley` en la fuente pixel: de la fila de arriba de las mayúsculas de la
/// primera línea a la de abajo de las de la última.
fn caps_block_height(galley: &egui::Galley) -> f32 {
    let baselines: Vec<f32> = galley
        .rows
        .iter()
        .filter_map(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y))
        .collect();
    match (baselines.first(), baselines.last()) {
        (Some(first), Some(last)) => last - first + tab_bar::cap_height(galley),
        _ => tab_bar::cap_height(galley),
    }
}

/// El tooltip de un botón de la barra de íconos: `text` en el globo `tooltip.bubble`, a la
/// derecha del botón, con la punta de la cola a `BUBBLE_GAP` de su borde y la fila de arriba de
/// la punta en su centro. El cuerpo crece con el texto (`color.text`, con `BUBBLE_PADDING` de
/// aire).
///
/// Aparece y se va como los tooltips de egui (`Tooltip::for_enabled`, con su demora). El texto
/// queda en el árbol de accesibilidad como etiqueta. Sin la pieza, el tooltip de egui.
pub fn bubble_tooltip(response: Response, skin: &Skin, text: &str) -> Response {
    match skin.piece(BUBBLE_PIECE, WidgetState::Normal) {
        Some(piece) => show_bubble(response, piece, skin.colors().text, text),
        None => response.on_hover_text(text),
    }
}

/// El skin de este frame para los widgets que no lo reciben (tooltips, botones sueltos, scroll,
/// campos): lo guarda la app con [`set_bubble`] en la memoria de egui, en un `Arc` (no se copia).
fn frame_skin_id() -> Id {
    Id::new("pixel_frame_skin")
}

/// El skin de este frame (ver [`set_bubble`]); `None` sin Pixel UI.
pub fn frame_skin(ctx: &egui::Context) -> Option<std::sync::Arc<Skin>> {
    ctx.data(|d| d.get_temp::<std::sync::Arc<Skin>>(frame_skin_id()))
}

/// El borde oscuro de los controles de este frame: el del skin o, sin Pixel UI, el del tema.
pub fn control_border(ctx: &egui::Context) -> Color32 {
    frame_skin(ctx).map_or(crate::theme::CANVAS_BORDER, |s| s.colors().control_border)
}

/// Con Pixel UI (`skin`), los widgets de este frame usan el skin (el globo, los botones, el
/// scroll y los campos del diseño); sin él, los de egui.
pub fn set_bubble(ctx: &egui::Context, skin: Option<std::sync::Arc<Skin>>) {
    ctx.data_mut(|d| match skin {
        Some(s) => {
            d.insert_temp(frame_skin_id(), s);
        }
        None => d.remove::<std::sync::Arc<Skin>>(frame_skin_id()),
    });
}

/// Un botón suelto con texto: con Pixel UI, el `button` del skin ([`pixel_button`]); sin él, el
/// de egui.
/// Un botón de 25×16 con el ícono `icon` del skin al centro (como el de la forma del pincel):
/// `label` es su nombre accesible y su tooltip. Con `on`, prendido (`button.active`). Sin Pixel UI
/// (o sin el ícono), un botón de texto con `label`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn icon_button(
    ui: &mut Ui,
    skin: Option<&Skin>,
    icon: &str,
    label: &str,
    on: Option<bool>,
) -> Response {
    let skin = skin.filter(|s| s.piece(icon, WidgetState::Normal).is_some());
    let Some(skin) = skin else {
        return match on {
            Some(on) => ui.selectable_label(on, label),
            None => ui.button(label),
        };
    };
    // Los íconos anchos (la cadena de Lospec) llevan 5 px de aire a cada lado.
    let icon_w = crate::pixel_draw::piece_size(skin, icon).x;
    let size = vec2(ICON_BUTTON.x.max(icon_w + 10.0), ICON_BUTTON.y);
    let (slot, _) = ui.allocate_exact_size(vec2(size.x, CONTROL_HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), size);
    under_light(ui, rect);
    let ppp = ui.ctx().pixels_per_point();
    let info = crate::pixel_draw::ButtonInfo {
        selected: on,
        ..crate::pixel_draw::ButtonInfo::new(ui.id().with(("icon_button", label)), label)
    };
    crate::pixel_draw::skin_button(ui, skin, rect, info, |inner| {
        // Centrado en el relleno (sin las filas de abajo del botón); si no cabe justo a la mitad,
        // el pixel de sobra queda arriba (como en el diseño y en `led_button`).
        let fill = Rect::from_min_max(inner.min, inner.max - vec2(0.0, 2.0));
        let size = crate::pixel_draw::piece_size(skin, icon);
        let at = pos2(
            (fill.center().x - size.x / 2.0).floor(),
            (fill.center().y - size.y / 2.0).ceil(),
        );
        crate::pixel_draw::piece_at(skin, icon, at, ppp)
    })
    .tip(label)
}

/// El botón con ícono: 25×16, como los de la barra.
const ICON_BUTTON: Vec2 = vec2(25.0, 16.0);

/// La norma de los controles con Pixel UI (botones, campos, combos, grupos de opciones): 17 de
/// alto, el cuerpo de 16 y debajo 1 fila de luz (`highlight`). En los campos la luz viene en su
/// pieza; a los botones se la pone [`under_light`]. Las mayúsculas de los textos van 4 filas
/// debajo del borde de arriba.
pub const CONTROL_HEIGHT: f32 = 17.0;
pub const CONTROL_BODY: f32 = 16.0;
pub const CONTROL_CAPS: f32 = 4.0;

/// Antes era el botón "con luz" de los paneles; ahora todos los botones la llevan.
pub fn lit_button(ui: &mut Ui, label: &str) -> Response {
    text_button(ui, label)
}

/// La luz de abajo de `rect` como en las casillas, con Pixel UI: en las dos esquinas de abajo
/// de `rect` (las de la pieza redondeada quedan vacías) y 1 fila debajo, entre las esquinas. Va
/// antes de dibujar el control, que la tapa donde tiene pixeles.
pub fn under_light(ui: &Ui, rect: Rect) {
    if let Some(skin) = frame_skin(ui.ctx()) {
        let ppp = ui.ctx().pixels_per_point();
        let (bottom, left, right) = (rect.bottom(), rect.left(), rect.right());
        let rows = [
            Rect::from_min_max(pos2(left + 1.0, bottom), pos2(right - 1.0, bottom + 1.0)),
            Rect::from_min_max(pos2(left, bottom - 1.0), pos2(left + 1.0, bottom)),
            Rect::from_min_max(pos2(right - 1.0, bottom - 1.0), pos2(right, bottom)),
        ];
        ui.painter().extend(rows.map(|r| {
            Shape::rect_filled(
                eframe::emath::GuiRounding::round_to_pixels(r, ppp),
                0.0,
                skin.colors().highlight,
            )
        }));
    }
}

pub fn text_button(ui: &mut Ui, label: &str) -> Response {
    match frame_skin(ui.ctx()) {
        Some(skin) => pixel_button(ui, &skin, label),
        None => ui.button(label),
    }
}

/// Las barras de scroll del diseño (frame 4): el track violeta de 10 con su borde claro y el
/// pulgar con las estrías, en 9-slice (las estrías van en el borde fijo).
const SCROLL_TRACK_V: &str = "scroll.track.v";
const SCROLL_THUMB_V: &str = "scroll.thumb.v";
const SCROLL_TRACK_H: &str = "scroll.track.h";
const SCROLL_THUMB_H: &str = "scroll.thumb.h";
/// El ancho de la barra y el largo mínimo del pulgar (entra entero, con sus estrías).
const SCROLL_BAR: f32 = 10.0;
const SCROLL_THUMB_MIN: f32 = 46.0;
/// Del borde del track al pulgar, de los cuatro lados (10 = 1 + 8 + 1 a lo ancho; a lo largo,
/// el pulgar no toca las puntas del track).
const SCROLL_THUMB_INSET: f32 = 1.0;

/// El pulgar del scroll en `track` (vertical o no) para `content` de largo visto en `view` y
/// desplazado `offset`: con 1 px de padding de cada lado del track, también en las puntas.
fn scroll_thumb(track: Rect, vertical: bool, content: f32, view: f32, offset: f32) -> Rect {
    // Lo que recorre el pulgar: el track sin el padding de las puntas.
    let span = if vertical {
        track.height()
    } else {
        track.width()
    } - 2.0 * SCROLL_THUMB_INSET;
    let len = (span * view / content)
        .max(SCROLL_THUMB_MIN)
        .min(span)
        .round();
    let max = (content - view).max(1.0);
    let at = SCROLL_THUMB_INSET + ((offset / max).clamp(0.0, 1.0) * (span - len)).round();
    if vertical {
        Rect::from_min_size(
            pos2(track.left() + SCROLL_THUMB_INSET, track.top() + at),
            vec2(SCROLL_BAR - 2.0 * SCROLL_THUMB_INSET, len),
        )
    } else {
        Rect::from_min_size(
            pos2(track.left() + at, track.top() + SCROLL_THUMB_INSET),
            vec2(len, SCROLL_BAR - 2.0 * SCROLL_THUMB_INSET),
        )
    }
}

/// Un `ScrollArea` de la app: con Pixel UI, con las barras del diseño. Las de egui siguen
/// manejando el arrastre y la rueda, pero invisibles (de 10, sin márgenes ni flotar); encima se
/// pintan el track y el pulgar del skin donde egui los tiene. Sin Pixel UI, las de siempre.
pub fn scroll<R>(
    ui: &mut Ui,
    area: egui::ScrollArea,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let skin =
        frame_skin(ui.ctx()).filter(|s| s.piece(SCROLL_THUMB_V, WidgetState::Normal).is_some());
    let Some(skin) = skin else {
        return area.show(ui, add_contents).inner;
    };
    ui.scope(|ui| {
        let s = &mut ui.spacing_mut().scroll;
        s.floating = false;
        s.bar_width = SCROLL_BAR;
        s.bar_inner_margin = 0.0;
        s.bar_outer_margin = 0.0;
        s.handle_min_length = SCROLL_THUMB_MIN;
        s.dormant_background_opacity = 0.0;
        s.active_background_opacity = 0.0;
        s.interact_background_opacity = 0.0;
        s.dormant_handle_opacity = 0.0;
        s.active_handle_opacity = 0.0;
        s.interact_handle_opacity = 0.0;
        let out = area.show(ui, add_contents);
        let ppp = ui.ctx().pixels_per_point();
        let inner = out.inner_rect;
        let mut shapes = Vec::new();
        for vertical in [true, false] {
            let (content, view) = if vertical {
                (out.content_size.y, inner.height())
            } else {
                (out.content_size.x, inner.width())
            };
            if content <= view + 0.5 {
                continue;
            }
            let offset = if vertical {
                out.state.offset.y
            } else {
                out.state.offset.x
            };
            let track = if vertical {
                Rect::from_x_y_ranges(inner.right()..=inner.right() + SCROLL_BAR, inner.y_range())
            } else {
                Rect::from_x_y_ranges(
                    inner.x_range(),
                    inner.bottom()..=inner.bottom() + SCROLL_BAR,
                )
            };
            let track = track.round_to_pixels(ppp);
            let thumb = scroll_thumb(track, vertical, content, view, offset);
            let (track_piece, thumb_piece) = if vertical {
                (SCROLL_TRACK_V, SCROLL_THUMB_V)
            } else {
                (SCROLL_TRACK_H, SCROLL_THUMB_H)
            };
            for (name, rect) in [(track_piece, track), (thumb_piece, thumb)] {
                if let Some(piece) = skin.piece(name, WidgetState::Normal) {
                    shapes.extend(piece_shapes(piece, rect, ppp));
                }
            }
        }
        ui.painter().extend(shapes);
        out.inner
    })
    .inner
}

/// Los tooltips de la app: con Pixel UI, el globo del skin (como los de la barra de íconos);
/// sin él, el tooltip de egui.
pub trait Tip {
    fn tip(self, text: impl AsRef<str>) -> Response;
}

impl Tip for Response {
    fn tip(self, text: impl AsRef<str>) -> Response {
        let text = text.as_ref();
        let skin = frame_skin(&self.ctx);
        match skin.as_ref().and_then(|s| {
            s.piece(BUBBLE_PIECE, WidgetState::Normal)
                .map(|p| (p.clone(), s.colors().text))
        }) {
            Some((piece, color)) => show_bubble(self, &piece, color, text),
            None => self.on_hover_text(text),
        }
    }
}

/// `shapes` espejados de izquierda a derecha alrededor del centro de `rect` (el globo con la cola
/// a la derecha).
fn mirrored(shapes: Vec<Shape>, rect: Rect) -> Vec<Shape> {
    let axis = rect.left() + rect.right();
    shapes
        .into_iter()
        .map(|shape| match shape {
            Shape::Mesh(mesh) => {
                let mut mesh = (*mesh).clone();
                for v in &mut mesh.vertices {
                    v.pos.x = axis - v.pos.x;
                }
                Shape::mesh(mesh)
            }
            other => other,
        })
        .collect()
}

/// El globo de `piece` con `text` junto a `response`: a la derecha, con la cola hacia el
/// elemento; si ahí no entra, a la izquierda y espejado (la cola del otro lado).
fn show_bubble(response: Response, piece: &Piece, color: Color32, text: &str) -> Response {
    let button = response.rect;
    let mut tooltip = egui::Tooltip::for_enabled(&response);
    // Sin marco, arriba alineado con el elemento: el área empieza en la punta de la cola.
    tooltip.popup = tooltip
        .popup
        .frame(egui::Frame::NONE)
        .align(egui::RectAlign::RIGHT_START)
        .align_alternatives(&[egui::RectAlign::LEFT_START])
        .gap(BUBBLE_GAP);
    tooltip.show(|ui| {
        let ppp = ui.ctx().pixels_per_point();
        // Los textos chicos (chimpfont con Pixel UI).
        let font = TextStyle::Small.resolve(ui.style());
        let galley = bubble_galley(ui, text, font, color);
        let body = vec2(
            (2.0 * BUBBLE_PADDING + galley.size().x).ceil(),
            2.0 * BUBBLE_PADDING + caps_block_height(&galley) + BUBBLE_SHADOW,
        );
        let origin = ui.max_rect().min;
        // ¿Quedó a la izquierda del elemento? Entonces va espejado.
        let left = origin.x < button.left();
        // La fila del centro del elemento (con alto impar, la que lo contiene).
        let tip = origin.y + (button.height() / 2.0).floor();
        let rect = Rect::from_min_size(
            pos2(origin.x, tip - BUBBLE_TIP_ROW),
            vec2(BUBBLE_TAIL + body.x, body.y),
        );
        let space = rect.union(Rect::from_min_size(origin, Vec2::ZERO));
        let label = ui.allocate_rect(space, Sense::hover());
        label.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
        let body_left = if left {
            rect.left()
        } else {
            rect.left() + BUBBLE_TAIL
        };
        let at = tab_bar::text_pos(
            &galley,
            body_left + BUBBLE_PADDING,
            rect.top() + BUBBLE_PADDING,
            ppp,
        );
        let mut shapes = piece_shapes(piece, rect, ppp);
        if left {
            shapes = mirrored(shapes, rect);
        }
        shapes.push(Shape::galley(at, galley, color));
        ui.painter().extend(shapes);
    });
    response
}

const WORKSPACE_PIECE: &str = "workspace.box";
/// Aire de fondo entre la caja del área de trabajo y lo que la rodea: arriba, a la derecha y
/// abajo. A la izquierda ya está el de la barra de íconos (`TOOLBAR_MARGIN`).
pub const WORKSPACE_MARGIN: i8 = 5;
/// Borde de la caja sin la pieza: la línea, el aire y otra línea, como en el mockup.
const FALLBACK_WORKSPACE_BORDER: f32 = 3.0;

/// La caja del área de trabajo del mockup (`workspace.box`, en 9-slice) en `rect`. Devuelve el
/// interior, donde van las reglas y el lienzo: `rect` sin los bordes de la pieza. Sin la pieza,
/// un borde `outline` de 1 px.
pub fn workspace_box(ui: &Ui, skin: &Skin, rect: Rect) -> Rect {
    let ppp = ui.ctx().pixels_per_point();
    match skin.piece(WORKSPACE_PIECE, WidgetState::Normal) {
        Some(piece) => {
            ui.painter().extend(piece_shapes(piece, rect, ppp));
            let [left, top, right, bottom] = borders(piece);
            Rect::from_min_max(rect.min + vec2(left, top), rect.max - vec2(right, bottom))
        }
        None => {
            ui.painter().rect_stroke(
                rect.round_to_pixels(ppp),
                0.0,
                Stroke::new(1.0, skin.colors().outline),
                StrokeKind::Inside,
            );
            rect.shrink(FALLBACK_WORKSPACE_BORDER)
        }
    }
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

/// Reparte en filas, en orden, los grupos de una barra pixel que no se parten (botones, tiras de
/// campos): cada fila lleva los que entran en `available`, y el que no entra pasa a la fila
/// siguiente.
/// - `widths[i]`: el ancho del grupo `i` solo en su fila, con sus dos puntas.
/// - `joins[i]` (uno menos que `widths`): cuánto cambia el ancho de la fila cuando los grupos `i`
///   e `i + 1` van juntos. El aire entre los dos, o un número negativo si al juntarse pierden sus
///   puntas y comparten una línea.
///
/// Un grupo más ancho que `available` va solo en su fila (no hay dónde partirlo). Devuelve las
/// filas como rangos de índices de `widths`.
pub(crate) fn wrap_rows(available: f32, widths: &[f32], joins: &[f32]) -> Vec<Range<usize>> {
    debug_assert_eq!(joins.len() + 1, widths.len().max(1));
    let mut rows = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (i, &width) in widths.iter().enumerate() {
        if i > start && used + joins[i - 1] + width > available {
            rows.push(start..i);
            start = i;
        }
        used = if i == start {
            width
        } else {
            used + joins[i - 1] + width
        };
    }
    if start < widths.len() {
        rows.push(start..widths.len());
    }
    rows
}

/// La ventana de la app, la misma en todos lados: con Pixel UI (`skin`), [`pixel_dialog_at`] (el
/// marco `window` del skin, el título centrado, la × si `closable` y se mueve de la barra); sin
/// él, la ventana de egui de siempre, fija y sin plegar. `id` es el del área del diálogo pixel.
/// Devuelve lo que devolvió el contenido (`None` si egui no lo mostró) y si se cerró con la ×.
pub fn app_window<R>(
    ctx: &egui::Context,
    skin: Option<&Skin>,
    id: &str,
    title: &str,
    closable: bool,
    anchor: (Align2, Vec2),
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> (Option<R>, bool) {
    match skin {
        Some(skin) => {
            let out = pixel_dialog_at(
                ctx,
                skin,
                Id::new(id),
                title,
                closable,
                anchor,
                add_contents,
            );
            (Some(out.inner), out.closed)
        }
        None => {
            let mut open = true;
            // Otro id que el del diálogo pixel: el área `id` es solo la de Pixel UI.
            let window = egui::Window::new(title.to_owned())
                .id(Id::new(id).with("egui"))
                .collapsible(false)
                .resizable(false)
                .anchor(anchor.0, anchor.1);
            let window = if closable {
                window.open(&mut open)
            } else {
                window
            };
            let inner = window.show(ctx, add_contents).and_then(|r| r.inner);
            (inner, !open)
        }
    }
}

/// La fila de botones de una ventana ([`app_window`]): con Pixel UI, [`pixel_buttons`] (los del
/// skin, del mismo ancho); sin él, los de egui en fila. Devuelve cuál se tocó.
pub fn dialog_buttons(ui: &mut Ui, skin: Option<&Skin>, labels: &[&str]) -> Option<usize> {
    match skin {
        Some(skin) => pixel_buttons(ui, skin, labels)
            .iter()
            .position(|r| r.clicked()),
        None => {
            ui.horizontal(|ui| {
                let mut clicked = None;
                for (i, label) in labels.iter().enumerate() {
                    if ui.button(*label).clicked() {
                        clicked = Some(i);
                    }
                }
                clicked
            })
            .inner
        }
    }
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
mod tests;
