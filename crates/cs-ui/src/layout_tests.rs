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
    /// Casillas del último frame: nombre, rect y si está marcada.
    checks: Vec<(String, Rect, bool)>,
    /// Botones de prender y apagar del último frame: nombre y si está prendido.
    selected: Vec<(String, bool)>,
    /// Todo lo que tiene nombre en el último frame (botones, sliders, campos): nombre y rect.
    nodes: Vec<(String, Rect)>,
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
    /// Cuándo pidió egui el próximo frame después del último.
    repaint_delay: std::time::Duration,
    /// Si la ventana tiene el foco.
    focused: bool,
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
            checks: Vec::new(),
            selected: Vec::new(),
            nodes: Vec::new(),
            shapes: Vec::new(),
            textures: Vec::new(),
            texts: Vec::new(),
            text_rects: Vec::new(),
            size,
            repaint_delay: std::time::Duration::MAX,
            focused: true,
        };
        for _ in 0..3 {
            h.frame();
        }
        h
    }

    /// El editor sin Pixel UI, con los paneles de siempre (línea de tiempo visible).
    fn editor() -> Self {
        Self::new(plain(), true)
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
            focused: self.focused,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut canvas = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.pixel.update(&ctx, &app.view);
            // Como `App::ui`: los atajos corren antes de dibujar los paneles.
            app.handle_shortcuts(&ctx);
            canvas = app.layout(ui, false);
        });
        // Las texturas (fuentes, skin y lienzo) no se suben a ninguna GPU.
        output.textures_delta.clear();
        self.repaint_delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(std::time::Duration::MAX, |v| v.repaint_delay);
        self.canvas = canvas;
        self.shapes = std::mem::take(&mut output.shapes);
        self.textures.clear();
        self.texts.clear();
        self.text_rects.clear();
        for clipped in &self.shapes {
            collect(&clipped.shape, &mut self.textures, &mut self.texts);
            collect_text_rects(&clipped.shape, &mut self.text_rects);
        }
        let nodes = output
            .platform_output
            .accesskit_update
            .map(|update| update.nodes)
            .unwrap_or_default();
        let named = |role: egui::accesskit::Role| {
            nodes
                .iter()
                .filter(move |(_, n)| n.role() == role)
                .filter_map(|(_, n)| {
                    let b = n.bounds()?;
                    let rect = Rect::from_min_max(
                        Pos2::new(b.x0 as f32, b.y0 as f32),
                        Pos2::new(b.x1 as f32, b.y1 as f32),
                    );
                    Some((n.label()?.to_string(), rect, n))
                })
        };
        self.buttons = named(egui::accesskit::Role::Button)
            .map(|(label, rect, _)| (label, rect))
            .collect();
        self.selected = named(egui::accesskit::Role::Button)
            .filter_map(|(label, _, n)| {
                let on = n.toggled()? == egui::accesskit::Toggled::True;
                Some((label, on))
            })
            .collect();
        self.nodes = nodes
            .iter()
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let rect = Rect::from_min_max(
                    Pos2::new(b.x0 as f32, b.y0 as f32),
                    Pos2::new(b.x1 as f32, b.y1 as f32),
                );
                Some((n.label()?.to_string(), rect))
            })
            .collect();
        self.checks = named(egui::accesskit::Role::CheckBox)
            .map(|(label, rect, n)| {
                let on = n.toggled() == Some(egui::accesskit::Toggled::True);
                (label, rect, on)
            })
            .collect();
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

    /// Lo que se ve de la pieza `name`: cada malla recortada por su clip.
    fn visible(&self, name: &str) -> Vec<Rect> {
        fn walk(shape: &egui::Shape, clip: Rect, id: TextureId, out: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, clip, id, out)),
                egui::Shape::Mesh(mesh) if mesh.texture_id == id => {
                    let seen = mesh.calc_bounds().intersect(clip);
                    if seen.is_positive() {
                        out.push(seen);
                    }
                }
                _ => {}
            }
        }
        let id = self.piece(name);
        let mut out = Vec::new();
        for clipped in &self.shapes {
            walk(&clipped.shape, clipped.clip_rect, id, &mut out);
        }
        out
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

/// Sin Pixel UI (la app abre con Pixel UI).
fn plain() -> ViewSettings {
    ViewSettings {
        pixel_ui: false,
        ..Default::default()
    }
}

fn pixel_ui() -> ViewSettings {
    ViewSettings {
        pixel_ui: true,
        ..Default::default()
    }
}

#[test]
fn the_app_opens_with_pixel_ui_on_home_and_in_the_editor() {
    // Lo de fábrica: Pixel UI. En los tres primeros frames ya rige (skin, menú de 17, Home con
    // su casita).
    let home = LayoutHarness::new(ViewSettings::default(), false);
    assert!(home.app.pixel.skin().is_some(), "Pixel UI on");
    assert_eq!(home.panel("menu").height(), 17.0);
    assert_eq!(home_frames(&home).len(), 1, "the Home house");
    let editor = LayoutHarness::new(ViewSettings::default(), true);
    assert!(editor.app.pixel.skin().is_some());
    assert_eq!(editor.panel("tool_options").height(), 23.0);
    assert!(!editor.drawn("toolbar.box").is_empty(), "the pixel toolbar");
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
fn the_right_panel_goes_down_beside_the_timeline() {
    let h = LayoutHarness::editor();
    let (palette, timeline) = (h.panel("palette"), h.panel("timeline"));
    assert!(
        palette.bottom() > timeline.top(),
        "full height: {palette:?} vs {timeline:?}"
    );
    assert_eq!(
        palette.bottom(),
        timeline.bottom(),
        "hasta la barra de estado"
    );
    assert!(
        timeline.right() <= palette.left(),
        "the timeline ends to its left: {timeline:?} vs {palette:?}"
    );
}

#[test]
fn with_pixel_ui_home_is_the_design_one() {
    let mut h = LayoutHarness::new(pixel_ui(), false);
    assert!(!h.app.has_doc());
    // Abajo, la barra con la versión.
    assert!(h.texts.iter().any(|(t, _)| t.starts_with("ChimpSprite ")));
    // Los botones de 71×17: New file y Open file en una fila (cada 74), Open demo debajo (20).
    let new = h.button("New file");
    let open = h.button("Open file");
    let demo = h.button("Open demo");
    assert_eq!(new.size(), Vec2::new(71.0, 17.0));
    assert_eq!((open.left() - new.left(), open.top()), (74.0, new.top()));
    assert_eq!((demo.left(), demo.top() - new.top()), (new.left(), 20.0));
    // Pixnews: una pestaña por fuente, la primera elegida.
    for source in crate::news::Source::ALL {
        let tab = h.button(source.label());
        assert_eq!(tab.size(), Vec2::new(26.0, 28.0), "{source:?}");
    }
    assert!(h.drawn("news.title").len() == 1);
    // Los botones sueltos (Open de Pixnews) también son la pieza `button` del skin.
    let open_site = h.button("Open");
    assert!(
        h.drawn("button").iter().any(|r| r.intersects(open_site)),
        "{open_site:?}"
    );
    h.click(h.button("itch.io").center(), PointerButton::Primary);
    assert_eq!(h.app.news.source, crate::news::Source::Itch);
    assert!(h.drawn("button").len() >= 3, "the skin's button piece");
    assert!(h.texts.iter().any(|(t, _)| t == "Recent Files"));
    assert!(h.texts.iter().any(|(t, _)| t == "Recent Folders"));
    // New file abre un documento.
    h.click(new.center(), PointerButton::Primary);
    assert!(h.app.has_doc());
}

/// Con el puntero quieto sobre `at` hasta que aparece el tooltip: los rects del globo.
fn bubble_over(h: &mut LayoutHarness, at: Pos2) -> Vec<Rect> {
    h.frame_with(vec![Event::PointerMoved(at)]);
    for _ in 0..30 {
        h.frame();
    }
    h.drawn("tooltip.bubble")
}

#[test]
fn with_pixel_ui_every_tooltip_is_the_bubble_and_it_flips_at_the_right_edge() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // Un botón con lugar a la derecha: el globo sale hacia la derecha.
    let dock = h.button("Artboards");
    let bubble = union(&bubble_over(&mut h, dock.center())).expect("the bubble");
    assert!(
        bubble.left() >= dock.right(),
        "{bubble:?} right of {dock:?}"
    );
    // Pegado al borde derecho: sale a la izquierda (espejado).
    let mask = h.button("Mask");
    let bubble = union(&bubble_over(&mut h, mask.center())).expect("the bubble");
    assert!(bubble.right() <= mask.left(), "{bubble:?} left of {mask:?}");
    assert!(h.screen().contains_rect(bubble));
}

