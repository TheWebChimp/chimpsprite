//! Edit → Keyboard Shortcuts…: los atajos del registro ([`crate::keymap`]) por sección, con un
//! buscador, y para cambiarlos como en Aseprite: un clic en las teclas de una acción y se
//! aprietan las nuevas (Esc no cambia nada; Delete la deja sin atajo). Las que tenían esas teclas
//! las pierden. Reset vuelve una acción (o todas) a las de fábrica. Lo que no se reasigna se ve
//! más tenue.

use eframe::egui::{self, Align2, Key, RichText, Sense, Ui, vec2};

use crate::keymap::{Action, Combo, FIXED, Keymap};
use crate::skin::Skin;

const WIDTH: f32 = 380.0;
const LIST_HEIGHT: f32 = 320.0;
const KEYS_WIDTH: f32 = 120.0;

/// El estado de la ventana: lo escrito en el buscador y la acción que espera sus teclas.
#[derive(Default)]
pub struct ShortcutsWindow {
    filter: String,
    capturing: Option<Action>,
}

/// Una fila de la lista.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    Action(Action),
    Fixed(&'static str, &'static str, &'static str),
}

impl Row {
    fn section(&self) -> &'static str {
        match self {
            Row::Action(a) => a.names().0,
            Row::Fixed(s, _, _) => s,
        }
    }
}

/// Las filas que coinciden con `filter` (en la acción, las teclas o la sección), por sección.
fn matching(keymap: &Keymap, filter: &str) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let mut rows: Vec<Row> = Action::all()
        .into_iter()
        .map(Row::Action)
        .chain(FIXED.iter().map(|&(s, a, k)| Row::Fixed(s, a, k)))
        .filter(|row| {
            let (section, name, keys) = match *row {
                Row::Action(a) => (a.names().0, a.names().1, keys_text(keymap, a)),
                Row::Fixed(s, a, k) => (s, a, k.to_owned()),
            };
            filter.is_empty()
                || [section, name, &keys]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&filter))
        })
        .collect();
    // Por sección, en el orden en que aparece cada una (estable: dentro, el de la lista).
    let order: Vec<&str> = {
        let mut seen = Vec::new();
        for row in &rows {
            if !seen.contains(&row.section()) {
                seen.push(row.section());
            }
        }
        seen
    };
    rows.sort_by_key(|r| order.iter().position(|s| *s == r.section()));
    rows
}

/// Todas las teclas de una acción, separadas por coma.
fn keys_text(keymap: &Keymap, action: Action) -> String {
    keymap
        .combos(action)
        .iter()
        .map(Combo::label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// La ventana. Devuelve si se cerró.
pub fn show(
    ctx: &egui::Context,
    window: &mut ShortcutsWindow,
    keymap: &mut Keymap,
    skin: Option<&Skin>,
) -> bool {
    // Esperando teclas: la primera que se apriete (con sus modificadores) es la nueva.
    if let Some(action) = window.capturing {
        let pressed = ctx.input_mut(|i| {
            let found = i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => Some((*key, *modifiers)),
                _ => None,
            });
            if found.is_some() {
                i.events.retain(|e| !matches!(e, egui::Event::Key { .. }));
            }
            found
        });
        if let Some((key, modifiers)) = pressed {
            match key {
                Key::Escape => {}
                Key::Delete | Key::Backspace => keymap.clear(action),
                _ => keymap.assign(
                    action,
                    Combo {
                        key,
                        ctrl: modifiers.command,
                        shift: modifiers.shift,
                        alt: modifiers.alt,
                    },
                ),
            }
            window.capturing = None;
        }
    }
    let anchor = (Align2::CENTER_CENTER, egui::Vec2::ZERO);
    let mut done = false;
    let (_, closed) = crate::pixel_ui::app_window(
        ctx,
        skin,
        "keyboard_shortcuts",
        "Keyboard Shortcuts",
        true,
        anchor,
        |ui| {
            contents(ui, window, keymap, skin);
            ui.add_space(6.0);
            match crate::pixel_ui::dialog_buttons(ui, skin, &["OK", "Reset All"]) {
                Some(0) => done = true,
                Some(_) => keymap.reset_all(),
                None => {}
            }
        },
    );
    let escape = window.capturing.is_none() && ctx.input(|i| i.key_pressed(Key::Escape));
    closed || done || escape
}

