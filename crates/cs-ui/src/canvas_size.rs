//! Diálogo "Canvas Size": tamaño nuevo, ancla 3×3 y bordes, con vista previa en el lienzo.

use cs_commands::{Command, DocCommand};
use cs_core::{Artboard, ArtboardId, Document};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};

use crate::{theme, widgets};

const MAX_SIZE: i64 = 16384;
const BORDER_RANGE: std::ops::RangeInclusive<i32> = -(MAX_SIZE as i32)..=MAX_SIZE as i32;

/// Estado del diálogo. Los bordes pueden ser negativos (recortan).
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSizeDialog {
    pub artboard: ArtboardId,
    original: (u32, u32),
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    /// Columna y fila del ancla (0..=2).
    anchor: (u8, u8),
    pub trim: bool,
}

/// Reparte `delta` entre (antes, después) según la posición del ancla en ese eje.
fn split(delta: i32, anchor: u8) -> (i32, i32) {
    match anchor {
        0 => (0, delta),
        1 => (delta.div_euclid(2), delta - delta.div_euclid(2)),
        _ => (delta, 0),
    }
}

impl CanvasSizeDialog {
    pub fn new(artboard: &Artboard) -> Self {
        Self {
            artboard: artboard.id,
            original: (artboard.sprite.width, artboard.sprite.height),
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
            anchor: (1, 1),
            trim: false,
        }
    }

    pub fn width(&self) -> i64 {
        self.original.0 as i64 + self.left as i64 + self.right as i64
    }

    pub fn height(&self) -> i64 {
        self.original.1 as i64 + self.top as i64 + self.bottom as i64
    }

    pub fn anchor(&self) -> (u8, u8) {
        self.anchor
    }

    pub fn set_width(&mut self, width: i64) {
        let delta = (width.clamp(1, MAX_SIZE) - self.original.0 as i64) as i32;
        (self.left, self.right) = split(delta, self.anchor.0);
    }

    pub fn set_height(&mut self, height: i64) {
        let delta = (height.clamp(1, MAX_SIZE) - self.original.1 as i64) as i32;
        (self.top, self.bottom) = split(delta, self.anchor.1);
    }

    /// Cambia el ancla conservando el tamaño elegido.
    pub fn set_anchor(&mut self, col: u8, row: u8) {
        let (w, h) = (self.width(), self.height());
        self.anchor = (col.min(2), row.min(2));
        self.set_width(w);
        self.set_height(h);
    }

    pub fn is_valid(&self) -> bool {
        (1..=MAX_SIZE).contains(&self.width()) && (1..=MAX_SIZE).contains(&self.height())
    }

    /// Rectángulo nuevo en coordenadas de mundo (mínimo inclusivo, máximo exclusivo).
    pub fn preview_rect(&self, doc: &Document) -> Option<((i32, i32), (i32, i32))> {
        let (x, y) = doc.artboard(self.artboard)?.position;
        let (x0, y0) = (x - self.left, y - self.top);
        Some((
            (x0, y0),
            (x0 + self.width() as i32, y0 + self.height() as i32),
        ))
    }

    /// Mueve un borde a la coordenada de mundo `coord` (al arrastrar su guía).
    /// El tamaño nunca baja de 1 px.
    pub fn set_edge(&mut self, edge: GuideEdge, coord: i32, doc: &Document) {
        let Some(artboard) = doc.artboard(self.artboard) else {
            return;
        };
        let (x, y) = artboard.position;
        let (w, h) = (self.original.0 as i32, self.original.1 as i32);
        match edge {
            GuideEdge::Left => self.left = (x - coord).max(1 - w - self.right),
            GuideEdge::Right => self.right = (coord - x - w).max(1 - w - self.left),
            GuideEdge::Top => self.top = (y - coord).max(1 - h - self.bottom),
            GuideEdge::Bottom => self.bottom = (coord - y - h).max(1 - h - self.top),
        }
    }