#[test]
fn with_pixel_ui_the_running_monkey_is_at_the_top_right() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let screen = h.screen();
    // 38×35, arriba de todo y a 5 del borde derecho (frame 4 del diseño).
    let expected = Rect::from_min_size(
        Pos2::new(screen.right() - 5.0 - 38.0, screen.top()),
        Vec2::new(38.0, 35.0),
    );
    assert!(
        h.textures.iter().any(|(_, r)| *r == expected),
        "no monkey at {expected:?}"
    );
}

#[test]
fn on_home_there_is_no_options_bar() {
    let h = LayoutHarness::new(plain(), false);
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
fn with_pixel_ui_the_options_bar_is_23_high() {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let h = LayoutHarness::new(view, true);
    assert_eq!(h.panel("tool_options").height(), 23.0);
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
        23.0,
        "19 rows, 2 of air and the etched line"
    );
    let width = h.screen().x_range();
    // Las dos líneas del mockup: debajo de las pestañas (y 69/70) y abajo de la barra (90/91 en el frame 4).
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
fn with_pixel_ui_the_menu_is_17_high_and_the_tabs_23() {
    let h = two_tabs(Some(0));
    // El mockup: el menú de la fila 34 a la 47 (14) y las pestañas de la 48 a la 70 (con el
    // grabado en la 69 y la 70). El menú tiene 3 filas más que en el mockup para que la caja del
    // título abierto entre entera arriba de las mayúsculas, con 1 de aire hasta el borde.
    assert_eq!(h.panel("menu").height(), 17.0);
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
    // Solo las de la barra de pestañas (las de Artboards, en el panel derecho, usan las mismas
    // piezas).
    let bar = h.panel("tabs");
    let in_bar = |name: &str| -> Vec<Rect> {
        h.drawn(name)
            .into_iter()
            .filter(|r| bar.intersects(*r))
            .collect()
    };
    let active = union(&in_bar("tab.active")).expect("the active box");
    assert_eq!(active, h.button("b.chimp"), "the box of the active tab");
    // La caja activa mide 17 (baja sobre el grabado); las otras, 15.
    assert_eq!(active.height(), 17.0);
    let boxes = in_bar("tab");
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
    // Home: un frame de la casita animada y el suelo.
    let home = h.button("Home");
    assert_eq!(h.drawn("home.ground").len(), 1);
    let house = union(&home_frames(&h)).expect("a frame of the house");
    assert_eq!(home_frames(&h).len(), 1, "a single frame");
    let tabs = h.panel("tabs");
    // El frame mide 21×21 y su casita empieza 1 px más a la derecha que la del mockup: en
    // (31,48), el cuerpo de la casita cae en la x 34 del mockup, como antes.
    assert_eq!(
        Rect::from_min_size(house.min - tabs.min.to_vec2(), house.size()),
        Rect::from_min_size(Pos2::new(3.0, 0.0), Vec2::splat(21.0)),
        "the mockup's (31,48)"
    );
    assert!(home.contains_rect(house));
    // Con una pestaña activa, Home se ve igual.
    let other = two_tabs(Some(0));
    assert_eq!(union(&home_frames(&other)), Some(house));
    assert!(!other.drawn("tab.active").is_empty());
}

/// Los rects de los frames de la casita (`icon.home.N`) dibujados.
fn home_frames(h: &LayoutHarness) -> Vec<Rect> {
    (0..3)
        .flat_map(|i| h.drawn(&format!("icon.home.{i}")))
        .collect()
}

#[test]
fn the_home_house_shows_the_frame_of_the_time_and_asks_for_the_next_one() {
    let mut h = two_tabs(None);
    // Frames de 300 ms (vuelta de 0,9 s): a los 9 s empieza una vuelta; a los 9,75 s va el
    // tercer frame, y faltan 150 ms para el siguiente.
    for (time, frame, next) in [(9.0, 0, 0.3), (9.3, 1, 0.3), (9.75, 2, 0.15)] {
        h.time = time - 0.05;
        h.frame();
        let drawn: Vec<usize> = (0..3)
            .filter(|i| !h.drawn(&format!("icon.home.{i}")).is_empty())
            .collect();
        assert_eq!(drawn, vec![frame], "at {time}");
        // egui adelanta el pedido lo que dura un frame (`predicted_dt`, 1/60 s por defecto). El
        // logo de Home (frames de 100 ms) puede pedirlo antes, pero nunca un repintado continuo.
        let delay = h.repaint_delay.as_secs_f64() + 1.0 / 60.0;
        assert!(
            delay <= next + 1e-3 && delay > 1.0 / 60.0 + 1e-3,
            "a repaint at most {next} s later, not a continuous one: {delay} at {time}"
        );
    }
    // Sin Pixel UI no hay casita animada: en Home solo se repinta para el logo (≤ 100 ms).
    let mut plain = LayoutHarness::new(plain(), false);
    plain.frame();
    let delay = plain.repaint_delay.as_secs_f64() + 1.0 / 60.0;
    assert!(delay <= 0.1 + 1e-3, "the logo's next frame: {delay}");
}

#[test]
fn without_window_focus_the_home_house_stops_and_asks_for_no_repaint() {
    let mut h = two_tabs(None);
    h.focused = false;
    for _ in 0..3 {
        h.frame();
    }
    // La casita se sigue dibujando (quieta), pero no pide frames: la app en segundo plano no
    // repinta 10 veces por segundo.
    assert_eq!(home_frames(&h).len(), 1);
    assert_eq!(h.repaint_delay, std::time::Duration::MAX);
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
fn the_dock_timeline_button_hides_and_shows_the_timeline() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.view.timeline);
    let button = h.button("Timeline");
    assert_eq!(button.size(), crate::dock::BUTTON_SIZE);
    // El ícono, de 9×15, centrado en el botón de 23×24.
    let icon = Rect::from_min_size(button.min + Vec2::new(7.0, 4.0), Vec2::new(9.0, 15.0));
    assert!(h.drawn("icon.timeline").contains(&icon), "{icon:?}");
    h.click(button.center(), PointerButton::Primary);
    assert!(!h.app.view.timeline, "hidden");
    h.click(button.center(), PointerButton::Primary);
    assert!(h.app.view.timeline, "shown again");
}

#[test]
fn the_dock_preview_button_opens_the_preview_window_above_the_timeline_button() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(!h.app.view.preview, "closed by default");
    let (button, timeline) = (h.button("Preview"), h.button("Timeline"));
    assert_eq!(
        timeline.top() - button.bottom(),
        3.0,
        "right above the timeline"
    );
    let icon = Rect::from_min_size(button.min + Vec2::new(3.0, 5.0), Vec2::new(17.0, 13.0));
    assert!(h.drawn("icon.preview").contains(&icon), "{icon:?}");
    h.click(button.center(), PointerButton::Primary);
    assert!(h.app.view.preview);
    // La ventana, con el dibujo del artboard al zoom que entra, sobre el lienzo.
    let size = crate::preview::VIEW;
    let shown = h
        .textures
        .iter()
        .filter(|(_, r)| r.width() <= size.x && r.height() <= size.y && r.width() > 16.0)
        .count();
    assert!(shown > 0, "the drawing in the preview");
    h.click(h.button("Preview").center(), PointerButton::Primary);
    assert!(!h.app.view.preview, "closed again");
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
    // El tiempo de cada foto, el mismo: la casita de Home está animada.
    let time = h.time;
    let before = h.textures.clone();
    let tab = h.button("Otro.chimp");
    h.time = time - 0.05;
    h.frame_with(vec![Event::PointerMoved(
        tab.left_center() + Vec2::new(8.0, 0.0),
    )]);
    h.time = time - 0.05;
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
    // Como en el mockup: desde la primera fila del menú hasta la de arriba del aire sobre el
    // grabado. La pieza mide 122×34; con el menú 3 filas más alto, sus últimas filas se repiten
    // abajo (el tramado tiene período 2).
    let expected = Rect::from_min_max(
        Pos2::new(screen.right() - 122.0, screen.top()),
        Pos2::new(screen.right(), screen.top() + 37.0),
    );
    assert_eq!(union(&fade), Some(expected), "{fade:?}");
    assert_eq!(expected.bottom(), h.panel("tabs").bottom() - 3.0);
    let piece = Rect::from_min_size(expected.min, Vec2::new(122.0, 34.0));
    assert!(fade.contains(&piece), "the piece at its size: {fade:?}");
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
    // El diseño: la primera palabra a 6 px del borde (lo que la caja del título abierto necesita
    // a la izquierda y 1 de aire) y 11 px de aire entre la tinta de una y la siguiente. Las letras de la fuente pixel empiezan en su borde y dejan 1 px vacío después:
    // del final de un texto al comienzo del siguiente van 10.
    assert_eq!(rects[0].left() - menu.left(), 6.0, "{rects:?}");
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
    // La del área de trabajo; la línea de tiempo, debajo, tiene la suya.
    let timeline = h.panel("timeline").top();
    let boxes: Vec<Rect> = h
        .drawn("workspace.box")
        .into_iter()
        .filter(|r| r.bottom() <= timeline)
        .collect();
    let workspace = union(&boxes).expect("the box");
    let toolbar = union(&h.drawn("toolbar.box")).expect("the toolbar box");
    // El mockup: la caja de la barra de íconos termina en la x 59, quedan 5 px de fondo
    // (60..64) y la del área de trabajo empieza en la 65. Arriba, las dos en la fila 95, 5 px
    // debajo del grabado de la barra de opciones (90/91 en el frame 4).
    assert_eq!(
        workspace.left() - toolbar.right(),
        5.0,
        "{workspace:?} {toolbar:?}"
    );
    assert_eq!(workspace.top(), toolbar.top());
    assert_eq!(workspace.top() - h.panel("tool_options").bottom(), 5.0);
    // A la derecha, 3 px hasta los botones del dock (754 → 758 en el mockup); abajo, 5 hasta la
    // línea de tiempo (el mockup no la tiene: el mismo aire).
    assert_eq!(h.panel("dock").left() - workspace.right(), 3.0);
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
    assert_eq!(canvas.right(), h.panel("dock").left());
    // Los números de las reglas son textos de egui.
    assert!(h.texts.iter().any(|(t, _)| t == "0"), "{:?}", h.texts);
}

