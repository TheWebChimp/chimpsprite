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
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), size);
    let ppp = ui.ctx().pixels_per_point();
    let info = crate::pixel_draw::ButtonInfo {
        selected: on,
        ..crate::pixel_draw::ButtonInfo::new(ui.id().with(("icon_button", label)), label)
    };
    crate::pixel_draw::skin_button(ui, skin, rect, info, |inner| {
        // Centrado en el relleno (sin las filas de abajo del botón).
        let fill = Rect::from_min_max(inner.min, inner.max - vec2(0.0, 2.0));
        crate::pixel_draw::piece_centered(skin, icon, fill, ppp)
    })
    .tip(label)
}

/// El botón con ícono: 25×16, como los de la barra.
const ICON_BUTTON: Vec2 = vec2(25.0, 16.0);

/// Un botón que se prende y se apaga con su LED: el `button` de siempre (no se hunde) con el ícono
/// `icon` y, a su derecha, el punto de los grupos de opciones (`button.dot.on` prendido,
/// `button.dot.off` apagado). `label` es su nombre y su tooltip. Sin Pixel UI, una opción de egui.
pub fn led_button(ui: &mut Ui, skin: Option<&Skin>, icon: &str, label: &str, on: bool) -> Response {
    let skin = skin.filter(|s| s.piece(icon, WidgetState::Normal).is_some());
    let Some(skin) = skin else {
        return ui.selectable_label(on, label);
    };
    let ppp = ui.ctx().pixels_per_point();
    let icon_size = crate::pixel_draw::piece_size(skin, icon);
    let led = if on { LED_ON } else { LED_OFF };
    let led_size = crate::pixel_draw::piece_size(skin, led);
    let width = LED_PAD + icon_size.x + LED_GAP + led_size.x + LED_PAD;
    let size = vec2(width, ICON_BUTTON.y);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), size);
    // Prendido o apagado lo dice el LED: el botón no queda hundido.
    let info = crate::pixel_draw::ButtonInfo {
        selected: None,
        ..crate::pixel_draw::ButtonInfo::new(ui.id().with(("led_button", label)), label)
    };
    let response = crate::pixel_draw::skin_button(ui, skin, rect, info, |inner| {
        // En el relleno (sin las filas de abajo del botón), centrados a lo alto.
        let fill = Rect::from_min_max(inner.min, inner.max - vec2(0.0, 2.0));
        // Si no cabe justo a la mitad, el pixel de sobra queda arriba (como en el diseño).
        let middle = |h: f32| (fill.center().y - h / 2.0).ceil();
        let icon_at = pos2(rect.left() + LED_PAD, middle(icon_size.y));
        let led_at = pos2(icon_at.x + icon_size.x + LED_GAP, middle(led_size.y));
        let mut shapes = crate::pixel_draw::piece_at(skin, icon, icon_at, ppp);
        shapes.extend(crate::pixel_draw::piece_at(skin, led, led_at, ppp));
        shapes
    });
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, label)
    });
    response.tip(label)
}

