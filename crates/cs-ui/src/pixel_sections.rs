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
/// Entre pestañas.
const TAB_GAP: f32 = 5.0;
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
pub const FIELD_HEIGHT: f32 = pixel_ui::CONTROL_HEIGHT;
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

/// Las flechas para recorrer las pestañas que no caben: del alto de una pestaña.
const ARROW_WIDTH: f32 = 11.0;
const ARROW_GAP: f32 = 2.0;

/// Un triángulo de 3×5 píxeles apuntando a la izquierda (`back`) o a la derecha, centrado en
/// `rect` (sin la sombra de abajo de la pestaña).
fn arrow_shapes(rect: Rect, back: bool, color: Color32, ppp: f32) -> Vec<Shape> {
    let center = pos2(
        rect.center().x.floor(),
        (rect.top() + TAB_CAPS_TOP + 2.0).floor(),
    );
    (0..3)
        .map(|col: i32| {
            // La punta es la columna de 1 píxel; la base, la de 5.
            let half = col as f32;
            let x = if back {
                center.x - 1.0 + col as f32
            } else {
                center.x + 1.0 - col as f32
            };
            let r = Rect::from_min_size(pos2(x, center.y - half), vec2(1.0, 2.0 * half + 1.0));
            Shape::rect_filled(
                eframe::emath::GuiRounding::round_to_pixels(r, ppp),
                0.0,
                color,
            )
        })
        .collect()
}

/// Una pestaña: su nombre y, si tiene, el color de su cuadrito.
pub struct Tab {
    pub name: String,
    pub color: Option<Color32>,
}

