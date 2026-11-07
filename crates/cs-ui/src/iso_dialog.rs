//! Edit → Iso Box…: una caja isométrica 2:1 (como Isobox for Aseprite) o pilas de bloques en una
//! rejilla (un builder sencillo, como IsoBuilder), con los colores de cada cara (tonos del
//! primario o a mano), contorno, highlight y vista previa. Place la deja flotando en la capa
//! actual, como un pegado.

use cs_core::Rgba;
use cs_raster::{IsoBlocks, IsoBox, IsoColors, IsoCorner, IsoParts};
use eframe::egui::{self, Color32, ColorImage, Rect, Sense, TextureHandle, vec2};

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El área de la vista previa.
const PREVIEW: egui::Vec2 = vec2(192.0, 128.0);
/// Lo más largo de una arista de la caja (en unidades) y lo más alto de sus costados.
pub const MAX_UNITS: u32 = 64;
pub const MAX_HEIGHT: u32 = 256;
/// Celdas por lado de la rejilla de bloques y bloques por pila.
pub const MAX_CELLS: u32 = 8;
pub const MAX_STACK: u32 = 16;
/// El lado de cada celda de la rejilla de alturas.
const CELL: f32 = 17.0;

/// Qué se arma.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IsoMode {
    #[default]
    Box,
    Blocks,
}

/// Lo elegido en el diálogo (se recuerda mientras la app está abierta).
#[derive(Clone, Debug, PartialEq)]
pub struct IsoSettings {
    pub mode: IsoMode,
    pub shape: IsoBox,
    pub blocks: IsoBlocks,
    /// Los colores salen de `base` (si no, cada uno a mano).
    pub shade: bool,
    pub base: Rgba,
    pub colors: IsoColors,
    pub outline: bool,
    pub highlight: bool,
    /// Cada color, el más cercano de la paleta del sprite.
    pub snap: bool,
}

impl Default for IsoSettings {
    fn default() -> Self {
        let base = Rgba::rgb(0x5b, 0x6e, 0xe1);
        let mut blocks = IsoBlocks::new(3, 3);
        blocks.heights = vec![2, 1, 1, 1, 0, 1, 1, 1, 3];
        Self {
            mode: IsoMode::Box,
            shape: IsoBox::default(),
            blocks,
            shade: true,
            base,
            colors: IsoColors::from_base(base),
            outline: true,
            highlight: true,
            snap: false,
        }
    }
}

impl IsoSettings {
    /// La esquina (la misma para la caja y los bloques).
    pub fn corner(&self) -> IsoCorner {
        self.shape.corner
    }

    pub fn set_corner(&mut self, corner: IsoCorner) {
        self.shape.corner = corner;
        self.blocks.corner = corner;
    }

    /// Cambia el color base; con Shade from, los cinco colores salen de él.
    pub fn set_base(&mut self, base: Rgba) {
        self.base = base;
        if self.shade {
            self.colors = IsoColors::from_base(base);
        }
    }

    /// Prende o apaga Shade from (prendido, los colores vuelven a salir del base).
    pub fn set_shade(&mut self, shade: bool) {
        self.shade = shade;
        if shade {
            self.colors = IsoColors::from_base(self.base);
        }
    }

    /// Cambia un color a mano: deja de seguir al base.
    pub fn set_color(&mut self, part: cs_raster::IsoPart, color: Rgba) {
        use cs_raster::IsoPart;
        let slot = match part {
            IsoPart::Top => &mut self.colors.top,
            IsoPart::Left => &mut self.colors.left,
            IsoPart::Right => &mut self.colors.right,
            IsoPart::Outline => &mut self.colors.outline,
            IsoPart::Highlight => &mut self.colors.highlight,
        };
        if *slot != color {
            *slot = color;
            self.shade = false;
        }
    }

    /// Suma `delta` a la pila de `(c, r)` (entre 0 y `MAX_STACK`).
    pub fn bump(&mut self, c: u32, r: u32, delta: i32) {
        let h = self.blocks.height_at(c, r) as i32 + delta;
        self.blocks
            .set_height(c, r, h.clamp(0, MAX_STACK as i32) as u32);
    }

    /// Las partes del dibujo.
    pub fn parts(&self) -> IsoParts {
        match self.mode {
            IsoMode::Box => cs_raster::iso_box(&self.shape, self.outline, self.highlight),
            IsoMode::Blocks => cs_raster::iso_blocks(&self.blocks, self.outline, self.highlight),
        }
    }
}

/// El dibujo terminado: su tamaño y el color de cada píxel (`None` = fuera de la forma).
#[derive(Clone, Debug, PartialEq)]
pub struct IsoImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<Option<Rgba>>,
}

pub struct IsoDialog {
    pub settings: IsoSettings,
    /// Los colores de la paleta del sprite (para Snap to palette).
    palette: Vec<Rgba>,
    /// El sprite es indexado: los colores van siempre a la paleta.
    indexed: bool,
    /// La vista previa y con qué se armó.
    preview: Option<(IsoSettings, TextureHandle)>,
}

