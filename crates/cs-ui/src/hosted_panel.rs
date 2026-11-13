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

/// `events` de un arnés, con las posiciones del puntero relativas a la hoja `leaf`, en la ventana.
pub(crate) fn to_window(events: Vec<egui::Event>, leaf: egui::Rect) -> Vec<egui::Event> {
    let offset = leaf.min.to_vec2();
    events
        .into_iter()
        .map(|e| match e {
            egui::Event::PointerMoved(p) => egui::Event::PointerMoved(p + offset),
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            } => egui::Event::PointerButton {
                pos: pos + offset,
                button,
                pressed,
                modifiers,
            },
            e => e,
        })
        .collect()
}

/// Los nodos con nombre (o, si no, con valor) de `output` que son del panel de la hoja `leaf`,
/// en orden, con su rect relativo a la hoja. Cuentan los que se cruzan con la hoja (con área:
/// tocar el borde, como la barra de opciones justo encima, no cuenta) y los de lo que va encima de
/// los paneles (un menú emergente, un combo abierto o una ventana: la capa que hay en el centro
/// del nodo, `Context::layer_id_at`, no es la del fondo), que pueden salir de la hoja. Los de las
/// barras de la ventana (la de opciones de la herramienta también tiene "Opacity" y "1"), del
/// lienzo y de la tira no: están en la capa del fondo, fuera de la hoja.
pub(crate) fn panel_nodes<'a>(
    ctx: &egui::Context,
    output: &'a egui::FullOutput,
    leaf: egui::Rect,
) -> Vec<(String, egui::Rect, &'a egui::accesskit::Node)> {
    let Some(update) = &output.platform_output.accesskit_update else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter_map(|(_, n)| {
            let b = n.bounds()?;
            let text = n.label().or_else(|| n.value())?.to_string();
            let rect = egui::Rect::from_min_max(
                egui::pos2(b.x0 as f32, b.y0 as f32),
                egui::pos2(b.x1 as f32, b.y1 as f32),
            );
            let popup = ctx
                .layer_id_at(rect.center())
                .is_some_and(|l| l.order != egui::Order::Background);
            let ours = clipped_to(rect, leaf) || popup;
            ours.then(|| (text, rect.translate(-leaf.min.to_vec2()), n))
        })
        .collect()
}

/// Si lo dibujado con recorte `clip` cae en `leaf` (con área: tocar el borde no cuenta). Lo del
/// panel y sus menús emergentes (recortados a la ventana) sí; las barras de arriba, la de estado
/// y el lienzo, no.
pub(crate) fn clipped_to(clip: egui::Rect, leaf: egui::Rect) -> bool {
    let both = clip.intersect(leaf);
    both.width() > 0.0 && both.height() > 0.0
}
