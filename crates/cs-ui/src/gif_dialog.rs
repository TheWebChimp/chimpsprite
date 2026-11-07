//! GIF settings (como la ventanita de picoCAD 2): exportar el modelo 3D girando (turntable) a un
//! GIF animado o a una tira PNG, con la cámara de Sheet settings más el FOV, el contorno, las
//! scanlines, un preset de render (resolución y paleta), la escala, el fondo, la duración de cada
//! frame y el loop. A la derecha, la vista previa animada.
//!
//! Las opciones se recuerdan entre sesiones en la configuración de la app (no en el documento):
//! ver [`GifSettings`].

use std::ops::RangeInclusive;
use std::time::Duration;

use cs_core::{ArtboardId, Document, Rgba};
use cs_render::{
    BAKE_FOV, BAKE_FRAMES, BAKE_SIZE, BAKE_TILT, Motion, RenderPreset, SCANLINES,
    TURNTABLE_FRAME_MS, TURNTABLE_SCALE, TurntableOptions,
};
use eframe::egui::{self, Color32, Rect, Ui, vec2};
use serde::{Deserialize, Serialize};

use crate::bake_dialog::{HEIGHT, ZOOM, camera_fields, check, number, number_sized, row};
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

/// Lo último que se exportó con GIF settings, para la próxima vez (también en otra sesión): se
/// guarda en la configuración de la app (`chimpsprite.gif`), no en el documento. Cada campo que
/// falte (un archivo de configuración de antes, o nada exportado todavía) toma lo del modelo
/// ([`TurntableOptions::from_model`]); un valor fuera de rango (el archivo a mano) se recorta.
/// Motion y Render van por su nombre: uno que ya no exista se ignora.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GifSettings {
    pub motion: Option<String>,
    pub frames: Option<u32>,
    pub size: Option<u32>,
    pub zoom: Option<u32>,
    /// En décimas de grado, como [`cs_render::BakeOptions::tilt`].
    pub tilt: Option<i32>,
    pub height: Option<i32>,
    pub ortho: Option<bool>,
    /// En décimas de grado.
    pub fov: Option<i32>,
    pub render: Option<String>,
    pub scale: Option<u32>,
    /// La casilla y su color (el color queda aunque se apague).
    pub outline: Option<bool>,
    pub outline_color: Option<Rgba>,
    pub scanlines: Option<bool>,
    pub scanline_strength: Option<u8>,
    pub background: Option<bool>,
    pub background_color: Option<Rgba>,
    pub frame_ms: Option<u32>,
    pub looping: Option<bool>,
    /// "gif" o "png".
    pub format: Option<String>,
}

impl GifSettings {
    /// Todo lo del diálogo `d`.
    pub fn of(d: &GifDialog) -> GifSettings {
        let (o, bake) = (&d.options, &d.options.bake);
        GifSettings {
            motion: Some(bake.motion.label().to_owned()),
            frames: Some(bake.frames),
            size: Some(bake.size),
            zoom: Some(bake.zoom),
            tilt: Some(bake.tilt),
            height: Some(bake.height),
            ortho: Some(bake.ortho),
            fov: Some(bake.fov),
            render: Some(o.preset.label().to_owned()),
            scale: Some(o.scale),
            outline: Some(o.outline.is_some()),
            outline_color: o.outline,
            scanlines: Some(o.scanlines.is_some()),
            scanline_strength: o.scanlines,
            background: Some(o.background.is_some()),
            background_color: o.background,
            frame_ms: Some(o.frame_ms),
            looping: Some(o.looping),
            format: Some(d.format.extension().to_owned()),
        }
    }

    /// Pone en `d` lo que se recuerda (lo que falte queda como está).
    fn apply(&self, d: &mut GifDialog) {
        let o = &mut d.options;
        let b = &mut o.bake;
        if let Some(m) = self.motion.as_deref().and_then(motion_named) {
            b.motion = m;
        }
        set(&mut b.frames, self.frames, BAKE_FRAMES);
        set(&mut b.size, self.size, BAKE_SIZE);
        set(&mut b.zoom, self.zoom, ZOOM);
        set(&mut b.tilt, self.tilt, BAKE_TILT);
        set(&mut b.height, self.height, HEIGHT);
        b.ortho = self.ortho.unwrap_or(b.ortho);
        set(&mut b.fov, self.fov, BAKE_FOV);
        if let Some(p) = self.render.as_deref().and_then(preset_named) {
            o.preset = p;
        }
        set(&mut o.scale, self.scale, TURNTABLE_SCALE);
        toggle(
            &mut o.outline,
            self.outline,
            self.outline_color,
            Rgba::rgb(0, 0, 0),
        );
        let strength = self
            .scanline_strength
            .map(|s| s.clamp(*SCANLINES.start(), *SCANLINES.end()));
        toggle(&mut o.scanlines, self.scanlines, strength, 50);
        toggle(
            &mut o.background,
            self.background,
            self.background_color,
            Rgba::rgb(0, 0, 0),
        );
        set(&mut o.frame_ms, self.frame_ms, TURNTABLE_FRAME_MS);
        o.looping = self.looping.unwrap_or(o.looping);
        match self.format.as_deref() {
            Some("gif") => d.format = GifFormat::Gif,
            Some("png") => d.format = GifFormat::Png,
            _ => {}
        }
    }
}

/// `value` recortado a `range`, si hay.
fn set<T: Copy + PartialOrd>(target: &mut T, value: Option<T>, range: RangeInclusive<T>) {
    if let Some(v) = value {
        *target = if v < *range.start() {
            *range.start()
        } else if v > *range.end() {
            *range.end()
        } else {
            v
        };
    }
}

/// Una casilla con su valor: `on` la prende (con `value`, lo que ya tenía o `default`) o la apaga;
/// sin `on`, queda como está.
fn toggle<T: Copy>(target: &mut Option<T>, on: Option<bool>, value: Option<T>, default: T) {
    match on {
        Some(true) => *target = Some(value.or(*target).unwrap_or(default)),
        Some(false) => *target = None,
        None => {}
    }
}

fn motion_named(name: &str) -> Option<Motion> {
    Motion::ALL.into_iter().find(|m| m.label() == name)
}

fn preset_named(name: &str) -> Option<RenderPreset> {
    RenderPreset::ALL.into_iter().find(|p| p.label() == name)
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
    /// [`TurntableOptions::from_model`] y encima lo que se recuerda de la última vez.
    pub fn with_settings(editor: &Editor, settings: &GifSettings) -> Self {
        let mut dialog = Self::new(editor);
        settings.apply(&mut dialog);
        dialog
    }

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
        let anim_ms = self.anim_ms;
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
                ui.vertical(|ui| look_fields(ui, skin, o, format, anim_ms));
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
/// Format. Con la animación (Motion), Frame (ms) es el de la animación (`anim_ms` repartidos
/// entre los frames) y no se cambia.
fn look_fields(
    ui: &mut Ui,
    skin: Option<&Skin>,
    o: &mut TurntableOptions,
    format: &mut GifFormat,
    anim_ms: u32,
) {
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
        if o.bake.motion.animates() {
            let mut shown = o.frame_ms(anim_ms);
            ui.add_enabled_ui(false, |ui| {
                number(ui, skin, "gif_frame_ms", &mut shown, TURNTABLE_FRAME_MS, 10)
            });
        } else {
            number(
                ui,
                skin,
                "gif_frame_ms",
                &mut o.frame_ms,
                TURNTABLE_FRAME_MS,
                10,
            )
        }
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
