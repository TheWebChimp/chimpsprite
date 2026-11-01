//! La pestaña UI Catalog: una página con scroll, sobre el fondo `background` del skin (#ddcebf,
//! el que pidió el diseñador), con una sección por componente del kit. Cada componente nuevo
//! agrega la suya.

use eframe::egui::{self, Pos2, Rect, Sense, Shape, TextStyle, Ui, Vec2, pos2, vec2};

use super::button::{self, ButtonState, Led};
use super::group::{self, GroupItem, SegmentLook};
use super::input::{self, Affixes, InputState, NumberOptions};
use super::tabs;
use crate::pixel_draw;
use crate::skin::Skin;

/// El nombre de la pestaña.
pub const TITLE: &str = "UI Catalog";

/// Medidas de la página, desde la esquina de arriba a la izquierda del contenido.
const MARGIN: Vec2 = vec2(12.0, 12.0);
/// De la línea grabada de las pestañas al contenido de la sección.
const CONTENT_TOP: f32 = 12.0;
/// El ancho de la página (las secciones; la tira de pestañas va a todo lo ancho).
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

/// Las secciones, una por pestaña.
const SECTIONS: [&str; 13] = [
    "Buttons",
    "Button group",
    "Inputs",
    "Tabs",
    "Checks",
    "Slider",
    "Color",
    "Select",
    "Stepper",
    "Title bar",
    "Menu",
    "Scroll",
    "Dock",
];

/// En la memoria de egui: la sección a la vista.
fn section_id() -> egui::Id {
    egui::Id::new("ui_catalog_section")
}

/// La página entera, en el panel central (`ui`), con el fondo `background` del skin: arriba, las
/// pestañas del kit (estilo documento), una por sección, y debajo solo la elegida. Para las
/// capturas, `CHIMPSPRITE_DEMO_CATALOG=<sección>` abre esa.
pub fn show(ui: &mut Ui, skin: &Skin) {
    let start = crate::demo::dev_var("CHIMPSPRITE_DEMO_CATALOG")
        .and_then(|name| {
            SECTIONS
                .iter()
                .position(|s| s.eq_ignore_ascii_case(name.trim()))
        })
        .unwrap_or(0);
    let mut section: usize = ui.data(|d| d.get_temp(section_id()).unwrap_or(start));
    let origin = ui.max_rect().min.round() + MARGIN;
    // Las pestañas de las secciones, a todo lo ancho (con el margen a la derecha).
    let right = (ui.max_rect().right() - MARGIN.x).max(origin.x + PAGE_WIDTH);
    let strip = tabs::Strip::new(origin.x, right.round(), origin.y);
    let items: Vec<tabs::TabItem> = SECTIONS.iter().map(|s| tabs::TabItem::new(*s)).collect();
    let id = ui.id().with("ui_catalog_tabs");
    if let Some(tabs::TabEvent::Select(i)) =
        tabs::tab_strip(ui, skin, id, strip, &items, Some(section))
    {
        section = i;
    }
    ui.data_mut(|d| d.insert_temp(section_id(), section));
    let header = strip.bottom() + crate::pixel_ui::ETCHED_SIZE - ui.max_rect().top();
    ui.add_space(header);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let origin = pos2(origin.x, ui.cursor().min.round().y + CONTENT_TOP);
            let bottom = match section {
                0 => buttons_section(ui, skin, origin),
                1 => group_section(ui, skin, origin),
                2 => inputs_section(ui, skin, origin),
                3 => tabs_section(ui, skin, origin),
                4 => checks_section(ui, skin, origin),
                5 => slider_section(ui, skin, origin),
                6 => color_section(ui, skin, origin),
                7 => select_section(ui, skin, origin),
                8 => stepper_section(ui, skin, origin),
                9 => title_section(ui, skin, origin),
                10 => menu_section(ui, skin, origin),
                11 => scroll_section(ui, skin, origin),
                _ => dock_section(ui, skin, origin),
            };
            // Lo que ocupa la página, para el scroll.
            let size = vec2(
                PAGE_WIDTH + 2.0 * MARGIN.x,
                bottom - ui.cursor().top() + MARGIN.y,
            );
            ui.allocate_exact_size(size, Sense::hover());
        });
}

