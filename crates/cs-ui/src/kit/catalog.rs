//! La pestaña UI Catalog: una página con scroll, sobre el fondo del área de trabajo, con una
//! sección por componente del kit. Cada componente nuevo agrega la suya.

use eframe::egui::{self, Pos2, Rect, Sense, Shape, TextStyle, Ui, Vec2, pos2, vec2};

use super::button::{self, ButtonState};
use crate::pixel_draw;
use crate::skin::Skin;

/// El nombre de la pestaña.
pub const TITLE: &str = "UI Catalog";

/// Medidas de la página, desde la esquina de arriba a la izquierda del contenido.
const MARGIN: Vec2 = vec2(12.0, 12.0);
/// El título de una sección (sus mayúsculas) y, debajo, la línea grabada.
const TITLE_TO_LINE: f32 = 10.0;
const LINE_TO_CONTENT: f32 = 10.0;
/// El ancho de la página (la línea grabada de las secciones).
const PAGE_WIDTH: f32 = 400.0;

/// Sección Buttons, con el layout del mockup (frame 9): el texto suelto antes de un botón (para
/// comparar las líneas base), la columna de los cuatro estados con su nombre 9 px a la derecha, y
/// de un botón al siguiente 19 filas.
const BASELINE_TEXT: &str = "Button baseline text";
const SAMPLE: &str = "Button";
const BASELINE_GAP: f32 = 12.0;
const STATE_PITCH: f32 = 19.0;
const STATE_LABEL_GAP: f32 = 9.0;
/// Del primer botón de la columna a la fila de arriba (la de la línea base).
const STATES_TOP: f32 = 27.0;
/// Las medidas anotadas, a la derecha de la columna.
const NOTES_LEFT: f32 = 160.0;
const NOTE_PITCH: f32 = 9.0;
/// El botón de verdad, debajo de la columna.
const LIVE_LABEL: &str = "Click me";

/// En la memoria de egui: cuántas veces se hizo clic en el botón de verdad.
fn clicks_id() -> egui::Id {
    egui::Id::new("ui_catalog_clicks")
}

/// La página entera, en el panel central (`ui`), con el fondo `canvas` del skin.
pub fn show(ui: &mut Ui, skin: &Skin) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let origin = ui.cursor().min.round() + MARGIN;
            let bottom = buttons_section(ui, skin, origin);
            // Lo que ocupa la página, para el scroll.
            let size = vec2(
                PAGE_WIDTH + 2.0 * MARGIN.x,
                bottom - ui.cursor().top() + MARGIN.y,
            );
            ui.allocate_exact_size(size, Sense::hover());
        });
}

/// El título de una sección con su línea grabada debajo, desde `top`. Devuelve dónde empieza el
/// contenido.
fn section_title(ui: &Ui, skin: &Skin, left: f32, top: f32, title: &str) -> f32 {
    let colors = skin.colors();
    let ppp = ui.ctx().pixels_per_point();
    let (text, _) = pixel_draw::caps_text(ui, title, left, top, colors.home_text);
    let line = top + TITLE_TO_LINE;
    ui.painter().add(text);
    ui.painter().add(crate::pixel_ui::etched_hline(
        egui::Rangef::new(left, left + PAGE_WIDTH),
        line,
        colors,
        ppp,
    ));
    line + crate::pixel_ui::ETCHED_SIZE + LINE_TO_CONTENT
}

/// La sección Buttons desde `origin`. Devuelve dónde termina.
fn buttons_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let top = section_title(ui, skin, origin.x, origin.y, "Buttons");
    let left = origin.x;
    let mut shapes: Vec<Shape> = Vec::new();
    // La línea base: el texto suelto con sus mayúsculas en la fila de las del botón.
    let (text, text_w) =
        pixel_draw::caps_text(ui, BASELINE_TEXT, left, top + button::CAPS_TOP, colors.text);
    shapes.push(text);
    let width = button::width(ui, SAMPLE);
    let at = pos2(left + text_w + BASELINE_GAP, top);
    let rect = Rect::from_min_size(at, vec2(width, button::HEIGHT));
    shapes.extend(button::button_shapes(
        skin,
        rect,
        SAMPLE,
        ButtonState::Normal,
        ui,
    ));
    // Los cuatro estados, forzados, con su nombre a la derecha.
    let column = left;
    let states_top = top + STATES_TOP;
    for (i, state) in ButtonState::ALL.into_iter().enumerate() {
        let at = pos2(column, states_top + i as f32 * STATE_PITCH);
        let rect = Rect::from_min_size(at, vec2(width, button::HEIGHT));
        shapes.extend(button::button_shapes(skin, rect, SAMPLE, state, ui));
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            rect.right() + STATE_LABEL_GAP,
            at.y + button::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
    }
    // Las medidas, en los textos chicos.
    let notes = [
        format!("h {}", button::HEIGHT),
        format!("pad {}", button::PAD_X),
        format!("caps {}", button::CAPS_TOP),
    ];
    for (i, note) in notes.iter().enumerate() {
        let (shape, _) = pixel_draw::caps_text_in(
            ui,
            TextStyle::Small,
            note,
            column + NOTES_LEFT,
            states_top + button::CAPS_TOP + i as f32 * NOTE_PITCH,
            colors.text_dim,
        );
        shapes.push(shape);
    }
    ui.painter().extend(shapes);
    // El de verdad: muestra el puntero encima y el presionado al usarlo, y cuenta los clics.
    let live_top = states_top + ButtonState::ALL.len() as f32 * STATE_PITCH + STATE_PITCH;
    let rect = Rect::from_min_size(
        pos2(column, live_top),
        vec2(button::width(ui, LIVE_LABEL), button::HEIGHT),
    );
    let mut clicks: u32 = ui.data(|d| d.get_temp(clicks_id()).unwrap_or_default());
    let id = ui.id().with("ui_catalog_live");
    if button::button_at(ui, skin, rect, id, LIVE_LABEL).clicked() {
        clicks += 1;
        ui.data_mut(|d| d.insert_temp(clicks_id(), clicks));
    }
    let (count, _) = pixel_draw::caps_text(
        ui,
        &format!("Clicked {clicks}×"),
        rect.right() + STATE_LABEL_GAP,
        live_top + button::CAPS_TOP,
        colors.text,
    );
    ui.painter().add(count);
    live_top + button::HEIGHT
}