/// El LED de un [`led_button`] (el de los grupos de opciones), del borde a lo de adentro y del
/// ícono al LED.
const LED_ON: &str = "button.dot.on";
const LED_OFF: &str = "button.dot.off";
const LED_PAD: f32 = 4.0;
const LED_GAP: f32 = 2.0;

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
        let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
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
mod tests {
    #[test]
    fn the_scroll_thumb_has_1_px_of_padding_on_every_side() {
        let track = Rect::from_min_size(pos2(100.0, 20.0), vec2(SCROLL_BAR, 200.0));
        // Arriba de todo y abajo de todo: a 1 px de las puntas y de los lados.
        let top = super::scroll_thumb(track, true, 1000.0, 200.0, 0.0);
        assert_eq!(top.min, pos2(101.0, 21.0));
        assert_eq!(top.width(), SCROLL_BAR - 2.0);
        let bottom = super::scroll_thumb(track, true, 1000.0, 200.0, 800.0);
        assert_eq!(bottom.bottom(), track.bottom() - 1.0);
        // A lo ancho, igual.
        let track = Rect::from_min_size(pos2(0.0, 50.0), vec2(300.0, SCROLL_BAR));
        let right = super::scroll_thumb(track, false, 900.0, 300.0, 600.0);
        assert_eq!((right.right(), right.top()), (299.0, 51.0));
    }

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
        assert_eq!(short.size(), vec2(49.0, 16.0), "the piece size");
        let (long, _) = button(&ctx, &skin, "A much longer button label", vec![]);
        assert!(long.width() > 49.0, "{long:?}");
        assert_eq!(long.height(), 16.0);
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
                pixel_dialog_at(
                    ui.ctx(),
                    &skin,
                    id,
                    "Unsaved changes",
                    false,
                    (Align2::CENTER_CENTER, Vec2::ZERO),
                    |ui| {
                        content = ui.max_rect();
                        ui.label("Save changes before closing?");
                    },
                );
            });
        }
        let frame = ctx
            .memory(|m| m.area_rect(id))
            .expect("the dialog is shown");
        // `window`: bordes de 3 px y barra de título de 14, más 4 px de margen.
        assert_eq!(content.min - frame.min, vec2(7.0, 18.0));
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
    fn with_pixel_ui_scroll_areas_draw_the_design_bars() {
        let ctx = egui::Context::default();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let thumb = skin
            .piece(SCROLL_THUMB_V, WidgetState::Normal)
            .unwrap()
            .texture
            .id();
        let track = skin
            .piece(SCROLL_TRACK_V, WidgetState::Normal)
            .unwrap()
            .texture
            .id();
        // Las mallas con `id` dibujadas en un frame con el contenido más alto que el área.
        let drawn = |pixel: bool| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                set_bubble(ui.ctx(), pixel.then(|| std::sync::Arc::new(skin.clone())));
                scroll(ui, egui::ScrollArea::vertical().max_height(100.0), |ui| {
                    ui.allocate_space(vec2(50.0, 400.0));
                });
            });
            output.textures_delta.clear();
            let mut found = Vec::new();
            for clipped in &output.shapes {
                for (id, rect) in meshes(&clipped.shape) {
                    if id == thumb || id == track {
                        found.push((id, rect));
                    }
                }
            }
            found
        };
        let found = drawn(true);
        assert!(found.iter().any(|(id, _)| *id == track), "the track");
        let thumb_rect = found
            .iter()
            .filter(|(id, _)| *id == thumb)
            .fold(Rect::NOTHING, |b, (_, r)| b.union(*r));
        // El pulgar: 8 de ancho y, con 100 de 400 a la vista, el largo mínimo (46).
        assert_eq!(thumb_rect.width(), 8.0, "{thumb_rect:?}");
        assert_eq!(thumb_rect.height(), SCROLL_THUMB_MIN, "{thumb_rect:?}");
        assert!(drawn(false).is_empty(), "without Pixel UI, egui's bar");
    }

    /// Las mallas (con su textura) de una forma, aplanada.
    fn meshes(shape: &Shape) -> Vec<(egui::TextureId, Rect)> {
        match shape {
            Shape::Vec(shapes) => shapes.iter().flat_map(meshes).collect(),
            Shape::Mesh(m) => vec![(m.texture_id, m.calc_bounds())],
            _ => Vec::new(),
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
            duration_ms: None,
            rgba: window.rgba.clone(),
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
                    pixel_dialog_at(
                        ui.ctx(),
                        skin,
                        id,
                        "Unsaved changes",
                        true,
                        (Align2::CENTER_CENTER, Vec2::ZERO),
                        |ui| {
                            content = ui.max_rect();
                            ui.label("A long question that is wider than the buttons, surely?");
                            rects = pixel_buttons(ui, skin, &["Save", "Discard", "Cancel"])
                                .iter()
                                .map(|r| r.rect)
                                .collect();
                        },
                    );
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
        assert_eq!(rect.size(), vec2(15.0, 16.0), "the piece size");
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
            assert_eq!(rect.size(), vec2(15.0, 16.0));
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

    #[test]
    fn wrap_rows_keeps_each_group_whole_and_moves_what_does_not_fit_to_the_next_row() {
        // Como el encabezado del editor de color: copiar (25), RGBA (116) y hex (46), con 2 de
        // aire entre los dos primeros y 4 menos al juntar RGBA con el hex (comparten la punta).
        let widths = [25.0, 116.0, 46.0];
        let joins = [2.0, -4.0];
        // Todo junto mide 25 + 2 + 116 - 4 + 46 = 185.
        assert_eq!(wrap_rows(185.0, &widths, &joins), vec![0..3]);
        assert_eq!(wrap_rows(400.0, &widths, &joins), vec![0..3]);
        assert_eq!(wrap_rows(184.0, &widths, &joins), vec![0..2, 2..3]);
        assert_eq!(wrap_rows(143.0, &widths, &joins), vec![0..2, 2..3]);
        assert_eq!(wrap_rows(142.0, &widths, &joins), vec![0..1, 1..2, 2..3]);
        // Más angosto que un grupo: igual va entero, solo en su fila.
        assert_eq!(wrap_rows(50.0, &widths, &joins), vec![0..1, 1..2, 2..3]);
        assert_eq!(wrap_rows(10.0, &[25.0], &[]), vec![0..1]);
        assert!(wrap_rows(10.0, &[], &[]).is_empty());
    }
}
