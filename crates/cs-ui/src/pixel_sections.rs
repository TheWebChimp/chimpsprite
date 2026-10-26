//! Con Pixel UI, las secciones de abajo del panel derecho como en el diseño (frame 4): el título
//! con sus botones chicos (+ y tacho) a la derecha, los elementos como pestañas del skin (`tab`,
//! `tab.active`) sobre la línea grabada y, debajo, los campos de lo elegido.

use eframe::egui::{
    self, Align, Color32, Layout, Rect, Response, Sense, Shape, TextStyle, Ui, UiBuilder, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::bar_fields;
use crate::pixel_draw;
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};

/// Los botones chicos del diseño: `button` del skin en 9-slice, de 25×16.
pub const BUTTON_SIZE: Vec2 = vec2(25.0, 16.0);
/// Entre los botones del título (x 943 → 946 en el diseño).
const BUTTON_GAP: f32 = 2.0;
/// Del borde derecho del panel a los botones del título; los de los campos llegan al borde.
const HEADER_RIGHT: f32 = 3.0;
/// Fila de las mayúsculas del título, desde arriba de los botones.
const HEADER_CAPS_TOP: f32 = 4.0;
/// De los botones del título a las pestañas.
const HEADER_TABS_GAP: f32 = 1.0;
const TAB_PIECE: &str = "tab";
const TAB_HEIGHT: f32 = 15.0;
/// Entre pestañas y entre filas de pestañas.
const TAB_GAP: f32 = 5.0;
const ROW_GAP: f32 = 2.0;
/// Del borde de la pestaña al texto (y al cuadrito de color, si hay).
const TAB_PADDING: f32 = 5.0;
const TAB_CAPS_TOP: f32 = 5.0;
/// El cuadrito de color de una pestaña (slices) y su aire hasta el nombre.
const SWATCH: f32 = 7.0;
const SWATCH_GAP: f32 = 3.0;
/// De la línea grabada de las pestañas a los campos.
const TABS_FIELDS_GAP: f32 = 9.0;
/// Los campos de texto del diseño: 16 de alto con la fila del brillo de abajo (como los combos,
/// los campos numéricos y los botones).
pub const FIELD_HEIGHT: f32 = 16.0;
/// Entre un campo y su botón (x 945 → 949).
const FIELD_BUTTON_GAP: f32 = 3.0;
/// Del borde del campo al texto.
const FIELD_TEXT_LEFT: f32 = 5.0;
/// Entre filas de campos.
const FIELD_ROW_GAP: f32 = 3.0;

/// Las piezas del skin: los íconos de los botones y el campo de texto (en 9-slice).
pub const ICON_ADD: &str = "icon.section.add";
pub const ICON_TRASH: &str = "icon.section.trash";
pub const ICON_CANVAS_SIZE: &str = "icon.section.canvas_size";
const FIELD_PIECE: &str = "field.text";

/// Un botón chico en `rect` (de `BUTTON_SIZE`) con `content` adentro. Su nombre accesible es
/// `label`, que es también su tooltip.
fn button_at(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    label: &str,
    enabled: bool,
    content: impl FnOnce(Rect) -> Vec<Shape>,
) -> Response {
    let info = pixel_draw::ButtonInfo {
        enabled,
        ..pixel_draw::ButtonInfo::new(ui.id().with(("pixel_button", label)), label)
    };
    pixel_draw::skin_button(ui, skin, rect, info, content).tip(label)
}

/// La pieza `icon` centrada en `rect`; desactivada, apagada.
fn centered(skin: &Skin, icon: &str, rect: Rect, enabled: bool, ppp: f32) -> Vec<Shape> {
    let shapes = pixel_draw::piece_centered(skin, icon, rect, ppp);
    if enabled {
        shapes
    } else {
        pixel_draw::dimmed(shapes)
    }
}

/// Un botón chico con un ícono, en el lugar que sigue de `ui`.
pub fn icon_button(ui: &mut Ui, skin: &Skin, icon: &str, label: &str, enabled: bool) -> Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let ppp = ui.ctx().pixels_per_point();
    button_at(ui, skin, rect, label, enabled, |r| {
        centered(skin, icon, r, enabled, ppp)
    })
}

/// Un botón chico que muestra `color` (un cuadrito con contorno), en el lugar que sigue de `ui`.
pub fn color_button(ui: &mut Ui, skin: &Skin, color: Color32, label: &str) -> Response {
    let (rect, _) = ui.allocate_exact_size(BUTTON_SIZE, Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), BUTTON_SIZE);
    let outline = skin.colors().field_text;
    button_at(ui, skin, rect, label, true, |r| {
        let size = vec2(11.0, 9.0);
        let swatch = Rect::from_min_size((r.center() - size / 2.0).floor(), size);
        vec![
            Shape::rect_filled(swatch, 0.0, outline),
            Shape::rect_filled(swatch.shrink(1.0), 0.0, color),
        ]
    })
}

