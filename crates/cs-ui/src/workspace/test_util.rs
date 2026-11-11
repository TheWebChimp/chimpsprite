//! Lo que comparten los tests del workspace para dibujarlo en un frame sin ventana.

use eframe::egui;

use super::show::show;
use crate::ChimpApp;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::view::ViewSettings;

/// La app con Pixel UI y una pestaña nueva.
pub(super) fn test_app() -> ChimpApp {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
    app.new_document();
    app
}

/// Un frame con el workspace en el panel central, con `events`.
pub(super) fn frame(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    frame_in(ctx, app, events, egui::vec2(1280.0, 800.0))
}

/// Como `frame`, en una ventana de `size`.
pub(super) fn frame_in(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    events: Vec<egui::Event>,
    size: egui::Vec2,
) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        app.pixel.update(ui.ctx(), &app.view);
        egui::CentralPanel::default().show(ui, |ui| show(app, ui, false));
    });
    output.textures_delta.clear();
    output
}

/// Unos frames (la escala de Pixel UI rige desde el segundo); la salida del último.
pub(super) fn frames(ctx: &egui::Context, app: &mut ChimpApp) -> egui::FullOutput {
    let mut out = frame(ctx, app, Vec::new());
    for _ in 0..3 {
        out = frame(ctx, app, Vec::new());
    }
    out
}

/// El botón con la etiqueta `label` en `output`.
pub(super) fn button(output: &egui::FullOutput, label: &str) -> Option<egui::accesskit::NodeId> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .find(|(_, n)| n.role() == egui::accesskit::Role::Button && n.label() == Some(label))
        .map(|(id, _)| *id)
}

pub(super) fn click(target: egui::accesskit::NodeId) -> egui::Event {
    egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::Click,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: target,
        data: None,
    })
}