/// La sección Buttons desde `origin`. Devuelve dónde termina.
fn buttons_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let top = origin.y;
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
    // Los botones con ícono (el de copiar del selector de color): el mismo marco y sus estados,
    // forzados, y uno de verdad al final.
    let icons_top = live_top + 2.0 * STATE_PITCH;
    let mut shapes = Vec::new();
    let mut x = column;
    for state in ButtonState::ALL {
        let rect = Rect::from_min_size(pos2(x, icons_top), ICON_BUTTON_SIZE);
        shapes.extend(button::frame_shapes(
            skin,
            rect,
            state,
            ui.ctx().pixels_per_point(),
        ));
        shapes.extend(button::icon_shapes(ui, skin, rect, ICON_SAMPLE, state));
        x += ICON_BUTTON_SIZE.x + LED_COLUMN_GAP;
    }
    ui.painter().extend(shapes);
    let rect = Rect::from_min_size(pos2(x + LED_COLUMN_GAP, icons_top), ICON_BUTTON_SIZE);
    let id = ui.id().with("ui_catalog_icon_live");
    button::icon_button_at(ui, skin, rect, id, ICON_SAMPLE, "Icon button");
    let (note, _) = pixel_draw::caps_text(
        ui,
        "Icon button",
        rect.right() + STATE_LABEL_GAP,
        icons_top + button::CAPS_TOP,
        colors.text,
    );
    ui.painter().add(note);
    icons_top + button::HEIGHT
}

/// Los botones con ícono de la sección Buttons: el de copiar del selector, de 25 de ancho.
const ICON_SAMPLE: &str = "icon.copy";
const ICON_BUTTON_SIZE: Vec2 = vec2(25.0, button::HEIGHT);

/// La sección Button group desde `origin`. Devuelve dónde termina.
fn group_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let top = origin.y;
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
    let top = origin.y;
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

/// Sección Tabs, como en el mockup: las pestañas de documento (la activa "Design.chimp" y la otra
/// "Otro.chimp", con cambios) sobre su línea grabada, que sigue 5 px antes y después; la misma
/// tira con las flechas; y una tira de verdad.
const DOC_TABS: [&str; 2] = ["Design.chimp", "Otro.chimp"];
const LINE_OVERHANG: f32 = 5.0;
const ARROWS_LINE_OVERHANG: f32 = 6.0;
const TABS_PITCH: f32 = 30.0;
const LIVE_TABS: [&str; 9] = [
    "Sprite.chimp",
    "Tiles.chimp",
    "Hero.chimp",
    "Map.chimp",
    "UI.chimp",
    "Enemies.chimp",
    "Items.chimp",
    "Title.chimp",
    "Credits.chimp",
];

/// En la memoria de egui: las pestañas de verdad que quedan y la activa.
fn live_tabs_id() -> egui::Id {
    egui::Id::new("ui_catalog_live_tabs")
}

/// Una tira de documento dibujada desde `left` (sin interacción): la primera activa y, si
/// `arrows`, las flechas después de la última. Devuelve dónde termina a lo ancho.
fn forced_strip(ui: &Ui, skin: &Skin, left: f32, top: f32, arrows: bool) -> Vec<Shape> {
    let strip = tabs::Strip::new(left, left + PAGE_WIDTH, top);
    let items: Vec<tabs::TabItem> = DOC_TABS
        .iter()
        .map(|t| tabs::TabItem::closable(*t, true))
        .collect();
    let lay = tabs::layout(ui, &strip, &items, 0);
    let last = lay.placed.last().map_or(left, |p| p.rect.right());
    let arrows_rect = arrows.then(|| {
        Rect::from_min_size(
            pos2(last + tabs::GAP, top + tabs::ARROWS_TOP),
            vec2(tabs::ARROWS_WIDTH, tabs::ARROW_SIZE),
        )
    });
    let right = match arrows_rect {
        Some(a) => a.right() + ARROWS_LINE_OVERHANG,
        None => last + LINE_OVERHANG,
    };
    let mut line = strip;
    line.line = egui::Rangef::new(left - LINE_OVERHANG, right);
    let active = lay
        .placed
        .first()
        .map(|p| p.rect.with_max_y(p.rect.top() + tabs::ACTIVE_HEIGHT));
    let mut shapes = tabs::line_shapes(ui, skin, &line, active);
    for (i, p) in lay.placed.iter().enumerate().rev() {
        let look = tabs::TabLook {
            active: i == 0,
            hovered: false,
            mark_hovered: false,
        };
        shapes.extend(tabs::tab_shapes(
            ui,
            skin,
            p.rect,
            &items[p.index],
            &p.name,
            look,
        ));
    }
    if let Some(a) = arrows_rect {
        shapes.extend(tabs::arrows_shapes(
            ui,
            skin,
            a,
            (false, true),
            (false, true),
        ));
    }
    shapes
}

