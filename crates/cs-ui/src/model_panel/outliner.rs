//! El outliner del panel 3D (la mitad de abajo, debajo de su encabezado): el árbol de objetos y
//! grupos del modelo, una fila cada uno, como el de Blockbench. Cada fila, con su sangría por
//! profundidad: ▸/▾ en los grupos (abre y cierra, sin paso de undo), el ícono (la carpeta o el
//! tipo del objeto), el nombre y, a la derecha, ◧ Double sided (solo objetos), el ojo y el
//! candado (un paso de undo cada clic). Lo oculto o bloqueado por un padre se ve atenuado.
//!
//! Un clic elige la fila (Shift: la suma o la quita de la selección); un doble clic la renombra
//! en el lugar (Enter confirma, Esc cancela); arrastrarla la reordena o la mete en un grupo (una
//! línea marca dónde cae); Supr con el puntero encima la borra.

use cs_core::{Model3D, Node};
use eframe::egui::{self, Color32, Painter, Pos2, Rect, Sense, Ui, Vec2, pos2, vec2};

use crate::editor::{DropAt, Editor, Row};
use crate::kit;
use crate::kit::button::ButtonState;
use crate::skin::{Skin, WidgetState};

/// El alto de una fila (como un ítem de menú) y la sangría de cada nivel.
pub const ROW_H: f32 = 13.0;
pub const INDENT: f32 = 8.0;
/// La columna del triángulo de los grupos (5 px con aire), antes del ícono.
const TRIANGLE_W: f32 = 8.0;
const TRIANGLE: f32 = 5.0;
/// Los íconos (el del tipo y los de prender y apagar): de 9×9, 2 filas debajo de lo alto de la
/// fila.
const ICON: f32 = 9.0;
const ICON_TOP: f32 = 2.0;
/// Del ícono al nombre, y de un ícono de la derecha al siguiente.
const NAME_GAP: f32 = 3.0;
const TOGGLE_PITCH: f32 = ICON + 2.0;
/// Las mayúsculas del nombre, 3 filas debajo de lo alto de la fila (como en los menús).
const CAPS_TOP: f32 = 3.0;
/// Sobre un grupo, el tercio del medio de su fila lo mete adentro.
const INTO_FROM: f32 = 1.0 / 3.0;
const INTO_TO: f32 = 2.0 / 3.0;
/// Lo heredado y Double sided apagado, a media opacidad.
const FADED: f32 = 0.5;

/// El estado del outliner entre frames.
#[derive(Default)]
pub struct OutlinerState {
    /// La fila que se renombra y lo escrito hasta ahora.
    pub rename: Option<(Row, String)>,
    /// La fila que se arrastra.
    pub drag: Option<Row>,
}

/// Dónde estaba la lista del outliner en el frame anterior (y en qué pasada): con el puntero
/// encima, Supr borra la fila activa (y no lo elegido en las vistas: ver
/// [`pointer_over_list`]).
fn list_id() -> egui::Id {
    egui::Id::new("model_outliner_list")
}

/// Si el puntero está sobre la lista del outliner dibujada en el frame anterior. Los atajos (que
/// corren antes de dibujar) y el outliner miran la misma, así Supr lo usa uno solo.
pub fn pointer_over_list(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let last: Option<(u64, Rect)> = ctx.data(|d| d.get_temp(list_id()));
    let Some((drawn, rect)) = last else {
        return false;
    };
    drawn + 1 >= pass && ctx.pointer_hover_pos().is_some_and(|p| rect.contains(p))
}

/// Una fila de la lista, ya resuelta del árbol.
struct Line {
    row: Row,
    depth: usize,
    name: String,
    icon: &'static str,
    hidden: bool,
    locked: bool,
    /// Oculto o bloqueado por un grupo de arriba.
    inherited: bool,
    /// Los objetos: su Double sided.
    double_sided: Option<bool>,
    /// Los grupos: abierto.
    open: Option<bool>,
}

