//! Lo que comparten los tests del workspace para dibujarlo en un frame sin ventana.

use eframe::egui;
use egui_dock::NodePath;

use super::show::show;
use super::{DockTarget, skin};
use crate::ChimpApp;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::panel_registry::PanelId;
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
    frame_with(ctx, app, events, size, |_| {})
}

/// Como `frame_in`, y después del workspace `after` (lo que la app dibuja encima: un diálogo).
pub(super) fn frame_with(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    events: Vec<egui::Event>,
    size: egui::Vec2,
    mut after: impl FnMut(&egui::Context),
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
        if let Some(area) = area
            && let Some(request) = area.dock.request
        {
            app.workspace.apply_strip_request(request, &area.strips);
        }
        after(ui.ctx());
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

/// La ventana de los tests del modal, de las pestañas y de las tiras.
pub(super) const SIZE: egui::Vec2 = egui::vec2(1280.0, 800.0);

/// Lo que se ve del acomodo: cada grupo (su superficie, sus pestañas, la activa y su rect), el
/// rect de cada ventana flotante, las tiras, el ancho de la columna, el flyout abierto y si
/// hay un menú abierto.
pub(super) fn snapshot(ctx: &egui::Context, app: &ChimpApp) -> String {
    let w = &app.workspace;
    let leaves: Vec<_> = w
        .state
        .iter_leaves()
        .map(|(path, leaf)| (path.surface, leaf.tabs.clone(), leaf.active.0, leaf.rect))
        .collect();
    let windows: Vec<_> = PanelId::ALL
        .iter()
        .filter(|&&p| w.floating(p))
        .map(|&p| (p, w.window_rect(ctx, p)))
        .collect();
    let menu = egui::Popup::is_any_open(ctx);
    format!(
        "{leaves:?}\n{windows:?}\n{:?}\n{:?} {:?} menu {menu}",
        w.strips,
        app.view.palette_width,
        crate::dock::open_panel(ctx),
    )
}

pub(super) fn leaf(app: &ChimpApp, id: PanelId) -> egui::Rect {
    app.workspace
        .leaf_rect(id)
        .unwrap_or_else(|| panic!("{id:?} in the tree"))
}

/// El nombre del ícono de `id` en la tira: su título con Pixel UI; sin él, lo que dice el
/// botón de egui.
pub(super) fn icon_label(app: &ChimpApp, id: PanelId) -> &'static str {
    if app.pixel.skin().is_some() {
        id.spec().title
    } else {
        id.spec().plain
    }
}

/// La × de la pestaña `id` (con Pixel UI, la del kit; sin él, la de egui_dock, al final de
/// su pestaña).
pub(super) fn close_tab(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    out: &egui::FullOutput,
    size: egui::Vec2,
    id: PanelId,
) {
    let title = id.spec().title;
    let at = if app.pixel.skin().is_some() {
        node_rect(out, &format!("Close {title}")).unwrap().center()
    } else {
        let tab = node_rect(out, title).unwrap();
        egui::pos2(tab.right() - 12.0, tab.center().y)
    };
    click_at(ctx, app, at, egui::PointerButton::Primary, size);
}

/// Library con siete paneles más en su grupo, debajo de Color en la columna (Library al
/// frente): sus pestañas no caben y su barra queda a media altura (algo puede ir encima).
pub(super) fn many_tabs_below_color(app: &mut ChimpApp) {
    let (s, n) = app.workspace.leaf_of(PanelId::Color).unwrap();
    let below = DockTarget::Split(s, n, egui_dock::Split::Below);
    app.workspace.dock(PanelId::Library, below);
    let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
    for id in [
        PanelId::Slices,
        PanelId::Artboards,
        PanelId::Lospec,
        PanelId::Groom,
        PanelId::Lumen,
        PanelId::Model,
        PanelId::Font,
    ] {
        app.workspace.dock(id, DockTarget::Leaf(s, n));
    }
    let (s, n) = app.workspace.leaf_of(PanelId::Library).unwrap();
    let path = NodePath {
        surface: s,
        node: n,
    };
    app.workspace.state.leaf_mut(path).unwrap().active = egui_dock::TabIndex(0);
}

/// Desde cuál pestaña se ve la barra del grupo de `id` (`None` sin Pixel UI: no hay flechas).
pub(super) fn tabs_offset(ctx: &egui::Context, app: &ChimpApp, id: PanelId) -> Option<usize> {
    let (surface, node) = app.workspace.leaf_of(id)?;
    let path = NodePath { surface, node };
    ctx.data(|d| d.get_temp::<(usize, Option<PanelId>)>(skin::tabs_offset_id(path)))
        .map(|(o, _)| o)
}

/// Un evento de la rueda (`delta` hacia arriba si es positivo) en `unit`, con `modifiers`.
pub(super) fn wheel(
    unit: egui::MouseWheelUnit,
    dy: f32,
    modifiers: egui::Modifiers,
) -> egui::Event {
    egui::Event::MouseWheel {
        unit,
        delta: egui::vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        modifiers,
    }
}