/// Un texto con sus mayúsculas en la fila `cap_top` y su borde izquierdo en `left`.
fn text_shape(ui: &Ui, text: &str, left: f32, cap_top: f32, color: Color32) -> Shape {
    pixel_draw::caps_text(ui, text, left, cap_top, color).0
}

fn text_width(ui: &Ui, text: &str) -> f32 {
    let font = TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::WHITE)
        .size()
        .x
        .ceil()
}

/// Lo que se pidió desde el título de una sección.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderAction {
    Add,
    Delete,
}

/// El título de una sección con + y tacho a la derecha. `can_delete` habilita el tacho.
pub fn header(
    ui: &mut Ui,
    skin: &Skin,
    title: &str,
    add: &str,
    delete: &str,
    can_delete: bool,
) -> Option<HeaderAction> {
    let width = ui.available_width();
    let (row, _) = ui.allocate_exact_size(vec2(width, BUTTON_SIZE.y), Sense::hover());
    let row = Rect::from_min_size(row.min.round(), row.size());
    let color = skin.colors().text;
    let title_shape = text_shape(ui, title, row.left(), row.top() + HEADER_CAPS_TOP, color);
    ui.painter().add(title_shape);
    let right = (row.right() - HEADER_RIGHT).floor();
    let delete_rect = Rect::from_min_size(pos2(right - BUTTON_SIZE.x, row.top()), BUTTON_SIZE);
    let add_rect = delete_rect.translate(vec2(-(BUTTON_SIZE.x + BUTTON_GAP), 0.0));
    let mut action = None;
    let ppp = ui.ctx().pixels_per_point();
    if button_at(ui, skin, add_rect, add, true, |r| {
        centered(skin, ICON_ADD, r, true, ppp)
    })
    .clicked()
    {
        action = Some(HeaderAction::Add);
    }
    if button_at(ui, skin, delete_rect, delete, can_delete, |r| {
        centered(skin, ICON_TRASH, r, can_delete, ppp)
    })
    .clicked()
    {
        action = Some(HeaderAction::Delete);
    }
    ui.add_space(HEADER_TABS_GAP - ui.spacing().item_spacing.y);
    action
}

/// Una pestaña: su nombre y, si tiene, el color de su cuadrito.
pub struct Tab {
    pub name: String,
    pub color: Option<Color32>,
}