/// La sección Tabs desde `origin`. Devuelve dónde termina.
fn tabs_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    // El mockup deja 5 px de línea antes de la primera pestaña.
    let left = origin.x + LINE_OVERHANG;
    let mut top = origin.y;
    let mut shapes = forced_strip(ui, skin, left, top, false);
    top += TABS_PITCH;
    shapes.extend(forced_strip(ui, skin, left, top, true));
    top += TABS_PITCH;
    ui.painter().extend(shapes);
    // Una tira de verdad: se eligen y se cierran (con la × o el botón del medio).
    top += TABS_PITCH - tabs::HEIGHT;
    let (mut open, mut active): (Vec<usize>, Option<usize>) = ui.data(|d| {
        d.get_temp(live_tabs_id())
            .unwrap_or(((0..LIVE_TABS.len()).collect(), Some(0)))
    });
    let items: Vec<tabs::TabItem> = open
        .iter()
        .map(|&i| tabs::TabItem::closable(LIVE_TABS[i], i == 1))
        .collect();
    let strip = tabs::Strip::new(left, left + PAGE_WIDTH, top);
    match tabs::tab_strip(
        ui,
        skin,
        ui.id().with("ui_catalog_live_strip"),
        strip,
        &items,
        active,
    ) {
        Some(tabs::TabEvent::Select(i)) => active = Some(i),
        Some(tabs::TabEvent::Close(i)) => {
            open.remove(i);
            active = match active {
                _ if open.is_empty() => None,
                Some(a) if a >= i => Some(a.saturating_sub(1).min(open.len() - 1)),
                other => other,
            };
        }
        None => {}
    }
    if open.is_empty() {
        open = (0..LIVE_TABS.len()).collect();
        active = Some(0);
    }
    ui.data_mut(|d| d.insert_temp(live_tabs_id(), (open, active)));
    top = strip.bottom() + crate::pixel_ui::ETCHED_SIZE + 4.0;
    let (note, _) = pixel_draw::caps_text_in(
        ui,
        TextStyle::Small,
        "click a tab to pick it, the x to close it",
        left,
        top,
        colors.text_dim,
    );
    ui.painter().add(note);
    top + 10.0
}

/// Sección Checks, como en el mockup: las casillas marcada, intermedia y vacía (una cada 16) y,
/// 21 px después, los radios apagado y prendido (uno cada 14), en una fila por estado con su
/// nombre a la derecha; debajo, unas de verdad: dos casillas con la de "All" intermedia cuando
/// una sí y otra no, y un grupo de radios.
const CHECK_PITCH: f32 = 16.0;
const CHECK_RADIO_GAP: f32 = 21.0;
const RADIO_PITCH: f32 = 14.0;
const LIVE_RADIOS: [&str; 3] = ["Layer", "Frame", "Sprite"];

/// En la memoria de egui: las casillas y el radio de verdad.
fn live_checks_id() -> egui::Id {
    egui::Id::new("ui_catalog_checks")
}

fn checks_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::check::{self, Check, Mark};
    let colors = *skin.colors();
    let left = origin.x;
    let mut y = origin.y;
    let mut shapes: Vec<Shape> = Vec::new();
    let states = [
        ButtonState::Normal,
        ButtonState::Hover,
        ButtonState::Disabled,
    ];
    for state in states {
        let mut x = left;
        for value in Check::ALL {
            shapes.extend(check::shapes(
                ui,
                skin,
                pos2(x, y),
                Mark::Check(value),
                "",
                state,
            ));
            x += CHECK_PITCH;
        }
        // Del último a los radios, 21 (de la x de uno a la del otro).
        x += CHECK_RADIO_GAP - CHECK_PITCH;
        for on in [false, true] {
            shapes.extend(check::shapes(
                ui,
                skin,
                pos2(x, y),
                Mark::Radio(on),
                "",
                state,
            ));
            x += RADIO_PITCH;
        }
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            x + STATE_LABEL_GAP,
            y + check::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
        y += STATE_PITCH;
    }
    ui.painter().extend(shapes);
    // Las de verdad.
    y += STATE_PITCH;
    let (mut a, mut b, mut pick): (bool, bool, usize) =
        ui.data(|d| d.get_temp(live_checks_id()).unwrap_or((true, false, 0)));
    let rect = Rect::from_min_size(pos2(left, y), vec2(PAGE_WIDTH, 3.0 * STATE_PITCH));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.spacing_mut().item_spacing = vec2(12.0, STATE_PITCH - check::HEIGHT);
        let all = match (a, b) {
            (true, true) => Check::On,
            (false, false) => Check::Off,
            _ => Check::Mixed,
        };
        if check::tri_checkbox(ui, skin, all, "All layers").clicked() {
            let next = all != Check::On;
            (a, b) = (next, next);
        }
        ui.horizontal(|ui| {
            check::checkbox(ui, skin, &mut a, "Background");
            check::checkbox(ui, skin, &mut b, "Sketch");
            ui.add_enabled_ui(false, |ui| {
                check::checkbox(ui, skin, &mut true, "Locked");
            });
        });
        ui.horizontal(|ui| {
            for (i, label) in LIVE_RADIOS.iter().enumerate() {
                check::radio_value(ui, skin, &mut pick, i, label);
            }
        });
    });
    ui.data_mut(|d| d.insert_temp(live_checks_id(), (a, b, pick)));
    y + 3.0 * STATE_PITCH
}

