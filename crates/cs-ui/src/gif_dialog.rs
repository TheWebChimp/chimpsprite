//! GIF settings (como la ventanita de picoCAD 2): exportar el modelo 3D girando (turntable) a un
//! GIF animado o a una tira PNG, con la cámara de Sheet settings más el FOV, el contorno, las
//! scanlines, un preset de render (resolución y paleta), la escala, el fondo, la duración de cada
//! frame y el loop. A la derecha, la vista previa animada.

use std::ops::RangeInclusive;
use std::time::Duration;

use cs_core::{ArtboardId, Document, Rgba};
use cs_render::{
    BAKE_FOV, RenderPreset, SCANLINES, TURNTABLE_FRAME_MS, TURNTABLE_SCALE, TurntableOptions,
};
use eframe::egui::{self, Color32, Rect, Ui, vec2};

use crate::bake_dialog::{camera_fields, check, number, number_sized, row};
use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// El cuadro de la vista previa, en puntos (la pantalla de la DS a escala 1 casi cabe).
const PREVIEW: egui::Vec2 = vec2(160.0, 120.0);
/// Entre las columnas.
const COLUMN_GAP: f32 = 12.0;
/// Los pixeles que puede tener el archivo entre todos sus frames (≈ 256 MB en RGBA): con Scale 8,
/// la DS y 64 frames se pediría mucha más memoria.
const MAX_PIXELS: u64 = 64 * 1024 * 1024;
/// El stepper de la intensidad de las scanlines (más angosto: va después de la casilla).
const SMALL_STEPPER_W: f32 = 64.0;
/// El FOV, en grados.
const FOV_DEGREES: RangeInclusive<i32> = (*BAKE_FOV.start() / 10)..=(*BAKE_FOV.end() / 10);

/// Lo que se escribe.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GifFormat {
    /// GIF animado.
    #[default]
    Gif,
    /// Una tira horizontal PNG con los frames.
    Png,
}

impl GifFormat {
    pub fn extension(self) -> &'static str {
        match self {
            GifFormat::Gif => "gif",
            GifFormat::Png => "png",
        }
    }
}

/// La vista previa calculada: para qué opciones, ángulo y escala, su tamaño y su textura.
struct Preview {
    key: (TurntableOptions, u32, u32),
    size: (u32, u32),
    texture: Option<egui::TextureHandle>,
}

pub struct GifDialog {
    pub options: TurntableOptions,
    pub format: GifFormat,
    /// El modelo que se exporta y su nombre (el del archivo).
    model: ArtboardId,
    name: String,
    /// El tamaño del artboard del modelo (da la proporción).
    model_size: (u32, u32),
    /// Lo que dura la animación del modelo (ms).
    anim_ms: u32,
    /// Se recalcula solo si cambian las opciones, el ángulo o la escala.
    preview: Option<Preview>,
}

impl GifDialog {
    /// Para el modelo que se edita en `editor`, con las opciones de
    /// [`TurntableOptions::from_model`].
    pub fn new(editor: &Editor) -> Self {
        let id = editor.model_id();
        let (name, model_size) = editor.document.artboard(id).map_or_else(
            || ("model".to_owned(), (1, 1)),
            |a| (a.name.clone(), (a.sprite.width, a.sprite.height)),
        );
        let model = editor.edited_model().cloned().unwrap_or_default();
        GifDialog {
            options: TurntableOptions::from_model(&model, model_size),
            format: GifFormat::Gif,
            model: id,
            name,
            model_size,
            anim_ms: model.anim_ms,
            preview: None,
        }
    }

    /// El modelo que se exporta.
    pub(crate) fn model(&self) -> ArtboardId {
        self.model
    }

    /// El nombre sugerido del archivo: el del modelo con la extensión del formato.
    pub fn file_name(&self) -> String {
        format!("{}.{}", self.name, self.format.extension())
    }

