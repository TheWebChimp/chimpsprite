//! El select (mockup de componentes, frame 9, x 469 y 454, y 321, 378 y 480):
//!
//! - **Cerrado:** el campo de texto del kit (`field.text`, `.disabled`) con lo elegido 5 px
//!   adentro, las mayúsculas en la fila 5, en `color.text` (deshabilitado,
//!   `color.field_text_disabled`), y el ▾ (`select.arrow`, 5×3, de una tinta, del color del
//!   texto) en las filas 7 a 9, 4 px antes del borde derecho. Del texto a la flecha, por lo menos 5.
//! - **Abierto:** el campo pierde su borde de abajo (`select.open`, 16 filas: la fila 15 es la de
//!   sombra, que queda sobre el borde del panel) y debajo, desde su fila 15, el panel
//!   (`select.list`): borde negro, bisel, 11 px más ancho que el campo y una sombra negra
//!   translúcida de 2 px abajo a la derecha. Las opciones son filas de 15 desde la fila 4 del panel
//!   (debajo, 1 fila de aire, el bisel y el borde), con el texto 6 px adentro y las mayúsculas en
//!   su fila 4. Con el puntero encima, la franja rosa `select.item.hover`; sin el puntero en el
//!   panel, la franja va en la elegida.
//!
//! Las opciones de siempre (`widgets::choice`, `bar_fields::choices`, `menus::choice`) se dibujan
//! así cuando están en el panel de un select: la API de los combos no cambia.

use eframe::egui::{
    self, Color32, Id, Popup, PopupCloseBehavior, Rect, RectAlign, Response, Sense, Shape, Ui,
    UiBuilder, UiStackInfo, Vec2, pos2, vec2,
};
use eframe::emath::GuiRounding as _;

use super::input::{self, Affixes, InputState};
use crate::pixel_ui;
use crate::skin::{Skin, WidgetState};
use crate::tab_bar;

pub const HEIGHT: f32 = input::HEIGHT;
pub const TEXT_LEFT: f32 = input::TEXT_LEFT;
pub const CAPS_TOP: f32 = input::CAPS_TOP;
/// La flecha: 5×3, en las filas 7 a 9, con 4 columnas hasta el borde derecho (1).
pub const ARROW: &str = "select.arrow";
const ARROW_SIZE: Vec2 = vec2(5.0, 3.0);
pub const ARROW_TOP: f32 = 7.0;
pub const ARROW_RIGHT: f32 = 4.0 + input::OUTLINE;
/// Del texto a la flecha, por lo menos.
pub const ARROW_GAP: f32 = 5.0;

pub const OPEN: &str = "select.open";
/// El campo abierto: 16 filas (sin el borde de abajo ni la luz).
pub const OPEN_HEIGHT: f32 = 16.0;
pub const LIST: &str = "select.list";
pub const ITEM_HOVER: &str = "select.item.hover";
/// El panel empieza en la fila 15 del campo y es 11 px más ancho.
pub const LIST_TOP: f32 = 15.0;
pub const LIST_EXTRA: f32 = 11.0;
/// Adentro del panel: las opciones desde la fila 4 (borde, blanco y 2 de relleno); debajo, 1 de
/// aire, el bisel y el borde. A los lados, el borde (las franjas van sobre el bisel).
pub const LIST_PAD_TOP: f32 = 4.0;
pub const LIST_PAD_BOTTOM: f32 = 3.0;
pub const LIST_SIDE: f32 = 1.0;
/// La sombra, abajo y a la derecha.
pub const SHADOW: f32 = 2.0;
/// Una opción: 15 filas, el texto 6 px adentro del borde del panel (5 de la franja) y las
/// mayúsculas en su fila 4.
pub const ITEM_HEIGHT: f32 = 15.0;
pub const ITEM_TEXT_LEFT: f32 = 5.0;
pub const ITEM_CAPS_TOP: f32 = 4.0;

const SELECT_TAG: &str = "chimpsprite_kit_select";

