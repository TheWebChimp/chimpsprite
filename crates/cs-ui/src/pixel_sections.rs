//! Con Pixel UI, las secciones de abajo del panel derecho como en el diseño (frame 4): el título
//! con sus botones chicos (+ y tacho) a la derecha, los elementos como pestañas del skin (`tab`,
//! `tab.active`) sobre la línea grabada y, debajo, los campos de lo elegido.

use eframe::egui::{self, Color32, Rect, Response, Sense, Shape, Ui, Vec2, pos2, vec2};

use crate::kit::tabs as kit_tabs;
use crate::pixel_draw;
use crate::pixel_ui;
use crate::pixel_ui::Tip as _;
use crate::skin::Skin;

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
/// De la línea grabada de las pestañas a los campos.
const TABS_FIELDS_GAP: f32 = 9.0;
/// Entre un campo y su botón (x 945 → 949).
const FIELD_BUTTON_GAP: f32 = 3.0;
/// Entre filas de campos.
const FIELD_ROW_GAP: f32 = 3.0;

/// Las piezas del skin: los íconos de los botones y el campo de texto (en 9-slice).
pub const ICON_ADD: &str = "icon.section.add";
pub const ICON_TRASH: &str = "icon.section.trash";
pub const ICON_CANVAS_SIZE: &str = "icon.section.canvas_size";

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

/// Las pestañas de la sección: las del UI kit (`kit::tabs`, estilo documento) en una sola fila
/// sobre la línea grabada, que se corta debajo de la activa. Si no entran, se achican parejo y
/// los nombres terminan en "..." hasta un mínimo; más allá, las flechas del final (o la rueda)
/// recorren las demás. La activa siempre queda a la vista. Devuelve la que se tocó.
pub fn tabs(ui: &mut Ui, skin: &Skin, tabs: &[Tab], active: Option<usize>) -> Option<usize> {
    let left = ui.cursor().left().round();
    let right = ui.max_rect().right();
    let top = ui.cursor().top().round();
    let strip = kit_tabs::Strip::new(left, right, top);
    let items: Vec<kit_tabs::TabItem> = tabs
        .iter()
        .map(|t| match t.color {
            Some(color) => kit_tabs::TabItem::colored(t.name.clone(), color),
            None => kit_tabs::TabItem::new(t.name.clone()),
        })
        .collect();
    let id = ui.id().with("pixel_tabs");
    let event = kit_tabs::tab_strip(ui, skin, id, strip, &items, active);
    let bottom = if tabs.is_empty() { top } else { strip.bottom() };
    let used = Rect::from_x_y_ranges(left..=right, top..=bottom + pixel_ui::ETCHED_SIZE);
    ui.allocate_rect(used, Sense::hover());
    ui.add_space(TABS_FIELDS_GAP - ui.spacing().item_spacing.y);
    match event {
        Some(kit_tabs::TabEvent::Select(i)) => Some(i),
        _ => None,
    }
}

/// Las filas de la selección y del cursor de escribir de los campos que todavía no son del UI kit
/// (el hex y los números del editor de color, el timeline): de la fila 2 del campo hasta 1 arriba
/// de la sombra de abajo.
const SELECTION_TOP: f32 = 2.0;
const SELECTION_BOTTOM: f32 = 4.0;

/// Las filas de un campo (`field`, con su borde) donde van la selección y el cursor de escribir.
pub(crate) fn text_rows(field: Rect) -> egui::Rangef {
    egui::Rangef::new(
        field.top() + SELECTION_TOP,
        field.bottom() - SELECTION_BOTTOM,
    )
}

/// El recorte de lo que egui pinta al escribir en un campo (`DragValue`, `TextEdit` sin la
/// selección propia): su selección y su cursor quedan en [`text_rows`], dentro del texto
/// (`inner`).
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
            .filter(|r| r.height() >= crate::kit::tabs::HEIGHT - 1.0)
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