/// Sección Slider, como en el mockup: la pista de 140 con la perilla 15 px adentro, en una fila
/// por estado con su nombre a la derecha; debajo, uno de verdad con su número al lado.
const SLIDER_WIDTH: f32 = 140.0;
const SLIDER_MOCKUP_LEFT: f32 = 15.0;

/// En la memoria de egui: el valor del slider de verdad.
fn live_slider_id() -> egui::Id {
    egui::Id::new("ui_catalog_slider")
}

fn slider_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::slider;
    let colors = *skin.colors();
    let left = origin.x;
    let mut y = origin.y;
    let ppp = ui.ctx().pixels_per_point();
    let mut shapes: Vec<Shape> = Vec::new();
    let t = SLIDER_MOCKUP_LEFT / (SLIDER_WIDTH - 13.0);
    for state in [
        ButtonState::Normal,
        ButtonState::Hover,
        ButtonState::Disabled,
    ] {
        let rect = Rect::from_min_size(pos2(left, y), vec2(SLIDER_WIDTH, slider::HEIGHT));
        shapes.extend(slider::slider_shapes(skin, rect, t, state, ppp));
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            rect.right() + STATE_LABEL_GAP,
            y + button::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
        y += STATE_PITCH;
    }
    ui.painter().extend(shapes);
    // El de verdad, con el número al lado (los dos cambian el mismo valor).
    y += STATE_PITCH;
    let mut value: i32 = ui.data(|d| d.get_temp(live_slider_id()).unwrap_or(32));
    let rect = Rect::from_min_size(pos2(left, y), vec2(PAGE_WIDTH, slider::HEIGHT));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = STATE_LABEL_GAP;
            slider::slider(ui, skin, SLIDER_WIDTH, "live", &mut value, 0..=255);
            let options = NumberOptions {
                affixes: Affixes::NONE,
                ..Default::default()
            };
            input::number_input(
                ui,
                skin,
                NUMBER_WIDTH,
                "live_slider",
                &mut value,
                0..=255,
                options,
            );
        });
    });
    ui.data_mut(|d| d.insert_temp(live_slider_id(), value));
    let (note, _) = pixel_draw::caps_text_in(
        ui,
        TextStyle::Small,
        "drag it, click the track, or tab to it and use the arrows",
        left,
        y + slider::HEIGHT + 4.0,
        colors.text_dim,
    );
    ui.painter().add(note);
    // El botón de restaurar en sus cuatro estados (el ↺ se porta como el texto) y, debajo, el
    // slider con encabezado de verdad, como en el mockup ("Height", 24px).
    let mut y = y + slider::HEIGHT + 14.0 + STATE_PITCH;
    let mut shapes = Vec::new();
    let mut x = left;
    for state in ButtonState::ALL {
        let rect = Rect::from_min_size(pos2(x, y), slider::RESET_SIZE);
        shapes.extend(button::frame_shapes(skin, rect, state, ppp));
        shapes.extend(button::icon_shapes(
            ui,
            skin,
            rect,
            slider::RESET_ICON,
            state,
        ));
        x += slider::RESET_SIZE.x + STATE_LABEL_GAP;
    }
    ui.painter().extend(shapes);
    let (name, _) = pixel_draw::caps_text(ui, "Reset button", x, y + button::CAPS_TOP, colors.text);
    ui.painter().add(name);
    y += STATE_PITCH + 4.0;
    let mut height: i32 = ui.data(|d| d.get_temp(live_labeled_id()).unwrap_or(24));
    let rect = Rect::from_min_size(pos2(left, y), vec2(SLIDER_WIDTH, slider::LABELED_HEIGHT));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        slider::labeled_slider(
            ui,
            skin,
            SLIDER_WIDTH,
            "live_labeled",
            "Height",
            &mut height,
            1..=200,
            24,
            |v| format!("{v}px"),
        );
    });
    ui.data_mut(|d| d.insert_temp(live_labeled_id(), height));
    y + slider::LABELED_HEIGHT
}

