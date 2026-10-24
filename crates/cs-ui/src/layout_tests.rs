//! Acomodo de los paneles de la ventana: la app entera, con una pestaña abierta, manejada con
//! eventos crudos. Los rects de los paneles salen del `PanelState` que egui guarda de cada uno.

use cs_commands::Command;
use cs_core::{LayerId, PixelValue, Rgba};
use eframe::egui::{
    self, Event, PointerButton, Pos2, RawInput, Rect, TextureId, Vec2,
    containers::panel::PanelState,
};

use crate::ChimpApp;
use crate::file_actions::Pending;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::skin::WidgetState;
use crate::tabs::DocState;
use crate::view::ViewSettings;

struct LayoutHarness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    /// Rect del lienzo en el último frame (`None` en Home).
    canvas: Option<Rect>,
    /// Botones del último frame: nombre y rect (en puntos).
    buttons: Vec<(String, Rect)>,
    /// Lo dibujado en el último frame.
    shapes: Vec<egui::epaint::ClippedShape>,
    /// Las mallas con textura del último frame (piezas del skin): textura y rect.
    textures: Vec<(TextureId, Rect)>,
    /// Los textos del último frame y dónde empiezan.
    texts: Vec<(String, Pos2)>,
    /// Los textos del último frame con su rect y su color.
    text_rects: Vec<(String, Rect, egui::Color32)>,
    /// Tamaño de la ventana.
    size: Vec2,
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
        Self::start(app)
    }

    /// Con Pixel UI y una pestaña por cada `(nombre, con cambios)` de `docs`; activa la `active`
    /// (`None`: Home).
    fn with_tabs(docs: &[(&str, bool)], active: Option<usize>) -> Self {
        Self::with_tabs_in(docs, active, Vec2::new(1200.0, 800.0))
    }

    /// Como `with_tabs`, en una ventana de `size`.
    fn with_tabs_in(docs: &[(&str, bool)], active: Option<usize>, size: Vec2) -> Self {
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
        for &(name, dirty) in docs {
            app.tabs.open(&mut app.doc, touched(name, dirty));
        }
        match active {
            Some(i) => app.tabs.switch(&mut app.doc, i),
            None => app.go_home(),
        }
        Self::start_in(app, size)
    }

    fn start(app: ChimpApp) -> Self {
        Self::start_in(app, Vec2::new(1200.0, 800.0))
    }

    fn start_in(app: ChimpApp, size: Vec2) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Self {
            ctx,
            app,
            time: 0.0,
            canvas: None,
            buttons: Vec::new(),
            shapes: Vec::new(),
            textures: Vec::new(),
            texts: Vec::new(),
            text_rects: Vec::new(),
            size,
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
        self.frame_with(Vec::new());
    }

    fn frame_with(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.time),
            events,
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
        self.shapes = std::mem::take(&mut output.shapes);
        self.textures.clear();
        self.texts.clear();
        self.text_rects.clear();
        for clipped in &self.shapes {
            collect(&clipped.shape, &mut self.textures, &mut self.texts);
            collect_text_rects(&clipped.shape, &mut self.text_rects);
        }
        self.buttons = output
            .platform_output
            .accesskit_update
            .map(|update| {
                update
                    .nodes
                    .iter()
                    .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                    .filter_map(|(_, n)| {
                        let b = n.bounds()?;
                        let rect = Rect::from_min_max(
                            Pos2::new(b.x0 as f32, b.y0 as f32),
                            Pos2::new(b.x1 as f32, b.y1 as f32),
                        );
                        Some((n.label()?.to_string(), rect))
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    /// Clic con `button` en `pos`: mover, apretar y soltar, un frame cada uno.
    fn click(&mut self, pos: Pos2, button: PointerButton) {
        let press = |pressed| Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        self.frame_with(vec![Event::PointerMoved(pos)]);
        self.frame_with(vec![press(true)]);
        self.frame_with(vec![press(false)]);
        self.frame();
    }

    /// La textura de la pieza `name` del skin.
    fn piece(&self, name: &str) -> TextureId {
        let skin = self.app.pixel.skin().expect("Pixel UI is on");
        let piece = skin.piece(name, WidgetState::Normal);
        piece.unwrap_or_else(|| panic!("{name}")).texture.id()
    }

    /// Los rects (uno por malla) de lo dibujado con la pieza `name`.
    fn drawn(&self, name: &str) -> Vec<Rect> {
        let id = self.piece(name);
        self.textures
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| *r)
            .collect()
    }

    fn button(&self, label: &str) -> Rect {
        self.buttons
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
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

    /// Los rects de color liso dibujados (los separadores grabados).
    fn colored_rects(&self) -> Vec<(egui::Color32, Rect)> {
        self.shapes
            .iter()
            .flat_map(|c| crate::pixel_ui::colored_rects(&c.shape))
            .collect()
    }

    /// Las líneas de egui del color de sus separadores que tocan `area`.
    fn separator_lines_in(&self, area: Rect) -> Vec<Rect> {
        let color = self
            .ctx
            .global_style()
            .visuals
            .widgets
            .noninteractive
            .bg_stroke
            .color;
        fn collect(shape: &egui::Shape, color: egui::Color32, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::LineSegment { points, stroke } if stroke.color == color => {
                    out.push(Rect::from_two_pos(points[0], points[1]));
                }
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, color, out)),
                _ => {}
            }
        }
        let mut lines = Vec::new();
        for c in &self.shapes {
            collect(&c.shape, color, &mut lines);
        }
        lines.retain(|r| r.expand(0.5).intersects(area));
        lines
    }
}

