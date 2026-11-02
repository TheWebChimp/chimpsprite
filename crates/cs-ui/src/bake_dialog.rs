//! Sheet settings: hornear el modelo 3D activo a sprites (frames, tamaño, zoom, inclinación,
//! altura, proyección, textura animada y salida), con una vista previa que gira.

use std::ops::RangeInclusive;
use std::time::Duration;

use cs_core::{ArtboardId, Document};
use cs_render::{BAKE_FRAMES, BAKE_SIZE, BAKE_TILT, BakeOptions, BakeOutput};
use eframe::egui::{self, Color32, Rect, Ui, vec2};

use crate::editor::Editor;
use crate::kit;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El lado del cuadro de la vista previa, en puntos.
const PREVIEW: f32 = 96.0;
/// El ancho de la columna de las etiquetas y el de los steppers.
const LABEL_W: f32 = 64.0;
const STEPPER_W: f32 = 96.0;
/// Ángulos por segundo de la vista previa.
const PREVIEW_FPS: f64 = 8.0;
/// Los límites del zoom (%) y de la altura (1/16 de unidad).
const ZOOM: RangeInclusive<u32> = 10..=1000;
const HEIGHT: RangeInclusive<i32> = -1024..=1024;

/// La vista previa calculada: para qué opciones y ángulo, su tamaño y su textura.
struct Preview {
    key: (BakeOptions, u32),
    size: (u32, u32),
    texture: Option<egui::TextureHandle>,
}

pub struct BakeDialog {
    pub options: BakeOptions,
    /// El modelo que se hornea (para la vista previa).
    model: ArtboardId,
    /// Los frames de la textura del modelo: con más de uno se puede animar.
    texture_frames: usize,
    /// Se recalcula solo si cambian las opciones o el ángulo.
    preview: Option<Preview>,
}

impl BakeDialog {
    /// Para el modelo activo de `editor`, con las opciones de [`BakeOptions::from_model`].
    pub fn new(editor: &Editor) -> Self {
        let texture_frames = editor.model_texture_frames();
        let sprite = editor.sprite();
        let model = editor.model().cloned().unwrap_or_default();
        BakeDialog {
            options: BakeOptions::from_model(&model, (sprite.width, sprite.height), texture_frames),
            model: editor.active,
            texture_frames,
            preview: None,
        }
    }

    /// El modelo que se hornea.
    pub(crate) fn model(&self) -> ArtboardId {
        self.model
    }

    /// Las opciones elegidas (sin textura animada, `animate_texture` queda apagado).
    pub fn chosen(&self) -> BakeOptions {
        BakeOptions {
            animate_texture: self.options.animate_texture && self.texture_frames > 1,
            ..self.options
        }
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin. `doc` es el documento del modelo
    /// (para la vista previa).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        skin: Option<&Skin>,
        doc: &Document,
    ) -> DialogResult {
        let animated = self.texture_frames > 1;
        // La vista previa gira: un ángulo nuevo cada 1/8 s.
        let frames = self.options.frames.max(1);
        let angle = (ctx.input(|i| i.time) * PREVIEW_FPS) as u32 % frames;
        if spins(frames) {
            ctx.request_repaint_after(Duration::from_millis(125));
        }
        self.update_preview(ctx, doc, angle);
        let (o, preview) = (&mut self.options, &self.preview);
        let buttons = ["Export", "Cancel"];
        let (result, open) =
            dialog_window_with(ctx, skin, "bake", "Sheet settings", buttons, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| fields(ui, skin, o, animated));
                    ui.add_space(12.0);
                    show_preview(ui, preview.as_ref());
                });
            });
        finish(ctx, open, result)
    }

    /// Recalcula la vista previa si cambiaron las opciones o el ángulo.
    fn update_preview(&mut self, ctx: &egui::Context, doc: &Document, angle: u32) {
        let options = self.chosen();
        let key = (options, angle);
        if self.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let rendered = cs_render::bake_preview(doc, self.model, &options, angle);
        let mut texture = self.preview.take().and_then(|p| p.texture);
        let size = match rendered {
            Some((w, h, pixels)) => {
                let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                let opts = egui::TextureOptions::NEAREST;
                match &mut texture {
                    Some(t) => t.set(image, opts),
                    None => texture = Some(ctx.load_texture("bake_preview", image, opts)),
                }
                (w, h)
            }
            None => (0, 0),
        };
        self.preview = Some(Preview { key, size, texture });
    }
}

