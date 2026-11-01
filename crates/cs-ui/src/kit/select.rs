//! El select (mockup de componentes, frame 9, x 469 y 454, y 321, 378 y 480):
//!
//! - **Cerrado:** el campo de texto del kit (`field.text`, `.disabled`) con lo elegido 5 px
//!   adentro, las mayúsculas en la fila 5, en `color.text` (deshabilitado,
//!   `color.field_text_disabled`), y el ▾ (`select.arrow`, 5×3, de una tinta, del color del
//!   texto) en las filas 7 a 9, 4 px antes del borde derecho. Del texto a la flecha, por lo menos 5.
//! - **Abierto:** el campo pierde su borde de abajo (`select.open`, 16 filas: la fila 15 es la de
//!   sombra, que queda sobre el borde del panel) y debajo, desde su fila 15, el panel
//!   (`select.list`): borde negro, bisel, 11 px más ancho que el campo y una sombra (5,4,3)
//!   con alfa 123 de 2 px abajo a la derecha. Las opciones son filas de 15 desde la fila 4 del panel
//!   (debajo, 1 fila de aire, el bisel y el borde), con el texto 6 px adentro y las mayúsculas en
//!   su fila 4. Con el puntero encima, la franja rosa `select.item.hover`; sin el puntero en el
//!   panel, la franja va en la elegida.
//!
//! - **Abierto hacia arriba** (sin lugar abajo): el panel (`select.list.alone`, con la esquina
//!   de arriba a la izquierda redondeada) termina con su borde de abajo sobre la fila 0 del campo,
//!   y el campo pierde su borde de arriba (`select.open.up`: la fila 0 es la de sombra, con el
//!   borde negro del panel a la derecha).
//! - **Con scroll:** la barra va adentro del panel, de la fila 4 a 3 antes de abajo, con 1 px de
//!   aire antes del bisel derecho (el panel crece 2); las franjas llegan hasta la barra.
//! - **Separador** ([`separator`]): un bloque de 7 filas con una línea de 1 px (`color.canvas`)
//!   en la del medio, de lado a lado de las opciones.
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
/// El campo abierto hacia arriba: sin el borde de arriba (las 17 filas).
pub const OPEN_UP: &str = "select.open.up";
/// El campo abierto: 16 filas (sin el borde de abajo ni la luz).
pub const OPEN_HEIGHT: f32 = 16.0;
pub const LIST: &str = "select.list";
/// El panel que no cuelga del campo (abierto hacia arriba): la esquina de arriba a la izquierda,
/// redondeada como la de la derecha.
pub const LIST_ALONE: &str = "select.list.alone";
/// El aire mínimo entre el panel y el borde de la ventana.
const SCREEN_AIR: f32 = 4.0;
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
/// Con scroll, el aire entre la barra y el bisel derecho (lo que crece el panel).
pub const SCROLL_AIR: f32 = 2.0;
/// El separador: 3 filas de aire, la línea y 3 de aire.
pub const SEPARATOR_HEIGHT: f32 = 7.0;
const SEPARATOR_AIR: f32 = 3.0;
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
    /// El primer frame abierto: la elegida se trae a la vista.
    just_opened: bool,
    /// En la memoria de egui: el ancho justo de la opción más ancha (cada una lo sube).
    width_id: Id,
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
    // Desarrollo: con CHIMPSPRITE_DEMO_OPEN_SELECT=<lo elegido>, ese select abierto (una vez),
    // para capturas.
    if crate::demo::dev_var("CHIMPSPRITE_DEMO_OPEN_SELECT") == Some(selected) {
        let done = id.with("demo_opened");
        if !ui.data(|d| d.get_temp::<bool>(done).unwrap_or(false)) {
            ui.data_mut(|d| d.insert_temp(done, true));
            Popup::open_id(ui.ctx(), popup_id);
        }
    }
    // Abierto hacia abajo, el campo se une a la lista; hacia arriba queda como cerrado.
    let above = ui
        .data(|d| d.get_temp::<bool>(above_id(id)))
        .unwrap_or(false);
    let open = if !Popup::is_id_open(ui.ctx(), popup_id) {
        Opened::No
    } else if above {
        Opened::Above
    } else {
        Opened::Below
    };
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

