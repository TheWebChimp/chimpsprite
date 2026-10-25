//! Apariencia de View → Pixel UI: la fuente pixel, la escala entera de la UI, el estilo sin
//! redondeo y el skin. Apagado, todo queda con el tema de siempre.

use std::sync::{Arc, OnceLock};

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Shadow,
    Stroke, TextStyle,
};

use crate::skin::{Skin, SkinColors};
use crate::theme;
use crate::view::ViewSettings;

/// Fuente pixel del autor del proyecto: grilla de 16 px por em, mayúsculas de 7 px.
const PIXEL_FONT: &[u8] = include_bytes!("../../../assets/ui/chimpsprite.ttf");
pub const PIXEL_FONT_NAME: &str = "chimpsprite";
/// A 16 puntos, un píxel de la grilla de la fuente mide un punto (un píxel del skin).
pub const PIXEL_FONT_SIZE: f32 = 16.0;
/// La fuente de los textos chicos (chimpfont): solo mayúsculas de 5 px, ASCII. Con Pixel UI es
/// la de `TextStyle::Small`: la barra de estado, los globos, las rutas de Home y Pixnews.
const SMALL_FONT: &[u8] = include_bytes!("../../../assets/ui/chimpfont.ttf");
pub const SMALL_FONT_NAME: &str = "chimpfont";
/// A 8 puntos, un píxel de su grilla mide un punto.
pub const SMALL_FONT_SIZE: f32 = 8.0;
/// Los emojis y símbolos en pixel art de GNU Unifont (SIL OFL 1.1), recortados a esos rangos:
/// lo que las fuentes pixel no tienen (❤ ✨ ☀, 🎨 🎃…), antes que las fuentes de egui.
const UNIFONT_SYMBOLS: &[u8] = include_bytes!("../../../assets/ui/unifont_symbols.otf");
const UNIFONT_EMOJI: &[u8] = include_bytes!("../../../assets/ui/unifont_emoji.otf");
const UNIFONT_SYMBOLS_NAME: &str = "unifont_symbols";
const UNIFONT_EMOJI_NAME: &str = "unifont_emoji";
/// La altura de las mayúsculas de chimpfont (las de la fuente pixel: `tab_bar::CAP_HEIGHT`).
pub const SMALL_CAP_HEIGHT: f32 = 5.0;

/// La familia de los textos chicos. Siempre existe (sin Pixel UI tiene las fuentes de egui).
pub fn small_family() -> FontFamily {
    FontFamily::Name("small".into())
}

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
            Arc::new(FontData::from_static(PIXEL_FONT).tweak(tweak.clone())),
        );
        for (name, bytes) in [
            (UNIFONT_SYMBOLS_NAME, UNIFONT_SYMBOLS),
            (UNIFONT_EMOJI_NAME, UNIFONT_EMOJI),
        ] {
            fonts.font_data.insert(
                name.to_owned(),
                Arc::new(FontData::from_static(bytes).tweak(tweak.clone())),
            );
        }
        let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
        proportional.insert(0, UNIFONT_EMOJI_NAME.to_owned());
        proportional.insert(0, UNIFONT_SYMBOLS_NAME.to_owned());
        proportional.insert(0, PIXEL_FONT_NAME.to_owned());
        fonts.font_data.insert(
            SMALL_FONT_NAME.to_owned(),
            Arc::new(FontData::from_static(SMALL_FONT).tweak(tweak)),
        );
    }
    // Los textos chicos: chimpfont con Pixel UI y, detrás, la proporcional (los glifos que no
    // tiene: acentos, `…`).
    let mut small: Vec<String> = Vec::new();
    if pixel {
        small.push(SMALL_FONT_NAME.to_owned());
    }
    small.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts.families.insert(small_family(), small);
    fonts
}

/// Si la fuente pixel tiene el glifo `c` (sin contar las de respaldo de egui). La fuente se lee
/// una sola vez.
pub fn pixel_font_has(c: char) -> bool {
    static FONT: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    FONT.get_or_init(|| {
        fontdue::Font::from_bytes(PIXEL_FONT, fontdue::FontSettings::default()).ok()
    })
    .as_ref()
    .is_some_and(|font| font.has_glyph(c))
}

/// Mitad y mitad de `a` y `b`.
fn mix(a: Color32, b: Color32) -> Color32 {
    a.lerp_to_gamma(b, 0.5)
}