/// Junta las mallas con textura y los textos de `shape`.
fn collect(
    shape: &egui::Shape,
    textures: &mut Vec<(TextureId, Rect)>,
    texts: &mut Vec<(String, Pos2)>,
) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, textures, texts)),
        egui::Shape::Mesh(mesh) if mesh.texture_id != TextureId::default() => {
            textures.push((mesh.texture_id, mesh.calc_bounds()));
        }
        egui::Shape::Text(text) => texts.push((text.galley.text().to_owned(), text.pos)),
        _ => {}
    }
}

/// Los textos de `shape` con su rect (el del galley) y su color.
fn collect_text_rects(shape: &egui::Shape, out: &mut Vec<(String, Rect, egui::Color32)>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_text_rects(s, out)),
        egui::Shape::Text(text) => {
            let rect = Rect::from_min_size(text.pos, text.galley.size());
            let color = text.override_text_color.unwrap_or(text.fallback_color);
            out.push((text.galley.text().to_owned(), rect, color));
        }
        _ => {}
    }
}

/// Un documento `name` con un trazo; sin cambios (`dirty` falso) si se guardó después.
fn touched(name: &str, dirty: bool) -> DocState {
    let mut doc = DocState::untitled(name);
    doc.editor.execute(Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
    });
    if !dirty {
        doc.saved_version = doc.editor.doc_version;
    }
    doc
}

/// Solo las líneas verticales: las horizontales de la línea de tiempo y de la barra de estado
/// tocan el borde de la barra de íconos.
fn vertical(lines: Vec<Rect>) -> Vec<Rect> {
    lines.into_iter().filter(|r| r.width() == 0.0).collect()
}