// Los menús desplegables con Pixel UI: el diseño de `Design.aseprite` (frame 3, "Layer 4").

/// Abre (o cierra) el menú `title` de la barra con un clic y espera a que termine de aparecer
/// (egui lo muestra con un fundido).
fn open_menu(h: &mut LayoutHarness, title: &str) {
    let at = h.button(title).center();
    h.click(at, PointerButton::Primary);
    for _ in 0..4 {
        h.frame();
    }
}

/// El texto `text` dibujado (el primero) con su rect y su color.
fn text_rect(h: &LayoutHarness, text: &str) -> (Rect, egui::Color32) {
    h.text_rects
        .iter()
        .find(|(t, _, _)| t == text)
        .map(|(_, r, c)| (*r, *c))
        .unwrap_or_else(|| panic!("no {text:?} in {:?}", h.texts))
}

/// Del borde de arriba de un texto de la fuente pixel a la fila de arriba de sus mayúsculas:
/// las de los títulos están en la fila 5 del menú.
fn caps_offset(h: &LayoutHarness) -> f32 {
    h.panel("menu").top() + crate::tab_bar::MENU_CAPS_TOP - text_rect(h, "File").0.top()
}

#[test]
fn opening_file_draws_the_open_title_box_and_the_panel() {
    let mut h = two_tabs(Some(0));
    assert_eq!(h.drawn("menu.title.open"), vec![], "closed");
    assert_eq!(h.drawn("menu.panel"), vec![]);
    open_menu(&mut h, "File");
    assert!(!h.drawn("menu.title.open").is_empty(), "the open title box");
    assert!(!h.drawn("menu.panel").is_empty(), "the panel");
    // El texto del título sigue donde estaba, sobre la caja.
    let color = h.app.pixel.skin().unwrap().colors().home_text;
    assert_eq!(text_rect(&h, "File").1, color);
}

#[test]
fn the_panel_hangs_from_the_open_title_like_in_the_design() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    let menu = h.panel("menu");
    for title in ["File", "Edit"] {
        open_menu(&mut h, title);
        let text = text_rect(&h, title).0;
        let caps = text.top() + offset;
        let boxed = union(&h.drawn("menu.title.open")).expect("the box");
        let panel = union(&h.drawn("menu.panel")).expect("the panel");
        // El diseño: la caja empieza 5 px antes del texto, termina 4 después (la tinta, más 5)
        // y empieza 5 filas arriba de las mayúsculas, sin recortes: la de File queda a 1 px del
        // borde de arriba y del izquierdo de la ventana.
        assert_eq!(
            boxed.left(),
            text.left() - 5.0,
            "{title}: {boxed:?} {text:?}"
        );
        assert_eq!(boxed.right(), text.right() + 4.0, "{title}");
        assert_eq!(boxed.top(), caps - 5.0, "{title}");
        assert_eq!(boxed.top(), menu.top() + 1.0, "{title}");
        if title == "File" {
            assert_eq!(boxed.left(), h.screen().left() + 1.0, "{boxed:?}");
        }
        // El panel empieza 12 filas debajo de las mayúsculas (1 debajo del menú) y con su borde
        // izquierdo en el de la caja (una sola línea); las dos filas de abajo de la caja tapan su
        // borde de arriba.
        assert_eq!(panel.top(), caps + 12.0, "{title}");
        assert_eq!(panel.top(), menu.bottom() + 1.0, "{title}");
        assert_eq!(panel.left(), boxed.left(), "{title}");
        assert_eq!(boxed.bottom(), panel.top() + 2.0, "{title}");
        // Sin huecos: la fila entre la barra y el panel también se ve de la caja.
        let gap = Rect::from_min_size(
            Pos2::new(boxed.left(), menu.bottom()),
            Vec2::new(boxed.width(), 1.0),
        );
        // (las mallas de la pieza en 9-slice: sus anchos en esa fila suman el de la caja).
        let seen: f32 = h
            .visible("menu.title.open")
            .iter()
            .map(|r| r.intersect(gap))
            .filter(|r| r.is_positive())
            .map(|r| r.width())
            .sum();
        assert_eq!(seen, gap.width(), "{title}: the row under the bar");
        // Y su columna de la izquierda hasta abajo, sobre la del panel.
        let left = Rect::from_min_max(
            Pos2::new(boxed.left(), menu.bottom()),
            Pos2::new(boxed.left() + 1.0, boxed.bottom()),
        );
        let seen: f32 = h
            .visible("menu.title.open")
            .iter()
            .map(|r| r.intersect(left))
            .filter(|r| r.is_positive())
            .map(|r| r.height())
            .sum();
        assert_eq!(seen, left.height(), "{title}: the left column");
        assert!(panel.right() > boxed.right(), "{title}");
        // Otro clic en el título lo cierra.
        open_menu(&mut h, title);
        assert_eq!(h.drawn("menu.panel"), vec![], "{title}: closed");
    }
}

#[test]
fn the_new_row_has_the_ctrl_badge_a_plus_and_the_key_in_the_dim_color() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    let panel = union(&h.drawn("menu.panel")).expect("the panel");
    let (new, color) = text_rect(&h, "New");
    assert_eq!(color, colors.text);
    // "New": 5 px adentro del borde y sus mayúsculas, 6 filas debajo del borde de arriba (la
    // línea, el bisel, 1 de aire y las 3 de arriba de la fila).
    let caps = new.top() + offset;
    assert_eq!(new.left(), panel.left() + 5.0);
    assert_eq!(caps, panel.top() + 6.0);
    // La tecla Ctrl a la altura de las mayúsculas; después, 1 px, el + de 5×5, 1 px y la N.
    let badges = h.drawn("key.ctrl");
    let badge = *badges
        .iter()
        .find(|r| r.top() == caps)
        .unwrap_or_else(|| panic!("Ctrl on the New row: {badges:?}"));
    assert_eq!(badge.size(), Vec2::new(19.0, 7.0));
    let rects = h.colored_rects();
    let plus_h = Rect::from_min_size(badge.right_top() + Vec2::new(1.0, 3.0), Vec2::new(5.0, 1.0));
    let plus_v = Rect::from_min_size(badge.right_top() + Vec2::new(3.0, 1.0), Vec2::new(1.0, 5.0));
    for plus in [plus_h, plus_v] {
        let dim: Vec<_> = rects
            .iter()
            .filter(|(c, _)| *c == colors.ruler_tick)
            .collect();
        assert!(
            rects.contains(&(colors.ruler_tick, plus)),
            "the + {plus:?} in {dim:?}"
        );
    }
    let key = h
        .text_rects
        .iter()
        .find(|(t, r, _)| t == "N" && r.left() == badge.right() + 7.0)
        .unwrap_or_else(|| panic!("N after the + in {:?}", h.text_rects));
    assert_eq!(key.2, colors.ruler_tick, "the dim key");
    // La tinta de la N termina 5 px antes del borde derecho (la fuente deja 1 px después).
    assert_eq!(key.1.right() - 1.0, panel.right() - 5.0);
}