/// Las filas que se ven de `nodes` (lo de adentro de un grupo cerrado no), en orden.
fn lines(model: &Model3D, nodes: &[Node], depth: usize, inherited: bool, out: &mut Vec<Line>) {
    for n in nodes {
        match n {
            Node::Object(o) => {
                let Some(obj) = model.objects.get(*o) else {
                    continue;
                };
                out.push(Line {
                    row: Row::Object(*o),
                    depth,
                    name: obj.name.clone(),
                    icon: super::kind_icon(obj.shown_kind()),
                    hidden: obj.hidden,
                    locked: obj.locked,
                    inherited,
                    double_sided: Some(obj.double_sided),
                    open: None,
                });
            }
            Node::Group { group, children } => {
                let Some(g) = model.groups.get(*group) else {
                    continue;
                };
                out.push(Line {
                    row: Row::Group(*group),
                    depth,
                    name: g.name.clone(),
                    icon: "icon.outliner.folder",
                    hidden: g.hidden,
                    locked: g.locked,
                    inherited,
                    double_sided: None,
                    open: Some(g.open),
                });
                if g.open {
                    let below = inherited || g.hidden || g.locked;
                    lines(model, children, depth + 1, below, out);
                }
            }
        }
    }
}

/// Lo que se pidió en una fila (se aplica después de dibujarlas todas).
enum Action {
    Choose(Row, bool),
    Rename(Row),
    Hide(Row),
    Lock(Row),
    DoubleSided(usize),
    Open(usize),
}

/// Los colores del outliner: los del skin o, sin Pixel UI, los de egui.
struct Look {
    text: Color32,
    selection: Color32,
}