/// Los colores de egui con Pixel UI, armados con los del skin: tema claro, sin redondeo ni
/// sombras.
/// - Paneles en `background`; ventanas, campos de texto y el fondo de las barras de scroll en
///   `panel`.
/// - Texto en `text` y el débil en `text_dim`; selección `selection` con borde `outline`.
/// - Botones (`weak_bg_fill`) en `panel`, más oscuros al pasar y al presionar; borde `outline`.
/// - `bg_fill` (casillas, sliders y la manija del scroll) en `selection`, y `scroll` al pasar o
///   arrastrar.
/// - Separadores en `canvas`, como la línea bajo la barra del mockup.
pub fn pixel_visuals(c: &SkinColors) -> egui::Visuals {
    let mut v = egui::Visuals::light();
    v.dark_mode = false;
    v.override_text_color = Some(c.text);
    v.weak_text_color = Some(c.text_dim);
    v.panel_fill = c.background;
    v.window_fill = c.panel;
    v.window_stroke = Stroke::new(1.0, c.outline);
    v.extreme_bg_color = c.panel;
    v.code_bg_color = c.panel;
    v.faint_bg_color = mix(c.background, c.panel);
    v.selection.bg_fill = c.selection;
    v.selection.stroke = Stroke::new(1.0, c.outline);
    v.hyperlink_color = c.accent;
    v.window_shadow = Shadow::NONE;
    v.popup_shadow = Shadow::NONE;
    v.window_corner_radius = CornerRadius::ZERO;
    v.menu_corner_radius = CornerRadius::ZERO;
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = c.background;
    w.noninteractive.weak_bg_fill = c.background;
    w.noninteractive.bg_stroke = Stroke::new(1.0, c.canvas);
    w.noninteractive.fg_stroke = Stroke::new(1.0, c.text);
    w.noninteractive.corner_radius = CornerRadius::ZERO;
    let states = [
        (&mut w.inactive, c.panel, c.selection),
        (&mut w.hovered, mix(c.panel, c.background), c.scroll),
        (&mut w.active, c.background, c.scroll),
        (&mut w.open, c.background, c.selection),
    ];
    for (state, weak, strong) in states {
        state.weak_bg_fill = weak;
        state.bg_fill = strong;
        state.bg_stroke = Stroke::new(1.0, c.outline);
        state.fg_stroke = Stroke::new(1.0, c.text);
        state.corner_radius = CornerRadius::ZERO;
        state.expansion = 0.0;
    }
    v
}

/// El tema de siempre. Con `pixel` (los colores del skin), además, la fuente pixel a 16 en los
/// textos proporcionales y los colores de `pixel_visuals`, sin nada redondeado.
pub fn apply_style(ctx: &egui::Context, pixel: Option<&SkinColors>) {
    theme::apply(ctx);
    let defaults = egui::Style::default().text_styles;
    ctx.all_styles_mut(|style| {
        style.text_styles = defaults.clone();
        // La manija del scroll: `bg_fill` con Pixel UI; el color del texto, como siempre, sin él.
        style.spacing.scroll.foreground_color = pixel.is_none();
        let Some(colors) = pixel else {
            return;
        };
        for text in [TextStyle::Body, TextStyle::Button, TextStyle::Heading] {
            style
                .text_styles
                .insert(text, FontId::proportional(PIXEL_FONT_SIZE));
        }
        style.text_styles.insert(
            TextStyle::Small,
            FontId::new(SMALL_FONT_SIZE, small_family()),
        );
        style.visuals = pixel_visuals(colors);
    });
}

/// Pixel UI en la app: el skin y lo último aplicado.
pub struct PixelUi {
    /// Se lee la primera vez que se enciende.
    skin: Option<Skin>,
    /// Lo último aplicado: `(encendido, escala)`.
    applied: Option<(bool, u8)>,
    /// El Ctrl+/- del usuario antes de encender Pixel UI; se devuelve al apagarlo.
    prev_zoom: f32,
    /// La escala entera y la fuente pixel ya rigen (`pixels_per_point` la alcanzó y las fuentes
    /// se aplicaron en un frame anterior): antes, los widgets pixel se dibujarían un frame a otra
    /// escala o con la fuente de egui.
    scale_ready: bool,
    /// El skin se recargó (`CHIMPSPRITE_SKIN`): sus colores se vuelven a aplicar a egui.
    skin_changed: bool,
    /// Recarga en vivo (`CHIMPSPRITE_SKIN`).
    #[cfg(not(target_arch = "wasm32"))]
    watcher: Option<crate::skin::watch::SkinWatcher>,
}

impl PixelUi {
    pub fn from_env() -> Self {
        Self {
            skin: None,
            applied: None,
            prev_zoom: 1.0,
            scale_ready: false,
            skin_changed: false,
            #[cfg(not(target_arch = "wasm32"))]
            watcher: crate::skin::watch::SkinWatcher::from_env(),
        }
    }

