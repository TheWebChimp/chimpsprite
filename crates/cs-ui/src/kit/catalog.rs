//! La pestaña UI Catalog: una página con scroll, sobre el fondo `background` del skin (#ddcebf,
//! el que pidió el diseñador), con una sección por componente del kit. Cada componente nuevo
//! agrega la suya.

use eframe::egui::{self, Pos2, Rect, Sense, Shape, TextStyle, Ui, Vec2, pos2, vec2};

use super::button::{self, ButtonState, Led};
use super::group::{self, GroupItem, SegmentLook};
use super::input::{self, Affixes, InputState, NumberOptions};
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
/// Los botones con LED: una columna prendida y otra apagada, en los cuatro estados, a la derecha
/// de las medidas; debajo, uno de verdad que prende y apaga su LED.
const LED_COLUMN: f32 = 230.0;
const LED_COLUMN_GAP: f32 = 12.0;
const LED_ON_LABEL: &str = "LED on";
const LED_OFF_LABEL: &str = "LED off";
const LED_LIVE_LABEL: &str = "Toggle LED";

/// De una sección a la siguiente y de una columna de secciones a la otra.
const SECTION_GAP: f32 = 24.0;
const COLUMN_GAP: f32 = 24.0;

/// Sección Button group, como en el mockup: el grupo de dos "Button" y, en la fila siguiente, el
/// mismo y 22 px a la derecha el de los LEDs (el primero elegido); de un grupo al siguiente,
/// 21 filas.
const GROUP_PITCH: f32 = 21.0;
const GROUP_GAP: f32 = 22.0;
/// El grupo de verdad, con una opción elegida a la vez.
const LIVE_OPTIONS: [&str; 3] = ["Fast", "RotSprite", "Smooth"];

/// Sección Inputs, como en el mockup: los campos de "Input Placeholder" (109 de ancho) normal,
/// con el foco, con todo elegido y deshabilitado, uno cada 21 filas; el numérico con "px" y el de
/// la letra X (42 de ancho); y, debajo, unos de verdad.
const INPUT_SAMPLE: &str = "Input Placeholder";
const INPUT_WIDTH: f32 = 109.0;
const NUMBER_WIDTH: f32 = 42.0;
const INPUT_PITCH: f32 = 21.0;
/// Del campo a su nombre y de la columna de los textos a la de los números.
const INPUT_NUMBERS_LEFT: f32 = 220.0;

/// En la memoria de egui: lo escrito en los campos de verdad.
fn live_text_id() -> egui::Id {
    egui::Id::new("ui_catalog_input_text")
}

fn live_numbers_id() -> egui::Id {
    egui::Id::new("ui_catalog_input_numbers")
}

/// En la memoria de egui: la opción elegida del grupo de verdad.
fn picked_id() -> egui::Id {
    egui::Id::new("ui_catalog_group_pick")
}

/// En la memoria de egui: si está prendido el LED del botón de verdad.
fn led_id() -> egui::Id {
    egui::Id::new("ui_catalog_led")
}

/// En la memoria de egui: cuántas veces se hizo clic en el botón de verdad.
fn clicks_id() -> egui::Id {
    egui::Id::new("ui_catalog_clicks")
}

