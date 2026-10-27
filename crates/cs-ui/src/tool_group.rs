//! Botón de un grupo de la barra de herramientas, compartido por la barra con nombres y la de
//! íconos (Pixel UI): clic para elegir la herramienta del grupo; clic derecho o mantener
//! presionado para la lista de variantes.

use eframe::egui::{self, Response, Ui, vec2};

use crate::editor::{Editor, ToolKind};
use crate::pixel_ui::Tip as _;
use crate::skin::{Skin, WidgetState};

/// Mantener presionado el botón de un grupo este tiempo (en segundos) abre la lista de variantes.
const LONG_PRESS_S: f64 = 0.4;

/// Segunda línea del tooltip de los grupos con variantes.
const MORE_HINT: &str = "Right-click or hold for more";

/// Cómo se dibuja el botón de un grupo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonState {
    /// La herramienta activa es de este grupo.
    pub selected: bool,
    /// El grupo tiene más de una herramienta.
    pub more: bool,
}

/// Emoji de la barra con nombres (están en las fuentes de egui).
pub fn emoji(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "🔲",
        ToolKind::EllipseSelect => "🔘",
        ToolKind::Lasso => "➰",
        ToolKind::PolygonLasso => "🔷",
        ToolKind::MagicWand => "✨",
        ToolKind::Move => "✚",
        ToolKind::Hand => "✋",
        ToolKind::Contour => "➿",
        ToolKind::Blur => "☁",
        ToolKind::Pencil => "✏",
        ToolKind::Spray => "💨",
        ToolKind::Eraser => "🗑",
        ToolKind::Eyedropper => "💧",
        ToolKind::Bucket => "🍶",
        ToolKind::Gradient => "🌈",
        ToolKind::Line => "📏",
        ToolKind::Rectangle => "⬜",
        ToolKind::Ellipse => "⭕",
        ToolKind::Polygon => "⬟",
        ToolKind::Jumble => "🎲",
        ToolKind::Text => "🔤",
        ToolKind::Slice => "✂",
        ToolKind::Artboard => "🖼",
    }
}

/// Pieza del skin con el ícono de la herramienta.
pub fn icon_piece(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "icon.marquee",
        ToolKind::EllipseSelect => "icon.ellipse_marquee",
        ToolKind::Lasso => "icon.lasso",
        ToolKind::PolygonLasso => "icon.polygon_lasso",
        ToolKind::MagicWand => "icon.magic_wand",
        ToolKind::Move => "icon.move",
        ToolKind::Hand => "icon.hand",
        ToolKind::Contour => "icon.contour",
        ToolKind::Blur => "icon.blur",
        ToolKind::Pencil => "icon.pencil",
        ToolKind::Spray => "icon.spray",
        ToolKind::Eraser => "icon.eraser",
        ToolKind::Eyedropper => "icon.eyedropper",
        ToolKind::Bucket => "icon.bucket",
        ToolKind::Gradient => "icon.gradient",
        ToolKind::Line => "icon.line",
        ToolKind::Rectangle => "icon.rectangle",
        ToolKind::Ellipse => "icon.ellipse",
        ToolKind::Polygon => "icon.polygon",
        ToolKind::Jumble => "icon.jumble",
        ToolKind::Text => "icon.text",
        ToolKind::Slice => "icon.slice",
        ToolKind::Artboard => "icon.artboard",
    }
}

/// "Nombre (Atajo)" y, si el grupo tiene variantes, una segunda línea que dice cómo verlas.
pub fn tooltip(ctx: &egui::Context, kind: ToolKind, more: bool) -> String {
    let keys = crate::keymap::label(ctx, crate::keymap::Action::Tool(kind));
    let first = format!("{} ({keys})", kind.label());
    if more {
        format!("{first}\n{MORE_HINT}")
    } else {
        first
    }
}

/// Qué pasa con el botón de un grupo mantenido presionado, en este frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    /// Nada que hacer.
    Idle,
    /// Se cumplió el tiempo: abrir la lista.
    Open,
    /// Se suelta el botón con la lista abierta por esta presión: el clic no elige ni cierra.
    Release,
}

