//! Expand to Spritesheet… y Slice into Frames… del menú de un artboard: abren sus diálogos con lo
//! que se ve del artboard activo (instancias y tilemaps resueltos) y agregan el artboard nuevo a
//! la derecha de todo, activo, en un paso de undo.

use eframe::egui;

use crate::ChimpApp;
use crate::expand_dialog::ExpandDialog;
use crate::sheet_dialog::SheetDialog;
use crate::tilemap_dialog::DialogResult;

impl ChimpApp {
    /// Expand to Spritesheet… del artboard activo (con todos sus frames resueltos).
    pub(crate) fn open_expand_sheet(&mut self) {
        let editor = &mut self.doc.editor;
        if !editor.can_play_in_place(editor.active) {
            return;
        }
        editor.commit_floating();
        let artboard = editor.artboard();
        let frames = 0..artboard.sprite.frames.len();
        let sprite =
            cs_render::materialize(&editor.document, &artboard.sprite, frames).into_owned();
        self.expand_dialog = Some(ExpandDialog::new(artboard.name.clone(), sprite));
    }

    /// Slice into Frames… del frame que muestra el artboard activo, aplanado.
    pub(crate) fn open_slice_frames(&mut self) {
        let editor = &mut self.doc.editor;
        let plain = editor.artboard().font.is_none() && editor.artboard().model.is_none();
        if !plain {
            return;
        }
        editor.commit_floating();
        let frame = editor.frame;
        let artboard = editor.artboard();
        let sprite = cs_render::materialize(&editor.document, &artboard.sprite, frame..frame + 1);
        let sheet = cs_io::export::Bitmap::from_frame(&sprite, frame, 1);
        self.sheet_dialog = Some(SheetDialog::slice(
            artboard.name.clone(),
            sheet,
            artboard.sprite.grid,
            artboard.sprite.palette.clone(),
            artboard.sprite.color_mode,
        ));
    }

    pub(crate) fn update_expand_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.expand_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.expand_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            DialogResult::Open => {}
            DialogResult::Accept => match dialog.sheet() {
                Ok(sheet) => {
                    let name = format!("{} Sheet", dialog.name);
                    self.expand_dialog = None;
                    self.doc.editor.add_artboard_with_sprite(name, sheet);
                }
                Err(e) => self.show_message("Can't expand", e),
            },
            DialogResult::Cancel => self.expand_dialog = None,
        }
    }

    /// El artboard nuevo de Slice into Frames (lo que aceptó `update_sheet_dialog`).
    pub(crate) fn add_sliced_artboard(&mut self, name: &str, sprite: cs_core::Sprite) {
        if self.has_doc() {
            self.doc
                .editor
                .add_artboard_with_sprite(format!("{name} Frames"), sprite);
        }
    }
}

#[cfg(test)]
#[path = "artboard_sheets_tests.rs"]
mod tests;