fn pixel_ui() -> ViewSettings {
    ViewSettings {
        pixel_ui: true,
        ..Default::default()
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

#[test]
fn with_pixel_ui_the_toolbar_is_5_points_the_box_and_5_points_below_the_options_bar() {
    use crate::skin::WidgetState;
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    let skin = h.app.pixel.skin().expect("the integer scale is in effect");
    let frame = skin.piece("toolbar.box", WidgetState::Normal).unwrap().size[0] as f32;
    let tools = h.panel("tools");
    // El mockup: 5 px de fondo, la caja de 27 y 5 px, sin línea contra el lienzo.
    assert_eq!(tools.width(), 5.0 + frame + 5.0, "{tools:?}");
    assert_eq!(tools.width(), 37.0, "{tools:?}");
    // El primer botón, a 1 px de línea y 1 de aire dentro de la caja: a 7 puntos de cada borde.
    let marquee = h.button("Rectangular Marquee");
    assert_eq!(marquee.width(), 23.0);
    assert_eq!(
        marquee.left() - tools.left(),
        7.0,
        "{marquee:?} in {tools:?}"
    );
    assert_eq!(tools.right() - marquee.right(), 7.0);
    // La caja empieza 5 px debajo de la barra de opciones, igual que el aire a los lados.
    let options = h.panel("tool_options");
    assert_eq!(marquee.top() - 2.0 - options.bottom(), 5.0, "{marquee:?}");
    // Los botones siguen pegados.
    assert_eq!(h.button("Hand").top(), marquee.bottom());
    // Y sigue a todo el alto, al lado de la línea de tiempo.
    assert!(tools.bottom() > h.panel("timeline").top());
}

#[test]
fn with_pixel_ui_the_canvas_has_the_skin_canvas_color() {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    let canvas = h.app.pixel.skin().expect("on").colors().canvas;
    assert_eq!(h.app.doc.canvas.background, canvas);
    // Sin Pixel UI, el fondo del tema.
    let h = LayoutHarness::editor();
    assert_eq!(h.app.doc.canvas.background, crate::theme::CANVAS_BG);
}

#[test]
fn with_pixel_ui_the_options_bar_is_19_high() {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    assert_eq!(h.panel("tool_options").height(), 19.0);
    // Sin Pixel UI, la de siempre.
    assert_eq!(LayoutHarness::editor().panel("tool_options").height(), 32.0);
}

#[test]
fn without_pixel_ui_the_named_toolbar_keeps_its_width() {
    let h = LayoutHarness::editor();
    assert_eq!(h.panel("tools").width(), 184.0);
}

#[test]
fn with_pixel_ui_the_tabs_and_the_options_bar_end_in_etched_lines() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let colors = *h.app.pixel.skin().expect("on").colors();
    let rects = h.colored_rects();
    let (tabs, options) = (h.panel("tabs"), h.panel("tool_options"));
    assert_eq!(tabs.bottom(), options.top(), "the bar right under the tabs");
    assert_eq!(
        options.height(),
        19.0,
        "the mockup's 19: 17 rows and the etched line"
    );
    let width = h.screen().x_range();
    // Las dos líneas del mockup: debajo de las pestañas (y 69/70) y abajo de la barra (88/89).
    // La de las pestañas se corta debajo de la activa (la tapa su caja): va en dos tramos.
    let active = h.button("Untitled");
    for (panel, spans) in [
        (
            tabs,
            vec![width.min..=active.left(), active.right()..=width.max],
        ),
        (options, vec![width.min..=width.max]),
    ] {
        let y = panel.bottom() - 2.0;
        for x in spans {
            let dark = Rect::from_x_y_ranges(x, y..=y + 1.0);
            let light = dark.translate(Vec2::new(0.0, 1.0));
            assert!(rects.contains(&(colors.canvas, dark)), "{dark:?}");
            assert!(rects.contains(&(colors.highlight, light)), "{light:?}");
        }
    }
    // Sin las líneas de egui: ni entre el menú y las pestañas, ni debajo de ellas, ni en la
    // barra, ni contra la barra de íconos.
    // (La del panel derecho empieza justo debajo de la barra: queda fuera.)
    let top = Rect::from_x_y_ranges(width, h.screen().top()..=options.bottom() - 1.0);
    assert_eq!(h.separator_lines_in(top), vec![], "menu, tabs and bar");
    let tools = h.panel("tools");
    let edge = Rect::from_x_y_ranges(
        tools.right()..=tools.right(),
        tools.top() + 1.0..=tools.bottom() - 1.0,
    );
    assert_eq!(vertical(h.separator_lines_in(edge)), vec![], "toolbar edge");
}

#[test]
fn without_pixel_ui_the_panels_keep_the_egui_lines() {
    let h = LayoutHarness::editor();
    let thin: Vec<_> = h
        .colored_rects()
        .into_iter()
        .filter(|(_, r)| r.width() == 1.0 || r.height() == 1.0)
        .collect();
    assert_eq!(thin, vec![], "no etched lines");
    let options = h.panel("tool_options");
    let top = Rect::from_x_y_ranges(h.screen().x_range(), h.screen().top()..=options.bottom());
    assert!(h.separator_lines_in(top).len() >= 3, "menu, tabs and bar");
    let tools = h.panel("tools");
    let edge = Rect::from_x_y_ranges(
        tools.right()..=tools.right(),
        tools.top() + 1.0..=tools.bottom() - 1.0,
    );
    assert!(
        !vertical(h.separator_lines_in(edge)).is_empty(),
        "toolbar edge"
    );
}

// Pestañas con Pixel UI (el mockup: `.design/Design.aseprite`, frame 3).

/// Las pestañas `Design.chimp` (con cambios) y `Otro.chimp` (sin cambios), con `active` activa.
fn two_tabs(active: Option<usize>) -> LayoutHarness {
    LayoutHarness::with_tabs(&[("Design.chimp", true), ("Otro.chimp", false)], active)
}

/// La unión de los rects.
fn union(rects: &[Rect]) -> Option<Rect> {
    rects.iter().copied().reduce(|a, b| a.union(b))
}

#[test]
fn with_pixel_ui_the_menu_is_14_high_and_the_tabs_23_like_the_mockup() {
    let h = two_tabs(Some(0));
    // El mockup: el menú de la fila 34 a la 47 y las pestañas de la 48 a la 70 (con el grabado
    // en la 69 y la 70); la barra de opciones empieza en la 71.
    assert_eq!(h.panel("menu").height(), 14.0);
    assert_eq!(h.panel("tabs").height(), 23.0);
    assert_eq!(h.panel("tabs").top(), h.panel("menu").bottom());
    // Sin Pixel UI, los de siempre.
    let plain = LayoutHarness::editor();
    let heights = (plain.panel("menu").height(), plain.panel("tabs").height());
    assert_eq!(heights, (23.0, 23.0));
}

#[test]
fn the_active_tab_is_tab_active_and_the_others_are_tab() {
    let docs = [("a.chimp", true), ("b.chimp", true), ("c.chimp", true)];
    let h = LayoutHarness::with_tabs(&docs, Some(1));
    let active = union(&h.drawn("tab.active")).expect("the active box");
    assert_eq!(active, h.button("b.chimp"), "the box of the active tab");
    // La caja activa mide 17 (baja sobre el grabado); las otras, 15.
    assert_eq!(active.height(), 17.0);
    let boxes = h.drawn("tab");
    for name in ["a.chimp", "c.chimp"] {
        let tab = h.button(name);
        assert_eq!(tab.height(), 15.0, "{name}");
        let parts: Vec<Rect> = boxes
            .iter()
            .copied()
            .filter(|r| tab.contains_rect(*r))
            .collect();
        assert_eq!(union(&parts), Some(tab), "{name}");
    }
    assert!(
        boxes.iter().all(|r| !active.intersects(*r)),
        "no inactive box under the active one"
    );
    // Arriba, alineadas: la activa empieza en la misma fila que las otras.
    assert_eq!(active.top(), h.button("a.chimp").top());
    assert_eq!(
        active.top() - h.panel("tabs").top(),
        6.0,
        "the mockup's row 54"
    );
}

#[test]
fn on_home_every_tab_is_inactive_and_home_has_no_box() {
    let h = two_tabs(None);
    assert_eq!(h.drawn("tab.active"), vec![], "no active box");
    for name in ["Design.chimp", "Otro.chimp"] {
        let tab = h.button(name);
        assert!(
            h.drawn("tab").iter().any(|r| tab.contains_rect(*r)),
            "{name}"
        );
    }
    // Home: la casita, el humo y el suelo, siempre igual.
    let home = h.button("Home");
    for piece in ["icon.home", "icon.home_smoke", "home.ground"] {
        assert_eq!(h.drawn(piece).len(), 1, "{piece}");
    }
    let house = h.drawn("icon.home")[0];
    assert_eq!(
        house,
        h.drawn("icon.home_smoke")[0],
        "the smoke goes over the house"
    );
    let tabs = h.panel("tabs");
    assert_eq!(
        house.min - tabs.min,
        Vec2::new(4.0, 0.0),
        "the mockup's (32,48)"
    );
    assert!(home.contains_rect(house));
    // Con una pestaña activa, Home se ve igual.
    let other = two_tabs(Some(0));
    assert_eq!(other.drawn("icon.home"), vec![house]);
    assert!(!other.drawn("tab.active").is_empty());
}

#[test]
fn the_marker_is_a_circle_with_unsaved_changes_and_a_cross_without() {
    let h = two_tabs(Some(0));
    let (dirty, clean) = (h.button("Design.chimp"), h.button("Otro.chimp"));
    let dots = h.drawn("icon.tab_dirty");
    let crosses = h.drawn("icon.tab_close");
    assert_eq!(dots.len(), 1);
    assert_eq!(crosses.len(), 1);
    assert!(dirty.contains_rect(dots[0]), "{dots:?} in {dirty:?}");
    assert!(clean.contains_rect(crosses[0]), "{crosses:?} in {clean:?}");
    // 5 px antes del borde derecho y 7 debajo del de arriba, como en el mockup.
    for (tab, mark) in [(dirty, dots[0]), (clean, crosses[0])] {
        assert_eq!(mark.size(), Vec2::new(5.0, 5.0));
        assert_eq!(tab.right() - mark.right(), 5.0);
        assert_eq!(mark.top() - tab.top(), 7.0);
    }
    // El nombre empieza 5 px adentro.
    let text = h
        .texts
        .iter()
        .find(|(t, _)| t == "Design.chimp")
        .expect("the name");
    assert_eq!(text.1.x - dirty.left(), 5.0);
}

#[test]
fn clicking_the_marker_closes_a_clean_tab() {
    let mut h = two_tabs(Some(0));
    let mark = h.button("Close Otro.chimp");
    h.click(mark.center(), PointerButton::Primary);
    assert_eq!(h.app.pending, None, "no question");
    assert_eq!(h.app.tabs.len(), 1);
    assert_eq!(h.app.doc.location.name, "Design.chimp");
}

#[test]
fn clicking_the_marker_of_a_dirty_tab_asks_first() {
    let mut h = two_tabs(Some(1));
    let mark = h.button("Close Design.chimp");
    h.click(mark.center(), PointerButton::Primary);
    assert_eq!(
        h.app.pending,
        Some(Pending::CloseTab(0)),
        "the Unsaved dialog"
    );
    assert_eq!(h.app.tabs.len(), 2, "still open");
    assert_eq!(
        h.app.tabs.active(),
        Some(1),
        "the click does not switch tabs"
    );
}

#[test]
fn tabs_switch_on_click_go_home_and_close_with_the_middle_button() {
    let mut h = two_tabs(Some(0));
    h.click(h.button("Otro.chimp").center(), PointerButton::Primary);
    assert_eq!(h.app.tabs.active(), Some(1));
    h.click(h.button("Home").center(), PointerButton::Primary);
    assert_eq!(h.app.tabs.active(), None);
    h.click(h.button("Otro.chimp").center(), PointerButton::Middle);
    assert_eq!(h.app.tabs.len(), 1, "closed: it had no changes");
}

#[test]
fn hovering_an_inactive_tab_does_not_change_it() {
    let mut h = two_tabs(Some(0));
    let before = h.textures.clone();
    let tab = h.button("Otro.chimp");
    h.frame_with(vec![Event::PointerMoved(
        tab.left_center() + Vec2::new(8.0, 0.0),
    )]);
    h.frame();
    assert_eq!(h.textures, before);
}

#[test]
fn with_many_tabs_the_names_end_in_an_ellipsis_and_every_tab_fits() {
    let names: Vec<String> = (0..14)
        .map(|i| format!("A sprite with a very long name {i}.chimp"))
        .collect();
    // La mitad, con cambios.
    let docs: Vec<(&str, bool)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i % 2 == 0))
        .collect();
    let h = LayoutHarness::with_tabs(&docs, Some(3));
    let right = h.screen().right();
    let tabs: Vec<Rect> = names.iter().map(|n| h.button(n)).collect();
    for pair in tabs.windows(2) {
        assert!(pair[0].right() < pair[1].left(), "in a row: {pair:?}");
        assert_eq!(pair[0].top(), pair[1].top(), "no wrapping");
    }
    assert!(tabs.last().unwrap().right() <= right, "{tabs:?} in {right}");
    // Se achican parejo: no difieren en más de 1 px.
    let widths: Vec<f32> = tabs.iter().map(|r| r.width()).collect();
    for w in &widths {
        assert!((w - widths[0]).abs() <= 1.0, "{widths:?}");
        assert_eq!(w.fract(), 0.0, "whole pixels: {widths:?}");
    }
    let shown: Vec<&String> = h
        .texts
        .iter()
        .map(|(t, _)| t)
        .filter(|t| t.starts_with("A sprite"))
        .collect();
    assert_eq!(shown.len(), names.len(), "{shown:?}");
    // La fuente pixel no tiene "…": se corta con tres puntos.
    assert!(shown.iter().all(|t| t.ends_with("...")), "{shown:?}");
    // Con pocas, el nombre entero.
    let few = two_tabs(Some(0));
    assert!(few.texts.iter().any(|(t, _)| t == "Otro.chimp"));
}