/// Lleva la cuenta de una presión larga con el botón principal sobre `response`.
fn hold_state(ui: &Ui, response: &Response) -> Hold {
    let id = response.id.with("long_press");
    let opened = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let (down, held) = ui.input(|i| {
        let held = i.pointer.press_start_time().map(|t| i.time - t);
        (i.pointer.primary_down(), held.unwrap_or(0.0))
    });
    if down && response.is_pointer_button_down_on() {
        if opened {
            return Hold::Idle;
        }
        if held >= LONG_PRESS_S {
            ui.data_mut(|d| d.insert_temp(id, true));
            return Hold::Open;
        }
        // Sin eventos egui no vuelve a pintar: pide un frame para cuando se cumpla el tiempo.
        ui.ctx()
            .request_repaint_after_secs((LONG_PRESS_S - held) as f32);
        return Hold::Idle;
    }
    if opened {
        ui.data_mut(|d| d.remove::<bool>(id));
        return Hold::Release;
    }
    Hold::Idle
}

/// Lista de variantes de un grupo junto al puntero. Se abre con clic derecho o manteniendo
/// presionado, y un clic normal en el botón la cierra. Usa el mismo id que `context_menu`.
fn variants_popup(response: &Response, hold: Hold, add_contents: impl FnOnce(&mut Ui)) {
    use egui::{Popup, PopupCloseBehavior};
    let open = open_command(response, hold);
    let mut popup = Popup::menu(response).open_memory(open).at_pointer_fixed();
    if hold == Hold::Release {
        // Soltar el botón cuenta como clic: no tiene que cerrar la lista recién abierta.
        popup = popup.close_behavior(PopupCloseBehavior::IgnoreClicks);
    }
    popup.show(add_contents);
}

/// Desarrollo: `CHIMPSPRITE_DEMO_TOOL_MENU=<grupo>` abre el menú de ese grupo al arrancar (una
/// vez), para capturas.
fn demo_open(ui: &Ui, group: usize) -> Option<egui::SetOpenCommand> {
    let wanted = crate::demo::dev_var("CHIMPSPRITE_DEMO_TOOL_MENU")?;
    if wanted.trim().parse::<usize>().ok()? != group {
        return None;
    }
    let id = egui::Id::new(("demo_tool_menu", group));
    let done = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if done || ui.input(|i| i.time) < 1.0 {
        return None;
    }
    ui.data_mut(|d| d.insert_temp(id, true));
    Some(egui::SetOpenCommand::Bool(true))
}

/// Con Pixel UI la lista se abre (o se cierra) con el clic mismo, además de con clic derecho o
/// manteniendo: el clic también elige la herramienta que muestra el botón.
fn pixel_open_command(response: &Response, hold: Hold) -> Option<egui::SetOpenCommand> {
    use egui::SetOpenCommand;
    if hold == Hold::Open || response.secondary_clicked() {
        Some(SetOpenCommand::Bool(true))
    } else if response.clicked() && hold != Hold::Release {
        Some(SetOpenCommand::Toggle)
    } else {
        None
    }
}

/// Abrir la lista (clic derecho o mantener) o cerrarla (un clic que no es el de soltar).
fn open_command(response: &Response, hold: Hold) -> Option<egui::SetOpenCommand> {
    use egui::SetOpenCommand;
    if hold == Hold::Open || response.secondary_clicked() {
        Some(SetOpenCommand::Bool(true))
    } else if response.clicked() && hold != Hold::Release {
        Some(SetOpenCommand::Bool(false))
    } else {
        None
    }
}

/// Con Pixel UI: las herramientas del grupo con su ícono, su nombre y su atajo, separadas por la
/// línea de los menús. Devuelve la elegida y cierra el menú.
fn pixel_variants(
    ui: &mut Ui,
    skin: &Skin,
    current: ToolKind,
    group: &[ToolKind],
) -> Option<ToolKind> {
    let mut pick = None;
    for (i, &kind) in group.iter().enumerate() {
        if i > 0 {
            crate::menus::separator(ui);
        }
        let icon = skin.piece(icon_piece(kind), WidgetState::Normal);
        let keys = crate::keymap::label(ui.ctx(), crate::keymap::Action::Tool(kind));
        let row = crate::menus::tool_row(ui, icon, kind.label(), &keys, current == kind);
        if row.clicked() {
            pick = Some(kind);
            ui.close();
        }
    }
    pick
}