impl IsoDialog {
    /// Con lo de la última vez (o lo de fábrica); con Shade from, desde `primary`.
    pub fn new(settings: IsoSettings, primary: Rgba, palette: &[Rgba], indexed: bool) -> Self {
        let mut settings = settings;
        if settings.shade {
            settings.set_base(primary);
        }
        Self {
            settings,
            palette: palette.to_vec(),
            indexed,
            preview: None,
        }
    }

    /// Los colores con que se pinta (pegados a la paleta si se pidió o si es indexado).
    pub fn colors(&self) -> IsoColors {
        let c = self.settings.colors;
        if self.settings.snap || self.indexed {
            c.snapped(&self.palette)
        } else {
            c
        }
    }

    /// El dibujo con lo elegido.
    pub fn image(&self) -> IsoImage {
        let parts = self.settings.parts();
        IsoImage {
            width: parts.width,
            height: parts.height,
            pixels: cs_raster::paint_parts(&parts, &self.colors()),
        }
    }

    /// El nombre del paso de undo al confirmar lo colocado.
    pub fn label(&self) -> &'static str {
        match self.settings.mode {
            IsoMode::Box => "Iso Box",
            IsoMode::Blocks => "Iso Blocks",
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) =
            dialog_window_with(ctx, skin, "iso_box", "Iso Box", ["Place", "Cancel"], |ui| {
                self.contents(ui)
            });
        // Sin nada que colocar (todas las pilas vacías), Place no hace nada.
        let result = match result {
            DialogResult::Accept if self.image().width == 0 => DialogResult::Open,
            other => other,
        };
        finish(ctx, open, result)
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let s = &mut self.settings;
        egui::Grid::new("iso_fields")
            .num_columns(2)
            .spacing(vec2(8.0, 4.0))
            .show(ui, |ui| {
                ui.label("Shape");
                ui.horizontal(|ui| {
                    widgets::radio_value(ui, &mut s.mode, IsoMode::Box, "Box");
                    widgets::radio_value(ui, &mut s.mode, IsoMode::Blocks, "Blocks");
                });
                ui.end_row();
                match s.mode {
                    IsoMode::Box => {
                        ui.label("Left");
                        widgets::number(ui, &mut s.shape.left, 1..=MAX_UNITS, " units")
                            .on_hover_text("Back-left edge: units of 2 px across, 1 px down");
                        ui.end_row();
                        ui.label("Right");
                        widgets::number(ui, &mut s.shape.right, 1..=MAX_UNITS, " units")
                            .on_hover_text("Back-right edge: units of 2 px across, 1 px down");
                        ui.end_row();
                        ui.label("Height");
                        widgets::number(ui, &mut s.shape.height, 0..=MAX_HEIGHT, " px");
                        ui.end_row();
                    }
                    IsoMode::Blocks => {
                        let (mut columns, mut rows) = (s.blocks.columns, s.blocks.rows);
                        ui.label("Columns");
                        widgets::number(ui, &mut columns, 1..=MAX_CELLS, "");
                        ui.end_row();
                        ui.label("Rows");
                        widgets::number(ui, &mut rows, 1..=MAX_CELLS, "");
                        ui.end_row();
                        if (columns, rows) != (s.blocks.columns, s.blocks.rows) {
                            s.blocks.resize(columns, rows);
                        }
                        ui.label("Block");
                        widgets::number(ui, &mut s.blocks.size, 1..=16, " units");
                        ui.end_row();
                        ui.label("Block height");
                        widgets::number(ui, &mut s.blocks.block_height, 0..=64, " px");
                        ui.end_row();
                    }
                }
                ui.label("Corner");
                ui.horizontal(|ui| {
                    let mut corner = s.corner();
                    for c in IsoCorner::ALL {
                        widgets::radio_value(ui, &mut corner, c, c.label());
                    }
                    s.set_corner(corner);
                });
                ui.end_row();
            });
        if s.mode == IsoMode::Blocks {
            ui.label("Stack heights (click +1, right-click −1):");
            heights_grid(ui, s);
        }
        ui.add_space(4.0);
        self.color_rows(ui);
        ui.add_space(4.0);
        self.preview(ui);
    }

    /// Shade from, los colores de cada parte, Outline, Highlight y Snap to palette.
    fn color_rows(&mut self, ui: &mut egui::Ui) {
        use cs_raster::IsoPart;
        let indexed = self.indexed;
        let s = &mut self.settings;
        ui.horizontal(|ui| {
            let mut shade = s.shade;
            if widgets::checkbox(ui, &mut shade, "Shade from").changed() {
                s.set_shade(shade);
            }
            let mut base = s.base;
            if widgets::color_edit(ui, &mut base, false, "Base color").changed() {
                // Elegir el base vuelve a sacar los tonos de él.
                s.shade = true;
                s.set_base(base);
            }
        });
        ui.horizontal(|ui| {
            for (part, label) in [
                (IsoPart::Top, "Top"),
                (IsoPart::Left, "Left"),
                (IsoPart::Right, "Right"),
            ] {
                ui.label(label);
                let mut c = s.colors.color(part);
                if widgets::color_edit(ui, &mut c, false, &format!("{label} face color")).changed()
                {
                    s.set_color(part, c);
                }
            }
        });
        ui.horizontal(|ui| {
            for (part, label) in [
                (IsoPart::Outline, "Outline"),
                (IsoPart::Highlight, "Highlight"),
            ] {
                let on = match part {
                    IsoPart::Outline => &mut s.outline,
                    _ => &mut s.highlight,
                };
                widgets::checkbox(ui, on, label);
                let mut c = s.colors.color(part);
                if widgets::color_edit(ui, &mut c, false, &format!("{label} color")).changed() {
                    s.set_color(part, c);
                }
            }
        });
        // En indexado siempre van a la paleta: la casilla queda prendida y quieta.
        if indexed {
            let mut on = true;
            ui.add_enabled_ui(false, |ui| {
                widgets::checkbox(ui, &mut on, "Snap to palette")
            });
        } else {
            widgets::checkbox(ui, &mut s.snap, "Snap to palette");
        }
    }

    /// El dibujo ampliado (en pasos enteros si entra) sobre el damero, y su tamaño.
    fn preview(&mut self, ui: &mut egui::Ui) {
        let image = self.image();
        let stale = self
            .preview
            .as_ref()
            .is_none_or(|(s, _)| *s != self.settings);
        if stale && image.width > 0 {
            let bytes: Vec<u8> = image
                .pixels
                .iter()
                .flat_map(|c| {
                    let c = c.unwrap_or(Rgba::TRANSPARENT);
                    [c.r, c.g, c.b, c.a]
                })
                .collect();
            let color = ColorImage::from_rgba_unmultiplied(
                [image.width as usize, image.height as usize],
                &bytes,
            );
            let texture =
                ui.ctx()
                    .load_texture("iso_box_preview", color, egui::TextureOptions::NEAREST);
            self.preview = Some((self.settings.clone(), texture));
        }
        let (area, _) = ui.allocate_exact_size(PREVIEW, Sense::hover());
        let painter = ui.painter_at(area);
        crate::panels::checker(&painter, area);
        if let (Some((_, texture)), true) = (&self.preview, image.width > 0) {
            let size = vec2(image.width as f32, image.height as f32);
            let fit = (PREVIEW.x / size.x).min(PREVIEW.y / size.y);
            let k = if fit >= 1.0 { fit.floor() } else { fit };
            let rect = Rect::from_center_size(area.center(), size * k);
            let full = Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));
            painter.image(texture.id(), rect, full, Color32::WHITE);
        }
        ui.label(if image.width == 0 {
            "Nothing to place: every stack is empty".to_owned()
        } else {
            format!("{} × {} px", image.width, image.height)
        });
    }
}

