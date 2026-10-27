//! File → Import Sprite Sheet…, como en Aseprite: una imagen con los frames en celdas se parte
//! en un sprite nuevo, un frame por celda. Se ve la hoja con la cuadrícula del corte.

use cs_io::export::Bitmap;
use cs_io::import::{SheetGrid, sprite_from_sheet};
use eframe::egui::{self, Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, vec2};

use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

/// El lado más largo de la vista previa.
const PREVIEW: f32 = 220.0;

pub struct SheetDialog {
    /// El nombre del archivo, sin la extensión.
    pub name: String,
    sheet: Bitmap,
    pub grid: SheetGrid,
    texture: Option<TextureHandle>,
}

impl SheetDialog {
    /// Con celdas cuadradas si la hoja es una tira (una fila o una columna de cuadrados); si no,
    /// la hoja entera.
    pub fn new(name: String, sheet: Bitmap) -> Self {
        let (w, h) = (sheet.width, sheet.height);
        let cell = if w > h && w % h == 0 {
            (h, h)
        } else if h > w && h % w == 0 {
            (w, w)
        } else {
            (w, h)
        };
        Self {
            name,
            sheet,
            grid: SheetGrid {
                cell,
                padding: (0, 0),
                offset: (0, 0),
            },
            texture: None,
        }
    }

    /// El sprite con los valores de ahora.
    pub fn sprite(&self) -> Option<cs_core::Sprite> {
        sprite_from_sheet(&self.sheet, self.grid)
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let mut result = DialogResult::Open;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "import_sheet",
            "Import Sprite Sheet",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                self.contents(ui);
                ui.add_space(6.0);
                match pixel_ui::dialog_buttons(ui, skin, &["Import", "Cancel"]) {
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
        if closed || escape {
            DialogResult::Cancel
        } else if enter {
            DialogResult::Accept
        } else {
            result
        }
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.sheet.width, self.sheet.height);
        let grid = &mut self.grid;
        egui::Grid::new("sheet_fields")
            .num_columns(3)
            .spacing(vec2(8.0, 4.0))
            .show(ui, |ui| {
                ui.label("Frame Size");
                widgets::number(ui, &mut grid.cell.0, 1..=w, " px");
                widgets::number(ui, &mut grid.cell.1, 1..=h, " px");
                ui.end_row();
                ui.label("Padding");
                widgets::number(ui, &mut grid.padding.0, 0..=w, " px");
                widgets::number(ui, &mut grid.padding.1, 0..=h, " px");
                ui.end_row();
                ui.label("Offset");
                widgets::number(ui, &mut grid.offset.0, 0..=w, " px");
                widgets::number(ui, &mut grid.offset.1, 0..=h, " px");
                ui.end_row();
            });
        let (cols, rows) = self.grid.count((w, h));
        ui.label(format!(
            "{cols} × {rows} cells from a {w}×{h} image ({} frames)",
            cols * rows
        ));
        self.preview(ui, cols, rows);
    }

    /// La hoja achicada (o agrandada) a `PREVIEW`, con el borde de cada celda.
    fn preview(&mut self, ui: &mut egui::Ui, cols: u32, rows: u32) {
        let sheet = &self.sheet;
        let texture = self.texture.get_or_insert_with(|| {
            let bytes: Vec<u8> = sheet
                .pixels
                .iter()
                .flat_map(|c| [c.r, c.g, c.b, c.a])
                .collect();
            let image = ColorImage::from_rgba_unmultiplied(
                [sheet.width as usize, sheet.height as usize],
                &bytes,
            );
            ui.ctx()
                .load_texture("import_sheet", image, egui::TextureOptions::NEAREST)
        });
        let k = PREVIEW / sheet.width.max(sheet.height) as f32;
        let size = vec2(sheet.width as f32, sheet.height as f32) * k;
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        crate::panels::checker(&painter, rect);
        let full = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        let g = self.grid;
        let stroke = Stroke::new(1.0, crate::theme::ACCENT);
        for row in 0..rows {
            for col in 0..cols {
                let x = g.offset.0 + col * (g.cell.0 + g.padding.0);
                let y = g.offset.1 + row * (g.cell.1 + g.padding.1);
                let cell = Rect::from_min_size(
                    rect.min + vec2(x as f32, y as f32) * k,
                    vec2(g.cell.0 as f32, g.cell.1 as f32) * k,
                );
                painter.rect_stroke(cell, 0.0, stroke, egui::StrokeKind::Inside);
            }
        }
    }
}