/// El outliner del modelo que se edita, de `width` de ancho, en `ui` (adentro del scroll de la
/// mitad de abajo).
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut OutlinerState,
    width: f32,
) {
    let Some(mut model) = editor.edited_model().cloned() else {
        return;
    };
    model.normalize_tree();
    let mut rows = Vec::new();
    lines(&model, &model.tree, 0, false, &mut rows);
    let look = match skin {
        Some(skin) => Look {
            text: skin.colors().text,
            selection: skin.colors().selection,
        },
        None => Look {
            text: ui.visuals().text_color(),
            selection: ui.visuals().selection.bg_fill,
        },
    };
    let active = editor.active_row();
    // Lo elegido en las vistas (los objetos de lo elegido, en cualquier modo).
    let chosen: Vec<usize> = editor
        .model_edit()
        .selection
        .items
        .iter()
        .map(|it| it.object())
        .collect();
    let height = ROW_H * rows.len() as f32;
    let (slot, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let list = Rect::from_min_size(slot.min.round(), slot.size());
    // Supr con el puntero sobre la lista (la del frame anterior, como los atajos): ver
    // [`pointer_over_list`].
    let delete = pointer_over_list(ui.ctx())
        && state.rename.is_none()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
    let pass = ui.ctx().cumulative_pass_nr();
    let seen = list.intersect(ui.clip_rect());
    ui.data_mut(|d| d.insert_temp(list_id(), (pass, seen)));
    let shift = ui.input(|i| i.modifiers.shift);
    let mut action = None;
    let mut rects = Vec::with_capacity(rows.len());
    for (k, line) in rows.iter().enumerate() {
        let rect = Rect::from_min_size(
            pos2(list.left(), list.top() + k as f32 * ROW_H),
            vec2(width, ROW_H),
        );
        rects.push((line.row, rect, line.open.is_some()));
        if let Some(a) = row(ui, skin, &look, state, line, rect, (active, &chosen, shift)) {
            action = Some(a);
        }
    }
    match action {
        Some(Action::Choose(r, add)) => editor.set_active_row(Some(r), add),
        Some(Action::Rename(r)) => {
            let name = rows.iter().find(|l| l.row == r).map(|l| l.name.clone());
            state.rename = name.map(|n| (r, n));
        }
        Some(Action::Hide(r)) => {
            editor.toggle_hidden(r);
        }
        Some(Action::Lock(r)) => {
            editor.toggle_row_locked(r);
        }
        Some(Action::DoubleSided(o)) => {
            editor.toggle_double_sided(o);
        }
        Some(Action::Open(g)) => editor.toggle_open(g),
        None => {}
    }
    rename(ui, editor, skin, state, &rows, &rects);
    drop_row(ui, editor, &look, state, &rects);
    if delete && let Some(r) = editor.active_row() {
        editor.delete_row(r);
    }
}

/// Una fila en `rect`: el fondo (la activa, o elegida en las vistas), el triángulo, el ícono, el
/// nombre y los íconos de la derecha. Devuelve lo pedido.
fn row(
    ui: &mut Ui,
    skin: Option<&Skin>,
    look: &Look,
    state: &mut OutlinerState,
    line: &Line,
    rect: Rect,
    (active, chosen, shift): (Option<Row>, &[usize], bool),
) -> Option<Action> {
    let left = (rect.left() + line.depth as f32 * INDENT).round();
    // Lo que se toca (y su nodo), desde la sangría; el fondo, de lado a lado.
    let id = ui.id().with(("outliner_row", line.row));
    let hit = Rect::from_min_max(pos2(left, rect.top()), rect.max);
    let response = ui.interact(hit, id, Sense::click_and_drag());
    let enabled = ui.is_enabled();
    let name = line.name.clone();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &name));
    let is_active = active == Some(line.row);
    let in_views = match line.row {
        Row::Object(o) => chosen.contains(&o),
        Row::Group(_) => false,
    };
    let painter = ui.painter().clone();
    if is_active {
        painter.rect_filled(rect, 0.0, look.selection);
    } else if in_views {
        painter.rect_filled(rect, 0.0, look.selection.gamma_multiply(FADED));
    }
    let mut content = painter.clone();
    if line.inherited {
        content.set_opacity(FADED);
    }
    let renaming = matches!(&state.rename, Some((r, _)) if *r == line.row);
    let mut action = None;
    if response.drag_started() && !renaming {
        state.drag = Some(line.row);
    }
    if response.double_clicked() && !renaming {
        action = Some(Action::Rename(line.row));
    } else if response.clicked() && !renaming {
        action = Some(Action::Choose(line.row, shift));
    }
    if let (Row::Group(g), Some(open)) = (line.row, line.open) {
        let at = Rect::from_min_size(pos2(left, rect.top()), vec2(TRIANGLE_W, ROW_H));
        let tid = ui.id().with(("outliner_open", g));
        let toggle = ui.interact(at, tid, Sense::click());
        let label = if open {
            format!("Collapse {}", line.name)
        } else {
            format!("Expand {}", line.name)
        };
        toggle.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        let color = if toggle.hovered() {
            kit::icon::lighter(look.text)
        } else {
            look.text
        };
        triangle(&content, pos2(left + 1.0, rect.top() + 4.0), open, color);
        if toggle.clicked() {
            action = Some(Action::Open(g));
        }
    }
    let icon_at = pos2(left + TRIANGLE_W, rect.top() + ICON_TOP);
    if let Some(skin) = skin {
        piece(&content, skin, line.icon, icon_at, ButtonState::Normal);
    }
    // Los íconos de la derecha, del candado hacia la izquierda.
    let mut x = rect.right() - ICON;
    let mut toggles = vec![
        (
            if line.locked { "Unlock" } else { "Lock" },
            if line.locked {
                "icon.outliner.lock"
            } else {
                "icon.outliner.unlock"
            },
            true,
            Action::Lock(line.row),
        ),
        (
            if line.hidden { "Show" } else { "Hide" },
            if line.hidden {
                "icon.outliner.eye_off"
            } else {
                "icon.outliner.eye"
            },
            true,
            Action::Hide(line.row),
        ),
    ];
    if let (Row::Object(o), Some(on)) = (line.row, line.double_sided) {
        toggles.push((
            "Double sided",
            "icon.outliner.double_sided",
            on,
            Action::DoubleSided(o),
        ));
    }
    for (verb, icon, on, pick) in toggles {
        let at = pos2(x, rect.top() + ICON_TOP);
        let hit = Rect::from_min_size(pos2(x - 2.0, rect.top()), vec2(TOGGLE_PITCH, ROW_H));
        let tid = ui.id().with(("outliner_toggle", verb, line.row));
        let toggle = ui.interact(hit, tid, Sense::click());
        let label = format!("{verb} {}", line.name);
        toggle.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, &label));
        let mut p = content.clone();
        if !on {
            p.set_opacity(p.opacity() * FADED);
        }
        let state = ButtonState::of(&toggle, enabled);
        match skin {
            Some(skin) => piece(&p, skin, icon, at, state),
            None => plain_mark(&p, at, verb, look.text),
        }
        if toggle.clicked() {
            action = Some(pick);
        }
        x -= TOGGLE_PITCH;
    }
    // El nombre, recortado antes de los íconos de la derecha.
    if !renaming {
        let name_left = icon_at.x + ICON + NAME_GAP;
        let clip = Rect::from_min_max(pos2(name_left, rect.top()), pos2(x + ICON, rect.bottom()))
            .intersect(painter.clip_rect());
        let font = egui::TextStyle::Body.resolve(ui.style());
        let galley = painter.layout_no_wrap(line.name.clone(), font, look.text);
        let ppp = ui.ctx().pixels_per_point();
        let at = crate::tab_bar::text_pos(&galley, name_left, rect.top() + CAPS_TOP, ppp);
        content.with_clip_rect(clip).galley(at, galley, look.text);
    }
    action
}

