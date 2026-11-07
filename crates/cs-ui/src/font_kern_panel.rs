//! La vista Kerning del panel FontForge: el texto de prueba con el kerning aplicado (un clic en
//! una letra elige el par que forma con la de antes y arrastrarla a los lados lo cambia), el par
//! elegido con su valor y de dónde sale, la lista de pares con una fila para agregar, y las
//! clases (First y Second) con sus caracteres. Todo lo cambiado en un frame (o en un arrastre) es
//! un paso de deshacer.

use cs_core::{ClassSide, FontKerning, KernClass, KernSide, KernSource};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Ui, Vec2, vec2};

use crate::editor::Editor;
use crate::font_panel::{self, FontPanelState};
use crate::kit;
use crate::panel_kit::label_with_hint;
use crate::pixel_ui;
use crate::skin::Skin;
use crate::widgets;

/// El aire entre secciones y entre filas, y entre controles de una fila.
const SECTION_AIR: f32 = 6.0;
const ROW_GAP: f32 = 3.0;
const GAP: f32 = 4.0;
/// Los anchos: el nombre de un par en la lista, su stepper, los campos First/Second, el nombre de
/// una clase, su lado y sus caracteres.
const PAIR_NAME_W: f32 = 90.0;
const VALUE_W: f32 = 62.0;
const SIDE_FIELD_W: f32 = 60.0;
const CLASS_NAME_W: f32 = 70.0;
const CLASS_SIDE_W: f32 = 70.0;
const CLASS_CHARS_W: f32 = 90.0;
/// El color de la marca del par elegido en el papel (el del glifo actual del mapa).
const MARK: Color32 = Color32::from_rgb(226, 114, 133);

/// El valor que vale entre `a` y `b` y el par de donde sale (su índice y qué tan específico es).
pub(crate) fn pair_value(k: &FontKerning, a: char, b: char) -> (i32, Option<(usize, KernSource)>) {
    let source = k.source(a, b);
    (source.map_or(0, |(i, _)| k.pairs[i].value), source)
}

/// Pone `value` entre `a` y `b`: en el par de donde sale (aunque sea de una clase) o, si no hay,
/// en un par nuevo de los dos caracteres.
pub(crate) fn apply_value(k: &mut FontKerning, a: char, b: char, value: i32) {
    match k.source(a, b) {
        Some((i, _)) => k.pairs[i].value = value,
        None => k.set(KernSide::Char(a), KernSide::Char(b), value),
    }
}

/// Le cambia el nombre a la clase `index` y a los pares de su lado que la usan.
pub(crate) fn rename_class(k: &mut FontKerning, index: usize, name: &str) {
    let Some(class) = k.classes.get(index).cloned() else {
        return;
    };
    let name = name.trim();
    let taken = k
        .classes
        .iter()
        .any(|c| c.side == class.side && c.name == name);
    if name.is_empty() || taken {
        return;
    }
    let old = KernSide::Class(class.name.clone());
    let new = KernSide::Class(name.to_owned());
    for p in &mut k.pairs {
        let side = match class.side {
            ClassSide::First => &mut p.first,
            ClassSide::Second => &mut p.second,
        };
        if *side == old {
            *side = new.clone();
        }
    }
    k.classes[index].name = name.to_owned();
    k.normalize();
}

/// Quita la clase `index` y los pares de su lado que la usan.
pub(crate) fn remove_class(k: &mut FontKerning, index: usize) {
    if index >= k.classes.len() {
        return;
    }
    let class = k.classes.remove(index);
    let gone = KernSide::Class(class.name);
    k.pairs.retain(|p| match class.side {
        ClassSide::First => p.first != gone,
        ClassSide::Second => p.second != gone,
    });
}

/// Una clase nueva (First, vacía) con un nombre libre ("class1", "class2"…).
pub(crate) fn new_class(k: &mut FontKerning) {
    let name = (1..)
        .map(|i| format!("class{i}"))
        .find(|n| !k.classes.iter().any(|c| c.name == *n))
        .expect("a free name");
    k.classes.push(KernClass {
        name,
        side: ClassSide::First,
        chars: Vec::new(),
    });
}

