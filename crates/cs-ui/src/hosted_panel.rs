//! Lo que comparten los tests de un panel que lo miden como lo hospeda la app (el timeline,
//! Color): la app entera con el panel solo junto al lienzo (`Workspace::solo`), dibujado por el
//! workspace con `ChimpApp::draw_panel`, manejada con eventos crudos.

use eframe::egui;

use crate::ChimpApp;
use crate::dock::Edge;
use crate::editor::Editor;
use crate::file_actions::DocLocation;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::panel_registry::PanelId;
use crate::tabs::DocState;
use crate::view::ViewSettings;
use crate::workspace::Workspace;

/// La app con `view`, una pestaña con `editor` y el acomodo `Workspace::solo(id, edge)`. El
/// timeline y el Preview se ven solo si son `id` (como `LayoutHarness::with_workspace`: si no,
/// `show` volvería a poner el timeline debajo del lienzo).
pub(crate) fn app(view: ViewSettings, editor: Editor, id: PanelId, edge: Edge) -> ChimpApp {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
    let location = DocLocation {
        name: "Test".into(),
        target: None,
    };
    app.tabs.open(&mut app.doc, DocState::new(editor, location));
    let w = Workspace::solo(id, edge);
    let tabs = w.state_tabs();
    app.view.timeline = tabs.contains(&PanelId::Timeline);
    app.view.preview = tabs.contains(&PanelId::Preview);
    app.workspace = w;
    app
}

/// Un frame de la app con `input`, como `App::ui` (y `LayoutHarness::frame_with`): Pixel UI, los
/// atajos antes de los paneles, los paneles y la ventana de sombreado. Las texturas (fuentes,
/// skin y lienzo) no se suben a ninguna GPU.
pub(crate) fn frame(
    ctx: &egui::Context,
    app: &mut ChimpApp,
    input: egui::RawInput,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input, |ui| {
        let ctx = ui.ctx().clone();
        app.pixel.update(&ctx, &app.view);
        app.handle_shortcuts(&ctx);
        app.layout(ui, false);
        app.update_shading_window(&ctx, false);
    });
    output.textures_delta.clear();
    output
}

/// La hoja de `id` en el último frame (`Workspace::leaf_rect`).
pub(crate) fn leaf(app: &ChimpApp, id: PanelId) -> egui::Rect {
    app.workspace
        .leaf_rect(id)
        .unwrap_or_else(|| panic!("{id:?} is not in the workspace"))
}

/// Si lo dibujado con recorte `clip` cae en `leaf` (con área: tocar el borde no cuenta). Lo del
/// panel y sus menús emergentes (recortados a la ventana) sí; las barras de arriba, la de estado
/// y el lienzo, no.
pub(crate) fn clipped_to(clip: egui::Rect, leaf: egui::Rect) -> bool {
    let both = clip.intersect(leaf);
    both.width() > 0.0 && both.height() > 0.0
}
