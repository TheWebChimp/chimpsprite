//! Widgets reutilizables. Los de formularios (números, texto, casillas, radios, combos) usan, con
//! Pixel UI, las piezas del skin (las de la barra de opciones y los menús); sin él, los de egui.

use std::ops::RangeInclusive;

use eframe::egui::{self, Key, Modifiers, emath::Numeric};

use crate::bar_fields::{self, BarLook};
use crate::menus;
use crate::pixel_ui;

/// Con Pixel UI, la "fila" de la barra que empieza donde sigue `ui`: los campos de la barra se
/// dibujan en la fila de `look.top`.
fn here(ui: &egui::Ui) -> f32 {
    ui.cursor().top().round() - bar_fields::FIELD_TOP
}

/// Un id de texto para los campos de la barra que lo piden (el siguiente de `ui`).
fn salt(ui: &egui::Ui) -> String {
    format!("w{}", ui.next_auto_id().value())
}

/// Paso de `Shift + ↑/↓`.
const BIG_STEP: f64 = 10.0;

/// Campo numérico: arrastrar, escribir, `↑`/`↓` (±1, lo hace egui) y `Shift + ↑/↓` (±10).
pub fn number<T: Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    suffix: &str,
) -> egui::Response {
    // Con Pixel UI, el campo numérico de la barra (con `px` si es en píxeles; otra unidad, como
    // texto al lado).
    if let Some(skin) = pixel_ui::frame_skin(ui.ctx()) {
        let look = BarLook {
            skin: &skin,
            top: here(ui),
        };
        let unit = suffix.trim();
        let id = salt(ui);
        return match unit {
            "" | "px" => bar_fields::number(ui, look, &id, value, range, unit == "px"),
            // Otra unidad (%, ms…), escrita adentro del campo.
            _ => bar_fields::number_with_suffix(ui, look, &id, value, range, unit),
        };
    }
    let mut response = ui.add(
        egui::DragValue::new(value)
            .range(range.clone())
            .speed(0.25)
            .suffix(suffix),
    );
    if response.has_focus() {
        let steps = ui.input_mut(|i| {
            i.count_and_consume_key(Modifiers::SHIFT, Key::ArrowUp) as f64
                - i.count_and_consume_key(Modifiers::SHIFT, Key::ArrowDown) as f64
        });
        if steps != 0.0 {
            let next = (value.to_f64() + steps * BIG_STEP)
                .clamp(range.start().to_f64(), range.end().to_f64());
            *value = T::from_f64(next);
            response.mark_changed();
            ui.ctx().request_repaint();
        }
    }
    response
}

/// Texto claro con contorno oscuro: se lee sobre cualquier fondo.
pub fn outlined_text(
    painter: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    font: egui::FontId,
) {
    for d in [
        egui::vec2(-1.0, 0.0),
        egui::vec2(1.0, 0.0),
        egui::vec2(0.0, -1.0),
        egui::vec2(0.0, 1.0),
    ] {
        painter.text(pos + d, align, text, font.clone(), egui::Color32::BLACK);
    }
    painter.text(pos, align, text, font, egui::Color32::WHITE);
}

/// Un campo de texto de una línea: con Pixel UI, el del diseño (`field.text`); sin él, el de egui.
pub fn text_line(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    text_line_named(ui, text, "")
}

/// Como [`text_line`], con `label` de nombre accesible (y para distinguir el campo).
pub fn text_line_named(ui: &mut egui::Ui, text: &mut String, label: &str) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let width = ui.spacing().text_edit_width.min(ui.available_width());
            crate::pixel_sections::text_field(ui, &skin, text, width, label, "")
        }
        None => ui.text_edit_singleline(text),
    }
}

/// Una casilla con su texto: con Pixel UI, la de la barra (`field.check.on`).
pub fn checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let look = BarLook {
                skin: &skin,
                top: here(ui),
            };
            bar_fields::checkbox(ui, look, checked, label)
        }
        None => ui.checkbox(checked, label),
    }
}

/// Una opción de un grupo: con Pixel UI, el radio del diseño (`menu.radio`, `menu.radio.on`)
/// con su texto; sin él, el de egui. Elegirla cambia `current` a `value`.
pub fn radio_value<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    label: &str,
) -> egui::Response {
    let Some(skin) = pixel_ui::frame_skin(ui.ctx()) else {
        return ui.radio_value(current, value, label);
    };
    use crate::skin::WidgetState;
    let on = *current == value;
    let piece = skin
        .piece(
            if on { "menu.radio.on" } else { "menu.radio" },
            WidgetState::Normal,
        )
        .or_else(|| skin.piece("menu.radio", WidgetState::Normal));
    let size = piece.map_or(egui::vec2(10.0, 10.0), |p| {
        egui::vec2(p.size[0] as f32, p.size[1] as f32)
    });
    let color = skin.colors().text;
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    const GAP: f32 = 3.0;
    let height = galley.size().y.max(size.y);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(size.x + GAP + galley.size().x, height),
        egui::Sense::click(),
    );
    if response.clicked() && !on {
        *current = value;
        response.mark_changed();
    }
    let enabled = ui.is_enabled();
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, enabled, on, label)
    });
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let left = rect.left().round();
        // Las mayúsculas (7) centradas en el alto del texto; el radio, centrado con ellas.
        let caps = (rect.center().y - crate::tab_bar::CAP_HEIGHT / 2.0).round();
        let mut shapes = Vec::new();
        if let Some(piece) = piece {
            let top = (caps + (crate::tab_bar::CAP_HEIGHT - size.y) / 2.0).floor();
            let at = egui::Rect::from_min_size(egui::pos2(left, top), size);
            shapes.extend(pixel_ui::piece_shapes(piece, at, ppp));
        }
        let at = crate::tab_bar::text_pos(&galley, left + size.x + GAP, caps, ppp);
        shapes.push(egui::Shape::galley(at, galley, color));
        ui.painter().extend(shapes);
    }
    response
}

/// Un desplegable con `selected` a la vista; `content` arma las opciones (con
/// [`choice`]). Con Pixel UI, el combo de la barra (`field.combo`) y su lista en el panel de los
/// menús; sin él, el `ComboBox` de egui (con `id_salt`).
pub fn combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &str,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let look = BarLook {
                skin: &skin,
                top: here(ui),
            };
            bar_fields::combo(ui, look, id_salt, selected, content)
        }
        None => {
            egui::ComboBox::from_id_salt(id_salt)
                .selected_text(selected)
                .show_ui(ui, content)
                .response
        }
    }
}

/// Una opción de un [`combo`]: elegirla cambia `current` a `value` (y cierra la lista).
pub fn choice<T: PartialEq>(ui: &mut egui::Ui, current: &mut T, value: T, label: &str) -> bool {
    let clicked = ui.add(menus::choice(*current == value, label)).clicked();
    if clicked {
        *current = value;
        ui.close();
    }
    clicked
}