/// Por qué la clase `index` no puede cambiar de lado (`None` si puede): la usan pares (del otro
/// lado ya no la encontrarían) o del otro lado ya hay una clase con su nombre.
pub(crate) fn side_change_blocked(k: &FontKerning, index: usize) -> Option<String> {
    let class = k.classes.get(index)?;
    let it = KernSide::Class(class.name.clone());
    let used = k
        .pairs
        .iter()
        .filter(|p| match class.side {
            ClassSide::First => p.first == it,
            ClassSide::Second => p.second == it,
        })
        .count();
    match used {
        0 => {}
        1 => return Some("Used by 1 pair: remove it to change the side".to_owned()),
        n => return Some(format!("Used by {n} pairs: remove them to change the side")),
    }
    let other = ClassSide::ALL.into_iter().find(|&s| s != class.side)?;
    k.classes
        .iter()
        .any(|c| c.side == other && c.name == class.name)
        .then(|| format!("There is already a {} class @{}", other.label(), class.name))
}

/// Los caracteres de la clase `index` que ya están en una clase anterior de su lado (cuentan en
/// esa) y el nombre de la primera de esas clases.
pub(crate) fn shadowed_chars(k: &FontKerning, index: usize) -> Option<(String, String)> {
    let class = k.classes.get(index)?;
    let mut owner = None;
    let chars: String = class
        .chars
        .iter()
        .filter(|&&c| {
            let first = k.classes[..index]
                .iter()
                .find(|o| o.side == class.side && o.chars.contains(&c));
            if let Some(o) = first {
                owner.get_or_insert_with(|| o.name.clone());
            }
            first.is_some()
        })
        .collect();
    owner.map(|o| (chars, o))
}

/// De dónde sale el valor de un par, para leerlo: "pair", "@round · V"…
fn source_label(k: &FontKerning, source: Option<(usize, KernSource)>) -> String {
    match source {
        None => "no pair yet".to_owned(),
        Some((_, KernSource::Exact)) => "pair".to_owned(),
        Some((i, _)) => {
            let p = &k.pairs[i];
            format!("{} · {}", p.first.label(), p.second.label())
        }
    }
}

fn button(ui: &mut Ui, skin: Option<&Skin>, label: &str) -> egui::Response {
    match skin {
        Some(skin) => kit::button::button(ui, skin, label),
        None => pixel_ui::text_button(ui, label),
    }
}

fn field(
    ui: &mut Ui,
    skin: Option<&Skin>,
    text: &mut String,
    width: f32,
    label: &str,
) -> egui::Response {
    match skin {
        Some(skin) => kit::input::text_input(ui, skin, text, width, label, label),
        None => ui.add(
            egui::TextEdit::singleline(text)
                .desired_width(width)
                .hint_text(label),
        ),
    }
}

fn number(
    ui: &mut Ui,
    skin: Option<&Skin>,
    id: &str,
    value: &mut i32,
    limit: i32,
) -> egui::Response {
    match skin {
        Some(skin) => kit::stepper::stepper(ui, skin, VALUE_W, id, value, -limit..=limit, 1, None),
        None => ui.add(egui::DragValue::new(value).range(-limit..=limit)),
    }
}

/// La vista Kerning (lo de en medio del panel).
pub(crate) fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut FontPanelState,
) {
    let Some(font) = editor.font().cloned() else {
        return;
    };
    let Some(view) = editor.artboard().font_view() else {
        return;
    };
    let limit = editor.sprite().width as i32;
    let mut k = font.kerning.clone();
    ui.spacing_mut().item_spacing.y = 0.0;
    let note = match k.pairs.len() {
        1 => "1 pair".to_owned(),
        n => format!("{n} pairs"),
    };
    label_with_hint(ui, "Kerning", &note);
    ui.add_space(SECTION_AIR);
    field(
        ui,
        skin,
        &mut state.test_text,
        font_panel::TEST_W,
        "Test text",
    );
    ui.add_space(4.0);
    paper(ui, &view, &mut k, state, limit);
    // El par elegido.
    ui.add_space(SECTION_AIR);
    let pair = state
        .kern_pair
        .filter(|&(a, b)| font.index_of(a).is_some() && font.index_of(b).is_some());
    match pair {
        Some((a, b)) => {
            let (mut value, source) = pair_value(&k, a, b);
            label_with_hint(
                ui,
                &format!("Pair {a} {b}"),
                &format!("({})", source_label(&k, source)),
            );
            ui.add_space(ROW_GAP);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GAP;
                if number(ui, skin, "kern_pair_value", &mut value, limit).changed() {
                    apply_value(&mut k, a, b, value);
                }
                let has = source.is_some();
                let remove = ui
                    .add_enabled_ui(has, |ui| button(ui, skin, "Remove"))
                    .inner
                    .on_disabled_hover_text("No pair to remove");
                if remove.clicked()
                    && let Some((i, _)) = source
                {
                    k.pairs.remove(i);
                }
                let from_class = source.is_some_and(|(_, s)| s != KernSource::Exact);
                let exception = ui
                    .add_enabled_ui(from_class, |ui| button(ui, skin, "Exception"))
                    .inner
                    .on_hover_text("A pair of these two letters that overrides the class")
                    .on_disabled_hover_text("Only for values that come from a class");
                if exception.clicked() {
                    k.set(KernSide::Char(a), KernSide::Char(b), value);
                }
            });
        }
        None => {
            label_with_hint(ui, "Pair", "(click a letter of the text)");
        }
    }
    pairs_list(ui, skin, &mut k, state, limit);
    classes(ui, skin, &mut k);
    // Un arrastre (de un stepper o de una letra) es un solo paso de undo.
    let group = ui.id().with("font_kerning").value();
    if k != font.kerning {
        editor.set_kerning_grouped(k, group);
    }
    if !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group);
    }
}