/// El ▸ (cerrado) o el ▾ (abierto) de 5 px con la esquina en `at`.
fn triangle(painter: &Painter, at: Pos2, open: bool, color: Color32) {
    for k in 0..3 {
        let k = k as f32;
        let r = if open {
            Rect::from_min_size(at + vec2(k, k), vec2(TRIANGLE - 2.0 * k, 1.0))
        } else {
            Rect::from_min_size(at + vec2(k, k), vec2(1.0, TRIANGLE - 2.0 * k))
        };
        painter.rect_filled(r, 0.0, color);
    }
}

/// La pieza `name` del skin con la esquina en `at`, en `state` (con la opacidad de `painter`).
fn piece(painter: &Painter, skin: &Skin, name: &str, at: Pos2, state: ButtonState) {
    if let Some(p) = skin.piece(name, WidgetState::Normal) {
        let ppp = painter.ctx().pixels_per_point();
        painter.extend(kit::icon::icon_shapes(p, at, state, ppp));
    }
}

/// Sin Pixel UI: la inicial del ícono de la derecha (`verb`) en lugar de su pieza.
fn plain_mark(painter: &Painter, at: Pos2, verb: &str, color: Color32) {
    let mark = verb.chars().next().unwrap_or('?').to_string();
    let font = egui::FontId::proportional(9.0);
    let center = at + Vec2::splat(ICON / 2.0);
    painter.text(center, egui::Align2::CENTER_CENTER, mark, font, color);
}