#[test]
fn the_fade_is_drawn_at_the_top_right_corner_behind_the_menu() {
    let h = two_tabs(Some(0));
    let fade = h.drawn("menubar.fade");
    let screen = h.screen();
    // 122×34 en el mockup: desde la primera fila del menú hasta la de arriba del grabado.
    let expected = Rect::from_min_max(
        Pos2::new(screen.right() - 122.0, screen.top()),
        Pos2::new(screen.right(), screen.top() + 34.0),
    );
    assert_eq!(union(&fade), Some(expected), "{fade:?}");
    assert_eq!(expected.bottom(), h.panel("tabs").bottom() - 3.0);
    // También en Home.
    let home = two_tabs(None);
    assert_eq!(union(&home.drawn("menubar.fade")), Some(expected));
}

#[test]
fn the_etched_line_under_the_tabs_stops_at_the_active_tab() {
    let h = two_tabs(Some(0));
    let colors = *h.app.pixel.skin().expect("on").colors();
    let active = h.button("Design.chimp");
    let y = h.panel("tabs").bottom() - 2.0;
    let rows: Vec<Rect> = h
        .colored_rects()
        .into_iter()
        .filter(|(c, r)| *c == colors.canvas && r.top() == y && r.height() == 1.0)
        .map(|(_, r)| r)
        .collect();
    assert_eq!(
        rows,
        vec![
            Rect::from_x_y_ranges(h.screen().left()..=active.left(), y..=y + 1.0),
            Rect::from_x_y_ranges(active.right()..=h.screen().right(), y..=y + 1.0),
        ]
    );
    // En Home, entera.
    let home = two_tabs(None);
    let full = Rect::from_x_y_ranges(home.screen().x_range(), y..=y + 1.0);
    assert!(home.colored_rects().contains(&(colors.canvas, full)));
}