/// Si el select está abierto y hacia dónde.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Opened {
    No,
    Below,
    Above,
}

/// El campo en `rect`: cerrado, el campo del kit; abierto, `select.open` (o `select.open.up`).
/// Con lo elegido y la flecha.
pub fn field_shapes(
    ui: &Ui,
    skin: &Skin,
    rect: Rect,
    selected: &str,
    state: InputState,
    open: Opened,
) -> Vec<Shape> {
    let ppp = ui.ctx().pixels_per_point();
    let colors = skin.colors();
    let open_piece = match open {
        Opened::No => None,
        Opened::Below => skin.piece(OPEN, WidgetState::Normal).map(|p| {
            (
                p,
                Rect::from_min_size(rect.min, vec2(rect.width(), OPEN_HEIGHT)),
            )
        }),
        Opened::Above => skin.piece(OPEN_UP, WidgetState::Normal).map(|p| (p, rect)),
    };
    let mut shapes = match open_piece {
        Some((piece, at)) => pixel_ui::piece_shapes(piece, at, ppp),
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
/// clic afuera). Va debajo del campo si entra; si no, del lado con más lugar (arriba: con la
/// esquina redondeada, `select.list.alone`, y su borde de abajo sobre la fila 0 del campo), y lo
/// que no entra se recorre con el scroll. Al abrirse, la elegida queda a la vista.
fn list<R>(
    ui: &Ui,
    skin: &Skin,
    response: &Response,
    rect: Rect,
    selected: &str,
    content: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    // El ancho de la opción más ancha, del frame anterior (este lo vuelven a medir).
    let width_id = response.id.with("list_width");
    let list_rect_id = response.id.with("list_rect");
    let content_id = response.id.with("list_content");
    let remembered: f32 = ui.data(|d| d.get_temp(width_id)).unwrap_or(0.0);
    let last_rect: Option<Rect> = ui.data(|d| d.get_temp(list_rect_id));
    let was_open = last_rect.is_some();
    let pointer_in_list = ui
        .ctx()
        .pointer_hover_pos()
        .zip(last_rect)
        .is_some_and(|(p, r)| r.contains(p));
    // Dónde va: lo que miden las opciones (del frame anterior) contra el lugar de cada lado.
    let content_h: f32 = ui.data(|d| d.get_temp(content_id)).unwrap_or(ITEM_HEIGHT);
    let screen = ui.ctx().content_rect();
    let pads = LIST_PAD_TOP + LIST_PAD_BOTTOM + SHADOW + SCREEN_AIR;
    let below_room = screen.bottom() - (rect.top() + LIST_TOP) - pads;
    let above_room = rect.top() - screen.top() - pads;
    let above = content_h > below_room && above_room > below_room;
    ui.data_mut(|d| d.insert_temp(above_id(response.id), above));
    let room = if above { above_room } else { below_room };
    let max_h = room.max(3.0 * ITEM_HEIGHT).floor();
    // Con scroll, la barra va adentro y el panel crece su aire.
    let scroll_air = if content_h > max_h + 0.5 {
        SCROLL_AIR
    } else {
        0.0
    };
    let bar = if scroll_air > 0.0 {
        pixel_ui::SCROLL_BAR
    } else {
        0.0
    };
    let panel_w = (rect.width() + LIST_EXTRA)
        .max(remembered + 2.0 * LIST_SIDE + bar)
        .ceil()
        + scroll_air;
    ui.data_mut(|d| d.insert_temp(width_id, 0.0f32));
    let (anchor, align) = if above {
        // El panel (sin la sombra) termina en la fila 0 del campo.
        (
            rect.translate(vec2(0.0, 1.0 + SHADOW)),
            RectAlign::TOP_START,
        )
    } else {
        (
            Rect::from_min_size(rect.min, vec2(rect.width(), LIST_TOP)),
            RectAlign::BOTTOM_START,
        )
    };
    let inner = Popup::menu(response)
        .close_behavior(PopupCloseBehavior::CloseOnClick)
        .anchor(anchor)
        .align(align)
        .align_alternatives(&[])
        .gap(0.0)
        .frame(egui::Frame::NONE)
        .show(|ui| {
            let ppp = ui.ctx().pixels_per_point();
            let top_left = ui.max_rect().min.round();
            let back = ui.painter().add(Shape::Noop);
            let look = SelectLook {
                skin: skin.clone(),
                pointer_in_list,
                just_opened: !was_open,
                width_id,
            };
            let items = Rect::from_min_size(
                top_left + vec2(LIST_SIDE, LIST_PAD_TOP),
                vec2(panel_w - 2.0 * LIST_SIDE - scroll_air, max_h),
            );
            let mut child = ui.new_child(
                UiBuilder::new()
                    .max_rect(items)
                    .layout(egui::Layout::top_down(egui::Align::Min))
                    .ui_stack_info(UiStackInfo::default().with_tag_value(SELECT_TAG, look)),
            );
            child.set_min_width(items.width());
            let area = egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true]);
            let (out, full_h) = pixel_ui::scroll(&mut child, area, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                ui.set_min_width(ui.available_width());
                let out = content(ui);
                (out, ui.min_rect().height())
            });
            let used = child.min_rect();
            let panel = Rect::from_min_size(
                top_left,
                vec2(panel_w, LIST_PAD_TOP + used.height() + LIST_PAD_BOTTOM),
            );
            let with_shadow = Rect::from_min_size(panel.min, panel.size() + Vec2::splat(SHADOW));
            ui.allocate_rect(with_shadow, Sense::hover());
            let name = if above { LIST_ALONE } else { LIST };
            let piece = skin
                .piece(name, WidgetState::Normal)
                .or_else(|| skin.piece(LIST, WidgetState::Normal));
            let back_shapes = match piece {
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
            let opened = if above { Opened::Above } else { Opened::Below };
            ui.painter().extend(field_shapes(
                ui,
                skin,
                rect,
                selected,
                InputState::Normal,
                opened,
            ));
            ui.data_mut(|d| {
                d.insert_temp(list_rect_id, panel);
                d.insert_temp(content_id, full_h);
            });
            out
        });
    if inner.is_none() {
        ui.data_mut(|d| d.remove::<Rect>(list_rect_id));
    }
    inner.map(|i| {
        response.ctx.move_to_top(i.response.layer_id);
        i.inner
    })
}