/// En la memoria de egui: el valor del slider con encabezado de verdad.
fn live_labeled_id() -> egui::Id {
    egui::Id::new("ui_catalog_labeled_slider")
}

/// Sección Color button: el amarillo del mockup en los cuatro estados (uno por fila, con su nombre
/// a la derecha) y, al lado, uno con alfa; debajo, dos de verdad (uno opaco y otro con alfa) que
/// abren el selector de la app.
const COLOR_SAMPLE: cs_core::Rgba = cs_core::Rgba::rgb(255, 214, 120);
const COLOR_ALPHA_SAMPLE: cs_core::Rgba = cs_core::Rgba::new(226, 114, 133, 128);
const COLOR_PITCH: f32 = 22.0;

/// En la memoria de egui: los colores de los de verdad.
fn live_colors_id() -> egui::Id {
    egui::Id::new("ui_catalog_colors")
}

fn color_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::color_button;
    let colors = *skin.colors();
    let left = origin.x;
    let mut y = origin.y;
    let ppp = ui.ctx().pixels_per_point();
    let size = color_button::size(skin);
    let mut shapes: Vec<Shape> = Vec::new();
    for state in ButtonState::ALL {
        for (i, color) in [COLOR_SAMPLE, COLOR_ALPHA_SAMPLE].into_iter().enumerate() {
            let rect = Rect::from_min_size(pos2(left + i as f32 * COLOR_PITCH, y), size);
            shapes.extend(color_button::swatch_shapes(skin, rect, color, state, ppp));
        }
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            left + COLOR_PITCH + size.x + STATE_LABEL_GAP,
            y + button::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
        y += STATE_PITCH;
    }
    ui.painter().extend(shapes);
    // Los de verdad, cada uno con su hex.
    y += STATE_PITCH;
    let (mut opaque, mut tint): (cs_core::Rgba, cs_core::Rgba) = ui.data(|d| {
        d.get_temp(live_colors_id())
            .unwrap_or((COLOR_SAMPLE, COLOR_ALPHA_SAMPLE))
    });
    let rect = Rect::from_min_size(pos2(left, y), vec2(PAGE_WIDTH, button::HEIGHT));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = STATE_LABEL_GAP;
            for (color, alpha, label) in
                [(&mut opaque, false, "Opaque"), (&mut tint, true, "Alpha")]
            {
                color_button::color_edit(ui, skin, color, alpha, label);
                let hex = format!(
                    "{label} #{:02X}{:02X}{:02X}{:02X}",
                    color.r, color.g, color.b, color.a
                );
                let (text, w) = pixel_draw::caps_text(
                    ui,
                    &hex,
                    ui.cursor().left(),
                    y + button::CAPS_TOP,
                    colors.text,
                );
                ui.painter().add(text);
                ui.add_space(w + 2.0 * STATE_LABEL_GAP);
            }
        });
    });
    ui.data_mut(|d| d.insert_temp(live_colors_id(), (opaque, tint)));
    y + button::HEIGHT
}

/// Sección Select, como en el mockup: el cerrado (109 de ancho) normal y deshabilitado, uno cada
/// 21 filas con su nombre a la derecha; a la derecha, el abierto con tres opciones (la primera
/// con el puntero encima); debajo, uno de verdad.
const SELECT_SAMPLE: &str = "Select an option…";
const SELECT_WIDTH: f32 = 109.0;
const SELECT_OPEN_LEFT: f32 = 200.0;
const SELECT_OPTIONS: [&str; 3] = ["Option 1", "Option 2", "Option 3"];
const LIVE_SELECT: [&str; 4] = ["Nearest", "Bilinear", "RotSprite", "Smooth edges"];

/// En la memoria de egui: la opción elegida del select de verdad.
fn live_select_id() -> egui::Id {
    egui::Id::new("ui_catalog_select")
}

