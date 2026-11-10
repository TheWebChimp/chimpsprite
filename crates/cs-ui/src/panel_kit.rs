//! Piezas de dibujo de los paneles grandes del dock (Groom, Lumen): notas en small caps, cajas
//! de doble línea con la esquina en escalón e hijos en un rectángulo.

use eframe::egui::{self, Color32, Rect, Sense, Ui, pos2, vec2};

use crate::pixel_ui;

/// Un texto chico en mayúsculas, apagado: lo que aclara al lado de un título o una casilla, o
/// una nota suelta.
pub(crate) fn hint(ui: &mut Ui, text: &str) {
    let galley = hint_galley(ui, text);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let color = hint_color(ui);
    ui.painter().galley(rect.min, galley, color);
}

/// Una nota (`hint`) de lo más ancha `width`: si no cabe, en varias líneas.
pub(crate) fn hint_wrapped(ui: &mut Ui, text: &str, width: f32) {
    let one_line = hint_galley(ui, text);
    let galley = if one_line.size().x <= width {
        one_line
    } else {
        let mut job = egui::text::LayoutJob::simple(
            text.to_uppercase(),
            one_line.job.sections[0].format.font_id.clone(),
            hint_color(ui),
            width.max(1.0),
        );
        job.wrap.break_anywhere = false;
        ui.painter().layout_job(job)
    };
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    let color = hint_color(ui);
    ui.painter().galley(rect.min, galley, color);
}

/// Un texto como el de `egui::Label` con `wrap` (en varias líneas si no cabe en el ancho de `ui`),
/// pero en puntos enteros: su rect llega al punto de arriba (el de egui medía lo que mide el texto,
/// con fracciones).
pub(crate) fn text_wrapped(ui: &mut Ui, text: &str) {
    let color = ui.visuals().text_color();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let width = ui.available_width().max(1.0);
    let galley = ui.painter().layout(text.to_string(), font, color, width);
    let (rect, response) = ui.allocate_exact_size(galley.size().ceil(), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, text));
    ui.painter().galley(rect.min, galley, color);
}

/// Entre la tinta de un texto y su nota.
pub(crate) const HINT_GAP: f32 = 4.0;

pub(crate) fn hint_color(ui: &Ui) -> Color32 {
    pixel_ui::frame_skin(ui.ctx()).map_or(ui.visuals().weak_text_color(), |s| s.colors().ruler_tick)
}

/// La nota en chimpfont (o, si sus fuentes no se cargaron todavía, en la de siempre).
pub(crate) fn hint_galley(ui: &Ui, text: &str) -> std::sync::Arc<egui::Galley> {
    let small = crate::appearance::small_family();
    let font = if ui.ctx().fonts(|f| f.families().contains(&small)) {
        egui::FontId::new(crate::appearance::SMALL_FONT_SIZE, small)
    } else {
        egui::TextStyle::Body.resolve(ui.style())
    };
    ui.painter()
        .layout_no_wrap(text.to_uppercase(), font, hint_color(ui))
}

/// La nota desde `left`, con su línea base en `baseline` (sin el espacio entre elementos de la
/// fila: es más ancho que el que lleva una nota).
pub(crate) fn hint_on_baseline(ui: &mut Ui, text: &str, left: f32, baseline: f32) {
    let galley = hint_galley(ui, text);
    let caps = crate::tab_bar::cap_height(&galley);
    let ppp = ui.ctx().pixels_per_point();
    let at = crate::tab_bar::text_pos(&galley, left, baseline - caps, ppp);
    let rect = Rect::from_min_size(at, galley.size());
    let color = hint_color(ui);
    ui.painter().galley(at, galley, color);
    ui.allocate_rect(rect, Sense::hover());
}

