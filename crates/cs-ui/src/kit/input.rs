//! Los campos de texto (paso 3 del UI kit): el campo de una línea, el numérico y sus cajas de
//! prefijo y de unidad, con las medidas del mockup de componentes (frame 9).
//!
//! - **Campo:** `field.text` en 9-slice (`field.text.disabled` deshabilitado), 17 de alto como el
//!   botón: el cuerpo de 16 y la línea clara de abajo. Adentro, blanco (deshabilitado `#ddcebf`) y
//!   la fila 14 `#e6e7f0` (deshabilitado `#bda499`).
//! - **Texto:** la tinta 5 px adentro (el borde y 4 de aire) y las mayúsculas en la fila 5, una
//!   más abajo que en el botón (lo decidió el diseñador): 3 filas de aire sobre el acento y los
//!   descendentes hasta la 12. Con el foco, `color.text`; sin él, `color.field_text`;
//!   deshabilitado, `color.field_text_disabled` (como el campo del mockup).
//! - **Selección:** un bloque `color.text_selection` de las filas 3 a 13, desde 2 px antes de la
//!   tinta elegida hasta el final de su último glifo (con su columna de aire), y el texto elegido
//!   en `color.text_selected`. El cursor de escribir, 1 px de ancho en esas filas.
//! - **Prefijo y unidad:** una caja `#bda499` adentro del borde, a la izquierda (`field.prefix`,
//!   la "X" de Position X) o a la derecha (`field.unit`, el "px"), de 3 + tinta + 3 de ancho (por
//!   lo menos 11, el de la "X"), con su fila 14 `#ddcebf` y el texto centrado, en
//!   `color.field_text`, en la fila de las mayúsculas.
//! - **Número:** el mismo campo con un `DragValue` de egui sin marco adentro: se arrastra en
//!   todo el campo o, con un clic, se escribe.

use std::ops::RangeInclusive;
use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, Galley, Id, Layout, Painter, Rect, Response, Sense, Shape, Stroke,
    TextStyle, Ui, UiBuilder, Vec2, WidgetInfo, WidgetType, emath, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::pixel_ui;
use crate::skin::{Skin, SkinColors, WidgetState};
use crate::tab_bar;

/// El alto del campo: el cuerpo de 16 y la línea clara de abajo, como el botón.
pub const HEIGHT: f32 = 17.0;
/// Del borde izquierdo a la tinta: el borde y 4 px de aire.
pub const TEXT_LEFT: f32 = 5.0;
/// Las mayúsculas, en la fila 5 del campo.
pub const CAPS_TOP: f32 = 5.0;
/// El bloque de la selección (y el cursor de escribir): de la fila 3, 11 filas.
pub const SELECTION_TOP: f32 = 3.0;
pub const SELECTION_ROWS: f32 = 11.0;
/// La selección empieza 2 px antes de la tinta elegida.
pub const SELECTION_LEFT: f32 = 2.0;
/// La caja del prefijo o de la unidad: por lo menos 3 px de aire a cada lado de su tinta (el "px"
/// del mockup, en 16) y por lo menos 11 de ancho (la "X" del mockup), para que las cajas de una
/// letra (X, Y, W, H) midan lo mismo. La tinta va centrada (el píxel de sobra, a la derecha).
pub const BOX_PAD: f32 = 3.0;
pub const BOX_MIN: f32 = 11.0;
/// El borde del campo, a cada lado.
pub const OUTLINE: f32 = 1.0;

/// Las piezas del skin.
pub const FIELD: &str = "field.text";
pub const UNIT: &str = "field.unit";
pub const PREFIX: &str = "field.prefix";

/// Del segundo, la parte en que se ve el cursor de escribir.
const CARET_ON: f64 = 0.5;

/// Los estados de un campo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputState {
    Normal,
    Focused,
    Disabled,
}

impl InputState {
    /// El nombre del estado (en el catálogo).
    pub fn name(self) -> &'static str {
        match self {
            InputState::Normal => "Normal",
            InputState::Focused => "Focused",
            InputState::Disabled => "Disabled",
        }
    }

    fn of(enabled: bool, focused: bool) -> Self {
        if !enabled {
            InputState::Disabled
        } else if focused {
            InputState::Focused
        } else {
            InputState::Normal
        }
    }

    fn piece(self) -> WidgetState {
        match self {
            InputState::Disabled => WidgetState::Disabled,
            _ => WidgetState::Normal,
        }
    }
}