/// Las pestañas en filas (pasan a la siguiente si no entran) sobre la línea grabada, que se
/// corta debajo de la activa si está en la última fila. Devuelve la que se tocó.
pub fn tabs(ui: &mut Ui, skin: &Skin, tabs: &[Tab], active: Option<usize>) -> Option<usize> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let left = ui.cursor().left().round();
    let right = ui.max_rect().right();
    let top = ui.cursor().top().round();
    // Dónde va cada una.
    let mut rects = Vec::with_capacity(tabs.len());
    let (mut x, mut y) = (left, top);
    for tab in tabs {
        let swatch = if tab.color.is_some() {
            SWATCH + SWATCH_GAP
        } else {
            0.0
        };
        let width = (TAB_PADDING + swatch + text_width(ui, &tab.name) + TAB_PADDING)
            .min(right - left)
            .ceil();
        if x > left && x + width > right {
            x = left;
            y += TAB_HEIGHT + ROW_GAP;
        }
        rects.push(Rect::from_min_size(pos2(x, y), vec2(width, TAB_HEIGHT)));
        x += width + TAB_GAP;
    }
    let bottom = rects.last().map_or(top, |r| r.bottom());
    let etched = pixel_ui::ETCHED_SIZE;
    // La línea grabada a todo el ancho, cortada debajo de la activa (si está en la última fila).
    let active_rect = active
        .and_then(|i| rects.get(i))
        .filter(|r| r.bottom() == bottom);
    let line = egui::Rangef::new(left, right);
    let spans = match active_rect {
        Some(r) => vec![
            egui::Rangef::new(line.min, r.left()),
            egui::Rangef::new(r.right(), line.max),
        ],
        None => vec![line],
    };
    let mut shapes: Vec<Shape> = spans
        .into_iter()
        .filter(|s| s.span() > 0.0)
        .map(|s| pixel_ui::etched_hline(s, bottom, &colors, ppp))
        .collect();
    let mut clicked = None;
    // Los nombres van encima de las pestañas (se pintan después).
    let mut texts = Vec::with_capacity(tabs.len());
    for (i, (tab, rect)) in tabs.iter().zip(&rects).enumerate() {
        let is_active = active == Some(i);
        let response = ui.interact(*rect, ui.id().with(("pixel_tab", i)), Sense::click());
        let enabled = ui.is_enabled();
        response.widget_info(|| {
            WidgetInfo::selected(WidgetType::SelectableLabel, enabled, is_active, &tab.name)
        });
        if response.clicked() {
            clicked = Some(i);
        }
        let (state, drawn) = if is_active {
            (WidgetState::Active, rect.with_max_y(rect.bottom() + etched))
        } else if response.hovered() {
            (WidgetState::Hover, *rect)
        } else {
            (WidgetState::Normal, *rect)
        };
        if let Some(piece) = skin.piece(TAB_PIECE, state) {
            shapes.extend(pixel_ui::piece_shapes(piece, drawn, ppp));
        }
        let mut text_left = rect.left() + TAB_PADDING;
        if let Some(color) = tab.color {
            let swatch = Rect::from_min_size(
                pos2(text_left, rect.top() + TAB_CAPS_TOP),
                Vec2::splat(SWATCH),
            );
            shapes.push(Shape::rect_filled(swatch, 0.0, colors.field_text));
            shapes.push(Shape::rect_filled(swatch.shrink(1.0), 0.0, color));
            text_left += SWATCH + SWATCH_GAP;
        }
        let color = if is_active {
            colors.text
        } else {
            colors.tab_text_dim
        };
        // Recortado a la pestaña: un nombre muy largo no se sale.
        let text = text_shape(ui, &tab.name, text_left, rect.top() + TAB_CAPS_TOP, color);
        texts.push((rect.shrink2(vec2(1.0, 0.0)), text));
    }
    ui.painter().extend(shapes);
    for (clip, text) in texts {
        ui.painter().with_clip_rect(clip).add(text);
    }
    let used = Rect::from_x_y_ranges(left..=right, top..=bottom + etched);
    ui.allocate_rect(used, Sense::hover());
    ui.add_space(TABS_FIELDS_GAP - ui.spacing().item_spacing.y);
    clicked
}

/// Un campo de texto del diseño de `width`, con `hint` cuando está vacío. Su nombre accesible
/// es `label`.
pub fn text_field(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    width: f32,
    label: &str,
    hint: &str,
) -> Response {
    text_field_sized(ui, skin, text, vec2(width, FIELD_HEIGHT), label, hint)
}

/// Como [`text_field`], de `size` (los paneles del dock los usan de 17, como sus botones).
pub fn text_field_sized(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    size: Vec2,
    label: &str,
    hint: &str,
) -> Response {
    text_field_framed(ui, skin, text, size, label, hint, true)
}

/// Como [`text_field_sized`] pero sin la caja (la dibuja quien lo usa): solo el texto, su
/// selección y su cursor.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn text_field_bare(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    size: Vec2,
    label: &str,
) -> Response {
    text_field_framed(ui, skin, text, size, label, "", false)
}