/// Dónde termina la tinta de `galley` (sin el avance de la última letra), desde su izquierda.
pub(crate) fn ink_right(galley: &egui::Galley) -> f32 {
    galley
        .rows
        .iter()
        .flat_map(|row| {
            row.glyphs
                .iter()
                .map(move |g| row.pos.x + g.pos.x + g.uv_rect.offset.x + g.uv_rect.size.x)
        })
        .fold(0.0, f32::max)
}

/// Un texto y su nota, los dos en la misma línea base.
pub(crate) fn label_with_hint(ui: &mut Ui, text: &str, note: &str) {
    ui.horizontal(|ui| {
        let color = ui.visuals().text_color();
        let galley = ui.painter().layout_no_wrap(
            text.to_string(),
            egui::TextStyle::Body.resolve(ui.style()),
            color,
        );
        let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
        let rect = Rect::from_min_size(rect.min.round(), rect.size());
        // La línea base: la de la primera letra.
        let baseline = galley
            .rows
            .first()
            .and_then(|row| row.glyphs.first().map(|g| row.pos.y + g.pos.y))
            .unwrap_or(galley.size().y);
        let right = rect.left() + ink_right(&galley);
        ui.painter().galley(rect.min, galley, color);
        hint_on_baseline(ui, note, (right + HINT_GAP).round(), rect.top() + baseline);
    });
}

/// Un hijo de `ui` en `rect`, de arriba a abajo (o con `layout`).
pub(crate) fn child(ui: &mut Ui, rect: Rect, layout: egui::Layout) -> Ui {
    ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(layout))
}

/// El color de las cajas: el del lienzo del skin.
pub(crate) fn box_color(ui: &Ui) -> Color32 {
    pixel_ui::frame_skin(ui.ctx()).map_or(crate::theme::CANVAS_BORDER, |s| s.colors().canvas)
}

/// La línea de 1 px de `rect` sin las esquinas: sin `notch` píxeles en diagonal en cada una.
pub(crate) fn notched_outline(rect: Rect, color: Color32, notch: f32) -> Vec<egui::Shape> {
    let line = |min: egui::Pos2, max: egui::Pos2| {
        egui::Shape::rect_filled(Rect::from_min_max(min, max), 0.0, color)
    };
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    let mut shapes = vec![
        line(pos2(l + notch, t), pos2(r - notch, t + 1.0)),
        line(pos2(l + notch, b - 1.0), pos2(r - notch, b)),
        line(pos2(l, t + notch), pos2(l + 1.0, b - notch)),
        line(pos2(r - 1.0, t + notch), pos2(r, b - notch)),
    ];
    // Con 2, el escalón de la esquina.
    if notch >= 2.0 {
        for (x, y) in [
            (l + 1.0, t + 1.0),
            (r - 2.0, t + 1.0),
            (l + 1.0, b - 2.0),
            (r - 2.0, b - 2.0),
        ] {
            shapes.push(line(pos2(x, y), pos2(x + 1.0, y + 1.0)));
        }
    }
    shapes
}

/// La caja de los colores y de las sugerencias: dos líneas, la de afuera con la esquina en
/// escalón.
pub(crate) fn double_box(ui: &Ui, rect: Rect) {
    let color = box_color(ui);
    let mut shapes = notched_outline(rect, color, 2.0);
    shapes.extend(notched_outline(rect.shrink(2.0), color, 1.0));
    ui.painter().extend(shapes);
}

/// Una fila como la de `ui.horizontal` (del alto de los campos de egui) con lo que tiene abajo:
/// los controles del kit (de 17) quedan en puntos enteros, donde ya se pintaban (centrados en la
/// fila de 18 quedaban a medio punto y el kit los pintaba redondeados hacia abajo). Los campos
/// numéricos se ponen solos arriba, a la altura de los de la barra.
pub(crate) fn row_bottom<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let size = vec2(ui.available_width(), ui.spacing().interact_size.y);
    let bottom = egui::Layout::left_to_right(egui::Align::Max);
    ui.allocate_ui_with_layout(size, bottom, add).inner
}