#[test]
fn without_pixel_ui_the_tab_bar_does_not_use_the_skin() {
    let h = LayoutHarness::editor();
    assert!(h.app.pixel.skin().is_none());
    assert!(h.texts.iter().any(|(t, _)| t == "🏠 Home"), "the egui tabs");
}

#[test]
fn hovering_the_dirty_marker_turns_it_into_a_cross() {
    let mut h = two_tabs(Some(1));
    let tab = h.button("Design.chimp");
    let dot = h.drawn("icon.tab_dirty");
    assert_eq!(dot.len(), 1, "a circle before hovering");
    let mark = dot[0];
    // El puntero sobre la marca, con eventos crudos: el hover rige desde el frame siguiente.
    h.frame_with(vec![Event::PointerMoved(mark.center())]);
    h.frame();
    assert_eq!(h.drawn("icon.tab_dirty"), vec![], "no circle while hovered");
    let crosses = h.drawn("icon.tab_close");
    assert!(
        crosses.contains(&mark),
        "a cross where the circle was: {crosses:?}"
    );
    assert!(tab.contains_rect(mark));
    // En otra parte de la pestaña vuelve el círculo.
    h.frame_with(vec![Event::PointerMoved(
        tab.left_center() + Vec2::new(3.0, 0.0),
    )]);
    h.frame();
    assert_eq!(h.drawn("icon.tab_dirty"), vec![mark]);
    // Una pestaña sin cambios muestra la cruz siempre.
    let clean = h.button("Otro.chimp");
    assert!(
        h.drawn("icon.tab_close")
            .iter()
            .any(|r| clean.contains_rect(*r))
    );
}