/// Lo que necesitan las opciones del panel: el skin y si el puntero está sobre el panel.
#[derive(Clone)]
struct SelectLook {
    skin: Skin,
    pointer_in_list: bool,
}

fn select_look(ui: &Ui) -> Option<SelectLook> {
    ui.stack()
        .iter()
        .find_map(|s| s.info.tags.get_downcast::<SelectLook>(SELECT_TAG))
        .cloned()
}

fn galley(ui: &Ui, text: &str) -> std::sync::Arc<egui::Galley> {
    let font = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER)
}

/// El ancho justo de un select con `text`: `5 + tinta + 5 + 5 + 4 + 1`.
pub fn width(ui: &Ui, text: &str) -> f32 {
    TEXT_LEFT + crate::menus::ink_width(&galley(ui, text)) + ARROW_GAP + ARROW_SIZE.x + ARROW_RIGHT
}

/// Un select de `width` (por lo menos el justo) en el lugar que sigue de `ui`; `content` arma las
/// opciones (con `widgets::choice` o [`item`]).
pub fn select<R>(
    ui: &mut Ui,
    skin: &Skin,
    id_salt: &str,
    selected: &str,
    width: Option<f32>,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let natural = self::width(ui, selected);
    let w = width.map_or(natural, |w| w.max(natural)).round();
    let (slot, _) = ui.allocate_exact_size(vec2(w, HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    let id = ui.id().with(("kit_select", id_salt));
    select_at(ui, skin, rect, id, selected, content)
}

/// [`select`] en `rect` (de [`HEIGHT`] de alto) con el id `id`.
pub fn select_at<R>(
    ui: &mut Ui,
    skin: &Skin,
    rect: Rect,
    id: Id,
    selected: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Response {
    let response = ui.interact(rect, id, Sense::click());
    let enabled = ui.is_enabled();
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ComboBox, enabled, selected));
    let popup_id = Popup::default_response_id(&response);
    let open = Popup::is_id_open(ui.ctx(), popup_id);
    let state = if enabled {
        InputState::Normal
    } else {
        InputState::Disabled
    };
    if ui.is_rect_visible(rect) {
        let shapes = field_shapes(ui, skin, rect, selected, state, open);
        let mut painter = ui.painter().clone();
        painter.set_opacity(1.0);
        painter.extend(shapes);
    }
    list(ui, skin, &response, rect, selected, content);
    response
}

/// El campo en `rect`: cerrado, el campo del kit; abierto, `select.open`. Con lo elegido y la
/// flecha.
pub fn field_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    selected: &str,
    state: InputState,
    open: bool,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = skin.colors();
    let open_piece = skin.piece(OPEN, WidgetState::Normal).filter(|_| open);
    let mut shapes = match open_piece {
        Some(piece) => pixel_ui::piece_shapes(
            piece,
            Rect::from_min_size(rect.min, vec2(rect.width(), OPEN_HEIGHT)),
            ppp,
        ),
        None => input::frame_shapes(ui, skin, rect, state, Affixes::NONE),
    };
    let color = match state {
        InputState::Disabled => colors.field_text_disabled,
        _ => colors.text,
    };
    let shown = ui.painter().layout_no_wrap(
        selected.replace('…', tab_bar::ellipsis()),
        egui::TextStyle::Body.resolve(ui.style()),
        color,
    );
    let at = tab_bar::text_pos(&shown, rect.left() + TEXT_LEFT, rect.top() + CAPS_TOP, ppp);
    shapes.push(Shape::galley(at, shown, color));
    let arrow_at = pos2(
        rect.right() - ARROW_RIGHT - ARROW_SIZE.x,
        rect.top() + ARROW_TOP,
    );
    shapes.extend(arrow_shapes(skin, arrow_at, color));
    shapes
}