/// El campo de renombrar sobre el nombre de la fila que se renombra: Enter (o salir) confirma
/// con `rename_row`, Esc cancela.
fn rename(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut OutlinerState,
    rows: &[Line],
    rects: &[(Row, Rect, bool)],
) {
    let Some((r, _)) = &state.rename else {
        return;
    };
    let r = *r;
    let (Some(line), Some((_, rect, _))) = (
        rows.iter().find(|l| l.row == r),
        rects.iter().find(|(row, _, _)| *row == r),
    ) else {
        // La fila ya no se ve (se cerró su grupo o se borró).
        state.rename = None;
        return;
    };
    let left = (rect.left() + line.depth as f32 * INDENT).round();
    let name_left = left + TRIANGLE_W + ICON + NAME_GAP;
    let toggles = if line.double_sided.is_some() {
        3.0
    } else {
        2.0
    };
    let right = rect.right() - toggles * TOGGLE_PITCH;
    let name_rect = Rect::from_min_max(pos2(name_left, rect.top()), pos2(right, rect.bottom()));
    let Some((_, text)) = state.rename.as_mut() else {
        return;
    };
    let edit = match skin {
        // El campo flotante del kit (el de renombrar capas), con el texto donde está el nombre.
        Some(skin) => {
            let at = pos2(
                name_rect.left() - kit::input::TEXT_LEFT,
                name_rect.top() + CAPS_TOP - kit::input::FLOATING_CAPS,
            );
            let min_width = name_rect.width() + 2.0 * kit::input::TEXT_LEFT;
            egui::Area::new(ui.id().with(("outliner_rename", r)))
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .show(ui.ctx(), |ui| {
                    kit::input::floating_input(ui, skin, at, min_width, text, "Rename")
                })
                .inner
        }
        None => ui.put(name_rect, egui::TextEdit::singleline(text)),
    };
    if !edit.has_focus() && !edit.lost_focus() {
        edit.request_focus();
        // Todo el nombre elegido: lo que se escriba lo reemplaza.
        let mut s = egui::TextEdit::load_state(ui.ctx(), edit.id).unwrap_or_default();
        let all = egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(text.chars().count()),
        );
        s.cursor.set_char_range(Some(all));
        s.store(ui.ctx(), edit.id);
    }
    if edit.lost_focus() {
        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        let Some((row, name)) = state.rename.take() else {
            return;
        };
        if !cancel {
            editor.rename_row(row, name);
        }
    }
}

/// Mientras se arrastra una fila: dónde caería bajo el puntero (marcado con una línea de 1 px,
/// o un recuadro sobre el grupo donde entraría) y, al soltar, `move_row`.
fn drop_row(
    ui: &Ui,
    editor: &mut Editor,
    look: &Look,
    state: &mut OutlinerState,
    rects: &[(Row, Rect, bool)],
) {
    let Some(dragged) = state.drag else {
        return;
    };
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let target = pointer.and_then(|p| {
        let (row, rect, group) = rects
            .iter()
            .find(|(_, r, _)| (r.top()..r.bottom()).contains(&p.y) && r.x_range().contains(p.x))?;
        let at = (p.y - rect.top()) / rect.height();
        let to = if *group && (INTO_FROM..INTO_TO).contains(&at) {
            match row {
                Row::Group(g) => DropAt::Into(*g),
                Row::Object(_) => DropAt::After(*row),
            }
        } else if at < 0.5 {
            DropAt::Before(*row)
        } else {
            DropAt::After(*row)
        };
        Some((to, *rect))
    });
    let released = ui.input(|i| !i.pointer.any_down());
    if released {
        state.drag = None;
        if let Some((to, _)) = target {
            editor.move_row(dragged, to);
        }
        return;
    }
    let Some((to, rect)) = target else {
        return;
    };
    let painter = ui.painter();
    match to {
        DropAt::Before(_) => {
            let line = Rect::from_min_size(rect.min, vec2(rect.width(), 1.0));
            painter.rect_filled(line, 0.0, look.text);
        }
        DropAt::After(_) => {
            let line = Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - 1.0),
                vec2(rect.width(), 1.0),
            );
            painter.rect_filled(line, 0.0, look.text);
        }
        DropAt::Into(_) => {
            let stroke = egui::Stroke::new(1.0, look.text);
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
        }
    }
}
