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

/// Campo numérico con una letra adelante (`W`, `H`, `X`, `Y`): con Pixel UI, el de la barra con
/// la letra en su cuadrito; sin él, el de egui con la letra de prefijo.
pub fn prefixed_number<T: Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    prefix: &str,
) -> egui::Response {
    if let Some(skin) = pixel_ui::frame_skin(ui.ctx()) {
        let look = BarLook {
            skin: &skin,
            top: here(ui),
        };
        let id = salt(ui);
        return bar_fields::number_with_prefix(ui, look, &id, value, range, prefix);
    }
    ui.add(
        egui::DragValue::new(value)
            .range(range)
            .speed(0.05)
            .prefix(format!("{prefix} ")),
    )
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

/// Un campo de texto de una línea: con Pixel UI, el del UI kit (`kit::input`); sin él, el de egui.
pub fn text_line(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    text_line_named(ui, text, "")
}

/// Como [`text_line`], con `label` de nombre accesible (y para distinguir el campo).
pub fn text_line_named(ui: &mut egui::Ui, text: &mut String, label: &str) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let width = ui.spacing().text_edit_width.min(ui.available_width());
            crate::kit::input::text_input(ui, &skin, text, width, label, "")
        }
        None => ui.text_edit_singleline(text),
    }
}

/// Una casilla con su texto: con Pixel UI, la del UI kit (`kit::check`); sin él, la de egui.
pub fn checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => crate::kit::check::checkbox(ui, &skin, checked, label),
        None => ui.checkbox(checked, label),
    }
}

/// Una opción de un grupo: con Pixel UI, el radio del UI kit (`kit::check`); sin él, el de egui.
/// Elegirla cambia `current` a `value`.
pub fn radio_value<T: PartialEq>(
    ui: &mut egui::Ui,
    current: &mut T,
    value: T,
    label: &str,
) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => crate::kit::check::radio_value(ui, &skin, current, value, label),
        None => ui.radio_value(current, value, label),
    }
}

/// Un botón de color que abre un selector: con Pixel UI, el del UI kit (`kit::color_button`, el
/// selector de la app); sin él, el de egui. Sin `alpha`, el color queda opaco. `changed()` si
/// cambió.
pub fn color_edit(
    ui: &mut egui::Ui,
    color: &mut cs_core::Rgba,
    alpha: bool,
    label: &str,
) -> egui::Response {
    if let Some(skin) = pixel_ui::frame_skin(ui.ctx()) {
        return crate::kit::color_button::color_edit(ui, &skin, color, alpha, label);
    }
    let mut c = egui::Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a);
    let mode = if alpha {
        egui::color_picker::Alpha::OnlyBlend
    } else {
        egui::color_picker::Alpha::Opaque
    };
    let response = egui::color_picker::color_edit_button_srgba(ui, &mut c, mode);
    if response.changed() {
        let [r, g, b, a] = c.to_srgba_unmultiplied();
        *color = cs_core::Rgba::new(r, g, b, if alpha { a } else { 255 });
    }
    response
}

/// Un desplegable con `selected` a la vista; `content` arma las opciones (con
/// [`choice`]). Con Pixel UI, el select del UI kit (`kit::select`) con su lista; sin él, el
/// `ComboBox` de egui (con `id_salt`).
pub fn combo(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &str,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    combo_sized(
        ui,
        id_salt,
        selected,
        None,
        bar_fields::FIELD_HEIGHT,
        content,
    )
}

/// Como [`combo`], de `height` y, con `width`, al menos de ese ancho.
pub fn combo_sized(
    ui: &mut egui::Ui,
    id_salt: &str,
    selected: &str,
    width: Option<f32>,
    height: f32,
    content: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    match pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let look = BarLook {
                skin: &skin,
                top: here(ui),
            };
            bar_fields::combo_sized(ui, look, id_salt, selected, width, height, content)
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