#[test]
fn ctrl_shift_shortcuts_draw_the_ctrl_and_the_shift_badges() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    // Save As… (Ctrl+Shift+S): Ctrl, 1 px, el +, 1 px, Shift, 1 px, el +, 1 px y la S, todo a
    // la altura de las mayúsculas de la fila.
    let shifts = h.drawn("key.shift");
    assert_eq!(
        shifts.len(),
        1,
        "only Save As has Shift in File: {shifts:?}"
    );
    let shift = shifts[0];
    assert_eq!(shift.size(), Vec2::new(22.0, 7.0));
    let ctrl = h
        .drawn("key.ctrl")
        .into_iter()
        .find(|r| r.top() == shift.top())
        .unwrap_or_else(|| panic!("Ctrl on the Shift row at {shift:?}"));
    assert_eq!(shift.left() - ctrl.right(), 7.0, "1 + 5 + 1 between them");
    let rects = h.colored_rects();
    let plus = Rect::from_min_size(ctrl.right_top() + Vec2::new(1.0, 3.0), Vec2::new(5.0, 1.0));
    assert!(rects.contains(&(colors.ruler_tick, plus)), "{plus:?}");
    let plus = Rect::from_min_size(shift.right_top() + Vec2::new(1.0, 3.0), Vec2::new(5.0, 1.0));
    assert!(rects.contains(&(colors.ruler_tick, plus)), "{plus:?}");
    let key = h
        .text_rects
        .iter()
        .find(|(t, r, _)| t == "S" && r.top() + offset == shift.top())
        .unwrap_or_else(|| panic!("S on the Shift row in {:?}", h.text_rects));
    assert_eq!(key.1.left(), shift.right() + 7.0, "after the +");
    assert_eq!(key.2, colors.ruler_tick, "the dim key");
    // Ya no es la tecla genérica con el nombre escrito.
    assert!(!h.texts.iter().any(|(t, _)| t == "Shift"), "{:?}", h.texts);
}

#[test]
fn separators_are_the_etched_line_of_the_design() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    let panel = union(&h.drawn("menu.panel")).expect("the panel");
    let rects = h.colored_rects();
    // Entre los biseles: de la x 2 del panel a 2 antes del borde derecho.
    let x = egui::Rangef::new(panel.left() + 2.0, panel.right() - 2.0);
    let lights: Vec<Rect> = rects
        .iter()
        .filter(|(c, r)| *c == colors.menu_highlight && r.height() == 1.0 && r.x_range() == x)
        .map(|(_, r)| *r)
        .collect();
    // El menú File tiene tres separadores.
    assert_eq!(lights.len(), 3, "{lights:?}");
    for light in lights {
        let dark = light.translate(Vec2::new(0.0, 1.0));
        assert!(rects.contains(&(colors.background, dark)), "{dark:?}");
    }
    assert_eq!(colors.menu_highlight, egui::Color32::WHITE);
    assert_eq!(colors.background, egui::Color32::from_rgb(221, 206, 191));
}

#[test]
fn closed_titles_underline_their_first_letter_and_the_open_one_does_not() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    let color = h.app.pixel.skin().unwrap().colors().home_text;
    let underline = |h: &LayoutHarness, title: &str| {
        let text = text_rect(h, title).0;
        let line = Rect::from_min_size(
            Pos2::new(text.left(), text.top() + offset + 8.0),
            Vec2::new(5.0, 1.0),
        );
        h.colored_rects().contains(&(color, line))
    };
    for title in ["File", "Sprite", "View", "Edit", "Layer", "Select"] {
        assert!(underline(&h, title), "{title}");
    }
    open_menu(&mut h, "Edit");
    assert!(!underline(&h, "Edit"), "the open title has no underline");
    assert!(underline(&h, "File"));
}

#[test]
fn clicking_a_pixel_menu_item_still_runs_its_action() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "File");
    let new = h.button("New Ctrl+N");
    h.click(new.center(), PointerButton::Primary);
    assert_eq!(h.app.tabs.len(), 3, "a new document");
    assert_eq!(h.drawn("menu.panel"), vec![], "the menu closes");
}

#[test]
fn hovered_rows_are_pink_with_pink_bevels_and_disabled_ones_dim_text() {
    // En Home no hay documento: Save está deshabilitado.
    let mut h = two_tabs(None);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    assert_eq!(text_rect(&h, "Save").1, colors.text_dim, "disabled");
    // Sin puntos suspensivos (en ningún lado de la app).
    assert_eq!(text_rect(&h, "Open").1, colors.text);
    let panel = union(&h.drawn("menu.panel")).expect("the panel");
    let inside: Vec<&String> = h
        .texts
        .iter()
        .filter(|(_, at)| panel.contains(*at))
        .map(|(t, _)| t)
        .collect();
    assert!(!inside.iter().any(|t| t.contains('…')), "{inside:?}");
    let open = h.button("Open Ctrl+O");
    assert_eq!(open.height(), 13.0, "the row height");
    h.frame_with(vec![Event::PointerMoved(open.center())]);
    h.frame();
    // La fila entera, entre los biseles y 1 fila más abajo, en el rosa de lo elegido; los
    // biseles a su altura, el de la izquierda en el rosa oscuro y el de la derecha en el claro.
    let row = Rect::from_x_y_ranges(
        panel.left() + 2.0..=panel.right() - 2.0,
        open.top()..=open.bottom() + 1.0,
    );
    let rects = h.colored_rects();
    let pink = |color| {
        rects
            .iter()
            .filter(|(c, _)| *c == color)
            .map(|(_, r)| *r)
            .collect::<Vec<_>>()
    };
    assert_eq!(pink(colors.timeline_selection), vec![row]);
    let bevel = |x: f32| Rect::from_x_y_ranges(x..=x + 1.0, row.y_range());
    assert_eq!(
        pink(colors.timeline_selection_dark),
        vec![bevel(panel.left() + 1.0)]
    );
    assert_eq!(
        pink(colors.timeline_selection_light),
        vec![bevel(panel.right() - 2.0)]
    );
}

#[test]
fn submenus_have_an_arrow_of_shapes_and_the_same_panel() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "View");
    let colors = *h.app.pixel.skin().unwrap().colors();
    let show = h.button("Show");
    // La flecha: un triángulo de 3×5 hecho con rects del color del texto, contra la derecha.
    let panel = union(&h.drawn("menu.panel")).expect("the panel");
    let arrow: Vec<Rect> = h
        .colored_rects()
        .into_iter()
        .filter(|(c, r)| *c == colors.text && show.intersects(*r) && r.width() == 1.0)
        .map(|(_, r)| r)
        .collect();
    let heights: Vec<f32> = arrow.iter().map(|r| r.height()).collect();
    assert_eq!(heights, vec![5.0, 3.0, 1.0], "{arrow:?}");
    assert_eq!(arrow[2].right(), panel.right() - 5.0);
    assert!(!h.texts.iter().any(|(t, _)| t.contains('⏵')), "no glyph");
    h.frame_with(vec![Event::PointerMoved(show.center())]);
    h.frame();
    h.frame();
    let panels = h.drawn("menu.panel");
    let sub = panels
        .iter()
        .filter(|r| r.left() >= panel.right())
        .copied()
        .reduce(|a, b| a.union(b))
        .expect("a second panel, to the right");
    assert!(sub.contains_rect(text_rect(&h, "Rulers").0), "{sub:?}");
    // Pegado al panel del menú, sin aire entre los dos.
    assert_eq!(sub.left(), panel.right(), "{sub:?} {panel:?}");
}

/// La casilla `label` del último frame: su rect y si está marcada.
fn check(h: &LayoutHarness, label: &str) -> (Rect, bool) {
    h.checks
        .iter()
        .find(|(t, _, _)| t == label)
        .map(|(_, r, on)| (*r, *on))
        .unwrap_or_else(|| panic!("no {label:?} checkbox in {:?}", h.checks))
}