    /// El archivo: el GIF o la tira PNG de los frames del turntable de `doc`.
    pub fn build(&self, doc: &Document) -> Result<Vec<u8>, String> {
        let (w, h) = self.options.output_size(self.model_size);
        let frames = u64::from(self.options.bake.frames.max(1));
        if u64::from(w) * u64::from(h) * frames > MAX_PIXELS {
            return Err(format!(
                "{w}×{h} with {frames} frames is too big: lower the scale, the size or the frames."
            ));
        }
        let t = cs_render::turntable(doc, self.model, &self.options)
            .ok_or_else(|| "The model can't be rendered.".to_owned())?;
        let bytes = match self.format {
            GifFormat::Gif => cs_io::export::encode_gif(
                t.width,
                t.height,
                &t.frames,
                self.options.frame_ms(self.anim_ms),
                self.options.looping,
            ),
            GifFormat::Png => {
                let mut strip =
                    cs_io::export::Bitmap::new(t.width * t.frames.len() as u32, t.height);
                for (i, pixels) in t.frames.into_iter().enumerate() {
                    let frame = cs_io::export::Bitmap {
                        width: t.width,
                        height: t.height,
                        pixels,
                    };
                    strip.blit(&frame, i as u32 * t.width, 0);
                }
                strip.to_png()
            }
        };
        bytes.map_err(|e| e.to_string())
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin. `doc` es el documento del modelo
    /// (para la vista previa).
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        skin: Option<&Skin>,
        doc: &Document,
    ) -> DialogResult {
        // La vista previa gira a la duración elegida.
        let frames = self.options.bake.frames.max(1);
        let ms = u64::from(self.options.frame_ms(self.anim_ms));
        let angle = ((ctx.input(|i| i.time) * 1000.0) as u64 / ms) as u32 % frames;
        if frames > 1 {
            ctx.request_repaint_after(Duration::from_millis(ms));
        }
        let (w, h) = self.options.output_size(self.model_size);
        let scale = self.options.scale();
        let shown_scale = preview_scale((w / scale, h / scale), scale);
        self.update_preview(ctx, doc, angle, shown_scale);
        let caption = format!("{w}×{h}, {frames} frames");
        let (o, format, preview) = (&mut self.options, &mut self.format, &self.preview);
        let buttons = ["Export", "Cancel"];
        let (result, open) = dialog_window_with(ctx, skin, "gif", "GIF settings", buttons, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    let native = o.preset == RenderPreset::Native;
                    camera_fields(ui, skin, &mut o.bake, native);
                    fov_row(ui, skin, &mut o.bake);
                });
                ui.add_space(COLUMN_GAP);
                ui.vertical(|ui| look_fields(ui, skin, o, format));
                ui.add_space(COLUMN_GAP);
                ui.vertical(|ui| {
                    show_preview(ui, preview.as_ref());
                    ui.label(caption);
                });
            });
        });
        finish(ctx, open, result)
    }

    /// Recalcula la vista previa si cambiaron las opciones, el ángulo o la escala.
    fn update_preview(&mut self, ctx: &egui::Context, doc: &Document, angle: u32, scale: u32) {
        let key = (self.options, angle, scale);
        if self.preview.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let rendered = cs_render::turntable_frame(doc, self.model, &self.options, angle, scale);
        let mut texture = self.preview.take().and_then(|p| p.texture);
        let size = match rendered {
            Some((w, h, pixels)) => {
                let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                let image =
                    egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                let opts = egui::TextureOptions::NEAREST;
                match &mut texture {
                    Some(t) => t.set(image, opts),
                    None => texture = Some(ctx.load_texture("gif_preview", image, opts)),
                }
                (w, h)
            }
            None => (0, 0),
        };
        self.preview = Some(Preview { key, size, texture });
    }
}

/// La escala de la vista previa para frames de `size` (a escala 1): la elegida si cabe en el
/// cuadro, si no la mayor que quepa (al menos 1). Así las scanlines se ven como en el archivo.
fn preview_scale(size: (u32, u32), chosen: u32) -> u32 {
    let fits = |s: u32| (size.0 * s) as f32 <= PREVIEW.x && (size.1 * s) as f32 <= PREVIEW.y;
    (1..=chosen.max(1)).rev().find(|&s| fits(s)).unwrap_or(1)
}