fn contents(ui: &mut Ui, window: &mut ShortcutsWindow, keymap: &mut Keymap, skin: Option<&Skin>) {
    ui.set_width(WIDTH);
    ui.horizontal(|ui| {
        ui.label("Search");
        crate::widgets::text_line(ui, &mut window.filter);
    });
    ui.add_space(4.0);
    let dim = skin.map_or(ui.visuals().weak_text_color(), |s| s.colors().text_dim);
    let rows = matching(keymap, &window.filter);
    crate::pixel_ui::scroll(
        ui,
        egui::ScrollArea::vertical()
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true]),
        |ui| {
            let mut section = "";
            egui::Grid::new("shortcuts")
                .num_columns(3)
                .spacing(vec2(8.0, 2.0))
                .show(ui, |ui| {
                    for row in rows {
                        if row.section() != section {
                            section = row.section();
                            ui.label(RichText::new(section).color(dim));
                            ui.end_row();
                        }
                        match row {
                            Row::Action(action) => {
                                action_row(ui, window, keymap, action);
                            }
                            Row::Fixed(_, name, keys) => {
                                ui.label(RichText::new(name).color(dim));
                                ui.label(RichText::new(keys).color(dim));
                            }
                        }
                        ui.end_row();
                    }
                });
        },
    );
}

/// Una acción: su nombre, sus teclas (un clic para cambiarlas) y, si no son las de fábrica,
/// Reset.
fn action_row(ui: &mut Ui, window: &mut ShortcutsWindow, keymap: &mut Keymap, action: Action) {
    ui.label(action.names().1);
    let capturing = window.capturing == Some(action);
    let text = if capturing {
        "Press keys…".to_owned()
    } else {
        let keys = keys_text(keymap, action);
        if keys.is_empty() {
            "—".to_owned()
        } else {
            keys
        }
    };
    let (rect, response) = ui.allocate_exact_size(vec2(KEYS_WIDTH, 15.0), Sense::click());
    let label = format!("{} keys", action.names().1);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
    // Una casilla como la de los campos: blanca con su borde; esperando las teclas, en el
    // rosa de lo elegido.
    let skin = crate::pixel_ui::frame_skin(ui.ctx());
    let (fill, border, ink) = match &skin {
        Some(s) => {
            let c = s.colors();
            let fill = if capturing {
                c.timeline_selection
            } else if response.hovered() {
                c.menu_highlight
            } else {
                egui::Color32::WHITE
            };
            (fill, c.outline, c.text)
        }
        None => {
            let v = ui.style().interact_selectable(&response, capturing);
            (v.weak_bg_fill, v.bg_stroke.color, v.text_color())
        }
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.left_center() + vec2(4.0, 0.0),
        Align2::LEFT_CENTER,
        text,
        egui::TextStyle::Body.resolve(ui.style()),
        ink,
    );
    if response.clicked() {
        window.capturing = if capturing { None } else { Some(action) };
    }
    if keymap.is_custom(action) {
        if ui.small_button("Reset").clicked() {
            keymap.reset(action);
        }
    } else {
        ui.label("");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ToolKind;

    #[test]
    fn the_list_has_every_action_and_the_fixed_ones_and_search_filters() {
        let keymap = Keymap::default();
        let all = matching(&keymap, "");
        assert!(all.contains(&Row::Action(Action::Tool(ToolKind::Pencil))));
        assert!(all.contains(&Row::Fixed("Edit", "Copy", "Ctrl+C")));
        assert_eq!(
            matching(&keymap, "stroke"),
            vec![Row::Action(Action::Stroke)]
        );
        // También por las teclas.
        assert!(
            matching(&keymap, "ctrl+alt+n")
                .contains(&Row::Action(Action::NewArtboardFromSelection))
        );
    }
}