#[test]
fn a_checkbox_under_the_pointer_uses_its_hover_piece() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "View");
    assert!(h.drawn("menu.check.on.hover").is_empty());
    let (timeline, _) = text_rect(&h, "Timeline");
    h.frame_with(vec![Event::PointerMoved(timeline.center())]);
    h.frame();
    let hovered = h.drawn("menu.check.on.hover");
    assert_eq!(hovered.len(), 1, "only the Timeline box: {hovered:?}");
    assert_eq!(hovered[0].right() + 3.0, timeline.left());
}

#[test]
fn menu_checkboxes_use_the_design_box_and_a_click_toggles_them() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "View");
    let panel = union(&h.drawn("menu.panel")).expect("the panel");
    // Timeline (marcada): la casilla del diseño, de 11×12, en el borde del texto de los ítems
    // (5 px adentro del panel) y 2 filas arriba de las mayúsculas; el texto, 3 px después.
    let (timeline, _) = text_rect(&h, "Timeline");
    let caps = timeline.top() + offset;
    let boxes = h.drawn("menu.check.on");
    let on = *boxes
        .iter()
        .find(|r| r.top() == caps - 2.0)
        .unwrap_or_else(|| panic!("the box on the Timeline row: {boxes:?}"));
    assert_eq!(on.size(), Vec2::new(11.0, 12.0));
    assert_eq!(on.left(), panel.left() + 5.0);
    assert_eq!(timeline.left(), on.right() + 3.0);
    // Una fila de 13, como los demás ítems.
    let (row, checked) = check(&h, "Timeline");
    assert!(checked);
    assert_eq!(row.height(), 13.0, "{row:?}");
    assert_eq!(caps, row.top() + 3.0);
    // Onion Skin (sin marcar): la casilla sin marcar del diseño (`menu.check`), en el mismo
    // lugar y del mismo tamaño, y el texto alineado con el de Timeline.
    let (onion, _) = text_rect(&h, "Onion Skin");
    let onion_caps = onion.top() + offset;
    assert!(
        !boxes.iter().any(|r| r.top() == onion_caps - 2.0),
        "{boxes:?}"
    );
    let off_boxes = h.drawn("menu.check");
    let off = *off_boxes
        .iter()
        .find(|r| r.top() == onion_caps - 2.0)
        .unwrap_or_else(|| panic!("the box on the Onion Skin row: {off_boxes:?}"));
    assert_eq!(off.size(), Vec2::new(11.0, 12.0));
    assert_eq!(off.left(), on.left());
    assert!(
        !off_boxes.iter().any(|r| r.top() == caps - 2.0),
        "Timeline is checked: {off_boxes:?}"
    );
    assert_eq!(onion.left(), timeline.left());
    assert!(!check(&h, "Onion Skin").1);
    // Un clic la marca (y cierra el menú, como cualquier ítem de egui).
    assert!(!h.app.doc.editor.onion_enabled);
    h.click(onion.center(), PointerButton::Primary);
    assert!(h.app.doc.editor.onion_enabled, "toggled on");
    open_menu(&mut h, "View");
    let (onion, _) = text_rect(&h, "Onion Skin");
    let onion_caps = onion.top() + offset;
    let boxes = h.drawn("menu.check.on");
    assert!(
        boxes.iter().any(|r| r.top() == onion_caps - 2.0),
        "{boxes:?}"
    );
    let off_boxes = h.drawn("menu.check");
    assert!(
        !off_boxes.iter().any(|r| r.top() == onion_caps - 2.0),
        "checked now: {off_boxes:?}"
    );
    assert!(check(&h, "Onion Skin").1);
    h.click(onion.center(), PointerButton::Primary);
    assert!(!h.app.doc.editor.onion_enabled, "toggled off");
}

#[test]
fn without_pixel_ui_the_menus_are_egui_menus() {
    let mut h = LayoutHarness::editor();
    open_menu(&mut h, "View");
    assert!(h.app.pixel.skin().is_none());
    // Las casillas son las de egui: siguen marcando con un clic.
    let (onion, checked) = check(&h, "Onion Skin");
    assert!(!checked);
    h.click(onion.center(), PointerButton::Primary);
    assert!(h.app.doc.editor.onion_enabled);
    open_menu(&mut h, "File");
    // El atajo es el texto de egui, sin teclas dibujadas.
    assert!(h.texts.iter().any(|(t, _)| t == "Ctrl+N"), "{:?}", h.texts);
    let before = h.app.doc.location.name.clone();
    let new = h.button("New Ctrl+N");
    h.click(new.center(), PointerButton::Primary);
    assert_ne!(h.app.doc.location.name, before, "a new document");
}

// --- Artboards con Pixel UI ---

/// El área de píxeles del artboard activo en el último frame (la malla de su textura).
fn artboard_pixels(h: &LayoutHarness) -> Rect {
    let id = h.app.doc.editor.artboard().id;
    let tex = h.app.doc.canvas.artboard_texture(id).expect("rendered");
    h.textures
        .iter()
        .find(|(t, _)| *t == tex)
        .map(|(_, r)| *r)
        .expect("the artboard is drawn")
}

#[test]
fn with_pixel_ui_the_artboard_is_a_pink_window_whose_bar_does_not_zoom() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::Actual);
    h.frame();
    let mut bars = Vec::new();
    for zoom in [1.0, 8.0] {
        h.app.doc.canvas.zoom = zoom;
        h.frame();
        let pixels = artboard_pixels(&h);
        let frame = union(&h.drawn("artboard.frame")).expect("the window frame");
        // Borde (1) + blanco (1) a los lados y abajo.
        assert_eq!(frame.left(), pixels.left() - 2.0, "zoom {zoom}");
        assert_eq!(frame.right(), pixels.right() + 2.0, "zoom {zoom}");
        assert_eq!(frame.bottom(), pixels.bottom() + 2.0, "zoom {zoom}");
        bars.push(pixels.top() - frame.top());
        assert_eq!(h.drawn("artboard.menu").len(), 1, "≡");
        assert_eq!(h.drawn("window.close").len(), 1, "○");
    }
    // La barra de 13 y la fila blanca no cambian con el zoom.
    assert_eq!(bars, vec![14.0, 14.0]);

    // Arrastrar la barra mueve el artboard (zoom 1: un punto, un píxel).
    h.app.doc.canvas.zoom = 1.0;
    h.frame();
    let from = artboard_pixels(&h).left_top() + Vec2::new(30.0, -6.0);
    let to = from + Vec2::new(10.0, 5.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![button(to, false)]);
    h.frame();
    assert_eq!(h.app.doc.editor.artboard().position, (10, 5));
}

#[test]
fn without_pixel_ui_the_artboard_has_no_window() {
    let h = LayoutHarness::editor();
    let skin = crate::skin::Skin::embedded(&h.ctx);
    let window = skin
        .piece("artboard.frame", WidgetState::Normal)
        .unwrap()
        .texture
        .id();
    assert!(h.textures.iter().all(|(t, _)| *t != window));
    assert!(h.app.doc.canvas.artboard_window.is_none());
}

impl LayoutHarness {
    /// La caja de los colores (la pieza `fgbg.box`), que tiene que estar una sola vez.
    fn colors_box(&self) -> Rect {
        let boxes = self.drawn("fgbg.box");
        assert_eq!(boxes.len(), 1, "{boxes:?}");
        boxes[0]
    }

    /// Si el botón `label` se anuncia elegido.
    fn selected(&self, label: &str) -> bool {
        self.selected.iter().any(|(l, on)| l == label && *on)
    }
}

/// El editor de color con Pixel UI muestra el hex del color que edita.
fn hex_shown(h: &LayoutHarness, color: Rgba) -> bool {
    h.text_rects.iter().any(|(t, _, _)| *t == color.to_hex())
}