/// La página entera, en el panel central (`ui`), con el fondo `background` del skin.
pub fn show(ui: &mut Ui, skin: &Skin) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let origin = ui.cursor().min.round() + MARGIN;
            let bottom = buttons_section(ui, skin, origin);
            let bottom = group_section(ui, skin, pos2(origin.x, bottom + SECTION_GAP));
            // La segunda columna, a la derecha de la primera.
            let second = origin.x + PAGE_WIDTH + COLUMN_GAP;
            let right = inputs_section(ui, skin, pos2(second, origin.y));
            // Lo que ocupa la página, para el scroll.
            let size = vec2(
                2.0 * PAGE_WIDTH + COLUMN_GAP + 2.0 * MARGIN.x,
                bottom.max(right) - ui.cursor().top() + MARGIN.y,
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
        // Con LED, prendido y apagado, en el mismo estado.
        let mut x = column + LED_COLUMN;
        for (label, led) in [(LED_ON_LABEL, Led::On), (LED_OFF_LABEL, Led::Off)] {
            let w = button::width_with_led(ui, label, Some(led));
            let rect = Rect::from_min_size(pos2(x, at.y), vec2(w, button::HEIGHT));
            shapes.extend(button::button_shapes_with_led(
                skin,
                rect,
                label,
                Some(led),
                state,
                ui,
            ));
            x += w + LED_COLUMN_GAP;
        }
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
    // El de verdad con LED: cada clic lo prende o lo apaga.
    let mut lit: bool = ui.data(|d| d.get_temp(led_id()).unwrap_or_default());
    let led = Led::lit(lit);
    let rect = Rect::from_min_size(
        pos2(column + LED_COLUMN, live_top),
        vec2(
            button::width_with_led(ui, LED_LIVE_LABEL, Some(led)),
            button::HEIGHT,
        ),
    );
    let clicked = ui
        .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            button::led_button(ui, skin, LED_LIVE_LABEL, led).clicked()
        })
        .inner;
    if clicked {
        lit = !lit;
        ui.data_mut(|d| d.insert_temp(led_id(), lit));
    }
    live_top + button::HEIGHT
}

/// La sección Button group desde `origin`. Devuelve dónde termina.
fn group_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let top = section_title(ui, skin, origin.x, origin.y, "Button group");
    let left = origin.x;
    let plain = [GroupItem::text(SAMPLE), GroupItem::text(SAMPLE)];
    // Con LED: prendido el de la opción elegida.
    let with_leds = |on: [bool; 2]| on.map(|on| GroupItem::with_led(SAMPLE, Led::lit(on)));
    let look = |state, on| SegmentLook { state, on };
    let normal = [look(ButtonState::Normal, false); 2];
    let first_on = [
        look(ButtonState::Normal, true),
        look(ButtonState::Normal, false),
    ];
    let mut shapes: Vec<Shape> = Vec::new();
    let name = |shapes: &mut Vec<Shape>, text: &str, x: f32, y: f32| {
        let (shape, _) = pixel_draw::caps_text(
            ui,
            text,
            x + STATE_LABEL_GAP,
            y + button::CAPS_TOP,
            colors.text,
        );
        shapes.push(shape);
    };
    // Como en el mockup: el grupo solo y, debajo, el mismo y el de los LEDs.
    let plain_w = group::width(ui, &plain);
    shapes.extend(group::group_shapes(
        ui,
        skin,
        pos2(left, top),
        &plain,
        &normal,
    ));
    let row = top + GROUP_PITCH;
    shapes.extend(group::group_shapes(
        ui,
        skin,
        pos2(left, row),
        &plain,
        &normal,
    ));
    let leds_left = left + plain_w + GROUP_GAP;
    let leds = with_leds([true, false]);
    shapes.extend(group::group_shapes(
        ui,
        skin,
        pos2(leds_left, row),
        &leds,
        &first_on,
    ));
    name(&mut shapes, "LED", leds_left + group::width(ui, &leds), row);
    // Los estados, forzados: el primero elegido, con el puntero encima del segundo y deshabilitado.
    let states = [
        ("Active", first_on),
        (
            "Hover",
            [
                look(ButtonState::Normal, true),
                look(ButtonState::Hover, false),
            ],
        ),
        (
            "Disabled",
            [
                look(ButtonState::Disabled, true),
                look(ButtonState::Disabled, false),
            ],
        ),
    ];
    let mut y = row + GROUP_PITCH;
    for (label, looks) in states {
        let items = with_leds(looks.map(|l| l.on));
        shapes.extend(group::group_shapes(ui, skin, pos2(left, y), &items, &looks));
        name(&mut shapes, label, left + group::width(ui, &items), y);
        y += GROUP_PITCH;
    }
    ui.painter().extend(shapes);
    // El de verdad: una opción elegida a la vez.
    let mut picked: usize = ui.data(|d| d.get_temp(picked_id()).unwrap_or_default());
    let items: [GroupItem; 3] =
        std::array::from_fn(|i| GroupItem::with_led(LIVE_OPTIONS[i], Led::lit(i == picked)));
    let rect = Rect::from_min_size(
        pos2(left, y),
        vec2(group::width(ui, &items), button::HEIGHT),
    );
    let clicked = ui
        .scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            super::button_group(ui, skin, &items, &|i| i == picked)
        })
        .inner;
    if let Some(i) = clicked {
        picked = i;
        ui.data_mut(|d| d.insert_temp(picked_id(), picked));
    }
    let (text, _) = pixel_draw::caps_text(
        ui,
        &format!("Picked {}", LIVE_OPTIONS[picked]),
        rect.right() + STATE_LABEL_GAP,
        y + button::CAPS_TOP,
        colors.text,
    );
    ui.painter().add(text);
    y + button::HEIGHT
}

