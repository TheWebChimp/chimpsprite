//! Expand to Spritesheet… del menú de un artboard: sus frames (todos, los de un tag o un rango)
//! acomodados en una hoja, en un artboard nuevo de un frame. Columnas y filas automáticas (casi
//! cuadrada) o fijas, el orden de las celdas, el aire entre ellas y el margen; abajo, la hoja
//! achicada con el borde de cada celda.

use cs_core::Sprite;
use cs_io::spritesheet::{ExpandSettings, SheetOrder, auto_columns, expand_frames, fit};
use eframe::egui::{self, Color32, ColorImage, Rect, Sense, Stroke, TextureHandle, vec2};

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El lado más largo de la vista previa.
const PREVIEW: f32 = 220.0;
/// El lado más largo que puede tener la hoja (el de Canvas Size).
const MAX_SIDE: u32 = 16384;

/// Qué frames van a la hoja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpandFrames {
    All,
    /// Los del tag de ese índice.
    Tag(usize),
    /// De `ExpandDialog::range`.
    Range,
}

/// Cómo se decide la forma de la hoja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetShape {
    /// Casi cuadrada: `ceil(sqrt(n))` columnas.
    Auto,
    /// `count` columnas y las filas que hagan falta.
    Columns,
    /// `count` filas y las columnas que hagan falta.
    Rows,
}

pub struct ExpandDialog {
    /// El nombre del artboard (el nuevo se llama "<nombre> Sheet").
    pub name: String,
    /// El sprite del artboard con las instancias y los tilemaps ya resueltos (todos sus frames).
    sprite: Sprite,
    pub frames: ExpandFrames,
    /// Primer y último frame del rango (desde 0, inclusivo).
    pub range: (usize, usize),
    pub shape: SheetShape,
    /// Columnas o filas con `SheetShape::Columns` / `Rows`.
    pub count: u32,
    pub spacing: u32,
    pub border: u32,
    pub order: SheetOrder,
    /// La vista previa y para qué frames y opciones se armó.
    preview: Option<(Vec<usize>, ExpandSettings, TextureHandle)>,
}

impl ExpandDialog {
    /// Con todos los frames de `sprite` (ya materializado), en una hoja casi cuadrada.
    pub fn new(name: String, sprite: Sprite) -> Self {
        let n = sprite.frames.len();
        Self {
            name,
            frames: ExpandFrames::All,
            range: (0, n.saturating_sub(1)),
            shape: SheetShape::Auto,
            count: auto_columns(n),
            spacing: 0,
            border: 0,
            order: SheetOrder::ByRows,
            sprite,
            preview: None,
        }
    }

    /// Los frames elegidos, en orden.
    pub fn frame_list(&self) -> Vec<usize> {
        let last = self.sprite.frames.len().saturating_sub(1);
        let (from, to) = match self.frames {
            ExpandFrames::All => (0, last),
            ExpandFrames::Tag(i) => self
                .sprite
                .tags
                .get(i)
                .map_or((0, last), |t| (t.from, t.to)),
            ExpandFrames::Range => self.range,
        };
        let (from, to) = (from.min(last), to.min(last));
        (from.min(to)..=to.max(from)).collect()
    }

    /// Las opciones de la hoja para los frames elegidos.
    pub fn settings(&self) -> ExpandSettings {
        let n = self.frame_list().len();
        let (columns, rows) = match self.shape {
            SheetShape::Auto => {
                let c = auto_columns(n);
                (c, fit(n, c))
            }
            SheetShape::Columns => {
                let c = self.count.clamp(1, n.max(1) as u32);
                (c, fit(n, c))
            }
            SheetShape::Rows => {
                let r = self.count.clamp(1, n.max(1) as u32);
                (fit(n, r), r)
            }
        };
        ExpandSettings {
            columns,
            rows,
            spacing: self.spacing,
            border: self.border,
            order: self.order,
        }
    }

    /// Lo que mide la hoja.
    pub fn sheet_size(&self) -> (u32, u32) {
        self.settings()
            .sheet_size((self.sprite.width, self.sprite.height))
    }

