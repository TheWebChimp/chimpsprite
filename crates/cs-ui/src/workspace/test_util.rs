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
        // Como `editor_panels`: el frame de los paneles empieza de cero. Con un diálogo abierto
        // (aquí no se dibuja), como en la app: modal.
        app.begin_panel_frame();
        let modal = app.is_modal();
        let area = egui::CentralPanel::default()
            .show(ui, |ui| show(app, ui, modal))
            .inner;
        // Lo que pidió una tira (como `editor_panels`): soltar, acoplar o flotar un ícono.
        if let Some(request) = area.and_then(|a| a.dock.request) {
            match request {
                crate::dock::StripRequest::Drop(id, pointer) => {
                    let target = super::drop_target(&app.workspace, pointer);
                    app.workspace.drop_panel(id, target);
                }
                crate::dock::StripRequest::Dock(id) => app.workspace.dock_from_strip(id),
                crate::dock::StripRequest::Float(id, at) => app.workspace.float(id, at),
            }
        }
    });
    output.textures_delta.clear();
    output
}

/// Como `frames`, en una ventana de `size`.
pub(super) fn frames_in(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    size: egui::Vec2,
) -> egui::FullOutput {
    let mut out = frame_in(ctx, app, Vec::new(), size);
    for _ in 0..3 {
        out = frame_in(ctx, app, Vec::new(), size);
    }
    out
}

/// El rect (el que ven los lectores de pantalla) del nodo con la etiqueta `label` en `output`.
pub(super) fn node_rect(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .filter(|(_, n)| n.label() == Some(label))
        .find_map(|(_, n)| {
            let b = n.bounds()?;
            Some(egui::Rect::from_min_max(
                egui::pos2(b.x0 as f32, b.y0 as f32),
                egui::pos2(b.x1 as f32, b.y1 as f32),
            ))
        })
}

/// Las etiquetas de los botones de `output`.
pub(super) fn buttons(output: &egui::FullOutput) -> Vec<String> {
    let Some(update) = output.platform_output.accesskit_update.as_ref() else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
        .filter_map(|(_, n)| Some(n.label()?.to_string()))
        .collect()
}

fn press(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    }
}

/// Un clic con `button` en `at` (en una ventana de `size`): mover, apretar y soltar, un frame
/// cada uno, y uno más. Devuelve la salida del último.
pub(super) fn click_at(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    at: egui::Pos2,
    button: egui::PointerButton,
    size: egui::Vec2,
) -> egui::FullOutput {
    frame_in(ctx, app, vec![egui::Event::PointerMoved(at)], size);
    frame_in(ctx, app, vec![press(at, button, true)], size);
    frame_in(ctx, app, vec![press(at, button, false)], size);
    frame_in(ctx, app, Vec::new(), size)
}

/// Un arrastre con el botón principal de `from` a `to` (en una ventana de `size`): mover,
/// apretar, ir hasta `to` en cuatro pasos y soltar, un frame cada uno, y unos frames más.
pub(super) fn drag_to(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    (from, to): (egui::Pos2, egui::Pos2),
    size: egui::Vec2,
) {
    let primary = egui::PointerButton::Primary;
    frame_in(ctx, app, vec![egui::Event::PointerMoved(from)], size);
    frame_in(ctx, app, vec![press(from, primary, true)], size);
    for k in 1..=4 {
        let at = from + (to - from) * (k as f32 / 4.0);
        frame_in(ctx, app, vec![egui::Event::PointerMoved(at)], size);
    }
    frame_in(ctx, app, vec![press(to, primary, false)], size);
    for _ in 0..3 {
        frame_in(ctx, app, Vec::new(), size);
    }
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