/// La sección Inputs desde `origin`. Devuelve dónde termina.
fn inputs_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let top = section_title(ui, skin, origin.x, origin.y, "Inputs");
    let left = origin.x;
    let mut shapes: Vec<Shape> = Vec::new();
    let name = |shapes: &mut Vec<Shape>, text: &str, x: f32, y: f32| {
        let (shape, _) = pixel_draw::caps_text(
            ui,
            text,
            x + STATE_LABEL_GAP,
            y + input::CAPS_TOP,
            colors.text,
        );
        shapes.push(shape);
    };
    // Los campos de texto, forzados.
    let all = INPUT_SAMPLE.chars().count();
    let rows: [(&str, InputState, Option<std::ops::Range<usize>>); 4] = [
        (InputState::Normal.name(), InputState::Normal, None),
        (InputState::Focused.name(), InputState::Focused, None),
        ("Selected text", InputState::Focused, Some(0..all)),
        (InputState::Disabled.name(), InputState::Disabled, None),
    ];
    let mut y = top;
    for (label, state, selected) in rows {
        let rect = Rect::from_min_size(pos2(left, y), vec2(INPUT_WIDTH, input::HEIGHT));
        shapes.extend(input::input_shapes(
            ui,
            skin,
            rect,
            INPUT_SAMPLE,
            "",
            state,
            selected,
            Affixes::NONE,
        ));
        name(&mut shapes, label, rect.right(), y);
        y += INPUT_PITCH;
    }
    // Los numéricos: con la unidad y con la letra adelante.
    let numbers: [(&str, &str, Affixes); 2] = [
        ("Unit (px)", "16", Affixes::suffix("px")),
        ("Prefix label (X)", "11", Affixes::prefix("X")),
    ];
    let x = left + INPUT_NUMBERS_LEFT;
    for (i, (label, value, affixes)) in numbers.into_iter().enumerate() {
        let at = pos2(x, top + i as f32 * INPUT_PITCH);
        let rect = Rect::from_min_size(at, vec2(NUMBER_WIDTH, input::HEIGHT));
        shapes.extend(input::input_shapes(
            ui,
            skin,
            rect,
            value,
            "",
            InputState::Normal,
            None,
            affixes,
        ));
        name(&mut shapes, label, rect.right(), at.y);
    }
    ui.painter().extend(shapes);
    // Los de verdad: un texto, un número con "px" y uno con la X.
    let mut text: String = ui.data(|d| {
        d.get_temp(live_text_id())
            .unwrap_or_else(|| "Edit me".to_owned())
    });
    let mut values: (i32, i32) = ui.data(|d| d.get_temp(live_numbers_id()).unwrap_or((16, 11)));
    let rect = Rect::from_min_size(pos2(left, y), vec2(INPUT_WIDTH, input::HEIGHT));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        input::text_input(ui, skin, &mut text, INPUT_WIDTH, "Live text", "Type here")
    });
    for (i, (affixes, value)) in [
        (Affixes::suffix("px"), &mut values.0),
        (Affixes::prefix("X"), &mut values.1),
    ]
    .into_iter()
    .enumerate()
    {
        let at = pos2(x + i as f32 * (NUMBER_WIDTH + STATE_LABEL_GAP), y);
        let rect = Rect::from_min_size(at, vec2(NUMBER_WIDTH, input::HEIGHT));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            let options = NumberOptions {
                affixes,
                ..Default::default()
            };
            input::number_input(
                ui,
                skin,
                NUMBER_WIDTH,
                &format!("live{i}"),
                value,
                0..=999,
                options,
            )
        });
    }
    ui.data_mut(|d| {
        d.insert_temp(live_text_id(), text);
        d.insert_temp(live_numbers_id(), values);
    });
    y + input::HEIGHT
}
