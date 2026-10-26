//! Diálogo "Canvas Size": tamaño nuevo, ancla 3×3 y bordes, con vista previa en el lienzo.

use cs_commands::{Command, DocCommand};
use cs_core::{Artboard, ArtboardId, Document};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};
use eframe::emath::GuiRounding as _;

use crate::skin::{Skin, WidgetState};
use crate::{pixel_ui, theme, widgets};

const MAX_SIZE: i64 = 16384;
/// Arriba a la derecha, para no tapar la vista previa del lienzo (con y sin Pixel UI).
const ANCHOR: (egui::Align2, Vec2) = (egui::Align2::RIGHT_TOP, Vec2::new(-240.0, 40.0));
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

/// Dibuja la ventana de la app (con `skin`, la de Pixel UI y sus colores). Enter acepta,
/// Escape y la × cancelan. OK no hace nada con un tamaño inválido.
pub fn show(
    ctx: &egui::Context,
    dialog: &mut CanvasSizeDialog,
    skin: Option<&Skin>,
) -> DialogResult {
    let accent = skin.map_or(theme::ACCENT, |s| s.colors().accent);
    let mut result = DialogResult::Open;
    let (_, closed) = pixel_ui::app_window(
        ctx,
        skin,
        "canvas_size",
        "Canvas Size",
        true,
        ANCHOR,
        |ui| {
            let clicked = match skin.filter(|s| s.piece(GRID_PIECE, WidgetState::Normal).is_some())
            {
                Some(skin) => pixel_contents(ui, dialog, accent, skin),
                None => {
                    contents(ui, dialog, accent, skin);
                    ui.add_space(6.0);
                    pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"])
                }
            };
            match clicked {
                Some(0) if dialog.is_valid() => result = DialogResult::Accept,
                Some(1) => result = DialogResult::Cancel,
                _ => {}
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
    } else if enter && dialog.is_valid() {
        DialogResult::Accept
    } else {
        result
    }
}

/// El diálogo con Pixel UI, como en el diseño (cada cosa en su lugar, desde la esquina del
/// contenido): Width y Height a la izquierda, centrados con el ancla; la línea grabada de lado a
/// lado; "Border" con sus cuatro campos en dos columnas; Trim; OK hasta el ancla y Cancel con
/// el resto. Devuelve el botón que se apretó.
fn pixel_contents(
    ui: &mut egui::Ui,
    dialog: &mut CanvasSizeDialog,
    accent: Color32,
    skin: &Skin,
) -> Option<usize> {
    let (area, _) = ui.allocate_exact_size(PIXEL_SIZE, Sense::hover());
    // La ventana deja 5 filas arriba que el diseño no tiene: se dibuja desde ahí.
    let o = area.min.round() - DESIGN_SHIFT;
    let at = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(o + Vec2::new(x, y), Vec2::new(w, h))
    };
    // Un renglón: el texto a la izquierda y el campo en su columna, centrados a lo alto.
    let row = |ui: &mut egui::Ui, rect: egui::Rect, add: &mut dyn FnMut(&mut egui::Ui)| {
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        add(&mut child);
    };
    let field_row = |ui: &mut egui::Ui, x: f32, y: f32, label: &str, field_x: f32| {
        // El texto, 1 px adentro y 1 fila abajo de lo que lo centraría egui (el diseño).
        let label_rect = at(x + LABEL_DX, y + LABEL_DY, field_x - x - LABEL_DX, FIELD_H);
        row(ui, label_rect, &mut |ui| {
            ui.label(label);
        });
        at(field_x, y, 60.0, FIELD_H)
    };
    // Tamaño.
    let rect = field_row(ui, 0.0, SIZE_TOP, "Width:", FIELD_X);
    row(ui, rect, &mut |ui| {
        let mut w = dialog.width();
        if widgets::number(ui, &mut w, 1..=MAX_SIZE, " px").changed() {
            dialog.set_width(w);
        }
    });
    let rect = field_row(ui, 0.0, SIZE_TOP + ROW_STEP, "Height:", FIELD_X);
    row(ui, rect, &mut |ui| {
        let mut h = dialog.height();
        if widgets::number(ui, &mut h, 1..=MAX_SIZE, " px").changed() {
            dialog.set_height(h);
        }
    });
    // El ancla.
    let mut grid_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(at(GRID_X, GRID_TOP, 88.0, 88.0)));
    pixel_anchor_grid(&mut grid_ui, dialog, skin);
    // La línea grabada, de borde a borde de la ventana.
    let ppp = ui.ctx().pixels_per_point();
    let line = egui::Rangef::new(o.x - WINDOW_SIDE, o.x + CONTENT_W + WINDOW_SIDE);
    ui.painter().add(pixel_ui::etched_hline(
        line,
        o.y + LINE_TOP,
        skin.colors(),
        ppp,
    ));
    // Los bordes.
    row(ui, at(0.0, BORDER_TOP, CONTENT_W, FIELD_H), &mut |ui| {
        ui.label("Border");
        if !dialog.is_valid() {
            ui.colored_label(accent, "Size must be between 1 and 16384 px");
        }
    });
    let fields: [(&str, f32, f32, f32); 4] = [
        ("Left:", 0.0, FIELDS_TOP, FIELD_X),
        ("Top:", COLUMN_2, FIELDS_TOP, FIELD_2_X),
        ("Right:", 0.0, FIELDS_TOP + ROW_STEP, FIELD_X),
        ("Bottom:", COLUMN_2, FIELDS_TOP + ROW_STEP, FIELD_2_X),
    ];
    for (i, (label, x, y, field_x)) in fields.into_iter().enumerate() {
        let rect = field_row(ui, x, y, label, field_x);
        let value = match i {
            0 => &mut dialog.left,
            1 => &mut dialog.top,
            2 => &mut dialog.right,
            _ => &mut dialog.bottom,
        };
        row(ui, rect, &mut |ui| {
            widgets::number(ui, value, BORDER_RANGE, " px");
        });
    }
    row(ui, at(1.0, TRIM_TOP, CONTENT_W, 14.0), &mut |ui| {
        crate::widgets::checkbox(ui, &mut dialog.trim, "Trim content outside the canvas");
    });
    // OK hasta el ancla y Cancel con lo que queda.
    let mut clicked = None;
    let buttons = [
        ("OK", at(0.0, BUTTONS_TOP, GRID_X, BUTTON_H)),
        (
            "Cancel",
            at(
                GRID_X + 4.0,
                BUTTONS_TOP,
                CONTENT_W - GRID_X - 4.0,
                BUTTON_H,
            ),
        ),
    ];
    for (i, (label, rect)) in buttons.into_iter().enumerate() {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        if pixel_ui::pixel_button_sized(&mut child, skin, label, rect.size()).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// Las medidas del diálogo con Pixel UI (desde la esquina del contenido, como en el diseño):
/// el contenido, de los textos al borde derecho del ancla, y lo que le queda a la ventana a los
/// lados (para la línea).
const PIXEL_SIZE: Vec2 = Vec2::new(179.0, 208.0);
const DESIGN_SHIFT: Vec2 = Vec2::new(0.0, 5.0);
/// El ancho del contenido en el diseño (de los textos al borde derecho del ancla).
const CONTENT_W: f32 = 179.0;
const WINDOW_SIDE: f32 = 6.0;
const GRID_X: f32 = 91.0;
const GRID_TOP: f32 = 5.0;
const FIELD_X: f32 = 39.0;
const FIELD_H: f32 = 17.0;
const ROW_STEP: f32 = 24.0;
const SIZE_TOP: f32 = 30.0;
const LINE_TOP: f32 = 100.0;
const BORDER_TOP: f32 = 102.0;
const FIELDS_TOP: f32 = 120.0;
const COLUMN_2: f32 = 98.0;
const LABEL_DX: f32 = 1.0;
const LABEL_DY: f32 = 1.0;
const FIELD_2_X: f32 = 137.0;
const TRIM_TOP: f32 = 171.0;
const BUTTONS_TOP: f32 = 196.0;
const BUTTON_H: f32 = 16.0;

/// Lo de adentro del diálogo, sin los botones: tamaño, ancla, bordes, recorte y el aviso.
fn contents(
    ui: &mut egui::Ui,
    dialog: &mut CanvasSizeDialog,
    accent: Color32,
    skin: Option<&Skin>,
) {
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
        match skin.filter(|s| s.piece(GRID_PIECE, WidgetState::Normal).is_some()) {
            Some(skin) => pixel_anchor_grid(ui, dialog, skin),
            None => anchor_grid(ui, dialog, accent),
        }
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
    crate::widgets::checkbox(ui, &mut dialog.trim, "Trim content outside the canvas");
    if !dialog.is_valid() {
        ui.colored_label(accent, "Size must be between 1 and 16384 px");
    }
}

/// La grilla del ancla del diseño (frame 4): la caja de 88 con sus divisiones punteadas, el mono
/// en la celda del ancla y, en las demás, la flecha hacia donde crece el lienzo
/// (`anchor.n`, `anchor.ne`, …).
const GRID_PIECE: &str = "anchor.grid";
/// Las celdas de la caja: de 28, a 1 del borde y separadas por la división (1).
const GRID_CELL: f32 = 28.0;
const GRID_FIRST: f32 = 1.0;
const GRID_STEP: f32 = 29.0;

/// La pieza de la celda que está `(dx, dy)` (en celdas) del ancla.
fn anchor_piece(dx: i8, dy: i8) -> &'static str {
    match (dx.signum(), dy.signum()) {
        (0, 0) => "anchor.center",
        (0, -1) => "anchor.n",
        (1, -1) => "anchor.ne",
        (1, 0) => "anchor.e",
        (1, 1) => "anchor.se",
        (0, 1) => "anchor.s",
        (-1, 1) => "anchor.sw",
        (-1, 0) => "anchor.w",
        _ => "anchor.nw",
    }
}

/// La grilla 3×3 del ancla con Pixel UI: las piezas del skin, centradas en cada celda.
fn pixel_anchor_grid(ui: &mut egui::Ui, dialog: &mut CanvasSizeDialog, skin: &Skin) {
    let Some(grid) = skin.piece(GRID_PIECE, WidgetState::Normal) else {
        return;
    };
    let size = Vec2::new(grid.size[0] as f32, grid.size[1] as f32);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let ppp = ui.ctx().pixels_per_point();
    let origin = rect.min.round_to_pixels(ppp);
    let mut shapes = pixel_ui::piece_shapes(grid, egui::Rect::from_min_size(origin, size), ppp);
    let (ac, ar) = dialog.anchor();
    for row in 0..3u8 {
        for col in 0..3u8 {
            let min =
                origin + Vec2::new(col as f32, row as f32) * GRID_STEP + Vec2::splat(GRID_FIRST);
            let cell = egui::Rect::from_min_size(min, Vec2::splat(GRID_CELL));
            let id = ui.id().with(("anchor", col, row));
            let response = ui
                .interact(cell, id, Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                dialog.set_anchor(col, row);
            }
            let name = anchor_piece(col as i8 - ac as i8, row as i8 - ar as i8);
            if let Some(piece) = skin.piece(name, WidgetState::Normal) {
                let piece_size = Vec2::new(piece.size[0] as f32, piece.size[1] as f32);
                let at = (cell.min + (Vec2::splat(GRID_CELL) - piece_size) / 2.0).floor();
                shapes.extend(pixel_ui::piece_shapes(
                    piece,
                    egui::Rect::from_min_size(at, piece_size),
                    ppp,
                ));
            }
        }
    }
    ui.painter().extend(shapes);
}

/// Grilla 3×3 del ancla sin Pixel UI. Cada celda muestra una flecha hacia donde crece el lienzo.
fn anchor_grid(ui: &mut egui::Ui, dialog: &mut CanvasSizeDialog, accent: Color32) {
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
            let dir = Vec2::new(col as f32 - ac as f32, row as f32 - ar as f32);
            if dir == Vec2::ZERO {
                painter.rect_filled(
                    egui::Rect::from_center_size(cell.center(), Vec2::splat(8.0)),
                    1.0,
                    accent,
                );
            } else {
                arrow(
                    painter,
                    cell.center(),
                    dir.normalized() * 7.0,
                    visuals.fg_stroke.color,
                );
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
    fn each_cell_points_away_from_the_anchor() {
        assert_eq!(anchor_piece(0, 0), "anchor.center");
        assert_eq!(anchor_piece(2, 0), "anchor.e");
        assert_eq!(anchor_piece(-1, -2), "anchor.nw");
        assert_eq!(anchor_piece(0, 2), "anchor.s");
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
