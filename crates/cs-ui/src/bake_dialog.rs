//! Bake…: hornear el modelo 3D activo a sprites (ángulos, cámara, textura animada y salida).

use cs_core::CameraPreset;
use cs_render::{BakeOptions, BakeOutput};
use eframe::egui;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window, finish};
use crate::widgets;

/// Los ángulos que se pueden elegir.
const ANGLES: [u32; 5] = [1, 4, 8, 16, 32];

/// Las cámaras: la del modelo o un preset.
const CAMERAS: [Option<CameraPreset>; 5] = [
    None,
    Some(CameraPreset::Iso),
    Some(CameraPreset::Top),
    Some(CameraPreset::Side),
    Some(CameraPreset::Front),
];

fn camera_name(c: Option<CameraPreset>) -> &'static str {
    match c {
        None => "Model",
        Some(CameraPreset::Iso) => "Iso",
        Some(CameraPreset::Top) => "Top",
        Some(CameraPreset::Side) => "Side",
        Some(CameraPreset::Front) => "Front",
    }
}

pub struct BakeDialog {
    pub options: BakeOptions,
    /// Los frames de la textura del modelo: con más de uno se puede animar.
    texture_frames: usize,
}

impl BakeDialog {
    /// Para el modelo activo de `editor`: 8 ángulos, su cámara, en frames y con la textura
    /// animada si tiene varios frames.
    pub fn new(editor: &Editor) -> Self {
        BakeDialog {
            options: BakeOptions {
                angles: 8,
                camera: None,
                animate_texture: true,
                output: BakeOutput::Frames,
            },
            texture_frames: editor.model_texture_frames(),
        }
    }

    /// Las opciones elegidas (sin textura animada, `animate_texture` queda apagado).
    pub fn chosen(&self) -> BakeOptions {
        BakeOptions {
            animate_texture: self.options.animate_texture && self.texture_frames > 1,
            ..self.options
        }
    }

    /// Con `skin` (Pixel UI), en el diálogo pixel del skin.
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let animated = self.texture_frames > 1;
        let o = &mut self.options;
        let (result, open) = dialog_window(ctx, skin, "bake", "Bake", |ui| {
            egui::Grid::new("bake_grid")
                .num_columns(2)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Angles:");
                    widgets::combo(ui, "bake_angles", &o.angles.to_string(), |ui| {
                        for a in ANGLES {
                            widgets::choice(ui, &mut o.angles, a, &a.to_string());
                        }
                    });
                    ui.end_row();
                    ui.label("Camera:");
                    widgets::combo(ui, "bake_camera", camera_name(o.camera), |ui| {
                        for c in CAMERAS {
                            widgets::choice(ui, &mut o.camera, c, camera_name(c));
                        }
                    });
                    ui.end_row();
                    if animated {
                        ui.label("");
                        widgets::checkbox(ui, &mut o.animate_texture, "Animate texture");
                        ui.end_row();
                    }
                    ui.label("Output:");
                    ui.horizontal(|ui| {
                        widgets::radio_value(ui, &mut o.output, BakeOutput::Frames, "Frames");
                        widgets::radio_value(
                            ui,
                            &mut o.output,
                            BakeOutput::Spritesheet,
                            "Spritesheet",
                        );
                    });
                    ui.end_row();
                });
        });
        finish(ctx, open, result)
    }
}
