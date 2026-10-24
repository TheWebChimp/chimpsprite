//! Acomodo de los paneles de la ventana: la app entera, con una pestaña abierta, manejada con
//! eventos crudos. Los rects de los paneles salen del `PanelState` que egui guarda de cada uno.

use eframe::egui::{self, Pos2, RawInput, Rect, Vec2, containers::panel::PanelState};

use crate::ChimpApp;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::view::ViewSettings;

struct LayoutHarness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    /// Rect del lienzo en el último frame (`None` en Home).
    canvas: Option<Rect>,
}

impl LayoutHarness {
    /// La app con `view`, una pestaña nueva si `doc`, y tres frames: con Pixel UI la escala
    /// entera rige desde el segundo.
    fn new(view: ViewSettings, doc: bool) -> Self {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
        if doc {
            app.new_document();
        }
        let mut h = Self {
            ctx: egui::Context::default(),
            app,
            time: 0.0,
            canvas: None,
        };
        for _ in 0..3 {
            h.frame();
        }
        h
    }

    /// El editor con los paneles de siempre (línea de tiempo visible).
    fn editor() -> Self {
        Self::new(ViewSettings::default(), true)
    }

    fn frame(&mut self) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(self.time),
            ..Default::default()
        };
        let app = &mut self.app;
        let mut canvas = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.pixel.update(&ctx, &app.view);
            canvas = app.layout(ui, false);
        });
        // Las texturas (fuentes, skin y lienzo) no se suben a ninguna GPU.
        output.textures_delta.clear();
        self.canvas = canvas;
    }

    /// Rect exterior del panel `id` (con su margen y la línea que lo separa del resto).
    fn panel(&self, id: &str) -> Rect {
        PanelState::load(&self.ctx, egui::Id::new(id))
            .unwrap_or_else(|| panic!("no panel {id:?}"))
            .outer_rect
    }

    fn screen(&self) -> Rect {
        self.ctx.content_rect()
    }
}

#[test]
fn the_toolbar_goes_down_beside_the_timeline() {
    let h = LayoutHarness::editor();
    let (tools, timeline) = (h.panel("tools"), h.panel("timeline"));
    assert!(
        tools.bottom() > timeline.top(),
        "full height: {tools:?} vs {timeline:?}"
    );
    assert!(
        tools.right() <= timeline.left(),
        "the timeline starts to its right: {tools:?} vs {timeline:?}"
    );
    assert_eq!(
        tools.bottom(),
        timeline.bottom(),
        "both end on the status bar"
    );
}

#[test]
fn the_options_bar_is_between_the_tabs_and_the_canvas() {
    let h = LayoutHarness::editor();
    let (tabs, options) = (h.panel("tabs"), h.panel("tool_options"));
    let canvas = h.canvas.expect("a document is open");
    assert!(tabs.bottom() <= options.top(), "{tabs:?} vs {options:?}");
    assert!(
        options.bottom() <= canvas.top(),
        "{options:?} vs {canvas:?}"
    );
    assert!(
        options.bottom() <= h.panel("tools").top(),
        "the toolbar starts under it"
    );
    assert_eq!(options.width(), h.screen().width(), "full width");
}

#[test]
fn the_status_bar_spans_the_window_under_everything() {
    let h = LayoutHarness::editor();
    let status = h.panel("status");
    assert_eq!(status.x_range(), h.screen().x_range());
    assert_eq!(status.bottom(), h.screen().bottom());
    for id in ["tools", "timeline", "palette"] {
        assert!(h.panel(id).bottom() <= status.top(), "{id}");
    }
}

#[test]
fn the_right_panel_keeps_ending_above_the_timeline() {
    let h = LayoutHarness::editor();
    let (palette, timeline) = (h.panel("palette"), h.panel("timeline"));
    assert!(
        palette.bottom() <= timeline.top(),
        "{palette:?} vs {timeline:?}"
    );
    // La fila de controles de la línea de tiempo no se parte: si es más ancha que el panel, el
    // rect del panel pasa del borde (egui recorta lo que sobra).
    assert!(
        timeline.right() >= h.screen().right(),
        "under the right panel too: {timeline:?}"
    );
}

#[test]
fn on_home_there_is_no_options_bar() {
    let h = LayoutHarness::new(ViewSettings::default(), false);
    assert!(h.canvas.is_none());
    assert!(PanelState::load(&h.ctx, egui::Id::new("tool_options")).is_none());
    assert!(PanelState::load(&h.ctx, egui::Id::new("tools")).is_none());
}