#[test]
fn with_pixel_ui_the_colors_box_goes_under_the_toolbar_box() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let tools = h.panel("tools");
    assert_eq!(tools.width(), 37.0, "the toolbar keeps its width");
    let toolbar = h.drawn("toolbar.box");
    let toolbar = toolbar
        .iter()
        .copied()
        .reduce(|a, b| a.union(b))
        .expect("the toolbar box");
    // La caja del mockup (x 33..59, y 425..473): 27×49, en la columna de la caja de la barra y 4
    // filas debajo (lo que hay en el mockup de la caja de la barra a la caja de abajo).
    let colors = h.colors_box();
    assert_eq!(colors.size(), Vec2::new(27.0, 49.0));
    assert_eq!(colors.left(), toolbar.left());
    assert_eq!(
        colors.top(),
        toolbar.bottom() + 4.0,
        "{colors:?} {toolbar:?}"
    );
    assert!(tools.contains_rect(colors));
    // El botón redondo al medio (x 42..50, y 445..453) y las dos mitades.
    assert_eq!(
        h.drawn("fgbg.swap"),
        vec![Rect::from_min_size(
            colors.min + Vec2::new(9.0, 20.0),
            Vec2::splat(9.0)
        )]
    );
    assert_eq!(
        h.button("Primary color"),
        Rect::from_min_size(colors.min + Vec2::new(3.0, 3.0), Vec2::splat(21.0))
    );
    assert_eq!(
        h.button("Secondary color"),
        Rect::from_min_size(colors.min + Vec2::new(3.0, 25.0), Vec2::splat(21.0))
    );
    // El primario arriba y el secundario abajo, en sus 19×19.
    let editor = &h.app.doc.editor;
    let fill = |v| crate::panels::to_color32(editor.resolve(v));
    let rects = h.colored_rects();
    let top = Rect::from_min_size(colors.min + Vec2::new(4.0, 4.0), Vec2::splat(19.0));
    let bottom = Rect::from_min_size(colors.min + Vec2::new(4.0, 26.0), Vec2::splat(19.0));
    assert!(rects.contains(&(fill(editor.primary), top)), "{top:?}");
    assert!(
        rects.contains(&(fill(editor.secondary), bottom)),
        "{bottom:?}"
    );
}

#[test]
fn with_pixel_ui_a_transparent_secondary_shows_the_checker_behind() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.doc.editor.secondary = PixelValue::Rgba(Rgba::TRANSPARENT);
    h.frame();
    let colors = h.colors_box();
    let bottom = Rect::from_min_size(colors.min + Vec2::new(4.0, 26.0), Vec2::splat(19.0));
    let top = Rect::from_min_size(colors.min + Vec2::new(4.0, 4.0), Vec2::splat(19.0));
    // El damero del mockup, en mosaico desde la esquina de la mitad: 3×3 tiras de 8.
    let tiles = h.drawn("fgbg.checker");
    assert_eq!(tiles.len(), 9, "{tiles:?}");
    assert!(tiles.iter().all(|t| t.size() == Vec2::splat(8.0)));
    assert!(tiles.contains(&Rect::from_min_size(bottom.min, Vec2::splat(8.0))));
    assert!(
        tiles
            .iter()
            .all(|t| t.intersects(bottom) && !t.intersects(top))
    );
    // Con alfa, el color encima del damero.
    let half = Rgba::new(255, 0, 0, 128);
    h.app.doc.editor.secondary = PixelValue::Rgba(half);
    h.frame();
    assert_eq!(h.drawn("fgbg.checker").len(), 9);
    assert!(
        h.colored_rects()
            .contains(&(crate::panels::to_color32(half), bottom))
    );
}

#[test]
fn with_pixel_ui_the_round_button_swaps_the_colors() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let (primary, secondary) = (h.app.doc.editor.primary, h.app.doc.editor.secondary);
    assert_ne!(primary, secondary);
    let swap = h.button("Swap colors");
    assert_eq!(swap, h.drawn("fgbg.swap")[0]);
    h.click(swap.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.primary, secondary);
    assert_eq!(h.app.doc.editor.secondary, primary);
}

#[test]
fn with_pixel_ui_clicking_a_half_picks_the_color_the_editor_edits() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    h.app.doc.editor.secondary = PixelValue::Rgba(Rgba::rgb(4, 5, 6));
    h.frame();
    assert!(h.selected("Primary color") && !h.selected("Secondary color"));
    assert!(hex_shown(&h, Rgba::rgb(1, 2, 3)));
    // Abajo: el editor de color pasa al secundario, con una marca en su mitad.
    h.click(h.button("Secondary color").center(), PointerButton::Primary);
    assert!(h.app.doc.editor.editing_secondary);
    assert!(h.selected("Secondary color") && !h.selected("Primary color"));
    assert!(hex_shown(&h, Rgba::rgb(4, 5, 6)));
    assert!(!hex_shown(&h, Rgba::rgb(1, 2, 3)));
    let colors = h.colors_box();
    let accent = h.app.pixel.skin().unwrap().colors().accent;
    let bottom = Rect::from_min_size(colors.min + Vec2::new(4.0, 26.0), Vec2::splat(19.0));
    let marks: Vec<Rect> = h
        .colored_rects()
        .into_iter()
        .filter(|&(c, r)| c == accent && bottom.contains_rect(r))
        .map(|(_, r)| r)
        .collect();
    assert_eq!(marks.len(), 4, "a 1 px outline: {marks:?}");
    // Arrastrar el S/V cambia el secundario y no el primario.
    let sv = h
        .nodes
        .iter()
        .find(|(l, _)| l == "Saturation and value")
        .map(|(_, r)| *r)
        .expect("the S/V square");
    let from = sv.center();
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from, true)]);
    h.frame_with(vec![press(from, false)]);
    h.frame();
    assert_eq!(
        h.app.doc.editor.primary,
        PixelValue::Rgba(Rgba::rgb(1, 2, 3))
    );
    assert_ne!(
        h.app.doc.editor.secondary,
        PixelValue::Rgba(Rgba::rgb(4, 5, 6))
    );
    // Arriba: otra vez el primario.
    h.click(h.button("Primary color").center(), PointerButton::Primary);
    assert!(!h.app.doc.editor.editing_secondary);
    assert!(hex_shown(&h, Rgba::rgb(1, 2, 3)));
}

#[test]
fn with_pixel_ui_the_right_panel_has_no_primary_and_secondary_swatches() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let palette = h.panel("palette");
    let texts: Vec<&str> = h
        .text_rects
        .iter()
        .filter(|(_, r, _)| palette.intersects(*r))
        .map(|(t, _, _)| t.as_str())
        .collect();
    assert!(!texts.contains(&"Color"), "{texts:?}");
    assert!(!texts.contains(&"🔄"), "{texts:?}");
    // Los dos colores y el intercambio están en la caja de la izquierda (y la X sigue).
    assert!(h.panel("tools").contains_rect(h.button("Swap colors")));
}

#[test]
fn without_pixel_ui_the_colors_stay_in_the_right_panel() {
    let h = LayoutHarness::editor();
    assert_eq!(h.panel("tools").width(), 184.0);
    for label in ["Primary color", "Secondary color", "Swap colors"] {
        assert!(!h.buttons.iter().any(|(l, _)| l == label), "{label}");
    }
    let palette = h.panel("palette");
    let texts: Vec<&str> = h
        .text_rects
        .iter()
        .filter(|(_, r, _)| palette.intersects(*r))
        .map(|(t, _, _)| t.as_str())
        .collect();
    assert!(
        texts.contains(&"Color") && texts.contains(&"🔄"),
        "{texts:?}"
    );
}

/// Un sprite indexado de 8×8 (con la paleta DB32) en la pestaña activa.
fn indexed_doc(h: &mut LayoutHarness) {
    let sprite = cs_core::Sprite::new(
        8,
        8,
        cs_core::ColorMode::Indexed {
            transparent_index: 0,
        },
    );
    h.app.doc.editor = crate::editor::Editor::with_sprite(sprite);
    h.frame();
}

#[test]
fn turning_pixel_ui_off_resets_which_color_the_editor_edits() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.button("Secondary color").center(), PointerButton::Primary);
    assert!(h.app.doc.editor.editing_secondary);
    // Otra pestaña, también con el secundario elegido; queda guardada al cambiar.
    h.app.new_document();
    h.frame();
    h.click(h.button("Secondary color").center(), PointerButton::Primary);
    assert!(h.app.doc.editor.editing_secondary);
    h.app.view.pixel_ui = false;
    h.frame();
    h.frame();
    assert!(!h.app.doc.editor.editing_secondary);
    // Al volver a prenderlo, ninguna pestaña sigue en el secundario.
    h.app.view.pixel_ui = true;
    h.frame();
    h.frame();
    assert!(!h.app.doc.editor.editing_secondary);
    h.app.tabs.switch(&mut h.app.doc, 0);
    h.frame();
    assert!(!h.app.doc.editor.editing_secondary);
}