/// Las cajas de un campo: el prefijo a la izquierda (la "X" de Position X) y la unidad a la
/// derecha (el "px").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Affixes<'a> {
    pub prefix: Option<&'a str>,
    pub suffix: Option<&'a str>,
}

impl<'a> Affixes<'a> {
    pub const NONE: Affixes<'static> = Affixes {
        prefix: None,
        suffix: None,
    };

    pub fn prefix(prefix: &'a str) -> Self {
        Affixes {
            prefix: Some(prefix),
            suffix: None,
        }
    }

    pub fn suffix(suffix: &'a str) -> Self {
        Affixes {
            prefix: None,
            suffix: Some(suffix),
        }
    }
}

/// El color del texto escrito en `state`.
pub fn text_color(colors: &SkinColors, state: InputState) -> Color32 {
    match state {
        InputState::Normal => colors.field_text,
        InputState::Focused => colors.text,
        InputState::Disabled => colors.field_text_disabled,
    }
}

/// El color del texto de las cajas (prefijo y unidad): `color.field_text`; deshabilitado, el de
/// los botones deshabilitados (el de los campos es el de la caja).
pub fn affix_color(colors: &SkinColors, state: InputState) -> Color32 {
    match state {
        InputState::Disabled => colors.button_text_disabled,
        _ => colors.field_text,
    }
}

fn galley(ui: &Ui, text: &str, color: Color32) -> Arc<Galley> {
    let font = TextStyle::Body.resolve(ui.style());
    ui.painter().layout_no_wrap(text.to_owned(), font, color)
}

/// El ancho de la tinta de `text`.
pub fn ink_width(ui: &Ui, text: &str) -> f32 {
    crate::menus::ink_width(&galley(ui, text, Color32::PLACEHOLDER))
}

/// El ancho de la caja de un prefijo o una unidad: `3 + tinta + 3`, por lo menos [`BOX_MIN`].
pub fn box_width(ui: &Ui, text: &str) -> f32 {
    (2.0 * BOX_PAD + ink_width(ui, text)).max(BOX_MIN)
}

/// La parte del campo `rect` donde se escribe: sin el borde ni las cajas.
pub fn text_area(ui: &Ui, rect: Rect, affixes: Affixes) -> Rect {
    let left = affixes.prefix.map_or(0.0, |p| box_width(ui, p));
    let right = affixes.suffix.map_or(0.0, |s| box_width(ui, s));
    Rect::from_min_max(
        pos2(rect.left() + OUTLINE + left, rect.top()),
        pos2(rect.right() - OUTLINE - right, rect.bottom()),
    )
}

/// Las filas del bloque de la selección y del cursor de escribir de un campo que empieza en `top`.
pub fn selection_rows(top: f32) -> egui::Rangef {
    egui::Rangef::new(top + SELECTION_TOP, top + SELECTION_TOP + SELECTION_ROWS)
}

/// El campo en `rect` sin lo escrito: la pieza del estado y las cajas con su texto. Sin las
/// piezas, un blanco con borde de 1 px.
pub fn frame_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    state: InputState,
    affixes: Affixes,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = skin.colors();
    let mut shapes = match skin.piece(FIELD, state.piece()) {
        Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
        None => vec![
            Shape::rect_filled(rect, 0.0, Color32::WHITE),
            Shape::rect_stroke(
                rect.shrink(0.5),
                0.0,
                (1.0, colors.outline),
                egui::StrokeKind::Middle,
            ),
        ],
    };
    let mut boxed = |text: &str, outer: Rect, inside: egui::Rangef, name: &str| {
        shapes.extend(match skin.piece(name, WidgetState::Normal) {
            Some(piece) => pixel_ui::piece_shapes(piece, outer, ppp),
            None => vec![Shape::rect_filled(
                outer.shrink(1.0).round_to_pixels(ppp),
                0.0,
                colors.canvas,
            )],
        });
        let color = affix_color(colors, state);
        let shown = galley(ui, text, color);
        let ink = crate::menus::ink_width(&shown);
        let ink_left = inside.min + ((inside.span() - ink) / 2.0).floor();
        let at = tab_bar::text_pos(&shown, ink_left, rect.top() + CAPS_TOP, ppp);
        shapes.push(Shape::galley(at, shown, color));
    };
    // Las piezas de las cajas traen el borde del campo de su lado.
    if let Some(prefix) = affixes.prefix {
        let w = box_width(ui, prefix);
        let outer = Rect::from_min_size(rect.min, vec2(OUTLINE + w, HEIGHT));
        let inside = egui::Rangef::new(outer.left() + OUTLINE, outer.right());
        boxed(prefix, outer, inside, PREFIX);
    }
    if let Some(suffix) = affixes.suffix {
        let w = box_width(ui, suffix);
        let outer = Rect::from_min_max(pos2(rect.right() - OUTLINE - w, rect.top()), rect.max);
        let inside = egui::Rangef::new(outer.left(), outer.right() - OUTLINE);
        boxed(suffix, outer, inside, UNIT);
    }
    shapes
}