/// El papel con el texto de prueba y su kerning: un clic en una letra elige el par con la de
/// antes y arrastrarla a los lados lo cambia por pixeles enteros. El par elegido va subrayado,
/// con una raya entre las dos letras.
fn paper(
    ui: &mut Ui,
    view: &cs_core::PixelFontView,
    k: &mut FontKerning,
    state: &mut FontPanelState,
    limit: i32,
) {
    let scale = font_panel::TEST_SCALE;
    let line_h = (view.cell.1 * scale) as f32;
    let lines = state.test_text.lines().count().max(1);
    let (rect, _) = ui.allocate_exact_size(
        vec2(
            font_panel::TEST_W,
            line_h * lines as f32 + 2.0 * font_panel::TEST_PAD,
        ),
        Sense::hover(),
    );
    let paper = Rect::from_min_size(rect.min.round(), rect.size());
    let painter = ui.painter().with_clip_rect(paper.intersect(ui.clip_rect()));
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    let origin = paper.min + Vec2::splat(font_panel::TEST_PAD);
    for (x, y, c) in cs_tools::text::font_text(view, &state.test_text, scale) {
        let color = c.map_or(font_panel::TEST_INK, |c| {
            Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
        });
        painter.rect_filled(
            Rect::from_min_size(origin + vec2(x as f32, y as f32), Vec2::splat(1.0)),
            0.0,
            color,
        );
    }
    let laid = cs_tools::text::font_layout(view, &state.test_text);
    let enabled = ui.is_enabled();
    for (i, l) in laid.iter().enumerate() {
        // Una letra que forma par con la de antes (las dos son glifos, en la misma línea).
        let prev = i
            .checked_sub(1)
            .map(|j| laid[j])
            .filter(|p| p.row == l.row && p.glyph && l.glyph);
        let letter = font_panel::letter_rect(origin, line_h, l).intersect(paper);
        if !letter.is_positive() {
            continue;
        }
        let Some(prev) = prev else {
            continue;
        };
        let (a, b) = (prev.ch, l.ch);
        if state.kern_pair == Some((a, b)) {
            let before = font_panel::letter_rect(origin, line_h, &prev).intersect(paper);
            let y = letter.bottom() - 1.0;
            let stroke = Stroke::new(1.0, MARK);
            painter.hline(before.left()..=letter.right(), y, stroke);
            painter.vline(letter.left(), letter.y_range(), stroke);
        }
        let id = ui.id().with(("kern_letter", l.row, l.col));
        let response = ui.interact(letter, id, Sense::click_and_drag());
        let label = format!("Kern {a}{b}");
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        if response.clicked() || response.drag_started() {
            state.kern_pair = Some((a, b));
        }
        if response.drag_started() {
            state.kern_drag = Some((pair_value(k, a, b).0, 0.0));
        }
        if response.dragged()
            && let Some((start, moved)) = &mut state.kern_drag
        {
            *moved += response.drag_delta().x;
            let value = (*start + (*moved / scale as f32).round() as i32).clamp(-limit, limit);
            if value != pair_value(k, a, b).0 {
                apply_value(k, a, b, value);
            }
        }
        if response.drag_stopped() {
            state.kern_drag = None;
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
}

/// La lista de pares (cada uno con su valor y Remove; elegirlo lo elige en el papel) y la fila
/// para agregar uno (First, Second y Add).
fn pairs_list(
    ui: &mut Ui,
    skin: Option<&Skin>,
    k: &mut FontKerning,
    state: &mut FontPanelState,
    limit: i32,
) {
    ui.add_space(SECTION_AIR);
    label_with_hint(ui, "Pairs", "(a letter or @class)");
    ui.add_space(ROW_GAP);
    let mut remove = None;
    for (i, pair) in k.pairs.clone().iter().enumerate() {
        let name = format!("{} {}", pair.first.label(), pair.second.label());
        let chars = match (&pair.first, &pair.second) {
            (KernSide::Char(a), KernSide::Char(b)) => Some((*a, *b)),
            _ => None,
        };
        let mut value = pair.value;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let on = chars.is_some() && state.kern_pair == chars;
            let picked = ui
                .allocate_ui_with_layout(
                    vec2(PAIR_NAME_W, kit::stepper::HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(PAIR_NAME_W);
                        match skin {
                            Some(skin) => kit::check::radio(ui, skin, on, &name),
                            None => ui.radio(on, &name),
                        }
                    },
                )
                .inner;
            if picked.clicked() && chars.is_some() {
                state.kern_pair = chars;
            }
            if number(ui, skin, &format!("kern_value{i}"), &mut value, limit).changed() {
                k.pairs[i].value = value;
            }
            if button(ui, skin, "Remove").clicked() {
                remove = Some(i);
            }
        });
        ui.add_space(ROW_GAP);
    }
    if let Some(i) = remove {
        k.pairs.remove(i);
    }
    let added = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let a = field(ui, skin, &mut state.kern_first, SIDE_FIELD_W, "First");
            let b = field(ui, skin, &mut state.kern_second, SIDE_FIELD_W, "Second");
            let entered =
                (a.lost_focus() || b.lost_focus()) && ui.input(|i| i.key_pressed(egui::Key::Enter));
            button(ui, skin, "Add").clicked() || entered
        })
        .inner;
    if added {
        let first = KernSide::parse(&state.kern_first);
        let second = KernSide::parse(&state.kern_second);
        if let (Some(first), Some(second)) = (first, second) {
            if k.index_of(&first, &second).is_none() {
                k.set(first.clone(), second.clone(), 0);
            }
            if let (KernSide::Char(a), KernSide::Char(b)) = (first, second) {
                state.kern_pair = Some((a, b));
            }
            state.kern_first.clear();
            state.kern_second.clear();
        }
    }
}