/// La flecha (de una tinta) desde `at`, en `color`, píxel por píxel; sin la pieza, un ▾ de 5×3.
fn arrow_shapes(skin: &Skin, at: egui::Pos2, color: Color32) -> Vec<Shape> {
    let dot = |x: u32, y: u32| {
        Shape::rect_filled(
            Rect::from_min_size(at + vec2(x as f32, y as f32), Vec2::splat(1.0)),
            0.0,
            color,
        )
    };
    match skin.piece(ARROW, WidgetState::Normal) {
        Some(piece) => {
            let [w, h] = piece.size;
            (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter(|&(x, y)| piece.rgba[((y * w + x) * 4 + 3) as usize] > 0)
                .map(|(x, y)| dot(x, y))
                .collect()
        }
        None => [(0..5), (1..4), (2..3)]
            .into_iter()
            .enumerate()
            .flat_map(|(y, xs)| xs.map(move |x| (x as u32, y as u32)))
            .map(|(x, y)| dot(x, y))
            .collect(),
    }
}

/// El panel con las opciones, abierto con un clic en el campo (y cerrado al elegir una o con un
/// clic afuera).
fn list<R>(
    ui: &Ui,
    skin: &Skin,
    response: &Response,
    rect: Rect,
    selected: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let width_id = response.id.with("list_width");
    let list_rect_id = response.id.with("list_rect");
    let remembered: f32 = ui.data(|d| d.get_temp(width_id)).unwrap_or(0.0);
    let panel_w = (rect.width() + LIST_EXTRA).max(remembered);
    let last_rect: Option<Rect> = ui.data(|d| d.get_temp(list_rect_id));
    let pointer_in_list = ui
        .ctx()
        .pointer_hover_pos()
        .zip(last_rect)
        .is_some_and(|(p, r)| r.contains(p));
    let anchor = Rect::from_min_size(rect.min, vec2(rect.width(), LIST_TOP));
    let inner = Popup::menu(response)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .anchor(anchor)
        .align(RectAlign::BOTTOM_START)
        .align_alternatives(&[RectAlign::TOP_START])
        .gap(0.0)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            let ppp = ui.ctx().pixels_per_point();
            let top_left = ui.max_rect().min.round();
            let back = ui.painter().add(Shape::Noop);
            let look = SelectLook {
                skin: skin.clone(),
                pointer_in_list,
            };
            let items = Rect::from_min_size(
                top_left + vec2(LIST_SIDE, LIST_PAD_TOP),
                vec2(panel_w - 2.0 * LIST_SIDE, f32::INFINITY),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(items)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .ui_stack_info(UiStackInfo::default().with_tag_value(SELECT_TAG, look)),
            );
            child.spacing_mut().item_spacing = Vec2::ZERO;
            child.set_min_width(items.width());
            let out = content(&mut child);
            let used = child.min_rect();
            let panel = Rect::from_min_size(
                top_left,
                vec2(
                    used.width().max(items.width()) + 2.0 * LIST_SIDE,
                    LIST_PAD_TOP + used.height() + LIST_PAD_BOTTOM,
                ),
            );
            let with_shadow = Rect::from_min_size(panel.min, panel.size() + Vec2::splat(SHADOW));
            ui.allocate_rect(with_shadow, Sense::hover());
            let back_shapes = match skin.piece(LIST, WidgetState::Normal) {
                Some(piece) => pixel_ui::piece_shapes(piece, with_shadow, ppp),
                None => {
                    let colors = skin.colors();
                    vec![
                        Shape::rect_filled(panel, 0.0, colors.panel),
                        Shape::rect_stroke(
                            panel.round_to_pixels(ppp),
                            0.0,
                            egui::Stroke::new(1.0, colors.outline),
                            egui::StrokeKind::Inside,
                        ),
                    ]
                }
            };
            ui.painter().set(back, Shape::Vec(back_shapes));
            // El campo abierto, encima del borde del panel (su fila de sombra queda sobre él).
            ui.painter().extend(field_shapes(
                ui,
                skin,
                rect,
                selected,
                InputState::Normal,
                true,
            ));
            ui.data_mut(|d| {
                d.insert_temp(width_id, panel.width());
                d.insert_temp(list_rect_id, panel);
            });
            out
        });
    inner.map(|i| {
        response.ctx.move_to_top(i.response.layer_id);
        i.inner
    })
}