/// La rejilla de alturas, vista de arriba como el rombo (la fila 0 arriba): un botón del kit por
/// celda con su altura; clic suma uno, clic derecho resta.
fn heights_grid(ui: &mut egui::Ui, s: &mut IsoSettings) {
    egui::Grid::new("iso_heights")
        .spacing(vec2(2.0, 2.0))
        .show(ui, |ui| {
            for r in 0..s.blocks.rows {
                for c in 0..s.blocks.columns {
                    let h = s.blocks.height_at(c, r);
                    let response = crate::pixel_ui::text_button_sized(ui, &h.to_string(), CELL)
                        .on_hover_text(format!("Column {}, row {}", c + 1, r + 1));
                    if response.clicked() {
                        s.bump(c, r, 1);
                    } else if response.secondary_clicked() {
                        s.bump(c, r, -1);
                    }
                }
                ui.end_row();
            }
        });
}

impl crate::ChimpApp {
    /// Edit → Iso Box…: con lo de la última vez y, con Shade from, el primario como base.
    pub(crate) fn open_iso_box(&mut self) {
        let editor = &self.doc.editor;
        if !editor.clip_layer_ok(true) {
            return;
        }
        let primary = editor.resolve(editor.primary);
        let sprite = editor.sprite();
        let indexed = matches!(sprite.color_mode, cs_core::ColorMode::Indexed { .. });
        self.iso_dialog = Some(Box::new(IsoDialog::new(
            self.iso_settings.clone(),
            primary,
            sprite.palette.colors(),
            indexed,
        )));
    }

    /// Place deja el dibujo flotando en la capa actual (centrado en la selección o en el
    /// lienzo); Cancel lo cierra. Lo elegido queda para la próxima vez.
    pub(crate) fn update_iso_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.iso_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.iso_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            DialogResult::Open => {}
            DialogResult::Accept => {
                self.iso_settings = dialog.settings.clone();
                let image = dialog.image();
                let label = dialog.label();
                self.iso_dialog = None;
                let editor = &mut self.doc.editor;
                let origin = editor.centered_origin((image.width, image.height));
                if let Some(clip) = crate::clipboard::Clip::from_shape(
                    origin,
                    (image.width, image.height),
                    &image.pixels,
                ) {
                    editor.place_floating(&clip, label);
                }
            }
            DialogResult::Cancel => {
                self.iso_settings = dialog.settings.clone();
                self.iso_dialog = None;
            }
        }
    }
}

#[cfg(test)]
#[path = "iso_dialog_tests.rs"]
mod tests;
