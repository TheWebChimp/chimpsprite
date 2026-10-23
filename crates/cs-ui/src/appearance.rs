//! Apariencia de View → Pixel UI: la fuente pixel, la escala entera de la UI, el estilo sin
//! redondeo y el skin. Apagado, todo queda con el tema de siempre.

use std::sync::Arc;

use eframe::egui::{
    self, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, TextStyle,
};

use crate::skin::Skin;
use crate::theme;
use crate::view::ViewSettings;

/// Fuente pixel del autor del proyecto: grilla de 16 px por em, mayúsculas de 7 px.
const PIXEL_FONT: &[u8] = include_bytes!("../../../assets/ui/chimpsprite.ttf");
pub const PIXEL_FONT_NAME: &str = "chimpsprite";
/// A 16 puntos, un píxel de la grilla de la fuente mide un punto (un píxel del skin).
pub const PIXEL_FONT_SIZE: f32 = 16.0;

/// Fuentes de egui. Con `pixel`, la fuente pixel va primera en la proporcional, y las de egui
/// quedan detrás para los glifos que no tiene (`•`, `…`, `▸`). La monoespaciada no cambia.
pub fn font_definitions(pixel: bool) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    if pixel {
        // Sin hinting: los contornos ya caen en la grilla y el hinting solo podría moverlos.
        let tweak = FontTweak {
            hinting: Some(false),
            ..Default::default()
        };
        fonts.font_data.insert(
            PIXEL_FONT_NAME.to_owned(),
            Arc::new(FontData::from_static(PIXEL_FONT).tweak(tweak)),
        );
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, PIXEL_FONT_NAME.to_owned());
    }
    fonts
}

/// El tema de siempre. Con `pixel`, además, la fuente pixel a 16 en los textos proporcionales y
/// nada redondeado (ventanas, menús y widgets).
pub fn apply_style(ctx: &egui::Context, pixel: bool) {
    theme::apply(ctx);
    let defaults = egui::Style::default().text_styles;
    ctx.all_styles_mut(|style| {
        style.text_styles = defaults.clone();
        if !pixel {
            return;
        }
        for text in [
            TextStyle::Small,
            TextStyle::Body,
            TextStyle::Button,
            TextStyle::Heading,
        ] {
            style
                .text_styles
                .insert(text, FontId::proportional(PIXEL_FONT_SIZE));
        }
        let visuals = &mut style.visuals;
        visuals.window_corner_radius = CornerRadius::ZERO;
        visuals.menu_corner_radius = CornerRadius::ZERO;
        for w in [
            &mut visuals.widgets.noninteractive,
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.open,
        ] {
            w.corner_radius = CornerRadius::ZERO;
        }
    });
}

/// Pixel UI en la app: el skin y lo último aplicado.
pub struct PixelUi {
    /// Se lee la primera vez que se enciende.
    skin: Option<Skin>,
    /// Lo último aplicado: `(encendido, escala)`.
    applied: Option<(bool, u8)>,
    /// Recarga en vivo (`CHIMPSPRITE_SKIN`).
    #[cfg(not(target_arch = "wasm32"))]
    watcher: Option<crate::skin::watch::SkinWatcher>,
}

impl PixelUi {
    pub fn from_env() -> Self {
        Self {
            skin: None,
            applied: None,
            #[cfg(not(target_arch = "wasm32"))]
            watcher: crate::skin::watch::SkinWatcher::from_env(),
        }
    }

    /// El skin, solo con Pixel UI encendido.
    pub fn skin(&self) -> Option<&Skin> {
        if self.applied.is_some_and(|(on, _)| on) {
            self.skin.as_ref()
        } else {
            None
        }
    }

    /// Una vez por frame, antes de dibujar: aplica el interruptor y la escala si cambiaron, y
    /// mantiene `pixels_per_point` entero aunque cambie la escala del sistema.
    pub fn update(&mut self, ctx: &egui::Context, view: &ViewSettings) {
        let state = (view.pixel_ui, view.pixel_scale());
        if view.pixel_ui {
            self.load_skin(ctx);
            // No hace nada si ya es esa escala; egui la traduce a zoom = escala / la del sistema.
            ctx.set_pixels_per_point(state.1 as f32);
        }
        if self.applied == Some(state) {
            return;
        }
        let was_on = self.applied.is_some_and(|(on, _)| on);
        self.applied = Some(state);
        ctx.set_fonts(font_definitions(view.pixel_ui));
        apply_style(ctx, view.pixel_ui);
        // El Ctrl+/- de egui rompería la escala entera. En la web viene apagado (lo maneja el
        // navegador).
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = !view.pixel_ui && !cfg!(target_arch = "wasm32");
        });
        if was_on && !view.pixel_ui {
            // Vuelve a la escala del sistema.
            ctx.set_zoom_factor(1.0);
        }
    }

    /// Lee el skin la primera vez. En escritorio, con `CHIMPSPRITE_SKIN`, lo vuelve a leer cuando
    /// cambia el archivo; si está roto, se queda el anterior y se avisa en el log.
    fn load_skin(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(watcher) = &mut self.watcher {
            // Revisa el archivo aunque no haya eventos.
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
            if let Some(bytes) = watcher.poll(ctx.input(|i| i.time)) {
                match Skin::load(ctx, &bytes) {
                    Ok(skin) => self.skin = Some(skin),
                    Err(e) => log::warn!("{e} (keeping the previous skin)"),
                }
            }
        }
        if self.skin.is_none() {
            self.skin = Some(Skin::embedded(ctx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, pixel: &mut PixelUi, view: &ViewSettings) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| pixel.update(ui.ctx(), view));
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
    }

    #[test]
    fn the_pixel_font_goes_first_only_in_the_proportional_family() {
        let plain = font_definitions(false);
        assert_eq!(plain, FontDefinitions::default());
        let pixel = font_definitions(true);
        let proportional = &pixel.families[&FontFamily::Proportional];
        assert_eq!(proportional[0], PIXEL_FONT_NAME);
        assert!(proportional.len() > 1, "egui fonts stay as fallback");
        assert_eq!(
            pixel.families[&FontFamily::Monospace],
            plain.families[&FontFamily::Monospace]
        );
    }

    #[test]
    fn switching_on_applies_scale_style_and_skin_and_off_restores_them() {
        let ctx = egui::Context::default();
        let mut pixel = PixelUi::from_env();
        let mut view = ViewSettings {
            pixel_ui: true,
            ui_scale: 3,
            ..Default::default()
        };
        // La escala nueva vale desde el frame siguiente.
        frame(&ctx, &mut pixel, &view);
        frame(&ctx, &mut pixel, &view);
        assert_eq!(ctx.pixels_per_point(), 3.0);
        let style = ctx.style_of(egui::Theme::Dark);
        let defaults = egui::Style::default().text_styles;
        assert_eq!(
            style.text_styles[&TextStyle::Body],
            FontId::proportional(PIXEL_FONT_SIZE)
        );
        assert_eq!(
            style.text_styles[&TextStyle::Monospace],
            defaults[&TextStyle::Monospace]
        );
        assert_eq!(style.visuals.window_corner_radius, CornerRadius::ZERO);
        assert!(pixel.skin().is_some());
        assert!(!ctx.options(|o| o.zoom_with_keyboard));

        view.pixel_ui = false;
        frame(&ctx, &mut pixel, &view);
        frame(&ctx, &mut pixel, &view);
        assert_eq!(ctx.pixels_per_point(), 1.0);
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(
            style.text_styles[&TextStyle::Body],
            defaults[&TextStyle::Body]
        );
        assert!(pixel.skin().is_none());
    }
}