fn select_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::select;
    let colors = *skin.colors();
    let left = origin.x;
    let mut y = origin.y;
    let mut shapes: Vec<Shape> = Vec::new();
    for state in [InputState::Normal, InputState::Disabled] {
        let rect = Rect::from_min_size(pos2(left, y), vec2(SELECT_WIDTH, select::HEIGHT));
        shapes.extend(select::forced_shapes(
            ui,
            skin,
            rect,
            SELECT_SAMPLE,
            state,
            &[],
            None,
        ));
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            rect.right() + STATE_LABEL_GAP,
            y + select::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
        y += INPUT_PITCH;
    }
    let open = Rect::from_min_size(
        pos2(left + SELECT_OPEN_LEFT, origin.y),
        vec2(SELECT_WIDTH, select::HEIGHT),
    );
    shapes.extend(select::forced_shapes(
        ui,
        skin,
        open,
        SELECT_SAMPLE,
        InputState::Normal,
        &SELECT_OPTIONS,
        Some(0),
    ));
    ui.painter().extend(shapes);
    // El de verdad.
    let list_bottom = open.top()
        + select::LIST_TOP
        + select::LIST_PAD_TOP
        + SELECT_OPTIONS.len() as f32 * select::ITEM_HEIGHT
        + select::LIST_PAD_BOTTOM;
    let y = y.max(list_bottom) + INPUT_PITCH;
    let mut pick: usize = ui.data(|d| d.get_temp(live_select_id()).unwrap_or(0));
    let rect = Rect::from_min_size(pos2(left, y), vec2(PAGE_WIDTH, select::HEIGHT));
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        select::select(
            ui,
            skin,
            "live",
            LIVE_SELECT[pick],
            Some(SELECT_WIDTH),
            |ui| {
                for (i, label) in LIVE_SELECT.iter().enumerate() {
                    crate::widgets::choice(ui, &mut pick, i, label);
                }
            },
        );
    });
    ui.data_mut(|d| d.insert_temp(live_select_id(), pick));
    // Uno con muchas opciones: la lista va debajo si entra (si no, del lado con más lugar) y lo
    // que no entra se recorre con el scroll.
    let long: Vec<String> = (1..=LONG_SELECT).map(|i| format!("Palette {i}")).collect();
    let mut pick: usize = ui.data(|d| d.get_temp(long_select_id()).unwrap_or(19));
    let rect = Rect::from_min_size(
        pos2(left + SELECT_OPEN_LEFT, y),
        vec2(PAGE_WIDTH, select::HEIGHT),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        select::select(ui, skin, "long", &long[pick], Some(SELECT_WIDTH), |ui| {
            for (i, label) in long.iter().enumerate() {
                crate::widgets::choice(ui, &mut pick, i, label);
            }
        });
    });
    ui.data_mut(|d| d.insert_temp(long_select_id(), pick));
    y + select::HEIGHT
}

/// Las opciones del select largo.
const LONG_SELECT: usize = 40;

/// En la memoria de egui: la opción elegida del select largo.
fn long_select_id() -> egui::Id {
    egui::Id::new("ui_catalog_long_select")
}

/// Sección Scroll: una caja con 40 filas que no entran, con la barra de scroll del skin
/// (`scroll.track.v` y `scroll.thumb.v`).
const SCROLL_BOX: Vec2 = vec2(160.0, 120.0);

fn scroll_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let colors = *skin.colors();
    let rect = Rect::from_min_size(origin, SCROLL_BOX);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        let area = egui::ScrollArea::vertical()
            .id_salt("ui_catalog_scroll")
            .max_height(SCROLL_BOX.y)
            .auto_shrink([false, false]);
        crate::pixel_ui::scroll(ui, area, |ui| {
            for i in 1..=40 {
                ui.label(format!("Row {i}"));
            }
        });
    });
    let (note, _) = pixel_draw::caps_text_in(
        ui,
        TextStyle::Small,
        "drag the thumb or use the wheel",
        origin.x,
        rect.bottom() + 6.0,
        colors.text_dim,
    );
    ui.painter().add(note);
    rect.bottom() + 16.0
}

/// Sección Stepper, como en el mockup: − 255 + (58 de ancho) y − 155 ms + (70); una fila por
/// estado de los botones con su nombre a la derecha y uno deshabilitado; debajo, dos de verdad.
const STEPPER_WIDTH: f32 = 58.0;
const STEPPER_UNIT_WIDTH: f32 = 70.0;
const STEPPER_UNIT_LEFT: f32 = 120.0;

/// En la memoria de egui: los valores de los steppers de verdad.
fn live_steppers_id() -> egui::Id {
    egui::Id::new("ui_catalog_steppers")
}