#[test]
fn with_pixel_ui_new_palette_on_an_indexed_sprite_asks_first() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    indexed_doc(&mut h);
    h.app.view.palette_locked = false;
    h.frame();
    let before = h.app.doc.editor.sprite().palette.clone();
    h.click(h.button("New palette").center(), PointerButton::Primary);
    assert!(
        matches!(
            h.app.palette_confirm,
            Some(crate::palette_ui::Confirm::NewPalette)
        ),
        "asks"
    );
    assert_eq!(h.app.doc.editor.sprite().palette, before);
}

#[test]
fn with_pixel_ui_new_palette_on_an_rgba_sprite_needs_no_confirmation() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.view.palette_locked = false;
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    h.frame();
    h.click(h.button("New palette").center(), PointerButton::Primary);
    assert!(h.app.palette_confirm.is_none());
    assert_eq!(
        h.app.doc.editor.sprite().palette.colors(),
        &[Rgba::rgb(1, 2, 3)]
    );
}

/// Un `eframe::Storage` en memoria.
#[derive(Default)]
struct MemoryStorage(std::collections::HashMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

#[test]
fn toggling_the_palette_lock_is_saved_with_the_view() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.view.palette_locked);
    h.click(h.button("Lock palette").center(), PointerButton::Primary);
    assert!(!h.app.doc.editor.palette_locked);
    // El candado pasa a la vista en el mismo frame, y la vista es lo que se guarda.
    assert!(!h.app.view.palette_locked);
    let mut storage = MemoryStorage::default();
    eframe::App::save(&mut h.app, &mut storage);
    let saved: ViewSettings = eframe::get_value(&storage, crate::VIEW_KEY).expect("saved");
    assert!(!saved.palette_locked);
}

/// Arrastre con el botón principal de `from` a `to`, en 4 pasos.
fn drag(h: &mut LayoutHarness, from: Pos2, to: Pos2) {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    for k in 1..=4 {
        h.frame_with(vec![Event::PointerMoved(from.lerp(to, k as f32 / 4.0))]);
    }
    h.frame_with(vec![button(to, false)]);
    h.frame();
}

impl LayoutHarness {
    fn node(&self, label: &str) -> Rect {
        self.nodes
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {label:?} in {:?}", self.nodes))
    }

    /// Las piezas `name` dibujadas, con las partes de un 9-slice (que se tocan) unidas.
    fn drawn_boxes(&self, name: &str) -> Vec<Rect> {
        let mut boxes: Vec<Rect> = Vec::new();
        for rect in self.drawn(name) {
            match boxes.iter_mut().find(|b| b.intersects(rect)) {
                Some(b) => *b = b.union(rect),
                None => boxes.push(rect),
            }
        }
        boxes
    }
}

/// Con Pixel UI, el editor de color entero dentro del panel derecho, hasta el borde de abajo de
/// la barra del alfa.
fn assert_color_editor_whole(h: &LayoutHarness) {
    let panel = h.panel("palette");
    let bars = h.drawn_boxes("picker.bars");
    assert_eq!(bars.len(), 1, "{bars:?}");
    let bars = bars[0];
    let alpha = h.node("Alpha");
    assert!(bars.contains_rect(alpha), "{alpha:?} in {bars:?}");
    assert!(bars.bottom() <= panel.bottom(), "{bars:?} in {panel:?}");
    for label in [
        "Copy color",
        "R",
        "G",
        "B",
        "A",
        "Hex",
        "Saturation and value",
        "Hue",
    ] {
        let rect = h.node(label);
        assert!(panel.contains_rect(rect), "{label} {rect:?} in {panel:?}");
    }
}

#[test]
fn with_pixel_ui_the_color_editor_is_whole() {
    let h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.view.timeline);
    assert_eq!(h.screen().size(), Vec2::new(1200.0, 800.0));
    assert_color_editor_whole(&h);
    // La caja de la paleta, la de fábrica (207); el cuadro S/V, de 64.
    assert_eq!(h.drawn_boxes("palette.box")[0].height(), 207.0);
    assert_eq!(h.node("Saturation and value").height(), 64.0);
    // Aunque lo guardado no entre, o la ventana sea baja: se achica la paleta, no el editor.
    let tall = ViewSettings {
        palette_height: 5000.0,
        ..pixel_ui()
    };
    let h = LayoutHarness::new(tall, true);
    assert_color_editor_whole(&h);
    let short = LayoutHarness::with_tabs_in(&[("Short", false)], Some(0), Vec2::new(1200.0, 400.0));
    assert_color_editor_whole(&short);
    let pbox = short.drawn_boxes("palette.box")[0];
    assert!(pbox.height() < 207.0 && pbox.height() >= 33.0, "{pbox:?}");
}

#[test]
fn with_pixel_ui_dragging_the_palette_splitter_changes_its_height_and_is_saved() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let pbox = h.drawn_boxes("palette.box")[0];
    let split = h.node("Palette height");
    assert_eq!(split.top(), pbox.bottom());
    drag(
        &mut h,
        split.center(),
        split.center() + Vec2::new(0.0, -40.0),
    );
    assert_eq!(h.drawn_boxes("palette.box")[0].height(), 167.0);
    assert_eq!(h.app.view.palette_height, 167.0);
    assert_color_editor_whole(&h);
    let mut storage = MemoryStorage::default();
    eframe::App::save(&mut h.app, &mut storage);
    let saved: ViewSettings = eframe::get_value(&storage, crate::VIEW_KEY).expect("saved");
    assert_eq!(saved.palette_height, 167.0);
    // Abajo de más: la caja llega hasta donde el editor sigue entero.
    let split = h.node("Palette height");
    drag(
        &mut h,
        split.center(),
        split.center() + Vec2::new(0.0, 600.0),
    );
    assert!(h.drawn_boxes("palette.box")[0].height() > 207.0);
    assert_color_editor_whole(&h);
}

#[test]
fn with_pixel_ui_the_right_panel_can_not_be_narrower_than_its_minimum() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let panel = h.panel("palette");
    let edge = Pos2::new(panel.left(), panel.center().y);
    drag(&mut h, edge, edge + Vec2::new(500.0, 0.0));
    let narrow = h.panel("palette");
    assert!(narrow.width() < panel.width(), "{panel:?} -> {narrow:?}");
    // 120 de la fila de RGBA, 16 de márgenes y la línea contra el lienzo.
    assert_eq!(narrow.width(), 137.0, "{narrow:?}");
    for label in ["Copy color", "R", "G", "B", "A", "Hex", "Mask"] {
        let rect = h.node(label);
        assert!(narrow.contains_rect(rect), "{label} {rect:?} in {narrow:?}");
    }
    assert_color_editor_whole(&h);
}

#[test]
fn without_pixel_ui_the_right_panel_has_no_splitter_and_keeps_its_scroll() {
    let h = LayoutHarness::editor();
    assert!(!h.nodes.iter().any(|(l, _)| l == "Palette height"));
    assert_eq!(h.app.view.palette_height, 207.0, "untouched");
    // El selector de egui, en el scroll de siempre.
    assert!(h.texts.iter().any(|(t, _)| t == "Color"));
}

/// Con Pixel UI y la paleta guardada enorme (5000), en una ventana de `size`: el editor de color
/// queda entero y Artboards (en el dock) se abre y se puede usar.
fn assert_sections_below_reachable(size: Vec2) {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let view = ViewSettings {
        palette_height: 5000.0,
        ..pixel_ui()
    };
    let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, size);
    assert_color_editor_whole(&h);
    assert_eq!(h.app.view.palette_height, 5000.0, "lo guardado no se toca");
    h.click(h.button("Artboards").center(), PointerButton::Primary);
    // El encabezado de Artboards se ve, dentro de la ventana.
    let (_, heading, _) = h
        .text_rects
        .iter()
        .find(|(t, _, _)| t == "Artboards")
        .unwrap_or_else(|| panic!("no Artboards heading in {:?}", h.text_rects))
        .clone();
    assert!(h.screen().contains_rect(heading), "{heading:?}");
    // Y se puede tocar: el + crea un artboard.
    let add = h.button("New 64×64 artboard");
    assert!(h.screen().contains_rect(add), "{add:?}");
    let before = h.app.doc.editor.document.artboards.len();
    h.click(add.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.document.artboards.len(), before + 1);
}