/// Una opción del panel de un select: la fila de 15 con su texto, la franja rosa con el puntero
/// encima (o en la elegida, sin el puntero en el panel). Fuera de un select, `None`.
pub fn item(ui: &mut Ui, selected: bool, label: &str) -> Option<Response> {
    let look = select_look(ui)?;
    let skin = &look.skin;
    let colors = skin.colors();
    let enabled = ui.is_enabled();
    let shown_text = label.replace('…', tab_bar::ellipsis());
    let ink = crate::menus::ink_width(&galley(ui, &shown_text));
    let natural = ITEM_TEXT_LEFT + ink + ITEM_TEXT_LEFT;
    let width = ui.available_width().min(1.0e6).max(natural);
    let (slot, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), Sense::click());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, label)
    });
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let lit = enabled && (response.hovered() || (selected && !look.pointer_in_list));
        let mut shapes = Vec::new();
        if lit {
            shapes.extend(match skin.piece(ITEM_HOVER, WidgetState::Normal) {
                Some(piece) => pixel_ui::piece_shapes(piece, rect, ppp),
                None => vec![Shape::rect_filled(rect, 0.0, colors.text_selection)],
            });
        }
        let color = if enabled {
            colors.text
        } else {
            colors.text_dim
        };
        let shown = ui.painter().layout_no_wrap(
            shown_text,
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
        let at = tab_bar::text_pos(
            &shown,
            rect.left() + ITEM_TEXT_LEFT,
            rect.top() + ITEM_CAPS_TOP,
            ppp,
        );
        shapes.push(Shape::galley(at, shown, color));
        ui.painter().extend(shapes);
    }
    Some(response)
}

/// Un select dibujado (sin interacción) cerrado en `state` o abierto con `items` (la `hovered`
/// con la franja): el catálogo lo usa para mostrar el mockup.
pub fn forced_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    selected: &str,
    state: InputState,
    items: &[&str],
    hovered: Option<usize>,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    if items.is_empty() {
        return field_shapes(ui, skin, rect, selected, state, false);
    }
    let colors = skin.colors();
    let top_left = rect.min + vec2(0.0, LIST_TOP);
    let panel = Rect::from_min_size(
        top_left,
        vec2(
            rect.width() + LIST_EXTRA,
            LIST_PAD_TOP + items.len() as f32 * ITEM_HEIGHT + LIST_PAD_BOTTOM,
        ),
    );
    let mut shapes = Vec::new();
    if let Some(piece) = skin.piece(LIST, WidgetState::Normal) {
        let with_shadow = Rect::from_min_size(panel.min, panel.size() + Vec2::splat(SHADOW));
        shapes.extend(pixel_ui::piece_shapes(piece, with_shadow, ppp));
    }
    for (i, label) in items.iter().enumerate() {
        let row = Rect::from_min_size(
            top_left + vec2(LIST_SIDE, LIST_PAD_TOP + i as f32 * ITEM_HEIGHT),
            vec2(panel.width() - 2.0 * LIST_SIDE, ITEM_HEIGHT),
        );
        if hovered == Some(i)
            && let Some(piece) = skin.piece(ITEM_HOVER, WidgetState::Normal)
        {
            shapes.extend(pixel_ui::piece_shapes(piece, row, ppp));
        }
        let (text, _) = crate::pixel_draw::caps_text(
            ui,
            label,
            row.left() + ITEM_TEXT_LEFT,
            row.top() + ITEM_CAPS_TOP,
            colors.text,
        );
        shapes.push(text);
    }
    shapes.extend(field_shapes(ui, skin, rect, selected, state, true));
    shapes
}

#[cfg(test)]
mod tests;