fn stepper_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::stepper;
    let colors = *skin.colors();
    let left = origin.x;
    let mut y = origin.y;
    let mut shapes: Vec<Shape> = Vec::new();
    for state in ButtonState::ALL {
        let enabled = state != ButtonState::Disabled;
        let rect = Rect::from_min_size(pos2(left, y), vec2(STEPPER_WIDTH, stepper::HEIGHT));
        shapes.extend(stepper::forced_shapes(
            ui, skin, rect, "255", None, state, state, enabled,
        ));
        let rect = Rect::from_min_size(
            pos2(left + STEPPER_UNIT_LEFT, y),
            vec2(STEPPER_UNIT_WIDTH, stepper::HEIGHT),
        );
        shapes.extend(stepper::forced_shapes(
            ui,
            skin,
            rect,
            "155",
            Some("ms"),
            state,
            state,
            enabled,
        ));
        let (name, _) = pixel_draw::caps_text(
            ui,
            state.name(),
            rect.right() + STATE_LABEL_GAP,
            y + super::input::CAPS_TOP,
            colors.text,
        );
        shapes.push(name);
        y += INPUT_PITCH;
    }
    ui.painter().extend(shapes);
    y += INPUT_PITCH;
    let (mut a, mut b): (i32, i32) =
        ui.data(|d| d.get_temp(live_steppers_id()).unwrap_or((255, 155)));
    for (x, width, salt, value, range, step, unit) in [
        (left, STEPPER_WIDTH, "live_a", &mut a, 0..=255, 1, None),
        (
            left + STEPPER_UNIT_LEFT,
            STEPPER_UNIT_WIDTH,
            "live_b",
            &mut b,
            10..=1000,
            10,
            Some("ms"),
        ),
    ] {
        let rect = Rect::from_min_size(pos2(x, y), vec2(width, stepper::HEIGHT));
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            stepper::stepper(ui, skin, width, salt, value, range, step, unit);
        });
    }
    ui.data_mut(|d| d.insert_temp(live_steppers_id(), (a, b)));
    y + stepper::HEIGHT
}

/// Sección Title bar, como en el mockup: la barra de 173 con el ≡, el título con acentos y
/// descendentes y el ○, con el foco y sin él (una cada 32 filas); a la derecha de cada una, la ×
/// con el puntero encima.
const TITLE_SAMPLE: &str = "Un título con áéíóú ygp";
const TITLE_WIDTH: f32 = 173.0;
const TITLE_PITCH: f32 = 32.0;

fn title_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use super::title_bar::{self, TitleLook, TitleState};
    let colors = *skin.colors();
    let look = TitleLook::new(skin);
    let ppp = ui.ctx().pixels_per_point();
    let left = origin.x;
    let mut y = origin.y;
    let mut shapes: Vec<Shape> = Vec::new();
    for (name, focused) in [("Focused", true), ("Unfocused", false)] {
        for (i, hovered) in [false, true].into_iter().enumerate() {
            let x = left + i as f32 * (TITLE_WIDTH + STATE_LABEL_GAP);
            let rect = Rect::from_min_size(pos2(x, y), vec2(TITLE_WIDTH, title_bar::HEIGHT));
            let state = TitleState {
                focused,
                menu: true,
                close: Some(hovered),
            };
            shapes.extend(title_bar::bar_shapes(
                ui.painter(),
                &look,
                rect,
                TITLE_SAMPLE,
                state,
                ppp,
            ));
        }
        let (label, _) = pixel_draw::caps_text_in(
            ui,
            TextStyle::Small,
            &format!("{name} (right: the x with the pointer over it)"),
            left,
            y + title_bar::HEIGHT + 4.0,
            colors.text_dim,
        );
        shapes.push(label);
        y += TITLE_PITCH;
    }
    // Sin ≡ ni ○ (un diálogo que no se cierra).
    let rect = Rect::from_min_size(pos2(left, y), vec2(TITLE_WIDTH, title_bar::HEIGHT));
    shapes.extend(title_bar::bar_shapes(
        ui.painter(),
        &look,
        rect,
        "Just a title",
        TitleState {
            focused: true,
            menu: false,
            close: None,
        },
        ppp,
    ));
    ui.painter().extend(shapes);
    y + title_bar::HEIGHT
}

/// Sección Menu, como en el mockup: el menú Frame abierto (con los menús de verdad: ítems con y
/// sin atajo, deshabilitados, un campo, separadores y una casilla) y, a su derecha, el menú chico
/// "Layer". Responden al puntero como los de la barra (los clics no hacen nada).
const MENU_FRAME_WIDTH: f32 = 112.0;
const MENU_LAYER_LEFT: f32 = 150.0;
const MENU_LAYER_WIDTH: f32 = 79.0;

/// En la memoria de egui: la casilla y el número del menú de muestra.
fn menu_demo_id() -> egui::Id {
    egui::Id::new("ui_catalog_menu")
}