/// Las herramientas de un grupo, una por fila con su atajo. Con skin, cada una lleva su ícono a
/// tamaño de pieza (la escala de la UI); sin skin o si falta el ícono, el emoji de siempre.
/// Devuelve la elegida y cierra el menú.
pub fn variants_list(
    ui: &mut Ui,
    skin: Option<&Skin>,
    current: ToolKind,
    group: &[ToolKind],
) -> Option<ToolKind> {
    let mut pick = None;
    for &kind in group {
        let selected = current == kind;
        let icon = skin.and_then(|s| s.piece(icon_piece(kind), WidgetState::Normal));
        let item = match icon {
            Some(piece) => {
                let size = vec2(piece.size[0] as f32, piece.size[1] as f32);
                let image =
                    egui::Image::new(egui::load::SizedTexture::new(piece.texture.id(), size))
                        .fit_to_exact_size(size);
                egui::Button::selectable(selected, (image, kind.label()))
            }
            None => egui::Button::selectable(selected, format!("{} {}", emoji(kind), kind.label())),
        };
        let keys = crate::keymap::label(ui.ctx(), crate::keymap::Action::Tool(kind));
        if ui.add(item.shortcut_text(keys)).clicked() {
            pick = Some(kind);
            ui.close();
        }
    }
    pick
}

/// El botón del grupo `group` de `ToolKind::GROUPS`. `draw` lo dibuja con la herramienta que
/// muestra el grupo y su estado, y devuelve su `Response`.
///
/// - Clic: elige esa herramienta.
/// - Clic derecho o mantener presionado 400 ms (si el grupo tiene variantes): abre la lista;
///   soltar después de mantener no cuenta como clic.
/// - Elegir en la lista: cambia la herramienta, que queda recordada en el grupo.
/// - Tooltip: "Nombre (Atajo)" y, con variantes, cómo verlas. Con skin (Pixel UI), en el globo
///   del diseño (`pixel_ui::bubble_tooltip`).
pub fn group_button(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    group: usize,
    draw: impl FnOnce(&mut Ui, ToolKind, ButtonState) -> Response,
) -> Response {
    let members = ToolKind::GROUPS[group];
    let shown = editor.group_tool(group);
    let state = ButtonState {
        selected: members.contains(&editor.tool),
        more: members.len() > 1,
    };
    let response = draw(ui, shown, state);
    let hold = if state.more {
        hold_state(ui, &response)
    } else {
        Hold::Idle
    };
    let mut pick = None;
    if response.clicked() && hold != Hold::Release {
        pick = Some(shown);
    }
    if state.more {
        let current = editor.tool;
        match skin {
            // Pixel UI: el panel del diseño, saliendo del botón, una herramienta por fila.
            Some(skin) => {
                let look = crate::menus::MenuLook::new(skin, response.rect.top());
                let picked = crate::menus::side_menu(
                    &look,
                    &response,
                    response.rect,
                    pixel_open_command(&response, hold).or_else(|| demo_open(ui, group)),
                    hold == Hold::Release,
                    |ui| pixel_variants(ui, skin, current, members),
                );
                if let Some(Some(kind)) = picked {
                    pick = Some(kind);
                }
            }
            None => variants_popup(&response, hold, |ui| {
                if let Some(kind) = variants_list(ui, skin, current, members) {
                    pick = Some(kind);
                }
            }),
        }
    }
    if let Some(kind) = pick {
        editor.set_tool(kind);
    }
    let text = tooltip(ui.ctx(), shown, state.more);
    match skin {
        // Pixel UI: el globo del diseño, a la derecha del botón.
        Some(skin) => crate::pixel_ui::bubble_tooltip(response, skin, &text),
        None => response.tip(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tooltip_has_the_name_the_shortcut_and_how_to_see_the_variants() {
        assert_eq!(
            tooltip(&egui::Context::default(), ToolKind::Hand, false),
            "Hand (H)"
        );
        assert_eq!(
            tooltip(&egui::Context::default(), ToolKind::Lasso, true),
            "Lasso (Q)\nRight-click or hold for more"
        );
    }

    #[test]
    fn every_tool_has_its_own_icon_in_the_embedded_skin() {
        let data = crate::skin::decode(crate::skin::EMBEDDED).unwrap();
        let kinds: Vec<ToolKind> = ToolKind::GROUPS
            .iter()
            .flat_map(|g| g.iter().copied())
            .collect();
        for (i, &kind) in kinds.iter().enumerate() {
            let name = icon_piece(kind);
            assert_eq!(data.pieces[name].size, [17, 17], "{kind:?}");
            assert!(
                kinds[..i].iter().all(|&k| icon_piece(k) != name),
                "{name} twice"
            );
        }
    }
}