fn text_field_framed(
    ui: &mut Ui,
    skin: &Skin,
    text: &mut String,
    size: Vec2,
    label: &str,
    hint: &str,
    framed: bool,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(vec2(size.x.floor(), size.y), Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    // La pieza `field.text` en 9-slice (sin ella, un borde de 1 px sobre blanco).
    let ppp = ui.ctx().pixels_per_point();
    if framed {
        let shapes = match skin.piece(FIELD_PIECE, WidgetState::Normal) {
            Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
            None => vec![
                Shape::rect_filled(rect, 0.0, Color32::WHITE),
                Shape::rect_stroke(
                    rect.shrink(0.5),
                    0.0,
                    (1.0, skin.colors().outline),
                    egui::StrokeKind::Middle,
                ),
            ],
        };
        ui.painter().extend(shapes);
    }
    // La selección va entre el campo y el texto: se pinta después, cuando se sabe qué hay.
    let selection = ui.painter().add(Shape::Noop);
    let colors = *skin.colors();
    let inner = Rect::from_min_max(
        pos2(rect.left() + FIELD_TEXT_LEFT, rect.top() + 1.0),
        pos2(rect.right() - FIELD_TEXT_LEFT, rect.bottom() - 4.0),
    );
    let mut child = ui.new_child(
        UiBuilder::new()
            .id_salt(("pixel_field", label))
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );
    bar_fields::plain_field_style(child.style_mut(), inner.size(), colors.text, colors.outline);
    // egui pinta el fondo de la selección y el cursor del alto de la fila: los pintamos nosotros
    // (abajo), en las filas del diseño.
    child.style_mut().visuals.selection.bg_fill = Color32::TRANSPARENT;
    child.style_mut().visuals.text_cursor.stroke = egui::Stroke::NONE;
    let output = egui::TextEdit::singleline(text)
        .frame(egui::Frame::NONE)
        .margin(egui::Margin::ZERO)
        .vertical_align(Align::Center)
        .min_size(inner.size())
        .hint_text(hint)
        .desired_width(inner.width())
        .show(&mut child);
    // Lo seleccionado, como en el diseño: rosa, con 1 px de aire contra el borde de arriba y
    // contra la sombra de abajo (el texto, blanco: `plain_field_style`).
    let x = |c| output.galley_pos.x + output.galley.pos_from_cursor(c).min.x;
    let rows = text_rows(rect);
    let clip = output.text_clip_rect.expand2(vec2(1.0, 0.0));
    match output.cursor_range {
        Some(range) if !range.is_empty() => {
            let [a, b] = range.sorted_cursors();
            let span = Rect::from_x_y_ranges(x(a).round()..=x(b).round(), rows).intersect(clip);
            if span.width() > 0.0 {
                ui.painter()
                    .set(selection, Shape::rect_filled(span, 0.0, SELECTION));
            }
        }
        // El cursor de escribir: 1 px de ancho, en las mismas filas que la selección, y
        // parpadeando (medio segundo prendido y medio apagado).
        Some(range) if output.response.response.has_focus() => {
            let time = ui.input(|i| i.time);
            if time.fract() < CARET_ON {
                let left = x(range.primary).round();
                let caret = Rect::from_x_y_ranges(left..=left + 1.0, rows).intersect(clip);
                ui.painter()
                    .set(selection, Shape::rect_filled(caret, 0.0, colors.outline));
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
    let response = output.response.response;
    let enabled = ui.is_enabled();
    response.widget_info(|| WidgetInfo::labeled(WidgetType::TextEdit, enabled, label));
    response
}

/// Lo seleccionado en un campo de texto: el rosa del diseño, de la fila 2 del campo hasta 1 arriba
/// de la sombra de abajo.
const SELECTION: Color32 = Color32::from_rgb(0xe2, 0x72, 0x85);
const SELECTION_TOP: f32 = 2.0;
const SELECTION_BOTTOM: f32 = 4.0;
/// Del segundo, la parte en que se ve el cursor de escribir.
const CARET_ON: f64 = 0.5;

/// Las filas de un campo (`field`, con su borde) donde van la selección y el cursor de escribir:
/// las mismas en todos los campos (texto, números, hex).
pub(crate) fn text_rows(field: Rect) -> egui::Rangef {
    egui::Rangef::new(
        field.top() + SELECTION_TOP,
        field.bottom() - SELECTION_BOTTOM,
    )
}

/// El recorte de lo que egui pinta al escribir en un campo (`DragValue`, `TextEdit` sin la
/// selección propia): su selección y su cursor quedan en [`text_rows`], dentro del texto
/// (`inner`), como en [`text_field`].
pub(crate) fn text_clip(ui: &Ui, field: Rect, inner: Rect) -> Rect {
    Rect::from_x_y_ranges(inner.x_range().expand(1.0), text_rows(field)).intersect(ui.clip_rect())
}

/// Lo que queda para un campo con un botón chico a su derecha.
pub fn field_width_with_button(ui: &Ui) -> f32 {
    (ui.available_width() - FIELD_BUTTON_GAP - BUTTON_SIZE.x).floor()
}

/// Un campo y su botón chico en una fila: `field` arma el campo con el ancho que queda y
/// `button` el botón.
pub fn field_row<A, B>(
    ui: &mut Ui,
    field: impl FnOnce(&mut Ui, f32) -> A,
    button: impl FnOnce(&mut Ui) -> B,
) -> (A, B) {
    let width = field_width_with_button(ui);
    let out = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = FIELD_BUTTON_GAP;
            let a = field(ui, width);
            let b = button(ui);
            (a, b)
        })
        .inner;
    ui.add_space(FIELD_ROW_GAP - ui.spacing().item_spacing.y);
    out
}

/// Un campo solo, a todo el ancho.
pub fn field_alone<A>(ui: &mut Ui, field: impl FnOnce(&mut Ui, f32) -> A) -> A {
    let width = ui.available_width().floor();
    let out = field(ui, width);
    ui.add_space(FIELD_ROW_GAP - ui.spacing().item_spacing.y);
    out
}

/// Un texto tenue (para "no hay slices").
pub fn note(ui: &mut Ui, skin: &Skin, text: &str) {
    let color = skin.colors().text_dim;
    ui.add(egui::Label::new(egui::RichText::new(text).color(color)).wrap());
}