/// `n` pestañas con nombres largos (la mitad con cambios) en una ventana de `width` de ancho.
fn many_tabs(n: usize, width: f32) -> (LayoutHarness, Vec<String>) {
    let names: Vec<String> = (0..n).map(|i| format!("Sprite number {i}.chimp")).collect();
    let docs: Vec<(&str, bool)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i % 2 == 0))
        .collect();
    let h = LayoutHarness::with_tabs_in(&docs, Some(0), Vec2::new(width, 600.0));
    (h, names)
}

#[test]
fn every_marker_and_its_hit_area_stay_inside_their_tab() {
    // De pestañas anchas a tan angostas que la marca no entra.
    for (n, width) in [
        (3, 600.0),
        (8, 400.0),
        (12, 400.0),
        (16, 400.0),
        (24, 400.0),
    ] {
        let (h, names) = many_tabs(n, width);
        let tabs: Vec<Rect> = names.iter().map(|name| h.button(name)).collect();
        assert!(
            tabs.last().unwrap().right() <= width,
            "{n} in {width}: {tabs:?}"
        );
        let marks: Vec<Rect> = ["icon.tab_dirty", "icon.tab_close"]
            .iter()
            .flat_map(|piece| h.drawn(piece))
            .collect();
        for mark in &marks {
            assert!(
                tabs.iter().any(|t| t.contains_rect(*mark)),
                "{n} in {width}: {mark:?} out of every tab {tabs:?}"
            );
        }
        for (name, tab) in names.iter().zip(&tabs) {
            let close = h
                .buttons
                .iter()
                .find(|(l, _)| *l == format!("Close {name}"));
            if let Some((_, hit)) = close {
                assert!(
                    tab.contains_rect(*hit),
                    "{n} in {width}: {hit:?} in {tab:?}"
                );
            }
        }
    }
}

