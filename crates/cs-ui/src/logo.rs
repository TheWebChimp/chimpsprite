//! Las animaciones de la UI hechas en Aseprite (embebidas), un frame por textura, al ritmo de
//! sus duraciones: el logo de Home (`assets/ui/logo.aseprite`) y el mono que corre arriba a la
//! derecha, sobre el degradé de la barra de menú (`assets/ui/running.aseprite`).

use cs_core::Rgba;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions, Vec2, vec2};

use crate::pixel_ui::Tip as _;
use crate::tab_bar;

pub const EMBEDDED: &[u8] = include_bytes!("../../../assets/ui/logo.aseprite");
pub const RUNNING: &[u8] = include_bytes!("../../../assets/ui/running.aseprite");
/// Cada píxel del logo, en puntos: con la escala entera de Pixel UI sigue nítido.
pub const SCALE: f32 = 2.0;

/// Los frames del logo ya compuestos (todas las capas visibles), sin egui.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogoFrames {
    pub size: [usize; 2],
    pub frames: Vec<Vec<Rgba>>,
    pub durations_ms: Vec<u32>,
}

/// Lee el logo de `bytes` (un `.aseprite`).
pub fn decode(bytes: &[u8]) -> Option<LogoFrames> {
    let (sprite, _) = cs_io::aseprite::read(bytes).ok()?;
    let frames = (0..sprite.frames.len())
        .map(|i| cs_render::render_frame(&sprite, i))
        .collect();
    let durations_ms = sprite
        .frames
        .iter()
        .map(|f| u32::from(f.duration_ms))
        .collect();
    Some(LogoFrames {
        size: [sprite.width as usize, sprite.height as usize],
        frames,
        durations_ms,
    })
}

/// El logo cargado en egui.
pub struct Logo {
    size: Vec2,
    /// Puntos por píxel del dibujo.
    scale: f32,
    frames: Vec<TextureHandle>,
    durations_ms: Vec<u32>,
}

impl Logo {
    /// El logo embebido; `None` si no se pudo leer.
    pub fn embedded(ctx: &egui::Context) -> Option<Logo> {
        Self::load(ctx, EMBEDDED, "logo", SCALE)
    }

    /// El mono que corre (a tamaño real); `None` si no se pudo leer.
    pub fn running(ctx: &egui::Context) -> Option<Logo> {
        Self::load(ctx, RUNNING, "running", 1.0)
    }

    /// La animación de `bytes` (un `.aseprite`), con sus texturas llamadas `name.N`.
    fn load(ctx: &egui::Context, bytes: &[u8], name: &str, scale: f32) -> Option<Logo> {
        let data = decode(bytes)?;
        let frames = data
            .frames
            .iter()
            .enumerate()
            .map(|(i, pixels)| {
                let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                let image = ColorImage::from_rgba_unmultiplied(data.size, &rgba);
                ctx.load_texture(format!("{name}.{i}"), image, TextureOptions::NEAREST)
            })
            .collect();
        Some(Logo {
            size: vec2(data.size[0] as f32, data.size[1] as f32),
            scale,
            frames,
            durations_ms: data.durations_ms,
        })
    }

    /// Tamaño en pantalla (puntos).
    pub fn size(&self) -> Vec2 {
        self.size * self.scale
    }

    /// La textura del frame de este momento. Pide el repintado para el siguiente (sin el foco
    /// de la ventana se queda quieto, como la casita de Home).
    fn current(&self, ui: &egui::Ui) -> &TextureHandle {
        let time = ui.input(|i| i.time);
        let (index, left) = tab_bar::animation_frame(&self.durations_ms, time).unwrap_or((0, 0.0));
        if self.frames.len() > 1 && ui.input(|i| i.focused) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs_f64(left));
        }
        &self.frames[index.min(self.frames.len() - 1)]
    }

    /// El frame de este momento, en el lugar que sigue de `ui`.
    pub fn show(&self, ui: &mut egui::Ui) -> egui::Response {
        let texture = self.current(ui);
        ui.add(egui::Image::new((texture.id(), self.size())))
            .tip("ChimpSprite")
    }

    /// El frame de este momento con su esquina en `at` (sin ocupar lugar en `ui`).
    pub fn paint_at(&self, ui: &egui::Ui, at: egui::Pos2) {
        let texture = self.current(ui);
        let rect = egui::Rect::from_min_size(at, self.size());
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        ui.painter()
            .image(texture.id(), rect, uv, egui::Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_running_monkey_is_animated() {
        let run = decode(RUNNING).expect("the embedded animation");
        assert_eq!(run.size, [38, 35]);
        assert_eq!(run.frames.len(), 4);
        assert!(run.durations_ms.iter().all(|&d| d > 0));
    }

    #[test]
    fn the_embedded_logo_is_animated() {
        let logo = decode(EMBEDDED).expect("the embedded logo");
        assert_eq!(logo.size, [78, 34]);
        assert!(logo.frames.len() > 1, "animated");
        assert_eq!(logo.frames.len(), logo.durations_ms.len());
        assert!(logo.durations_ms.iter().all(|&d| d > 0));
        assert!(
            logo.frames
                .iter()
                .all(|f| f.len() == 78 * 34 && f.iter().any(|c| c.a > 0)),
            "every frame has something drawn"
        );
    }
}
