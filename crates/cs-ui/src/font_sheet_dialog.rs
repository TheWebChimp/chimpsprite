//! FontForge → Generate ▾ → Glyphs from image…: una hoja con las letras en cuadrícula (PNG) se
//! parte como en Import Sprite Sheet y cada celda va al glifo de su carácter (en el orden del
//! texto). Se eligen la celda, el aire, el margen, la fila de la baseline, la tinta y el fondo.

use cs_core::{ArtboardId, FontMode, GlyphBuild};
use cs_io::export::Bitmap;
use cs_io::font::sheet::{GlyphSheet, glyphs_from_sheet, sheet_chars, sheet_colors};
use cs_io::import::SheetGrid;
use eframe::egui::{self, Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, vec2};

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El lado más largo de la vista de la hoja.
const PREVIEW: f32 = 260.0;
/// El orden de siempre: los ASCII imprimibles.
const ASCII: &str = "!\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";

pub struct GlyphSheetDialog {
    /// La fuente donde van los glifos.
    pub artboard: ArtboardId,
    sheet: Bitmap,
    pub options: GlyphSheet,
    /// Si se pisan los glifos ya dibujados.
    pub replace: bool,
    texture: Option<TextureHandle>,
}

impl GlyphSheetDialog {
    /// Para la fuente activa: celda = la de la fuente, baseline = la suya, fondo = la esquina,
    /// tinta = el color más común, orden ASCII. `None` si el activo no es una fuente.
    pub fn new(editor: &Editor, sheet: Bitmap) -> Option<Self> {
        let font = editor.font()?;
        let sprite = editor.sprite();
        let cell = (
            sprite.width.min(sheet.width.max(1)),
            sprite.height.min(sheet.height.max(1)),
        );
        let (paper, ink) = sheet_colors(&sheet);
        Some(GlyphSheetDialog {
            artboard: editor.active,
            options: GlyphSheet {
                grid: SheetGrid {
                    cell,
                    padding: (0, 0),
                    offset: (0, 0),
                },
                chars: ASCII.to_owned(),
                ink,
                paper,
                color: font.mode == FontMode::Color,
                baseline: font.metrics.baseline.min(cell.1),
                target_baseline: font.metrics.baseline,
            },
            sheet,
            replace: false,
            texture: None,
        })
    }

    /// Los glifos con las opciones de ahora.
    pub fn glyphs(&self) -> Vec<GlyphBuild> {
        glyphs_from_sheet(&self.sheet, &self.options)
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let buttons = ["Import", "Cancel"];
        let (result, open) = dialog_window_with(
            ctx,
            skin,
            "font_glyph_sheet",
            "Glyphs from Image",
            buttons,
            |ui| self.contents(ui),
        );
        finish(ctx, open, result)
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let (w, h) = (self.sheet.width, self.sheet.height);
        let o = &mut self.options;
        egui::Grid::new("font_sheet_fields")
            .num_columns(3)
            .spacing(vec2(8.0, 4.0))
            .show(ui, |ui| {
                ui.label("Cell");
                widgets::number(ui, &mut o.grid.cell.0, 1..=w.max(1), " px");
                widgets::number(ui, &mut o.grid.cell.1, 1..=h.max(1), " px");
                ui.end_row();
                ui.label("Padding");
                widgets::number(ui, &mut o.grid.padding.0, 0..=w, " px");
                widgets::number(ui, &mut o.grid.padding.1, 0..=h, " px");
                ui.end_row();
                ui.label("Offset");
                widgets::number(ui, &mut o.grid.offset.0, 0..=w, " px");
                widgets::number(ui, &mut o.grid.offset.1, 0..=h, " px");
                ui.end_row();
                ui.label("Baseline");
                widgets::number(ui, &mut o.baseline, 0..=o.grid.cell.1, " row");
                ui.end_row();
                ui.label("Ink");
                ui.add_enabled_ui(!o.color, |ui| {
                    widgets::color_edit(ui, &mut o.ink, false, "Ink color")
                })
                .response
                .on_disabled_hover_text("Color fonts keep the image's colors");
                ui.end_row();
                ui.label("Background");
                widgets::color_edit(ui, &mut o.paper, true, "Background color");
                ui.end_row();
            });
        ui.label("Characters, in order (a space skips a cell):");
        widgets::text_line_named(ui, &mut self.options.chars, "Characters");
        widgets::checkbox(ui, &mut self.replace, "Replace drawn glyphs");
        let (cols, rows) = self.options.grid.count((w, h));
        let named = sheet_chars(&self.options.chars).len();
        let found = self.glyphs().len();
        ui.weak(format!(
            "{cols} × {rows} cells, {named} characters: {found} glyphs with ink"
        ));
        self.preview(ui, cols, rows);
    }

    /// La hoja a `PREVIEW` con el borde de cada celda que tiene carácter.
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
                .load_texture("font_glyph_sheet", image, egui::TextureOptions::NEAREST)
        });
        let k = PREVIEW / sheet.width.max(sheet.height).max(1) as f32;
        let size = vec2(sheet.width as f32, sheet.height as f32) * k;
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let painter = ui.painter_at(rect);
        crate::panels::checker(&painter, rect);
        let full = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        let g = self.options.grid;
        let named = sheet_chars(&self.options.chars);
        let stroke = Stroke::new(1.0, crate::theme::ACCENT);
        let baseline = Stroke::new(1.0, crate::font_overlay::GUIDE_DIM);
        let cells = (0..rows).flat_map(|row| (0..cols).map(move |col| (col, row)));
        for ((col, row), ch) in cells.zip(named) {
            if ch.is_none() {
                continue;
            }
            let x = g.offset.0 + col * (g.cell.0 + g.padding.0);
            let y = g.offset.1 + row * (g.cell.1 + g.padding.1);
            let cell = Rect::from_min_size(
                rect.min + vec2(x as f32, y as f32) * k,
                vec2(g.cell.0 as f32, g.cell.1 as f32) * k,
            );
            painter.rect_stroke(cell, 0.0, stroke, egui::StrokeKind::Inside);
            let line = cell.top() + self.options.baseline as f32 * k;
            painter.hline(cell.x_range(), line, baseline);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{ColorMode, Rgba, Sprite};

    #[test]
    fn the_dialog_starts_from_the_font_and_reads_the_sheet() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        assert!(e.new_font("Mine", (4, 4), FontMode::OneBit));
        let mut sheet = Bitmap::new(8, 4);
        sheet.pixels.fill(Rgba::new(255, 255, 255, 255));
        sheet.pixels[1] = Rgba::new(0, 0, 0, 255);
        sheet.pixels[8 + 5] = Rgba::new(0, 0, 0, 255);
        let dialog = GlyphSheetDialog::new(&e, sheet).unwrap();
        assert_eq!(dialog.options.grid.cell, (4, 4));
        assert_eq!(dialog.options.paper, Rgba::new(255, 255, 255, 255));
        assert_eq!(dialog.options.ink, Rgba::new(0, 0, 0, 255));
        assert_eq!(dialog.options.baseline, 3);
        let glyphs = dialog.glyphs();
        let chars: Vec<char> = glyphs.iter().map(|g| g.ch).collect();
        assert_eq!(chars, vec!['!', '"']);
        assert_eq!(e.fill_glyphs(&glyphs, dialog.replace), 2);
    }
}