#[test]
fn with_pixel_ui_the_status_bar_is_the_design_one() {
    let h = LayoutHarness::new(pixel_ui(), true);
    let status = h.panel("status");
    assert_eq!(status.height(), 20.0);
    assert_eq!(status.bottom(), h.screen().bottom());
    // Con el fondo del skin (sin él, el panel quedaba negro).
    let background = h.app.pixel.skin().unwrap().colors().background;
    let filled = h.shapes.iter().any(
        |c| matches!(&c.shape, egui::Shape::Rect(r) if r.fill == background && r.rect == status),
    );
    assert!(filled, "the status bar has the skin background");
    let name = h.app.doc.editor.artboard().name.clone();
    let (artboard, _) = text_rect(&h, &format!("{name} 64×64"));
    let (frame, _) = text_rect(&h, "1/1");
    // En la barra, de izquierda a derecha: el artboard (ícono de 13 + 3 de aire desde el borde
    // y 3 hasta el texto) y el frame después.
    assert!(status.contains_rect(artboard) && status.contains_rect(frame));
    assert_eq!(artboard.left(), status.left() + 3.0 + 13.0 + 3.0);
    assert!(frame.left() > artboard.right());
}

#[test]
fn with_pixel_ui_the_artboards_are_tabs_with_the_design_buttons() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // Están en el dock: no en el panel derecho hasta abrirlas.
    assert!(!h.buttons.iter().any(|(l, _)| l == "New 64×64 artboard"));
    let dock = h.button("Artboards");
    assert_eq!(dock.size(), Vec2::new(23.0, 24.0));
    assert!(h.panel("dock").contains_rect(dock));
    h.click(dock.center(), PointerButton::Primary);
    // El panel flotante, a la izquierda del botón.
    let panel = Rect::from_min_max(Pos2::ZERO, Pos2::new(dock.left(), 10000.0));
    let inside = |h: &LayoutHarness, name: &str| -> Vec<Rect> {
        h.drawn(name)
            .into_iter()
            .filter(|r| panel.contains_rect(*r) && r.top() >= dock.top())
            .collect()
    };
    let add = h.button("New 64×64 artboard");
    assert_eq!(add.size(), Vec2::new(25.0, 16.0));
    assert!(!h.app.doc.editor.document.artboards.is_empty());
    h.click(add.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.document.artboards.len(), 2);
    // Cada artboard, una pestaña; la del activo es `tab.active`. Sin la marca ○.
    let names: Vec<String> = h
        .app
        .doc
        .editor
        .document
        .artboards
        .iter()
        .map(|a| a.name.clone())
        .collect();
    for name in &names {
        assert!(panel.contains_rect(h.node(name)), "{name}");
    }
    assert!(!inside(&h, "tab.active").is_empty());
    assert!(inside(&h, "icon.tab_close").is_empty(), "no mark");
    // Clic en la otra pestaña: se activa.
    let first = h.app.doc.editor.document.artboards[0].id;
    assert_ne!(h.app.doc.editor.active, first);
    h.click(h.node(&names[0]).center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.active, first);
    // El tacho borra el activo.
    h.click(h.button("Delete artboard").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.document.artboards.len(), 1);
    // El botón del ícono abre Canvas Size.
    assert!(h.app.canvas_size.is_none());
    h.click(h.button("Canvas Size").center(), PointerButton::Primary);
    assert!(h.app.canvas_size.is_some());
}

#[test]
fn the_lospec_dock_button_opens_and_closes_the_lospec_panel() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // El de la columna del dock (la ventana también tiene una pestaña "Lospec").
    let dock = h.panel("dock");
    let button = |h: &LayoutHarness| {
        h.buttons
            .iter()
            .find(|(l, r)| l == "Lospec" && dock.contains_rect(*r))
            .map(|(_, r)| *r)
            .expect("the Lospec dock button")
    };
    assert_eq!(button(&h).size(), Vec2::new(23.0, 24.0));
    assert!(!h.app.lospec_open());
    h.click(button(&h).center(), PointerButton::Primary);
    assert!(h.app.lospec_open(), "opens the panel");
    assert!(h.app.presets_window.is_none(), "not the presets window");
    h.click(button(&h).center(), PointerButton::Primary);
    assert!(!h.app.lospec_open(), "and closes it");
}

#[test]
fn with_pixel_ui_a_slice_can_be_renamed() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.button("Slices").center(), PointerButton::Primary);
    h.click(h.button("New slice").center(), PointerButton::Primary);
    h.click(h.node("Slice name").center(), PointerButton::Primary);
    h.frame_with(vec![Event::Text("x".into())]);
    let key = |pressed| Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    h.frame_with(vec![key(true), key(false)]);
    h.frame();
    assert_eq!(h.app.doc.editor.sprite().slices[0].name, "Slice 1x");
}

#[test]
fn with_pixel_ui_slices_are_tabs_too() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.doc.editor.sprite().slices.is_empty());
    h.click(h.button("Slices").center(), PointerButton::Primary);
    h.click(h.button("New slice").center(), PointerButton::Primary);
    let slices = &h.app.doc.editor.sprite().slices;
    assert_eq!(slices.len(), 1);
    assert_eq!(slices[0].name, "Slice 1");
    assert_eq!(h.app.doc.editor.slice, Some(0));
    let dock = h.button("Slices");
    for label in ["Slice 1", "Slice name"] {
        assert!(
            h.node(label).right() <= dock.left(),
            "{label} left of the dock"
        );
    }
    assert!(h.button("Slice color").right() <= dock.left());
    // Abrir Artboards cierra Slices: uno por vez.
    h.click(h.button("Artboards").center(), PointerButton::Primary);
    assert!(!h.nodes.iter().any(|(l, _)| l == "Slice name"));
    assert!(h.buttons.iter().any(|(l, _)| l == "New 64×64 artboard"));
    // Y su botón lo vuelve a cerrar.
    h.click(h.button("Artboards").center(), PointerButton::Primary);
    assert!(!h.buttons.iter().any(|(l, _)| l == "New 64×64 artboard"));
    h.click(h.button("Slices").center(), PointerButton::Primary);
    h.click(h.button("Delete slice").center(), PointerButton::Primary);
    assert!(h.app.doc.editor.sprite().slices.is_empty());
}

#[test]
fn with_pixel_ui_a_huge_palette_height_keeps_artboards_reachable() {
    assert_sections_below_reachable(Vec2::new(1200.0, 800.0));
}

#[test]
fn with_pixel_ui_a_short_window_keeps_artboards_reachable() {
    assert_sections_below_reachable(Vec2::new(1200.0, 560.0));
}

/// Aprieta `key` con Ctrl un frame.
fn press_ctrl(h: &mut LayoutHarness, key: egui::Key) {
    h.frame_with(vec![Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
}

/// Un documento con el campo hex del editor de color con el foco (se necesita Pixel UI).
fn typing_in_hex() -> LayoutHarness {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.node("Hex").center(), PointerButton::Primary);
    assert!(h.ctx.egui_wants_keyboard_input(), "the hex field has focus");
    h
}

#[test]
fn ctrl_a_with_a_text_field_focused_leaves_the_canvas_selection_alone() {
    let mut h = typing_in_hex();
    press_ctrl(&mut h, egui::Key::A);
    assert!(h.app.doc.editor.selection.is_none());
    assert!(
        h.ctx.egui_wants_keyboard_input(),
        "the field keeps the focus"
    );
}

#[test]
fn ctrl_a_without_a_focused_field_selects_all_the_canvas() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(!h.ctx.egui_wants_keyboard_input());
    press_ctrl(&mut h, egui::Key::A);
    assert!(h.app.doc.editor.selection.is_some());
}

#[test]
fn ctrl_z_with_a_text_field_focused_does_not_undo_the_document() {
    let mut h = typing_in_hex();
    h.app.doc.editor.add_layer(cs_core::LayerKind::Image);
    h.frame();
    assert!(h.app.doc.editor.history.can_undo());
    let revision = h.app.doc.editor.revision;
    press_ctrl(&mut h, egui::Key::Z);
    assert_eq!(h.app.doc.editor.revision, revision);
    assert!(h.app.doc.editor.history.can_undo());
}

#[test]
fn a_new_note_from_the_edit_menu_takes_what_is_typed() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "Edit");
    h.click(h.button("New Note").center(), PointerButton::Primary);
    h.frame();
    // Como un teclado de verdad: la tecla y su texto (las letras son atajos de herramientas).
    for (key, text) in [
        (egui::Key::H, "h"),
        (egui::Key::O, "o"),
        (egui::Key::L, "l"),
        (egui::Key::A, "a"),
    ] {
        let press = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        h.frame_with(vec![press(true), Event::Text(text.into())]);
        h.frame_with(vec![press(false)]);
    }
    h.frame();
    h.frame_with(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.frame();
    let notes = &h.app.doc.editor.document.notes;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "hola");
}