    /// Comando a ejecutar al aceptar; `None` si no hay cambios o el tamaño no es válido.
    pub fn command(&self, doc: &Document) -> Option<DocCommand> {
        let unchanged = (self.left, self.top, self.right, self.bottom) == (0, 0, 0, 0);
        if !self.is_valid() || (unchanged && !self.trim) {
            return None;
        }
        let ((x0, y0), _) = self.preview_rect(doc)?;
        let mut cmds = vec![DocCommand::OnArtboard {
            id: self.artboard,
            cmd: Command::ResizeCanvas {
                width: self.width() as u32,
                height: self.height() as u32,
                offset: (self.left, self.top),
            },
        }];
        if self.trim {
            cmds.push(DocCommand::OnArtboard {
                id: self.artboard,
                cmd: Command::TrimToCanvas,
            });
        }
        cmds.push(DocCommand::SetArtboardProps {
            id: self.artboard,
            name: None,
            position: Some((x0, y0)),
        });
        Some(DocCommand::Batch(cmds))
    }
}

/// Guía arrastrable de la vista previa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuideEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl GuideEdge {
    pub fn is_vertical(self) -> bool {
        matches!(self, GuideEdge::Left | GuideEdge::Right)
    }
}

/// Lo que el usuario decidió en este frame.
pub enum DialogResult {
    Open,
    Accept,
    Cancel,
}

/// Dibuja la ventana. Enter acepta, Escape cancela.
pub fn show(ctx: &egui::Context, dialog: &mut CanvasSizeDialog) -> DialogResult {
    let mut result = DialogResult::Open;
    let mut open = true;
    egui::Window::new("Canvas Size")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-240.0, 40.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::Grid::new("canvas_size_dims")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Width:");
                        let mut w = dialog.width();
                        if widgets::number(ui, &mut w, 1..=MAX_SIZE, " px").changed() {
                            dialog.set_width(w);
                        }
                        ui.end_row();
                        ui.label("Height:");
                        let mut h = dialog.height();
                        if widgets::number(ui, &mut h, 1..=MAX_SIZE, " px").changed() {
                            dialog.set_height(h);
                        }
                        ui.end_row();
                    });
                ui.add_space(12.0);
                anchor_grid(ui, dialog);
            });

            ui.add_space(6.0);
            ui.label(egui::RichText::new("Borders").strong());
            egui::Grid::new("canvas_size_borders")
                .num_columns(4)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Left:");
                    widgets::number(ui, &mut dialog.left, BORDER_RANGE, " px");
                    ui.label("Top:");
                    widgets::number(ui, &mut dialog.top, BORDER_RANGE, " px");
                    ui.end_row();
                    ui.label("Right:");
                    widgets::number(ui, &mut dialog.right, BORDER_RANGE, " px");
                    ui.label("Bottom:");
                    widgets::number(ui, &mut dialog.bottom, BORDER_RANGE, " px");
                    ui.end_row();
                });

            ui.add_space(6.0);
            ui.checkbox(&mut dialog.trim, "Trim content outside the canvas");
            if !dialog.is_valid() {
                ui.colored_label(theme::ACCENT, "Size must be between 1 and 16384 px");
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(dialog.is_valid(), egui::Button::new("OK"))
                    .clicked()
                {
                    result = DialogResult::Accept;
                }
                if ui.button("Cancel").clicked() {
                    result = DialogResult::Cancel;
                }
            });
        });
    let (enter, escape) = ctx.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if !open || escape {
        DialogResult::Cancel
    } else if enter && dialog.is_valid() {
        DialogResult::Accept
    } else {
        result
    }
}

/// Grilla 3×3 del ancla. Cada celda muestra una flecha hacia donde crece el lienzo.
fn anchor_grid(ui: &mut egui::Ui, dialog: &mut CanvasSizeDialog) {
    const CELL: f32 = 24.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(CELL * 3.0), Sense::hover());
    let (ac, ar) = dialog.anchor();
    for row in 0..3u8 {
        for col in 0..3u8 {
            let min = rect.min + Vec2::new(col as f32, row as f32) * CELL;
            let cell = egui::Rect::from_min_size(min, Vec2::splat(CELL)).shrink(1.0);
            let id = ui.id().with(("anchor", col, row));
            let response = ui.interact(cell, id, Sense::click());
            if response.clicked() {
                dialog.set_anchor(col, row);
            }
            let visuals = ui.style().interact(&response);
            let painter = ui.painter();
            painter.rect_filled(cell, 2.0, visuals.weak_bg_fill);
            let color = visuals.fg_stroke.color;
            let dir = Vec2::new(col as f32 - ac as f32, row as f32 - ar as f32);
            if dir == Vec2::ZERO {
                painter.rect_filled(
                    egui::Rect::from_center_size(cell.center(), Vec2::splat(8.0)),
                    1.0,
                    theme::ACCENT,
                );
            } else {
                arrow(painter, cell.center(), dir.normalized() * 7.0, color);
            }
        }
    }
}

