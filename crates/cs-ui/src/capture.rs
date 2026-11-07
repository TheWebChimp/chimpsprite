//! Captura de pantalla automática para desarrollo: `CHIMPSPRITE_SCREENSHOT=<ruta.png>` (y
//! `CHIMPSPRITE_SCREENSHOT_DELAY=<segundos>` para esperar, por ejemplo, lo que baja de la red).

use std::path::{Path, PathBuf};

use eframe::egui;

/// Frames a esperar antes de capturar, para que el layout se estabilice.
const WARMUP_FRAMES: u32 = 10;

#[derive(Default)]
pub struct Capture {
    path: Option<PathBuf>,
    /// Segundos a esperar además de los frames de calentamiento.
    delay: f64,
    frames: u32,
    requested: bool,
}

impl Capture {
    pub fn from_env() -> Self {
        Self {
            path: std::env::var_os("CHIMPSPRITE_SCREENSHOT").map(PathBuf::from),
            delay: std::env::var("CHIMPSPRITE_SCREENSHOT_DELAY")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0),
            ..Default::default()
        }
    }

    /// Devuelve `true` en el frame en que, ya guardada la captura, pide cerrar la ventana: la app
    /// cierra sin preguntar por los cambios (las demos modifican el documento).
    pub fn update(&mut self, ctx: &egui::Context) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        self.frames += 1;
        ctx.request_repaint();
        let waited = ctx.input(|i| i.time) >= self.delay;
        if !self.requested && self.frames >= WARMUP_FRAMES && waited {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.requested = true;
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = shot {
            match save_png(&path, &image) {
                Ok(()) => log::info!("screenshot saved to {}", path.display()),
                Err(e) => log::error!("screenshot failed: {e}"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return true;
        }
        false
    }
}

fn save_png(path: &Path, image: &egui::ColorImage) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, image.size[0] as u32, image.size[1] as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    encoder.write_header()?.write_image_data(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn after_saving_the_shot_it_closes_and_says_so() {
        let path = std::env::temp_dir().join(format!("cs-capture-{}.png", std::process::id()));
        let mut capture = Capture {
            path: Some(path.clone()),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut closing = Vec::new();
        for frame in 0..WARMUP_FRAMES + 1 {
            let mut input = egui::RawInput::default();
            if frame == WARMUP_FRAMES {
                input.events.push(egui::Event::Screenshot {
                    viewport_id: egui::ViewportId::ROOT,
                    user_data: Default::default(),
                    image: std::sync::Arc::new(egui::ColorImage::filled(
                        [2, 2],
                        egui::Color32::RED,
                    )),
                });
            }
            let mut output = ctx.run_ui(input, |ui| closing.push(capture.update(ui.ctx())));
            output.textures_delta.clear();
        }
        assert_eq!(closing.iter().filter(|c| **c).count(), 1);
        assert_eq!(closing.last(), Some(&true));
        assert!(path.exists());
        let _ = std::fs::remove_file(path);
        assert!(!Capture::default().update(&ctx), "without a path, nothing");
    }
}