/// Las clases: su nombre, su lado y sus caracteres (campos que se guardan al salir de ellos), y
/// Remove; New class agrega una.
fn classes(ui: &mut Ui, skin: Option<&Skin>, k: &mut FontKerning) {
    ui.add_space(SECTION_AIR);
    label_with_hint(ui, "Classes", "(First: left of the pair; Second: right)");
    ui.add_space(ROW_GAP);
    let mut remove = None;
    for (i, class) in k.classes.clone().iter().enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let name_id = ui.id().with(("kern_class_name", i));
            if let Some(name) = kit::input::buffered(ui, name_id, &class.name, |ui, text| {
                field(ui, skin, text, CLASS_NAME_W, "Class name")
            }) {
                rename_class(k, i, &name);
            }
            let mut side = class.side;
            // Con pares que la usan no cambia de lado (se quedarían sin clase): se avisa por qué.
            let blocked = side_change_blocked(k, i);
            let picker = ui.add_enabled_ui(blocked.is_none(), |ui| match skin {
                Some(skin) => kit::select::select(
                    ui,
                    skin,
                    &format!("kern_class_side{i}"),
                    side.label(),
                    Some(CLASS_SIDE_W),
                    |ui| {
                        for s in ClassSide::ALL {
                            widgets::choice(ui, &mut side, s, s.label());
                        }
                    },
                ),
                None => {
                    egui::ComboBox::from_id_salt(("kern_class_side", i))
                        .selected_text(side.label())
                        .show_ui(ui, |ui| {
                            for s in ClassSide::ALL {
                                widgets::choice(ui, &mut side, s, s.label());
                            }
                        })
                        .response
                }
            });
            if let Some(why) = &blocked {
                picker.inner.on_disabled_hover_text(why);
            }
            if side != class.side && blocked.is_none() {
                k.classes[i].side = side;
            }
            let chars: String = class.chars.iter().collect();
            let chars_id = ui.id().with(("kern_class_chars", i));
            if let Some(text) = kit::input::buffered(ui, chars_id, &chars, |ui, text| {
                field(ui, skin, text, CLASS_CHARS_W, "Characters")
            }) {
                let mut list: Vec<char> = Vec::new();
                for c in text.chars().filter(|c| !c.is_whitespace()) {
                    if !list.contains(&c) {
                        list.push(c);
                    }
                }
                k.classes[i].chars = list;
            }
            if button(ui, skin, "Remove").clicked() {
                remove = Some(i);
            }
        });
        // Un carácter en dos clases del mismo lado cuenta en la primera: se avisa.
        if let Some((chars, owner)) = shadowed_chars(k, i) {
            ui.add_space(ROW_GAP);
            crate::panel_kit::hint(
                ui,
                &format!("{chars}: already in @{owner}, they count there"),
            );
        }
        ui.add_space(ROW_GAP);
    }
    if let Some(i) = remove {
        remove_class(k, i);
    }
    if button(ui, skin, "New class").clicked() {
        new_class(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::KernSide::{Char, Class};

    fn kerning() -> FontKerning {
        let mut k = FontKerning {
            pairs: Vec::new(),
            classes: vec![KernClass {
                name: "round".into(),
                side: ClassSide::First,
                chars: vec!['O', 'Q'],
            }],
        };
        k.set(Class("round".into()), Char('V'), -1);
        k
    }

    #[test]
    fn a_value_goes_to_the_pair_it_comes_from_or_a_new_one() {
        let mut k = kerning();
        apply_value(&mut k, 'Q', 'V', -2);
        assert_eq!(k.pairs.len(), 1, "the class pair changed");
        assert_eq!(k.value('O', 'V'), -2);
        apply_value(&mut k, 'A', 'V', -3);
        assert_eq!(k.pairs.len(), 2);
        assert_eq!(pair_value(&k, 'A', 'V').0, -3);
        assert_eq!(source_label(&k, k.source('O', 'V')), "@round · V");
        assert_eq!(source_label(&k, k.source('A', 'V')), "pair");
        assert_eq!(source_label(&k, None), "no pair yet");
    }

    #[test]
    fn renaming_and_removing_a_class_follow_its_pairs() {
        let mut k = kerning();
        rename_class(&mut k, 0, " soft ");
        assert_eq!(k.classes[0].name, "soft");
        assert_eq!(k.pairs[0].first, Class("soft".into()));
        assert_eq!(k.value('O', 'V'), -1);
        rename_class(&mut k, 0, "");
        assert_eq!(k.classes[0].name, "soft", "an empty name does nothing");
        new_class(&mut k);
        assert_eq!(k.classes[1].name, "class1");
        remove_class(&mut k, 0);
        assert!(k.pairs.is_empty());
        assert_eq!(k.classes.len(), 1);
    }

    #[test]
    fn a_class_with_pairs_or_a_namesake_on_the_other_side_keeps_its_side() {
        let mut k = kerning();
        assert_eq!(
            side_change_blocked(&k, 0).as_deref(),
            Some("Used by 1 pair: remove it to change the side")
        );
        k.set(Class("round".into()), Char('W'), -1);
        assert!(
            side_change_blocked(&k, 0)
                .unwrap()
                .starts_with("Used by 2 pairs")
        );
        k.pairs.clear();
        assert_eq!(side_change_blocked(&k, 0), None);
        k.classes.push(KernClass {
            name: "round".into(),
            side: ClassSide::Second,
            chars: vec!['O'],
        });
        assert_eq!(
            side_change_blocked(&k, 0).as_deref(),
            Some("There is already a Second class @round")
        );
        assert_eq!(side_change_blocked(&k, 9), None);
    }

    #[test]
    fn letters_already_in_an_earlier_class_of_the_same_side_are_pointed_out() {
        let mut k = kerning();
        k.classes.push(KernClass {
            name: "o".into(),
            side: ClassSide::First,
            chars: vec!['o', 'O', 'Q'],
        });
        k.classes.push(KernClass {
            name: "other side".into(),
            side: ClassSide::Second,
            chars: vec!['O'],
        });
        assert_eq!(shadowed_chars(&k, 0), None, "the first one wins");
        assert_eq!(shadowed_chars(&k, 1), Some(("OQ".into(), "round".into())));
        assert_eq!(shadowed_chars(&k, 2), None, "the other side does not count");
    }
}