/// En la memoria de egui: si la lista del select `id` va arriba del campo.
fn above_id(id: Id) -> Id {
    id.with("list_above")
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
    ui.data_mut(|d| {
        let widest = d.get_temp_mut_or(look.width_id, 0.0f32);
        *widest = widest.max(natural);
    });
    let width = ui.available_width().min(1.0e6).max(natural);
    let (slot, response) = ui.allocate_exact_size(vec2(width, ITEM_HEIGHT), Sense::click());
    let rect = Rect::from_min_size(slot.min.round(), slot.size());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, label)
    });
    if selected && look.just_opened {
        response.scroll_to_me(Some(egui::Align::Center));
    }
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

/// Un separador en el panel de un select: 7 filas con la línea (`color.canvas`, 1 px) en la del
/// medio, de lado a lado de las opciones. Fuera de un select, `false` (y no hace nada).
pub fn separator(ui: &mut Ui) -> bool {
    let Some(look) = select_look(ui) else {
        return false;
    };
    let width = if ui.is_sizing_pass() {
        0.0
    } else {
        ui.available_width()
    };
    let (slot, _) = ui.allocate_exact_size(vec2(width, SEPARATOR_HEIGHT), Sense::hover());
    let line = Rect::from_min_size(
        pos2(slot.left(), slot.top() + SEPARATOR_AIR).round(),
        vec2(slot.width(), 1.0),
    );
    ui.painter()
        .rect_filled(line, 0.0, look.skin.colors().canvas);
    true
}

/// Un separador en una lista de opciones: en el panel de un select, el del kit; si no, el de egui.
pub fn list_separator(ui: &mut Ui) {
    if !separator(ui) {
        ui.separator();
    }
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
        return field_shapes(ui, skin, rect, selected, state, Opened::No);
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
    shapes.extend(field_shapes(ui, skin, rect, selected, state, Opened::Below));
    shapes
}

#[cfg(test)]
mod tests;
