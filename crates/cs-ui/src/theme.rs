use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const ACCENT: Color32 = Color32::from_rgb(0xf2, 0x8c, 0x28);
pub const CANVAS_BG: Color32 = Color32::from_rgb(0x1b, 0x1c, 0x22);
pub const CANVAS_BORDER: Color32 = Color32::from_rgb(0x0e, 0x0e, 0x12);
pub const CHECKER_LIGHT: Color32 = Color32::from_rgb(0x9a, 0x9a, 0xa2);
pub const CHECKER_DARK: Color32 = Color32::from_rgb(0x7a, 0x7a, 0x82);
pub const GRID: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 40);
pub const GUIDE: Color32 = Color32::from_rgb(0x4a, 0x7d, 0xff);
pub const GUIDE_LINE: Color32 = Color32::from_rgb(0x2e, 0xd3, 0xe6);
pub const RULER_BG: Color32 = Color32::from_rgb(0x22, 0x23, 0x2a);
pub const RULER_TICK: Color32 = Color32::from_rgb(0x6a, 0x6b, 0x78);
pub const LABEL: Color32 = Color32::from_rgb(0x9a, 0x9b, 0xa8);
pub const BRUSH_OUTLINE: Color32 = Color32::from_rgba_premultiplied(230, 230, 230, 230);
/// Grilla de tiles de la capa tilemap activa (naranja translúcido, distinto del grid de trabajo).
pub const TILE_GRID: Color32 = Color32::from_rgba_premultiplied(140, 77, 22, 140);

const PANEL: Color32 = Color32::from_rgb(0x26, 0x27, 0x2f);
const WIDGET: Color32 = Color32::from_rgb(0x33, 0x34, 0x3e);
const WIDGET_HOVER: Color32 = Color32::from_rgb(0x40, 0x42, 0x4e);

/// Tema oscuro de ChimpSprite.
pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL;
    visuals.extreme_bg_color = CANVAS_BG;
    visuals.selection.bg_fill = ACCENT.linear_multiply(0.6);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    let radius = CornerRadius::same(3);
    for w in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = radius;
    }
    visuals.widgets.inactive.weak_bg_fill = WIDGET;
    visuals.widgets.inactive.bg_fill = WIDGET;
    visuals.widgets.hovered.weak_bg_fill = WIDGET_HOVER;
    visuals.widgets.hovered.bg_fill = WIDGET_HOVER;
    visuals.widgets.active.weak_bg_fill = ACCENT.linear_multiply(0.8);
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);
}