/// El bloque de la selección entre las columnas `from` y `to` (de los cursores de los extremos)
/// de un campo que empieza en `top`.
pub fn selection_block(colors: &SkinColors, from: f32, to: f32, top: f32) -> Shape {
    let rect = Rect::from_x_y_ranges((from - SELECTION_LEFT)..=to, selection_rows(top));
    Shape::rect_filled(rect, 0.0, colors.text_selection)
}

/// Un campo dibujado en `state` con `text` y, si hay, los caracteres `selected` elegidos (sin
/// interacción: el catálogo los muestra así). El texto vacío muestra `hint`.
#[allow(clippy::too_many_arguments)]
pub fn input_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    text: &str,
    hint: &str,
    state: InputState,
    selected: Option<std::ops::Range<usize>>,
    affixes: Affixes,
) -> Vec<Shape> {
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes = frame_shapes(ui, skin, rect, state, affixes);
    let area = text_area(ui, rect, affixes);
    let left = area.left() + TEXT_LEFT - OUTLINE;
    let font = TextStyle::Body.resolve(ui.style());
    let (shown, color) = if text.is_empty() {
        (hint, colors.text_dim)
    } else {
        (text, text_color(&colors, state))
    };
    let mut job = egui::text::LayoutJob::default();
    let chars: Vec<char> = shown.chars().collect();
    let range = selected.unwrap_or(0..0);
    let parts = [
        (0..range.start.min(chars.len()), color),
        (
            range.start.min(chars.len())..range.end.min(chars.len()),
            colors.text_selected,
        ),
        (range.end.min(chars.len())..chars.len(), color),
    ];
    for (part, c) in parts {
        let s: String = chars[part].iter().collect();
        job.append(&s, 0.0, egui::TextFormat::simple(font.clone(), c));
    }
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let at = tab_bar::text_pos(&galley, left, rect.top() + CAPS_TOP, ppp);
    if !range.is_empty() {
        let x = |i: usize| {
            at.x + galley
                .pos_from_cursor(egui::text::CCursor::new(i.min(chars.len())))
                .min
                .x
        };
        shapes.push(selection_block(
            &colors,
            x(range.start),
            x(range.end),
            rect.top(),
        ));
    }
    shapes.push(Shape::galley(at, galley, color));
    shapes
}

/// Un campo de texto de una línea de `width` (con `hint` vacío). Su nombre accesible es `label`.
pub fn text_input(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    width: f32,
    label: &str,
    hint: &str,
) -> Response {
    text_input_with(ui, skin, text, width, label, hint, Affixes::NONE)
}

/// [`text_input`] con prefijo o unidad.
pub fn text_input_with(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    width: f32,
    label: &str,
    hint: &str,
    affixes: Affixes,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.floor(), HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let painter = ui.painter().clone();
    field(ui, skin, &painter, rect, text, label, hint, affixes, true)
}

/// Solo lo escrito (sin la caja, que la dibuja quien lo usa) en `size`, con las medidas del campo
/// (la tinta 5 px adentro, las mayúsculas en la fila 5): la selección y el cursor, como en los
/// campos.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn bare_text_input(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    size: Vec2,
    label: &str,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(size.x.floor(), size.y), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let painter = ui.painter().clone();
    field(
        ui,
        skin,
        &painter,
        rect,
        text,
        label,
        "",
        Affixes::NONE,
        false,
    )
}