fn menu_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    use crate::menus;
    let (mut onion, mut opacity): (bool, u8) =
        ui.data(|d| d.get_temp(menu_demo_id()).unwrap_or((false, 255)));
    let title_left = origin.x + 5.0;
    let frame = menus::open_menu_preview(
        ui,
        skin,
        title_left,
        origin.y,
        "Frame",
        MENU_FRAME_WIDTH,
        |ui| {
            ui.add(menus::item("New Frame", "Alt+N"));
            ui.add(menus::plain("New Empty Frame"));
            ui.add_enabled(false, menus::plain("Delete Frame"));
            ui.add(menus::plain("Frame Properties…"));
            ui.add_enabled(false, menus::plain("Reverse Frames"));
            ui.horizontal(|ui| {
                ui.label("Cel Opacity");
                crate::widgets::number(ui, &mut opacity, 0..=255, "");
            });
            menus::separator(ui);
            ui.add(menus::plain("New Tag"));
            menus::separator(ui);
            ui.add(menus::plain("First Frame"));
            ui.add(menus::item("Previous Frame", ","));
            ui.add(menus::item("Next Frame", "."));
            ui.add(menus::plain("Last Frame"));
            ui.add(menus::item("Play Animation", "Enter"));
            menus::separator(ui);
            ui.add(menus::check(&mut onion, "Onion Skin"));
        },
    );
    let layer = menus::open_menu_preview(
        ui,
        skin,
        title_left + MENU_LAYER_LEFT,
        origin.y,
        "Layer",
        MENU_LAYER_WIDTH,
        |ui| {
            ui.add(menus::plain("Item 1"));
            ui.add(menus::plain("Item 2"));
            menus::separator(ui);
            ui.add(menus::plain("Item 3"));
        },
    );
    ui.data_mut(|d| d.insert_temp(menu_demo_id(), (onion, opacity)));
    frame.bottom().max(layer.bottom()) + 8.0
}

/// Sección Dock, como en el mockup: un botón del dock en sus tres estados (normal, con el puntero
/// encima y apretado), uno cada 26; debajo, la columna de botones (uno cada 27) y, a su derecha,
/// un panel flotante abierto con la pestaña que envuelve al botón apretado.
const DOCK_ICONS: [&str; 7] = [
    "icon.dock.artboards",
    "icon.dock.library",
    "icon.dock.slices",
    "icon.dock.lospec",
    "icon.dock.dafont",
    "icon.dock.groom",
    "icon.dock.lumen",
];
const DOCK_STATE_PITCH: f32 = 26.0;
const DOCK_PITCH: f32 = 27.0;
const DOCK_PANEL: Vec2 = vec2(120.0, 120.0);
const DOCK_PANEL_GAP: f32 = 4.0;
const DOCK_OPEN: usize = 2;

/// Un botón del dock dibujado en `state`: la pieza `button` (con su línea clara debajo) y el ícono
/// centrado, que baja 1 px apretado.
fn dock_button_shapes(
    skin: &Skin,
    rect: Rect,
    icon: &str,
    state: ButtonState,
    ppp: f32,
) -> Vec<Shape> {
    let mut shapes = button::frame_shapes(skin, rect.with_max_y(rect.bottom() + 1.0), state, ppp);
    let down = if state == ButtonState::Pressed {
        1.0
    } else {
        0.0
    };
    shapes.extend(crate::pixel_draw::piece_centered(
        skin,
        icon,
        rect.translate(vec2(0.0, down)),
        ppp,
    ));
    shapes
}

fn dock_section(ui: &mut Ui, skin: &Skin, origin: Pos2) -> f32 {
    let ppp = ui.ctx().pixels_per_point();
    let size = crate::dock::BUTTON_SIZE;
    let mut shapes: Vec<Shape> = Vec::new();
    for (i, state) in [
        ButtonState::Normal,
        ButtonState::Hover,
        ButtonState::Pressed,
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(origin + vec2(i as f32 * DOCK_STATE_PITCH, 0.0), size);
        shapes.extend(dock_button_shapes(skin, rect, DOCK_ICONS[0], state, ppp));
    }
    // La columna, con el panel abierto del tercero.
    let top = origin.y + size.y + 1.0 + DOCK_PITCH - size.y;
    let column_left = origin.x + DOCK_PANEL.x + DOCK_PANEL_GAP;
    let mut open_button = Rect::NOTHING;
    for (i, icon) in DOCK_ICONS.iter().enumerate() {
        let rect = Rect::from_min_size(pos2(column_left, top + i as f32 * DOCK_PITCH), size);
        if i == DOCK_OPEN {
            open_button = rect;
        } else {
            shapes.extend(dock_button_shapes(
                skin,
                rect,
                icon,
                ButtonState::Normal,
                ppp,
            ));
        }
    }
    let panel = Rect::from_min_size(pos2(origin.x, top), DOCK_PANEL);
    shapes.extend(crate::dock::open_preview_shapes(
        skin,
        panel,
        open_button,
        DOCK_ICONS[DOCK_OPEN],
        ppp,
    ));
    ui.painter().extend(shapes);
    top + DOCK_ICONS.len() as f32 * DOCK_PITCH
}