    /// El skin, solo con Pixel UI encendido y la escala entera ya aplicada.
    pub fn skin(&self) -> Option<&Skin> {
        if self.scale_ready && self.applied.is_some_and(|(on, _)| on) {
            self.skin.as_ref()
        } else {
            None
        }
    }

    /// Una vez por frame, antes de dibujar: aplica el interruptor y la escala si cambiaron, y
    /// mantiene `pixels_per_point` entero aunque cambie la escala del sistema.
    pub fn update(&mut self, ctx: &egui::Context, view: &ViewSettings) {
        let state = (view.pixel_ui, view.pixel_scale());
        let was_on = self.applied.is_some_and(|(on, _)| on);
        if view.pixel_ui {
            if !was_on {
                self.prev_zoom = ctx.zoom_factor();
            }
            self.load_skin(ctx);
            // Lo que rige en este frame: la escala y las fuentes nuevas valen desde el siguiente
            // (al arrancar con Pixel UI, aunque la pantalla ya esté a esa escala).
            self.scale_ready =
                self.applied == Some(state) && ctx.pixels_per_point() == state.1 as f32;
            // No hace nada si ya es esa escala; egui la traduce a zoom = escala / la del sistema.
            ctx.set_pixels_per_point(state.1 as f32);
        } else {
            self.scale_ready = false;
        }
        // Los tooltips de este frame: el globo del skin con Pixel UI; sin él, los de egui.
        crate::pixel_ui::set_bubble(ctx, self.skin.as_ref().filter(|_| view.pixel_ui));
        if self.applied == Some(state) && !std::mem::take(&mut self.skin_changed) {
            return;
        }
        self.applied = Some(state);
        ctx.set_fonts(font_definitions(view.pixel_ui));
        // Las fuentes nuevas rigen desde el frame siguiente: se pide ya, sin esperar un evento.
        ctx.request_repaint();
        let colors = self.skin.as_ref().map(|skin| *skin.colors());
        apply_style(ctx, colors.as_ref().filter(|_| view.pixel_ui));
        // El Ctrl+/- de egui rompería la escala entera. En la web viene apagado (lo maneja el
        // navegador).
        ctx.options_mut(|o| {
            o.zoom_with_keyboard = !view.pixel_ui && !cfg!(target_arch = "wasm32");
        });
        if was_on && !view.pixel_ui {
            // Devuelve el zoom que tenía el usuario.
            ctx.set_zoom_factor(self.prev_zoom);
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
                    Ok(skin) => {
                        self.skin = Some(skin);
                        // Sus colores se vuelven a aplicar a egui en `update`.
                        self.skin_changed = true;
                    }
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

    #[test]
    fn the_pixel_font_knows_its_glyphs() {
        assert!(pixel_font_has('F'));
        assert!(pixel_font_has('.'));
        // Sin "…": egui lo toma de su fuente de respaldo, que no es pixel art.
        assert!(!pixel_font_has('…'));
    }

    #[test]
    fn the_colon_sits_on_the_baseline_and_only_the_semicolon_has_a_tail() {
        let font = fontdue::Font::from_bytes(PIXEL_FONT, fontdue::FontSettings::default()).unwrap();
        // `ymin` es el borde de abajo del glifo respecto a la línea base (negativo: baja de ella).
        let bottom = |c: char| font.metrics(c, 16.0).ymin;
        assert!(bottom(':') >= 0, "':' no baja de la línea base como ';'");
        assert!(bottom(';') < 0, "';' lleva su colita");
    }

    fn frame(ctx: &egui::Context, pixel: &mut PixelUi, view: &ViewSettings) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| pixel.update(ui.ctx(), view));
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
    }