/// El campo en `rect`: la caja (si `framed`), un `TextEdit` de egui sin marco en las filas del
/// texto, la selección y el cursor pintados en las filas del diseño.
#[allow(clippy::too_many_arguments)]
fn field(
    ui: &mut Ui,
    skin: &Skin,
    painter: &Painter,
    rect: Rect,
    text: &mut String,
    label: &str,
    hint: &str,
    affixes: Affixes,
    framed: bool,
) -> Response {
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let enabled = ui.is_enabled();
    // Deshabilitado se ve entero (sin la transparencia de egui): cambian la pieza y el texto.
    let mut painter = painter.clone();
    painter.set_opacity(1.0);
    let frame = painter.add(Shape::Noop);
    // La selección va entre el campo y el texto: se pinta después, cuando se sabe qué hay.
    let selection = painter.add(Shape::Noop);
    let area = text_area(ui, rect, affixes);
    let probe = galley(ui, "H", colors.text);
    let top = tab_bar::text_pos(&probe, 0.0, rect.top() + CAPS_TOP, ppp).y;
    let inner = Rect::from_min_max(
        pos2(area.left() + TEXT_LEFT - OUTLINE, top),
        pos2(area.right() - TEXT_LEFT + OUTLINE, top + probe.size().y),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(("kit_input", label))
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_opacity(1.0);
    // El id de siempre del `TextEdit` (el siguiente de su ui): distinto en cada lugar.
    let id = child.next_auto_id();
    let state = InputState::of(enabled, ui.memory(|m| m.has_focus(id)));
    plain_field_style(
        child.style_mut(),
        inner.size(),
        text_color(&colors, state),
        colors.outline,
        &colors,
    );
    // egui pinta el fondo de la selección y el cursor del alto de la fila: los pintamos nosotros.
    child.style_mut().visuals.selection.bg_fill = Color32::TRANSPARENT;
    child.style_mut().visuals.text_cursor.stroke = Stroke::NONE;
    let output = egui::TextEdit::singleline(text)
        .id(id)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .min_size(inner.size())
        .hint_text(egui::RichText::new(hint).color(colors.text_dim))
        .desired_width(inner.width())
        .show(&mut child);
    let response = output.response.response;
    let state = InputState::of(enabled, response.has_focus());
    if framed {
        painter.set(
            frame,
            Shape::Vec(frame_shapes(ui, skin, rect, state, affixes)),
        );
    }
    let x = |c| output.galley_pos.x + output.galley.pos_from_cursor(c).min.x;
    let rows = selection_rows(rect.top());
    let clip = Rect::from_x_y_ranges(area.x_range(), rows);
    match output.cursor_range {
        Some(range) if !range.is_empty() => {
            let [a, b] = range.sorted_cursors();
            let block = selection_block(&colors, x(a).round(), x(b).round(), rect.top());
            if let Shape::Rect(r) = &block {
                let span = r.rect.intersect(clip);
                if span.width() > 0.0 {
                    painter.set(
                        selection,
                        Shape::rect_filled(span, 0.0, colors.text_selection),
                    );
                }
            }
        }
        // El cursor de escribir: 1 px de ancho, en las filas de la selección, y parpadeando
        // (medio segundo prendido y medio apagado).
        Some(range) if response.has_focus() => {
            let time = ui.input(|i| i.time);
            if time.fract() < CARET_ON {
                let left = x(range.primary).round();
                let caret = Rect::from_x_y_ranges(left..=left + 1.0, rows).intersect(clip);
                painter.set(selection, Shape::rect_filled(caret, 0.0, colors.outline));
            }
            let wait = if time.fract() < CARET_ON {
                CARET_ON - time.fract()
            } else {
                1.0 - time.fract()
            };
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(wait.max(0.01)));
        }
        _ => {}
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, enabled, label));
    response
}

/// Un campo de una línea que edita una copia de `current`: lo escrito se guarda (con `id`, en la
/// memoria de egui) mientras tiene el foco y se devuelve al terminar (Enter o al salir) si
/// cambió; Escape lo descarta. `edit` dibuja el campo.
pub fn buffered(
    ui: &mut Ui,
    id: Id,
    current: &str,
    edit: impl FnOnce(&mut Ui, &mut String) -> Response,
) -> Option<String> {
    let mut text = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| current.to_owned());
    let response = edit(ui, &mut text);
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
    (response.lost_focus() && !cancelled && text != current).then_some(text)
}

