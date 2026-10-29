//! Frame → Frame Properties…, como en Aseprite: la duración de los frames elegidos (o del
//! actual), en un paso de undo.

use eframe::egui;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

pub struct FramePropertiesDialog {
    duration_ms: u16,
}

impl FramePropertiesDialog {
    /// Con la duración del primer frame elegido.
    pub fn new(editor: &Editor) -> Self {
        let (first, _) = editor.selected_frames();
        Self {
            duration_ms: editor.sprite().frames[first].duration_ms,
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        skin: Option<&Skin>,
    ) -> DialogResult {
        let (a, b) = editor.selected_frames();
        let title = if a == b {
            format!("Frame {} Properties", a + 1)
        } else {
            format!("Frames {}-{} Properties", a + 1, b + 1)
        };
        let mut result = DialogResult::Open;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "frame_properties",
            &title,
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Duration");
                    widgets::number(ui, &mut self.duration_ms, 1..=65535, " ms");
                });
                ui.add_space(6.0);
                match pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"]) {
                    Some(0) => result = DialogResult::Accept,
                    Some(_) => result = DialogResult::Cancel,
                    None => {}
                }
            },
        );
        let (enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let result = if closed || escape {
            DialogResult::Cancel
        } else if enter {
            DialogResult::Accept
        } else {
            result
        };
        if matches!(result, DialogResult::Accept) {
            editor.set_duration(self.duration_ms);
        }
        result
    }
}