/// Las pestañas en una sola fila sobre la línea grabada, que se corta debajo de la activa. Si no
/// entran, se achican parejo y los nombres terminan en "..." (como las de documentos), hasta un
/// mínimo: más allá, se ven las que caben y las flechas ◀ ▶ del final (o la rueda) recorren
/// las demás. La activa siempre queda a la vista. Devuelve la que se tocó.
pub fn tabs(ui: &mut Ui, skin: &Skin, tabs: &[Tab], active: Option<usize>) -> Option<usize> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = *skin.colors();
    let left = ui.cursor().left().round();
    let right = ui.max_rect().right();
    let top = ui.cursor().top().round();
    // En una sola fila: si no caben, se achican parejo y los nombres terminan en "..." (como
    // las pestañas de documentos).
    let chrome = |tab: &Tab| {
        let swatch = if tab.color.is_some() {
            SWATCH + SWATCH_GAP
        } else {
            0.0
        };
        TAB_PADDING + swatch + TAB_PADDING
    };
    let natural: Vec<f32> = tabs
        .iter()
        .map(|t| (chrome(t) + text_width(ui, &t.name)).ceil())
        .collect();
    let n = tabs.len();
    let gaps = TAB_GAP * n.saturating_sub(1) as f32;
    let room = (right - left - gaps).floor().max(0.0);
    // Ninguna baja de tres letras y los puntos (la que es más corta entera, queda entera).
    let min =
        (TAB_PADDING * 2.0 + text_width(ui, &format!("Abc{}", crate::tab_bar::ellipsis()))).ceil();
    let widths: Vec<f32> = crate::tab_bar::fit_widths(&natural, room, min)
        .into_iter()
        .zip(&natural)
        .map(|(w, &full)| w.max(min.min(full)))
        .collect();
    let overflow = widths.iter().sum::<f32>() + gaps > right - left;
    // Con flechas, las pestañas llegan hasta ellas.
    let arrows = Rect::from_min_max(
        pos2(right - 2.0 * ARROW_WIDTH - ARROW_GAP, top),
        pos2(right, top + TAB_HEIGHT),
    );
    let tabs_right = if overflow {
        arrows.left() - TAB_GAP
    } else {
        right
    };
    // Desde cuál se ven (y la activa de la vez pasada, para mostrarla cuando cambia).
    let memory = ui.id().with("pixel_tabs_offset");
    let (mut offset, seen): (usize, Option<usize>) =
        ui.data(|d| d.get_temp(memory)).unwrap_or((0, None));
    // Si las de `from` a `to` entran.
    let fits = |from: usize, to: usize| {
        let used: f32 = widths[from..=to].iter().sum::<f32>() + TAB_GAP * (to - from) as f32;
        left + used <= tabs_right
    };
    // La última desde la que se ve hasta el final.
    let last_offset = (0..n)
        .find(|&o| fits(o, n - 1))
        .unwrap_or(n.saturating_sub(1));
    if !overflow {
        offset = 0;
    } else {
        if let Some(a) = active.filter(|&a| seen != Some(a) && a < n) {
            if a < offset {
                offset = a;
            }
            while offset < a && !fits(offset, a) {
                offset += 1;
            }
        }
        // La rueda sobre la fila las recorre.
        let row = Rect::from_x_y_ranges(left..=right, top..=top + TAB_HEIGHT);
        let hovered = ui.rect_contains_pointer(row);
        let scroll: f32 = ui.input(|i| {
            i.raw
                .events
                .iter()
                .map(|e| match e {
                    egui::Event::MouseWheel { delta, .. } => delta.y + delta.x,
                    _ => 0.0,
                })
                .sum()
        });
        if hovered && scroll > 0.0 {
            offset = offset.saturating_sub(1);
        } else if hovered && scroll < 0.0 {
            offset += 1;
        }
        offset = offset.min(last_offset);
    }
    let font = TextStyle::Body.resolve(ui.style());
    let mut rects: Vec<Option<Rect>> = vec![None; n];
    let mut names = vec![String::new(); n];
    let mut x = left;
    for i in offset..n {
        if i > offset && x + widths[i] > tabs_right {
            break;
        }
        rects[i] = Some(Rect::from_min_size(
            pos2(x, top),
            vec2(widths[i], TAB_HEIGHT),
        ));
        let text_room = widths[i] - chrome(&tabs[i]);
        names[i] = crate::tab_bar::fit_text(ui, &tabs[i].name, &font, text_room);
        x += widths[i] + TAB_GAP;
    }
    let last_shown = rects.iter().rposition(Option::is_some).unwrap_or(0);
    let bottom = if n == 0 { top } else { top + TAB_HEIGHT };
    let etched = pixel_ui::ETCHED_SIZE;
    // La línea grabada a todo el ancho, cortada debajo de la activa (si se ve).
    let active_rect = active.and_then(|i| rects.get(i).copied().flatten());
    let line = egui::Rangef::new(left, right);
    let spans = match active_rect.as_ref() {
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
    for (i, ((tab, rect), name)) in tabs.iter().zip(&rects).zip(&names).enumerate() {
        let Some(rect) = rect else {
            continue;
        };
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
        let text = text_shape(ui, name, text_left, rect.top() + TAB_CAPS_TOP, color);
        texts.push((rect.shrink2(vec2(1.0, 0.0)), text));
    }
    // Las flechas para recorrerlas.
    if overflow {
        let prev = Rect::from_min_size(arrows.min, vec2(ARROW_WIDTH, TAB_HEIGHT));
        let next = prev.translate(vec2(ARROW_WIDTH + ARROW_GAP, 0.0));
        for (rect, back, enabled) in [(prev, true, offset > 0), (next, false, last_shown + 1 < n)] {
            let label = if back { "Previous tabs" } else { "Next tabs" };
            let response = ui.interact(rect, ui.id().with(label), Sense::click());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
            let state = if enabled && response.hovered() {
                WidgetState::Hover
            } else {
                WidgetState::Normal
            };
            if let Some(piece) = skin.piece(TAB_PIECE, state) {
                shapes.extend(pixel_ui::piece_shapes(piece, rect, ppp));
            }
            let ink = if enabled {
                colors.text
            } else {
                colors.tab_text_dim.gamma_multiply(0.5)
            };
            shapes.extend(arrow_shapes(rect, back, ink, ppp));
            if enabled && response.clicked() {
                offset = if back {
                    offset.saturating_sub(1)
                } else {
                    (offset + 1).min(last_offset)
                };
            }
        }
    }
    ui.data_mut(|d| d.insert_temp(memory, (offset, active)));
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

#[cfg(test)]
mod tests {
    use eframe::egui::{RawInput, Shape};

    use super::*;

    /// Las pestañas en un panel de `width`: dónde quedó cada caja (las piezas de `tab`) y los
    /// textos dibujados.
    fn draw(names: &[&str], width: f32) -> (Vec<Rect>, Vec<String>) {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::appearance::font_definitions(true));
        draw_in(&ctx, names, width, Some(0))
    }

    fn draw_in(
        ctx: &egui::Context,
        names: &[&str],
        width: f32,
        active: Option<usize>,
    ) -> (Vec<Rect>, Vec<String>) {
        let skin = Skin::embedded(ctx);
        let list: Vec<Tab> = names
            .iter()
            .map(|n| Tab {
                name: n.to_string(),
                color: None,
            })
            .collect();
        let mut out = ctx.run_ui(RawInput::default(), |ui| {
            ui.allocate_ui(vec2(width, 100.0), |ui| {
                ui.set_max_width(width);
                tabs(ui, &skin, &list, active);
            });
        });
        out.textures_delta.clear();
        let mut texts = Vec::new();
        let mut meshes = Vec::new();
        for c in out.shapes {
            match c.shape {
                Shape::Text(t) => texts.push(t.galley.text().to_string()),
                Shape::Mesh(m) => meshes.push(m.calc_bounds()),
                _ => {}
            }
        }
        (meshes, texts)
    }

    #[test]
    fn tabs_that_do_not_fit_stay_in_one_row_and_end_in_dots() {
        let names = ["Randy", "Banana", "Artboard 3", "Artboard 4", "Artboard 5"];
        let (boxes, texts) = draw(&names, 200.0);
        let tops: std::collections::BTreeSet<i32> = boxes
            .iter()
            .filter(|r| r.height() >= TAB_HEIGHT - 1.0)
            .map(|r| r.top() as i32)
            .collect();
        assert_eq!(tops.len(), 1, "one row: {boxes:?}");
        assert!(
            boxes.iter().all(|r| r.right() <= 201.0),
            "inside the panel: {boxes:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.ends_with(crate::tab_bar::ellipsis())),
            "{texts:?}"
        );
        // Si entran, los nombres van enteros.
        let (_, texts) = draw(&["A", "B"], 200.0);
        assert!(
            texts.iter().any(|t| t == "A") && texts.iter().any(|t| t == "B"),
            "{texts:?}"
        );
    }

    #[test]
    fn too_many_tabs_keep_a_minimum_width_and_scroll_to_the_active_one() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::appearance::font_definitions(true));
        let names: Vec<String> = (1..=20).map(|i| format!("Artboard {i}")).collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let tab_texts = |texts: &[String]| -> Vec<String> {
            texts
                .iter()
                .filter(|t| t.starts_with("Ar"))
                .cloned()
                .collect()
        };
        let (_, texts) = draw_in(&ctx, &names, 200.0, Some(0));
        let shown = tab_texts(&texts);
        assert!(shown.len() < 20, "only what fits: {shown:?}");
        assert!(
            shown.iter().all(|t| t.chars().count() >= 4),
            "a minimum: {shown:?}"
        );
        // Elegir la última la trae a la vista (al frame siguiente ya está).
        draw_in(&ctx, &names, 200.0, Some(19));
        let (_, texts) = draw_in(&ctx, &names, 200.0, Some(19));
        let shown = tab_texts(&texts);
        assert!(
            shown
                .iter()
                .any(|t| t.starts_with("Ar") && t.ends_with("20") || t.contains("...")),
            "{shown:?}"
        );
        assert!(
            !shown.iter().any(|t| t == "Artboard 1"),
            "the first ones scrolled away: {shown:?}"
        );
    }
}