/// Cómo va el número en un campo numérico.
#[derive(Clone, Copy, Debug)]
pub struct NumberOptions<'a> {
    pub affixes: Affixes<'a>,
    /// A la izquierda (como el texto) o centrado en el blanco.
    pub align: Align,
    /// Cuánto cambia por punto arrastrado; sin él, según el rango.
    pub speed: Option<f64>,
}

impl Default for NumberOptions<'_> {
    fn default() -> Self {
        NumberOptions {
            affixes: Affixes::NONE,
            align: Align::Min,
            speed: None,
        }
    }
}

/// Un campo numérico en `rect` (de [`HEIGHT`] de alto) dibujado con `painter`: el campo y sus
/// cajas, y adentro un `DragValue` de egui sin marco (se arrastra en todo el campo o, con un
/// clic, se escribe). Devuelve la respuesta del `DragValue`.
#[allow(clippy::too_many_arguments)]
pub fn number_input_at<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    painter: &Painter,
    rect: Rect,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    options: NumberOptions,
) -> Response {
    let colors = *skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let enabled = ui.is_enabled();
    // Deshabilitado se ve entero (sin la transparencia de egui).
    let mut painter = painter.clone();
    painter.set_opacity(1.0);
    let frame = painter.add(Shape::Noop);
    let area = text_area(ui, rect, options.affixes);
    let probe = galley(ui, "0", colors.text);
    let top = tab_bar::text_pos(&probe, 0.0, rect.top() + CAPS_TOP, ppp).y;
    let pad = TEXT_LEFT - OUTLINE;
    let inner = Rect::from_min_max(
        pos2(area.left() + pad, top),
        pos2(area.right() - pad, top + probe.size().y),
    );
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    let speed = options
        .speed
        .unwrap_or(((end - start) / 128.0).clamp(0.1, 1.0));
    let look = NumberLook {
        focused_text: Some(colors.text),
        rows: Some(selection_rows(rect.top())),
        ..NumberLook::new(
            &colors,
            text_color(&colors, InputState::of(enabled, false)),
            options.align,
        )
    };
    let (response, _) = drag_number(ui, rect, inner, id_salt, value, range, speed, look);
    let state = InputState::of(enabled, response.has_focus());
    painter.set(
        frame,
        Shape::Vec(frame_shapes(ui, skin, rect, state, options.affixes)),
    );
    response
}