#[test]
fn tabs_too_narrow_for_the_marker_hide_it_and_its_hit_area() {
    let (mut h, names) = many_tabs(24, 400.0);
    let tabs: Vec<Rect> = names.iter().map(|name| h.button(name)).collect();
    assert!(tabs.iter().all(|t| t.width() < 20.0), "narrow: {tabs:?}");
    assert_eq!(h.drawn("icon.tab_dirty"), vec![], "no circle");
    assert_eq!(h.drawn("icon.tab_close"), vec![], "no cross");
    assert!(
        !h.buttons.iter().any(|(l, _)| l.starts_with("Close ")),
        "no hit area"
    );
    // Un clic donde iría la marca cambia de pestaña, no cierra nada.
    let tab = tabs[3];
    h.click(
        Pos2::new(tab.right() - 2.0, tab.top() + 9.0),
        PointerButton::Primary,
    );
    assert_eq!(h.app.tabs.len(), 24);
    assert_eq!(h.app.pending, None);
    assert_eq!(h.app.tabs.active(), Some(3));
}

#[test]
fn with_pixel_ui_the_menu_has_the_mockup_color_and_spacing() {
    let h = two_tabs(Some(0));
    let color = h.app.pixel.skin().expect("on").colors().home_text;
    assert_eq!(
        color,
        egui::Color32::from_rgb(51, 39, 42),
        "the mockup's menu text"
    );
    let menu = h.panel("menu");
    let titles = ["File", "Sprite", "View", "Edit", "Layer", "Select"];
    let rects: Vec<Rect> = titles
        .iter()
        .map(|t| {
            let (_, rect, c) = h
                .text_rects
                .iter()
                .find(|(s, r, _)| s == t && r.top() < menu.bottom())
                .unwrap_or_else(|| panic!("no {t:?} in the menu"));
            assert_eq!(*c, color, "{t}");
            *rect
        })
        .collect();
    // El mockup: la primera palabra a 3 px del borde y 11 px de aire entre la tinta de una y la
    // siguiente. Las letras de la fuente pixel empiezan en su borde y dejan 1 px vacío después:
    // del final de un texto al comienzo del siguiente van 10.
    assert_eq!(rects[0].left() - menu.left(), 3.0, "{rects:?}");
    for pair in rects.windows(2) {
        assert_eq!(pair[1].left() - pair[0].right(), 10.0, "{pair:?}");
    }
    // Sin Pixel UI, el menú de egui con sus colores.
    let plain = LayoutHarness::editor();
    let file = plain
        .text_rects
        .iter()
        .find(|(s, _, _)| s == "File")
        .unwrap();
    assert_ne!(file.2, color);
}