/// El FOV en grados (lo guardado, en décimas), deshabilitado en orto.
fn fov_row(ui: &mut Ui, skin: Option<&Skin>, o: &mut cs_render::BakeOptions) {
    row(ui, "FOV (°)", |ui| {
        ui.add_enabled_ui(!o.ortho, |ui| {
            let shown = (o.fov / 10).clamp(*FOV_DEGREES.start(), *FOV_DEGREES.end());
            let mut degrees = shown;
            number(ui, skin, "gif_fov", &mut degrees, FOV_DEGREES, 5);
            if degrees != shown {
                o.fov = degrees * 10;
            }
        });
    });
}

/// Una casilla con un color al lado (solo si está prendida): `value` es `None` apagada. Al
/// prenderla toma `default`.
fn color_check(
    ui: &mut Ui,
    skin: Option<&Skin>,
    value: &mut Option<Rgba>,
    label: &str,
    default: Rgba,
) {
    row(ui, "", |ui| {
        let mut on = value.is_some();
        check(ui, skin, &mut on, label);
        if on != value.is_some() {
            *value = on.then_some(default);
        }
        if let Some(color) = value {
            widgets::color_edit(ui, color, false, &format!("{label} color"));
        }
    });
}

/// La columna del aspecto: Render, Scale, Outline, Scanlines, Background, Frame (ms), Loop y
/// Format.
fn look_fields(ui: &mut Ui, skin: Option<&Skin>, o: &mut TurntableOptions, format: &mut GifFormat) {
    ui.spacing_mut().item_spacing.y = 6.0;
    row(ui, "Render", |ui| {
        widgets::combo(ui, "gif_render", o.preset.label(), |ui| {
            for preset in RenderPreset::ALL {
                widgets::choice(ui, &mut o.preset, preset, preset.label());
            }
        });
    });
    row(ui, "Scale", |ui| {
        number(ui, skin, "gif_scale", &mut o.scale, TURNTABLE_SCALE, 1)
    });
    color_check(ui, skin, &mut o.outline, "Outline", Rgba::rgb(0, 0, 0));
    row(ui, "", |ui| {
        let mut on = o.scanlines.is_some();
        check(ui, skin, &mut on, "Scanlines");
        if on != o.scanlines.is_some() {
            o.scanlines = on.then_some(50);
        }
        if let Some(strength) = &mut o.scanlines {
            ui.add_space(4.0);
            number_sized(
                ui,
                skin,
                SMALL_STEPPER_W,
                "gif_scanlines",
                strength,
                SCANLINES,
                10,
            );
        }
    });
    color_check(
        ui,
        skin,
        &mut o.background,
        "Background",
        Rgba::rgb(0, 0, 0),
    );
    row(ui, "Frame (ms)", |ui| {
        number(
            ui,
            skin,
            "gif_frame_ms",
            &mut o.frame_ms,
            TURNTABLE_FRAME_MS,
            10,
        )
    });
    row(ui, "", |ui| check(ui, skin, &mut o.looping, "Loop"));
    row(ui, "Format", |ui| {
        ui.vertical(|ui| {
            widgets::radio_value(ui, format, GifFormat::Gif, "GIF");
            widgets::radio_value(ui, format, GifFormat::Png, "PNG strip");
        });
    });
}

/// El cuadro de la vista previa: damero de fondo y el frame a la mayor escala entera que quepa
/// (si no cabe ni a 1×, achicado), centrado.
fn show_preview(ui: &mut Ui, preview: Option<&Preview>) {
    let (rect, _) = ui.allocate_exact_size(PREVIEW, egui::Sense::hover());
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
    let fit = (PREVIEW.x / *w as f32).min(PREVIEW.y / *h as f32);
    let scale = if fit >= 1.0 { fit.floor() } else { fit };
    let size = vec2(*w as f32, *h as f32) * scale;
    let image = Rect::from_center_size(rect.center(), size);
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    painter.image(texture.id(), image, uv, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_preview_uses_the_chosen_scale_while_it_fits() {
        assert_eq!(preview_scale((32, 32), 3), 3);
        assert_eq!(preview_scale((32, 32), 8), 3);
        assert_eq!(preview_scale((256, 192), 2), 1);
        assert_eq!(preview_scale((80, 60), 4), 2);
        assert_eq!(preview_scale((10, 10), 0), 1);
    }
}