    #[test]
    fn the_pixel_font_goes_first_only_in_the_proportional_family() {
        let plain = font_definitions(false);
        // Sin Pixel UI, las de egui y la familia de los textos chicos con las proporcionales.
        let mut expected = FontDefinitions::default();
        let proportional = expected.families[&FontFamily::Proportional].clone();
        expected.families.insert(small_family(), proportional);
        assert_eq!(plain, expected);
        let pixel = font_definitions(true);
        let proportional = &pixel.families[&FontFamily::Proportional];
        assert_eq!(proportional[0], PIXEL_FONT_NAME);
        // Los emojis de Unifont, antes que las de egui.
        assert_eq!(
            proportional[1..3],
            [UNIFONT_SYMBOLS_NAME, UNIFONT_EMOJI_NAME]
        );
        assert!(proportional.len() > 1, "egui fonts stay as fallback");
        // Los textos chicos: chimpfont primero y la proporcional detrás.
        let small = &pixel.families[&small_family()];
        assert_eq!(small[0], SMALL_FONT_NAME);
        assert_eq!(&small[1..], proportional.as_slice());
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
        assert!(pixel.skin().is_none(), "the scale is not in effect yet");
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

    #[test]
    fn with_pixel_ui_the_egui_colors_come_from_the_skin_and_off_they_are_the_theme_again() {
        let ctx = egui::Context::default();
        let mut pixel = PixelUi::from_env();
        let mut view = ViewSettings {
            pixel_ui: true,
            ui_scale: 2,
            ..Default::default()
        };
        frame(&ctx, &mut pixel, &view);
        frame(&ctx, &mut pixel, &view);
        let colors = *pixel.skin().expect("on").colors();
        assert_eq!(colors.background, Color32::from_rgb(221, 206, 191));
        let visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
        assert_eq!(visuals.panel_fill, colors.background);
        assert_eq!(visuals.override_text_color, Some(colors.text));
        assert_eq!(visuals.weak_text_color, Some(colors.text_dim));
        assert_eq!(visuals.window_fill, colors.panel);
        assert_eq!(visuals.extreme_bg_color, colors.panel);
        assert_eq!(visuals.selection.bg_fill, colors.selection);
        assert_eq!(visuals.widgets.inactive.bg_stroke.color, colors.outline);
        assert_eq!(visuals.window_shadow, Shadow::NONE);
        assert!(!visuals.dark_mode);
        assert_eq!(visuals.widgets.inactive.corner_radius, CornerRadius::ZERO);
        assert!(
            !ctx.style_of(egui::Theme::Dark)
                .spacing
                .scroll
                .foreground_color
        );

        view.pixel_ui = false;
        frame(&ctx, &mut pixel, &view);
        let plain = egui::Context::default();
        theme::apply(&plain);
        assert_eq!(
            ctx.style_of(egui::Theme::Dark).visuals,
            plain.style_of(egui::Theme::Dark).visuals,
            "the dark theme of always"
        );
        assert!(
            ctx.style_of(egui::Theme::Dark)
                .spacing
                .scroll
                .foreground_color
        );
    }

    /// Un frame con la pantalla a `native` píxeles por punto: si egui pidió otro enseguida.
    fn frame_on(
        ctx: &egui::Context,
        pixel: &mut PixelUi,
        view: &ViewSettings,
        native: f32,
    ) -> bool {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(native);
        let mut output = ctx.run_ui(input, |ui| pixel.update(ui.ctx(), view));
        output.textures_delta.clear();
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay == std::time::Duration::ZERO
    }

    /// La fuente pixel ya rige en `ctx`.
    fn pixel_font_active(ctx: &egui::Context) -> bool {
        ctx.fonts(|f| f.definitions().font_data.contains_key(PIXEL_FONT_NAME))
    }

    #[test]
    fn at_startup_the_skin_waits_for_the_pixel_font_and_the_scale_and_asks_for_the_frame() {
        // La app abre con Pixel UI, con la pantalla a 1× (la escala cambia) o ya a 2× (no
        // cambia): en los dos casos el primer frame es sin skin (la fuente pixel y la escala
        // valen desde el siguiente) y pide el siguiente enseguida, sin esperar un evento.
        for native in [1.0, 2.0] {
            let ctx = egui::Context::default();
            let mut pixel = PixelUi::from_env();
            let view = ViewSettings::default();
            assert!(view.pixel_ui, "on by default");
            let again = frame_on(&ctx, &mut pixel, &view, native);
            assert!(pixel.skin().is_none(), "{native}: not on the first frame");
            assert!(again, "{native}: asks for the second frame");
            frame_on(&ctx, &mut pixel, &view, native);
            assert_eq!(ctx.pixels_per_point(), 2.0, "{native}");
            assert!(pixel_font_active(&ctx), "{native}");
            assert!(pixel.skin().is_some(), "{native}: on from the second frame");
        }
    }

    #[test]
    fn switching_off_gives_back_the_users_zoom() {
        let ctx = egui::Context::default();
        let mut pixel = PixelUi::from_env();
        ctx.set_zoom_factor(1.25);
        let mut view = ViewSettings {
            pixel_ui: true,
            ui_scale: 3,
            ..Default::default()
        };
        for _ in 0..3 {
            frame(&ctx, &mut pixel, &view);
        }
        view.pixel_ui = false;
        for _ in 0..3 {
            frame(&ctx, &mut pixel, &view);
        }
        assert_eq!(ctx.zoom_factor(), 1.25);
    }
}