// El área de trabajo con Pixel UI: la caja y las reglas del mockup.

#[test]
fn with_pixel_ui_the_workspace_box_is_5_px_from_its_neighbours() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let workspace = union(&h.drawn("workspace.box")).expect("the box");
    let toolbar = union(&h.drawn("toolbar.box")).expect("the toolbar box");
    // El mockup: la caja de la barra de íconos termina en la x 59, quedan 5 px de fondo
    // (60..64) y la del área de trabajo empieza en la 65. Arriba, las dos en la fila 95, 5 px
    // debajo del grabado de la barra de opciones (88/89).
    assert_eq!(
        workspace.left() - toolbar.right(),
        5.0,
        "{workspace:?} {toolbar:?}"
    );
    assert_eq!(workspace.top(), toolbar.top());
    assert_eq!(workspace.top() - h.panel("tool_options").bottom(), 5.0);
    // A la derecha, 5 px hasta el panel (en el mockup, hasta su caja: 776..780); abajo, 5 hasta
    // la línea de tiempo (el mockup no la tiene: el mismo aire).
    assert_eq!(h.panel("palette").left() - workspace.right(), 5.0);
    assert_eq!(h.panel("timeline").top() - workspace.bottom(), 5.0);
    // Las reglas y el lienzo, dentro de la línea clara, el aire y la línea oscura.
    let canvas = h.canvas.expect("a document is open");
    assert_eq!(canvas, workspace.shrink(3.0));
}

#[test]
fn with_pixel_ui_the_rulers_are_12_thick_with_the_smiley_and_the_digit_pieces() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // El zoom del mockup: una marca chica por píxel y un número cada 5.
    h.app.doc.canvas.zoom = 8.0;
    h.frame();
    h.frame();
    let canvas = h.canvas.expect("a document is open");
    let colors = *h.app.pixel.skin().expect("on").colors();
    assert_eq!(
        h.drawn("ruler.corner"),
        vec![Rect::from_min_size(canvas.min, Vec2::splat(11.0))],
        "the smiley in the corner"
    );
    let rects = h.colored_rects();
    let row =
        |y: f32, x0: f32| Rect::from_min_max(Pos2::new(x0, y), Pos2::new(canvas.right(), y + 1.0));
    // El brillo en la fila 10 y la línea oscura en la 11: el lienzo empieza en la 12.
    assert!(rects.contains(&(
        colors.highlight,
        row(canvas.top() + 10.0, canvas.left() + 12.0)
    )));
    assert!(rects.contains(&(colors.ruler_tick, row(canvas.top() + 11.0, canvas.left()))));
    // Los números con las cifras del skin: algún "10" es un 1 y, 2 px después, un 0.
    let zeros = h.drawn("digit.0");
    let tens = h
        .drawn("digit.1")
        .into_iter()
        .filter(|one| {
            let zero = Rect::from_min_size(one.min + Vec2::new(2.0, 0.0), Vec2::new(3.0, 5.0));
            zeros.contains(&zero)
        })
        .count();
    assert!(
        tens > 0,
        "a 10: ones {:?} zeros {zeros:?}",
        h.drawn("digit.1")
    );
    assert!(!h.drawn("digit.5").is_empty(), "a label every 5");
    // Y ningún número de las reglas de egui.
    assert!(!h.texts.iter().any(|(t, _)| t == "10"), "{:?}", h.texts);
}

#[test]
fn without_pixel_ui_the_canvas_has_no_box_and_the_egui_rulers() {
    let h = LayoutHarness::editor();
    let canvas = h.canvas.expect("a document is open");
    assert_eq!(canvas.left(), h.panel("tools").right(), "no air");
    assert_eq!(canvas.top(), h.panel("tool_options").bottom());
    assert_eq!(canvas.right(), h.panel("palette").left());
    // Los números de las reglas son textos de egui.
    assert!(h.texts.iter().any(|(t, _)| t == "0"), "{:?}", h.texts);
}
