//! Widgets reutilizables.

use std::ops::RangeInclusive;

use eframe::egui::{self, Key, Modifiers, emath::Numeric};

/// Paso de `Shift + ↑/↓`.
const BIG_STEP: f64 = 10.0;

/// Campo numérico: arrastrar, escribir, `↑`/`↓` (±1, lo hace egui) y `Shift + ↑/↓` (±10).
pub fn number<T: Numeric>(
    ui: &mut egui::Ui,
    value: &mut T,
    range: RangeInclusive<T>,
    suffix: &str,
) -> egui::Response {
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