/// [`number_input_at`] de `width`, en el lugar que sigue de `ui`.
pub fn number_input<Num: emath::Numeric>(
    ui: &mut Ui,
    skin: &Skin,
    width: f32,
    id_salt: &str,
    value: &mut Num,
    range: RangeInclusive<Num>,
    options: NumberOptions,
) -> Response {
    let (slot, _) = ui.allocate_exact_size(vec2(width.floor(), HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let painter = ui.painter().clone();
    number_input_at(ui, skin, &painter, rect, id_salt, value, range, options)
}

/// Cómo se ve el texto de [`drag_number`]: su color (y con el foco, si cambia), el del cursor de
/// escribir, los de lo seleccionado, hacia dónde se alinea en su rect y las filas de la selección
/// (sin ellas, las de [`crate::pixel_sections::text_rows`]).
#[derive(Clone, Copy)]
pub(crate) struct NumberLook {
    pub text: Color32,
    pub focused_text: Option<Color32>,
    pub outline: Color32,
    pub selection: Color32,
    pub selected: Color32,
    pub align: Align,
    pub rows: Option<egui::Rangef>,
}

impl NumberLook {
    /// El texto en `text` alineado según `align`, con el cursor y la selección del skin.
    pub fn new(colors: &SkinColors, text: Color32, align: Align) -> Self {
        NumberLook {
            text,
            focused_text: None,
            outline: colors.outline,
            selection: colors.text_selection,
            selected: colors.text_selected,
            align,
            rows: None,
        }
    }
}

/// Un número que se arrastra en toda la zona `whole` y se escribe con el `DragValue` de egui sin
/// marco que va en `inner` (el texto, alineado según `look`). Un clic en la zona entra a
/// escribir. Lo usan los campos numéricos, la tira de canales del editor de color y el timeline.
/// Devuelve la respuesta del `DragValue` y la de la zona.
#[allow(clippy::too_many_arguments)]
pub(crate) fn drag_number<Num: emath::Numeric>(
    ui: &mut Ui,
    whole: Rect,
    inner: Rect,
    id_salt: impl std::hash::Hash + std::fmt::Debug + Copy,
    value: &mut Num,
    range: RangeInclusive<Num>,
    speed: f64,
    look: NumberLook,
) -> (Response, Response) {
    let (start, end) = (range.start().to_f64(), range.end().to_f64());
    // La zona va debajo del `DragValue`, que gana donde se solapan.
    let zone = ui.interact(
        whole,
        ui.id().with(("bar_number_zone", id_salt)),
        Sense::click_and_drag(),
    );
    if zone.hovered() || zone.dragged() {
        ui.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if zone.drag_started() {
        ui.data_mut(|d| d.insert_temp(zone.id, value.to_f64()));
    }
    if zone.dragged() {
        // Se acumula en `f64`: con enteros y velocidad baja, cada paso solo no llegaría a 1.
        let mut acc = ui
            .data(|d| d.get_temp::<f64>(zone.id))
            .unwrap_or_else(|| value.to_f64());
        acc = (acc + f64::from(zone.drag_delta().x) * speed).clamp(start, end);
        ui.data_mut(|d| d.insert_temp(zone.id, acc));
        *value = Num::from_f64(acc);
    }
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(("bar_number", id_salt))
            .max_rect(inner)
            .layout(Layout::top_down(look.align)),
    );
    let id = child.next_auto_id();
    let focused = ui.memory(|m| m.has_focus(id));
    let text = match look.focused_text {
        Some(color) if focused => color,
        _ => look.text,
    };
    plain_field_style_colors(child.style_mut(), inner.size(), text, look.outline);
    let v = &mut child.style_mut().visuals;
    v.selection.bg_fill = look.selection;
    v.selection.stroke = Stroke::new(1.0, look.selected);
    // Al escribir, la selección y el cursor de egui en las filas de los demás campos.
    let rows = look
        .rows
        .unwrap_or_else(|| crate::pixel_sections::text_rows(whole));
    let clip = Rect::from_x_y_ranges(inner.x_range().expand(1.0), rows).intersect(ui.clip_rect());
    child.set_clip_rect(clip);
    let response = child.add(egui::DragValue::new(value).range(range).speed(speed));
    // Un clic en la zona de afuera del texto también entra a escribir.
    if zone.clicked() {
        ui.memory_mut(|m| m.request_focus(response.id));
    }
    (response, zone)
}

/// El estilo del `TextEdit` o `DragValue` de un campo: del tamaño de su texto, sin marco ni fondo
/// (los pone la pieza), con el texto en `text`, lo seleccionado en los colores de selección del
/// skin, el cursor en `outline` y sin el borde de foco de egui.
pub(crate) fn plain_field_style(
    style: &mut egui::Style,
    size: Vec2,
    text: Color32,
    outline: Color32,
    colors: &SkinColors,
) {
    plain_field_style_colors(style, size, text, outline);
    let v = &mut style.visuals;
    v.selection.bg_fill = colors.text_selection;
    v.selection.stroke = Stroke::new(1.0, colors.text_selected);
}

/// [`plain_field_style`] sin los colores de la selección.
fn plain_field_style_colors(style: &mut egui::Style, size: Vec2, text: Color32, outline: Color32) {
    style.spacing.interact_size = size;
    style.spacing.button_padding = Vec2::ZERO;
    let v = &mut style.visuals;
    v.button_frame = false;
    v.text_edit_bg_color = Some(Color32::TRANSPARENT);
    // El cursor de escribir, fino y en el contorno del skin: el de egui es azul claro de 2 px y
    // sobre el blanco del campo casi no se ve.
    v.text_cursor.stroke = Stroke::new(1.0, outline);
    v.override_text_color = Some(text);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = Color32::TRANSPARENT;
        w.weak_bg_fill = Color32::TRANSPARENT;
        w.bg_stroke = Stroke::NONE;
        w.expansion = 0.0;
    }
}

#[cfg(test)]
mod tests;
