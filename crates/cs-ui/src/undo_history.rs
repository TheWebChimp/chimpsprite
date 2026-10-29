//! Edit → Undo History, como en Aseprite: una ventana con los pasos del historial, del estado
//! inicial al último. Los deshechos (los que se pueden rehacer) van más tenues; un clic en un
//! paso deshace o rehace hasta ahí.

use eframe::egui::{self, Align2, Response, RichText, Sense, Ui, WidgetInfo, WidgetType, vec2};

use crate::editor::Editor;
use crate::skin::Skin;

/// El alto de la lista.
const LIST_HEIGHT: f32 = 220.0;
const WIDTH: f32 = 160.0;

impl Editor {
    /// Cuántos pasos hay hechos (0 es el estado inicial).
    pub fn steps_done(&self) -> usize {
        self.history.done().count()
    }

    /// Deshace o rehace hasta que queden `done` pasos hechos.
    pub fn go_to_step(&mut self, done: usize) {
        self.commit_floating();
        while self.steps_done() > done && self.history.can_undo() {
            self.undo();
        }
        while self.steps_done() < done && self.history.can_redo() {
            self.redo();
        }
    }
}

/// La ventana. Devuelve si se cerró con la ×.
pub fn show(ctx: &egui::Context, editor: &mut Editor, skin: Option<&Skin>) -> bool {
    let anchor = (Align2::RIGHT_TOP, vec2(-48.0, 96.0));
    let (_, closed) = crate::pixel_ui::app_window(
        ctx,
        skin,
        "undo_history",
        "Undo History",
        true,
        anchor,
        |ui| contents(ui, editor, skin),
    );
    closed
}

fn contents(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>) {
    let done: Vec<&str> = editor.history.done().collect();
    let undone: Vec<&str> = editor.history.undone().collect();
    let current = done.len();
    let mut go = None;
    ui.set_width(WIDTH);
    let rows = std::iter::once("Initial State")
        .chain(done.iter().copied())
        .chain(undone.iter().copied())
        .enumerate();
    crate::pixel_ui::scroll(
        ui,
        egui::ScrollArea::vertical()
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true]),
        |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (step, label) in rows {
                let row = step_row(ui, skin, label, step == current, step > current);
                if step == current {
                    row.scroll_to_me(None);
                }
                if row.clicked() {
                    go = Some(step);
                }
            }
        },
    );
    if let Some(step) = go {
        editor.go_to_step(step);
    }
}

/// Un paso de la lista. Con Pixel UI, una fila como las de los menús: el actual en el rosa de lo
/// elegido, los demás con el resaltado de los menús bajo el puntero y los deshechos más tenues.
fn step_row(
    ui: &mut Ui,
    skin: Option<&Skin>,
    label: &str,
    current: bool,
    undone: bool,
) -> Response {
    let Some(skin) = skin else {
        let mut text = RichText::new(label);
        if undone {
            text = text.color(ui.visuals().weak_text_color());
        }
        return ui.add(egui::Button::selectable(current, text));
    };
    let colors = skin.colors();
    let color = if undone { colors.text_dim } else { colors.text };
    let text = crate::menus::galley(ui, label, color);
    let size = vec2(ui.available_width(), crate::menus::ROW_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), current, label)
    });
    if ui.is_rect_visible(rect) {
        let ppp = ui.ctx().pixels_per_point();
        let fill = if current {
            Some(colors.timeline_selection)
        } else if response.hovered() {
            Some(colors.menu_highlight)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, 0.0, fill);
        }
        let at = crate::tab_bar::text_pos(&text, rect.left() + ROW_PAD, rect.top() + ROW_CAPS, ppp);
        ui.painter().galley(at, text, color);
    }
    response
}

/// El texto de una fila: 3 px adentro y las mayúsculas 3 filas abajo (como en los menús).
const ROW_PAD: f32 = 3.0;
const ROW_CAPS: f32 = 3.0;

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, LayerKind, PixelValue, Rgba, Sprite};

    use super::*;

    #[test]
    fn going_to_a_step_undoes_or_redoes_up_to_it() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red)],
        });
        e.add_layer(LayerKind::Image);
        e.insert_empty_frame();
        assert_eq!(
            e.history.done().collect::<Vec<_>>(),
            ["Draw", "New Layer", "New Frame"]
        );
        e.go_to_step(1);
        assert_eq!(e.steps_done(), 1);
        assert_eq!(e.sprite().layers.len(), 1, "before the new layer");
        assert_eq!(e.history.undone().count(), 2);
        e.go_to_step(3);
        assert_eq!(e.frame_count(), 2, "redone");
        e.go_to_step(0);
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 0, 0),
            e.sprite().color_mode.transparent()
        );
    }

    #[test]
    fn steps_are_named_after_the_tool_or_the_menu_item() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.primary = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        e.set_tool(crate::editor::ToolKind::Pencil);
        crate::tool_tests::stroke(&mut e, &[(1, 1), (3, 1)], false);
        e.select_all();
        e.fill_selection();
        e.flip(true);
        e.commit_floating();
        let clip = e.copy().expect("copied");
        e.paste(&clip);
        e.commit_floating();
        e.invert_colors();
        e.apply_filter(&crate::filters::Filter::ReplaceColor {
            from: Rgba::rgb(0, 255, 255),
            to: Rgba::rgb(0, 0, 255),
            tolerance: 0,
        });
        e.cut();
        assert_eq!(
            e.history.done().collect::<Vec<_>>(),
            [
                "Pencil",
                "Fill",
                "Flip Horizontal",
                "Paste",
                "Invert",
                "Replace Color",
                "Cut"
            ]
        );
    }
}