fn arrow(painter: &egui::Painter, center: Pos2, vec: Vec2, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    let (tail, tip) = (center - vec, center + vec);
    painter.line_segment([tail, tip], stroke);
    let back = -vec.normalized() * 4.0;
    let side = Vec2::new(-back.y, back.x);
    painter.line_segment([tip, tip + back + side], stroke);
    painter.line_segment([tip, tip + back - side], stroke);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};

    fn doc() -> Document {
        let mut d = Document::with_sprite("a", Sprite::new(10, 6, ColorMode::Rgba));
        d.artboards[0].position = (100, 50);
        d
    }

    fn dialog(d: &Document) -> CanvasSizeDialog {
        CanvasSizeDialog::new(&d.artboards[0])
    }

    #[test]
    fn width_is_split_by_anchor() {
        let d = doc();
        let mut c = dialog(&d);
        c.set_width(15);
        assert_eq!((c.left, c.right), (2, 3));
        c.set_anchor(0, 1);
        assert_eq!((c.left, c.right, c.width()), (0, 5, 15));
        c.set_anchor(2, 1);
        assert_eq!((c.left, c.right), (5, 0));
        c.set_height(2);
        assert_eq!((c.top, c.bottom), (-2, -2));
    }

    #[test]
    fn dragging_guides_sets_borders_and_keeps_min_size() {
        let d = doc(); // 10×6 en (100, 50)
        let mut c = dialog(&d);
        c.set_edge(GuideEdge::Left, 96, &d);
        c.set_edge(GuideEdge::Bottom, 60, &d);
        assert_eq!((c.left, c.bottom), (4, 4));
        assert_eq!(c.preview_rect(&d), Some(((96, 50), (110, 60))));
        // Arrastrar el borde derecho más allá del izquierdo deja 1 px.
        c.set_edge(GuideEdge::Right, 0, &d);
        assert_eq!(c.width(), 1);
        c.set_edge(GuideEdge::Top, 500, &d);
        assert_eq!(c.height(), 1);
    }

    #[test]
    fn borders_define_size() {
        let d = doc();
        let mut c = dialog(&d);
        c.left = 3;
        c.bottom = -1;
        assert_eq!((c.width(), c.height()), (13, 5));
        assert_eq!(c.preview_rect(&d), Some(((97, 50), (110, 55))));
    }

    #[test]
    fn no_change_means_no_command_and_invalid_is_rejected() {
        let d = doc();
        let mut c = dialog(&d);
        assert_eq!(c.command(&d), None);
        c.right = -10;
        assert!(!c.is_valid());
        assert_eq!(c.command(&d), None);
    }

    #[test]
    fn command_keeps_content_in_world_and_can_trim() {
        let mut d = doc();
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        DocCommand::OnArtboard {
            id: ArtboardId(1),
            cmd: Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(0, 0, red), (9, 5, red)],
            },
        }
        .apply(&mut d)
        .unwrap();
        let before = d.clone();
        let mut c = dialog(&d);
        c.left = 2;
        c.right = -5;
        c.trim = true;
        let inv = c.command(&d).unwrap().apply(&mut d).unwrap();
        let ab = &d.artboards[0];
        assert_eq!(ab.position, (98, 50));
        assert_eq!((ab.sprite.width, ab.sprite.height), (7, 6));
        // El píxel del mundo (100, 50) sigue en su lugar: local (2, 0).
        assert_eq!(ab.sprite.pixel(LayerId(1), 0, 2, 0), red);
        let cel = ab.sprite.cel(LayerId(1), 0).unwrap();
        let img = ab.sprite.image(cel.image).unwrap();
        assert_eq!((img.width(), img.height()), (5, 6), "recortado al lienzo");
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
    }
}
