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

    pub fn update(&mut self, ctx: &egui::Context) {
        let Some(path) = self.path.clone() else {
            return;
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
        }
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
