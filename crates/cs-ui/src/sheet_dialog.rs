//! File → Import Sprite Sheet…, como en Aseprite: una imagen con los frames en celdas se parte
//! en un sprite nuevo, un frame por celda. Se ve la hoja con la cuadrícula del corte.
//!
//! El mismo diálogo es Slice into Frames… del menú de un artboard ([`SheetDialog::slice`]): parte
//! el frame que muestra el artboard en un artboard nuevo animado, con su paleta.
//!
//! - La celda se da por tamaño (**Cell Size**, con padding y offset) o por cantidad
//!   (**Columns × Rows**: la celda que deja esa cantidad en la hoja).
//! - **Order:** por filas o por columnas. **Duration:** la de cada frame.

use cs_core::{ColorMode, GridSettings, Palette};
use cs_io::export::Bitmap;
use cs_io::import::{SheetGrid, sprite_from_sheet_ordered};
use cs_io::spritesheet::{SheetOrder, adopt_palette};
use eframe::egui::{self, Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, vec2};

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El lado más largo de la vista previa.
const PREVIEW: f32 = 220.0;
/// La duración de cada frame por defecto (la de un frame nuevo).
const DURATION_MS: u16 = 100;

/// Cómo se da la celda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CutMode {
    /// Por su tamaño (`SheetDialog::grid`).
    CellSize,
    /// Por cuántas columnas y filas hay (`SheetDialog::counts`).
    Counts,
}

/// Para qué se abrió: importar un archivo o partir un artboard.
#[derive(Clone, Debug, PartialEq)]
pub enum SheetPurpose {
    Import,
    /// Slice into Frames: el artboard nuevo se lleva la paleta y, si se puede, el modo de color.
    Slice {
        palette: Palette,
        mode: ColorMode,
    },
}

pub struct SheetDialog {
    /// El nombre del archivo, sin la extensión (o el del artboard).
    pub name: String,
    pub purpose: SheetPurpose,
    sheet: Bitmap,
    pub grid: SheetGrid,
    pub mode: CutMode,
    /// Columnas y filas con `CutMode::Counts`.
    pub counts: (u32, u32),
    pub order: SheetOrder,
    pub duration_ms: u16,
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
        let grid = SheetGrid {
            cell,
            padding: (0, 0),
            offset: (0, 0),
        };
        Self {
            name,
            purpose: SheetPurpose::Import,
            counts: grid.count((w, h)),
            sheet,
            grid,
            mode: CutMode::CellSize,
            order: SheetOrder::ByRows,
            duration_ms: DURATION_MS,
            texture: None,
        }
    }

    /// Slice into Frames de un artboard: `sheet` es su frame aplanado. Si su grilla no es la de
    /// por defecto y deja al menos dos celdas, la celda es la de la grilla (con su offset); si
    /// no, la sugerencia del import.
    pub fn slice(
        name: String,
        sheet: Bitmap,
        grid: GridSettings,
        palette: Palette,
        mode: ColorMode,
    ) -> Self {
        let size = (sheet.width, sheet.height);
        let mut dialog = Self::new(name, sheet);
        dialog.purpose = SheetPurpose::Slice { palette, mode };
        if grid != GridSettings::default() {
            let (gw, gh) = (grid.size.0.max(1), grid.size.1.max(1));
            let offset = (
                grid.offset.0.rem_euclid(gw as i32) as u32,
                grid.offset.1.rem_euclid(gh as i32) as u32,
            );
            let candidate = SheetGrid {
                cell: (gw, gh),
                padding: (0, 0),
                offset,
            };
            let (cols, rows) = candidate.count(size);
            if cols * rows >= 2 {
                dialog.grid = candidate;
                dialog.counts = (cols, rows);
            }
        }
        dialog
    }

    /// La celda con la que se corta: la del tamaño o la que dejan las columnas y filas.
    pub fn cut_grid(&self) -> SheetGrid {
        match self.mode {
            CutMode::CellSize => self.grid,
            CutMode::Counts => SheetGrid::from_counts(
                (self.sheet.width, self.sheet.height),
                self.counts,
                self.grid.padding,
                self.grid.offset,
            ),
        }
    }

    /// El sprite con los valores de ahora.
    pub fn sprite(&self) -> Option<cs_core::Sprite> {
        let mut sprite =
            sprite_from_sheet_ordered(&self.sheet, self.cut_grid(), self.order, self.duration_ms)?;
        if let SheetPurpose::Slice { palette, mode } = &self.purpose {
            adopt_palette(&mut sprite, palette, *mode);
        }
        Some(sprite)
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (id, title, accept) = match self.purpose {
            SheetPurpose::Import => ("import_sheet", "Import Sprite Sheet", "Import"),
            SheetPurpose::Slice { .. } => ("slice_frames", "Slice into Frames", "Slice"),
        };
        let (result, open) = dialog_window_with(ctx, skin, id, title, [accept, "Cancel"], |ui| {
            self.contents(ui)
        });
        finish(ctx, open, result)
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.sheet.width, self.sheet.height);
        egui::Grid::new("sheet_fields")
            .num_columns(3)
            .spacing(vec2(8.0, 4.0))
            .show(ui, |ui| {
                ui.label("Split By");
                widgets::radio_value(ui, &mut self.mode, CutMode::CellSize, "Cell Size");
                widgets::radio_value(ui, &mut self.mode, CutMode::Counts, "Columns × Rows");
                ui.end_row();
                let grid = &mut self.grid;
                match self.mode {
                    CutMode::CellSize => {
                        ui.label("Frame Size");
                        widgets::number(ui, &mut grid.cell.0, 1..=w, " px");
                        widgets::number(ui, &mut grid.cell.1, 1..=h, " px");
                    }
                    CutMode::Counts => {
                        ui.label("Columns / Rows");
                        widgets::number(ui, &mut self.counts.0, 1..=w, "");
                        widgets::number(ui, &mut self.counts.1, 1..=h, "");
                    }
                }
                ui.end_row();
                ui.label("Padding");
                widgets::number(ui, &mut grid.padding.0, 0..=w, " px");
                widgets::number(ui, &mut grid.padding.1, 0..=h, " px");
                ui.end_row();
                ui.label("Offset");
                widgets::number(ui, &mut grid.offset.0, 0..=w, " px");
                widgets::number(ui, &mut grid.offset.1, 0..=h, " px");
                ui.end_row();
                ui.label("Order");
                widgets::radio_value(ui, &mut self.order, SheetOrder::ByRows, "By Rows");
                widgets::radio_value(ui, &mut self.order, SheetOrder::ByColumns, "By Columns");
                ui.end_row();
                ui.label("Duration");
                widgets::number(ui, &mut self.duration_ms, 1..=65535, " ms");
                ui.end_row();
            });
        let grid = self.cut_grid();
        let (cols, rows) = grid.count((w, h));
        ui.label(format!(
            "{cols} × {rows} cells of {}×{} from a {w}×{h} image ({} frames)",
            grid.cell.0,
            grid.cell.1,
            cols * rows
        ));
        self.preview(ui, grid, cols, rows);
    }

    /// La hoja achicada (o agrandada) a `PREVIEW`, con el borde de cada celda.
    fn preview(&mut self, ui: &mut egui::Ui, g: SheetGrid, cols: u32, rows: u32) {
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