/// La vista previa solo gira (y pide repintar) con más de un frame.
fn spins(frames: u32) -> bool {
    frames > 1
}

/// Los campos, en columna: etiqueta y stepper (o casilla, o radios).
fn fields(ui: &mut Ui, skin: Option<&Skin>, o: &mut BakeOptions, animated: bool) {
    ui.spacing_mut().item_spacing.y = 6.0;
    row(ui, "Frames", |ui| {
        number(ui, skin, "bake_frames", &mut o.frames, BAKE_FRAMES, 1)
    });
    row(ui, "Size (px)", |ui| {
        number(ui, skin, "bake_size", &mut o.size, BAKE_SIZE, 1)
    });
    row(ui, "Zoom (%)", |ui| {
        number(ui, skin, "bake_zoom", &mut o.zoom, ZOOM, 10)
    });
    // La inclinación se edita en grados; un valor que no sea de grados enteros queda igual hasta
    // que se cambie.
    row(ui, "Tilt (°)", |ui| {
        let range = BAKE_TILT.start() / 10..=BAKE_TILT.end() / 10;
        let shown = (o.tilt / 10).clamp(*range.start(), *range.end());
        let mut degrees = shown;
        number(ui, skin, "bake_tilt", &mut degrees, range, 1);
        if degrees != shown {
            o.tilt = degrees * 10;
        }
    });
    row(ui, "Height", |ui| {
        number(ui, skin, "bake_height", &mut o.height, HEIGHT, 1)
    });
    row(ui, "", |ui| check(ui, skin, &mut o.ortho, "Ortho"));
    if animated {
        row(ui, "", |ui| {
            check(ui, skin, &mut o.animate_texture, "Animate texture")
        });
    }
    row(ui, "Output", |ui| {
        ui.vertical(|ui| {
            widgets::radio_value(ui, &mut o.output, BakeOutput::Frames, "Frames");
            widgets::radio_value(ui, &mut o.output, BakeOutput::Spritesheet, "Spritesheet");
        });
    });
}

/// Una fila: la etiqueta en una columna de [`LABEL_W`] y lo que agregue `add`.
fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            vec2(LABEL_W, kit::stepper::HEIGHT),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.set_min_width(LABEL_W);
                ui.label(label);
            },
        );
        add(ui);
    });
}

/// El stepper del kit (sin Pixel UI, el campo de egui).
fn number<N: egui::emath::Numeric>(
    ui: &mut Ui,
    skin: Option<&Skin>,
    salt: &str,
    value: &mut N,
    range: RangeInclusive<N>,
    step: N,
) {
    match skin {
        Some(skin) => {
            kit::stepper::stepper(ui, skin, STEPPER_W, salt, value, range, step, None);
        }
        None => {
            ui.add(egui::DragValue::new(value).range(range));
        }
    }
}

/// La casilla del kit (sin Pixel UI, la de egui).
fn check(ui: &mut Ui, skin: Option<&Skin>, on: &mut bool, label: &str) {
    match skin {
        Some(skin) => kit::check::checkbox(ui, skin, on, label),
        None => ui.checkbox(on, label),
    };
}

/// El cuadro de la vista previa: damero de fondo y el frame a la mayor escala entera que quepa
/// (si no cabe ni a 1×, achicado), centrado.
fn show_preview(ui: &mut Ui, preview: Option<&Preview>) {
    let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW, PREVIEW), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    crate::panels::checker(&painter, rect);
    let Some(Preview {
        size: (w, h),
        texture: Some(texture),
        ..
    }) = preview
    else {
        return;
    };
    if *w == 0 || *h == 0 {
        return;
    }
    let fit = (PREVIEW / *w as f32).min(PREVIEW / *h as f32);
    let scale = if fit >= 1.0 { fit.floor() } else { fit };
    let size = vec2(*w as f32, *h as f32) * scale;
    let image = Rect::from_center_size(rect.center(), size);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), image, uv, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::spins;

    #[test]
    fn a_single_frame_does_not_spin() {
        assert!(!spins(1));
        assert!(spins(2));
    }
}