    /// La hoja con lo elegido, o por qué no se puede.
    pub fn sheet(&self) -> Result<Sprite, String> {
        let (w, h) = self.sheet_size();
        if w > MAX_SIDE || h > MAX_SIDE {
            return Err(format!(
                "A {w}×{h} sheet is too big (at most {MAX_SIDE} on each side)."
            ));
        }
        expand_frames(&self.sprite, &self.frame_list(), self.settings())
            .ok_or_else(|| "There are no frames to expand.".to_owned())
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) = dialog_window_with(
            ctx,
            skin,
            "expand_sheet",
            "Expand to Spritesheet",
            ["Expand", "Cancel"],
            |ui| self.contents(ui),
        );
        finish(ctx, open, result)
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let n = self.sprite.frames.len();
        let tags: Vec<String> = self.sprite.tags.iter().map(|t| t.name.clone()).collect();
        let selected = match self.frames {
            ExpandFrames::All => "All Frames".to_owned(),
            ExpandFrames::Tag(i) => tags.get(i).cloned().unwrap_or_default(),
            ExpandFrames::Range => "Frame Range".to_owned(),
        };
        egui::Grid::new("expand_fields")
            .num_columns(2)
            .spacing(vec2(8.0, 4.0))
            .show(ui, |ui| {
                ui.label("Frames");
                widgets::combo(ui, "expand_frames", &selected, |ui| {
                    widgets::choice(ui, &mut self.frames, ExpandFrames::All, "All Frames");
                    for (i, name) in tags.iter().enumerate() {
                        widgets::choice(ui, &mut self.frames, ExpandFrames::Tag(i), name);
                    }
                    widgets::choice(ui, &mut self.frames, ExpandFrames::Range, "Frame Range");
                });
                ui.end_row();
                if self.frames == ExpandFrames::Range {
                    // En pantalla, desde 1 (como el timeline).
                    let (mut from, mut to) = (self.range.0 + 1, self.range.1 + 1);
                    ui.label("From / To");
                    ui.horizontal(|ui| {
                        widgets::number(ui, &mut from, 1..=n.max(1), "");
                        widgets::number(ui, &mut to, 1..=n.max(1), "");
                    });
                    ui.end_row();
                    // El que se movió arrastra al otro.
                    if from - 1 != self.range.0 {
                        to = to.max(from);
                    } else {
                        from = from.min(to);
                    }
                    self.range = (from - 1, to - 1);
                }
                ui.label("Layout");
                ui.horizontal(|ui| {
                    widgets::radio_value(ui, &mut self.shape, SheetShape::Auto, "Auto");
                    widgets::radio_value(ui, &mut self.shape, SheetShape::Columns, "Columns");
                    widgets::radio_value(ui, &mut self.shape, SheetShape::Rows, "Rows");
                });
                ui.end_row();
                if self.shape != SheetShape::Auto {
                    ui.label(match self.shape {
                        SheetShape::Rows => "Rows",
                        _ => "Columns",
                    });
                    let most = self.frame_list().len().max(1) as u32;
                    widgets::number(ui, &mut self.count, 1..=most, "");
                    ui.end_row();
                }
                ui.label("Order");
                ui.horizontal(|ui| {
                    widgets::radio_value(ui, &mut self.order, SheetOrder::ByRows, "By Rows");
                    widgets::radio_value(ui, &mut self.order, SheetOrder::ByColumns, "By Columns");
                });
                ui.end_row();
                ui.label("Spacing");
                widgets::number(ui, &mut self.spacing, 0..=256, " px");
                ui.end_row();
                ui.label("Border");
                widgets::number(ui, &mut self.border, 0..=256, " px");
                ui.end_row();
            });
        let s = self.settings();
        let (w, h) = self.sheet_size();
        ui.label(format!(
            "{} frames in {} × {} cells, {w}×{h} px",
            self.frame_list().len(),
            s.columns,
            s.rows
        ));
        if w > MAX_SIDE || h > MAX_SIDE {
            ui.label(format!("Too big: at most {MAX_SIDE} px on each side."));
            return;
        }
        self.preview(ui);
    }

    /// La hoja achicada (o agrandada) a `PREVIEW`, con el borde de cada celda. Se rearma solo si
    /// cambian los frames o las opciones.
    fn preview(&mut self, ui: &mut egui::Ui) {
        let frames = self.frame_list();
        let settings = self.settings();
        let stale = self
            .preview
            .as_ref()
            .is_none_or(|(f, s, _)| *f != frames || *s != settings);
        if stale {
            let Some(sheet) = expand_frames(&self.sprite, &frames, settings) else {
                return;
            };
            let pixels = cs_render::render_frame(&sheet, 0);
            let bytes: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
            let image = ColorImage::from_rgba_unmultiplied(
                [sheet.width as usize, sheet.height as usize],
                &bytes,
            );
            let texture =
                ui.ctx()
                    .load_texture("expand_sheet", image, egui::TextureOptions::NEAREST);
            self.preview = Some((frames.clone(), settings, texture));
        }
        let Some((_, _, texture)) = &self.preview else {
            return;
        };
        let cell = (self.sprite.width, self.sprite.height);
        let (w, h) = settings.sheet_size(cell);
        let k = PREVIEW / w.max(h) as f32;
        let (rect, _) = ui.allocate_exact_size(vec2(w as f32, h as f32) * k, Sense::hover());
        let painter = ui.painter_at(rect);
        crate::panels::checker(&painter, rect);
        let full = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
        painter.image(texture.id(), rect, full, Color32::WHITE);
        let stroke = Stroke::new(1.0, crate::theme::ACCENT);
        for i in 0..frames.len() {
            let (x, y) = settings.cell_origin(i as u32, cell);
            let r = Rect::from_min_size(
                rect.min + vec2(x as f32, y as f32) * k,
                vec2(cell.0 as f32, cell.1 as f32) * k,
            );
            painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Inside);
        }
    }
}

#[cfg(test)]
#[path = "expand_dialog_tests.rs"]
mod tests;
