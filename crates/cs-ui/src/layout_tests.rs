//! Acomodo de los paneles de la ventana: la app entera, con una pestaña abierta, manejada con
//! eventos crudos. Los rects de los paneles salen del `PanelState` que egui guarda de cada uno.

use cs_commands::Command;
use cs_core::{LayerId, PixelValue, Rgba};
use eframe::egui::{
    self, Event, PointerButton, Pos2, RawInput, Rect, TextureId, Vec2,
    containers::panel::PanelState, pos2, vec2,
};

use crate::ChimpApp;
use crate::ToolKind;
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
    /// Los botones deshabilitados del último frame (sus nombres).
    disabled: Vec<String>,
    /// Todo lo deshabilitado con nombre del último frame (botones, campos…).
    off: Vec<String>,
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
            disabled: Vec::new(),
            off: Vec::new(),
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
            app.update_shading_window(&ctx, false);
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
        self.disabled = named(egui::accesskit::Role::Button)
            .filter(|(_, _, n)| n.is_disabled())
            .map(|(label, _, _)| label)
            .collect();
        self.off = nodes
            .iter()
            .filter(|(_, n)| n.is_disabled())
            .filter_map(|(_, n)| Some(n.label()?.to_string()))
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

    /// Doble clic con el botón principal en `pos`: mover y dos pares de apretar y soltar, un
    /// frame cada uno. Antes deja pasar medio segundo para que un clic anterior no cuente como
    /// el primero de este (egui junta los clics seguidos aunque sean en otro lado).
    fn double_click(&mut self, pos: Pos2) {
        let press = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for _ in 0..10 {
            self.frame();
        }
        self.frame_with(vec![Event::PointerMoved(pos)]);
        for _ in 0..2 {
            self.frame_with(vec![press(true)]);
            self.frame_with(vec![press(false)]);
        }
        self.frame();
    }

    /// Arrastre con el botón principal de `from` a `to`: mover, apretar, mover y soltar, un
    /// frame cada uno.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame_with(vec![Event::PointerMoved(from)]);
        self.frame_with(vec![press(from, true)]);
        self.frame_with(vec![Event::PointerMoved(to)]);
        self.frame_with(vec![press(to, false)]);
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
/// El panel del menú abierto, sin su sombra (2 px abajo y a la derecha, afuera del panel).
fn menu_panel(h: &LayoutHarness) -> Rect {
    let drawn = union(&h.drawn("menu.list")).expect("the panel");
    Rect::from_min_max(drawn.min, drawn.max - Vec2::splat(2.0))
}

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
    // 6 px antes del borde derecho (el borde de 2 y 4 de aire) y 7 debajo del de arriba, como
    // en el mockup de componentes.
    for (tab, mark) in [(dirty, dots[0]), (clean, crosses[0])] {
        assert_eq!(mark.size(), Vec2::new(5.0, 5.0));
        assert_eq!(tab.right() - mark.right(), 6.0);
        assert_eq!(mark.top() - tab.top(), 7.0);
    }
    // El nombre empieza 6 px adentro.
    let text = h
        .texts
        .iter()
        .find(|(t, _)| t == "Design.chimp")
        .expect("the name");
    assert_eq!(text.1.x - dirty.left(), 6.0);
}

#[test]
fn the_dock_timeline_button_hides_and_shows_the_timeline() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.view.timeline);
    let button = h.button("Timeline");
    assert_eq!(button.size(), crate::dock::BUTTON_SIZE);
    // El ícono, de 9×15, centrado en el botón de 23×24; con el timeline a la vista, el botón
    // está prendido (presionado, como el elegido de un grupo del kit) y el ícono baja 1 px.
    let icon = Rect::from_min_size(button.min + Vec2::new(7.0, 5.0), Vec2::new(9.0, 15.0));
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
        4.0,
        "right above the timeline (the light row and 3 of air)"
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
fn ctrl_c_and_ctrl_v_without_a_selection_copy_and_paste_frames() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let editor = &mut h.app.doc.editor;
    let layer = editor.layer;
    let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    editor.execute(cs_commands::Command::DrawPixels {
        layer,
        frame: 0,
        pixels: vec![(0, 0, red)],
    });
    // El frame elegido en el encabezado, sin nada seleccionado en el lienzo.
    editor.frames_picked = true;
    assert!(editor.selection.is_none());
    let frames = editor.frame_count();
    h.frame_with(vec![Event::Copy]);
    h.frame_with(vec![Event::Paste(crate::CLIPBOARD_MARKER.into())]);
    let editor = &h.app.doc.editor;
    assert_eq!(
        editor.frame_count(),
        frames + 1,
        "pasted after the current frame"
    );
    assert_eq!(editor.sprite().pixel(layer, 1, 0, 0), red);
}

#[test]
fn ctrl_v_with_a_coolors_link_opens_the_pasted_palette() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let before = h.app.doc.editor.sprite().palette.clone();
    let link = "https://coolors.co/palette/264653-2a9d8f-e9c46a-f4a261-e76f51";
    h.frame_with(vec![Event::Paste(link.into())]);
    let window = h
        .app
        .presets_window
        .as_ref()
        .expect("Palette Presets opened");
    assert_eq!(window.tab, crate::palette_ui::PresetsTab::Lospec);
    assert_eq!(
        window.lookup_title(),
        Some("Coolors 264653-2a9d8f-e9c46a-f4a261-e76f51")
    );
    assert_eq!(
        h.app.doc.editor.sprite().palette,
        before,
        "nothing applied yet"
    );
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
fn hovering_an_inactive_tab_lightens_only_that_tab() {
    let mut h = two_tabs(Some(0));
    let tab = h.button("Otro.chimp");
    assert!(h.drawn("tab.hover").is_empty());
    h.frame_with(vec![Event::PointerMoved(
        tab.left_center() + Vec2::new(8.0, 0.0),
    )]);
    h.frame();
    // Con el puntero encima, `tab.hover` (el relleno más claro) en esa pestaña; la activa no
    // cambia.
    let hovered = h.drawn("tab.hover");
    assert!(!hovered.is_empty());
    assert!(hovered.iter().all(|r| tab.contains_rect(*r)), "{hovered:?}");
    assert!(!h.drawn("tab.active").is_empty());
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
    let titles = [
        "File", "Edit", "Sprite", "Layer", "Frame", "Select", "View", "Help",
    ];
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
    assert_eq!(h.drawn("menu.list"), vec![]);
    open_menu(&mut h, "File");
    assert!(!h.drawn("menu.title.open").is_empty(), "the open title box");
    assert!(!h.drawn("menu.list").is_empty(), "the panel");
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
        let panel = menu_panel(&h);
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
        assert_eq!(h.drawn("menu.list"), vec![], "{title}: closed");
    }
}

#[test]
fn the_new_row_has_the_ctrl_badge_a_plus_and_the_key_in_the_dim_color() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    let panel = menu_panel(&h);
    let (new, color) = text_rect(&h, "New");
    assert_eq!(color, colors.text);
    // "New": 5 px adentro del borde y sus mayúsculas, 6 filas debajo del borde de arriba (la
    // línea, el bisel, 1 de aire y las 3 de arriba de la fila).
    let caps = new.top() + offset;
    assert_eq!(new.left(), panel.left() + 5.0);
    assert_eq!(caps, panel.top() + 6.0);
    // La badge CTRL a la altura de las mayúsculas; después, 1 px, el + de 5×5, 1 px y la N.
    let badge = badge(&h, "CTRL", caps);
    assert_eq!(badge.height(), 7.0);
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

/// La badge de la tecla `name` (en mayúsculas) con su fila de mayúsculas en `caps`: la pieza `key`
/// estirada debajo del nombre en chimpfont, con 3 px de cada lado de la tinta.
fn badge(h: &LayoutHarness, name: &str, caps: f32) -> Rect {
    let text = h
        .text_rects
        .iter()
        // El nombre en esa fila (la caja del texto de chimpfont empieza arriba de sus mayúsculas).
        .find(|(t, r, _)| t == name && r.top() <= caps && r.bottom() > caps + 5.0)
        .map(|(_, r, _)| *r)
        .unwrap_or_else(|| panic!("{name} on the row of {caps} in {:?}", h.texts));
    let badge = h
        .drawn("key")
        .into_iter()
        .filter(|k| {
            k.top() >= caps
                && k.bottom() <= caps + 7.0
                && k.left() >= text.left() - 4.0
                && k.right() <= text.right() + 4.0
        })
        .fold(Rect::NOTHING, |a, b| a.union(b));
    // 3 px antes de la tinta y 4 después (chimpfont deja 2 px vacíos al final).
    assert_eq!(
        text.left() - badge.left(),
        3.0,
        "{name}: {text:?} in {badge:?}"
    );
    assert_eq!(
        badge.right() - (text.right() - 2.0),
        4.0,
        "{name}: {text:?} in {badge:?}"
    );
    badge
}

#[test]
fn ctrl_shift_shortcuts_are_ctrl_and_shift_badges() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    // Save As… (Ctrl+Shift+S): CTRL, 1 px, el +, 1 px, SHIFT, 1 px, el +, 1 px y la S, todo a la
    // altura de las mayúsculas de la fila.
    let (save_as, _) = text_rect(&h, "Save As");
    let caps = save_as.top() + offset;
    let ctrl = badge(&h, "CTRL", caps);
    let shift = badge(&h, "SHIFT", caps);
    assert_eq!(shift.left() - ctrl.right(), 7.0, "1 + 5 + 1 between them");
    let rects = h.colored_rects();
    let plus = Rect::from_min_size(ctrl.right_top() + Vec2::new(1.0, 3.0), Vec2::new(5.0, 1.0));
    assert!(rects.contains(&(colors.ruler_tick, plus)), "{plus:?}");
    let key = h
        .text_rects
        .iter()
        .find(|(t, r, _)| t == "S" && r.top() + offset == caps)
        .unwrap_or_else(|| panic!("S on the Shift row in {:?}", h.text_rects));
    assert_eq!(key.1.left(), shift.right() + 7.0, "after the +");
    assert_eq!(key.2, colors.ruler_tick, "the dim key");
}

#[test]
fn ctrl_alt_shortcuts_have_the_alt_badge() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    open_menu(&mut h, "Sprite");
    // Sprite Size (Ctrl+Alt+I): CTRL, el +, ALT, el + y la I.
    let (size, _) = text_rect(&h, "Sprite Size");
    let caps = size.top() + offset;
    let ctrl = badge(&h, "CTRL", caps);
    let alt = badge(&h, "ALT", caps);
    assert_eq!(alt.left() - ctrl.right(), 7.0, "1 + 5 + 1 between them");
}

#[test]
fn separators_are_the_divider_of_the_design_from_border_to_border() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "File");
    let panel = menu_panel(&h);
    // `menu.separator` (la fila blanca sobre la (189,164,153)), de borde a borde: encima de los
    // biseles, adentro de la línea.
    let mut lines: Vec<Rect> = Vec::new();
    for r in h.drawn("menu.separator") {
        match lines.iter_mut().find(|l| l.top() == r.top()) {
            Some(l) => *l = l.union(r),
            None => lines.push(r),
        }
    }
    // El menú File tiene tres separadores.
    assert_eq!(lines.len(), 3, "{lines:?}");
    for line in lines {
        assert_eq!(
            line.x_range(),
            egui::Rangef::new(panel.left() + 1.0, panel.right() - 1.0)
        );
        assert_eq!(line.height(), 2.0);
    }
}

#[test]
fn closed_titles_underline_their_first_letter_and_the_open_one_does_not() {
    let mut h = two_tabs(Some(0));
    let offset = caps_offset(&h);
    let color = h.app.pixel.skin().unwrap().colors().home_text;
    let underline = |h: &LayoutHarness, title: &str| {
        let text = text_rect(h, title).0;
        let line = Rect::from_min_size(
            Pos2::new(text.left(), text.top() + offset + 7.0),
            // La tinta de la primera letra: la V mide 5 y las otras mayúsculas 4.
            Vec2::new(if title.starts_with('V') { 5.0 } else { 4.0 }, 1.0),
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
    assert_eq!(h.drawn("menu.list"), vec![], "the menu closes");
}

#[test]
fn hovered_rows_are_pink_with_pink_bevels_and_disabled_ones_dim_text() {
    // En Home no hay documento: Save está deshabilitado.
    let mut h = two_tabs(None);
    open_menu(&mut h, "File");
    let colors = *h.app.pixel.skin().unwrap().colors();
    assert_eq!(text_rect(&h, "Save").1, colors.text_dim, "disabled");
    let panel = menu_panel(&h);
    // Sin puntos suspensivos (en ningún lado de la app). El de Open del menú (no el botón Open
    // de Pixnews, en Home, que va con su sombra).
    let open = h
        .text_rects
        .iter()
        .find(|(t, r, _)| t == "Open" && panel.contains_rect(*r))
        .expect("Open in the menu");
    assert_eq!(open.2, colors.text);
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
    // En el rosa oscuro, el bisel de la izquierda y, del atajo (Ctrl+O), el +: en la fila con el
    // puntero, el atajo va en ese rosa.
    let dark = pink(colors.timeline_selection_dark);
    assert_eq!(dark[0], bevel(panel.left() + 1.0));
    assert!(
        dark[1..].iter().all(|r| r.left() > panel.center().x),
        "only the shortcut besides the bevel: {dark:?}"
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
    let panel = menu_panel(&h);
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
    let panels = h.drawn("menu.list");
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
    let panel = menu_panel(&h);
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
        // La barra del UI kit (con el ≡ y el ○).
        assert!(!h.drawn("title.bar").is_empty(), "the bar");
    }
    // La barra de 14 y la fila blanca no cambian con el zoom.
    assert_eq!(bars, vec![15.0, 15.0]);

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
    // El primario arriba en sus 19×19 (el secundario empieza en la máscara: su cuadriculado
    // lo revisa `with_pixel_ui_a_transparent_secondary_shows_the_checker_behind`).
    let editor = &h.app.doc.editor;
    let fill = |v| crate::panels::to_color32(editor.resolve(v));
    let rects = h.colored_rects();
    let top = Rect::from_min_size(colors.min + Vec2::new(4.0, 4.0), Vec2::splat(19.0));
    assert!(rects.contains(&(fill(editor.primary), top)), "{top:?}");
    assert!(editor.is_mask(editor.secondary));
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
fn the_saved_palette_width_is_the_right_panels_width_from_the_first_frame() {
    for base in [pixel_ui(), ViewSettings::default()] {
        let view = ViewSettings {
            palette_width: Some(180.0),
            ..base
        };
        let h = LayoutHarness::new(view, true);
        assert_eq!(h.panel("palette").width(), 180.0);
        assert_eq!(h.app.view.palette_width, Some(180.0));
    }
}

#[test]
fn dragging_the_right_panel_edge_saves_its_width() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let panel = h.panel("palette");
    let edge = Pos2::new(panel.left(), panel.center().y);
    drag(&mut h, edge, edge + Vec2::new(-30.0, 0.0));
    let wide = h.panel("palette").width();
    assert_eq!(wide, panel.width() + 30.0);
    assert_eq!(h.app.view.palette_width, Some(wide));
    let mut storage = MemoryStorage::default();
    eframe::App::save(&mut h.app, &mut storage);
    let saved: ViewSettings = eframe::get_value(&storage, crate::VIEW_KEY).expect("saved");
    assert_eq!(saved.palette_width, Some(wide));
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
    // El cuerpo de 23×23 (la línea clara va debajo), como en el mockup de componentes.
    assert_eq!(dock.size(), Vec2::new(23.0, 23.0));
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
    assert_eq!(button(&h).size(), Vec2::new(23.0, 23.0));
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

#[test]
fn pointing_at_colors_shows_them_in_a_tab_by_the_picker() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // La caja entera: la unión de las partes del 9-slice.
    let sv_box = h
        .drawn("picker.sv")
        .into_iter()
        .fold(Rect::NOTHING, |a, b| a.union(b));
    // El borde oscuro de arriba de la pestañita: 13 a la izquierda de la caja, 2 abajo.
    let border = Rect::from_min_size(sv_box.min + Vec2::new(-12.0, 2.0), Vec2::new(11.0, 1.0));
    let badge = |h: &LayoutHarness| {
        h.colored_rects()
            .iter()
            .any(|(c, r)| *r == border && *c == egui::Color32::from_rgb(89, 77, 77))
    };
    assert!(!badge(&h), "not without the pointer");
    h.frame_with(vec![Event::PointerMoved(sv_box.center())]);
    assert!(badge(&h), "over the S/V square");
    h.frame_with(vec![Event::PointerMoved(h.node("#3").center())]);
    h.frame();
    assert!(badge(&h), "over a color of the palette");
}

#[test]
fn with_a_menu_open_hovering_another_title_switches_to_it() {
    let mut h = two_tabs(Some(0));
    let has = |h: &LayoutHarness, t: &str| h.texts.iter().any(|(s, _)| s == t);
    // Sin menú abierto, pasar por un título no abre nada.
    h.frame_with(vec![Event::PointerMoved(h.button("Edit").center())]);
    h.frame();
    assert!(!has(&h, "Copy Merged"), "nothing open");
    open_menu(&mut h, "File");
    assert!(has(&h, "Export"), "File open");
    // El menú dibuja "…" como "...".
    assert!(has(&h, "Export Tiled Map...") && has(&h, "Import Tiled Map..."));
    h.frame_with(vec![Event::PointerMoved(h.button("Edit").center())]);
    for _ in 0..3 {
        h.frame();
    }
    assert!(has(&h, "Copy Merged"), "Edit opened by hovering");
    assert!(!has(&h, "Export"), "File closed");
}

#[test]
fn named_keys_are_badges_in_chimpfont() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "Edit");
    // Delete (Del): la tecla estirada, del alto de Ctrl y Shift, con DEL en mayúsculas.
    assert!(h.texts.iter().any(|(t, _)| t == "DEL"), "{:?}", h.texts);
    // Todas las badges, del mismo alto.
    let keys = h.drawn("key");
    assert!(!keys.is_empty());
    let rows: Vec<f32> = keys.iter().map(|k| k.top()).collect();
    for top in rows {
        let badge = keys
            .iter()
            .filter(|k| k.top() == top)
            .fold(Rect::NOTHING, |a, b| a.union(*b));
        assert!(badge.height() <= 7.0, "{badge:?}");
    }
}

#[test]
fn the_edit_menu_is_aseprites() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "Edit");
    let order = [
        "Undo",
        "Redo",
        "Cut",
        "Copy",
        "Copy Merged",
        "Paste",
        "Paste Special",
        "Delete",
        "Fill",
        "Stroke",
        "Rotate",
        "Flip Horizontal",
        "Flip Vertical",
        "Transform",
        "Shift",
        "New Artboard From Selection",
        "Invert",
        "Insert Text",
    ];
    let y = |label: &str| text_rect(&h, label).0.top();
    for pair in order.windows(2) {
        assert!(y(pair[0]) < y(pair[1]), "{} before {}", pair[0], pair[1]);
    }
}

/// Aprieta `key` con `modifiers` un frame (y la suelta en el siguiente).
fn press(h: &mut LayoutHarness, key: egui::Key, modifiers: egui::Modifiers) {
    let event = |pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    };
    h.frame_with(vec![event(true)]);
    h.frame_with(vec![event(false)]);
}

#[test]
fn a_reassigned_shortcut_does_its_action_and_the_old_keys_no_longer_do() {
    use crate::ToolKind;
    use crate::keymap::{Action, Combo};
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(!h.app.doc.editor.playing);
    h.app
        .keymap
        .assign(Action::PlayPause, Combo::new(egui::Key::P));
    press(&mut h, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!h.app.doc.editor.playing, "Enter is not Play anymore");
    press(&mut h, egui::Key::P, egui::Modifiers::NONE);
    assert!(h.app.doc.editor.playing, "P is");
    // Una herramienta: su letra nueva.
    h.app
        .keymap
        .assign(Action::Tool(ToolKind::Eraser), Combo::new(egui::Key::J));
    press(&mut h, egui::Key::J, egui::Modifiers::NONE);
    assert_eq!(h.app.doc.editor.tool, ToolKind::Eraser);
    // Se guarda y vuelve igual.
    let saved = h.app.keymap.settings();
    assert_eq!(crate::keymap::Keymap::from_settings(&saved), h.app.keymap);
}

#[test]
fn ctrl_alt_n_makes_an_artboard_from_the_selection_and_not_a_new_document() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let tabs = h.app.tabs.len();
    let artboards = h.app.doc.editor.document.artboards.len();
    h.app.doc.editor.select_all();
    h.frame_with(vec![Event::Key {
        key: egui::Key::N,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND | egui::Modifiers::ALT,
    }]);
    assert_eq!(h.app.tabs.len(), tabs, "no new document");
    assert_eq!(h.app.doc.editor.document.artboards.len(), artboards + 1);
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

/// La app con Pixel UI y un símbolo "Coin" en la biblioteca, elegido.
fn with_a_symbol(pixel: bool) -> LayoutHarness {
    let view = if pixel { pixel_ui() } else { plain() };
    let mut h = LayoutHarness::new(view, true);
    let editor = &mut h.app.doc.editor;
    let mut coin = editor.document.artboards[0].clone();
    coin.id = editor.document.next_artboard_id();
    coin.name = "Coin".into();
    coin.in_library = true;
    let id = coin.id;
    let index = editor.document.artboards.len();
    assert!(editor.execute_doc(cs_commands::DocCommand::AddArtboard {
        index,
        artboard: Box::new(coin),
    }));
    editor.library_selected = Some(id);
    h.frame();
    h
}

#[test]
fn the_library_dock_button_shows_the_symbols() {
    for pixel in [true, false] {
        let mut h = with_a_symbol(pixel);
        assert!(!h.texts.iter().any(|(t, _)| t == "Coin"));
        // Sin Pixel UI, el botón se llama como su glifo (el de más arriba: el otro "▣" es el del
        // preview, abajo).
        let button = if pixel {
            h.button("Library")
        } else {
            h.buttons
                .iter()
                .filter(|(l, _)| l == "▣")
                .map(|(_, r)| *r)
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("the Library dock button")
        };
        h.click(button.center(), PointerButton::Primary);
        h.frame();
        assert!(
            h.texts.iter().any(|(t, _)| t == "Coin"),
            "pixel {pixel}: {:?}",
            h.texts
        );
    }
}

#[test]
fn the_library_button_shows_its_plain_glyph_while_the_skin_lacks_the_icon() {
    let h = with_a_symbol(true);
    let has_icon = h
        .app
        .pixel
        .skin()
        .is_some_and(|s| s.piece("icon.dock.library", WidgetState::Normal).is_some());
    let button = h.button("Library");
    let glyph = h
        .texts
        .iter()
        .any(|(t, p)| t == "▣" && button.expand(1.0).contains(*p));
    assert_eq!(glyph, !has_icon, "{:?}", h.texts);
}

#[test]
fn f2_renames_the_selected_symbol_in_the_library() {
    let mut h = with_a_symbol(true);
    h.click(h.button("Library").center(), PointerButton::Primary);
    let key = |key, pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    h.frame_with(vec![key(egui::Key::F2, true), key(egui::Key::F2, false)]);
    h.frame();
    h.frame_with(vec![Event::Text("s".into())]);
    h.frame_with(vec![
        key(egui::Key::Enter, true),
        key(egui::Key::Enter, false),
    ]);
    h.frame();
    let id = h.app.doc.editor.library_selected.unwrap();
    let name = &h.app.doc.editor.document.artboard(id).unwrap().name;
    assert_eq!(name, "Coins");
}

/// Agrega a la biblioteca un símbolo `name` (copia del primer artboard) y lo elige.
fn add_symbol(h: &mut LayoutHarness, name: &str) -> cs_core::ArtboardId {
    let editor = &mut h.app.doc.editor;
    let mut symbol = editor.document.artboards[0].clone();
    symbol.id = editor.document.next_artboard_id();
    symbol.name = name.into();
    symbol.in_library = true;
    let id = symbol.id;
    let index = editor.document.artboards.len();
    assert!(editor.execute_doc(cs_commands::DocCommand::AddArtboard {
        index,
        artboard: Box::new(symbol),
    }));
    editor.library_selected = Some(id);
    id
}

fn key_tap(key: egui::Key) -> Vec<Event> {
    let event = |pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Default::default(),
    };
    vec![event(true), event(false)]
}

#[test]
fn a_rename_of_a_symbol_that_was_undone_away_does_not_block_f2() {
    let mut h = with_a_symbol(true);
    let gem = add_symbol(&mut h, "Gem");
    h.frame();
    h.click(h.button("Library").center(), PointerButton::Primary);
    h.frame_with(key_tap(egui::Key::F2));
    h.frame();
    assert_eq!(h.app.doc.library.renaming.as_ref().map(|r| r.0), Some(gem));
    // El undo saca a Gem de la biblioteca mientras se renombraba.
    h.app.doc.editor.undo();
    h.frame();
    h.frame();
    assert_eq!(h.app.doc.library.renaming, None);
    // F2 sigue andando con Coin.
    let coin = h.app.doc.editor.document.library().next().unwrap().id;
    h.app.doc.editor.library_selected = Some(coin);
    h.frame();
    h.frame_with(key_tap(egui::Key::F2));
    h.frame();
    h.frame_with(vec![Event::Text("s".into())]);
    h.frame_with(key_tap(egui::Key::Enter));
    h.frame();
    let name = &h.app.doc.editor.document.artboard(coin).unwrap().name;
    assert_eq!(name, "Coins");
}

#[test]
fn the_library_state_belongs_to_its_tab() {
    let mut h = with_a_symbol(true);
    h.click(h.button("Library").center(), PointerButton::Primary);
    h.frame_with(key_tap(egui::Key::F2));
    h.frame();
    assert!(h.app.doc.library.renaming.is_some());
    // Otra pestaña con su propio símbolo: empieza sin nada a medias.
    h.app.new_document();
    h.frame();
    assert_eq!(h.app.tabs.len(), 2);
    assert_eq!(h.app.doc.library.renaming, None);
    add_symbol(&mut h, "Other");
    h.frame();
    assert!(h.texts.iter().any(|(t, _)| t == "Other"), "{:?}", h.texts);
    assert!(!h.texts.iter().any(|(t, _)| t == "Coin"));
    // De vuelta en la primera, su panel sigue como estaba.
    h.app.tabs.switch(&mut h.app.doc, 0);
    h.frame();
    assert!(h.app.doc.library.renaming.is_some());
}

#[test]
fn editing_in_place_draws_the_symbol_where_its_instance_is_over_the_dimmed_scene() {
    for pixel in [true, false] {
        let view = if pixel { pixel_ui() } else { plain() };
        let mut h = LayoutHarness::new(view, true);
        let editor = &mut h.app.doc.editor;
        let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
        editor.execute(cs_commands::Command::DrawPixels {
            layer: editor.layer,
            frame: 0,
            pixels: vec![(4, 6, red), (7, 8, red)],
        });
        let coin = editor
            .convert_to_symbol("Coin", cs_commands::symbol::SymbolTiming::Graphic)
            .unwrap();
        let host = editor.active;
        let (layer, object) = (editor.layer, editor.current_object().unwrap());
        h.frame();
        h.frame();
        let zoom = h.app.doc.canvas.zoom;
        let host_texture = h.app.doc.canvas.artboard_texture(host).expect("rendered");
        let drawn = |h: &LayoutHarness, t: TextureId| {
            h.textures.iter().find(|(x, _)| *x == t).map(|(_, r)| *r)
        };
        let host_rect = drawn(&h, host_texture).expect("the host is drawn");
        assert!(h.app.doc.editor.enter_symbol_in_place(layer, object));
        h.frame();
        h.frame();
        assert_eq!(h.app.doc.canvas.zoom, zoom, "pixel {pixel}: the view stays");
        // El host ya no se dibuja como artboard: su contexto (atenuado) sí.
        assert!(drawn(&h, host_texture).is_none(), "pixel {pixel}");
        let context = h.app.doc.canvas.context_textures();
        assert_eq!(context.len(), 1, "pixel {pixel}");
        let context_rect = drawn(&h, context[0]).expect("the dimmed scene");
        assert!(
            (context_rect.min - host_rect.min).length() < 0.5,
            "pixel {pixel}"
        );
        // El símbolo, justo donde estaba la instancia (4, 6) del host.
        let coin_texture = h.app.doc.canvas.artboard_texture(coin).expect("rendered");
        let coin_rect = drawn(&h, coin_texture).expect("the symbol is drawn");
        let width = h
            .app
            .doc
            .editor
            .document
            .artboard(host)
            .unwrap()
            .sprite
            .width;
        let scale = host_rect.width() / width as f32;
        let expected = host_rect.min + Vec2::new(4.0, 6.0) * scale;
        assert!(
            (coin_rect.min - expected).length() < 0.5,
            "pixel {pixel}: {coin_rect:?} vs {expected:?}"
        );
        assert!(h.texts.iter().any(|(t, _)| t == "Scene"), "pixel {pixel}");
        // Esc sale y el host vuelve.
        h.app.doc.editor.escape();
        h.frame();
        assert!(drawn(&h, host_texture).is_some(), "pixel {pixel}");
        assert!(h.app.doc.canvas.context_textures().is_empty());
    }
}

#[test]
fn symbol_mode_shows_only_the_symbol_under_a_breadcrumb_that_leaves_it() {
    for pixel in [true, false] {
        let mut h = with_a_symbol(pixel);
        let host = h.app.doc.editor.active;
        let coin = h.app.doc.editor.library_selected.unwrap();
        let zoom = h.app.doc.canvas.zoom;
        assert!(!h.texts.iter().any(|(t, _)| t == "Scene"));
        let host_texture = h.app.doc.canvas.artboard_texture(host).expect("rendered");
        assert!(h.app.doc.editor.enter_symbol(coin));
        h.app.doc.canvas.request_zoom(crate::view::ZoomRequest::Fit);
        h.frame();
        h.frame();
        // Se dibuja solo el símbolo (el host está en el mismo lugar, pero no se ve).
        let coin_texture = h.app.doc.canvas.artboard_texture(coin).expect("rendered");
        let drawn = |t: TextureId| h.textures.iter().any(|(x, _)| *x == t);
        assert!(drawn(coin_texture), "pixel {pixel}");
        assert!(!drawn(host_texture), "pixel {pixel}: only the symbol");
        let crumb = |h: &LayoutHarness, name: &str| {
            h.text_rects
                .iter()
                .find(|(t, ..)| t == name)
                .map(|(_, r, _)| *r)
        };
        assert!(crumb(&h, "Coin").is_some(), "{:?}", h.texts);
        let document = crumb(&h, "Scene").expect("the breadcrumb");
        // Un clic en "Scene" vuelve al documento, con su vista de antes.
        h.click(document.center(), PointerButton::Primary);
        assert!(!h.app.doc.editor.in_symbol_mode(), "pixel {pixel}");
        assert_eq!(h.app.doc.editor.active, host);
        h.frame();
        assert!(crumb(&h, "Scene").is_none());
        assert_eq!(h.app.doc.canvas.zoom, zoom, "the document view is back");
    }
}

#[test]
fn move_to_library_is_in_the_sprite_menu_without_pixel_ui() {
    let mut h = LayoutHarness::editor();
    open_menu(&mut h, "Sprite");
    // Con un solo artboard en el lienzo, no se puede.
    h.click(h.button("Move to Library").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.document.canvas_count(), 1);
    h.app.doc.editor.add_artboard(4, 4);
    let moved = h.app.doc.editor.active;
    h.frame();
    if !h.buttons.iter().any(|(l, _)| l == "Move to Library") {
        open_menu(&mut h, "Sprite");
    }
    h.click(h.button("Move to Library").center(), PointerButton::Primary);
    let editor = &h.app.doc.editor;
    assert!(editor.document.artboard(moved).unwrap().in_library);
    assert_ne!(editor.active, moved);
}

#[test]
fn deleting_a_symbol_in_use_asks_first() {
    for pixel in [true, false] {
        let mut h = with_a_symbol(pixel);
        let coin = h.app.doc.editor.library_selected.unwrap();
        // Una instancia de Coin en el host.
        let editor = &mut h.app.doc.editor;
        let add =
            cs_commands::ops::new_object_layer(editor.sprite(), None, 0, coin, "Coin 1", (0, 0));
        assert!(editor.execute(add));
        h.frame();
        let library = if pixel {
            h.button("Library")
        } else {
            h.buttons
                .iter()
                .filter(|(l, _)| l == "▣")
                .map(|(_, r)| *r)
                .min_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("the Library dock button")
        };
        h.click(library.center(), PointerButton::Primary);
        h.frame();
        let question = "Used by 1 object. Delete anyway?";
        // Pide borrar desde el menú de la fila y contesta `answer` en la confirmación.
        let ask = |h: &mut LayoutHarness, answer: &str| {
            let row = h
                .nodes
                .iter()
                .filter(|(l, _)| l == "Coin")
                .map(|(_, r)| *r)
                .max_by(|a, b| a.top().total_cmp(&b.top()))
                .expect("the Coin row");
            h.click(row.center(), PointerButton::Secondary);
            h.frame();
            h.click(h.button("Delete").center(), PointerButton::Primary);
            h.frame();
            let (_, text, _) = h
                .text_rects
                .iter()
                .find(|(t, _, _)| t == question)
                .cloned()
                .unwrap_or_else(|| panic!("pixel {pixel}: no confirmation in {:?}", h.texts));
            let button = h
                .buttons
                .iter()
                .filter(|(l, r)| l == answer && r.top() > text.top())
                .map(|(_, r)| *r)
                .next()
                .unwrap_or_else(|| panic!("no {answer:?} under the question"));
            h.click(button.center(), PointerButton::Primary);
            h.frame();
        };
        ask(&mut h, "Cancel");
        assert!(h.app.doc.editor.document.artboard(coin).is_some(), "kept");
        assert_eq!(h.app.doc.library.confirm_delete, None);
        assert!(!h.texts.iter().any(|(t, _)| t == question));
        ask(&mut h, "Delete");
        assert!(
            h.app.doc.editor.document.artboard(coin).is_none(),
            "pixel {pixel}: deleted"
        );
    }
}

/// Un documento con una capa de objetos con dos objetos (`a`, `b`: `b` arriba) y la capa
/// activa, con Move.
fn with_two_objects() -> (LayoutHarness, LayerId, [cs_core::ObjectId; 2]) {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.add_artboard(4, 4);
    assert!(e.move_active_to_library());
    e.activate(cs_core::ArtboardId(1));
    e.add_instance_layer(cs_core::ArtboardId(2));
    let layer = e.layer;
    let (cmd, b) =
        cs_commands::ops::add_object(e.sprite(), layer, 0, cs_core::ArtboardId(2), "o", (0, 0))
            .unwrap();
    e.execute(cmd);
    let a = e.sprite().layer(layer).unwrap().objects[0].id;
    // El objeto nuevo queda seleccionado: se arranca sin selección.
    e.clear_object_selection();
    e.set_tool(ToolKind::Move);
    h.frame();
    (h, layer, [a, b])
}

fn object_order(h: &LayoutHarness, layer: LayerId) -> Vec<cs_core::ObjectId> {
    let sprite = h.app.doc.editor.sprite();
    sprite
        .layer(layer)
        .unwrap()
        .objects
        .iter()
        .map(|o| o.id)
        .collect()
}

#[test]
fn object_shortcuts_arrange_duplicate_and_need_the_move_tool() {
    use egui::{Key, Modifiers};
    let (mut h, layer, [a, b]) = with_two_objects();
    let ctrl_shift = Modifiers::COMMAND | Modifiers::SHIFT;
    // Sin selección no hacen nada.
    press(&mut h, Key::ArrowDown, Modifiers::COMMAND);
    assert_eq!(object_order(&h, layer), vec![a, b]);
    h.app.doc.editor.select_object(layer, b, false);
    press(&mut h, Key::ArrowDown, Modifiers::COMMAND);
    assert_eq!(object_order(&h, layer), vec![b, a], "Send Backward");
    press(&mut h, Key::ArrowUp, Modifiers::COMMAND);
    assert_eq!(object_order(&h, layer), vec![a, b], "Bring Forward");
    press(&mut h, Key::ArrowDown, ctrl_shift);
    assert_eq!(object_order(&h, layer), vec![b, a], "Send to Back");
    press(&mut h, Key::ArrowUp, ctrl_shift);
    assert_eq!(object_order(&h, layer), vec![a, b], "Bring to Front");
    // Con otra herramienta, nada.
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    press(&mut h, Key::ArrowDown, ctrl_shift);
    press(&mut h, Key::J, Modifiers::COMMAND);
    assert_eq!(object_order(&h, layer), vec![a, b]);
    h.app.doc.editor.set_tool(ToolKind::Move);
    press(&mut h, Key::J, Modifiers::COMMAND);
    assert_eq!(object_order(&h, layer).len(), 3, "Duplicate Object");
    assert_eq!(h.app.doc.editor.selected_objects().len(), 1);
    assert_ne!(
        h.app.doc.editor.selected_objects()[0],
        b,
        "the copy is selected"
    );
}

#[test]
fn the_arrange_submenu_is_disabled_without_selected_objects() {
    let (mut h, _, _) = with_two_objects();
    open_menu(&mut h, "Layer");
    let colors = *h.app.pixel.skin().unwrap().colors();
    assert_eq!(
        text_rect(&h, "Arrange").1,
        colors.text_dim,
        "nothing selected"
    );
    let (mut h, layer, [_, b]) = with_two_objects();
    h.app.doc.editor.select_object(layer, b, false);
    open_menu(&mut h, "Layer");
    assert_eq!(
        text_rect(&h, "Arrange").1,
        colors.text,
        "an object is selected"
    );
}

// El panel Object.

#[test]
fn the_object_panel_flips_every_selected_object_in_one_undo() {
    let (mut h, layer, [a, b]) = with_two_objects();
    h.app.doc.editor.set_selected_objects(vec![a, b]);
    h.frame();
    assert!(h.texts.iter().any(|t| t.0 == "Object"), "{:?}", h.texts);
    assert!(h.texts.iter().any(|t| t.0 == "2 objects selected"));
    let (_, check, _) = h.checks.iter().find(|c| c.0 == "Flip H").unwrap().clone();
    h.click(check.center(), PointerButton::Primary);
    let flips = |h: &LayoutHarness| -> Vec<bool> {
        let e = &h.app.doc.editor;
        let objects = &e.sprite().layer(layer).unwrap().objects;
        objects
            .iter()
            .map(|o| o.track.key_at(e.frame).unwrap().flip_h)
            .collect()
    };
    assert_eq!(flips(&h), [true, true]);
    h.app.doc.editor.undo();
    assert_eq!(flips(&h), [false, false]);
}

#[test]
fn an_empty_object_layer_asks_for_an_object_and_image_layers_show_no_panel() {
    let (mut h, layer, [a, b]) = with_two_objects();
    let e = &mut h.app.doc.editor;
    let cmd = cs_commands::ops::remove_objects(e.sprite(), layer, &[a, b]).unwrap();
    e.execute(cmd);
    h.frame();
    assert!(
        h.texts
            .iter()
            .any(|t| t.0 == "Select an object on the canvas")
    );
    h.app.doc.editor.select_layer(LayerId(1));
    h.frame();
    assert!(!h.texts.iter().any(|t| t.0 == "Object"));
}

#[test]
fn help_ui_catalog_opens_a_closable_tab_with_the_four_button_states() {
    let mut h = two_tabs(Some(0));
    open_menu(&mut h, "Help");
    let item = h.button("UI Catalog");
    h.click(item.center(), PointerButton::Primary);
    h.frame();
    // Una pestaña más, a la vista, sin documento activo.
    assert_eq!(h.app.tabs.page(), Some(crate::tabs::Page::Catalog));
    assert_eq!(h.app.tabs.active(), None);
    assert_eq!(h.app.tabs.bar_active(&h.app.doc), Some(2));
    assert!(h.texts.iter().any(|(t, _)| t == "UI Catalog"), "the tab");
    for state in ["Normal", "Hover", "Pressed", "Disabled"] {
        assert!(h.texts.iter().any(|(t, _)| t == state), "{state}");
    }
    // Cada estado con su pieza, de 17 de alto.
    for piece in ["button", "button.hover", "button.down", "button.disabled"] {
        let boxes = h.drawn_boxes(piece);
        assert!(!boxes.is_empty(), "{piece}");
        assert!(
            boxes.iter().all(|b| b.height() == 17.0),
            "{piece} {boxes:?}"
        );
    }
    // El botón de verdad cuenta los clics.
    let live = h.button("Click me");
    h.click(live.center(), PointerButton::Primary);
    h.frame();
    assert!(
        h.texts.iter().any(|(t, _)| t == "Clicked 1×"),
        "{:?}",
        h.texts
    );
    // Se cierra como cualquier pestaña: queda Home y los documentos siguen.
    h.app
        .tab_action(crate::tabs::TabRef::Page(crate::tabs::Page::Catalog), true);
    h.frame();
    assert_eq!(h.app.tabs.page(), None);
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2);
}

#[test]
fn a_font_artboard_shows_its_glyph_in_the_title_and_hides_the_timeline() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let with_timeline = h.canvas.expect("a document is open");
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.app.doc.editor.go_to_glyph('A');
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    assert!(
        h.texts.iter().any(|(t, _)| t.contains("A · Mine")),
        "{:?}",
        h.texts
    );
    // Sin la timeline, el lienzo baja hasta donde estaba ella.
    let canvas = h.canvas.expect("a document is open");
    assert!(
        canvas.bottom() > with_timeline.bottom() + 50.0,
        "{canvas:?} vs {with_timeline:?}"
    );
    assert!(
        h.app.view.timeline,
        "the setting stays: it comes back for sprites"
    );
}

#[test]
fn dragging_the_advance_line_sets_a_manual_advance() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    e.go_to_glyph('A');
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    assert_eq!(
        h.app.doc.editor.glyph_advance(),
        Some(8),
        "empty: half the cell"
    );
    let line = h
        .app
        .doc
        .canvas
        .advance_line_rect()
        .expect("the advance line");
    let scale = h.app.doc.canvas.scale().x;
    h.drag(line.center(), line.center() + egui::vec2(3.0 * scale, 0.0));
    let e = &h.app.doc.editor;
    assert_eq!(e.glyph_advance(), Some(8 + 3));
    assert_eq!(e.font().unwrap().glyphs[e.frame].advance, Some(11));
    // La herramienta no recibió el clic: el glifo sigue vacío.
    assert!(
        e.sprite().cel(e.layer, e.frame).is_none(),
        "nothing painted"
    );
    let moved = h
        .app
        .doc
        .canvas
        .advance_line_rect()
        .expect("the advance line");
    assert_eq!(moved.center().x, line.center().x + 3.0 * scale);
}

#[test]
fn the_advance_drag_sets_the_glyph_where_it_started() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    e.go_to_glyph('A');
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    let line = h.app.doc.canvas.advance_line_rect().expect("the line");
    let scale = h.app.doc.canvas.scale().x;
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let to = line.center() + egui::vec2(3.0 * scale, 0.0);
    h.frame_with(vec![Event::PointerMoved(line.center())]);
    h.frame_with(vec![button(line.center(), true)]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    // A mitad del arrastre cambia el glifo actual (un atajo, el panel…).
    h.app.doc.editor.go_to_glyph('B');
    h.frame_with(vec![button(to, false)]);
    h.frame();
    let font = h.app.doc.editor.font().unwrap();
    assert_eq!(font.glyphs[font.index_of('A').unwrap()].advance, Some(11));
    assert_eq!(font.glyphs[font.index_of('B').unwrap()].advance, None);
}

#[test]
fn a_monospace_font_has_no_advance_line_to_drag() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    let mut info = e.font().unwrap().clone();
    info.monospace = true;
    e.set_font_info(info);
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    assert!(h.app.doc.canvas.advance_line_rect().is_none());
}

/// Un documento con su artboard normal y una fuente de 16×16 activa, centrada a 800 %.
fn with_a_font_at_800() -> (LayoutHarness, cs_core::ArtboardId, cs_core::ArtboardId) {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let normal = h.app.doc.editor.artboard().id;
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    e.go_to_glyph('A');
    let font = e.artboard().id;
    h.app.doc.canvas.zoom = 8.0;
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    (h, normal, font)
}

/// A `zoom`, con el artboard activo centrado.
fn zoom_centered(h: &mut LayoutHarness, zoom: f32) {
    h.app.doc.canvas.zoom = zoom;
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
}

#[test]
fn the_font_workspace_draws_only_its_artboard_and_painting_is_one_undo_step() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let normal = h.app.doc.editor.artboard().id;
    let ctx = h.ctx.clone();
    assert!(
        h.app
            .create_font(&ctx, "Mine", (16, 16), cs_core::FontMode::OneBit)
    );
    h.app.doc.editor.go_to_glyph('A');
    h.frame();
    h.frame();
    h.frame();
    // El lienzo dibuja solo la fuente: el artboard normal no se ve.
    let font = h.app.doc.editor.workspace().expect("the font workspace");
    assert!(
        union(&h.drawn("title.bar.font")).is_some(),
        "the font window"
    );
    assert!(h.drawn("title.bar").is_empty(), "no normal artboard");
    if let Some(tex) = h.app.doc.canvas.artboard_texture(normal) {
        assert!(h.textures.iter().all(|(t, _)| *t != tex), "hidden");
    }
    // Pintar un pixel cambia el glifo y es un solo paso de undo.
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = cs_core::ONE_BIT_INK;
    let steps = h.app.doc.editor.history.done_count();
    // Lejos de la línea del ancho (a media celda), que se lleva el clic.
    let art = artboard_pixels(&h);
    let at = art.left_top() + art.size() * 0.25;
    h.click(at, PointerButton::Primary);
    let e = &h.app.doc.editor;
    assert_eq!(e.artboard().id, font);
    assert!(e.sprite().cel(e.layer, e.frame).is_some(), "painted");
    assert_eq!(e.history.done_count(), steps + 1);
    // Desde la pestaña del documento se deshace.
    h.app.tabs.switch(&mut h.app.doc, 0);
    h.app.doc.editor.undo();
    h.app.doc.editor.enter_workspace(font);
    let e = &h.app.doc.editor;
    assert!(e.sprite().cel(e.layer, e.frame).is_none(), "undone");
}

#[test]
fn a_font_artboard_has_the_orange_bar_and_a_normal_one_the_pink_one() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.frame();
    assert!(!h.drawn("title.bar").is_empty(), "the normal artboard");
    assert!(h.drawn("title.bar.font").is_empty(), "no font yet");
    let (h, _, _) = with_a_font_at_800();
    let pixels = artboard_pixels(&h);
    let bar = union(&h.drawn("title.bar.font")).expect("the orange bar");
    // La barra de la ventana de la fuente: del borde izquierdo al derecho, sobre la fila blanca.
    assert_eq!(bar.left(), pixels.left() - 2.0);
    assert_eq!(bar.right(), pixels.right() + 2.0);
    assert_eq!(bar.bottom(), pixels.top() - 1.0);
    assert!(
        h.drawn("title.bar").iter().all(|r| !r.intersects(bar)),
        "the pink bar is not under it"
    );
}

#[test]
fn zoomed_far_out_a_font_artboard_is_a_card_with_a_sample() {
    let (mut h, _, font) = with_a_font_at_800();
    assert!(h.app.doc.canvas.advance_line_rect().is_some());
    zoom_centered(&mut h, 0.5);
    let tex = h.app.doc.canvas.artboard_texture(font).expect("rendered");
    assert!(
        h.textures.iter().all(|(t, _)| *t != tex),
        "the card has no glyph pixels"
    );
    assert!(h.app.doc.canvas.advance_line_rect().is_none(), "no overlay");
    let bar = union(&h.drawn("title.bar.font")).expect("the card's orange bar");
    // El marco de la tarjeta (el del artboard normal queda aparte, a la izquierda).
    let frames: Vec<Rect> = h
        .drawn("artboard.frame")
        .into_iter()
        .filter(|r| r.left() >= bar.left() && r.right() <= bar.right())
        .collect();
    let card = union(&frames).expect("the card frame");
    assert_eq!(bar.left_top(), card.left_top());
    assert_eq!(bar.width(), card.width());
    // Más grande que la ventana del glifo a ese zoom (16×16 a 50 %: 12×27).
    assert!(card.width() > 12.0 && card.height() > 27.0, "{card:?}");
    // De vuelta a 800 %, la ventana del glifo.
    zoom_centered(&mut h, 8.0);
    assert!(h.textures.iter().any(|(t, _)| *t == tex));
    assert!(h.app.doc.canvas.advance_line_rect().is_some());
}

#[test]
fn a_guide_dragged_from_the_ruler_can_be_dropped_on_the_advance_line() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.app.doc.editor.go_to_glyph('A');
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
    // Con el puntero ya sobre el lienzo (el diseño se acomoda al entrar).
    let canvas = h.canvas.expect("a document is open");
    let from = egui::pos2(canvas.left() + 5.0, canvas.center().y);
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame();
    let line = h
        .app
        .doc
        .canvas
        .advance_line_rect()
        .expect("the advance line");
    assert_eq!(h.app.doc.editor.glyph_advance(), Some(8));
    // Desde la regla izquierda: al empezar el arrastre el diseño se acomoda, así que se lee la
    // línea de nuevo antes de soltarla justo encima.
    let from = egui::pos2(from.x, line.center().y);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    let mid = egui::pos2(from.x + 40.0, from.y);
    h.frame_with(vec![Event::PointerMoved(mid)]);
    h.frame();
    let line = h
        .app
        .doc
        .canvas
        .advance_line_rect()
        .expect("the advance line");
    let to = line.center();
    for k in 1..=4 {
        h.frame_with(vec![Event::PointerMoved(mid.lerp(to, k as f32 / 4.0))]);
    }
    h.frame_with(vec![button(to, false)]);
    h.frame();
    let e = &h.app.doc.editor;
    let guides = e.guides();
    assert_eq!(guides.len(), 1, "{guides:?}");
    assert_eq!(guides[0].axis, cs_core::Axis::Vertical);
    assert_eq!(guides[0].position, 8, "the column under the drop");
    // La línea del ancho no se quedó con el arrastre ni con la suelta.
    assert_eq!(e.glyph_advance(), Some(8));
    assert_eq!(e.font().unwrap().glyphs[e.frame].advance, None);
}

#[test]
fn the_timeline_comes_back_when_a_sprite_artboard_is_activated() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let with_timeline = h.canvas.expect("a document is open");
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.app.doc.editor.go_to_glyph('A');
    h.frame();
    h.frame();
    let font_canvas = h.canvas.expect("a document is open");
    assert!(font_canvas.bottom() > with_timeline.bottom() + 50.0);
    h.app.doc.editor.activate(cs_core::ArtboardId(1));
    h.frame();
    h.frame();
    let back = h.canvas.expect("a document is open");
    assert_eq!(back.bottom(), with_timeline.bottom(), "{back:?}");
}

#[test]
fn the_font_panel_offers_new_font_and_its_map_picks_glyphs() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // El botón FontForge del dock, con su ícono del skin.
    let button = h.button("FontForge");
    assert!(
        h.drawn("icon.dock.font")
            .iter()
            .any(|r| button.contains_rect(*r)),
        "the FontForge icon"
    );
    h.click(button.center(), PointerButton::Primary);
    h.frame();
    assert!(h.button("New Font…").width() > 0.0);
    h.app
        .doc
        .editor
        .new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.frame();
    h.click(h.button("Glyph B").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.current_glyph(), Some('B'));
    // Doble clic en un carácter que falta (página Latin-1) lo agrega.
    h.click(h.button("Latin-1").center(), PointerButton::Primary);
    let cell = h.button("Glyph á").center();
    h.double_click(cell);
    assert!(h.app.doc.editor.font().unwrap().index_of('á').is_some());
    assert_eq!(h.app.doc.editor.current_glyph(), Some('á'));
}

#[test]
fn new_font_in_the_font_panel_opens_the_dialog() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.click(h.button("New Font…").center(), PointerButton::Primary);
    assert!(h.app.new_font.is_some());
}

#[test]
fn a_new_font_is_fitted_beside_the_font_panel() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    // Lo que hace el OK de File → New Font… (el diálogo no se dibuja en este arnés).
    let ctx = h.ctx.clone();
    assert!(
        h.app
            .create_font(&ctx, "Mine", (8, 8), cs_core::FontMode::OneBit)
    );
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.doc.editor.font().is_some());
    let flyout = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("dock_flyout", "FontForge"))))
        .expect("the Font panel is open");
    let canvas = h.canvas.expect("a document is open");
    let art = artboard_pixels(&h);
    // Entera a la izquierda del panel y dentro del lienzo: no queda tapada.
    assert!(art.right() <= flyout.left(), "{art:?} vs {flyout:?}");
    assert!(canvas.contains_rect(art), "{art:?} vs {canvas:?}");
    // Ajustada: no se queda chiquita.
    assert!(art.width() >= 100.0, "{art:?}");
}

#[test]
fn switching_to_a_font_view_tab_opens_the_font_panel_beside_the_glyph() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let ctx = h.ctx.clone();
    assert!(
        h.app
            .create_font(&ctx, "Mine", (8, 8), cs_core::FontMode::OneBit)
    );
    for _ in 0..3 {
        h.frame();
    }
    // De vuelta al documento, con el panel cerrado.
    let doc = h.button(&h.app.doc.location.name.clone());
    h.click(doc.center(), PointerButton::Primary);
    crate::dock::set_open(&ctx, None);
    h.frame();
    assert_eq!(h.app.doc.editor.workspace(), None);
    let view = h.button("Mine");
    h.click(view.center(), PointerButton::Primary);
    for _ in 0..3 {
        h.frame();
    }
    let flyout = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("dock_flyout", "FontForge"))))
        .expect("the Font panel is open");
    let art = artboard_pixels(&h);
    assert!(art.right() <= flyout.left(), "{art:?} vs {flyout:?}");
    assert!(art.width() >= 100.0, "{art:?}");
}

#[test]
fn the_font_panel_adds_the_typed_chars() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .editor
        .new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    let field = h
        .nodes
        .iter()
        .find(|(l, _)| l == "Add char")
        .map(|(_, r)| *r)
        .expect("the Add char field");
    h.click(field.center(), PointerButton::Primary);
    h.frame_with(vec![Event::Text("ñ €".into())]);
    h.frame();
    h.click(h.button("Add").center(), PointerButton::Primary);
    let font = h.app.doc.editor.font().unwrap();
    assert!(font.index_of('ñ').is_some());
    assert!(font.index_of('€').is_some());
    assert!(h.app.font_panel.add_char.is_empty());
}

/// El panel FontForge abierto con una fuente de 8×8 en una ventana de `size`.
fn font_panel_open(size: Vec2) -> LayoutHarness {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, size);
    h.app
        .doc
        .editor
        .new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    h
}

/// La caja del panel FontForge, como la pinta el dock (en puntos enteros).
fn font_flyout(h: &LayoutHarness) -> Rect {
    let area = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("dock_flyout", "FontForge"))))
        .expect("the FontForge panel is open");
    Rect::from_min_size(area.min.round(), area.size().round())
}

/// Los rects llenos de un color (los `Shape::Rect` y las mallas sin textura).
fn filled_rects(h: &LayoutHarness) -> Vec<(egui::Color32, Rect)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(egui::Color32, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, out)),
            egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
            _ => out.extend(crate::pixel_ui::colored_rects(shape)),
        }
    }
    let mut out = Vec::new();
    for clipped in &h.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

#[test]
fn the_font_panel_keeps_its_logo_and_build_while_the_middle_scrolls() {
    let mut h = font_panel_open(Vec2::new(1200.0, 420.0));
    let flyout = font_flyout(&h);
    let logos = h.drawn("fontforge.logo");
    assert_eq!(logos.len(), 1, "{logos:?}");
    let logo = logos[0];
    assert_eq!(logo.min, flyout.min + vec2(8.0, 8.0), "{flyout:?}");
    let build = h.button("Build ▾");
    assert!(flyout.contains_rect(build), "{build:?} in {flyout:?}");
    assert_eq!(build.right(), flyout.right() - 8.0, "{flyout:?}");
    assert!(
        build.bottom() > flyout.bottom() - 20.0,
        "{build:?} {flyout:?}"
    );
    assert!(
        !h.disabled.iter().any(|l| l == "Build ▾"),
        "a font can be built"
    );
    let map_top = |h: &LayoutHarness| {
        h.text_rects
            .iter()
            .find(|(t, _, _)| t == "Map")
            .map(|(_, r, _)| r.top())
    };
    let before = map_top(&h).expect("the Map label");
    // La rueda sobre el mapa desplaza el medio.
    let over = h.button("Glyph A").center();
    h.frame_with(vec![Event::PointerMoved(over)]);
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -120.0),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    }]);
    for _ in 0..10 {
        h.frame();
    }
    assert!(
        map_top(&h).is_none_or(|top| top < before - 20.0),
        "the middle scrolled"
    );
    assert_eq!(h.drawn("fontforge.logo"), vec![logo]);
    assert_eq!(h.button("Build ▾"), build);
}

#[test]
fn the_header_icons_switch_between_the_map_and_the_font_settings() {
    let mut h = font_panel_open(Vec2::new(1200.0, 800.0));
    let flyout = font_flyout(&h);
    let has_text = |h: &LayoutHarness, t: &str| h.text_rects.iter().any(|(s, _, _)| s == t);
    let has_button = |h: &LayoutHarness, t: &str| h.buttons.iter().any(|(s, _)| s == t);
    assert!(has_button(&h, "Glyph A"));
    assert!(!has_text(&h, "Baseline"));
    let gear = h.button("Font Settings");
    let edit = h.button("Edit Glyphs");
    assert_eq!(gear.min, flyout.min + vec2(292.0, 9.0));
    assert_eq!(edit.min, flyout.min + vec2(309.0, 9.0));
    assert_eq!(h.drawn("icon.fontforge.config").len(), 1);
    assert_eq!(h.drawn("icon.fontforge.edit").len(), 1);
    h.click(gear.center(), PointerButton::Primary);
    assert!(has_text(&h, "Baseline"));
    assert!(!has_button(&h, "Glyph A"));
    h.click(edit.center(), PointerButton::Primary);
    assert!(has_button(&h, "Glyph A"));
    assert!(!has_text(&h, "Baseline"));
}

#[test]
fn without_pixel_ui_the_font_panel_falls_back_to_egui() {
    let mut h = LayoutHarness::editor();
    h.app
        .doc
        .editor
        .new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    crate::dock::set_open(&h.ctx, Some(crate::dock::DockPanel::Font));
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.texts.iter().any(|(t, _)| t == "FontForge"), "the title");
    h.click(h.button("Font Settings").center(), PointerButton::Primary);
    assert!(h.texts.iter().any(|(t, _)| t == "Baseline"));
    h.click(h.button("Edit Glyphs").center(), PointerButton::Primary);
    assert!(h.button("Glyph A").width() > 0.0);
    assert!(h.button("Build ▾").width() > 0.0);
}

#[test]
fn the_font_panel_without_a_font_has_no_view_icons() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    assert!(h.button("New Font…").width() > 0.0);
    assert_eq!(h.drawn("fontforge.logo").len(), 1);
    assert!(!h.buttons.iter().any(|(l, _)| l == "Font Settings"));
    assert!(h.button("Build ▾").width() > 0.0);
}

#[test]
fn the_font_panel_rows_are_the_mockup_rows() {
    let mut h = font_panel_open(Vec2::new(1200.0, 800.0));
    // Que termine de aparecer (el Area entra con fundido).
    for _ in 0..10 {
        h.frame();
    }
    let f = font_flyout(&h);
    let top = |r: Rect| r.top() - f.top();
    // Las líneas del título y del pie: blanca arriba y la del lienzo abajo.
    let colors = *h.app.pixel.skin().unwrap().colors();
    let rows: Vec<(egui::Color32, f32)> = filled_rects(&h)
        .into_iter()
        .filter(|(_, r)| r.height() == 1.0 && r.width() == f.width() - 2.0)
        .map(|(c, r)| (c, top(r)))
        .collect();
    let bottom = f.height();
    for want in [
        (egui::Color32::WHITE, 29.0),
        (colors.canvas, 30.0),
        (egui::Color32::WHITE, bottom - 33.0),
        (colors.canvas, bottom - 32.0),
    ] {
        assert!(rows.contains(&want), "{want:?} in {rows:?}");
    }
    // La cuadrícula desde la fila 85 (su línea), Add desde la 276, el papel del Test Drive
    // desde la 332 y Build a 25 del borde de abajo.
    assert_eq!(top(h.button("Glyph !")) - 1.0, 85.0);
    assert_eq!(top(h.button("Add")), 276.0);
    assert_eq!(top(h.button("Add")), top(h.button("Add All")));
    let paper = filled_rects(&h)
        .into_iter()
        .find(|(c, r)| *c == egui::Color32::WHITE && r.width() == 284.0)
        .map(|(_, r)| r)
        .expect("the Test Drive paper");
    assert_eq!(top(paper), 332.0);
    assert_eq!(top(h.button("Build ▾")), bottom - 25.0);
}

#[test]
fn the_font_map_is_a_grid_of_21_by_23() {
    let h = font_panel_open(Vec2::new(1200.0, 800.0));
    let flyout = font_flyout(&h);
    let first = h.button("Glyph !");
    let next = h.button("Glyph \"");
    let below = h.button("Glyph .");
    assert_eq!(first.size(), vec2(20.0, 22.0));
    assert_eq!(next.left() - first.left(), 21.0);
    assert_eq!(below.top() - first.top(), 23.0);
    assert_eq!(below.left(), first.left());
    // La línea izquierda de la cuadrícula en la columna 8 del panel.
    assert_eq!(first.left() - 1.0, flyout.left() + 8.0);
}

#[test]
fn add_all_adds_the_missing_chars_of_the_page_in_one_step() {
    let mut h = font_panel_open(Vec2::new(1200.0, 800.0));
    h.click(h.button("Latin-1").center(), PointerButton::Primary);
    assert!(!h.disabled.iter().any(|l| l == "Add All"));
    h.click(h.button("Add All").center(), PointerButton::Primary);
    let has = |h: &LayoutHarness, ch| h.app.doc.editor.font().unwrap().index_of(ch).is_some();
    assert!(('¡'..='ÿ').all(|ch| has(&h, ch)));
    assert!(h.disabled.iter().any(|l| l == "Add All"), "nothing left");
    h.app.doc.editor.undo();
    assert!(('¡'..='ÿ').all(|ch| !has(&h, ch)), "one undo step");
    h.frame();
    h.click(h.button("All").center(), PointerButton::Primary);
    assert!(h.disabled.iter().any(|l| l == "Add All"), "not on All");
}

#[test]
fn a_map_cell_shows_the_drawn_glyph_or_its_char_in_gray() {
    let mut h = font_panel_open(Vec2::new(1200.0, 800.0));
    let e = &mut h.app.doc.editor;
    e.go_to_glyph('A');
    let (layer, frame) = (e.layer, e.frame);
    e.execute(Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(1, 1, cs_core::ONE_BIT_INK)],
    });
    h.frame();
    h.frame();
    let ink = egui::Color32::from_rgb(5, 4, 3);
    let gray = egui::Color32::from_rgb(197, 199, 221);
    let a = h.button("Glyph A");
    assert!(
        filled_rects(&h)
            .iter()
            .any(|(c, r)| *c == ink && a.contains_rect(*r)),
        "the ink of A in its cell"
    );
    let text_in = |cell: Rect| {
        h.text_rects
            .iter()
            .filter(|(_, r, _)| cell.contains(r.center()))
            .map(|(t, _, c)| (t.clone(), *c))
            .collect::<Vec<_>>()
    };
    assert_eq!(text_in(a), vec![], "only the glyph, no letter");
    let b = h.button("Glyph B");
    assert_eq!(text_in(b), vec![("B".to_owned(), gray)], "an empty glyph");
}

#[test]
fn the_test_drive_draws_the_text_and_clicking_a_letter_goes_to_its_glyph() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.app.font_panel.test_text = "AB".into();
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    let drive = h.button("Test Drive B");
    h.click(drive.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.current_glyph(), Some('B'));
}

#[test]
fn the_font_settings_change_the_metrics_with_undo() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    // Los ajustes van en la vista del engrane.
    h.click(h.button("Font Settings").center(), PointerButton::Primary);
    // Los + de los steppers del kit se llaman todos "Increase": el de la fila de "Baseline".
    let label = h
        .text_rects
        .iter()
        .find(|(t, _, _)| t == "Baseline")
        .map(|(_, r, _)| *r)
        .expect("the Baseline label");
    let plus = h
        .buttons
        .iter()
        .filter(|(l, r)| {
            l == "Increase" && r.y_range().contains(label.center().y) && r.left() > label.right()
        })
        .min_by(|a, b| a.1.left().total_cmp(&b.1.left()))
        .map(|(_, r)| *r)
        .expect("the Baseline + button");
    h.click(plus.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.font().unwrap().metrics.baseline, 13);
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.font().unwrap().metrics.baseline, 12);
}

#[test]
fn the_font_numbers_go_in_two_columns_inside_the_panel() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    // Los ajustes van en la vista del engrane.
    h.click(h.button("Font Settings").center(), PointerButton::Primary);
    let flyout = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("dock_flyout", "FontForge"))))
        .expect("the Font panel is open");
    let label = |name: &str| {
        h.text_rects
            .iter()
            .find(|(t, _, _)| t == name)
            .map(|(_, r, _)| *r)
            .unwrap_or_else(|| panic!("the {name} label"))
    };
    // Cada par comparte fila: la segunda etiqueta, a la derecha de la primera.
    for (left, right) in [
        ("Baseline", "Cap height"),
        ("x-height", "Descent"),
        ("Letter spacing", "Space width"),
    ] {
        let (a, b) = (label(left), label(right));
        assert!(b.y_range().contains(a.center().y), "{left} / {right}");
        assert!(b.left() > a.right(), "{left} / {right}");
        // Los dos steppers de la fila, dentro del panel.
        let plus: Vec<Rect> = h
            .buttons
            .iter()
            .filter(|(l, r)| l == "Increase" && r.y_range().contains(a.center().y))
            .map(|(_, r)| *r)
            .collect();
        assert_eq!(plus.len(), 2, "{left} / {right}");
        for r in plus {
            assert!(flyout.contains_rect(r), "{r:?} vs {flyout:?}");
        }
    }
    assert!(label("x-height").top() > label("Baseline").bottom());
}

#[test]
fn dragging_a_font_stepper_is_one_undo_step() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app
        .doc
        .editor
        .new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.click(h.button("FontForge").center(), PointerButton::Primary);
    h.frame();
    // Los ajustes van en la vista del engrane.
    h.click(h.button("Font Settings").center(), PointerButton::Primary);
    let label = h
        .text_rects
        .iter()
        .find(|(t, _, _)| t == "Baseline")
        .map(|(_, r, _)| *r)
        .expect("the Baseline label");
    let in_row = |name: &str| {
        h.buttons
            .iter()
            .filter(|(l, r)| {
                l == name && r.y_range().contains(label.center().y) && r.left() > label.right()
            })
            .min_by(|a, b| a.1.left().total_cmp(&b.1.left()))
            .map(|(_, r)| *r)
            .expect("a Baseline button")
    };
    // El número, entre el − y el +: arrastrarlo a la izquierda baja la baseline.
    let field = in_row("Decrease")
        .center()
        .lerp(in_row("Increase").center(), 0.5);
    let steps = h.app.doc.editor.history.done_count();
    drag(&mut h, field, field - egui::vec2(80.0, 0.0));
    let baseline = h.app.doc.editor.font().unwrap().metrics.baseline;
    assert!(baseline < 11, "it moved more than one step: {baseline}");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.font().unwrap().metrics.baseline, 12);
}

/// Con Pixel UI, un modelo 3D de 32×32 nuevo abierto en su pestaña (su workspace de ChimpCAD,
/// con las vistas ya encuadradas).
fn with_a_model_at_800() -> LayoutHarness {
    let mut h = with_model_tab();
    h.frame();
    h
}

/// Dónde cae el artboard del modelo en el render de la perspectiva (en el cuadrante Persp del
/// workspace o arriba en el modo Texture): el render llena el cuadro y el artboard va al centro.
fn persp_pixels(h: &LayoutHarness) -> Rect {
    let drawn = persp_texture_rect(h);
    let (full, artboard) = h
        .app
        .doc
        .canvas
        .persp_drawn(&h.app.doc.editor)
        .expect("the perspective");
    assert_eq!(drawn, full, "the render is drawn where it says");
    artboard
}

/// Dónde se dibuja la textura del render de la perspectiva.
fn persp_texture_rect(h: &LayoutHarness) -> Rect {
    let tex = h
        .app
        .doc
        .canvas
        .model_persp_texture()
        .expect("the perspective is rendered");
    h.textures
        .iter()
        .find(|(t, _)| *t == tex)
        .map(|(_, r)| *r)
        .expect("the perspective is drawn")
}

#[test]
fn zoomed_in_the_persp_render_fills_its_quadrant_and_picks_outside_the_artboard() {
    let mut h = with_a_model_at_800();
    let q = h.app.doc.canvas.model_edit_quadrants().unwrap()[1];
    let artboard = persp_pixels(&h);
    h.frame_with(vec![Event::PointerMoved(artboard.center())]);
    for _ in 0..8 {
        h.frame_with(vec![line_wheel(1.0, Default::default())]);
    }
    h.frame();
    // La escala no cambia: el artboard sigue en el mismo lugar.
    assert_eq!(persp_pixels(&h), artboard);
    // El render llega a los cuatro bordes del cuadrante (se pasa menos de un pixel del render).
    let drawn = persp_texture_rect(&h);
    let k = h
        .app
        .doc
        .canvas
        .persp_map(&h.app.doc.editor)
        .unwrap()
        .scale
        .0;
    assert!(drawn.contains_rect(q), "{drawn:?} vs {q:?}");
    for over in [
        q.left() - drawn.left(),
        drawn.right() - q.right(),
        q.top() - drawn.top(),
        drawn.bottom() - q.bottom(),
    ] {
        assert!((0.0..=k).contains(&over), "{drawn:?} vs {q:?} at {k}");
    }
    // Un clic en el modelo afuera del cuadrado del artboard lo elige.
    // (A la izquierda, en el aire que quede entre el cuadrante y el cuadrado: hasta un pixel y
    // medio del render.)
    let left = (artboard.left() - q.left()).min(k * 3.0) / 2.0;
    let at = pos2(artboard.left() - left, artboard.center().y);
    assert!(
        q.contains(at) && !artboard.contains(at),
        "{at:?} {q:?} {artboard:?} {k}"
    );
    h.click(at, PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(0), "the cube");
}

#[test]
fn the_ortho_views_are_pixel_images_over_their_quadrants() {
    let mut h = with_a_model_at_800();
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let ppp = h.ctx.pixels_per_point();
    let first = h.app.doc.canvas.model_ortho_textures();
    for (k, i) in [0, 2, 3].into_iter().enumerate() {
        let (_, tex) = first[k].expect("the view is rasterized");
        let r = h
            .textures
            .iter()
            .find(|(t, _)| *t == tex)
            .map(|(_, r)| *r)
            .expect("the view is drawn");
        // Toda la vista, un pixel por punto, en puntos (y pixeles físicos) enteros.
        assert_eq!(r, quads[i], "view {i}");
        assert_eq!(r.min, r.min.round(), "{r:?}");
        assert_eq!(r.size(), r.size().round(), "{r:?}");
        let px = r.min.to_vec2() * ppp;
        assert_eq!(px, px.round(), "{r:?} at {ppp}");
    }
    // Sin cambios no se vuelve a pintar; con otra selección, sí.
    h.frame();
    assert_eq!(h.app.doc.canvas.model_ortho_textures(), first);
    let editor = &mut h.app.doc.editor;
    editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Object,
        items: [cs_3d::Item::Object(0)].into(),
    });
    h.frame();
    let next = h.app.doc.canvas.model_ortho_textures();
    for k in 0..3 {
        assert_ne!(next[k].unwrap().0, first[k].unwrap().0, "view {k}");
        assert_eq!(next[k].unwrap().1, first[k].unwrap().1, "same texture {k}");
    }
}

#[test]
fn the_cad_workspace_fills_the_canvas_with_the_four_views() {
    let mut h = with_a_model_at_800();
    assert!(h.app.view.rulers, "the rulers are on in the document");
    // El artboard del lienzo ya tiene su textura (del primer frame): no se dibuja.
    let plain = h.app.doc.editor.document.artboards[0].id;
    assert!(h.app.doc.canvas.artboard_texture(plain).is_some());
    let canvas = h.canvas.expect("the workspace area");
    let quads = h.app.doc.canvas.model_edit_quadrants().expect("the views");
    // Los cuatro cubren todo el área debajo del encabezado y de la barrita del objeto (sin
    // reglas), partida a la mitad en puntos enteros.
    let header = h.app.doc.canvas.cad_header().expect("the header");
    let bar = crate::panels::options_bar_height(h.app.pixel.skin());
    let views = Rect::from_min_max(pos2(canvas.left(), header.bottom() + 1.0 + bar), canvas.max);
    let all = quads.iter().fold(quads[0], |a, q| a.union(*q));
    assert_eq!(all, views);
    let mid = views.min + ((views.size() / 2.0).floor());
    assert_eq!(quads[0], Rect::from_min_max(views.min, mid), "Front");
    assert_eq!(quads[3], Rect::from_min_max(mid, views.max), "Side");
    // Ni la textura del modelo, ni otros artboards, ni barras de ventana.
    let ids: Vec<_> = h
        .app
        .doc
        .editor
        .document
        .artboards
        .iter()
        .map(|a| a.id)
        .collect();
    for id in ids {
        if let Some(tex) = h.app.doc.canvas.artboard_texture(id) {
            assert!(h.textures.iter().all(|(t, _)| *t != tex), "{id:?} drawn");
        }
    }
    assert!(h.drawn("title.bar").is_empty() && h.drawn("title.bar.model").is_empty());
    h.frame();
    assert_eq!(h.app.doc.canvas.model_edit_quadrants(), Some(quads));
}

#[test]
fn the_cad_views_end_left_of_the_3d_panel() {
    let h = with_a_model_at_800();
    let canvas = h.canvas.expect("the workspace area");
    // El panel 3D va fijo al lado (fuera del lienzo): las vistas llenan el área entera.
    let panel = model_flyout(&h);
    assert!(canvas.right() <= panel.left(), "{panel:?} vs {canvas:?}");
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    assert_eq!(quads[1].right(), canvas.right());
    assert_eq!(quads[3].right(), canvas.right());
    assert_eq!(quads[0].left(), canvas.left());
    assert!(quads[1].contains_rect(persp_pixels(&h)), "the render too");
}

#[test]
fn the_perspective_is_drawn_at_an_integer_scale() {
    let h = with_a_model_at_800();
    let q = h.app.doc.canvas.model_edit_quadrants().unwrap()[1];
    let r = persp_pixels(&h);
    let s = r.width() / 32.0;
    assert!(s >= 1.0 && s.fract() == 0.0, "{r:?}");
    assert_eq!(r.height(), 32.0 * s);
    // La más grande que cabe, centrada.
    assert!(
        32.0 * (s + 1.0) > q.width().min(q.height()),
        "{r:?} in {q:?}"
    );
    assert!(q.contains_rect(r), "{r:?} in {q:?}");
    assert!((r.center().x - q.center().x).abs() <= 1.0, "{r:?} in {q:?}");
    assert!((r.center().y - q.center().y).abs() <= 1.0, "{r:?} in {q:?}");
    // En pixeles físicos enteros.
    let ppp = h.ctx.pixels_per_point();
    let px = r.min.to_vec2() * ppp;
    assert_eq!(px, px.round(), "{r:?} at {ppp}");
}

#[test]
fn the_wheel_on_front_zooms_the_views_not_the_canvas() {
    let mut h = with_a_model_at_800();
    let canvas_zoom = h.app.doc.canvas.zoom;
    let pan = h.app.doc.canvas.pan();
    let zoom = h.app.doc.editor.model_edit().zoom;
    let at = h.app.doc.canvas.model_edit_quadrants().unwrap()[0].center();
    let wheel = |modifiers| Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers,
    };
    h.frame_with(vec![Event::PointerMoved(at)]);
    // Sin Ctrl, la rueda también hace zoom en las vistas.
    h.frame_with(vec![wheel(Default::default())]);
    h.frame();
    let plain = h.app.doc.editor.model_edit().zoom;
    assert!(plain > zoom, "{zoom} -> {plain}");
    assert_eq!(h.app.doc.canvas.zoom, canvas_zoom);
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::COMMAND),
        wheel(egui::Modifiers::COMMAND),
    ]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    assert!(h.app.doc.editor.model_edit().zoom > plain);
    assert_eq!(h.app.doc.canvas.zoom, canvas_zoom);
    assert_eq!(h.app.doc.canvas.pan(), pan);
}

#[test]
fn painting_tools_and_the_hand_do_nothing_in_the_cad_workspace() {
    let mut h = with_a_model_at_800();
    let pan = h.app.doc.canvas.pan();
    let revision = h.app.doc.editor.revision;
    let side = h.app.doc.canvas.model_edit_quadrants().unwrap()[3];
    for tool in [ToolKind::Pencil, ToolKind::Hand] {
        h.app.doc.editor.tool = tool;
        drag_with(
            &mut h,
            side.left_top() + vec2(4.0, 12.0),
            side.left_top() + vec2(40.0, 40.0),
            Default::default(),
        );
        assert!(!h.app.doc.editor.is_painting(), "{tool:?}");
    }
    assert_eq!(h.app.doc.canvas.pan(), pan);
    assert_eq!(h.app.doc.editor.revision, revision, "nothing painted");
}

fn tap_key(h: &mut LayoutHarness, key: egui::Key) {
    for pressed in [true, false] {
        h.frame_with(vec![Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        }]);
    }
    h.frame();
}

#[test]
fn tab_in_a_model_workspace_is_the_timeline() {
    let mut h = with_a_model_at_800();
    assert!(h.app.doc.editor.model_edit_on());
    let timeline = h.app.view.timeline;
    tap_key(&mut h, egui::Key::Tab);
    assert_ne!(h.app.view.timeline, timeline, "Tab toggles the timeline");
    assert!(
        h.app.doc.editor.model_edit_on(),
        "the workspace stays in Edit"
    );
}

fn editing_model() -> LayoutHarness {
    // El modelo abierto en su pestaña: su workspace, siempre en modo Edit.
    let h = with_a_model_at_800();
    assert!(h.app.doc.editor.model_edit_on());
    h
}

fn front_screen(h: &LayoutHarness, w: cs_3d::P3) -> Pos2 {
    let map = h
        .app
        .doc
        .canvas
        .edit_view_map(&h.app.doc.editor, cs_3d::OrthoView::Front)
        .expect("front view");
    let (x, y) = map.to_screen(w);
    Pos2::new(x, y)
}

fn drag_with(h: &mut LayoutHarness, from: Pos2, to: Pos2, modifiers: egui::Modifiers) {
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![button(to, false)]);
    h.frame();
}

#[test]
fn clicking_a_vertex_in_front_selects_it_and_shift_click_toggles() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let corner = front_screen(&h, [16.0, 16.0, 16.0]);
    h.click(corner, PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.model_edit().selection.items,
        [cs_3d::Item::Vertex(0, 7)].into()
    );
    let other = front_screen(&h, [-16.0, -16.0, 16.0]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::SHIFT)]);
    drag_with(&mut h, other, other, egui::Modifiers::SHIFT);
    assert_eq!(h.app.doc.editor.model_edit().selection.items.len(), 2);
}

#[test]
fn dragging_a_vertex_in_front_moves_it_in_one_undo_step() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let from = front_screen(&h, [16.0, 16.0, 16.0]);
    let to = front_screen(&h, [24.0, 16.0, 16.0]);
    drag_with(&mut h, from, to, Default::default());
    let v = h.app.doc.editor.model().unwrap().objects[0].mesh.vertices[7];
    assert_eq!((v.x, v.y, v.z), (24, 16, 16));
    h.app.doc.editor.undo();
    let v = h.app.doc.editor.model().unwrap().objects[0].mesh.vertices[7];
    assert_eq!(v.x, 16);
}

#[test]
fn a_box_in_an_empty_spot_selects_the_vertices_inside() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let corner = front_screen(&h, [16.0, 16.0, 16.0]);
    drag_with(
        &mut h,
        corner + vec2(8.0, -8.0),
        corner + vec2(-8.0, 8.0),
        Default::default(),
    );
    assert_eq!(
        h.app.doc.editor.model_edit().selection.items,
        [cs_3d::Item::Vertex(0, 3), cs_3d::Item::Vertex(0, 7)].into()
    );
}

#[test]
fn right_drag_pans_the_ortho_views_and_ctrl_wheel_zooms_them() {
    let mut h = editing_model();
    let center = h.app.doc.editor.model_edit().center;
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let at = quads[0].center();
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![button(at, true)]);
    h.frame_with(vec![Event::PointerMoved(at + vec2(20.0, 0.0))]);
    h.frame_with(vec![button(at + vec2(20.0, 0.0), false)]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().center[0] < center[0]);
    let zoom = h.app.doc.editor.model_edit().zoom;
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().zoom > zoom);
}

fn camera(h: &LayoutHarness) -> cs_core::Camera3D {
    h.app.doc.editor.model().expect("a model").camera
}

#[test]
fn right_dragging_a_model_orbits_the_camera_in_one_undo_step() {
    let mut h = with_a_model_at_800();
    let before = camera(&h);
    let steps = h.app.doc.editor.history.done_count();
    let from = persp_pixels(&h).center();
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from, true)]);
    for dx in [5.0, 10.0, 20.0] {
        h.frame_with(vec![Event::PointerMoved(from + vec2(dx, 4.0))]);
    }
    h.frame_with(vec![press(from + vec2(20.0, 4.0), false)]);
    h.frame();
    let after = camera(&h);
    assert_eq!(after.yaw, before.yaw + 100);
    assert_eq!(after.pitch, before.pitch + 20);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(camera(&h), before);
}

#[test]
fn ctrl_wheel_over_a_model_changes_the_camera_distance_not_the_zoom() {
    let mut h = with_a_model_at_800();
    let before = camera(&h).distance;
    let zoom = h.app.doc.canvas.zoom;
    let at = persp_pixels(&h).center();
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::COMMAND),
        Event::PointerMoved(at),
    ]);
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    let after = camera(&h).distance;
    assert_eq!(after, (before as f32 * 1.1).round() as i32, "away");
    assert_eq!(h.app.doc.canvas.zoom, zoom);
}

/// Un paso de rueda de ratón (hacia arriba si `dy > 0`).
fn line_wheel(dy: f32, modifiers: egui::Modifiers) -> Event {
    Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        modifiers,
    }
}

#[test]
fn alt_wheel_over_persp_changes_the_fov_in_one_undo_step() {
    let mut h = with_a_model_at_800();
    let before = camera(&h);
    assert_eq!(
        before.projection,
        cs_core::Projection::Perspective { fov: 600 }
    );
    let steps = h.app.doc.editor.history.done_count();
    let at = persp_pixels(&h).center();
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::ALT),
        Event::PointerMoved(at),
    ]);
    // Hacia arriba cierra el FOV (de 5° en 5°); la distancia no cambia.
    for _ in 0..2 {
        h.frame_with(vec![line_wheel(1.0, egui::Modifiers::ALT)]);
    }
    h.frame_with(vec![line_wheel(-1.0, egui::Modifiers::ALT)]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    let after = camera(&h);
    assert_eq!(
        after.projection,
        cs_core::Projection::Perspective { fov: 550 }
    );
    assert_eq!(after.distance, before.distance);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.frame();
    assert!(
        h.texts.iter().any(|(t, _)| t == "Persp 55°"),
        "the label says the fov"
    );
    h.app.doc.editor.undo();
    assert_eq!(camera(&h), before);
}

#[test]
fn the_fov_stops_at_its_limits() {
    let mut h = with_a_model_at_800();
    let at = persp_pixels(&h).center();
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::ALT),
        Event::PointerMoved(at),
    ]);
    for _ in 0..20 {
        h.frame_with(vec![line_wheel(1.0, egui::Modifiers::ALT)]);
    }
    assert_eq!(
        camera(&h).projection,
        cs_core::Projection::Perspective { fov: 100 }
    );
}

#[test]
fn a_burst_of_wheel_steps_over_persp_is_one_undo_step() {
    let mut h = with_a_model_at_800();
    let before = camera(&h);
    let steps = h.app.doc.editor.history.done_count();
    let at = persp_pixels(&h).center();
    h.frame_with(vec![Event::PointerMoved(at)]);
    for _ in 0..4 {
        h.frame_with(vec![line_wheel(1.0, Default::default())]);
    }
    h.frame();
    assert!(camera(&h).distance < before.distance);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(camera(&h), before);
}

#[test]
fn a_pause_or_another_gesture_starts_a_new_wheel_step() {
    let mut h = with_a_model_at_800();
    let steps = h.app.doc.editor.history.done_count();
    let at = persp_pixels(&h).center();
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    // Una pausa de más de 0,6 s corta el grupo.
    h.time += 1.0;
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    h.frame();
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 2);
    // Otro gesto (un cambio de cámara aparte) también corta el grupo.
    let mut cam = camera(&h);
    cam.yaw += 100;
    h.app.doc.editor.set_camera(cam);
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    h.frame();
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 4);
}

/// El punto del plano del objetivo (perpendicular a la mirada) que se ve en `at`.
fn persp_focal_point(h: &LayoutHarness, at: Pos2) -> cs_3d::P3 {
    let map = h
        .app
        .doc
        .canvas
        .persp_map(&h.app.doc.editor)
        .expect("the perspective");
    let (o, d) = map.ray((at.x, at.y));
    let fwd = map.projector().axes()[2];
    let t = map.camera.target;
    let t = [t.x as f32, t.y as f32, t.z as f32];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let s = dot([t[0] - o[0], t[1] - o[1], t[2] - o[2]], fwd) / dot(d, fwd);
    std::array::from_fn(|i| o[i] + d[i] * s)
}

#[test]
fn a_middle_drag_over_persp_pans_in_one_undo_step() {
    let mut h = with_a_model_at_800();
    let before = camera(&h);
    let steps = h.app.doc.editor.history.done_count();
    let from = persp_pixels(&h).center();
    let to = from + vec2(40.0, 0.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Middle,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![button(to, false)]);
    h.frame();
    let after = camera(&h);
    // Se mueve el objetivo, sin orbitar ni acercarse.
    assert_ne!(after.target, before.target, "panned");
    assert_eq!(
        (after.yaw, after.pitch, after.distance),
        (before.yaw, before.pitch, before.distance)
    );
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(camera(&h), before);
}

#[test]
fn the_plain_wheel_over_persp_dollies_toward_the_pointer() {
    let mut h = with_a_model_at_800();
    let before = camera(&h);
    let zoom = h.app.doc.canvas.zoom;
    let ortho = h.app.doc.editor.model_edit().zoom;
    let pixels = persp_pixels(&h);
    // Fuera del centro: el objetivo se corre hacia ahí.
    let at = pixels.center() + vec2(pixels.width() / 4.0, -pixels.height() / 8.0);
    let point = persp_focal_point(&h, at);
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    h.frame();
    let after = camera(&h);
    assert_eq!(
        after.distance,
        (before.distance as f32 / 1.1).round() as i32,
        "closer"
    );
    assert_ne!(after.target, before.target, "toward the pointer");
    // Lo que estaba bajo el puntero sigue ahí (a menos de un pixel del render).
    let map = h.app.doc.canvas.persp_map(&h.app.doc.editor).unwrap();
    let (x, y) = map.to_screen(point).expect("in front");
    assert!(
        (x - at.x).abs() <= map.scale.0 && (y - at.y).abs() <= map.scale.1,
        "{:?} vs {at:?}",
        (x, y)
    );
    assert_eq!(h.app.doc.canvas.zoom, zoom, "not the canvas");
    assert_eq!(h.app.doc.editor.model_edit().zoom, ortho, "not the views");
    // Ctrl + rueda hace lo mismo.
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::COMMAND),
        line_wheel(-1.0, egui::Modifiers::COMMAND),
    ]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    assert_eq!(
        camera(&h).distance,
        (after.distance as f32 * 1.1).round() as i32,
        "away"
    );
}

#[test]
fn the_plain_wheel_over_the_texture_mode_render_dollies_the_camera() {
    let mut h = with_a_model_in_texture_mode();
    let model = |h: &LayoutHarness| h.app.doc.editor.workspace_model().unwrap().camera;
    let before = model(&h);
    let view = h.app.doc.editor.model_edit().texture_view;
    let zoom = h.app.doc.canvas.zoom;
    let (top, _) = h.app.doc.canvas.texture_panes().unwrap();
    h.frame_with(vec![Event::PointerMoved(top.center())]);
    h.frame_with(vec![line_wheel(1.0, Default::default())]);
    h.frame();
    let after = model(&h);
    assert_eq!(
        after.distance,
        (before.distance as f32 / 1.1).round() as i32,
        "closer"
    );
    assert_eq!(
        h.app.doc.editor.model_edit().texture_view,
        view,
        "not the texture"
    );
    assert_eq!(h.app.doc.canvas.zoom, zoom);
}

#[test]
fn clicking_a_model_picks_the_object_under_the_pointer() {
    let mut h = with_a_model_at_800();
    let pixels = persp_pixels(&h);
    h.click(pixels.left_top() + vec2(4.0, 4.0), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), None, "an empty corner");
    h.click(pixels.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(0), "the cube");
}

#[test]
fn shift_s_and_shift_r_keep_their_shortcuts_in_the_cad_workspace() {
    let mut h = with_a_model_at_800();
    h.app.doc.editor.select_model_object(Some(0));
    let key = |key, pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    };
    let at = h.app.doc.canvas.model_edit_quadrants().unwrap()[0].center();
    h.frame_with(vec![
        Event::PointerMoved(at),
        Event::ModifiersChanged(egui::Modifiers::SHIFT),
    ]);
    for k in [egui::Key::S, egui::Key::R] {
        h.frame_with(vec![key(k, true), key(k, false)]);
        h.frame();
        let e = h.app.doc.editor.model_edit();
        assert!(
            e.op.is_none() && e.key_request.is_none(),
            "{k:?} started a gesture"
        );
    }
    // Shift+R sigue siendo la herramienta Jumble.
    assert_eq!(h.app.doc.editor.tool, ToolKind::Jumble);
}

#[test]
fn a_right_click_on_a_model_does_not_reopen_an_old_selection_menu() {
    let mut h = with_a_model_at_800();
    let model = h.app.doc.editor.active;
    h.app.doc.editor.tool = ToolKind::RectSelect;
    // El menú de la selección en el artboard normal, en el lienzo.
    h.app.doc.editor.exit_workspace();
    h.app.doc.editor.select_all();
    zoom_centered(&mut h, 8.0);
    h.click(artboard_pixels(&h).center(), PointerButton::Secondary);
    let menu = |h: &LayoutHarness| h.texts.iter().any(|(t, _)| t == "Flip Horizontal");
    assert!(menu(&h), "the selection menu");
    h.frame_with(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    h.frame();
    // El derecho sobre la perspectiva del modelo orbita: no vuelve a abrir aquel menú.
    h.app.doc.editor.enter_workspace(model);
    h.frame();
    h.click(persp_pixels(&h).center(), PointerButton::Secondary);
    h.frame();
    assert!(!menu(&h), "an old menu opened over the model");
}

#[test]
fn the_other_button_during_a_model_drag_reaches_no_tool() {
    // Con una selección y Move, el izquierdo adentro empezaría a transformarla.
    let mut h = with_a_model_at_800();
    h.app.doc.editor.tool = ToolKind::Move;
    h.app.doc.editor.select_all();
    let from = persp_pixels(&h).center();
    let button = |pos, button, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    let yaw = camera(&h).yaw;
    // El derecho orbita; a la mitad, el izquierdo.
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, PointerButton::Secondary, true)]);
    let mid = from + vec2(10.0, 0.0);
    h.frame_with(vec![Event::PointerMoved(mid)]);
    h.frame_with(vec![button(mid, PointerButton::Primary, true)]);
    assert!(
        h.app.doc.editor.transform_drag.is_none(),
        "the transform got the press"
    );
    h.frame_with(vec![button(mid, PointerButton::Primary, false)]);
    h.frame_with(vec![button(mid, PointerButton::Secondary, false)]);
    h.frame();
    assert_ne!(camera(&h).yaw, yaw, "the drag orbited");
    assert!(h.app.doc.editor.transform_drag.is_none());
    assert!(!h.app.doc.editor.is_painting());
}

// --- El panel 3D ---

/// El panel 3D (con `model`, un modelo nuevo de 32×32 con su cubo, en su pestaña; sin él, el
/// del dock abierto).
fn model_panel_open(model: bool) -> LayoutHarness {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    if model {
        // En la pestaña del modelo el panel 3D va fijo (sin el dock).
        assert!(h.app.doc.editor.new_model("Ship", (32, 32)));
    } else {
        h.click(h.button("3D").center(), PointerButton::Primary);
    }
    h.frame();
    h
}

fn node(h: &LayoutHarness, label: &str) -> Rect {
    h.nodes
        .iter()
        .find(|(l, _)| l == label)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no node {label:?}"))
}

#[test]
fn the_model_panel_without_a_model_offers_new_3d_model() {
    let mut h = model_panel_open(false);
    h.click(h.button("New 3D Model…").center(), PointerButton::Primary);
    assert!(h.app.new_model.is_some());
}

#[test]
fn the_model_panel_lists_the_objects_and_adds_a_cylinder() {
    let mut h = model_panel_open(true);
    let flyout = model_flyout(&h);
    let cube = node(&h, "Cube");
    assert!(flyout.contains_rect(cube), "{cube:?} vs {flyout:?}");
    add_object(&mut h, "Cylinder");
    let editor = &h.app.doc.editor;
    let objects = &editor.model().unwrap().objects;
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[1].name, "Cylinder");
    // La malla de la primitiva (sus UVs, en su lugar libre de la textura).
    let want = cs_3d::primitive_mesh(&cs_3d::Primitive::Cylinder { sides: 8 });
    assert_eq!(objects[1].mesh.vertices, want.vertices);
    assert_eq!(objects[1].mesh.faces.len(), want.faces.len());
    for (a, b) in objects[1].mesh.faces.iter().zip(&want.faces) {
        assert_eq!((&a.verts, a.part), (&b.verts, b.part));
    }
    assert_eq!(editor.selected_object(), Some(1));
    // Un clic en la fila del cubo lo elige.
    h.click(node(&h, "Cube").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(0));
}

/// El panel 3D: su caja (en la pestaña del modelo, la del panel fijo; si no, la del dock).
fn model_flyout(h: &LayoutHarness) -> Rect {
    if h.app.doc.editor.model_edit_on() {
        let side = h.panel("model_panel");
        let air = crate::dock::RIGHT_MARGIN as f32;
        return Rect::from_min_max(
            pos2(side.left(), side.top() + 2.0 + 5.0),
            pos2(side.right() - air, side.bottom() - 2.0 - 5.0),
        );
    }
    h.ctx
        .memory(|m| m.area_rect(egui::Id::new(("dock_flyout", "3D"))))
        .expect("the 3D panel is open")
}

/// El nodo `label` dentro del panel 3D (no el de otro lado con el mismo nombre).
fn in_model_panel(h: &LayoutHarness, label: &str) -> Option<Rect> {
    let flyout = model_flyout(h);
    h.nodes
        .iter()
        .find(|(l, r)| l == label && flyout.contains_rect(*r))
        .map(|(_, r)| *r)
}

/// Agrega un objeto `kind` con el + del encabezado del outliner: en su menú, el ícono del tipo y
/// "Add <tipo>".
fn add_object(h: &mut LayoutHarness, kind: &str) {
    let plus = open_add_menu(h);
    let icon = in_add_menu(h, plus, kind).unwrap_or_else(|| panic!("no {kind:?} in the + menu"));
    h.click(icon.center(), PointerButton::Primary);
    let add = in_add_menu(h, plus, &format!("Add {kind}")).expect("the Add button");
    h.click(add.center(), PointerButton::Primary);
}

/// Abre el menú del + del panel 3D; devuelve el +.
fn open_add_menu(h: &mut LayoutHarness) -> Rect {
    let plus = in_model_panel(h, "Add object").expect("the + button");
    h.click(plus.center(), PointerButton::Primary);
    plus
}

/// El nodo `label` en el menú del + (debajo del +; no una fila del outliner, de lado a lado).
fn in_add_menu(h: &LayoutHarness, plus: Rect, label: &str) -> Option<Rect> {
    h.nodes
        .iter()
        .find(|(l, r)| l == label && r.top() > plus.bottom() && r.width() < 100.0)
        .map(|(_, r)| *r)
}

/// La caja del menú del + (sin su sombra ni su pestaña): la `menu.list` que empieza 10 filas
/// debajo de lo alto del + y termina con él a la derecha (como el boceto).
fn add_menu_box(h: &LayoutHarness, plus: Rect) -> Option<Rect> {
    let shadow = crate::menus::PANEL_SHADOW;
    // Las partes del 9-slice de la caja (y las de la pestaña que caen sobre ella).
    let parts = h
        .drawn(crate::menus::PANEL)
        .into_iter()
        .filter(|r| r.top() >= plus.top() + 10.0);
    parts
        .reduce(|a, b| a.union(b))
        .filter(|r| r.top() == plus.top() + 10.0 && r.right() == plus.right() + shadow)
        .map(|r| Rect::from_min_max(r.min, r.max - Vec2::splat(shadow)))
}

/// Si el stepper de los lados del menú del + está deshabilitado (su campo, el apagado).
fn sides_disabled(h: &LayoutHarness, menu: Rect) -> bool {
    let inside = |name: &str| h.drawn(name).iter().any(|r| menu.contains_rect(*r));
    let off = inside("stepper.field.disabled");
    assert_ne!(off, inside(crate::kit::stepper::FIELD), "one stepper field");
    off
}

/// `h` sin objeto elegido (desde el editor).
fn without_an_object(mut h: LayoutHarness) -> LayoutHarness {
    h.app.doc.editor.select_model_object(None);
    h.frame();
    h.frame();
    assert_eq!(h.app.doc.editor.selected_object(), None);
    h
}

/// La fila de la línea que parte el panel 3D: como en el boceto, a 197/408 (≈ 48 %) del alto de
/// su caja (en puntos enteros).
fn model_split(h: &LayoutHarness) -> f32 {
    let flyout = model_flyout(h);
    flyout.top() + (flyout.height() * 197.0 / 408.0).floor()
}

/// La mitad de arriba del panel 3D (hasta la línea) y la de abajo (desde la línea).
fn model_halves(h: &LayoutHarness) -> (Rect, Rect) {
    let flyout = model_flyout(h);
    let split = model_split(h);
    (
        Rect::from_min_max(flyout.min, pos2(flyout.right(), split)),
        Rect::from_min_max(pos2(flyout.left(), split), flyout.max),
    )
}

/// Lo general del modelo (arriba) y la fila de botones del objeto (abajo), en modo Model.
const GENERAL_TEXTS: [&str; 5] = [
    "Grid",
    "Texture",
    "Texture Size",
    "Camera",
    "Light & Outline",
];
const OBJECT_BUTTONS: [&str; 4] = ["Extrude", "Subdivide", "Merge", "Delete"];

#[test]
fn the_3d_panel_is_split_at_48_percent_of_its_height_by_an_etched_line() {
    let h = model_panel_tall();
    let flyout = model_flyout(&h);
    let split = model_split(&h);
    assert_eq!(split, split.round(), "whole points");
    // La línea grabada (la oscura y su brillo), de lado a lado de lo de adentro del marco.
    let colors = *h.app.pixel.skin().expect("on").colors();
    let rects = h.colored_rects();
    let dark = Rect::from_x_y_ranges(flyout.x_range().shrink(1.0), split..=split + 1.0);
    assert!(rects.contains(&(colors.canvas, dark)), "{dark:?}");
    let light = dark.translate(vec2(0.0, 1.0));
    assert!(rects.contains(&(colors.highlight, light)), "{light:?}");
    // El encabezado del outliner, justo debajo de la línea.
    let plus = node(&h, "Add object");
    assert!(plus.top() > split + 1.0, "{plus:?} under {split}");
    assert!(plus.top() - split <= 8.0, "{plus:?} right under {split}");
    // Otra ventana, otra línea.
    let h = model_panel_in(Vec2::new(1200.0, 640.0));
    let split = model_split(&h);
    let plus = node(&h, "Add object");
    assert!(plus.top() > split + 1.0 && plus.top() - split <= 8.0);
}

#[test]
fn the_general_part_goes_above_the_line_and_the_object_below() {
    let h = model_panel_tall();
    assert_eq!(h.app.doc.editor.selected_object(), Some(0));
    let (top, bottom) = model_halves(&h);
    let header = node(&h, "Add object");
    let below_tabs = Rect::from_min_max(pos2(bottom.left(), header.bottom()), bottom.max);
    for t in GENERAL_TEXTS {
        assert!(text_in(&h, top, t).is_some(), "{t} above the line");
        assert!(text_in(&h, bottom, t).is_none(), "{t} below the line");
    }
    for label in ["Shading…", "Sheet settings…", "GIF settings…", "New"] {
        let r = in_model_panel(&h, label).unwrap_or_else(|| panic!("{label}"));
        assert!(top.contains_rect(r), "{label} {r:?} in {top:?}");
    }
    for l in OBJECT_BUTTONS {
        let r = in_model_panel(&h, l).unwrap_or_else(|| panic!("{l}"));
        assert!(below_tabs.contains_rect(r), "{l} {r:?} below the header");
    }
}

#[test]
fn the_outliner_has_no_model_row_and_one_row_per_object() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    away(&mut h);
    let flyout = model_flyout(&h);
    assert!(in_model_panel(&h, "Model").is_none(), "no model row");
    for name in [
        "icon.object.cone",
        "icon.object.sphere",
        "icon.object.plane",
    ] {
        assert!(
            !h.drawn(name).iter().any(|r| flyout.contains_rect(*r)),
            "{name}: only the kinds of the objects"
        );
    }
    // Una fila debajo de la otra, en el orden del árbol, al principio de lo de adentro.
    let (cube, cylinder) = (node(&h, "Cube"), node(&h, "Cylinder"));
    assert_eq!(
        (cube.left(), cube.width()),
        (cylinder.left(), cylinder.width())
    );
    assert_eq!(
        cylinder.top() - cube.top(),
        crate::model_panel::outliner::ROW_H
    );
    let inner = flyout.shrink(crate::dock::FLYOUT_MARGIN as f32);
    assert!(
        cube.left() - inner.left() <= 4.0,
        "{cube:?} first in {inner:?}"
    );
    h.click(cube.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(0));
    h.click(node(&h, "Cylinder").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(1));
    // La activa, con el fondo de la selección del kit.
    let colors = *h.app.pixel.skin().expect("on").colors();
    let (cube, cylinder) = (node(&h, "Cube"), node(&h, "Cylinder"));
    let rects = filled_rects(&h);
    assert!(
        rects.contains(&(colors.selection, cylinder)),
        "{cylinder:?}"
    );
    assert!(!rects.contains(&(colors.selection, cube)), "{cube:?}");
    // Sin objeto elegido, ninguna activa.
    let h = without_an_object(h);
    let rects = filled_rects(&h);
    for row in ["Cube", "Cylinder"] {
        let r = node(&h, row);
        assert!(!rects.contains(&(colors.selection, r)), "{row} active");
    }
}

/// El puntero lejos del panel (que nada se dibuje con el hover).
fn away(h: &mut LayoutHarness) {
    h.frame_with(vec![Event::PointerMoved(pos2(1.0, 1.0))]);
    h.frame();
}

#[test]
fn without_an_object_only_the_buttons_and_the_outliner_show_below_the_line() {
    let h = without_an_object(model_panel_tall());
    let (top, bottom) = model_halves(&h);
    let header = node(&h, "Add object");
    let below = Rect::from_min_max(pos2(bottom.left(), header.bottom() + 2.0), bottom.max);
    let mut texts: Vec<&str> = h
        .text_rects
        .iter()
        .filter(|(_, r, _)| below.intersects(*r))
        .map(|(t, _, _)| t.as_str())
        .collect();
    texts.sort_unstable();
    // (Los botones deshabilitados dibujan su texto dos veces: el grabado.)
    texts.dedup();
    assert_eq!(
        texts,
        vec!["Cube", "Delete", "Extrude", "Merge", "Subdivide"],
        "only the object buttons and the outliner under the header"
    );
    let mut controls: Vec<&str> = h
        .nodes
        .iter()
        .filter(|(_, r)| below.intersects(*r))
        .map(|(l, _)| l.as_str())
        .collect();
    controls.sort_unstable();
    controls.dedup();
    assert_eq!(
        controls,
        vec![
            "Cube",
            "Delete",
            "Double sided Cube",
            "Extrude",
            "Hide Cube",
            "Lock Cube",
            "Merge",
            "Subdivide"
        ],
        "only the object buttons and the row of the cube under the header"
    );
    // Lo general, arriba como siempre.
    for t in GENERAL_TEXTS {
        assert!(text_in(&h, top, t).is_some(), "{t} above the line");
    }
}

#[test]
fn each_half_of_the_3d_panel_scrolls_on_its_own() {
    // Una ventana baja: ni lo general ni el outliner (con ocho cubos) caben en su parte.
    let mut h = model_panel_in(Vec2::new(1200.0, 360.0));
    for _ in 0..7 {
        assert!(h.app.doc.editor.add_primitive(cs_3d::Primitive::Cube));
    }
    h.frame();
    h.frame();
    let (top, bottom) = model_halves(&h);
    let tracks = h.drawn("scroll.track.v");
    assert!(tracks.iter().any(|r| top.contains_rect(*r)), "{tracks:?}");
    assert!(
        tracks.iter().any(|r| bottom.contains_rect(*r)),
        "{tracks:?}"
    );
    let split = model_split(&h);
    let cube = node(&h, "Add object");
    let first = |h: &LayoutHarness, t: &str, area: Rect| text_in(h, area, t);
    let object = first(&h, "Grid", top).expect("Grid at the top");
    let position = first(&h, "Extrude", bottom).expect("Extrude below");
    let wheel = |h: &mut LayoutHarness, at: Pos2| {
        h.frame_with(vec![Event::PointerMoved(at)]);
        h.frame_with(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -60.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..10 {
            h.frame();
        }
    };
    // Arriba: se mueve lo general; la línea, la tira y lo del objeto se quedan.
    wheel(&mut h, object.center());
    let moved = first(&h, "Grid", top);
    assert!(moved.is_none_or(|r| r.top() < object.top()), "{moved:?}");
    assert_eq!(model_split(&h), split);
    assert_eq!(node(&h, "Add object"), cube);
    assert_eq!(first(&h, "Extrude", bottom), Some(position));
    let general = h
        .text_rects
        .iter()
        .filter(|(_, r, _)| top.contains_rect(*r))
        .map(|(t, r, _)| (t.clone(), *r))
        .collect::<Vec<_>>();
    // Abajo: se mueve lo del objeto; lo general se queda donde quedó.
    wheel(&mut h, position.center());
    let moved = first(&h, "Extrude", bottom);
    assert!(moved.is_none_or(|r| r.top() < position.top()), "{moved:?}");
    assert_eq!(node(&h, "Add object"), cube);
    let still = h
        .text_rects
        .iter()
        .filter(|(_, r, _)| top.contains_rect(*r))
        .map(|(t, r, _)| (t.clone(), *r))
        .collect::<Vec<_>>();
    assert_eq!(still, general);
    // Lo que no cabe en su mitad no se ve en la otra (lo recorta su scroll).
    let seen = |h: &LayoutHarness, area: Rect, t: &str| {
        h.shapes.iter().any(|c| {
            let mut texts = Vec::new();
            collect_text_rects(&c.shape, &mut texts);
            texts
                .iter()
                .any(|(s, r, _)| s == t && r.intersect(c.clip_rect).intersect(area).is_positive())
        })
    };
    assert!(seen(&h, top, "Camera"), "the general part is drawn");
    for t in GENERAL_TEXTS {
        assert!(!seen(&h, bottom, t), "{t} seen below the line");
    }
    for t in OBJECT_BUTTONS {
        assert!(!seen(&h, top, t), "{t} seen above the line");
    }
}

#[test]
fn the_plus_opens_the_add_menu_like_the_sketch() {
    let mut h = without_an_object(model_panel_open(true));
    // Ni Add ni Sides en el cuerpo: van en el menú del +.
    let flyout = model_flyout(&h);
    assert!(in_model_panel(&h, "Add").is_none());
    assert!(text_in(&h, flyout, "Sides").is_none());
    let plus = in_model_panel(&h, "Add object").expect("the + button");
    let title = text_in(&h, flyout, "Outliner").expect("the Outliner header");
    // Al final del encabezado, de 13×13 y a la altura del título.
    assert!(plus.left() > title.right());
    assert_eq!(plus.size(), vec2(13.0, 13.0), "{plus:?}");
    assert!(
        (plus.center().y - title.center().y).abs() <= 3.0,
        "{plus:?} {title:?}"
    );
    assert!(add_menu_box(&h, plus).is_none(), "closed");
    open_add_menu(&mut h);
    // La ventanita del boceto: 136×87, 10 filas debajo de lo alto del + y con él a la derecha.
    let menu = add_menu_box(&h, plus).expect("the add menu");
    assert_eq!(menu.size(), vec2(136.0, 87.0));
    // Los cinco íconos en una fila, uno cada 25 px, en el orden del boceto.
    let icons: Vec<Rect> = ["Cube", "Cylinder", "Cone", "Sphere", "Plane"]
        .iter()
        .map(|k| in_add_menu(&h, plus, k).unwrap_or_else(|| panic!("no {k} icon")))
        .collect();
    for (i, pair) in icons.windows(2).enumerate() {
        assert_eq!(pair[1].left() - pair[0].left(), 25.0, "icon {i}");
        assert_eq!(pair[1].top(), pair[0].top(), "icon {i}");
    }
    assert!(icons.iter().all(|r| menu.contains_rect(*r)));
    for (i, kind) in ["cube", "cylinder", "cone", "sphere", "plane"]
        .iter()
        .enumerate()
    {
        let name = format!("icon.add.{kind}");
        let drawn = h.drawn(&name);
        assert!(
            drawn.iter().any(|r| icons[i].contains_rect(*r)),
            "{name} in {:?}: {drawn:?}",
            icons[i]
        );
    }
    // Elegido de entrada, el cubo: Add Cube abajo a la derecha (8 px antes del borde).
    let add = in_add_menu(&h, plus, "Add Cube").expect("Add Cube");
    assert_eq!(add.right(), menu.right() - 8.0, "{add:?} {menu:?}");
    assert_eq!(add.top(), menu.top() + 62.0, "{add:?} {menu:?}");
    assert_eq!(add.height(), crate::kit::button::HEIGHT);
    assert!(h.selected.contains(&("Cube".to_owned(), true)));
    assert!(text_in(&h, menu, "Sides").is_some());
    // El cubo no tiene lados.
    assert!(sides_disabled(&h, menu));
    // La línea divisoria, de borde a borde, 33 filas debajo de lo alto de la caja.
    let line = h
        .drawn(crate::menus::SEPARATOR)
        .into_iter()
        .filter(|r| menu.contains_rect(*r))
        .reduce(|a, b| a.union(b))
        .expect("the divider");
    assert_eq!(line.top(), menu.top() + 33.0);
    assert_eq!(line.x_range(), menu.x_range().shrink(1.0));
}

#[test]
fn clicking_an_icon_of_the_add_menu_chooses_the_type() {
    let mut h = without_an_object(model_panel_open(true));
    let plus = open_add_menu(&mut h);
    let menu = add_menu_box(&h, plus).expect("the add menu");
    let cylinder = in_add_menu(&h, plus, "Cylinder").unwrap();
    h.click(cylinder.center(), PointerButton::Primary);
    assert!(add_menu_box(&h, plus).is_some(), "it stays open");
    assert!(in_add_menu(&h, plus, "Add Cylinder").is_some());
    assert!(in_add_menu(&h, plus, "Add Cube").is_none());
    assert!(!sides_disabled(&h, menu), "a cylinder has sides");
    assert!(h.selected.contains(&("Cylinder".to_owned(), true)));
    // El plano, como el cubo, no.
    let plane = in_add_menu(&h, plus, "Plane").unwrap();
    h.click(plane.center(), PointerButton::Primary);
    assert!(in_add_menu(&h, plus, "Add Plane").is_some());
    assert!(sides_disabled(&h, menu));
    // Nada se agregó todavía.
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), 1);
}

#[test]
fn add_in_the_add_menu_adds_the_type_with_the_sides_and_selects_its_row() {
    let mut h = without_an_object(model_panel_open(true));
    let plus = open_add_menu(&mut h);
    let menu = add_menu_box(&h, plus).expect("the add menu");
    let cone = in_add_menu(&h, plus, "Cone").unwrap();
    h.click(cone.center(), PointerButton::Primary);
    // Sides, en el mismo menú: dos más (de 8 a 10) sin cerrarlo.
    let more = h
        .buttons
        .iter()
        .find(|(l, r)| l == "Increase" && menu.contains_rect(*r))
        .map(|(_, r)| *r)
        .expect("the Sides + in the menu");
    h.click(more.center(), PointerButton::Primary);
    h.click(more.center(), PointerButton::Primary);
    let add = in_add_menu(&h, plus, "Add Cone").expect("the menu stays open");
    let steps = h.app.doc.editor.history.done_count();
    h.click(add.center(), PointerButton::Primary);
    h.frame();
    let editor = &h.app.doc.editor;
    let objects = &editor.model().unwrap().objects;
    assert_eq!(objects.len(), 2);
    // La malla de la primitiva (sus UVs, en su lugar libre de la textura).
    let want = cs_3d::primitive_mesh(&cs_3d::Primitive::Cone { sides: 10 });
    assert_eq!(objects[1].mesh.vertices, want.vertices);
    assert_eq!(objects[1].mesh.faces.len(), want.faces.len());
    for (a, b) in objects[1].mesh.faces.iter().zip(&want.faces) {
        assert_eq!((&a.verts, a.part), (&b.verts, b.part));
    }
    assert_eq!(editor.history.done_count(), steps + 1);
    // Su fila, la activa; el menú, cerrado.
    assert_eq!(editor.selected_object(), Some(1));
    assert_eq!(editor.active_row(), Some(crate::editor::Row::Object(1)));
    assert!(add_menu_box(&h, plus).is_none(), "the menu closed");
    assert!(in_add_menu(&h, plus, "Add Cone").is_none());
    assert!(in_model_panel(&h, "Cone").is_some(), "the cone row");
}

/// Un clic afuera del menú del + solo lo cierra: no llega a lo de abajo (aquí, la fila del cubo
/// en el outliner, que lo elegiría).
#[test]
fn a_click_outside_the_add_menu_only_closes_it() {
    let mut h = without_an_object(model_panel_open(true));
    let plus = open_add_menu(&mut h);
    let menu = add_menu_box(&h, plus).expect("the add menu");
    let row = h
        .nodes
        .iter()
        .find(|(l, r)| l == "Cube" && r.width() >= 100.0)
        .map(|(_, r)| *r)
        .expect("the cube row");
    let at = pos2(row.left() + 4.0, row.center().y);
    assert!(!menu.contains(at), "{at:?} outside {menu:?}");
    h.click(at, PointerButton::Primary);
    h.frame();
    assert!(add_menu_box(&h, plus).is_none(), "the menu closed");
    assert_eq!(h.app.doc.editor.active_row(), None, "the row was not hit");
    // El clic siguiente ya llega.
    h.click(at, PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.active_row(),
        Some(crate::editor::Row::Object(0))
    );
}

#[test]
fn without_pixel_ui_the_plus_menu_has_the_types_sides_and_add() {
    let mut h = LayoutHarness::new(plain(), true);
    assert!(h.app.doc.editor.new_model("Ship", (32, 32)));
    h.frame();
    // El + del encabezado del outliner (en su fila, con New group).
    let row = h.button("New group");
    let plus = h
        .buttons
        .iter()
        .find(|(l, r)| l == "+" && r.top() == row.top())
        .map(|(_, r)| *r)
        .expect("the + of the outliner");
    h.click(plus.center(), PointerButton::Primary);
    let cylinder = h.button("Cylinder");
    // De entrada, el cubo.
    h.button("Add Cube");
    h.click(cylinder.center(), PointerButton::Primary);
    h.click(h.button("Add Cylinder").center(), PointerButton::Primary);
    h.frame();
    let editor = &h.app.doc.editor;
    assert_eq!(editor.model().unwrap().objects.len(), 2);
    assert_eq!(editor.selected_object(), Some(1));
    assert!(
        !h.buttons.iter().any(|(l, _)| l.starts_with("Add ")),
        "closed"
    );
}

#[test]
fn escape_or_a_click_outside_closes_the_add_menu_without_adding() {
    let mut h = without_an_object(model_panel_open(true));
    let plus = open_add_menu(&mut h);
    assert!(add_menu_box(&h, plus).is_some());
    tap_key(&mut h, egui::Key::Escape);
    assert!(add_menu_box(&h, plus).is_none(), "closed");
    assert!(in_add_menu(&h, plus, "Add Cube").is_none());
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), 1);
    // Un clic afuera también lo cierra.
    open_add_menu(&mut h);
    assert!(add_menu_box(&h, plus).is_some());
    let away = model_flyout(&h).left_top() + vec2(4.0, 4.0);
    h.click(away, PointerButton::Primary);
    assert!(
        add_menu_box(&h, plus).is_none(),
        "closed by a click outside"
    );
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), 1);
}

#[test]
fn the_outliner_header_has_duplicate_new_group_and_plus_at_the_right() {
    let h = model_panel_open(true);
    let flyout = model_flyout(&h);
    let title = text_in(&h, flyout, "Outliner").expect("the Outliner title");
    let [copy, group, plus] = ["Duplicate", "New group", "Add object"]
        .map(|l| in_model_panel(&h, l).unwrap_or_else(|| panic!("no {l:?}")));
    // Tres botones de 13×13 en una fila, con 4 px entre uno y otro, al final del encabezado.
    for b in [copy, group, plus] {
        assert_eq!(b.size(), vec2(13.0, 13.0), "{b:?}");
        assert_eq!(b.top(), plus.top(), "{b:?}");
    }
    assert_eq!(group.left() - copy.right(), 4.0, "{copy:?} {group:?}");
    assert_eq!(plus.left() - group.right(), 4.0, "{group:?} {plus:?}");
    let inner = model_flyout(&h).shrink(crate::dock::FLYOUT_MARGIN as f32);
    assert!(plus.right() <= inner.right(), "{plus:?} in {inner:?}");
    assert!(
        inner.right() - plus.right() <= 1.0,
        "{plus:?} at the end of {inner:?}"
    );
    // El título al principio, a la altura de los botones.
    assert!(title.left() - inner.left() <= 2.0, "{title:?}");
    assert!(title.right() < copy.left());
    assert!(
        (title.center().y - plus.center().y).abs() <= 3.0,
        "{title:?} {plus:?}"
    );
    let icon = h.drawn("icon.outliner.new_group");
    assert!(icon.iter().any(|r| group.contains_rect(*r)), "{icon:?}");
}

#[test]
fn with_an_object_chosen_the_bottom_needs_no_scroll_in_the_capture_window() {
    // La de la captura (en una más baja, como en el boceto, lo del objeto se desplaza).
    {
        let size = Vec2::new(1920.0, 1009.0);
        let h = model_panel_in(size);
        assert_eq!(h.app.doc.editor.selected_object(), Some(0));
        let (_, bottom) = model_halves(&h);
        let cube = node(&h, "Cube");
        assert!(bottom.contains_rect(cube), "{cube:?} in {bottom:?}");
        assert!(
            !h.drawn("scroll.track.v")
                .iter()
                .any(|r| r.intersect(bottom).is_positive()),
            "the object tab scrolls at {size:?}"
        );
    }
}

#[test]
fn the_outliner_lists_every_object_with_its_icon_eye_and_lock() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    away(&mut h);
    let flyout = model_flyout(&h);
    for name in ["Cube", "Cylinder"] {
        for label in [
            name.to_owned(),
            format!("Hide {name}"),
            format!("Lock {name}"),
            format!("Double sided {name}"),
        ] {
            assert!(in_model_panel(&h, &label).is_some(), "no {label:?}");
        }
    }
    let (cube, cylinder) = (node(&h, "Cube"), node(&h, "Cylinder"));
    for (piece, row) in [
        ("icon.object.cube", cube),
        ("icon.object.cylinder", cylinder),
    ] {
        assert!(
            h.drawn(piece).iter().any(|r| row.contains_rect(*r)),
            "{piece} in {row:?}"
        );
    }
    // A la derecha de la fila: ◧, el ojo y el candado (abiertos), en ese orden.
    let [double, eye, lock] = ["Double sided Cube", "Hide Cube", "Lock Cube"].map(|l| node(&h, l));
    assert!(double.right() <= eye.left() && eye.right() <= lock.left());
    assert!(cube.contains_rect(lock), "{lock:?} in {cube:?}");
    assert!(
        cube.right() - lock.right() <= 1.0,
        "{lock:?} at the end of {cube:?}"
    );
    for (piece, at) in [
        ("icon.outliner.double_sided", double),
        ("icon.outliner.eye", eye),
        ("icon.outliner.unlock", lock),
    ] {
        assert!(
            h.drawn(piece).iter().any(|r| at.contains_rect(*r)),
            "{piece} in {at:?}"
        );
    }
    // El encabezado.
    assert!(text_in(&h, flyout, "Outliner").is_some());
    for label in ["Duplicate", "New group", "Add object"] {
        assert!(in_model_panel(&h, label).is_some(), "no {label:?}");
    }
}

#[test]
fn delete_over_the_outliner_removes_the_active_row_in_one_undo_step() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    h.click(node(&h, "Cube").center(), PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.active_row(),
        Some(crate::editor::Row::Object(0))
    );
    let steps = h.app.doc.editor.history.done_count();
    // Con el puntero todavía sobre la fila.
    tap_key(&mut h, egui::Key::Delete);
    let names = |h: &LayoutHarness| -> Vec<String> {
        let model = h.app.doc.editor.model().unwrap();
        model.objects.iter().map(|o| o.name.clone()).collect()
    };
    assert_eq!(names(&h), vec!["Cylinder".to_owned()]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(names(&h), vec!["Cube".to_owned(), "Cylinder".to_owned()]);
}

#[test]
fn many_rows_scroll_in_the_outliner() {
    let mut h = model_panel_in(Vec2::new(1200.0, 640.0));
    let editor = &mut h.app.doc.editor;
    for _ in 0..39 {
        assert!(editor.add_primitive(cs_3d::Primitive::Cylinder { sides: 8 }));
    }
    let mut model = editor.model().unwrap().clone();
    for (i, o) in model.objects.iter_mut().enumerate() {
        o.name = format!("Part {i}");
    }
    assert!(editor.set_model_grouped(model, egui::Id::new("rename")));
    let h = without_an_object(h);
    let mut h = h;
    let (_, bottom) = model_halves(&h);
    let in_panel = |h: &LayoutHarness, label: &str| {
        h.nodes
            .iter()
            .find(|(l, r)| l == label && bottom.contains_rect(*r))
            .map(|(_, r)| *r)
    };
    let first = in_panel(&h, "Part 0").expect("the first row");
    assert!(in_panel(&h, "Part 39").is_none(), "they do not all fit");
    let tracks = h.drawn("scroll.track.v");
    assert!(
        tracks.iter().any(|r| bottom.contains_rect(*r)),
        "{tracks:?}"
    );
    h.frame_with(vec![Event::PointerMoved(first.center())]);
    for _ in 0..4 {
        h.frame_with(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
    }
    for _ in 0..10 {
        h.frame();
    }
    assert!(in_panel(&h, "Part 0").is_none(), "scrolled past the first");
    assert!(in_panel(&h, "Part 39").is_some(), "the last one in view");
}

/// Clic con Shift en `pos`.
fn shift_click(h: &mut LayoutHarness, pos: Pos2) {
    let shift = egui::Modifiers::SHIFT;
    let press = |pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: shift,
    };
    h.frame_with(vec![
        Event::ModifiersChanged(shift),
        Event::PointerMoved(pos),
    ]);
    h.frame_with(vec![press(true)]);
    h.frame_with(vec![press(false)]);
    h.frame_with(vec![Event::ModifiersChanged(Default::default())]);
}

/// Lo elegido en las vistas del modelo.
fn chosen_items(h: &LayoutHarness) -> Vec<cs_3d::Item> {
    let items = &h.app.doc.editor.model_edit().selection.items;
    items.iter().copied().collect()
}

#[test]
fn clicking_a_row_chooses_it_and_shift_adds() {
    use cs_3d::Item::Object;
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    h.click(node(&h, "Cube").center(), PointerButton::Primary);
    h.click(node(&h, "Cylinder").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.selected_object(), Some(1));
    assert_eq!(chosen_items(&h), vec![Object(1)]);
    let at = node(&h, "Cube").center();
    shift_click(&mut h, at);
    assert_eq!(chosen_items(&h), vec![Object(0), Object(1)]);
    // La activa es la última tocada; la otra, elegida en las vistas, también resaltada.
    assert_eq!(h.app.doc.editor.selected_object(), Some(0));
    let colors = *h.app.pixel.skin().expect("on").colors();
    let rects = filled_rects(&h);
    let cylinder = node(&h, "Cylinder");
    assert!(
        rects
            .iter()
            .any(|(c, r)| *r == cylinder && *c != colors.selection && c.a() > 0),
        "the cylinder row, highlighted"
    );
}

#[test]
fn the_eye_hides_in_one_step_and_the_render_changes() {
    let mut h = model_panel_open(true);
    let before = model_pixels(&h);
    let steps = h.app.doc.editor.history.done_count();
    let hidden = |h: &LayoutHarness| h.app.doc.editor.model().unwrap().objects[0].hidden;
    h.click(node(&h, "Hide Cube").center(), PointerButton::Primary);
    assert!(hidden(&h));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_ne!(model_pixels(&h), before, "the cube is not drawn");
    // Ahora el ojo dice Show (cerrado); otro clic lo vuelve.
    away(&mut h);
    let show = node(&h, "Show Cube");
    let off = h.drawn("icon.outliner.eye_off");
    assert!(off.iter().any(|r| show.contains_rect(*r)), "{off:?}");
    h.click(show.center(), PointerButton::Primary);
    assert!(!hidden(&h));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 2);
    assert_eq!(model_pixels(&h), before);
}

#[test]
fn new_group_wraps_the_chosen_rows_and_its_triangle_folds_it() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    h.click(node(&h, "Cube").center(), PointerButton::Primary);
    let at = node(&h, "Cylinder").center();
    shift_click(&mut h, at);
    let steps = h.app.doc.editor.history.done_count();
    let new_group = in_model_panel(&h, "New group").expect("New group");
    h.click(new_group.center(), PointerButton::Primary);
    let model = h.app.doc.editor.model().unwrap().clone();
    assert_eq!(model.groups.len(), 1);
    assert_eq!(model.groups[0].name, "Group");
    assert_eq!(model.object_parents(0), vec![0]);
    assert_eq!(model.object_parents(1), vec![0]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Los de adentro, con más sangría y debajo del grupo.
    let (group, cube) = (node(&h, "Group"), node(&h, "Cube"));
    assert!(cube.left() > group.left(), "{cube:?} {group:?}");
    assert!(cube.top() > group.top());
    // El triángulo lo cierra (sin paso de undo): lo de adentro no se dibuja.
    h.click(node(&h, "Collapse Group").center(), PointerButton::Primary);
    assert!(!h.app.doc.editor.model().unwrap().groups[0].open);
    assert!(in_model_panel(&h, "Cube").is_none(), "folded");
    assert!(in_model_panel(&h, "Cylinder").is_none(), "folded");
    assert!(in_model_panel(&h, "Expand Group").is_some());
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Y lo vuelve a abrir.
    h.click(node(&h, "Expand Group").center(), PointerButton::Primary);
    assert!(in_model_panel(&h, "Cube").is_some());
}

#[test]
fn double_click_renames_a_row() {
    let mut h = model_panel_open(true);
    let steps = h.app.doc.editor.history.done_count();
    let name = |h: &LayoutHarness| h.app.doc.editor.model().unwrap().objects[0].name.clone();
    h.double_click(node(&h, "Cube").center());
    assert!(h.nodes.iter().any(|(l, _)| l == "Rename"), "the field");
    h.frame_with(vec![Event::Text("Body".into())]);
    tap_key(&mut h, egui::Key::Enter);
    assert_eq!(name(&h), "Body");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert!(!h.nodes.iter().any(|(l, _)| l == "Rename"), "closed");
    // Esc cancela.
    h.double_click(node(&h, "Body").center());
    h.frame_with(vec![Event::Text("Head".into())]);
    tap_key(&mut h, egui::Key::Escape);
    assert_eq!(name(&h), "Body");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert!(!h.nodes.iter().any(|(l, _)| l == "Rename"), "closed");
}

#[test]
fn renaming_a_row_letter_by_letter_keeps_what_was_typed() {
    let mut h = model_panel_open(true);
    let name = |h: &LayoutHarness| h.app.doc.editor.model().unwrap().objects[0].name.clone();
    let at = node(&h, "Cube").center();
    h.double_click(at);
    // Como en la app: una letra por frame, con frames sin nada y el puntero moviéndose entre medio.
    // Cada letra llega como tecla y como texto, como en la app (y las teclas son atajos: B, O,
    // D, Y…).
    let keys = [egui::Key::B, egui::Key::O, egui::Key::D, egui::Key::Y];
    for (i, (c, key)) in ["B", "o", "d", "y"].into_iter().zip(keys).enumerate() {
        h.frame_with(vec![Event::PointerMoved(at + vec2(i as f32, 0.0))]);
        h.frame();
        let press = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        h.frame_with(vec![press(true), Event::Text(c.into())]);
        h.frame_with(vec![press(false)]);
        h.frame();
    }
    tap_key(&mut h, egui::Key::Enter);
    assert_eq!(name(&h), "Body");
}

#[test]
fn dragging_a_row_onto_a_group_moves_it_inside() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    h.click(node(&h, "Cube").center(), PointerButton::Primary);
    let new_group = in_model_panel(&h, "New group").expect("New group");
    h.click(new_group.center(), PointerButton::Primary);
    let parents = |h: &LayoutHarness| h.app.doc.editor.model().unwrap().object_parents(1);
    assert_eq!(parents(&h), Vec::<usize>::new(), "the cylinder, outside");
    let steps = h.app.doc.editor.history.done_count();
    let (from, to) = (node(&h, "Cylinder").center(), node(&h, "Group").center());
    h.drag(from, to);
    assert_eq!(parents(&h), vec![0]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

/// Lo que se ve de los textos "3D" y "Grid" (la primera fila del modo Edit) en el panel recién
/// abierto.
fn panel_top_texts(h: &LayoutHarness) -> (Vec<Rect>, Rect) {
    let find = |t: &str| -> Vec<Rect> {
        h.text_rects
            .iter()
            .filter(|(s, _, _)| s == t)
            .map(|(_, r, _)| *r)
            .collect()
    };
    (
        find("3D"),
        find("Grid").first().copied().unwrap_or(Rect::NOTHING),
    )
}

#[test]
fn the_3d_panel_opens_at_the_top_with_edit_visible() {
    for h in [model_panel_open(true), model_panel_tall()] {
        let (titles, edit) = panel_top_texts(&h);
        // Ya no hay título "3D" en el panel (solo el botón del dock y el de la barra): arriba,
        // lo general con "Grid" a la vista, y el encabezado del outliner debajo de la línea.
        let flyout = model_flyout(&h);
        assert!(
            !titles.iter().any(|t| flyout.contains_rect(*t)),
            "{titles:?}"
        );
        let (top, _) = model_halves(&h);
        let tab = node(&h, "Add object");
        assert!(flyout.contains_rect(tab), "{tab:?} in {flyout:?}");
        assert!(top.contains_rect(edit), "{edit:?} in {top:?}");
        assert!(
            edit.top() - top.top() <= 16.0,
            "{edit:?} at the top of {top:?}"
        );
        assert!(edit.bottom() < tab.top(), "{edit:?} above {tab:?}");
    }
}

#[test]
fn reopening_the_3d_panel_starts_at_the_top_even_if_it_was_scrolled() {
    // Una ventana baja: el panel no cabe y se desplaza.
    let mut h = model_panel_in(Vec2::new(1200.0, 380.0));
    let (_, top) = panel_top_texts(&h);
    let over = top.center();
    h.frame_with(vec![Event::PointerMoved(over)]);
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -120.0),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    }]);
    for _ in 0..10 {
        h.frame();
    }
    let (_, scrolled) = panel_top_texts(&h);
    // Desplazado, "Grid" sale de la vista (no se dibuja) o queda más arriba.
    assert!(
        scrolled == Rect::NOTHING || scrolled.top() < top.top() - 20.0,
        "el panel no se desplazó"
    );
    // Ir al lienzo y volver a la pestaña del modelo (el panel no se cierra de otro modo).
    let doc = h.button(&h.app.doc.location.name.clone());
    let ship = h.app.doc.editor.workspace().expect("the model's workspace");
    h.click(doc.center(), PointerButton::Primary);
    h.frame();
    assert!(!h.app.doc.editor.model_edit_on());
    h.app.tabs.open_view(&mut h.app.doc, ship);
    for _ in 0..3 {
        h.frame();
    }
    let (_, reopened) = panel_top_texts(&h);
    assert_eq!(reopened.top(), top.top());
}

#[test]
fn the_3d_panel_has_the_modes_and_the_grid() {
    let mut h = model_panel_open(true);
    let text = |h: &LayoutHarness, t: &str| {
        h.text_rects
            .iter()
            .find(|(s, _, _)| s == t)
            .map(|(_, r, _)| *r)
            .unwrap_or_else(|| panic!("no text {t:?}"))
    };
    // Como el boceto: los cuatro íconos (13×13, uno cada 17) al principio de la fila, sin
    // texto, y "Grid" con su select (de 46) al final.
    let modes: Vec<Rect> = ["Object", "Vertex", "Edge", "Face"]
        .iter()
        .map(|l| node(&h, l))
        .collect();
    assert!(!h.text_rects.iter().any(|(s, _, _)| s == "Vertex"));
    for (k, r) in modes.iter().enumerate() {
        assert_eq!(r.size(), Vec2::splat(13.0), "{r:?}");
        assert_eq!(r.left() - modes[0].left(), 17.0 * k as f32);
        assert_eq!(r.top(), modes[0].top());
    }
    let grid = in_model_panel(&h, "1/4").expect("the grid select");
    assert_eq!(grid.right(), model_row_right(&h));
    assert_eq!(grid.width(), 46.0);
    assert_eq!(modes[0].top() - grid.top(), 2.0);
    let label = text_in(&h, model_flyout(&h), "Grid").expect("Grid");
    assert!(
        label.right() < grid.left() && label.left() > modes[3].right(),
        "{label:?} {grid:?} {modes:?}"
    );
    assert!(h.app.doc.editor.model_edit_on());
    h.click(modes[1].center(), PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.model_edit().selection.mode,
        cs_3d::SelectMode::Vertex
    );
    h.click(text(&h, "1/4").center(), PointerButton::Primary);
    h.click(text(&h, "1/2").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model_edit().grid, 8);
}

#[test]
fn delete_is_for_the_selection_and_the_outliner_has_no_delete_mark() {
    // El objeto lo quita Supr en el outliner (no hay marca en su fila); el botón Delete (de la
    // fila de topología) borra lo elegido en las vistas: sin nada elegido, deshabilitado.
    let h = model_panel_tall();
    assert!(!h.nodes.iter().any(|(l, _)| l == "Delete Cube"));
    assert_eq!(
        h.buttons.iter().filter(|(l, _)| l == "Delete").count(),
        1,
        "{:?}",
        h.buttons
    );
    assert!(h.app.doc.editor.model_edit().selection.is_empty());
    assert!(h.disabled.iter().any(|l| l == "Delete"));
}

/// La primera vez que se ve el texto `t` dentro de `area`.
fn text_in(h: &LayoutHarness, area: Rect, t: &str) -> Option<Rect> {
    h.text_rects
        .iter()
        .find(|(s, r, _)| s == t && area.contains_rect(*r))
        .map(|(_, r, _)| *r)
}

// --- El panel 3D: textura, cámara, luz y contorno ---

/// Como [`model_panel_open`] con un modelo, en una ventana alta (que quepa todo el panel), y con
/// un artboard "Skin" de pixel art más para elegir de textura.
fn model_panel_tall() -> LayoutHarness {
    model_panel_in(Vec2::new(1200.0, 1800.0))
}

/// Como [`model_panel_tall`] en una ventana de `size`.
fn model_panel_in(size: Vec2) -> LayoutHarness {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, size);
    let editor = &mut h.app.doc.editor;
    assert!(editor.add_artboard_with_sprite(
        "Skin".into(),
        cs_core::Sprite::new(16, 16, cs_core::ColorMode::Rgba)
    ));
    assert!(editor.new_model("Ship", (32, 32)));
    // En el workspace del modelo no hay barra de opciones y el panel 3D va fijo: todo se acomoda
    // en el frame que sigue.
    h.frame();
    h.frame();
    h
}

fn model_pixels(h: &LayoutHarness) -> Vec<Rgba> {
    let editor = &h.app.doc.editor;
    let s = cs_render::materialize(&editor.document, editor.sprite(), 0..1);
    cs_render::render_frame(&s, 0)
}

#[test]
fn the_texture_select_skips_the_library_and_marks_a_missing_texture() {
    let mut h = model_panel_tall();
    let doc = &mut h.app.doc.editor.document;
    doc.artboards
        .iter_mut()
        .find(|a| a.name == "Skin")
        .unwrap()
        .in_library = true;
    h.frame();
    h.click(node(&h, "Ship Texture").center(), PointerButton::Primary);
    assert!(
        !h.nodes.iter().any(|(l, _)| l == "Skin"),
        "a library artboard is offered"
    );
    h.click(node(&h, "None").center(), PointerButton::Primary);
    // Una textura que ya no existe (borrada por un script): "(missing)", no "None".
    let editor = &mut h.app.doc.editor;
    let active = editor.active;
    editor
        .document
        .artboard_mut(active)
        .unwrap()
        .model
        .as_mut()
        .unwrap()
        .texture = Some(cs_core::ArtboardId(999));
    h.frame();
    h.frame();
    node(&h, "(missing)");
}

#[test]
fn choosing_another_texture_in_the_model_panel_changes_the_model() {
    let mut h = model_panel_tall();
    let before = h.app.doc.editor.model().unwrap().texture;
    h.click(node(&h, "Ship Texture").center(), PointerButton::Primary);
    h.click(node(&h, "Skin").center(), PointerButton::Primary);
    let after = h.app.doc.editor.model().unwrap().texture;
    assert_ne!(before, after);
    let skin = h
        .app
        .doc
        .editor
        .document
        .artboards
        .iter()
        .find(|a| a.name == "Skin")
        .unwrap()
        .id;
    assert_eq!(after, Some(skin));
}

#[test]
fn new_texture_adds_a_template_artboard_and_assigns_it_in_one_step() {
    let mut h = model_panel_tall();
    let steps = h.app.doc.editor.history.done_count();
    let count = h.app.doc.editor.document.artboards.len();
    h.click(
        in_model_panel(&h, "New").unwrap().center(),
        PointerButton::Primary,
    );
    let editor = &h.app.doc.editor;
    assert_eq!(editor.document.artboards.len(), count + 1);
    assert_eq!(editor.history.done_count(), steps + 1);
    let new = editor.document.artboards.last().unwrap();
    assert_eq!(new.name, "Ship Texture");
    assert_eq!(editor.model().unwrap().texture, Some(new.id));
    assert!(editor.is_model(), "the model stays active");
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.document.artboards.len(), count);
}

/// Hasta dónde llegan las filas de lo general del panel 3D: lo de adentro sin la barra del scroll
/// y el aire de antes (como las filas de los ejes).
fn model_row_right(h: &LayoutHarness) -> f32 {
    let inner = model_flyout(h).shrink(crate::dock::FLYOUT_MARGIN as f32);
    inner.right() - crate::pixel_ui::SCROLL_BAR - 4.0
}

/// El ritmo de las filas de lo general (el del boceto): del título "Camera" a su fila y de esa
/// fila al título que sigue ("Light & Outline").
fn row_rhythm(h: &LayoutHarness) -> (f32, f32) {
    let flyout = model_flyout(h);
    let camera = text_in(h, flyout, "Camera").unwrap();
    let light = text_in(h, flyout, "Light & Outline").unwrap();
    let row = in_model_panel(h, "Perspective 60°").expect("the projection select");
    (row.top() - camera.top(), light.top() - row.top())
}

/// El select del tamaño de la textura: el control del panel en la fila de `texture`, a la
/// derecha de `new`.
fn texture_size_select(h: &LayoutHarness, texture: Rect, new: Rect) -> Rect {
    let flyout = model_flyout(h);
    h.nodes
        .iter()
        .find(|(_, r)| {
            flyout.contains_rect(*r) && r.top() == texture.top() && r.left() > new.right()
        })
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no Texture Size select in {:?}", h.nodes))
}

#[test]
fn the_texture_row_glues_new_to_its_select_with_the_size_in_the_right_column() {
    let h = model_panel_tall();
    let (top, _) = model_halves(&h);
    let texture = node(&h, "Ship Texture");
    let new = in_model_panel(&h, "New").expect("New beside the texture");
    // New pegado al select: comparten la columna del borde, del mismo alto.
    assert_eq!(new.left(), texture.right() - 1.0, "{texture:?} {new:?}");
    assert_eq!((new.top(), new.height()), (texture.top(), texture.height()));
    // El tamaño, en la columna de la derecha (≈ 60 % de la fila, como el boceto), en la misma
    // fila, 4 después de New y hasta el final de la fila.
    let size = texture_size_select(&h, texture, new);
    assert_eq!(size.left() - new.right(), 4.0, "{new:?} {size:?}");
    let right = model_row_right(&h);
    assert_eq!(size.right(), right);
    let at = (size.left() - texture.left()) / (right - texture.left());
    assert!((0.58..=0.66).contains(&at), "column at {at}");
    // Las etiquetas, arriba de cada una y en la misma fila.
    let label = text_in(&h, top, "Texture").unwrap();
    let size_label = text_in(&h, top, "Texture Size").unwrap();
    assert_eq!(label.top(), size_label.top(), "{label:?} {size_label:?}");
    // De la etiqueta al select, lo del boceto: como del título de la cámara a su fila.
    let (title, row) = row_rhythm(&h);
    assert_eq!(texture.top() - label.top(), title, "{label:?} {texture:?}");
    assert!(row > 0.0);
    assert!((label.left() - texture.left()).abs() <= 1.0, "{label:?}");
    assert!(
        (size_label.left() - size.left()).abs() <= 1.0,
        "{size_label:?}"
    );
    assert!(in_model_panel(&h, "New Texture").is_none());
}

#[test]
fn without_a_texture_the_select_asks_for_an_option_and_keeps_none_in_the_list() {
    let mut h = model_panel_tall();
    h.click(node(&h, "Ship Texture").center(), PointerButton::Primary);
    h.click(node(&h, "None").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model().unwrap().texture, None);
    let texture = in_model_panel(&h, "Select an option…").expect("the placeholder");
    // La fila no cambia: New pegado y el tamaño (vacío) a la derecha.
    let new = in_model_panel(&h, "New").unwrap();
    assert_eq!(new.left(), texture.right() - 1.0);
    texture_size_select(&h, texture, new);
    // None sigue en la lista.
    h.click(texture.center(), PointerButton::Primary);
    node(&h, "None");
}

#[test]
fn the_camera_row_is_a_projection_select_and_the_preset_group() {
    let h = model_panel_tall();
    let (top, _) = model_halves(&h);
    let texture = node(&h, "Ship Texture");
    let projection = in_model_panel(&h, "Perspective 60°").expect("the projection select");
    assert!(projection.top() > texture.bottom());
    assert_eq!(projection.left(), texture.left());
    let presets: Vec<Rect> = ["Iso", "Top", "Side", "Front"]
        .iter()
        .map(|l| in_model_panel(&h, l).unwrap_or_else(|| panic!("{l}")))
        .collect();
    // El grupo, pegado (cada uno comparte la columna con el que sigue), en la fila del select,
    // 4 después de él y hasta el final de la fila.
    for pair in presets.windows(2) {
        assert_eq!(pair[1].left(), pair[0].right() - 1.0, "{pair:?}");
    }
    for r in &presets {
        assert_eq!(
            (r.top(), r.height()),
            (projection.top(), projection.height())
        );
    }
    assert_eq!(presets[0].left() - projection.right(), 4.0);
    assert_eq!(presets[3].right(), model_row_right(&h));
    // Como el boceto: de la fila de la textura al título de la cámara, lo mismo que de la fila
    // de la cámara al título que sigue; y de la etiqueta de la textura a su select, lo de la
    // cámara a la suya.
    let label = text_in(&h, top, "Camera").unwrap();
    let (title, row) = row_rhythm(&h);
    assert_eq!(label.top() - texture.top(), row, "{texture:?} {label:?}");
    let texture_label = text_in(&h, top, "Texture").unwrap();
    assert_eq!(texture.top() - texture_label.top(), title);
    let light = text_in(&h, top, "Light & Outline").unwrap();
    assert_eq!(
        light.top() - projection.top(),
        row,
        "{projection:?} {light:?}"
    );
    // Ni Yaw, Pitch ni Reset en la cámara (la mueven la órbita, el gizmo y los presets): los
    // únicos Yaw y Pitch son los de la luz.
    assert!(in_model_panel(&h, "Reset").is_none());
    for t in ["Yaw", "Pitch"] {
        let all: Vec<Rect> = h
            .text_rects
            .iter()
            .filter(|(s, r, _)| s == t && top.contains_rect(*r))
            .map(|(_, r, _)| *r)
            .collect();
        assert_eq!(all.len(), 1, "{t}: {all:?}");
        assert!(all[0].top() > light.bottom(), "{t} is the light's");
    }
}

#[test]
fn each_camera_preset_is_one_undo_step() {
    let mut h = model_panel_tall();
    let presets = [
        ("Iso", cs_core::CameraPreset::Iso),
        ("Top", cs_core::CameraPreset::Top),
        ("Side", cs_core::CameraPreset::Side),
        ("Front", cs_core::CameraPreset::Front),
    ];
    for (label, preset) in presets {
        let before = h.app.doc.editor.model().unwrap().camera;
        let steps = h.app.doc.editor.history.done_count();
        h.click(node(&h, label).center(), PointerButton::Primary);
        let cam = h.app.doc.editor.model().unwrap().camera;
        assert_eq!(
            cam,
            preset.camera(before.distance, before.target),
            "{label}"
        );
        assert_eq!(cam.projection, cs_core::Projection::Orthographic);
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1, "{label}");
        h.app.doc.editor.undo();
        assert_eq!(h.app.doc.editor.model().unwrap().camera, before, "{label}");
        h.frame();
    }
}

#[test]
fn the_projection_select_switches_the_camera_in_one_step() {
    let mut h = model_panel_tall();
    let steps = h.app.doc.editor.history.done_count();
    h.click(node(&h, "Perspective 60°").center(), PointerButton::Primary);
    h.click(node(&h, "Orthographic").center(), PointerButton::Primary);
    let cam = h.app.doc.editor.model().unwrap().camera;
    assert_eq!(cam.projection, cs_core::Projection::Orthographic);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    in_model_panel(&h, "Orthographic").expect("the select shows it");
    h.app.doc.editor.undo();
    let cam = h.app.doc.editor.model().unwrap().camera;
    assert_eq!(cam, cs_core::Model3D::default().camera);
}

#[test]
fn the_projection_select_offers_fov_presets() {
    let mut h = model_panel_tall();
    let steps = h.app.doc.editor.history.done_count();
    h.click(node(&h, "Perspective 60°").center(), PointerButton::Primary);
    for label in ["Perspective 30°", "Perspective 45°", "Orthographic"] {
        node(&h, label);
    }
    h.click(node(&h, "Perspective 90°").center(), PointerButton::Primary);
    let cam = h.app.doc.editor.model().unwrap().camera;
    assert_eq!(
        cam.projection,
        cs_core::Projection::Perspective { fov: 900 }
    );
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    in_model_panel(&h, "Perspective 90°").expect("the select shows it");
}

#[test]
fn the_projection_select_goes_back_to_perspective() {
    let mut h = model_panel_tall();
    h.click(node(&h, "Perspective 60°").center(), PointerButton::Primary);
    h.click(node(&h, "Orthographic").center(), PointerButton::Primary);
    h.click(node(&h, "Orthographic").center(), PointerButton::Primary);
    h.click(node(&h, "Perspective 60°").center(), PointerButton::Primary);
    let cam = h.app.doc.editor.model().unwrap().camera;
    assert_eq!(
        cam.projection,
        cs_core::Model3D::default().camera.projection
    );
}

#[test]
fn turning_the_light_off_changes_the_render() {
    let mut h = model_panel_tall();
    let lit = model_pixels(&h);
    assert!(check(&h, "Light").1);
    h.click(check(&h, "Light").0.center(), PointerButton::Primary);
    assert!(!h.app.doc.editor.model().unwrap().light.enabled);
    assert_ne!(model_pixels(&h), lit);
}

#[test]
fn dither_turns_on_the_dithered_shadows_in_one_step() {
    let mut h = model_panel_tall();
    let (dither, on) = check(&h, "Dither");
    assert!(!on, "off by default");
    // En la fila de Outline, a su derecha.
    let outline = check(&h, "Outline").0;
    assert!(dither.left() > outline.right());
    assert!(dither.top() < outline.bottom() && dither.bottom() > outline.top());
    let plain = model_pixels(&h);
    let steps = h.app.doc.editor.history.done_count();
    h.click(dither.center(), PointerButton::Primary);
    assert!(h.app.doc.editor.model().unwrap().light.dither);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_ne!(model_pixels(&h), plain, "the render changes");
    // Sin luz se ve pero no se usa.
    h.click(check(&h, "Light").0.center(), PointerButton::Primary);
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    h.click(check(&h, "Dither").0.center(), PointerButton::Primary);
    assert!(h.app.doc.editor.model().unwrap().light.dither, "unchanged");
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

#[test]
fn turning_the_outline_on_sets_the_primary_color() {
    let mut h = model_panel_tall();
    assert_eq!(h.app.doc.editor.model().unwrap().outline, None);
    h.click(check(&h, "Outline").0.center(), PointerButton::Primary);
    let editor = &h.app.doc.editor;
    let primary = editor.resolve(editor.primary);
    assert_eq!(editor.model().unwrap().outline, Some(primary));
    h.click(check(&h, "Outline").0.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model().unwrap().outline, None);
}

#[test]
fn the_model_tab_has_a_sheet_settings_button() {
    let h = model_panel_tall();
    h.button("Sheet settings…");
    assert!(!h.buttons.iter().any(|(l, _)| l == "Bake…"));
}

#[test]
fn every_model_panel_control_fits_beside_the_scroll_bar() {
    // Una ventana baja y una alta (cabe todo, sin barra).
    // Con el cubo elegido y sin objeto: los 4 steppers de lo general (Yaw y Pitch de la luz; la
    // cámara ya no los tiene; la transform va en la barrita del objeto).
    let sizes = [Vec2::new(1200.0, 480.0), Vec2::new(1200.0, 1800.0)];
    for (size, object) in sizes.into_iter().flat_map(|s| [(s, true), (s, false)]) {
        let mut h = model_panel_in(size);
        if !object {
            h = without_an_object(h);
        }
        h.frame();
        let flyout = model_flyout(&h);
        // Lo de adentro del marco y, sin la barra del scroll (a la derecha), lo del contenido.
        let inner = flyout.shrink(crate::dock::FLYOUT_MARGIN as f32);
        let content = Rect::from_min_max(
            inner.min,
            egui::pos2(inner.right() - crate::pixel_ui::SCROLL_BAR, inner.bottom()),
        );
        let inside = |r: Rect, area: Rect| r.left() >= area.left() && r.right() <= area.right();
        let controls: Vec<_> = h
            .buttons
            .iter()
            .chain(h.nodes.iter())
            .filter(|(_, r)| r.intersects(flyout))
            .collect();
        let steppers = controls
            .iter()
            .filter(|(l, _)| l == "Increase" || l == "Decrease");
        assert!(steppers.clone().count() >= 4, "{controls:?}");
        // Los steppers (de ancho fijo) caben siempre al lado de la barra.
        for (label, r) in steppers {
            assert!(inside(*r, content), "{label} {r:?} outside {content:?}");
        }
        // Todo, al lado de la barra de su mitad si la hay (las filas de la lista llenan el
        // ancho). El encabezado del outliner (con el +) está arriba del scroll de abajo: llega
        // hasta el borde.
        let (top, bottom) = model_halves(&h);
        let tabs = node(&h, "Add object");
        let scrolls = |half: Rect| {
            h.drawn("scroll.track.v")
                .iter()
                .any(|r| half.contains_rect(*r) && r.left() >= content.right() - 1.0)
        };
        let (top_scrolls, bottom_scrolls) = (scrolls(top), scrolls(bottom));
        for (label, r) in controls {
            let scrolled = if r.top() < top.bottom() {
                top_scrolls
            } else {
                bottom_scrolls && r.top() > tabs.bottom()
            };
            let area = if scrolled { content } else { inner };
            assert!(inside(*r, area), "{label} {r:?} outside {area:?}");
        }
    }
}

#[test]
fn values_outside_the_panel_ranges_are_not_rewritten_by_drawing_the_panel() {
    let mut h = model_panel_tall();
    // Lo que deja orbitar mucho, un script o un archivo: fuera de lo que muestran los steppers.
    let mut model = h.app.doc.editor.model().unwrap().clone();
    model.camera.yaw = 7200;
    model.light.yaw = -5000;
    let t = &mut model.objects[0].transform;
    t.rotation = cs_core::Vec3i {
        x: 4000,
        y: -9000,
        z: 3610,
    };
    t.scale = cs_core::Vec3i {
        x: 20000,
        y: -1000,
        z: 5,
    };
    t.position.x = 9999 * cs_core::FIXED;
    assert!(
        h.app
            .doc
            .editor
            .set_model_grouped(model.clone(), egui::Id::new("script"))
    );
    h.app.doc.editor.end_group();
    let steps = h.app.doc.editor.history.done_count();
    for _ in 0..3 {
        h.frame();
    }
    assert_eq!(h.app.doc.editor.model().unwrap(), &model);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    // Y sin objeto elegido (solo lo general: la cámara y la luz).
    let h = without_an_object(h);
    assert_eq!(h.app.doc.editor.model().unwrap(), &model);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

// --- El panel 3D: la ventana Shading ---

/// [`model_panel_tall`] con la ventana Shading abierta (con Shading… del panel).
fn shading_open() -> LayoutHarness {
    let mut h = model_panel_tall();
    assert!(
        !h.nodes.iter().any(|(l, _)| l == "Shade 6 step 1"),
        "not in the panel"
    );
    h.click(h.button("Shading…").center(), PointerButton::Primary);
    h.frame();
    assert!(h.app.model_panel.shading);
    h
}

#[test]
fn shading_opens_its_window_and_the_x_closes_it() {
    let mut h = shading_open();
    let flyout = model_flyout(&h);
    let title = h
        .text_rects
        .iter()
        .find(|(s, r, _)| s == "Shading" && !flyout.contains_rect(*r))
        .map(|(_, r, _)| *r)
        .expect("the window title");
    // La cuadrícula, debajo del título de la ventana (y no en el panel).
    let cell = node(&h, "Shade 6 step 1");
    assert!(cell.top() > title.bottom(), "{cell:?} under {title:?}");
    // El panel sigue usable con la ventana abierta (no es un diálogo).
    assert!(!h.app.is_modal());
    h.app.model_panel.shading = false;
    h.frame();
    assert!(!h.nodes.iter().any(|(l, _)| l == "Shade 6 step 1"));
}

#[test]
fn the_shading_window_closes_without_a_model() {
    let mut h = shading_open();
    h.app
        .doc
        .editor
        .document
        .artboards
        .iter_mut()
        .for_each(|a| a.model = None);
    h.frame();
    h.frame();
    assert!(!h.app.model_panel.shading);
}

#[test]
fn the_light_row_has_light_yaw_and_pitch_like_the_mockup() {
    let h = model_panel_tall();
    let (top, _) = model_halves(&h);
    let light = check(&h, "Light").0;
    let yaw = text_in(&h, top, "Yaw").unwrap();
    let pitch = text_in(&h, top, "Pitch").unwrap();
    // En la fila de la casilla, en orden.
    for r in [yaw, pitch] {
        assert!(
            r.top() < light.bottom() && r.bottom() > light.top(),
            "{r:?} {light:?}"
        );
    }
    assert!(light.right() < yaw.left() && yaw.right() < pitch.left());
    // Los dos steppers, del mismo ancho (±1) y el de Pitch hasta el final de la fila.
    let mut plus: Vec<Rect> = h
        .buttons
        .iter()
        .filter(|(l, r)| l == "Increase" && top.contains_rect(*r))
        .map(|(_, r)| *r)
        .collect();
    let mut minus: Vec<Rect> = h
        .buttons
        .iter()
        .filter(|(l, r)| l == "Decrease" && top.contains_rect(*r))
        .map(|(_, r)| *r)
        .collect();
    assert_eq!((plus.len(), minus.len()), (2, 2), "{plus:?} {minus:?}");
    plus.sort_by(|a, b| a.left().total_cmp(&b.left()));
    minus.sort_by(|a, b| a.left().total_cmp(&b.left()));
    let widths: Vec<f32> = (0..2).map(|i| plus[i].right() - minus[i].left()).collect();
    assert!((widths[0] - widths[1]).abs() <= 1.0, "{widths:?}");
    assert_eq!(plus[1].right(), model_row_right(&h));
    // Outline, debajo.
    let outline = check(&h, "Outline").0;
    assert!(outline.top() >= light.bottom(), "{outline:?} {light:?}");
}

#[test]
fn shading_sheet_and_gif_settings_end_the_row_after_the_outline() {
    let h = model_panel_tall();
    let shading = h.button("Shading…");
    let sheet = h.button("Sheet settings…");
    let gif = h.button("GIF settings…");
    assert_eq!(gif.right(), model_row_right(&h));
    assert_eq!(gif.left() - sheet.right(), 4.0);
    assert_eq!(sheet.left() - shading.right(), 4.0);
    assert_eq!(shading.top(), sheet.top());
    assert_eq!(gif.top(), sheet.top());
    assert!(shading.top() > check(&h, "Outline").0.bottom());
    // Caben en la fila (sin taparse con el borde de la izquierda).
    let (top, _) = model_halves(&h);
    assert!(shading.left() >= top.left(), "{shading:?} in {top:?}");
}

#[test]
fn gif_settings_opens_its_dialog() {
    let mut h = model_panel_tall();
    let gif = h.button("GIF settings…");
    h.click(gif.center(), PointerButton::Primary);
    assert!(h.app.gif.is_some());
}

/// El color de la paleta del modelo con índice `i`.
fn model_palette(h: &LayoutHarness, i: usize) -> Rgba {
    h.app.doc.editor.sprite().palette.colors()[i]
}

#[test]
fn picking_a_shade_pins_the_step_in_one_undo_step_and_changes_the_render() {
    let mut h = shading_open();
    // El 6 es el color del cubo en la plantilla de la textura.
    let (cube, red) = (model_palette(&h, 6), model_palette(&h, 27));
    let before = model_pixels(&h);
    let steps = h.app.doc.editor.history.done_count();
    h.click(node(&h, "Shade 6 step 1").center(), PointerButton::Primary);
    h.click(node(&h, "Shade color 27").center(), PointerButton::Primary);
    let editor = &h.app.doc.editor;
    assert_eq!(editor.model().unwrap().shading.get(cube, 1), Some(red));
    assert_eq!(editor.history.done_count(), steps + 1);
    assert_ne!(model_pixels(&h), before);
    // El popup se cerró.
    assert!(!h.nodes.iter().any(|(l, _)| l == "Shade color 27"));
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.model().unwrap().shading.get(cube, 1), None);
}

#[test]
fn a_right_click_on_a_shade_step_returns_it_to_auto() {
    let mut h = shading_open();
    let (cube, red) = (model_palette(&h, 6), model_palette(&h, 27));
    h.click(node(&h, "Shade 6 step 2").center(), PointerButton::Primary);
    h.click(node(&h, "Shade color 27").center(), PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.model().unwrap().shading.get(cube, 2),
        Some(red)
    );
    h.click(
        node(&h, "Shade 6 step 2").center(),
        PointerButton::Secondary,
    );
    let shading = &h.app.doc.editor.model().unwrap().shading;
    assert_eq!(shading, &cs_core::ShadingTable::default());
}

#[test]
fn auto_empties_the_shading_table() {
    let mut h = shading_open();
    assert!(
        h.disabled.iter().any(|l| l == "Auto"),
        "Auto with an empty table"
    );
    let mut table = cs_core::ShadingTable::default();
    table.set(model_palette(&h, 6), 1, Some(model_palette(&h, 27)));
    table.set(model_palette(&h, 9), 2, Some(model_palette(&h, 0)));
    assert!(h.app.doc.editor.set_shading(table));
    h.frame();
    h.frame();
    assert!(!h.disabled.iter().any(|l| l == "Auto"));
    let steps = h.app.doc.editor.history.done_count();
    h.click(h.button("Auto").center(), PointerButton::Primary);
    let editor = &h.app.doc.editor;
    assert_eq!(
        editor.model().unwrap().shading,
        cs_core::ShadingTable::default()
    );
    assert_eq!(editor.history.done_count(), steps + 1);
}

/// Cambia la paleta del modelo (el artboard activo) sin pasar por el historial.
fn set_model_palette(h: &mut LayoutHarness, colors: Vec<Rgba>) {
    let editor = &mut h.app.doc.editor;
    let id = editor.active;
    editor.document.artboard_mut(id).unwrap().sprite.palette = cs_core::Palette::new(colors);
    h.frame();
    h.frame();
}

#[test]
fn entries_of_colors_outside_the_palette_show_as_orphan_columns() {
    let mut h = shading_open();
    let gone = Rgba::new(1, 2, 3, 255);
    let mut table = cs_core::ShadingTable::default();
    table.set(gone, 1, Some(model_palette(&h, 27)));
    table.set(gone, 2, Some(model_palette(&h, 0)));
    // Un color de la paleta que deja de ser opaco también queda huérfano.
    let mut colors = h.app.doc.editor.sprite().palette.colors().to_vec();
    let faded = colors[9];
    colors[9].a = 0;
    table.set(faded, 1, Some(model_palette(&h, 6)));
    assert!(h.app.doc.editor.set_shading(table));
    set_model_palette(&mut h, colors);
    let flyout = h.ctx.content_rect();
    // Ordenadas como la tabla: (1, 2, 3) antes que el 9 de la paleta.
    let first = node(&h, "Shade orphan 0 color");
    let second = node(&h, "Shade orphan 1 color");
    for label in [
        "Shade orphan 0 step 1",
        "Shade orphan 0 step 2",
        "Shade orphan 1 step 1",
    ] {
        assert!(flyout.contains_rect(node(&h, label)), "{label}");
    }
    assert!(flyout.contains_rect(first) && flyout.contains_rect(second));
    assert!(!h.nodes.iter().any(|(l, _)| l == "Shade 9 step 1"));
    // Después de las columnas de la paleta.
    let n = h.app.doc.editor.sprite().palette.len();
    let last = node(&h, &format!("Shade {} step 1", n - 1));
    let orphan = node(&h, "Shade orphan 0 step 1");
    assert!(
        orphan.min.y > last.min.y || (orphan.min.y == last.min.y && orphan.min.x > last.min.x),
        "{orphan:?} after {last:?}"
    );
    // Clic derecho en un paso lo quita; en el color, la entrada entera.
    let steps = h.app.doc.editor.history.done_count();
    h.click(
        node(&h, "Shade orphan 0 step 2").center(),
        PointerButton::Secondary,
    );
    let shading = &h.app.doc.editor.model().unwrap().shading;
    assert_eq!(shading.get(gone, 2), None);
    assert_eq!(shading.get(gone, 1), Some(model_palette(&h, 27)));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.click(
        node(&h, "Shade orphan 1 color").center(),
        PointerButton::Secondary,
    );
    let shading = &h.app.doc.editor.model().unwrap().shading;
    assert_eq!(shading.get(faded, 1), None);
    assert_eq!(shading.entries.len(), 1);
    // Sin esa columna la ventana se angosta y se vuelve a centrar.
    h.frame();
    h.frame();
    h.click(
        node(&h, "Shade orphan 0 step 1").center(),
        PointerButton::Secondary,
    );
    let shading = &h.app.doc.editor.model().unwrap().shading;
    assert_eq!(shading, &cs_core::ShadingTable::default());
    h.frame();
    assert!(!h.nodes.iter().any(|(l, _)| l.starts_with("Shade orphan")));
}

#[test]
fn the_shade_popup_offers_only_opaque_colors() {
    let mut h = shading_open();
    let mut colors = h.app.doc.editor.sprite().palette.colors().to_vec();
    colors[27].a = 0;
    set_model_palette(&mut h, colors);
    h.click(node(&h, "Shade 6 step 1").center(), PointerButton::Primary);
    assert!(h.nodes.iter().any(|(l, _)| l == "Shade color 26"));
    assert!(!h.nodes.iter().any(|(l, _)| l == "Shade color 27"));
}

#[test]
fn the_shading_grid_is_disabled_with_the_light_off() {
    let mut h = shading_open();
    assert!(!h.disabled.iter().any(|l| l == "Shade 6 step 1"));
    let mut model = h.app.doc.editor.model().unwrap().clone();
    model.light.enabled = false;
    assert!(
        h.app
            .doc
            .editor
            .set_model_grouped(model, egui::Id::new("script"))
    );
    h.app.doc.editor.end_group();
    h.frame();
    h.frame();
    assert!(h.disabled.iter().any(|l| l == "Shade 6 step 1"));
    assert!(
        h.text_rects
            .iter()
            .any(|(s, _, _)| s == "Turn the light on to edit.")
    );
    let steps = h.app.doc.editor.history.done_count();
    h.click(node(&h, "Shade 6 step 1").center(), PointerButton::Primary);
    assert!(
        !h.nodes.iter().any(|(l, _)| l == "Shade color 27"),
        "no popup"
    );
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

#[test]
fn every_shade_cell_is_on_screen() {
    let h = shading_open();
    let flyout = h.ctx.content_rect();
    let palette = h.app.doc.editor.sprite().palette.colors().to_vec();
    let mut count = 0;
    for (i, c) in palette.iter().enumerate().filter(|(_, c)| c.a == 255) {
        for k in 1..=2 {
            let r = node(&h, &format!("Shade {i} step {k}"));
            assert!(
                flyout.contains_rect(r),
                "{i} {k} {r:?} vs {flyout:?} ({c:?})"
            );
            count += 1;
        }
    }
    assert_eq!(count, 2 * palette.len());
}

/// Un modelo en modo Edit con el objeto elegido, el puntero sobre Front y una G pedida.
fn grabbing_with_the_keyboard() -> (LayoutHarness, Pos2) {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Object);
    let c = front_screen(&h, [0.0, 0.0, 0.0]);
    h.click(c, PointerButton::Primary);
    assert!(!h.app.doc.editor.model_edit().selection.items.is_empty());
    h.frame_with(vec![Event::PointerMoved(c)]);
    h.app
        .doc
        .editor
        .request_model_key_op(crate::editor::KeyOp::Grab);
    h.frame();
    assert!(h.app.doc.editor.model_edit().op.is_some(), "grab started");
    let to = c + vec2(60.0, 0.0);
    h.frame_with(vec![Event::PointerMoved(to)]);
    (h, to)
}

fn mesh_x(h: &LayoutHarness) -> Vec<f32> {
    let m = h.app.doc.editor.model().unwrap();
    cs_3d::world_points(m, 0).iter().map(|p| p[0]).collect()
}

#[test]
fn a_left_click_commits_a_keyboard_grab_in_one_undo_step() {
    let (mut h, to) = grabbing_with_the_keyboard();
    let before = mesh_x(&h);
    h.click(to, PointerButton::Primary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_ne!(mesh_x(&h), before, "moved");
    h.app.doc.editor.undo();
    assert_eq!(mesh_x(&h), before, "one undo step");
}

#[test]
fn a_right_click_cancels_a_keyboard_grab() {
    let (mut h, to) = grabbing_with_the_keyboard();
    let before = mesh_x(&h);
    let steps = h.app.doc.editor.history.done_count();
    h.click(to, PointerButton::Secondary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(mesh_x(&h), before);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

#[test]
fn a_left_click_in_the_perspective_still_commits_a_keyboard_grab() {
    let (mut h, _) = grabbing_with_the_keyboard();
    let selection = h.app.doc.editor.model_edit().selection.clone();
    let steps = h.app.doc.editor.history.done_count();
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    h.click(quads[1].center(), PointerButton::Primary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_eq!(h.app.doc.editor.model_edit().selection, selection);
}

#[test]
fn number_keys_switch_the_selection_mode() {
    let mut h = editing_model();
    for (key, mode) in [
        (egui::Key::Num2, cs_3d::SelectMode::Vertex),
        (egui::Key::Num3, cs_3d::SelectMode::Edge),
        (egui::Key::Num4, cs_3d::SelectMode::Face),
        (egui::Key::Num1, cs_3d::SelectMode::Object),
    ] {
        tap_key(&mut h, key);
        assert_eq!(h.app.doc.editor.model_edit().selection.mode, mode);
    }
}

#[test]
fn a_corner_handle_scales_both_axes_of_the_view() {
    let mut h = editing_model();
    // El cubo elegido en modo Objeto: su caja es ±16.
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    let from = front_screen(&h, [16.0, 16.0, 0.0]);
    let to = front_screen(&h, [48.0, 48.0, 0.0]);
    drag_with(&mut h, from, to, Default::default());
    let t = h.app.doc.editor.model().unwrap().objects[0].transform;
    assert_eq!((t.scale.x, t.scale.y, t.scale.z), (2000, 2000, 1000));
}

#[test]
fn s_follows_the_pointer_and_escape_cancels_without_an_undo_step() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    let start = front_screen(&h, [8.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(start)]);
    let steps = h.app.doc.editor.history.done_count();
    tap_key(&mut h, egui::Key::S);
    h.frame_with(vec![Event::PointerMoved(start + vec2(20.0, 0.0))]);
    h.frame();
    assert!(h.app.doc.editor.edit_model_shown().unwrap() != h.app.doc.editor.model().unwrap());
    tap_key(&mut h, egui::Key::Escape);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    assert_eq!(
        h.app.doc.editor.model().unwrap().objects[0]
            .transform
            .scale
            .x,
        1000
    );
}

#[test]
fn escape_without_a_gesture_clears_the_selection() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    tap_key(&mut h, egui::Key::Escape);
    assert!(h.app.doc.editor.model_edit().selection.is_empty());
}

#[test]
fn g_then_a_click_moves_and_arrows_nudge_by_the_grid() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    let start = front_screen(&h, [0.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(start)]);
    tap_key(&mut h, egui::Key::G);
    let to = front_screen(&h, [8.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.click(to, PointerButton::Primary);
    let x = |h: &LayoutHarness| {
        h.app.doc.editor.model().unwrap().objects[0]
            .transform
            .position
            .x
    };
    assert_eq!(x(&h), 8);
    let steps = h.app.doc.editor.history.done_count();
    tap_key(&mut h, egui::Key::ArrowRight);
    assert_eq!(x(&h), 12);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn a_click_on_an_off_grid_vertex_changes_nothing() {
    let mut h = editing_model();
    assert!(
        h.app
            .doc
            .editor
            .add_primitive(cs_3d::Primitive::Cylinder { sides: 8 })
    );
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    h.frame();
    // Un vértice del cilindro fuera de la cuadrícula (x = ±11), el más cercano a la vista.
    let points = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 1);
    let w = points
        .iter()
        .copied()
        .filter(|p| p[0].rem_euclid(4.0) != 0.0)
        .max_by(|a, b| a[2].total_cmp(&b[2]))
        .expect("an off-grid vertex");
    let model = h.app.doc.editor.model().cloned();
    let steps = h.app.doc.editor.history.done_count();
    h.click(front_screen(&h, w), PointerButton::Primary);
    let sel = &h.app.doc.editor.model_edit().selection;
    assert!(
        !sel.is_empty()
            && sel
                .items
                .iter()
                .all(|i| matches!(i, cs_3d::Item::Vertex(1, _))),
        "the cylinder vertex got selected: {sel:?}"
    );
    assert_eq!(h.app.doc.editor.model().cloned(), model, "nothing moved");
    assert_eq!(h.app.doc.editor.history.done_count(), steps, "no undo step");
}

#[test]
fn s_with_the_pointer_on_the_center_uses_a_fixed_reference() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    let c = front_screen(&h, [0.0, 0.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(c)]);
    tap_key(&mut h, egui::Key::S);
    let scale_x = |h: &LayoutHarness| {
        h.app.doc.editor.edit_model_shown().unwrap().objects[0]
            .transform
            .scale
            .x
    };
    h.frame_with(vec![Event::PointerMoved(c + vec2(24.0, 0.0))]);
    assert_eq!(scale_x(&h), 1000, "24 points away is the reference");
    h.frame_with(vec![Event::PointerMoved(c + vec2(48.0, 0.0))]);
    assert_eq!(scale_x(&h), 2000);
    tap_key(&mut h, egui::Key::Escape);
}

#[test]
fn edit_gestures_do_not_recompose_the_artboards() {
    let mut h = editing_model();
    h.frame();
    let composed = h.app.doc.canvas.composed_count();
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let at = quads[0].center();
    let button = |pos, button, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![button(at, PointerButton::Secondary, true)]);
    for dx in [5.0, 10.0, 20.0] {
        h.frame_with(vec![Event::PointerMoved(at + vec2(dx, 0.0))]);
    }
    h.frame_with(vec![button(
        at + vec2(20.0, 0.0),
        PointerButton::Secondary,
        false,
    )]);
    h.frame();
    // Una caja en un lugar vacío (cambia la selección, no el documento).
    let corner = quads[0].left_top() + vec2(4.0, 12.0);
    drag_with(
        &mut h,
        corner,
        corner + vec2(10.0, 10.0),
        Default::default(),
    );
    assert_eq!(
        h.app.doc.canvas.composed_count(),
        composed,
        "panning and a box only redraw the views"
    );
}

#[test]
fn shift_on_a_corner_handle_scales_proportionally() {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    let from = front_screen(&h, [16.0, 16.0, 0.0]);
    let to = front_screen(&h, [48.0, 32.0, 0.0]);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::SHIFT)]);
    drag_with(&mut h, from, to, egui::Modifiers::SHIFT);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    let t = h.app.doc.editor.model().unwrap().objects[0].transform;
    assert_eq!((t.scale.x, t.scale.y, t.scale.z), (2000, 2000, 1000));
    assert_eq!(
        h.app.doc.editor.model_edit().selection.items,
        [cs_3d::Item::Object(0)].into(),
        "Shift on the handle did not toggle the selection"
    );
}

#[test]
fn dragging_a_selected_extreme_vertex_moves_instead_of_scaling() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    h.app.doc.editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Vertex,
        items: [
            cs_3d::Item::Vertex(0, 0),
            cs_3d::Item::Vertex(0, 3),
            cs_3d::Item::Vertex(0, 7),
        ]
        .into(),
    });
    h.frame();
    let vertex = |h: &LayoutHarness, v: usize| {
        let m = h.app.doc.editor.model().unwrap();
        cs_3d::world_points(m, 0)[v]
    };
    let v0 = vertex(&h, 0);
    // El vértice 7 cae justo en el asa de la esquina de arriba a la derecha.
    let from = front_screen(&h, [16.0, 16.0, 16.0]);
    let to = front_screen(&h, [24.0, 16.0, 16.0]);
    drag_with(&mut h, from, to, Default::default());
    assert_eq!(vertex(&h, 7), [24.0, 16.0, 16.0]);
    assert_eq!(vertex(&h, 0)[0], v0[0] + 8.0, "everything selected moved");
    assert_eq!(h.app.doc.editor.model_edit().selection.items.len(), 3);
}

#[test]
fn a_middle_press_on_the_first_edit_frame_does_not_pick() {
    let mut h = with_a_model_at_800();
    let ship = h.app.doc.editor.active;
    h.app.doc.editor.select_model_object(None);
    h.app.doc.editor.exit_workspace();
    h.frame();
    assert_eq!(h.app.doc.canvas.model_edit_quadrants(), None, "the canvas");
    // Sobre el centro de la perspectiva (donde se ve el cubo) en cuanto se entre.
    let canvas = h.canvas.expect("the canvas");
    let at = canvas.lerp_inside(vec2(0.75, 0.25));
    h.frame_with(vec![Event::PointerMoved(at)]);
    // El workspace se abre sin un frame de por medio: todavía no hay cuadrantes del frame
    // anterior.
    h.app.doc.editor.enter_workspace(ship);
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Middle,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![button(true)]);
    h.frame_with(vec![button(false)]);
    h.frame();
    assert_eq!(
        h.app.doc.editor.selected_object(),
        None,
        "the cube got picked"
    );
}

#[test]
fn a_straight_selected_edge_drags_from_its_midpoint() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Edge);
    // La arista de arriba al frente (vértices 6 y 7): en Front su caja no tiene alto.
    h.app.doc.editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Edge,
        items: [cs_3d::Item::Edge(0, 6, 7)].into(),
    });
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    let from = front_screen(&h, [0.0, 16.0, 16.0]);
    let to = front_screen(&h, [0.0, 24.0, 16.0]);
    drag_with(&mut h, from, to, Default::default());
    let points = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    assert_eq!(points[6], [-16.0, 24.0, 16.0]);
    assert_eq!(points[7], [16.0, 24.0, 16.0]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn a_click_on_a_selected_edge_keeps_only_that_edge_and_a_drag_moves_all() {
    let mut h = editing_model();
    assert_eq!(
        h.app.doc.editor.model_edit().selection.mode,
        cs_3d::SelectMode::Edge,
        "a model opens in Edge mode"
    );
    let all = cs_3d::Selection {
        mode: cs_3d::SelectMode::Object,
        items: [cs_3d::Item::Object(0)].into(),
    }
    .converted(h.app.doc.editor.model().unwrap(), cs_3d::SelectMode::Edge);
    assert_eq!(all.items.len(), 12);
    h.app.doc.editor.set_model_selection(all);
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    // A un cuarto de la arista de arriba: lejos de las asas de la caja.
    let at = front_screen(&h, [8.0, 16.0, 16.0]);
    drag_with(&mut h, at, at, Default::default());
    assert_eq!(h.app.doc.editor.model_edit().selection.items.len(), 1);
    assert_eq!(h.app.doc.editor.history.done_count(), steps, "no move");
}

// --- Los gizmos en las vistas orto ---

/// El botón `label` del encabezado de ChimpCAD (si se ve).
fn header_button(h: &LayoutHarness, label: &str) -> Option<Rect> {
    let header = h.app.doc.canvas.cad_header()?;
    h.nodes
        .iter()
        .find(|(l, r)| l == label && header.contains_rect(*r))
        .map(|(_, r)| *r)
}

#[test]
fn the_gizmo_buttons_and_w_e_r_switch_the_gizmo_and_turn_it_off() {
    use crate::editor::Gizmo;
    let mut h = editing_model();
    let gizmo = |h: &LayoutHarness| h.app.doc.editor.model_edit().gizmo;
    assert_eq!(gizmo(&h), Gizmo::Move, "Move by default");
    let steps = h.app.doc.editor.history.done_count();
    let button = |h: &LayoutHarness, label| header_button(h, label).expect(label);
    let (mv, rotate, scale) = (
        button(&h, "Move"),
        button(&h, "Rotate"),
        button(&h, "Scale"),
    );
    // Un grupo a la izquierda de Model | Texture, en puntos enteros.
    let model = button(&h, "Model");
    assert!(mv.right() <= rotate.left() + 1.0 && rotate.right() <= scale.left() + 1.0);
    assert!(scale.right() < model.left(), "{scale:?} {model:?}");
    assert_eq!(mv.min, mv.min.round());
    let down = |h: &LayoutHarness, r: Rect| {
        h.drawn("button.down")
            .iter()
            .any(|d| r.expand(1.0).contains_rect(*d))
    };
    assert!(down(&h, mv) && !down(&h, rotate) && !down(&h, scale));
    // Con algo elegido, cambiar de gizmo vuelve a pintar las vistas.
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    let keys = h.app.doc.canvas.model_ortho_textures();
    h.click(rotate.center(), PointerButton::Primary);
    assert_eq!(gizmo(&h), Gizmo::Rotate);
    assert!(down(&h, rotate) && !down(&h, mv));
    let next = h.app.doc.canvas.model_ortho_textures();
    assert_ne!(next[0].unwrap().0, keys[0].unwrap().0, "Front redrawn");
    // Clic en el activo: ninguno.
    h.click(rotate.center(), PointerButton::Primary);
    assert_eq!(gizmo(&h), Gizmo::None);
    assert!(!down(&h, rotate));
    h.click(scale.center(), PointerButton::Primary);
    assert_eq!(gizmo(&h), Gizmo::Scale);
    // W, E y R; la misma otra vez lo apaga.
    for (key, want) in [
        (egui::Key::W, Gizmo::Move),
        (egui::Key::W, Gizmo::None),
        (egui::Key::E, Gizmo::Rotate),
        (egui::Key::R, Gizmo::Scale),
        (egui::Key::R, Gizmo::None),
        (egui::Key::E, Gizmo::Rotate),
    ] {
        tap_key(&mut h, key);
        assert_eq!(gizmo(&h), want, "{key:?}");
    }
    assert_eq!(
        h.app.doc.editor.history.done_count(),
        steps,
        "no undo steps"
    );
    // En modo Texture no hay gizmos.
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    assert!(header_button(&h, "Move").is_none());
    tap_key(&mut h, egui::Key::W);
    assert_eq!(gizmo(&h), Gizmo::Rotate, "W in Texture mode");
}

/// El cubo elegido en modo Objeto con el gizmo `g`, y dónde cae su centro (el origen) en Front.
fn cube_with_gizmo(g: crate::editor::Gizmo) -> (LayoutHarness, Pos2) {
    let mut h = editing_model();
    h.app.doc.editor.select_model_object(Some(0));
    h.app.doc.editor.set_gizmo(g);
    h.frame();
    let c = front_screen(&h, [0.0; 3]);
    (h, c)
}

fn cube_transform(h: &LayoutHarness) -> cs_core::Transform3D {
    h.app.doc.editor.model().unwrap().objects[0].transform
}

fn v3(x: i32, y: i32, z: i32) -> cs_core::Vec3i {
    cs_core::Vec3i { x, y, z }
}

#[test]
fn dragging_the_x_arrow_in_front_moves_only_x_in_one_undo_step() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let steps = h.app.doc.editor.history.done_count();
    let zoom = h.app.doc.editor.model_edit().zoom;
    // Sobre la flecha X; el puntero también sube, pero solo cuenta X.
    let from = c + vec2(20.0, 0.0);
    drag_with(
        &mut h,
        from,
        from + vec2(8.0 * zoom, -5.0 * zoom),
        Default::default(),
    );
    assert_eq!(cube_transform(&h).position, v3(8, 0, 0));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(cube_transform(&h).position, v3(0, 0, 0));
}

#[test]
fn dragging_the_ring_in_front_rotates_in_15_degree_steps_and_ctrl_in_1() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Rotate);
    let r = crate::editor::GIZMO_PX;
    let at = |deg: f32| {
        let a = deg.to_radians();
        c + vec2(a.cos(), -a.sin()) * r
    };
    let steps = h.app.doc.editor.history.done_count();
    // 50° al revés del reloj en Front: alrededor de +Z, en pasos de 15°.
    drag_with(&mut h, at(0.0), at(50.0), Default::default());
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, 450));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    h.frame();
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::COMMAND)]);
    drag_with(&mut h, at(0.0), at(50.0), egui::Modifiers::COMMAND);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, 500));
    // Con el reloj, negativo.
    h.app.doc.editor.undo();
    h.frame();
    drag_with(&mut h, at(0.0), at(-35.0), Default::default());
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, -300));
}

#[test]
fn the_scale_cubes_scale_one_axis_or_all_three() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Scale);
    let r = crate::editor::GIZMO_PX;
    let steps = h.app.doc.editor.history.done_count();
    // El cubito X (en la punta del eje) al doble de lejos: solo X.
    drag_with(
        &mut h,
        c + vec2(r, 0.0),
        c + vec2(2.0 * r, 10.0),
        Default::default(),
    );
    assert_eq!(cube_transform(&h).scale, v3(2000, 1000, 1000));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    h.frame();
    // El del centro: parejo.
    drag_with(&mut h, c, c + vec2(r, 0.0), Default::default());
    assert_eq!(cube_transform(&h).scale, v3(2000, 2000, 2000));
    assert_eq!(cube_transform(&h).position, v3(0, 0, 0));
}

#[test]
fn a_click_on_the_gizmo_without_dragging_changes_nothing() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let model = h.app.doc.editor.model().cloned();
    let selection = h.app.doc.editor.model_edit().selection.clone();
    let steps = h.app.doc.editor.history.done_count();
    let from = c + vec2(20.0, 0.0);
    drag_with(&mut h, from, from + vec2(2.0, 0.0), Default::default());
    assert_eq!(h.app.doc.editor.model().cloned(), model);
    assert_eq!(h.app.doc.editor.model_edit().selection, selection);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

#[test]
fn escape_or_a_right_click_cancels_a_gizmo_drag() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let steps = h.app.doc.editor.history.done_count();
    let zoom = h.app.doc.editor.model_edit().zoom;
    let from = c + vec2(20.0, 0.0);
    let to = from + vec2(8.0 * zoom, 0.0);
    let button = |button, pos, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    for cancel in ["escape", "right"] {
        h.frame_with(vec![Event::PointerMoved(from)]);
        h.frame_with(vec![button(PointerButton::Primary, from, true)]);
        h.frame_with(vec![Event::PointerMoved(to)]);
        assert_ne!(
            h.app.doc.editor.edit_model_shown(),
            h.app.doc.editor.model(),
            "a preview"
        );
        match cancel {
            "escape" => tap_key(&mut h, egui::Key::Escape),
            _ => {
                h.frame_with(vec![button(PointerButton::Secondary, to, true)]);
                h.frame_with(vec![button(PointerButton::Secondary, to, false)]);
            }
        }
        h.frame_with(vec![button(PointerButton::Primary, to, false)]);
        h.frame();
        assert!(h.app.doc.editor.model_edit().op.is_none(), "{cancel}");
        assert_eq!(cube_transform(&h).position, v3(0, 0, 0), "{cancel}");
        assert_eq!(h.app.doc.editor.history.done_count(), steps, "{cancel}");
    }
}

#[test]
fn a_right_click_that_cancels_a_gizmo_drag_does_not_pan() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let center = h.app.doc.editor.model_edit().center;
    let from = c + vec2(20.0, 0.0);
    let button = |button, pos, pressed| Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(PointerButton::Primary, from, true)]);
    let to = from + vec2(30.0, 0.0);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![button(PointerButton::Secondary, to, true)]);
    // Con el izquierdo todavía apretado, el puntero sigue.
    h.frame_with(vec![Event::PointerMoved(to + vec2(40.0, 20.0))]);
    let end = to + vec2(60.0, 30.0);
    h.frame_with(vec![button(PointerButton::Secondary, end, false)]);
    h.frame_with(vec![Event::PointerMoved(end)]);
    h.frame_with(vec![button(PointerButton::Primary, end, false)]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(
        h.app.doc.editor.model_edit().center,
        center,
        "the views panned"
    );
    assert_eq!(cube_transform(&h).position, v3(0, 0, 0));
}

#[test]
fn w_e_r_do_nothing_during_a_gizmo_drag() {
    use crate::editor::Gizmo;
    let (mut h, c) = cube_with_gizmo(Gizmo::Move);
    let zoom = h.app.doc.editor.model_edit().zoom;
    let from = c + vec2(20.0, 0.0);
    let to = from + vec2(8.0 * zoom, 0.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    for key in [egui::Key::W, egui::Key::E, egui::Key::R] {
        tap_key(&mut h, key);
        assert_eq!(h.app.doc.editor.model_edit().gizmo, Gizmo::Move, "{key:?}");
    }
    h.frame_with(vec![button(to, false)]);
    h.frame();
    assert_eq!(cube_transform(&h).position, v3(8, 0, 0), "the drag went on");
}

#[test]
fn g_and_s_do_nothing_during_a_gizmo_drag() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let zoom = h.app.doc.editor.model_edit().zoom;
    let steps = h.app.doc.editor.history.done_count();
    let from = c + vec2(20.0, 0.0);
    let to = from + vec2(8.0 * zoom, 0.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    for key in [egui::Key::G, egui::Key::S] {
        tap_key(&mut h, key);
        assert!(
            matches!(
                h.app.doc.editor.model_edit().op,
                Some(crate::editor::EditOp::Gizmo { .. })
            ),
            "{key:?}: {:?}",
            h.app.doc.editor.model_edit().op
        );
    }
    h.frame_with(vec![button(to, false)]);
    h.frame();
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(cube_transform(&h).position, v3(8, 0, 0), "the drag went on");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn w_e_r_do_nothing_in_texture_mode_without_a_texture() {
    use crate::editor::Gizmo;
    let mut h = editing_model();
    assert!(h.app.doc.editor.set_texture(None));
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    // Sin textura sigue activo el modelo (los atajos de pixeles están bloqueados).
    assert!(h.app.doc.editor.pixel_actions_blocked());
    let before = h.app.doc.editor.model_edit().gizmo;
    for key in [egui::Key::W, egui::Key::E, egui::Key::R] {
        tap_key(&mut h, key);
        assert_eq!(h.app.doc.editor.model_edit().gizmo, before, "{key:?}");
    }
    assert_eq!(before, Gizmo::Move);
}

#[test]
fn a_negative_scale_from_the_gizmo_survives_the_scale_field() {
    let (mut h, c) = cube_with_gizmo(crate::editor::Gizmo::Scale);
    assert!(
        h.texts.iter().any(|(t, _)| t == "Scale"),
        "the object tab is shown"
    );
    let r = crate::editor::GIZMO_PX;
    let steps = h.app.doc.editor.history.done_count();
    // El cubito X más allá del centro: escala negativa en X.
    drag_with(
        &mut h,
        c + vec2(r, 0.0),
        c - vec2(r, 0.0),
        Default::default(),
    );
    h.frame();
    h.frame();
    let scale = cube_transform(&h).scale;
    assert!(scale.x < 0, "{scale:?}");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

/// El centro del aro `axis` del gizmo de la perspectiva (el promedio de sus puntos).
fn persp_ring_center(h: &LayoutHarness, axis: usize) -> cs_3d::P3 {
    let cs_3d::HandleGeom::Ring(pts) = persp_handle(h, cs_3d::GizmoPart::Ring(axis)) else {
        panic!("a ring");
    };
    let n = pts.len() as f32;
    std::array::from_fn(|i| pts.iter().map(|p| p[i]).sum::<f32>() / n)
}

#[test]
fn the_rotate_gizmo_stays_put_while_its_ring_is_dragged() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    // Tres esquinas de adelante (sin la de abajo a la izquierda): al girar, el centro de su
    // caja se corre.
    let pts = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    let items = (0..pts.len())
        .filter(|&i| pts[i][2] > 0.0 && !(pts[i][0] < 0.0 && pts[i][1] < 0.0))
        .map(|i| cs_3d::Item::Vertex(0, i as u32))
        .collect();
    h.app.doc.editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Vertex,
        items,
    });
    h.app.doc.editor.set_gizmo(crate::editor::Gizmo::Rotate);
    h.frame();
    let center = [0.0, 0.0, 16.0];
    assert_eq!(persp_ring_center(&h, 2).map(f32::round), center);
    let c = front_screen(&h, center);
    let r = crate::editor::GIZMO_PX;
    let at = |deg: f32| {
        let a = deg.to_radians();
        c + vec2(a.cos(), -a.sin()) * r
    };
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(at(0.0))]);
    h.frame_with(vec![button(at(0.0), true)]);
    h.frame_with(vec![Event::PointerMoved(at(20.0))]);
    h.frame_with(vec![Event::PointerMoved(at(45.0))]);
    h.frame();
    let shown = h.app.doc.editor.edit_model_shown().unwrap();
    let sel = &h.app.doc.editor.model_edit().selection;
    let moved = cs_3d::center(cs_3d::bounds(shown, sel).unwrap());
    assert!(
        (moved[1] - center[1]).abs() > 1.0,
        "the box drifts: {moved:?}"
    );
    // El gizmo (orto y perspectiva) sigue en el centro del giro.
    let ring = persp_ring_center(&h, 2);
    for i in 0..3 {
        assert!((ring[i] - center[i]).abs() < 0.01, "{ring:?}");
    }
    let ortho = h
        .app
        .doc
        .canvas
        .ortho_gizmo_center(&h.app.doc.editor)
        .expect("a gizmo");
    assert_eq!(ortho, center);
    h.frame_with(vec![button(at(45.0), false)]);
    h.frame();
}

#[test]
fn a_gizmo_handle_under_the_navigation_gizmo_does_not_light_up() {
    let mut h = editing_model();
    let balls = h.app.doc.canvas.nav_gizmo_balls(&h.app.doc.editor);
    let ball = |axis, negative| {
        balls
            .iter()
            .find(|b| (b.0, b.1) == (axis, negative))
            .map(|b| b.2)
            .unwrap()
    };
    // Cerca del centro del gizmo de navegación (no en una bolita): ahí se orbita.
    let p = ball(0, false).lerp(ball(0, true), 0.5) + vec2(6.0, 6.0);
    // El cubo corrido para que el cuadrito del centro de su gizmo Move caiga en `p`.
    let pm = h.app.doc.canvas.persp_map(&h.app.doc.editor).unwrap();
    let (o, d) = pm.ray((p.x, p.y));
    let cam = camera(&h);
    let t = cam.distance as f32;
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let w: [f32; 3] = std::array::from_fn(|i| o[i] + d[i] / len * t);
    let mut model = h.app.doc.editor.model().unwrap().clone();
    model.objects[0].transform.position = v3(
        w[0].round() as i32,
        w[1].round() as i32,
        w[2].round() as i32,
    );
    let group = egui::Id::new("move the cube");
    h.app.doc.editor.set_model_grouped(model, group);
    h.app.doc.editor.end_group();
    h.app.doc.editor.select_model_object(Some(0));
    h.app.doc.editor.set_gizmo(crate::editor::Gizmo::Move);
    h.frame();
    assert_eq!(
        persp_hit(&h, p),
        Some(cs_3d::GizmoPart::Plane),
        "the gizmo is under the navigation gizmo"
    );
    h.frame_with(vec![Event::PointerMoved(p)]);
    h.frame();
    assert_eq!(h.app.doc.canvas.persp_hover_gizmo(), None);
}

#[test]
fn the_gizmo_wins_over_the_box_select() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let all = cs_3d::Selection {
        mode: cs_3d::SelectMode::Object,
        items: [cs_3d::Item::Object(0)].into(),
    }
    .converted(h.app.doc.editor.model().unwrap(), cs_3d::SelectMode::Vertex);
    assert_eq!(all.items.len(), 8);
    h.app.doc.editor.set_model_selection(all.clone());
    h.frame();
    let before = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    let zoom = h.app.doc.editor.model_edit().zoom;
    // Sin el gizmo, ahí (lejos de los vértices) empezaría una caja de selección.
    let from = front_screen(&h, [0.0; 3]) + vec2(20.0, 0.0);
    drag_with(
        &mut h,
        from,
        from + vec2(8.0 * zoom, 0.0),
        Default::default(),
    );
    let after = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    for (a, b) in before.iter().zip(&after) {
        assert_eq!([a[0] + 8.0, a[1], a[2]], *b);
    }
    assert_eq!(h.app.doc.editor.model_edit().selection, all);
}

// --- Los gizmos en la perspectiva ---

/// Dónde se ve el punto `p` del mundo en la perspectiva del workspace.
fn persp_screen(h: &LayoutHarness, p: cs_3d::P3) -> Pos2 {
    let pm = h
        .app
        .doc
        .canvas
        .persp_map(&h.app.doc.editor)
        .expect("the perspective");
    let (x, y) = pm.to_screen(p).expect("in front of the camera");
    Pos2::new(x, y)
}

/// El asa `part` del gizmo como se ve en la perspectiva.
fn persp_handle(h: &LayoutHarness, part: cs_3d::GizmoPart) -> cs_3d::HandleGeom {
    h.app
        .doc
        .canvas
        .persp_gizmo_shape(&h.app.doc.editor)
        .expect("a gizmo in the perspective")
        .handles
        .into_iter()
        .find(|x| x.part == part)
        .expect("the handle")
        .geom
}

/// El asa del gizmo de la perspectiva bajo `at`.
fn persp_hit(h: &LayoutHarness, at: Pos2) -> Option<cs_3d::GizmoPart> {
    let pm = h.app.doc.canvas.persp_map(&h.app.doc.editor)?;
    h.app.doc.canvas.persp_gizmo_shape(&h.app.doc.editor)?.hit(
        |p| pm.to_screen(p).unwrap_or((1e9, 1e9)),
        (at.x, at.y),
        cs_3d::PICK_RADIUS,
    )
}

#[test]
fn the_gizmo_and_the_navigation_gizmo_are_drawn_over_the_persp_render() {
    let (h, _) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let overlay = h
        .app
        .doc
        .canvas
        .persp_overlay_texture()
        .expect("the overlay is rasterized");
    let render = h.app.doc.canvas.model_persp_texture().unwrap();
    let at = |tex| h.textures.iter().position(|(t, _)| *t == tex);
    let (i, j) = (
        at(render).unwrap(),
        at(overlay).expect("the overlay is drawn"),
    );
    assert!(j > i, "over the render");
    // Un pixel por punto sobre todo el cuadrante, en puntos enteros.
    let r = h.textures[j].1;
    assert_eq!(r.min, quads[1].min.round());
    assert_eq!(r.max, quads[1].max.round());
    // Las tres flechas, cada una hacia su eje (en pantalla).
    let c = persp_screen(&h, [0.0; 3]);
    for axis in 0..3 {
        let cs_3d::HandleGeom::Arrow { tip, .. } = persp_handle(&h, cs_3d::GizmoPart::Axis(axis))
        else {
            panic!("an arrow");
        };
        let mut unit = [0.0; 3];
        unit[axis] = 16.0;
        let (t, u) = (persp_screen(&h, tip) - c, persp_screen(&h, unit) - c);
        assert!(t.normalized().dot(u.normalized()) > 0.99, "axis {axis}");
    }
    // Sin nada elegido no hay gizmo (el de navegación sigue).
    let mut h = editing_model();
    h.frame();
    assert!(
        h.app
            .doc
            .canvas
            .persp_gizmo_shape(&h.app.doc.editor)
            .is_none()
    );
    assert_eq!(h.app.doc.canvas.nav_gizmo_balls(&h.app.doc.editor).len(), 6);
}

#[test]
fn dragging_the_x_arrow_in_persp_moves_only_x_in_one_undo_step() {
    let (mut h, _) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let steps = h.app.doc.editor.history.done_count();
    let cs_3d::HandleGeom::Arrow { from, tip, .. } = persp_handle(&h, cs_3d::GizmoPart::Axis(0))
    else {
        panic!("an arrow");
    };
    let (a, b) = (persp_screen(&h, from), persp_screen(&h, tip));
    let start = a.lerp(b, 0.6);
    assert_eq!(persp_hit(&h, start), Some(cs_3d::GizmoPart::Axis(0)));
    let dir = (b - a).normalized();
    // A lo largo de la flecha y también de costado: solo cuenta X.
    drag_with(
        &mut h,
        start,
        start + dir * 40.0 + dir.rot90() * 10.0,
        Default::default(),
    );
    let p = cube_transform(&h).position;
    assert!(p.x > 0 && p.x % 4 == 0, "on the grid: {p:?}");
    assert_eq!((p.y, p.z), (0, 0));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Hacia el otro lado, negativo.
    h.app.doc.editor.undo();
    h.frame();
    assert_eq!(cube_transform(&h).position, v3(0, 0, 0));
    drag_with(&mut h, start, start - dir * 40.0, Default::default());
    let p = cube_transform(&h).position;
    assert!(p.x < 0 && (p.y, p.z) == (0, 0), "{p:?}");
}

#[test]
fn dragging_the_view_square_in_persp_moves_in_the_view_plane() {
    let (mut h, _) = cube_with_gizmo(crate::editor::Gizmo::Move);
    let center = persp_screen(&h, [0.0; 3]);
    assert_eq!(persp_hit(&h, center), Some(cs_3d::GizmoPart::Plane));
    let cam = camera(&h);
    drag_with(&mut h, center, center + vec2(30.0, 0.0), Default::default());
    // A la derecha de la cámara (yaw 30°: hacia +X y un poco hacia -Z), en la cuadrícula.
    let p = cube_transform(&h).position;
    assert!(p.x > 0 && p.z < 0 && p.y == 0, "{p:?}");
    assert!(p.x % 4 == 0 && p.z % 4 == 0, "{p:?}");
    assert_eq!(camera(&h), cam, "the camera stays");
}

#[test]
fn dragging_a_ring_in_persp_rotates_in_15_degree_steps_and_ctrl_in_1() {
    let (mut h, _) = cube_with_gizmo(crate::editor::Gizmo::Rotate);
    let cs_3d::HandleGeom::Ring(pts) = persp_handle(&h, cs_3d::GizmoPart::Ring(2)) else {
        panic!("a ring");
    };
    // Un punto del aro de Z que no comparte con los otros aros.
    let from = pts
        .iter()
        .map(|&p| persp_screen(&h, p))
        .find(|&s| {
            persp_hit(&h, s) == Some(cs_3d::GizmoPart::Ring(2))
                && [
                    vec2(4.0, 0.0),
                    vec2(0.0, 4.0),
                    vec2(-4.0, 0.0),
                    vec2(0.0, -4.0),
                ]
                .iter()
                .all(|d| persp_hit(&h, s + *d) != Some(cs_3d::GizmoPart::Ring(0)))
        })
        .expect("a free spot on the ring");
    let c = persp_screen(&h, [0.0; 3]);
    // Girado `deg` al revés del reloj en pantalla alrededor del centro: la cámara mira desde
    // +Z, así que es el sentido positivo de Z.
    let turned = |deg: f32| {
        let (s, k) = deg.to_radians().sin_cos();
        let d = from - c;
        c + vec2(d.x * k + d.y * s, -d.x * s + d.y * k)
    };
    let steps = h.app.doc.editor.history.done_count();
    drag_with(&mut h, from, turned(50.0), Default::default());
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, 450));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    h.frame();
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::COMMAND)]);
    drag_with(&mut h, from, turned(50.0), egui::Modifiers::COMMAND);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, 500));
    h.app.doc.editor.undo();
    h.frame();
    drag_with(&mut h, from, turned(-35.0), Default::default());
    assert_eq!(cube_transform(&h).rotation, v3(0, 0, -300));
}

#[test]
fn the_scale_cubes_in_persp_scale_one_axis_or_all_three() {
    let (mut h, _) = cube_with_gizmo(crate::editor::Gizmo::Scale);
    let c = persp_screen(&h, [0.0; 3]);
    assert_eq!(persp_hit(&h, c), Some(cs_3d::GizmoPart::ScaleUniform));
    let steps = h.app.doc.editor.history.done_count();
    drag_with(
        &mut h,
        c,
        c + vec2(crate::editor::GIZMO_PX, 0.0),
        Default::default(),
    );
    assert_eq!(cube_transform(&h).scale, v3(2000, 2000, 2000));
    assert_eq!(cube_transform(&h).position, v3(0, 0, 0));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    h.frame();
    // El cubito de Y, al doble de lejos del centro: solo Y.
    let cs_3d::HandleGeom::Stick { tip, .. } = persp_handle(&h, cs_3d::GizmoPart::ScaleAxis(1))
    else {
        panic!("a stick");
    };
    let t = persp_screen(&h, tip);
    drag_with(&mut h, t, c + (t - c) * 2.0, Default::default());
    assert_eq!(cube_transform(&h).scale, v3(1000, 2000, 1000));
}

#[test]
fn the_persp_gizmo_wins_over_picking_and_moving_the_camera() {
    let mut h = editing_model();
    h.app.doc.editor.set_select_mode(cs_3d::SelectMode::Vertex);
    let all = cs_3d::Selection {
        mode: cs_3d::SelectMode::Object,
        items: [cs_3d::Item::Object(0)].into(),
    }
    .converted(h.app.doc.editor.model().unwrap(), cs_3d::SelectMode::Vertex);
    h.app.doc.editor.set_model_selection(all.clone());
    h.frame();
    let cam = camera(&h);
    let before = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    // La flecha X cae sobre el cubo: sin el gizmo, el clic elegiría el objeto.
    let cs_3d::HandleGeom::Arrow { from, tip, .. } = persp_handle(&h, cs_3d::GizmoPart::Axis(0))
    else {
        panic!("an arrow");
    };
    let (a, b) = (persp_screen(&h, from), persp_screen(&h, tip));
    let start = a.lerp(b, 0.3);
    let pm = h.app.doc.canvas.persp_map(&h.app.doc.editor).unwrap();
    let local = (
        ((start.x - pm.min.0) / pm.scale.0) as u32,
        ((start.y - pm.min.1) / pm.scale.1) as u32,
    );
    let doc = &h.app.doc.editor.document;
    let artboard = doc.artboard(h.app.doc.editor.model_id()).unwrap();
    assert_eq!(
        cs_render::model_pick(doc, artboard, local.0 as i32, local.1 as i32),
        Some(0)
    );
    // Con Shift (que movería el objetivo de la cámara) también gana el gizmo.
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::SHIFT)]);
    drag_with(&mut h, start, start + (b - a) * 0.5, egui::Modifiers::SHIFT);
    h.frame_with(vec![Event::ModifiersChanged(egui::Modifiers::NONE)]);
    let after = cs_3d::world_points(h.app.doc.editor.model().unwrap(), 0);
    let dx = after[0][0] - before[0][0];
    assert!(dx > 0.0, "moved along X");
    for (p, q) in before.iter().zip(&after) {
        assert_eq!([p[0] + dx, p[1], p[2]], *q);
    }
    assert_eq!(h.app.doc.editor.model_edit().selection, all);
    assert_eq!(camera(&h), cam);
}

/// Corre el cubo del modelo lejos del origen y aleja la cámara hacia otro lado: devuelve el
/// centro del modelo (en 1/16, redondeado), donde tiene que quedar el objetivo al recentrar.
fn move_the_model_off_center(h: &mut LayoutHarness) -> cs_core::Vec3i {
    let editor = &mut h.app.doc.editor;
    let mut obj = editor.edited_model().unwrap().objects[0].clone();
    obj.transform.position = cs_core::Vec3i {
        x: 40,
        y: 24,
        z: -16,
    };
    // Puede que ya estuviera ahí (de una vuelta anterior de la prueba).
    editor.set_object(0, obj);
    let mut cam = editor.edited_model().unwrap().camera;
    cam.target = cs_core::Vec3i {
        x: -300,
        y: 120,
        z: 90,
    };
    cam.distance = 40 * cs_core::FIXED;
    editor.set_camera(cam);
    h.frame();
    let c = cs_3d::center(cs_3d::model_bounds(h.app.doc.editor.edited_model().unwrap()).unwrap());
    cs_core::Vec3i {
        x: c[0].round() as i32,
        y: c[1].round() as i32,
        z: c[2].round() as i32,
    }
}

/// Que todo el modelo se vea en el render de la perspectiva, con aire, y que lo llene bastante
/// (no quedó chiquito a lo lejos).
fn assert_the_model_fills_the_persp(h: &LayoutHarness) {
    let editor = &h.app.doc.editor;
    let model = editor.edited_model().unwrap();
    let artboard = editor.document.artboard(editor.model_id()).unwrap();
    let size = (artboard.sprite.width, artboard.sprite.height);
    let projector = cs_3d::Projector::new(&model.camera, size);
    let (lo, hi) = cs_3d::model_bounds(model).unwrap();
    let (mut min, mut max) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for i in 0..8 {
        let p = [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ];
        let (x, y) = projector.project(p).expect("in front of the camera");
        min = (min.0.min(x), min.1.min(y));
        max = (max.0.max(x), max.1.max(y));
    }
    let (w, h) = (size.0 as f32, size.1 as f32);
    assert!(
        min.0 >= 1.0 && min.1 >= 1.0 && max.0 <= w - 1.0 && max.1 <= h - 1.0,
        "the model fits with air: {min:?} {max:?} in {size:?}"
    );
    let span = (max.0 - min.0).max(max.1 - min.1);
    assert!(span >= 0.4 * w.min(h), "the model fills the view: {span}");
}

#[test]
fn home_frames_the_whole_model_in_one_undo_step() {
    let mut h = editing_model();
    for projection in [
        cs_core::Projection::Orthographic,
        cs_core::Projection::Perspective { fov: 600 },
    ] {
        let center = move_the_model_off_center(&mut h);
        let mut cam = camera(&h);
        cam.projection = projection;
        cam.yaw = 300;
        cam.pitch = 200;
        h.app.doc.editor.set_camera(cam);
        h.frame();
        let before = camera(&h);
        let steps = h.app.doc.editor.history.done_count();
        h.frame_with(key_tap(egui::Key::Home));
        h.frame();
        let after = camera(&h);
        assert_eq!(after.target, center, "{projection:?}");
        assert_eq!(
            (after.yaw, after.pitch, after.projection),
            (before.yaw, before.pitch, before.projection)
        );
        assert_the_model_fills_the_persp(&h);
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
        h.app.doc.editor.undo();
        assert_eq!(camera(&h), before);
        h.frame();
    }
}

#[test]
fn home_over_the_texture_mode_render_frames_the_model() {
    let mut h = with_a_model_in_texture_mode();
    let camera = |h: &LayoutHarness| h.app.doc.editor.edited_model().unwrap().camera;
    let center = move_the_model_off_center(&mut h);
    let (top, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    // Sobre la textura de abajo, Home no toca la cámara.
    let before = camera(&h);
    h.frame_with(vec![Event::PointerMoved(bottom.center())]);
    h.frame_with(key_tap(egui::Key::Home));
    h.frame();
    assert_eq!(camera(&h), before, "not over the texture");
    h.frame_with(vec![Event::PointerMoved(top.center())]);
    let steps = h.app.doc.editor.history.done_count();
    h.frame_with(key_tap(egui::Key::Home));
    h.frame();
    assert_eq!(camera(&h).target, center);
    assert_the_model_fills_the_persp(&h);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn the_camera_presets_and_the_navigation_balls_recenter_on_the_model() {
    let mut h = model_panel_tall();
    let center = move_the_model_off_center(&mut h);
    for label in ["Iso", "Top", "Side", "Front"] {
        let before = camera(&h);
        h.click(node(&h, label).center(), PointerButton::Primary);
        let cam = camera(&h);
        assert_eq!(cam.target, center, "{label}");
        assert_eq!(cam.distance, before.distance, "{label}");
        h.app.doc.editor.undo();
        h.frame();
    }
    let mut h = editing_model();
    let center = move_the_model_off_center(&mut h);
    let before = camera(&h);
    let balls = h.app.doc.canvas.nav_gizmo_balls(&h.app.doc.editor);
    let (_, _, at) = balls
        .into_iter()
        .find(|b| (b.0, b.1) == (1, false))
        .expect("the ball");
    h.click(at, PointerButton::Primary);
    let cam = camera(&h);
    assert_eq!((cam.yaw, cam.pitch), (0, 900));
    assert_eq!(cam.target, center, "the ball");
    assert_eq!(cam.distance, before.distance, "the ball");
}

#[test]
fn clicking_a_navigation_ball_looks_from_that_side_in_one_undo_step() {
    let mut h = editing_model();
    let start = camera(&h);
    for (axis, negative, want) in [
        (0, false, (900, 0)),
        (1, false, (0, 900)),
        (2, true, (-1800, 0)),
        (0, true, (-900, 0)),
    ] {
        let steps = h.app.doc.editor.history.done_count();
        let balls = h.app.doc.canvas.nav_gizmo_balls(&h.app.doc.editor);
        let (_, _, at) = balls
            .into_iter()
            .find(|b| (b.0, b.1) == (axis, negative))
            .expect("the ball");
        h.click(at, PointerButton::Primary);
        let cam = camera(&h);
        assert_eq!((cam.yaw, cam.pitch), want, "{axis} {negative}");
        assert_eq!(
            (cam.distance, cam.target, cam.projection),
            (start.distance, start.target, start.projection)
        );
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    }
    h.app.doc.editor.undo();
    assert_eq!((camera(&h).yaw, camera(&h).pitch), (-1800, 0));
}

#[test]
fn dragging_the_navigation_gizmo_orbits_in_one_undo_step() {
    let mut h = editing_model();
    let start = camera(&h);
    let balls = h.app.doc.canvas.nav_gizmo_balls(&h.app.doc.editor);
    let ball = |axis, negative| {
        balls
            .iter()
            .find(|b| (b.0, b.1) == (axis, negative))
            .map(|b| b.2)
            .unwrap()
    };
    let center = ball(0, false).lerp(ball(0, true), 0.5);
    let steps = h.app.doc.editor.history.done_count();
    // Cerca del centro (no en una bolita): 20 puntos a la derecha = 10° de yaw.
    let from = center + vec2(6.0, 6.0);
    drag_with(&mut h, from, from + vec2(20.0, 0.0), Default::default());
    assert_eq!(
        (camera(&h).yaw, camera(&h).pitch),
        (start.yaw + 100, start.pitch)
    );
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Desde una bolita también orbita (y no salta a su lado al soltar).
    let x = ball(0, false);
    drag_with(&mut h, x, x + vec2(0.0, 10.0), Default::default());
    assert_eq!(
        (camera(&h).yaw, camera(&h).pitch),
        (start.yaw + 100, start.pitch + 50)
    );
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 2);
    // Un clic al lado (fuera del gizmo) no cambia la cámara.
    let before = camera(&h);
    h.click(center + vec2(-40.0, 30.0), PointerButton::Primary);
    assert_eq!(camera(&h), before);
}

/// Un modelo "Ship" con su pestaña de vista abierta, en el documento de la prueba.
fn with_model_tab() -> LayoutHarness {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    assert!(h.app.doc.editor.new_model("Ship", (32, 32)));
    let id = h.app.doc.editor.workspace().expect("the workspace");
    h.app.tabs.open_view(&mut h.app.doc, id);
    h.frame();
    h
}

#[test]
fn pixel_shortcuts_do_not_reach_the_hidden_sprite_in_the_cad_workspace() {
    let mut h = with_model_tab();
    let steps = h.app.doc.editor.history.done_count();
    press_ctrl(&mut h, egui::Key::A);
    h.frame();
    assert!(h.app.doc.editor.selection.is_none(), "no marquee");
    h.frame_with(key_tap(egui::Key::Delete));
    h.frame_with(key_tap(egui::Key::F));
    press(&mut h, egui::Key::H, egui::Modifiers::SHIFT);
    press_ctrl(&mut h, egui::Key::I);
    h.frame();
    let e = &h.app.doc.editor;
    assert!(e.model_edit_on());
    assert_eq!(e.history.done_count(), steps, "no invisible undo steps");
}

#[test]
fn pixel_shortcuts_act_on_the_texture_in_texture_mode() {
    let mut h = with_model_tab();
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    let texture = h.app.doc.editor.workspace_model().unwrap().texture.unwrap();
    assert_eq!(h.app.doc.editor.active, texture);
    press_ctrl(&mut h, egui::Key::A);
    h.frame();
    let e = &h.app.doc.editor;
    assert!(e.model_edit_on());
    assert!(e.selection.is_some(), "Select All on the texture");
    assert_eq!(e.active, texture);
}

#[test]
fn pixel_shortcuts_stay_blocked_in_texture_mode_without_a_texture() {
    let mut h = with_model_tab();
    assert!(h.app.doc.editor.set_texture(None));
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    press_ctrl(&mut h, egui::Key::A);
    h.frame();
    h.frame_with(key_tap(egui::Key::Delete));
    h.frame_with(key_tap(egui::Key::F));
    let e = &h.app.doc.editor;
    assert!(e.is_model(), "the model is still active");
    assert!(e.selection.is_none(), "no marquee on the hidden sprite");
    assert_eq!(e.history.done_count(), steps, "no invisible undo steps");
}

#[test]
fn a_model_view_tab_sits_right_after_its_document_and_switches_and_closes() {
    let mut h = with_model_tab();
    let doc = h.button(&h.app.doc.location.name.clone());
    let view = h.button("Ship");
    assert!(view.left() > doc.right(), "{doc:?} {view:?}");
    assert!(view.left() - doc.right() < 40.0, "right after it");
    assert_eq!(h.app.tabs.bar_active(&h.app.doc), Some(1), "its view shows");
    // El ícono antes del nombre (el modelo dibuja ▣ con formas, sin texto).
    assert!(!h.drawn("tab.active").is_empty());
    // La pestaña del documento regresa al lienzo.
    h.click(doc.center(), PointerButton::Primary);
    h.frame();
    assert_eq!(h.app.doc.editor.workspace(), None);
    assert_eq!(h.app.tabs.bar_active(&h.app.doc), Some(0));
    // La de la vista vuelve al workspace.
    let view = h.button("Ship");
    h.click(view.center(), PointerButton::Primary);
    h.frame();
    assert!(h.app.doc.editor.workspace().is_some());
    assert_eq!(h.app.tabs.bar_active(&h.app.doc), Some(1));
    // La × la cierra sin preguntar.
    let close = h.button("Close Ship");
    h.click(close.center(), PointerButton::Primary);
    h.frame();
    assert_eq!(h.app.pending, None);
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 1);
    assert_eq!(h.app.doc.editor.workspace(), None);
}

#[test]
fn deleting_the_model_closes_its_view_tab() {
    let mut h = with_model_tab();
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2);
    while h
        .app
        .doc
        .editor
        .document
        .artboards
        .iter()
        .any(|a| a.model.is_some())
    {
        h.app.doc.editor.undo();
    }
    h.frame();
    h.frame();
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 1);
    assert!(!h.texts.iter().any(|t| t.0 == "Ship"));
}

/// Un documento con el símbolo "Coin", el modelo "Ship" y la fuente "Pixie", sin ninguna vista
/// abierta, y el panel Library abierto.
fn with_library_items() -> LayoutHarness {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    add_symbol(&mut h, "Coin");
    let editor = &mut h.app.doc.editor;
    assert!(editor.new_model("Ship", (32, 32)));
    editor.exit_workspace();
    h.frame();
    h.click(h.button("Library").center(), PointerButton::Primary);
    h.frame();
    h
}

/// Vuelve a abrir el panel Library: desde la pestaña de un modelo (sin dock), de vuelta al
/// lienzo, donde sigue abierto; desde la de una fuente (que abre su panel encima), su botón.
fn reopen_library(h: &mut LayoutHarness) {
    if h.app.doc.editor.model_edit_on() {
        let doc = h.button(&h.app.doc.location.name.clone());
        h.click(doc.center(), PointerButton::Primary);
        h.frame();
        assert_eq!(
            crate::dock::open_panel(&h.ctx),
            Some(crate::dock::DockPanel::Library)
        );
        return;
    }
    h.click(h.button("Library").center(), PointerButton::Primary);
    h.frame();
}

/// La fila `name` del panel Library (la de más abajo con ese nombre: la pestaña está arriba).
fn library_row(h: &LayoutHarness, name: &str) -> Rect {
    h.nodes
        .iter()
        .filter(|(l, _)| l == name)
        .map(|(_, r)| *r)
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .unwrap_or_else(|| panic!("no {name:?} row in {:?}", h.nodes))
}

#[test]
fn the_library_has_symbols_models_and_fonts_sections_with_their_counts() {
    let mut h = with_library_items();
    let text = |h: &LayoutHarness, t: &str| {
        h.text_rects
            .iter()
            .find(|(x, _, _)| x == t)
            .map(|(_, r, _)| *r)
    };
    let symbols = text(&h, "Symbols").expect("Symbols title");
    let models = text(&h, "Models").expect("Models title");
    let fonts = text(&h, "Fonts").expect("Fonts title");
    assert!(symbols.top() < models.top() && models.top() < fonts.top());
    // La cuenta junto al título: 1 símbolo, 1 modelo, 0 fuentes.
    let count_of = |h: &LayoutHarness, title: Rect| {
        h.text_rects
            .iter()
            .find(|(_, r, _)| {
                (r.center().y - title.center().y).abs() < 4.0 && r.left() > title.right()
            })
            .map(|(t, _, _)| t.clone())
    };
    assert_eq!(count_of(&h, symbols).as_deref(), Some("1"));
    assert_eq!(count_of(&h, models).as_deref(), Some("1"));
    assert_eq!(count_of(&h, fonts).as_deref(), Some("0"));
    // Las secciones con algo no piden nada; la vacía dice cómo crear una.
    let has = |t: &str| h.texts.iter().any(|(x, _)| x == t);
    assert!(!has("F8 makes a symbol") && !has("File → New 3D Model…"));
    assert!(has("File → New Font…"), "{:?}", h.texts);
    assert!(library_row(&h, "Coin").top() < library_row(&h, "Ship").top());
    // Un documento vacío muestra las tres ayudas.
    let mut empty = LayoutHarness::new(pixel_ui(), true);
    empty.click(empty.button("Library").center(), PointerButton::Primary);
    empty.frame();
    for t in [
        "F8 makes a symbol",
        "File → New 3D Model…",
        "File → New Font…",
    ] {
        assert!(empty.texts.iter().any(|(x, _)| x == t), "{t}");
    }
    h.frame();
}

#[test]
fn double_clicking_a_model_in_the_library_opens_its_tab() {
    let mut h = with_library_items();
    let ship = h.app.doc.editor.document.models().next().unwrap().id;
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 1);
    let row = library_row(&h, "Ship");
    h.double_click(row.center());
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2, "a view tab");
    assert_eq!(h.app.doc.editor.workspace(), Some(ship));
    reopen_library(&mut h);
    // Un símbolo sigue entrando en su modo.
    let coin = library_row(&h, "Coin");
    h.double_click(coin.center());
    assert!(h.app.doc.editor.in_symbol_mode());
}

#[test]
fn double_clicking_a_font_in_the_library_opens_its_tab_and_panel() {
    let mut h = with_library_items();
    assert!(
        h.app
            .doc
            .editor
            .new_font("Pixie", (8, 8), cs_core::FontMode::OneBit)
    );
    h.app.doc.editor.exit_workspace();
    h.frame();
    h.frame();
    let font = h.app.doc.editor.document.fonts().next().unwrap().id;
    let row = library_row(&h, "Pixie");
    h.double_click(row.center());
    assert_eq!(h.app.doc.editor.workspace(), Some(font));
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2);
    assert!(h.app.doc.editor.is_font());
}

#[test]
fn the_library_lists_the_fonts_of_a_family_under_its_name() {
    let mut h = with_library_items();
    let e = &mut h.app.doc.editor;
    assert!(e.new_font("Mine", (8, 8), cs_core::FontMode::OneBit));
    let style = cs_core::FontStyle {
        name: "Bold".into(),
        weight: 700,
        ..Default::default()
    };
    let recipe = cs_core::VariantRecipe::new(cs_core::VariantKind::Bold);
    assert!(e.add_variant(&recipe, style));
    assert!(e.new_font("Pixie", (8, 8), cs_core::FontMode::OneBit));
    e.exit_workspace();
    h.frame();
    h.frame();
    let text = |t: &str| {
        h.text_rects
            .iter()
            .filter(|(x, _, _)| x == t)
            .map(|(_, r, _)| *r)
            .max_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("no {t:?} in {:?}", h.texts))
    };
    let (family, regular, bold, pixie) =
        (text("Mine"), text("Regular"), text("Bold"), text("Pixie"));
    assert!(family.top() < regular.top() && regular.top() < bold.top());
    assert!(
        bold.top() < pixie.top(),
        "the families and loose fonts by name"
    );
    assert!(regular.left() > pixie.left(), "the styles go indented");
    // Las filas siguen siendo las fuentes (con su nombre completo): se abren con doble clic.
    let row = library_row(&h, "Mine Bold");
    h.double_click(row.center());
    assert_eq!(h.app.doc.editor.artboard().name, "Mine Bold");
    // La cuenta del título: las tres fuentes.
    reopen_library(&mut h);
    assert!(h.text_rects.iter().any(|(t, _, _)| t == "3"));
}

#[test]
fn renaming_a_model_in_the_library_renames_its_tab() {
    let mut h = with_library_items();
    let row = library_row(&h, "Ship");
    h.double_click(row.center());
    h.frame();
    reopen_library(&mut h);
    let row = library_row(&h, "Ship");
    h.click(row.center(), PointerButton::Secondary);
    h.frame();
    h.click(h.button("Rename").center(), PointerButton::Primary);
    h.frame();
    h.frame_with(vec![Event::Text("x".into())]);
    h.frame_with(key_tap(egui::Key::Enter));
    h.frame();
    h.frame();
    let ship = h.app.doc.editor.document.models().next().unwrap();
    assert_eq!(ship.name, "Shipx");
    assert!(
        h.buttons.iter().any(|(l, _)| l == "Shipx"),
        "the tab: {:?}",
        h.buttons
    );
}

#[test]
fn deleting_a_model_from_the_library_closes_its_tab() {
    let mut h = with_library_items();
    let row = library_row(&h, "Ship");
    h.double_click(row.center());
    h.frame();
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2);
    reopen_library(&mut h);
    let row = library_row(&h, "Ship");
    h.click(row.center(), PointerButton::Secondary);
    h.frame();
    h.click(h.button("Delete").center(), PointerButton::Primary);
    h.frame();
    h.frame();
    assert_eq!(h.app.doc.editor.document.models().count(), 0);
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 1);
    assert_eq!(h.app.doc.editor.workspace(), None);
}

#[test]
fn duplicating_a_model_makes_another_model_not_a_symbol() {
    let mut h = with_library_items();
    let row = library_row(&h, "Ship");
    h.click(row.center(), PointerButton::Secondary);
    h.frame();
    h.click(h.button("Duplicate").center(), PointerButton::Primary);
    h.frame();
    let doc = &h.app.doc.editor.document;
    assert_eq!(doc.models().count(), 2);
    assert_eq!(doc.library().count(), 1, "still only Coin");
}

#[test]
fn dragging_a_model_from_the_library_to_the_canvas_places_an_instance() {
    let mut h = with_library_items();
    let ship = h.app.doc.editor.document.models().next().unwrap().id;
    let row = library_row(&h, "Ship");
    let host = h
        .app
        .doc
        .editor
        .document
        .canvas_artboards()
        .next()
        .unwrap()
        .id;
    let texture = h.app.doc.canvas.artboard_texture(host).expect("rendered");
    let to = h
        .textures
        .iter()
        .find(|(t, _)| *t == texture)
        .map(|(_, r)| *r)
        .expect("the host on screen");
    // Un punto del artboard que no quede debajo del panel.
    let to = Pos2::new(
        (to.left() + to.right().min(row.left() - 4.0)) / 2.0,
        to.center().y,
    );
    drag(&mut h, row.center(), to);
    assert_eq!(h.app.doc.editor.document.instance_uses(ship), 1);
}

#[test]
fn place_on_canvas_of_a_model_puts_an_instance_on_the_canvas() {
    let mut h = with_library_items();
    let ship = h.app.doc.editor.document.models().next().unwrap().id;
    let row = library_row(&h, "Ship");
    h.click(row.center(), PointerButton::Secondary);
    h.frame();
    h.click(h.button("Place on Canvas").center(), PointerButton::Primary);
    h.frame();
    assert_eq!(h.app.doc.editor.document.instance_uses(ship), 1);
    assert!(
        h.app
            .doc
            .editor
            .document
            .artboard(ship)
            .unwrap()
            .model
            .is_some()
    );
    assert_eq!(h.app.doc.editor.document.models().count(), 1);
}

/// Los botones del dock dibujados en el último frame (los de los paneles y los de abajo).
fn dock_buttons(h: &LayoutHarness) -> Vec<String> {
    let labels: Vec<&str> = crate::dock::DockPanel::ALL
        .iter()
        .map(|p| p.label())
        .chain(["Timeline", "Preview"])
        .collect();
    h.buttons
        .iter()
        .filter(|(l, _)| labels.contains(&l.as_str()))
        .map(|(l, _)| l.clone())
        .collect()
}

/// En la pestaña del modelo se ve el panel 3D (el + de su tira dentro de su caja).
fn model_panel_shown(h: &LayoutHarness) -> bool {
    in_model_panel(h, "Add object").is_some()
}

#[test]
fn opening_a_model_view_shows_the_3d_panel_without_the_dock() {
    use crate::dock::DockPanel;
    let mut h = with_library_items();
    assert!(!dock_buttons(&h).is_empty(), "the canvas tab has the dock");
    let row = library_row(&h, "Ship");
    h.double_click(row.center());
    h.frame();
    assert!(h.app.doc.editor.model_edit_on());
    assert!(
        model_panel_shown(&h),
        "Library double-click shows the 3D panel"
    );
    assert_eq!(dock_buttons(&h), Vec::<String>::new(), "no dock column");
    // El panel del dock abierto en el lienzo sigue siendo el de antes (Library).
    assert_eq!(crate::dock::open_panel(&h.ctx), Some(DockPanel::Library));
    // De vuelta al lienzo, el dock con su panel; y otra vez a la pestaña del modelo, el panel 3D.
    let doc = h.button(&h.app.doc.location.name.clone());
    h.click(doc.center(), PointerButton::Primary);
    h.frame();
    assert!(!h.app.doc.editor.model_edit_on());
    assert!(dock_buttons(&h).contains(&"Library".to_string()));
    assert_eq!(crate::dock::open_panel(&h.ctx), Some(DockPanel::Library));
    let tab = h.button("Ship");
    h.click(tab.center(), PointerButton::Primary);
    h.frame();
    assert!(model_panel_shown(&h), "clicking its tab shows the 3D panel");
    assert_eq!(dock_buttons(&h), Vec::<String>::new());
}

#[test]
fn switching_tabs_keeps_the_canvas_tab_dock_panel() {
    use crate::dock::DockPanel;
    let mut h = with_model_tab();
    let doc_tab = |h: &LayoutHarness| h.button(&h.app.doc.location.name.clone());
    for open in [None, Some(DockPanel::Lumen), Some(DockPanel::Model)] {
        h.click(doc_tab(&h).center(), PointerButton::Primary);
        h.frame();
        crate::dock::set_open(&h.ctx, open);
        h.frame();
        h.click(h.button("Ship").center(), PointerButton::Primary);
        h.frame();
        assert!(model_panel_shown(&h));
        assert_eq!(crate::dock::open_panel(&h.ctx), open, "on the model tab");
        h.click(doc_tab(&h).center(), PointerButton::Primary);
        h.frame();
        assert_eq!(crate::dock::open_panel(&h.ctx), open, "back on the canvas");
        assert!(dock_buttons(&h).contains(&"Timeline".to_string()));
    }
}

#[test]
fn the_preview_window_hides_on_the_model_tab_and_comes_back_on_the_canvas() {
    let mut h = with_model_tab();
    let doc_tab = |h: &LayoutHarness| h.button(&h.app.doc.location.name.clone());
    let preview_shown = |h: &LayoutHarness| h.text_rects.iter().any(|(t, _, _)| t == "Preview");
    // En el lienzo, con su botón del dock.
    h.click(doc_tab(&h).center(), PointerButton::Primary);
    h.frame();
    h.click(h.button("Preview").center(), PointerButton::Primary);
    h.frame();
    assert!(h.app.view.preview);
    assert!(preview_shown(&h), "the window on the canvas");
    // En la pestaña del modelo no está su botón: tampoco la ventana (sin apagarla).
    h.click(h.button("Ship").center(), PointerButton::Primary);
    h.frame();
    h.frame();
    assert!(h.app.doc.editor.model_edit_on());
    assert!(!preview_shown(&h), "no window on the model tab");
    assert!(h.app.view.preview, "still on for the canvas");
    // De vuelta al lienzo, otra vez.
    h.click(doc_tab(&h).center(), PointerButton::Primary);
    h.frame();
    h.frame();
    assert!(preview_shown(&h), "back on the canvas");
    // Apagada, no vuelve.
    h.click(h.button("Preview").center(), PointerButton::Primary);
    h.frame();
    assert!(!h.app.view.preview);
    h.click(h.button("Ship").center(), PointerButton::Primary);
    h.frame();
    h.click(doc_tab(&h).center(), PointerButton::Primary);
    h.frame();
    h.frame();
    assert!(!preview_shown(&h));
}

#[test]
fn the_3d_panel_of_the_model_tab_sits_between_the_workspace_and_the_colors() {
    let h = with_model_tab();
    assert_eq!(dock_buttons(&h), Vec::<String>::new());
    let side = h.panel("model_panel");
    let colors = h.panel("palette");
    assert_eq!(side.right(), colors.left(), "{side:?} {colors:?}");
    let canvas = h.canvas.expect("the workspace area");
    // La caja del panel: 3 px de aire hasta el lienzo y hasta la columna de colores.
    let panel = model_flyout(&h);
    assert_eq!(
        panel.left() - canvas.right(),
        3.0 + 3.0,
        "{panel:?} {canvas:?}"
    );
    assert_eq!(colors.left() - panel.right(), 3.0);
    assert_eq!(
        panel.width(),
        crate::model_panel::WIDTH + 2.0 * crate::dock::FLYOUT_MARGIN as f32
    );
    // Sin pestaña: la caja no pasa de su borde derecho.
    let boxes = union(&h.drawn("menu.side")).expect("the panel box");
    assert_eq!(boxes, panel, "only the box, without the tab");
    // A lo alto, como la columna del dock: de las reglas al borde de abajo, con 5 de aire.
    assert_eq!(boxes.top(), side.top() + 2.0 + 5.0);
    assert_eq!(boxes.bottom(), side.bottom() - 2.0 - 5.0);
}

#[test]
fn creating_a_model_or_font_opens_its_view_tab_and_panel() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let ctx = h.ctx.clone();
    assert!(h.app.create_model("Hut", (32, 32)));
    h.frame();
    let model = h.app.doc.editor.workspace().expect("in the workspace");
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2, "a view tab");
    assert!(h.app.doc.views.contains(&model));
    assert!(h.app.doc.editor.is_model());
    assert!(
        h.app
            .create_font(&ctx, "Pix", (8, 8), cs_core::FontMode::OneBit)
    );
    h.frame();
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 3);
    assert!(h.app.doc.editor.is_font());
}

#[test]
fn double_clicking_a_model_instance_opens_its_view_tab() {
    let mut h = with_library_items();
    let ship = h.app.doc.editor.document.models().next().unwrap().id;
    assert!(h.app.doc.editor.drop_symbol(ship, (8, 8)).is_ok());
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 1);
    // Lo que hace el doble clic con Move sobre la instancia: queda elegida y se entra a ella.
    assert!(!h.app.doc.editor.selected_objects().is_empty());
    assert!(h.app.doc.editor.enter_instance());
    h.frame();
    assert_eq!(h.app.doc.editor.workspace(), Some(ship));
    assert_eq!(h.app.tabs.bar(&h.app.doc).len(), 2, "a view tab");
    assert!(h.app.doc.views.contains(&ship));
}

#[test]
fn baking_from_the_model_tab_returns_to_the_document_tab_with_the_baked_artboard() {
    let mut h = with_library_items();
    let ship = h.app.doc.editor.document.models().next().unwrap().id;
    h.double_click(library_row(&h, "Ship").center());
    assert_eq!(h.app.doc.editor.workspace(), Some(ship));
    let count = h.app.doc.editor.document.artboards.len();
    // Lo que hace Export del diálogo Bake… (sus botones pixel no están en el arnés).
    let dialog = crate::bake_dialog::BakeDialog::new(&h.app.doc.editor);
    h.app.finish_bake(dialog.model(), dialog.chosen());
    h.frame();
    let e = &h.app.doc.editor;
    assert_eq!(e.document.artboards.len(), count + 1, "the baked artboard");
    assert_eq!(e.workspace(), None, "back on the canvas");
    let baked = e.document.artboards.last().unwrap();
    assert!(baked.name.ends_with("bake"));
    assert_eq!(e.active, baked.id);
    assert!(baked.on_canvas());
}
/// El modelo de 32×32 en su pestaña, en modo Texture (la textura activa).
fn with_a_model_in_texture_mode() -> LayoutHarness {
    let mut h = with_a_model_at_800();
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    h.frame();
    h
}

/// Dónde se dibuja la textura del modelo (abajo, en modo Texture).
fn texture_pixels(h: &LayoutHarness) -> Rect {
    let id = h.app.doc.editor.workspace_model().unwrap().texture.unwrap();
    let tex = h
        .app
        .doc
        .canvas
        .artboard_texture(id)
        .expect("the texture is composed");
    h.textures
        .iter()
        .find(|(t, _)| *t == tex)
        .map(|(_, r)| *r)
        .expect("the texture is drawn")
}

#[test]
fn the_model_and_texture_buttons_switch_the_mode_without_undo() {
    use crate::editor::CadMode;
    let mut h = with_a_model_at_800();
    let canvas = h.canvas.expect("the workspace area");
    let steps = h.app.doc.editor.history.done_count();
    let (model, texture) = (node(&h, "Model"), node(&h, "Texture"));
    // Arriba a la derecha del área, uno junto al otro, en puntos enteros.
    assert!(
        texture.left() >= model.right() - 1.0,
        "{model:?} {texture:?}"
    );
    // Anim, el último del grupo, va contra la derecha.
    let anim = node(&h, "Anim");
    assert!(anim.left() >= texture.right() - 1.0, "{texture:?} {anim:?}");
    assert!(
        canvas.right() - anim.right() < 8.0,
        "{anim:?} in {canvas:?}"
    );
    assert!(model.top() - canvas.top() < 8.0 && model.top() >= canvas.top());
    assert_eq!(model.min, model.min.round());
    // El activo se ve presionado.
    let down = |h: &LayoutHarness, r: Rect| {
        h.drawn("button.down")
            .iter()
            .any(|d| r.expand(1.0).contains_rect(*d))
    };
    assert!(down(&h, model) && !down(&h, texture));
    h.click(texture.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.cad_mode(), CadMode::Texture);
    let (model, texture) = (node(&h, "Model"), node(&h, "Texture"));
    assert!(
        canvas.right() - node(&h, "Anim").right() < 8.0,
        "also in Texture"
    );
    assert!(down(&h, texture) && !down(&h, model));
    h.click(model.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.cad_mode(), CadMode::Model);
    assert_eq!(
        h.app.doc.editor.history.done_count(),
        steps,
        "no undo steps"
    );
}

#[test]
fn texture_mode_splits_the_area_top_and_bottom_left_of_the_panel() {
    let h = with_a_model_in_texture_mode();
    let canvas = h.canvas.expect("the workspace area");
    let (top, bottom) = h.app.doc.canvas.texture_panes().expect("the two panes");
    // Arriba, debajo del encabezado, de su línea y de la barrita del objeto; abajo, después de
    // la línea de 1 punto, la fila del pincel y la textura.
    let header = h.app.doc.canvas.cad_header().expect("the header");
    let bar = crate::panels::options_bar_height(h.app.pixel.skin());
    assert_eq!(top.min, pos2(canvas.left(), header.bottom() + 1.0 + bar));
    assert_eq!(top.height(), ((canvas.bottom() - top.top()) / 2.0).floor());
    assert_eq!(
        bottom.top(),
        top.bottom() + 1.0 + bar,
        "the line and the brush row"
    );
    assert_eq!(bottom.bottom(), canvas.bottom());
    assert_eq!(top.right(), canvas.right());
    assert_eq!(bottom.x_range(), top.x_range());
    assert!(top.contains_rect(persp_pixels(&h)), "the model on top");
    assert!(
        bottom.intersects(texture_pixels(&h)),
        "the texture below: {:?} in {bottom:?}",
        texture_pixels(&h)
    );
    assert!(
        h.app.doc.canvas.model_edit_quadrants().is_none(),
        "no ortho views"
    );
    // Encuadrada al entrar: a zoom entero y centrada en su panel.
    let r = texture_pixels(&h);
    let zoom = h.app.doc.editor.model_edit().texture_view.unwrap().zoom as f32;
    let side = h.app.doc.editor.sprite().width as f32;
    assert_eq!(r.width(), side * zoom, "{r:?}");
    let room = bottom.width().min(bottom.height()) * 0.85;
    assert!(side * (zoom + 1.0) > room, "the largest that fits");
    assert!(
        (r.center() - bottom.center()).length() <= 1.0,
        "{r:?} in {bottom:?}"
    );
    // A la izquierda del panel 3D (fijo).
    let panel = model_flyout(&h);
    let (top, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    assert!(top.right() <= panel.left() && bottom.right() <= panel.left());
    assert!(
        node(&h, "Texture").right() <= panel.left(),
        "the buttons too"
    );
}

#[test]
fn clicking_the_front_face_of_the_render_selects_it() {
    let mut h = with_a_model_in_texture_mode();
    let e = &mut h.app.doc.editor;
    let camera = cs_core::CameraPreset::Front.camera(4 * cs_core::FIXED, Default::default());
    assert!(e.set_camera(camera));
    h.frame();
    let r = persp_pixels(&h);
    let steps = h.app.doc.editor.history.done_count();
    h.click(r.center(), PointerButton::Primary);
    let e = &h.app.doc.editor;
    let faces = e.selected_faces();
    assert_eq!(faces.len(), 1, "{faces:?}");
    let mesh = &e.workspace_model().unwrap().objects[faces[0].0].mesh;
    assert!(
        mesh.faces[faces[0].1]
            .verts
            .iter()
            .all(|&v| mesh.vertices[v as usize].z > 0),
        "the front face"
    );
    assert_eq!(e.history.done_count(), steps, "picking is not undoable");
    assert_eq!(e.active, e.workspace_model().unwrap().texture.unwrap());
    // Shift + clic la quita y la vuelve a poner; un clic en vacío vacía.
    let shift_click = |h: &mut LayoutHarness, pos: Pos2| {
        let press = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::SHIFT,
        };
        h.frame_with(vec![Event::PointerMoved(pos)]);
        h.frame_with(vec![
            Event::ModifiersChanged(egui::Modifiers::SHIFT),
            press(true),
        ]);
        h.frame_with(vec![press(false)]);
        h.frame_with(vec![Event::ModifiersChanged(Default::default())]);
    };
    shift_click(&mut h, r.center());
    assert!(h.app.doc.editor.selected_faces().is_empty(), "toggled off");
    shift_click(&mut h, r.center());
    assert_eq!(h.app.doc.editor.selected_faces(), faces, "toggled on");
    let (top, _) = h.app.doc.canvas.texture_panes().unwrap();
    h.click(
        top.min + vec2(4.0, top.height() - 4.0),
        PointerButton::Primary,
    );
    assert!(h.app.doc.editor.selected_faces().is_empty(), "empty clears");
}

#[test]
fn the_pencil_paints_the_texture_below() {
    let mut h = with_a_model_in_texture_mode();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    let id = h.app.doc.editor.workspace_model().unwrap().texture.unwrap();
    let before = h
        .app
        .doc
        .editor
        .document
        .artboard(id)
        .unwrap()
        .sprite
        .clone();
    let steps = h.app.doc.editor.history.done_count();
    let r = texture_pixels(&h);
    let (_, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    let at = r.intersect(bottom).center();
    h.click(at, PointerButton::Primary);
    let e = &h.app.doc.editor;
    assert_eq!(e.history.done_count(), steps + 1);
    let after = &e.document.artboard(id).unwrap().sprite;
    assert_ne!(*after, before, "the texture changed");
    assert_eq!(e.cad_mode(), crate::editor::CadMode::Texture);
}

#[test]
fn the_model_on_top_follows_the_stroke_while_painting() {
    let mut h = with_a_model_in_texture_mode();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    let r = texture_pixels(&h);
    let (_, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    let r = r.intersect(bottom);
    let (from, to) = (r.lerp_inside(vec2(0.3, 0.5)), r.lerp_inside(vec2(0.7, 0.5)));
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![button(from, true)]);
    let key = |h: &LayoutHarness| h.app.doc.canvas.model_persp_key();
    let mut last = key(&h);
    // Cada paso del trazo (sin soltar) vuelve a renderizar el modelo de arriba.
    for t in [0.25, 0.5, 0.75, 1.0] {
        h.frame_with(vec![Event::PointerMoved(from.lerp(to, t))]);
        assert!(h.app.doc.editor.is_painting());
        let now = key(&h);
        assert_ne!(now, last, "re-rendered mid-stroke at {t}");
        last = now;
    }
    h.frame_with(vec![button(to, false)]);
    assert!(!h.app.doc.editor.is_painting());
}

#[test]
fn the_wheel_below_zooms_the_texture_not_the_ortho_views() {
    let mut h = with_a_model_in_texture_mode();
    let view = h.app.doc.editor.model_edit().texture_view.expect("fitted");
    assert!(view.zoom >= 1);
    let ortho = h.app.doc.editor.model_edit().zoom;
    let canvas_zoom = h.app.doc.canvas.zoom;
    let (_, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    let at = bottom.center();
    h.frame_with(vec![Event::PointerMoved(at)]);
    h.frame_with(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
    }]);
    h.frame();
    let next = h.app.doc.editor.model_edit().texture_view.unwrap();
    assert!(next.zoom > view.zoom, "{view:?} -> {next:?}");
    assert_eq!(h.app.doc.editor.model_edit().zoom, ortho);
    assert_eq!(h.app.doc.canvas.zoom, canvas_zoom, "the document's zoom");
}

#[test]
fn texture_mode_without_a_texture_offers_a_new_one() {
    let mut h = with_a_model_at_800();
    assert!(h.app.doc.editor.set_texture(None));
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    h.frame();
    assert!(
        h.texts.iter().any(|(t, _)| t == "No texture"),
        "{:?}",
        h.texts
    );
    h.click(h.button("New Texture").center(), PointerButton::Primary);
    h.frame();
    let e = &h.app.doc.editor;
    let texture = e.workspace_model().unwrap().texture.expect("a new texture");
    assert_eq!(e.active, texture, "and it is painted on");
}

const PERSP_QUAD: usize = 1;

/// Lo que cambia la pestaña del modelo en la ventana (con Pixel UI): en modo Model, sin la barra
/// de opciones de herramienta, el lienzo empieza donde empezaba ella; en modo Texture (se pinta
/// la textura) la barra vuelve, como fila arriba de la textura.
#[test]
fn the_model_tab_has_the_tool_options_bar_only_in_texture_mode() {
    let doc = LayoutHarness::new(pixel_ui(), true);
    let bar = |h: &LayoutHarness| h.texts.iter().any(|(t, _)| t == "Brush Size");
    assert!(bar(&doc), "the bar on the canvas tab: {:?}", doc.texts);
    let options = doc.panel("tool_options");
    let doc_canvas = doc.canvas.expect("the canvas");
    let mut h = with_a_model_at_800();
    assert!(!bar(&h), "no bar in the model tab");
    let canvas = h.canvas.expect("the workspace area");
    // (Abajo cambia aparte: el workspace no tiene línea de tiempo.)
    assert_eq!(canvas.top(), doc_canvas.top() - options.height());
    // A lo ancho, del mismo borde izquierdo hasta el panel 3D fijo (en lugar del dock).
    assert_eq!(canvas.left(), doc_canvas.left());
    assert_eq!(canvas.right(), model_flyout(&h).left() - 6.0);
    // En modo Texture, la barra (y el área, debajo de ella, como en el lienzo).
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    h.frame();
    assert!(bar(&h), "the bar in Texture: {:?}", h.texts);
    assert!(h.texts.iter().any(|(t, _)| t == "Ink"), "{:?}", h.texts);
    // El encabezado no se mueve; la fila va debajo del render y arriba de la textura.
    assert_eq!(h.canvas, Some(canvas));
    let header = h.app.doc.canvas.cad_header().expect("the header");
    let size = text_in(&h, canvas, "Brush Size").expect("in the area");
    let (top, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    assert!(
        size.top() > top.bottom() && size.bottom() < bottom.top(),
        "{size:?} between {top:?} and {bottom:?}"
    );
    // La barrita del objeto y la del pincel, del mismo alto que la barra del lienzo.
    assert_eq!(top.top() - header.bottom() - 1.0, options.height());
    assert_eq!(bottom.top() - top.bottom() - 1.0, options.height());
    // Y de vuelta en Model, otra vez sin ella.
    h.app.doc.editor.set_cad_mode(crate::editor::CadMode::Model);
    h.frame();
    h.frame();
    assert!(!bar(&h));
    assert_eq!(h.canvas, Some(canvas));
}

/// En la pestaña del modelo no hay dock: la fila del pincel no ofrece "Dafont" (abriría un panel
/// del dock que ahí no se ve). En el lienzo, sí.
#[test]
fn the_texture_mode_text_options_have_no_dafont_button() {
    let dafont = |h: &LayoutHarness| h.texts.iter().any(|(t, _)| t == "Dafont");
    let mut doc = LayoutHarness::new(pixel_ui(), true);
    doc.app.doc.editor.set_tool(ToolKind::Text);
    doc.frame();
    doc.frame();
    assert!(dafont(&doc), "on the canvas tab: {:?}", doc.texts);
    let mut h = with_a_model_in_texture_mode();
    h.app.doc.editor.set_tool(ToolKind::Text);
    h.frame();
    h.frame();
    assert!(
        h.texts.iter().any(|(t, _)| t == "Load font"),
        "the text options are there: {:?}",
        h.texts
    );
    assert!(!dafont(&h), "no Dafont in the model tab");
}

/// El encabezado de ChimpCAD (boceto a 1×): una franja de 24 filas arriba del área, de lado a
/// lado, con el logo a (3, 2) y los botones Model | Texture a 3 de arriba y 4 de la derecha; una
/// línea oscura y, debajo, las vistas.
#[test]
fn the_chimpcad_header_has_the_logo_and_the_mode_buttons_over_the_views() {
    let mut h = with_a_model_at_800();
    let canvas = h.canvas.expect("the workspace area");
    let header = h.app.doc.canvas.cad_header().expect("the header");
    assert_eq!(header.min, canvas.min);
    assert_eq!(header.right(), canvas.right());
    assert_eq!(header.height(), 24.0);
    let logo = h.drawn("logo.chimpcad");
    assert_eq!(
        logo,
        vec![Rect::from_min_size(
            header.min + vec2(3.0, 2.0),
            vec2(94.0, 20.0)
        )]
    );
    let (model, texture) = (node(&h, "Model"), node(&h, "Texture"));
    assert_eq!(model.top(), header.top() + 3.0, "{model:?} in {header:?}");
    assert_eq!(
        node(&h, "Anim").right(),
        header.right() - 4.0,
        "{texture:?}"
    );
    assert!(header.contains_rect(model) && header.contains_rect(texture));
    // La línea oscura de abajo, de lado a lado.
    let line = Rect::from_x_y_ranges(header.x_range(), header.bottom()..=header.bottom() + 1.0);
    let border = crate::pixel_ui::control_border(&h.ctx);
    let rects = h.colored_rects();
    assert!(rects.contains(&(border, line)), "the line under the header");
    // El bisel: arriba, el brillo blanco del boceto (el de los paneles).
    let top = Rect::from_x_y_ranges(header.x_range(), header.top()..=header.top() + 1.0);
    let white = h.app.pixel.skin().unwrap().colors().menu_highlight;
    assert!(rects.contains(&(white, top)), "the bevel on top");
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    let bar = crate::panels::options_bar_height(h.app.pixel.skin());
    assert_eq!(
        quads[0].top(),
        line.bottom() + bar,
        "the views start below the object bar"
    );
    // En modo Texture, igual: el render empieza debajo.
    h.click(texture.center(), PointerButton::Primary);
    h.frame();
    assert_eq!(h.app.doc.canvas.cad_header(), Some(header));
    assert_eq!(h.drawn("logo.chimpcad").len(), 1);
    let (top, _) = h.app.doc.canvas.texture_panes().unwrap();
    assert_eq!(top.top(), line.bottom() + bar, "under the object bar");
    assert!(!top.intersects(node(&h, "Model")));
    // En el lienzo no hay encabezado.
    let doc = LayoutHarness::new(pixel_ui(), true);
    assert_eq!(doc.app.doc.canvas.cad_header(), None);
    assert!(doc.drawn("logo.chimpcad").is_empty());
}

/// Sin Pixel UI, el encabezado dice "ChimpCAD" en texto.
#[test]
fn without_pixel_ui_the_chimpcad_header_says_its_name() {
    let mut h = LayoutHarness::new(plain(), true);
    assert!(h.app.doc.editor.new_model("Ship", (32, 32)));
    let id = h.app.doc.editor.workspace().expect("the workspace");
    h.app.tabs.open_view(&mut h.app.doc, id);
    h.frame();
    h.frame();
    let header = h.app.doc.canvas.cad_header().expect("the header");
    assert!(
        h.texts
            .iter()
            .any(|(t, p)| t == "ChimpCAD" && header.contains(*p)),
        "{:?}",
        h.texts
    );
    // El botón Model del encabezado (el panel 3D, al lado, tiene su pestaña Model).
    assert!(
        h.nodes
            .iter()
            .any(|(l, r)| l == "Model" && header.contains_rect(*r)),
        "{:?} in {header:?}",
        h.nodes
    );
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    assert!(quads[0].top() > header.bottom());
}

#[test]
fn the_mode_buttons_do_not_touch_the_face_selection_below() {
    let mut h = with_a_model_in_texture_mode();
    let e = &mut h.app.doc.editor;
    let sel = cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: [cs_3d::Item::Face(0, 0)].into(),
    };
    e.set_model_selection(sel.clone());
    h.frame();
    // El activo otra vez: no cambia nada, ni la selección de debajo.
    h.click(node(&h, "Texture").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model_edit().selection, sel);
    // A Model y de vuelta: la selección sigue.
    h.click(node(&h, "Model").center(), PointerButton::Primary);
    h.click(node(&h, "Texture").center(), PointerButton::Primary);
    let e = &h.app.doc.editor;
    assert_eq!(e.cad_mode(), crate::editor::CadMode::Texture);
    assert_eq!(e.model_edit().selection, sel);
}

#[test]
fn the_mode_buttons_do_not_touch_the_object_in_persp() {
    let mut h = with_a_model_at_800();
    let e = &mut h.app.doc.editor;
    e.select_model_object(Some(0));
    let sel = e.model_edit().selection.clone();
    h.frame();
    let model = node(&h, "Model");
    let quads = h.app.doc.canvas.model_edit_quadrants().unwrap();
    // En el encabezado, arriba de Persp: no sobre ella.
    assert!(
        quads.iter().all(|q| !q.intersects(model)),
        "not over the views"
    );
    assert!(model.bottom() <= quads[PERSP_QUAD].top());
    h.click(model.center(), PointerButton::Primary);
    let e = &h.app.doc.editor;
    assert_eq!(e.cad_mode(), crate::editor::CadMode::Model);
    assert_eq!(e.selected_object(), Some(0), "the panel's object");
    assert_eq!(e.model_edit().selection, sel);
}

/// En modo Texture con la cara de enfrente del cubo (la 0, en los pixeles 0..16 de la textura)
/// elegida.
fn with_the_front_face_chosen() -> LayoutHarness {
    let mut h = with_a_model_in_texture_mode();
    h.app.doc.editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: [cs_3d::Item::Face(0, 0)].into(),
    });
    h.frame();
    h.frame();
    h
}

/// El punto de pantalla del punto `(x, y)` de la textura (en pixeles; `x + 0.5` es el centro).
fn texel(h: &LayoutHarness, x: f32, y: f32) -> Pos2 {
    let r = texture_pixels(h);
    let side = h.app.doc.editor.sprite().width as f32;
    r.min + vec2(x, y) * (r.width() / side)
}

fn front_uvs(h: &LayoutHarness) -> Vec<(i32, i32)> {
    h.app.doc.editor.workspace_model().unwrap().objects[0]
        .mesh
        .faces[0]
        .uvs
        .clone()
}

/// La textura del modelo (su sprite).
fn texture_sprite(h: &LayoutHarness) -> cs_core::Sprite {
    let e = &h.app.doc.editor;
    let id = e.workspace_model().unwrap().texture.unwrap();
    e.document.artboard(id).unwrap().sprite.clone()
}

/// Como `drag`, con `modifiers` apretados todo el arrastre.
fn drag_holding(h: &mut LayoutHarness, from: Pos2, to: Pos2, modifiers: egui::Modifiers) {
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    };
    h.frame_with(vec![
        Event::ModifiersChanged(modifiers),
        Event::PointerMoved(from),
    ]);
    h.frame_with(vec![press(from, true)]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.frame_with(vec![press(to, false)]);
    h.frame_with(vec![Event::ModifiersChanged(Default::default())]);
}

#[test]
fn the_pencil_paints_only_inside_the_chosen_face() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    h.app.doc.editor.primary = red;
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    h.click(texel(&h, 40.5, 40.5), PointerButton::Primary);
    assert_ne!(texture_sprite(&h).pixel(layer, 0, 40, 40), red, "outside");
    h.click(texel(&h, 8.5, 8.5), PointerButton::Primary);
    assert_eq!(texture_sprite(&h).pixel(layer, 0, 8, 8), red, "inside");
}

#[test]
fn picking_a_face_on_top_focuses_its_uvs_below() {
    let mut h = with_a_model_in_texture_mode();
    let e = &mut h.app.doc.editor;
    let camera = cs_core::CameraPreset::Front.camera(4 * cs_core::FIXED, Default::default());
    assert!(e.set_camera(camera));
    h.frame();
    let fitted = h.app.doc.editor.model_edit().texture_view.unwrap();
    h.click(persp_pixels(&h).center(), PointerButton::Primary);
    h.frame();
    assert_eq!(h.app.doc.editor.selected_faces(), vec![(0, 0)]);
    let (_, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    let view = h.app.doc.editor.model_edit().texture_view.unwrap();
    let room = bottom.width().min(bottom.height()) * 0.85;
    let expected = ((room / 16.0).floor() as u32).clamp(1, 64);
    assert_eq!(view.zoom, expected, "the largest integer zoom that fits");
    assert!(view.zoom > fitted.zoom, "{fitted:?} -> {view:?}");
    let center = texel(&h, 8.0, 8.0);
    assert!(
        (center - bottom.center()).length() <= 1.0,
        "{center:?} in {bottom:?}"
    );
}

#[test]
fn dragging_inside_the_outline_with_move_moves_the_uvs_in_one_step() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let before = front_uvs(&h);
    let pixels = texture_sprite(&h);
    let steps = h.app.doc.editor.history.done_count();
    // Un clic no cambia nada.
    h.click(texel(&h, 8.5, 8.5), PointerButton::Primary);
    assert_eq!(front_uvs(&h), before);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    h.drag(texel(&h, 8.5, 8.5), texel(&h, 11.5, 10.5));
    let moved: Vec<_> = before.iter().map(|&(x, y)| (x + 3, y + 2)).collect();
    assert_eq!(front_uvs(&h), moved);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_eq!(texture_sprite(&h), pixels, "the pixels stay");
}

#[test]
fn alt_drag_with_the_pencil_moves_the_uvs_instead_of_painting() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    let before = front_uvs(&h);
    let pixels = texture_sprite(&h);
    let steps = h.app.doc.editor.history.done_count();
    let (from, to) = (texel(&h, 8.5, 8.5), texel(&h, 6.5, 12.5));
    drag_holding(&mut h, from, to, egui::Modifiers::ALT);
    let moved: Vec<_> = before.iter().map(|&(x, y)| (x - 2, y + 4)).collect();
    assert_eq!(front_uvs(&h), moved);
    assert_eq!(texture_sprite(&h), pixels, "nothing painted");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn an_alt_press_left_hanging_by_a_mode_change_does_not_click_later() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    let under = texture_sprite(&h).pixel(layer, 0, 8, 8);
    let primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    assert_ne!(primary, under);
    h.app.doc.editor.primary = primary;
    let at = texel(&h, 8.5, 8.5);
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::ALT,
    };
    // Alt + prensa sobre la cara (espera al umbral) y, sin soltar, a Model: ahí se suelta.
    h.frame_with(vec![
        Event::ModifiersChanged(egui::Modifiers::ALT),
        Event::PointerMoved(at),
    ]);
    h.frame_with(vec![press(true)]);
    h.app.doc.editor.set_cad_mode(crate::editor::CadMode::Model);
    h.frame_with(vec![press(false)]);
    h.frame_with(vec![Event::ModifiersChanged(Default::default())]);
    // De vuelta en Texture, la prensa de antes no le llega a la herramienta (el gotero).
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    h.frame();
    assert_eq!(
        h.app.doc.editor.primary, primary,
        "no late eyedropper click"
    );
}

/// El centro del asa `(u, v)` (−1, 0, 1 por eje) de la caja de UVs `(min, max)` en pantalla.
fn uv_handle(h: &LayoutHarness, (min, max): ((f32, f32), (f32, f32)), (u, v): (i8, i8)) -> Pos2 {
    let pick = |s: i8, lo: f32, hi: f32| match s {
        -1 => lo,
        1 => hi,
        _ => (lo + hi) / 2.0,
    };
    let at = texel(h, pick(u, min.0, max.0), pick(v, min.1, max.1));
    at + vec2(u as f32, v as f32) * crate::canvas::UV_HANDLE_OFFSET
}

fn front_bounds(h: &LayoutHarness) -> Option<((i32, i32), (i32, i32))> {
    cs_3d::uv_bounds(h.app.doc.editor.workspace_model().unwrap(), &[(0, 0)])
}

#[test]
fn dragging_a_corner_handle_scales_the_uvs_in_whole_pixels() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    assert_eq!(front_bounds(&h), Some(((0, 0), (16, 16))));
    let pixels = texture_sprite(&h);
    let steps = h.app.doc.editor.history.done_count();
    let from = uv_handle(&h, ((0.0, 0.0), (16.0, 16.0)), (1, 1));
    // Un poco más de 8 texels: el borde cae en el pixel entero.
    let to = from + (texel(&h, 8.3, 8.3) - texel(&h, 0.0, 0.0));
    h.drag(from, to);
    assert_eq!(front_bounds(&h), Some(((0, 0), (24, 24))));
    let before: Vec<_> = [(0, 0), (16, 0), (16, 16), (0, 16)].into();
    let mut uvs = front_uvs(&h);
    uvs.sort();
    let mut expected: Vec<_> = before
        .iter()
        .map(|&(x, y)| (x * 3 / 2, y * 3 / 2))
        .collect();
    expected.sort();
    assert_eq!(uvs, expected);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_eq!(texture_sprite(&h), pixels, "the pixels stay");
    h.app.doc.editor.undo();
    assert_eq!(front_bounds(&h), Some(((0, 0), (16, 16))));
}

#[test]
fn shift_on_a_side_handle_scales_proportionally() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let from = uv_handle(&h, ((0.0, 0.0), (16.0, 16.0)), (1, 0));
    let to = from - (texel(&h, 8.0, 0.0) - texel(&h, 0.0, 0.0));
    // Sin Shift solo cambia el ancho.
    h.drag(from, to);
    assert_eq!(front_bounds(&h), Some(((0, 0), (8, 16))));
    h.app.doc.editor.undo();
    // Con Shift, el alto también, por el mismo factor (respecto del medio).
    drag_holding(&mut h, from, to, egui::Modifiers::SHIFT);
    assert_eq!(front_bounds(&h), Some(((0, 4), (8, 12))));
}

#[test]
fn a_handle_never_scales_below_one_pixel() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let from = uv_handle(&h, ((0.0, 0.0), (16.0, 16.0)), (1, 1));
    // Más allá del lado opuesto: la caja no se voltea ni se aplasta.
    let to = from - (texel(&h, 30.0, 30.0) - texel(&h, 0.0, 0.0));
    h.drag(from, to);
    assert_eq!(front_bounds(&h), Some(((0, 0), (1, 1))));
}

#[test]
fn with_an_object_chosen_the_box_covers_all_its_faces() {
    let mut h = with_a_model_in_texture_mode();
    let e = &mut h.app.doc.editor;
    e.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: Default::default(),
    });
    e.select_model_object(Some(0));
    e.set_tool(ToolKind::Move);
    h.frame();
    h.frame();
    let faces = h.app.doc.editor.workspace_model().unwrap().objects[0]
        .mesh
        .faces
        .len();
    let all: Vec<_> = (0..faces).map(|f| (0, f)).collect();
    let mut boxed = h.app.doc.editor.uv_box_faces();
    boxed.sort();
    assert_eq!(boxed, all);
    let uvs_of = |h: &LayoutHarness| {
        h.app.doc.editor.workspace_model().unwrap().objects[0]
            .mesh
            .faces
            .iter()
            .map(|f| f.uvs.clone())
            .collect::<Vec<_>>()
    };
    let before = uvs_of(&h);
    let steps = h.app.doc.editor.history.done_count();
    // La caja de la fila (sin caras elegidas) mueve las UVs con Alt (sin Alt, Move es de los
    // pixeles).
    let (from, to) = (texel(&h, 8.5, 8.5), texel(&h, 11.5, 10.5));
    drag_holding(&mut h, from, to, egui::Modifiers::ALT);
    let moved: Vec<Vec<_>> = before
        .iter()
        .map(|f| f.iter().map(|&(x, y)| (x + 3, y + 2)).collect())
        .collect();
    assert_eq!(uvs_of(&h), moved);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn handles_drag_with_the_pencil_too() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    let pixels = texture_sprite(&h);
    let steps = h.app.doc.editor.history.done_count();
    let from = uv_handle(&h, ((0.0, 0.0), (16.0, 16.0)), (1, 1));
    let to = from + (texel(&h, 8.0, 4.0) - texel(&h, 0.0, 0.0));
    h.drag(from, to);
    assert_eq!(front_bounds(&h), Some(((0, 0), (24, 20))));
    assert_eq!(texture_sprite(&h), pixels, "nothing painted");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

/// En modo Texture sin caras elegidas y con la fila del cubo activa (la caja es la del objeto).
fn with_the_cube_row_active() -> LayoutHarness {
    let mut h = with_a_model_in_texture_mode();
    let e = &mut h.app.doc.editor;
    e.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: Default::default(),
    });
    e.select_model_object(Some(0));
    h.frame();
    h.frame();
    h
}

fn cube_uvs(h: &LayoutHarness) -> Vec<Vec<(i32, i32)>> {
    h.app.doc.editor.workspace_model().unwrap().objects[0]
        .mesh
        .faces
        .iter()
        .map(|f| f.uvs.clone())
        .collect()
}

/// Con la fila del cubo activa: el pixel `(8, 8)` de la textura pintado de rojo y una selección
/// de pixeles de 4..12 alrededor.
fn with_a_red_pixel_selected_in_the_cube_box() -> (LayoutHarness, PixelValue) {
    let mut h = with_the_cube_row_active();
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = red;
    h.click(texel(&h, 8.5, 8.5), PointerButton::Primary);
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    assert_eq!(texture_sprite(&h).pixel(layer, 0, 8, 8), red);
    let s = texture_sprite(&h);
    let mut mask = cs_core::Mask::new(s.width, s.height);
    for y in 4..12 {
        for x in 4..12 {
            mask.set(x, y, true);
        }
    }
    h.app.doc.editor.set_selection(Some(mask));
    h.app.doc.editor.set_tool(ToolKind::Move);
    h.frame();
    (h, red)
}

#[test]
fn with_the_row_box_a_move_drag_inside_moves_the_pixels_not_the_uvs() {
    let (mut h, red) = with_a_red_pixel_selected_in_the_cube_box();
    let uvs = cube_uvs(&h);
    h.drag(texel(&h, 8.5, 8.5), texel(&h, 11.5, 10.5));
    assert_eq!(cube_uvs(&h), uvs, "the UVs stay");
    h.app.doc.editor.commit_floating();
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    assert_eq!(
        texture_sprite(&h).pixel(layer, 0, 11, 10),
        red,
        "the pixels moved"
    );
}

#[test]
fn with_the_row_box_an_alt_drag_inside_moves_the_uvs_not_the_pixels() {
    let (mut h, _) = with_a_red_pixel_selected_in_the_cube_box();
    let uvs = cube_uvs(&h);
    let pixels = texture_sprite(&h);
    let (from, to) = (texel(&h, 8.5, 8.5), texel(&h, 11.5, 10.5));
    drag_holding(&mut h, from, to, egui::Modifiers::ALT);
    let moved: Vec<Vec<_>> = uvs
        .iter()
        .map(|f| f.iter().map(|&(x, y)| (x + 3, y + 2)).collect())
        .collect();
    assert_eq!(cube_uvs(&h), moved);
    assert!(h.app.doc.editor.floating.is_none());
    assert_eq!(texture_sprite(&h), pixels, "the pixels stay");
}

#[test]
fn with_the_pencil_a_click_on_a_handle_paints_and_does_not_scale() {
    let mut h = with_the_cube_row_active();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    let primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    h.app.doc.editor.primary = primary;
    let faces: Vec<_> = (0..cube_uvs(&h).len()).map(|f| (0, f)).collect();
    let b = cs_3d::uv_bounds(h.app.doc.editor.workspace_model().unwrap(), &faces).unwrap();
    let side = texture_sprite(&h).width as i32;
    assert!(
        b.1.0 < side - 1 && b.1.1 < side - 1,
        "room for the handle: {b:?}"
    );
    let f = |(x, y): (i32, i32)| (x as f32, y as f32);
    let at = uv_handle(&h, (f(b.0), f(b.1)), (1, 1));
    // El pixel bajo el asa.
    let r = texture_pixels(&h);
    let k = r.width() / side as f32;
    let (px, py) = (
        ((at.x - r.min.x) / k).floor() as i32,
        ((at.y - r.min.y) / k).floor() as i32,
    );
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    assert_ne!(texture_sprite(&h).pixel(layer, 0, px, py), primary);
    let uvs = cube_uvs(&h);
    h.click(at, PointerButton::Primary);
    assert_eq!(cube_uvs(&h), uvs, "no scale");
    assert_eq!(
        texture_sprite(&h).pixel(layer, 0, px, py),
        primary,
        "painted"
    );
}

#[test]
fn a_corner_vertex_is_a_free_point_with_a_bigger_grab_area() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let before = front_uvs(&h);
    let k = before.iter().position(|&p| p == (16, 16)).unwrap();
    let steps = h.app.doc.editor.history.done_count();
    // Agarrado a 3 puntos del vértice (el punto de 5×5 llega ahí) y llevado a cualquier pixel.
    h.drag(
        texel(&h, 16.0, 16.0) + vec2(3.0, 3.0),
        texel(&h, 19.0, 11.0) + vec2(3.0, 3.0),
    );
    let after = front_uvs(&h);
    for (i, (a, b)) in after.iter().zip(&before).enumerate() {
        if i == k {
            assert_eq!(*a, (19, 11));
        } else {
            assert_eq!(a, b, "vertex {i}");
        }
    }
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(front_uvs(&h), before);
}

#[test]
fn a_uv_vertex_moves_alone() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let before = front_uvs(&h);
    let k = before.iter().position(|&p| p == (16, 0)).unwrap();
    h.drag(texel(&h, 16.0, 0.0), texel(&h, 20.0, 3.0));
    let after = front_uvs(&h);
    for (i, (a, b)) in after.iter().zip(&before).enumerate() {
        if i == k {
            assert_eq!(*a, (20, 3));
        } else {
            assert_eq!(a, b, "vertex {i}");
        }
    }
}

#[test]
fn with_the_pencil_dragging_a_uv_vertex_moves_it_and_a_click_paints() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    let primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    h.app.doc.editor.primary = primary;
    let before = front_uvs(&h);
    let pixels = texture_sprite(&h);
    let k = before.iter().position(|&p| p == (16, 0)).unwrap();
    let steps = h.app.doc.editor.history.done_count();
    // Arrastrar el vértice lo mueve, sin pintar, en un paso.
    h.drag(texel(&h, 16.0, 0.0), texel(&h, 20.0, 3.0));
    let after = front_uvs(&h);
    for (i, (a, b)) in after.iter().zip(&before).enumerate() {
        if i == k {
            assert_eq!(*a, (20, 3));
        } else {
            assert_eq!(a, b, "vertex {i}");
        }
    }
    assert_eq!(texture_sprite(&h), pixels, "nothing painted");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Un clic sin arrastre en un vértice pinta el pixel bajo el puntero (el de la esquina de
    // arriba a la izquierda, dentro de la cara: solo ahí se pinta).
    assert_ne!(texture_sprite(&h).pixel(layer, 0, 0, 0), primary);
    h.click(texel(&h, 0.0, 0.0) + vec2(1.0, 1.0), PointerButton::Primary);
    assert_eq!(front_uvs(&h), after, "the UVs stay");
    assert_eq!(texture_sprite(&h).pixel(layer, 0, 0, 0), primary);
    // Y arrastrar dentro de la cara pinta, no mueve las UVs.
    h.drag(texel(&h, 8.5, 8.5), texel(&h, 10.5, 8.5));
    assert_eq!(front_uvs(&h), after);
    assert_eq!(texture_sprite(&h).pixel(layer, 0, 10, 8), primary);
}

/// Con el panel 3D a la vista (en la pestaña del modelo va fijo).
fn with_the_model_panel(h: LayoutHarness) -> LayoutHarness {
    assert!(h.app.doc.editor.model_edit_on());
    h
}

#[test]
fn the_uv_buttons_rotate_and_flip_the_chosen_faces() {
    let mut h = with_the_model_panel(with_a_model_in_texture_mode());
    for label in ["Rotate 90°", "Flip H", "Flip V"] {
        h.button(label);
        assert!(
            h.disabled.iter().any(|l| l == label),
            "{label} without faces"
        );
    }
    h.app.doc.editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: [cs_3d::Item::Face(0, 0)].into(),
    });
    h.frame();
    let before = front_uvs(&h);
    let m = h.app.doc.editor.workspace_model().unwrap();
    let rotated = cs_3d::rotate_uvs_90(m, &[(0, 0)]).objects[0].mesh.faces[0]
        .uvs
        .clone();
    let steps = h.app.doc.editor.history.done_count();
    h.click(h.button("Rotate 90°").center(), PointerButton::Primary);
    assert_eq!(front_uvs(&h), rotated);
    assert_ne!(front_uvs(&h), before);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    h.click(h.button("Flip H").center(), PointerButton::Primary);
    let flipped: Vec<_> = before.iter().map(|&(x, y)| (16 - x, y)).collect();
    assert_eq!(front_uvs(&h), flipped);
    h.app.doc.editor.undo();
    h.click(h.button("Flip V").center(), PointerButton::Primary);
    let flipped: Vec<_> = before.iter().map(|&(x, y)| (x, 16 - y)).collect();
    assert_eq!(front_uvs(&h), flipped);
}

#[test]
fn texture_size_128_from_the_panel_and_shrinking_asks() {
    let mut h = with_the_model_panel(with_a_model_in_texture_mode());
    h.click(node(&h, "64").center(), PointerButton::Primary);
    h.click(node(&h, "128").center(), PointerButton::Primary);
    let s = texture_sprite(&h);
    assert_eq!((s.width, s.height), (128, 128));
    // Achicar a 32 deja fuera caras del cubo: pide confirmación.
    h.click(node(&h, "128").center(), PointerButton::Primary);
    h.click(node(&h, "32").center(), PointerButton::Primary);
    let s = texture_sprite(&h);
    assert_eq!((s.width, s.height), (128, 128), "not yet");
    assert!(matches!(
        h.app.palette_confirm,
        Some(crate::palette_ui::Confirm::TextureSize(32))
    ));
}

#[test]
fn texture_mode_disables_the_select_modes() {
    let mut h = with_the_model_panel(with_a_model_at_800());
    let has = |h: &LayoutHarness, l: &str| h.nodes.iter().any(|(n, _)| n == l);
    assert!(has(&h, "Vertex") && has(&h, "Edge"));
    // La sección UV es de la pestaña de un objeto, y solo en Texture.
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    h.frame();
    assert!(!has(&h, "Rotate 90°"), "no UV section in Model");
    h.app.doc.editor.select_model_object(None);
    h.app
        .doc
        .editor
        .set_cad_mode(crate::editor::CadMode::Texture);
    h.frame();
    h.frame();
    // Se ven (la fila no se va) pero no se usan.
    for l in ["Object", "Vertex", "Edge", "Face"] {
        assert!(has(&h, l) && h.disabled.iter().any(|d| d == l), "{l}");
    }
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    h.frame();
    assert!(has(&h, "Rotate 90°"));
}

#[test]
fn in_texture_mode_the_object_tab_starts_with_the_uv_buttons() {
    let h = with_the_model_panel(with_a_model_in_texture_mode());
    assert!(h.app.doc.editor.selected_object().is_some());
    let (_, bottom) = model_halves(&h);
    let cube = node(&h, "Add object");
    // Como el boceto: los botones de las UVs primero, en una fila debajo del encabezado (sin
    // título), y después el outliner.
    let labels = ["Rotate 90°", "Flip H", "Flip V"];
    let buttons: Vec<Rect> = labels.iter().map(|l| h.button(l)).collect();
    for (b, l) in buttons.iter().zip(labels) {
        assert!(bottom.contains_rect(*b), "{l} {b:?} in {bottom:?}");
        assert_eq!(b.top(), buttons[0].top(), "{l} in one row");
    }
    // 9 filas de aire desde la línea grabada del encabezado (como el boceto).
    let line = model_split(&h) + 5.0 + crate::kit::tabs::HEIGHT;
    assert!(buttons[0].top() > cube.bottom());
    // (El botón se pinta en su lugar redondeado.)
    assert_eq!(
        buttons[0].top().round(),
        line + 2.0 + 9.0,
        "{:?}",
        buttons[0]
    );
    assert!(buttons[0].left() < buttons[1].left() && buttons[1].left() < buttons[2].left());
    let row = node(&h, "Cube");
    assert!(row.top() > buttons[0].bottom(), "{row:?}");
    assert!(text_in(&h, bottom, "UV").is_none(), "no UV title");
    // Texture Size no está abajo (va con lo general).
    assert!(text_in(&h, bottom, "Texture Size").is_none());
}

#[test]
fn pack_uvs_is_in_the_uv_row_and_packs_without_chosen_faces() {
    let mut h = with_the_model_panel(with_a_model_in_texture_mode());
    // En la fila de Rotate 90° / Flip, después de Flip V.
    let pack = h.button("Pack UVs");
    let flip = h.button("Flip V");
    assert_eq!(pack.top(), flip.top(), "in the same row");
    assert!(pack.left() > flip.left());
    // Es de todo el modelo: sin caras elegidas también se usa.
    assert!(h.app.doc.editor.selected_faces().is_empty());
    assert!(!h.disabled.iter().any(|l| l == "Pack UVs"));
    // El cubo corrido (5, 5): Pack UVs lo regresa a la esquina.
    let e = &mut h.app.doc.editor;
    let id = e.workspace().unwrap();
    let moved = cs_3d::place_object(e.workspace_model().unwrap(), 0, (5, 5));
    assert!(e.execute_doc(cs_commands::DocCommand::SetModel {
        id,
        model: Some(Box::new(moved)),
    }));
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    h.click(h.button("Pack UVs").center(), PointerButton::Primary);
    let m = h.app.doc.editor.workspace_model().unwrap();
    assert_eq!(cs_3d::object_uv_rect(m, 0), Some((0, 0, 48, 32)));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn pack_uvs_is_off_with_a_locked_texture_layer_and_says_why() {
    let mut h = with_the_model_panel(with_a_model_in_texture_mode());
    let e = &mut h.app.doc.editor;
    let id = e.workspace_model().unwrap().texture.unwrap();
    let layer = e.document.artboard(id).unwrap().sprite.layers[0].id;
    assert!(e.execute_doc(cs_commands::DocCommand::OnArtboard {
        id,
        cmd: cs_commands::Command::SetLayerProps {
            id: layer,
            props: cs_commands::LayerProps {
                locked: Some(true),
                ..Default::default()
            },
        },
    }));
    h.frame();
    assert!(h.disabled.iter().any(|l| l == "Pack UVs"));
    let pack = h.button("Pack UVs");
    bubble_over(&mut h, pack.center());
    let why = "Unlock the texture layers to pack";
    assert!(h.text_rects.iter().any(|(t, _, _)| *t == why), "{why:?}");
}

#[test]
fn opening_the_panel_beside_with_a_face_chosen_focuses_its_uvs() {
    let mut h = with_the_front_face_chosen();
    let fitted = h.app.doc.editor.model_edit().texture_view.unwrap();
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::FitActiveBeside { right_inset: 0.0 });
    h.frame();
    let view = h.app.doc.editor.model_edit().texture_view.unwrap();
    assert!(view.zoom > fitted.zoom, "{fitted:?} -> {view:?}");
    let (_, bottom) = h.app.doc.canvas.texture_panes().unwrap();
    let center = texel(&h, 8.0, 8.0);
    assert!((center - bottom.center()).length() <= 1.0);
}

#[test]
fn a_move_drag_outside_the_face_changes_nothing() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Move);
    let (uvs, pixels) = (front_uvs(&h), texture_sprite(&h));
    let selection = h.app.doc.editor.selection.clone();
    let steps = h.app.doc.editor.history.done_count();
    h.drag(texel(&h, 40.5, 40.5), texel(&h, 45.5, 44.5));
    let e = &h.app.doc.editor;
    assert_eq!(texture_sprite(&h), pixels, "no pixels cut or moved");
    assert_eq!(front_uvs(&h), uvs);
    assert_eq!(e.selection, selection);
    assert!(e.selection_is_face_mask());
    assert!(e.floating.is_none());
    assert_eq!(e.history.done_count(), steps);
}

#[test]
fn alt_click_with_the_pencil_inside_a_face_still_picks_the_color() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::Pencil);
    h.app.doc.editor.primary = PixelValue::Rgba(Rgba::rgb(1, 2, 3));
    let uvs = front_uvs(&h);
    let steps = h.app.doc.editor.history.done_count();
    let layer = texture_sprite(&h).layers.last().unwrap().id;
    let under = texture_sprite(&h).pixel(layer, 0, 8, 8);
    let at = texel(&h, 8.5, 8.5);
    drag_holding(&mut h, at, at, egui::Modifiers::ALT);
    let e = &h.app.doc.editor;
    assert_eq!(e.primary, under, "the eyedropper picked it");
    assert_eq!(front_uvs(&h), uvs, "no UV change");
    assert_eq!(e.history.done_count(), steps);
}

#[test]
fn alt_drag_with_a_selection_tool_on_a_face_moves_its_uvs() {
    let mut h = with_the_front_face_chosen();
    h.app.doc.editor.set_tool(ToolKind::RectSelect);
    let before = front_uvs(&h);
    let (from, to) = (texel(&h, 8.5, 8.5), texel(&h, 9.5, 13.5));
    drag_holding(&mut h, from, to, egui::Modifiers::ALT);
    let moved: Vec<_> = before.iter().map(|&(x, y)| (x + 1, y + 5)).collect();
    assert_eq!(front_uvs(&h), moved);
    assert!(
        h.app.doc.editor.selection_is_face_mask(),
        "the face's pixels"
    );
}

// --- ChimpCAD: Extrude, Subdivide, Merge y Delete ---

/// El modelo en su pestaña con la cara de arriba del cubo elegida (modo Face) y el puntero en
/// su centro en la vista Front. Devuelve la cara.
fn with_the_top_face_chosen() -> (LayoutHarness, usize) {
    let mut h = editing_model();
    let mesh = &h.app.doc.editor.model().unwrap().objects[0].mesh;
    let top = mesh
        .faces
        .iter()
        .position(|f| f.verts.iter().all(|&v| mesh.vertices[v as usize].y > 0))
        .expect("the top face");
    let e = &mut h.app.doc.editor;
    e.set_select_mode(cs_3d::SelectMode::Face);
    e.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: [cs_3d::Item::Face(0, top)].into(),
    });
    let at = front_screen(&h, [0.0, 16.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(at)]);
    (h, top)
}

fn top_ys(h: &LayoutHarness, top: usize) -> Vec<i32> {
    let mesh = &h.app.doc.editor.model().unwrap().objects[0].mesh;
    mesh.faces[top]
        .verts
        .iter()
        .map(|&v| mesh.vertices[v as usize].y)
        .collect()
}

fn vertex_count(h: &LayoutHarness) -> usize {
    h.app.doc.editor.model().unwrap().objects[0]
        .mesh
        .vertices
        .len()
}

#[test]
fn ctrl_e_then_a_move_and_a_click_extrudes_in_one_undo_step() {
    let (mut h, top) = with_the_top_face_chosen();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    assert!(
        matches!(
            h.app.doc.editor.model_edit().op,
            Some(crate::editor::EditOp::Extrude { .. })
        ),
        "the extrude follows the pointer"
    );
    assert!(h.app.export.is_none(), "Ctrl+E is not Export here");
    let to = front_screen(&h, [0.0, 20.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    assert_eq!(vertex_count(&h), 8, "the document waits");
    h.click(to, PointerButton::Primary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(vertex_count(&h), 12);
    assert_eq!(top_ys(&h, top), vec![20; 4], "moved outward");
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.app.doc.editor.undo();
    assert_eq!(vertex_count(&h), 8, "one undo step");
}

#[test]
fn escape_cancels_an_extrude_without_an_undo_step() {
    let (mut h, top) = with_the_top_face_chosen();
    let before = h.app.doc.editor.model().cloned();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    h.frame_with(vec![Event::PointerMoved(front_screen(
        &h,
        [0.0, 20.0, 0.0],
    ))]);
    tap_key(&mut h, egui::Key::Escape);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(h.app.doc.editor.model().cloned(), before);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    assert_eq!(top_ys(&h, top), vec![16; 4]);
}

#[test]
fn a_right_click_cancels_an_extrude() {
    let (mut h, _) = with_the_top_face_chosen();
    let before = h.app.doc.editor.model().cloned();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    let to = front_screen(&h, [0.0, 20.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.click(to, PointerButton::Secondary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(h.app.doc.editor.model().cloned(), before);
}

#[test]
fn ctrl_d_m_and_delete_are_one_undo_step_each() {
    let (mut h, _) = with_the_top_face_chosen();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::D, egui::Modifiers::COMMAND);
    h.frame();
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Subdivide"));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    tap_key(&mut h, egui::Key::M);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Merge"));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 2);
    // Lo unido queda elegido (un vértice): Delete lo borra con sus caras.
    tap_key(&mut h, egui::Key::Delete);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Delete"));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 3);
}

#[test]
fn backspace_deletes_too() {
    let (mut h, _) = with_the_top_face_chosen();
    let steps = h.app.doc.editor.history.done_count();
    tap_key(&mut h, egui::Key::Backspace);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    let faces = h.app.doc.editor.model().unwrap().objects[0]
        .mesh
        .faces
        .len();
    assert_eq!(faces, 5);
}

#[test]
fn ctrl_e_and_ctrl_d_do_not_export_or_deselect_in_the_cad_workspace() {
    let mut h = editing_model();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    press(&mut h, egui::Key::D, egui::Modifiers::COMMAND);
    h.frame();
    assert!(h.app.export.is_none(), "no Export");
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    assert!(
        h.app.doc.editor.model_edit().op.is_none(),
        "nothing to extrude"
    );
}

#[test]
fn on_the_canvas_ctrl_e_still_exports_and_ctrl_d_deselects() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.doc.editor.select_all();
    h.frame();
    assert!(h.app.doc.editor.selection.is_some());
    press(&mut h, egui::Key::D, egui::Modifiers::COMMAND);
    h.frame();
    assert!(h.app.doc.editor.selection.is_none(), "deselected");
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    assert!(h.app.export.is_some(), "Export");
}

#[test]
fn the_topology_buttons_follow_the_selection_and_the_mode() {
    let mut h = with_the_model_panel(with_a_model_at_800());
    let labels = ["Extrude", "Subdivide", "Merge", "Delete"];
    let enabled = |h: &LayoutHarness| -> Vec<bool> {
        labels
            .iter()
            .map(|l| {
                h.button(l);
                !h.disabled.iter().any(|d| d == l)
            })
            .collect()
    };
    let choose = |h: &mut LayoutHarness, sel: cs_3d::Selection| {
        h.app.doc.editor.set_select_mode(sel.mode);
        h.app.doc.editor.set_model_selection(sel);
        h.frame();
        h.frame();
    };
    let vertices = |vs: &[u32]| cs_3d::Selection {
        mode: cs_3d::SelectMode::Vertex,
        items: vs.iter().map(|&v| cs_3d::Item::Vertex(0, v)).collect(),
    };
    h.app.doc.editor.select_model_object(Some(0));
    h.frame();
    h.frame();
    assert_eq!(enabled(&h), [false, false, false, true], "Object mode");
    choose(&mut h, vertices(&[]));
    assert_eq!(enabled(&h), [false, false, false, false], "nothing chosen");
    choose(&mut h, vertices(&[0]));
    assert_eq!(enabled(&h), [false, false, false, true], "one vertex");
    choose(&mut h, vertices(&[0, 1]));
    assert_eq!(enabled(&h), [false, false, true, true], "two vertices");
    choose(
        &mut h,
        cs_3d::Selection {
            mode: cs_3d::SelectMode::Face,
            items: [cs_3d::Item::Face(0, 0)].into(),
        },
    );
    assert_eq!(enabled(&h), [true, true, true, true], "a face");
    // Los botones van en una fila arriba del outliner.
    let (_, bottom) = model_halves(&h);
    let buttons: Vec<Rect> = labels.iter().map(|l| h.button(l)).collect();
    let row = node(&h, "Cube");
    for (b, l) in buttons.iter().zip(labels) {
        assert!(bottom.contains_rect(*b), "{l} {b:?} in {bottom:?}");
        assert_eq!(b.top(), buttons[0].top(), "{l} in one row");
        assert!(b.bottom() < row.top(), "{l} above the outliner");
    }
}

#[test]
fn the_topology_buttons_run_the_commands() {
    let mut h = with_the_model_panel(with_a_model_at_800());
    let face = cs_3d::Selection {
        mode: cs_3d::SelectMode::Face,
        items: [cs_3d::Item::Face(0, 0)].into(),
    };
    let choose = |h: &mut LayoutHarness, sel: cs_3d::Selection| {
        h.app.doc.editor.set_select_mode(sel.mode);
        h.app.doc.editor.set_model_selection(sel);
        h.frame();
        h.frame();
    };
    choose(&mut h, face.clone());
    let steps = h.app.doc.editor.history.done_count();
    h.click(h.button("Subdivide").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Subdivide"));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    h.click(h.button("Delete").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Delete"));
    choose(&mut h, face);
    h.click(h.button("Extrude").center(), PointerButton::Primary);
    h.frame();
    assert!(
        matches!(
            h.app.doc.editor.model_edit().op,
            Some(crate::editor::EditOp::Extrude { .. })
        ),
        "the button starts the interactive extrude"
    );
    let steps = h.app.doc.editor.history.done_count();
    tap_key(&mut h, egui::Key::Enter);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Extrude"));
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn object_mode_delete_removes_the_object_and_merge_does_nothing() {
    let mut h = editing_model();
    assert!(h.app.doc.editor.add_primitive(cs_3d::Primitive::Cube));
    h.app.doc.editor.select_model_object(Some(1));
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    tap_key(&mut h, egui::Key::M);
    press(&mut h, egui::Key::D, egui::Modifiers::COMMAND);
    h.frame();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    assert!(h.app.doc.editor.model_edit().op.is_none(), "no extrude");
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    tap_key(&mut h, egui::Key::Delete);
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), 1);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn g_and_s_during_an_extrude_do_not_drop_it() {
    let (mut h, top) = with_the_top_face_chosen();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    tap_key(&mut h, egui::Key::G);
    tap_key(&mut h, egui::Key::S);
    assert!(
        matches!(
            h.app.doc.editor.model_edit().op,
            Some(crate::editor::EditOp::Extrude { .. })
        ),
        "still extruding"
    );
    let to = front_screen(&h, [0.0, 20.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.click(to, PointerButton::Primary);
    assert!(h.app.doc.editor.model_edit().op.is_none());
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Extrude"));
    assert_eq!(vertex_count(&h), 12);
    assert_eq!(top_ys(&h, top), vec![20; 4]);
}

#[test]
fn number_keys_and_arrows_do_nothing_during_an_extrude() {
    let (mut h, top) = with_the_top_face_chosen();
    let steps = h.app.doc.editor.history.done_count();
    press(&mut h, egui::Key::E, egui::Modifiers::COMMAND);
    h.frame();
    let sel = h.app.doc.editor.model_edit().selection.clone();
    tap_key(&mut h, egui::Key::Num2);
    tap_key(&mut h, egui::Key::ArrowRight);
    assert_eq!(h.app.doc.editor.model_edit().selection, sel);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
    let to = front_screen(&h, [0.0, 20.0, 0.0]);
    h.frame_with(vec![Event::PointerMoved(to)]);
    h.click(to, PointerButton::Primary);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    assert_eq!(h.app.doc.editor.history.done().last(), Some("Extrude"));
    assert_eq!(top_ys(&h, top), vec![20; 4], "the cap moved");
}

#[test]
fn the_duplicate_button_sits_left_of_the_plus_and_duplicates() {
    let mut h = model_panel_open(true);
    let plus = in_model_panel(&h, "Add object").expect("the + button");
    let copy = in_model_panel(&h, "Duplicate").expect("the duplicate button");
    let group = in_model_panel(&h, "New group").expect("the new group button");
    assert_eq!(copy.size(), plus.size());
    assert_eq!(copy.top(), plus.top());
    assert_eq!(group.left() - copy.right(), 4.0, "{copy:?} {group:?}");
    assert_eq!(plus.left() - group.right(), 4.0, "{group:?} {plus:?}");
    let n = h.app.doc.editor.model().unwrap().objects.len();
    h.click(copy.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), n + 1);
    // Su fila aparece y queda elegida.
    h.frame();
    assert!(h.nodes.iter().any(|(l, _)| l == "Cube copy"));
    assert_eq!(h.app.doc.editor.selected_object(), Some(n));
    // Sin objeto elegido, deshabilitado.
    let h = without_an_object(h);
    assert!(
        h.disabled.iter().any(|l| l == "Duplicate"),
        "{:?}",
        h.disabled
    );
}

#[test]
fn ctrl_j_duplicates_the_3d_object_in_the_model_tab() {
    let mut h = model_panel_open(true);
    assert!(h.app.doc.editor.pixel_actions_blocked());
    let n = h.app.doc.editor.model().unwrap().objects.len();
    press_ctrl(&mut h, egui::Key::J);
    h.frame();
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), n + 1);
}

#[test]
fn ctrl_j_with_a_group_active_duplicates_the_group() {
    let mut h = model_panel_open(true);
    assert!(h.app.doc.editor.new_group());
    h.frame();
    assert_eq!(
        h.app.doc.editor.active_row(),
        Some(crate::editor::Row::Group(0))
    );
    press_ctrl(&mut h, egui::Key::J);
    h.frame();
    let model = h.app.doc.editor.model().unwrap();
    assert_eq!(model.groups.len(), 2);
    assert_eq!(model.objects.len(), 2);
    assert_eq!(model.objects[1].name, "Cube", "only the group is a copy");
    assert_eq!(
        h.app.doc.editor.active_row(),
        Some(crate::editor::Row::Group(1))
    );
}

#[test]
fn the_duplicate_tooltip_says_group_with_a_group_active() {
    let mut h = model_panel_open(true);
    let copy = in_model_panel(&h, "Duplicate").expect("the duplicate button");
    bubble_over(&mut h, copy.center());
    let shortcut = crate::keymap::label(&h.ctx, crate::keymap::Action::DuplicateObject);
    let object = format!("Duplicate object ({shortcut})");
    assert!(
        h.text_rects.iter().any(|(t, _, _)| *t == object),
        "{object:?}"
    );
    assert!(h.app.doc.editor.new_group());
    bubble_over(&mut h, copy.center());
    let group = format!("Duplicate group ({shortcut})");
    assert!(
        h.text_rects.iter().any(|(t, _, _)| *t == group),
        "{group:?}"
    );
}

#[test]
fn delete_over_the_outliner_removes_every_chosen_row() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    add_object(&mut h, "Plane");
    let editor = &mut h.app.doc.editor;
    editor.set_active_row(Some(crate::editor::Row::Object(0)), false);
    editor.set_active_row(Some(crate::editor::Row::Object(2)), true);
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    h.frame_with(vec![Event::PointerMoved(node(&h, "Cube").center())]);
    tap_key(&mut h, egui::Key::Delete);
    let model = h.app.doc.editor.model().unwrap();
    let names: Vec<&str> = model.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["Cylinder"]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn the_delete_button_with_a_group_active_deletes_the_whole_group() {
    let mut h = model_panel_tall();
    assert!(h.app.doc.editor.add_primitive(cs_3d::Primitive::Plane));
    h.app.doc.editor.set_active_row(None, false);
    assert!(h.app.doc.editor.new_group());
    assert!(h.app.doc.editor.move_row(
        crate::editor::Row::Object(0),
        crate::editor::DropAt::Into(0)
    ));
    h.app
        .doc
        .editor
        .set_active_row(Some(crate::editor::Row::Group(0)), false);
    h.frame();
    h.frame();
    let steps = h.app.doc.editor.history.done_count();
    h.click(h.button("Delete").center(), PointerButton::Primary);
    let model = h.app.doc.editor.model().unwrap();
    assert!(model.groups.is_empty());
    let names: Vec<&str> = model.objects.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, vec!["Plane"]);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
}

#[test]
fn delete_over_a_menu_on_the_outliner_does_not_delete_the_row() {
    let mut h = model_panel_open(true);
    add_object(&mut h, "Cylinder");
    add_object(&mut h, "Plane");
    // La fila activa es el cubo, pero en las vistas no hay nada elegido: Supr en las vistas no
    // borra nada.
    let editor = &mut h.app.doc.editor;
    editor.set_active_row(Some(crate::editor::Row::Object(0)), false);
    editor.set_select_mode(cs_3d::SelectMode::Vertex);
    editor.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Vertex,
        items: Default::default(),
    });
    h.frame();
    assert_eq!(
        h.app.doc.editor.active_row(),
        Some(crate::editor::Row::Object(0))
    );
    let plus = open_add_menu(&mut h);
    let menu = add_menu_box(&h, plus).expect("the + menu");
    let rows: Vec<Rect> = ["Cube", "Cylinder", "Plane"]
        .iter()
        .filter_map(|l| in_model_panel(&h, l))
        .collect();
    let over = rows
        .iter()
        .map(|r| r.intersect(menu))
        .find(|r| r.is_positive())
        .unwrap_or_else(|| panic!("the menu {menu:?} over a row {rows:?}"));
    let steps = h.app.doc.editor.history.done_count();
    h.frame_with(vec![Event::PointerMoved(over.center())]);
    h.frame();
    tap_key(&mut h, egui::Key::Delete);
    assert_eq!(h.app.doc.editor.model().unwrap().objects.len(), 3);
    assert_eq!(h.app.doc.editor.history.done_count(), steps);
}

// --- La barrita del objeto, debajo del encabezado de ChimpCAD ---

mod object_bar {
    use super::*;
    use crate::editor::Row;

    /// Los nueve campos de la barrita, por su nombre.
    const FIELDS: [&str; 9] = [
        "Position X",
        "Position Y",
        "Position Z",
        "Rotation X",
        "Rotation Y",
        "Rotation Z",
        "Scale X",
        "Scale Y",
        "Scale Z",
    ];

    /// El área del workspace debajo del encabezado y su línea.
    fn below_header(h: &LayoutHarness) -> Rect {
        let canvas = h.canvas.expect("the workspace area");
        let header = h.app.doc.canvas.cad_header().expect("the header");
        Rect::from_min_max(pos2(canvas.left(), header.bottom() + 1.0), canvas.max)
    }

    /// Los textos dibujados dentro de `r`.
    fn texts_in(h: &LayoutHarness, r: Rect) -> Vec<String> {
        h.text_rects
            .iter()
            .filter(|(_, t, _)| r.contains_rect(*t))
            .map(|(s, _, _)| s.clone())
            .collect()
    }

    fn settle(h: &mut LayoutHarness) {
        h.frame();
        h.frame();
    }

    /// Escribe `text` en el campo `label` de la barrita (clic, todo elegido, borrar, el texto y
    /// Enter).
    fn type_in(h: &mut LayoutHarness, label: &str, text: &str) {
        h.click(node(h, label).center(), PointerButton::Primary);
        assert!(h.ctx.egui_wants_keyboard_input(), "{label} has focus");
        press(h, egui::Key::A, egui::Modifiers::COMMAND);
        h.frame_with(key_tap(egui::Key::Backspace));
        h.frame_with(vec![Event::Text(text.into())]);
        h.frame_with(key_tap(egui::Key::Enter));
        settle(h);
    }

    fn transform(h: &LayoutHarness, o: usize) -> cs_core::Transform3D {
        h.app.doc.editor.model().unwrap().objects[o].transform
    }

    #[test]
    fn the_object_bar_sits_under_the_chimpcad_header_in_both_modes() {
        let mut h = with_a_model_at_800();
        let header = h.app.doc.canvas.cad_header().expect("the header");
        let area = below_header(&h);
        let bar = crate::panels::options_bar_height(h.app.pixel.skin());
        let quads = h.app.doc.canvas.model_edit_quadrants().expect("the views");
        // Las vistas empiezan debajo de la barrita.
        assert_eq!(quads[0].top(), area.top() + bar, "{quads:?}");
        let titles = ["Position", "Rotation", "Scale"]
            .map(|t| text_in(&h, area, t).unwrap_or_else(|| panic!("{t} under the header")));
        for (t, r) in ["Position", "Rotation", "Scale"].iter().zip(titles) {
            assert!(r.top() > header.bottom(), "{t} {r:?} under {header:?}");
            assert!(r.bottom() < quads[0].top(), "{t} {r:?} over the views");
            assert_eq!(r.top(), titles[0].top(), "{t} in one row");
        }
        assert!(titles[0].right() < titles[1].left() && titles[1].right() < titles[2].left());
        // Cada título con sus tres campos después, en la fila de la barrita.
        for (k, f) in FIELDS.iter().enumerate() {
            let r = node(&h, f);
            assert!(
                area.contains_rect(r) && r.bottom() < quads[0].top(),
                "{f} {r:?}"
            );
            assert!(r.left() > titles[k / 3].right(), "{f} after its title");
        }
        // En modo Texture, la barrita en el mismo lugar y, debajo, el render; la fila del
        // pincel va arriba de la textura (entre la línea del medio y la textura).
        h.app
            .doc
            .editor
            .set_cad_mode(crate::editor::CadMode::Texture);
        settle(&mut h);
        assert_eq!(
            h.app.doc.canvas.cad_header(),
            Some(header),
            "the header stays"
        );
        assert_eq!(text_in(&h, area, "Position"), Some(titles[0]));
        let (top, bottom) = h.app.doc.canvas.texture_panes().unwrap();
        assert_eq!(top.top(), area.top() + bar);
        let size = text_in(&h, area, "Brush Size").expect("the brush row");
        let ink = text_in(&h, area, "Ink").expect("the ink");
        for r in [size, ink] {
            assert!(
                r.top() > top.bottom() && r.bottom() < bottom.top(),
                "{r:?} between the render {top:?} and the texture {bottom:?}"
            );
        }
    }

    #[test]
    fn dragging_a_position_field_moves_the_object_in_one_step() {
        let mut h = with_a_model_at_800();
        assert_eq!(h.app.doc.editor.active_row(), Some(Row::Object(0)));
        let x = node(&h, "Position X");
        let steps = h.app.doc.editor.history.done_count();
        h.drag(x.center(), x.center() + vec2(20.0, 0.0));
        settle(&mut h);
        let p = transform(&h, 0).position;
        assert!(p.x > 0, "{p:?}");
        assert_eq!((p.y, p.z), (0, 0));
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
        h.app.doc.editor.undo();
        assert_eq!(transform(&h, 0).position.x, 0);
    }

    /// Escribir un número tecla por tecla es un solo paso de undo (no uno por dígito); el campo
    /// siguiente, otro.
    #[test]
    fn typing_in_a_field_is_one_undo_step() {
        let mut h = with_a_model_at_800();
        let steps = h.app.doc.editor.history.done_count();
        let x = node(&h, "Position X");
        h.click(x.center(), PointerButton::Primary);
        press(&mut h, egui::Key::A, egui::Modifiers::COMMAND);
        h.frame_with(key_tap(egui::Key::Backspace));
        for c in ["1", "2", "3"] {
            h.frame_with(vec![Event::Text(c.into())]);
            h.frame();
        }
        h.frame_with(key_tap(egui::Key::Enter));
        settle(&mut h);
        assert_eq!(transform(&h, 0).position.x, 123);
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
        type_in(&mut h, "Position Y", "7");
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 2);
        h.app.doc.editor.undo();
        assert_eq!(transform(&h, 0).position.y, 0);
        assert_eq!(transform(&h, 0).position.x, 123);
        h.app.doc.editor.undo();
        assert_eq!(transform(&h, 0).position.x, 0);
    }

    #[test]
    fn with_a_group_active_the_bar_edits_the_group() {
        let mut h = with_a_model_at_800();
        assert!(h.app.doc.editor.new_group());
        h.app.doc.editor.set_active_row(Some(Row::Group(0)), false);
        settle(&mut h);
        assert_eq!(h.app.doc.editor.active_row(), Some(Row::Group(0)));
        let steps = h.app.doc.editor.history.done_count();
        type_in(&mut h, "Position X", "8");
        let model = h.app.doc.editor.model().unwrap();
        assert_eq!(model.groups[0].transform.position.x, 8);
        assert_eq!(model.objects[0].transform.position.x, 0, "the cube stays");
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    }

    #[test]
    fn the_bar_is_disabled_without_an_active_row_or_when_locked_with_its_values() {
        let mut h = with_a_model_at_800();
        let all_off = |h: &LayoutHarness| FIELDS.iter().all(|f| h.off.iter().any(|o| o == f));
        let none_off = |h: &LayoutHarness| !FIELDS.iter().any(|f| h.off.iter().any(|o| o == f));
        assert!(none_off(&h), "{:?}", h.off);
        // Con valor: el número en el campo.
        assert!(texts_in(&h, node(&h, "Scale X")).contains(&"100".to_owned()));
        // Sin fila activa: los nueve, vacíos y deshabilitados.
        h.app.doc.editor.set_active_row(None, false);
        settle(&mut h);
        assert!(all_off(&h), "{:?}", h.off);
        for f in FIELDS {
            // Solo la letra del eje: ningún número.
            let texts = texts_in(&h, node(&h, f));
            let numbers = texts.iter().any(|t| t.chars().any(|c| c.is_ascii_digit()));
            assert!(!numbers, "{f} empty: {texts:?}");
        }
        let steps = h.app.doc.editor.history.done_count();
        let x = node(&h, "Position X");
        h.drag(x.center(), x.center() + vec2(20.0, 0.0));
        assert_eq!(
            h.app.doc.editor.history.done_count(),
            steps,
            "nothing to move"
        );
        // Con el cubo bloqueado, también, pero con sus valores.
        h.app.doc.editor.set_active_row(Some(Row::Object(0)), false);
        settle(&mut h);
        assert!(none_off(&h));
        assert!(h.app.doc.editor.toggle_row_locked(Row::Object(0)));
        settle(&mut h);
        assert!(all_off(&h), "locked: {:?}", h.off);
        assert!(
            texts_in(&h, node(&h, "Scale X")).contains(&"100".to_owned()),
            "the locked values show"
        );
        let steps = h.app.doc.editor.history.done_count();
        h.drag(x.center(), x.center() + vec2(20.0, 0.0));
        settle(&mut h);
        assert_eq!(h.app.doc.editor.history.done_count(), steps, "locked");
        // Y con un grupo bloqueado que lo contiene (bloqueado por herencia).
        assert!(h.app.doc.editor.toggle_row_locked(Row::Object(0)));
        assert!(h.app.doc.editor.new_group());
        assert!(h.app.doc.editor.toggle_row_locked(Row::Group(0)));
        h.app.doc.editor.set_active_row(Some(Row::Object(0)), false);
        settle(&mut h);
        assert!(all_off(&h), "inherited lock: {:?}", h.off);
        assert!(texts_in(&h, node(&h, "Scale X")).contains(&"100".to_owned()));
    }

    #[test]
    fn with_other_objects_chosen_in_the_views_the_bar_is_disabled() {
        let mut h = with_a_model_at_800();
        assert!(h.app.doc.editor.add_primitive(cs_3d::Primitive::Cube));
        h.app.doc.editor.set_active_row(Some(Row::Object(0)), false);
        settle(&mut h);
        assert!(!h.off.iter().any(|o| o == "Position X"));
        // El otro cubo también elegido en las vistas: no es de la fila activa.
        h.app.doc.editor.set_active_row(Some(Row::Object(1)), true);
        h.app.doc.editor.set_active_row(Some(Row::Object(0)), true);
        settle(&mut h);
        let items = &h.app.doc.editor.model_edit().selection.items;
        assert!(items.contains(&cs_3d::Item::Object(1)), "{items:?}");
        assert!(h.off.iter().any(|o| o == "Position X"), "{:?}", h.off);
        // Los dos en un grupo activo: sí (son suyos).
        assert!(h.app.doc.editor.new_group());
        h.app.doc.editor.set_active_row(Some(Row::Group(0)), false);
        settle(&mut h);
        assert!(!h.off.iter().any(|o| o == "Position X"), "{:?}", h.off);
    }

    #[test]
    fn dragging_the_rotation_z_field_turns_the_object_in_one_step() {
        let mut h = with_a_model_at_800();
        let z = node(&h, "Rotation Z");
        let steps = h.app.doc.editor.history.done_count();
        h.drag(z.center(), z.center() + vec2(10.0, 0.0));
        settle(&mut h);
        let r = transform(&h, 0).rotation;
        // En grados enteros (se guarda en décimas).
        assert!(r.z > 0 && r.z % 10 == 0, "{r:?}");
        assert_eq!((r.x, r.y), (0, 0));
        assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
        h.app.doc.editor.undo();
        assert_eq!(transform(&h, 0).rotation, cs_core::Vec3i::default());
    }

    #[test]
    fn the_rotation_field_shows_the_wrapped_angle_and_scale_stops_at_one_percent() {
        let mut h = with_a_model_at_800();
        let mut model = h.app.doc.editor.model().unwrap().clone();
        model.objects[0].transform.rotation.z = 3700;
        assert!(
            h.app
                .doc
                .editor
                .set_model_grouped(model, egui::Id::new("script"))
        );
        h.app.doc.editor.end_group();
        settle(&mut h);
        // 370° se ve como 10°.
        assert!(texts_in(&h, node(&h, "Rotation Z")).contains(&"10".to_owned()));
        // La escala no baja de 1 % (10 milésimas).
        let x = node(&h, "Scale X");
        h.drag(x.center(), x.center() - vec2(400.0, 0.0));
        settle(&mut h);
        assert_eq!(transform(&h, 0).scale.x, 10);
    }

    #[test]
    fn values_outside_the_ranges_are_not_rewritten_by_drawing_the_bar() {
        let mut h = with_a_model_at_800();
        let mut model = h.app.doc.editor.model().unwrap().clone();
        let t = &mut model.objects[0].transform;
        t.rotation = cs_core::Vec3i {
            x: 4000,
            y: -9000,
            z: 3610,
        };
        t.scale = cs_core::Vec3i {
            x: 20000,
            y: -1000,
            z: 5,
        };
        t.position.x = 9999 * cs_core::FIXED;
        assert!(
            h.app
                .doc
                .editor
                .set_model_grouped(model.clone(), egui::Id::new("script"))
        );
        h.app.doc.editor.end_group();
        let steps = h.app.doc.editor.history.done_count();
        for _ in 0..3 {
            h.frame();
        }
        assert_eq!(h.app.doc.editor.model().unwrap(), &model);
        assert_eq!(h.app.doc.editor.history.done_count(), steps);
    }

    #[test]
    fn delete_and_backspace_in_a_bar_field_do_not_delete_geometry() {
        let (mut h, _) = with_the_top_face_chosen();
        h.click(node(&h, "Position X").center(), PointerButton::Primary);
        assert!(h.ctx.egui_wants_keyboard_input(), "the field has focus");
        let model = h.app.doc.editor.model().cloned();
        let steps = h.app.doc.editor.history.done_count();
        for key in [egui::Key::Backspace, egui::Key::Delete] {
            h.frame_with(key_tap(key));
        }
        h.frame();
        assert_eq!(
            h.app.doc.editor.model().cloned(),
            model,
            "no geometry deleted"
        );
        assert_eq!(h.app.doc.editor.history.done_count(), steps);
        assert!(!h.app.doc.editor.model_edit().selection.is_empty());
    }

    #[test]
    fn the_object_tab_has_no_transform_rows_any_more() {
        let h = model_panel_tall();
        for h in [&h, &without_an_object(model_panel_tall())] {
            let flyout = model_flyout(h);
            for t in ["Position", "Rotation", "Scale", "Double sided"] {
                assert!(text_in(h, flyout, t).is_none(), "{t} in the panel");
            }
            assert!(!h.checks.iter().any(|(l, _, _)| l == "Double sided"));
            // Los botones del objeto, entre el encabezado del outliner y su lista.
            let header = node(h, "Add object");
            let cube = node(h, "Cube");
            for l in ["Extrude", "Subdivide", "Merge", "Delete"] {
                let b = h.button(l);
                assert!(flyout.contains_rect(b), "{l} {b:?}");
                assert!(
                    b.top() > header.bottom() && b.bottom() < cube.top(),
                    "{l} {b:?}"
                );
            }
        }
    }
}

/// Solo un cilindro de 8 lados en el modelo, en modo Face.
fn editing_a_cylinder() -> LayoutHarness {
    let mut h = editing_model();
    let e = &mut h.app.doc.editor;
    assert!(e.add_primitive(cs_3d::Primitive::Cylinder { sides: 8 }));
    assert!(e.remove_object(0));
    e.set_select_mode(cs_3d::SelectMode::Face);
    h.frame();
    h
}

/// Las caras de la tapa de arriba del cilindro (la parte 1), y el centro en Top de los gajos
/// `a` y `b` de ella.
fn top_cap_wedges(h: &LayoutHarness, a: usize, b: usize) -> (Vec<cs_3d::Item>, Pos2, Pos2) {
    let editor = &h.app.doc.editor;
    let model = editor.model().unwrap();
    let mesh = &model.objects[0].mesh;
    let cap: Vec<usize> = (0..mesh.faces.len())
        .filter(|&f| mesh.faces[f].part == Some(1))
        .collect();
    let map = h
        .app
        .doc
        .canvas
        .edit_view_map(editor, cs_3d::OrthoView::Top)
        .expect("top view");
    let pts = cs_3d::world_points(model, 0);
    let center = |f: usize| {
        let vs = &mesh.faces[f].verts;
        let (x, y) = vs.iter().fold((0.0, 0.0), |(x, y), &v| {
            let s = map.to_screen(pts[v as usize]);
            (x + s.0, y + s.1)
        });
        let k = vs.len() as f32;
        Pos2::new(x / k, y / k)
    };
    let items = cap.iter().map(|&f| cs_3d::Item::Face(0, f)).collect();
    (items, center(cap[a]), center(cap[b]))
}

#[test]
fn clicking_a_wedge_of_the_cylinder_cap_selects_the_whole_cap() {
    let mut h = editing_a_cylinder();
    let (cap, first, other) = top_cap_wedges(&h, 0, 3);
    assert_eq!(cap.len(), 8);
    h.click(first, PointerButton::Primary);
    assert_eq!(chosen_items(&h), cap, "the whole cap");
    // Shift + clic en otro gajo de la misma tapa: se quita entera.
    shift_click(&mut h, other);
    assert!(chosen_items(&h).is_empty(), "{:?}", chosen_items(&h));
    // Y otra vez: se agrega entera.
    shift_click(&mut h, other);
    assert_eq!(chosen_items(&h), cap);
}

#[test]
fn switching_to_face_mode_chooses_whole_faces() {
    let mut h = editing_a_cylinder();
    let e = &mut h.app.doc.editor;
    e.set_select_mode(cs_3d::SelectMode::Vertex);
    let mesh = &e.model().unwrap().objects[0].mesh;
    let quad = mesh.faces[0].verts.clone();
    let sides: Vec<cs_3d::Item> = (0..mesh.faces.len())
        .filter(|&f| mesh.faces[f].part == Some(0))
        .map(|f| cs_3d::Item::Face(0, f))
        .collect();
    assert_eq!(sides.len(), 8);
    e.set_model_selection(cs_3d::Selection {
        mode: cs_3d::SelectMode::Vertex,
        items: quad.iter().map(|&v| cs_3d::Item::Vertex(0, v)).collect(),
    });
    // Los 4 vértices de un lado dan esa cara y, con ella, todos los lados.
    e.set_select_mode(cs_3d::SelectMode::Face);
    assert_eq!(chosen_items(&h), sides);
}

#[test]
fn build_opens_the_format_menu_and_the_dialog() {
    let mut h = font_panel_open(Vec2::new(1200.0, 800.0));
    h.click(h.button("Build ▾").center(), PointerButton::Primary);
    h.frame();
    assert!(h.button("TTF…").width() > 0.0);
    assert!(h.button("BMFont (.fnt + .png)…").width() > 0.0);
    assert!(
        h.disabled.iter().any(|l| l == "Color TTF (COLR)…"),
        "1 bit: no color TTF"
    );
    h.click(h.button("TTF…").center(), PointerButton::Primary);
    h.frame();
    assert_eq!(
        h.app.build_font.as_ref().map(|d| d.format),
        Some(crate::font_io::BuildFormat::Ttf)
    );
}

#[test]
fn the_library_offers_importing_a_font() {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, Vec2::new(1200.0, 800.0));
    h.click(h.button("Library").center(), PointerButton::Primary);
    h.frame();
    assert!(h.button("Import…").width() > 0.0);
}

/// El modelo de 800 en modo Anim (con el botón del encabezado).
fn with_a_model_in_anim_mode() -> LayoutHarness {
    let mut h = with_a_model_at_800();
    h.click(node(&h, "Anim").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.cad_mode(), crate::editor::CadMode::Anim);
    h.frame();
    h
}

/// Dónde está el bloque `i` de la pista `row` en el timeline del modo Anim.
fn anim_block(h: &LayoutHarness, row: crate::editor::Row, i: usize) -> Rect {
    use crate::canvas::anim_mode_block;
    anim_mode_block(&h.app.doc.canvas, &h.app.doc.editor, row, i).expect("the block is shown")
}

#[test]
fn anim_mode_has_the_persp_on_top_and_the_bar_timeline_and_panel_below() {
    let h = with_a_model_in_anim_mode();
    let canvas = h.canvas.expect("the workspace area");
    let panes = h.app.doc.canvas.anim_panes().expect("the anim panes");
    assert!(
        panes.top.contains_rect(persp_pixels(&h)),
        "the model on top"
    );
    assert_eq!(panes.bar.top(), panes.top.bottom() + 1.0);
    assert!(panes.ruler.top() >= panes.bar.bottom());
    assert_eq!(panes.tracks.bottom(), canvas.bottom());
    assert_eq!(panes.panel.right(), canvas.right());
    assert!(panes.names.right() < panes.tracks.left());
    // La barra: los botones de reproducción, el tiempo, Length, Loop y Add.
    for label in [
        "Go to start",
        "Play",
        "Go to end",
        "Loop",
        "Rotate",
        "Spin",
        "Bob",
    ] {
        assert!(h.nodes.iter().any(|(l, _)| l == label), "{label}");
    }
    assert!(h.texts.iter().any(|(t, _)| t == "0/2s"), "{:?}", h.texts);
    assert!(h.texts.iter().any(|(t, _)| t == "No block"));
    // Sin gizmos ni vistas orto.
    assert!(!h.nodes.iter().any(|(l, _)| {
        l == "Move"
            && h.app
                .doc
                .canvas
                .cad_header()
                .unwrap()
                .contains_rect(node(&h, "Move"))
    }));
    assert!(h.app.doc.canvas.model_edit_quadrants().is_none());
}

#[test]
fn adding_a_block_and_dragging_it_in_the_timeline() {
    use crate::editor::Row;
    let mut h = with_a_model_in_anim_mode();
    // El cubo es la pista elegida: Add → Rotate le agrega un bloque en 0.
    h.click(node(&h, "Rotate").center(), PointerButton::Primary);
    let clip = |h: &LayoutHarness| h.app.doc.editor.edited_model().unwrap().objects[0].anim[0];
    assert_eq!((clip(&h).start_ms, clip(&h).duration_ms), (0, 1000));
    assert!(
        h.texts.iter().any(|(t, _)| t == "Rotate"),
        "the block panel"
    );
    let steps = h.app.doc.editor.history.done_count();
    // Arrastrarlo un cuarto de la animación (500 ms) a la derecha.
    let r = anim_block(&h, Row::Object(0), 0);
    let panes = h.app.doc.canvas.anim_panes().unwrap();
    let per_ms = (panes.tracks.width() - 8.0) / 2000.0;
    h.drag(r.center(), r.center() + vec2(500.0 * per_ms, 0.0));
    assert_eq!(clip(&h).start_ms, 500);
    assert_eq!(h.app.doc.editor.history.done_count(), steps + 1);
    // Estirar el final hasta el final de la animación.
    let r = anim_block(&h, Row::Object(0), 0);
    h.drag(
        pos2(r.right() - 1.0, r.center().y),
        pos2(panes.tracks.right(), r.center().y),
    );
    assert_eq!(clip(&h).end_ms(), 2000);
    // Supr lo borra.
    press(&mut h, egui::Key::Delete, Default::default());
    assert!(
        h.app.doc.editor.edited_model().unwrap().objects[0]
            .anim
            .is_empty()
    );
}

#[test]
fn the_ruler_moves_the_time_and_play_advances_it() {
    let mut h = with_a_model_in_anim_mode();
    let panes = h.app.doc.canvas.anim_panes().unwrap();
    let x = panes.tracks.left() + 4.0 + (panes.tracks.width() - 8.0) / 2.0;
    h.click(pos2(x, panes.ruler.center().y), PointerButton::Primary);
    let t = h.app.doc.editor.anim().time_ms;
    assert!((990..=1010).contains(&t), "{t}");
    assert!(
        h.texts
            .iter()
            .any(|(s, _)| s.starts_with('1') && s.ends_with("/2s"))
    );
    h.click(node(&h, "Play").center(), PointerButton::Primary);
    assert!(h.app.doc.editor.anim().playing);
    let before = h.app.doc.editor.anim().time_ms;
    for _ in 0..4 {
        h.frame();
    }
    assert!(h.app.doc.editor.anim().time_ms > before);
    // Enter pausa.
    press(&mut h, egui::Key::Enter, Default::default());
    assert!(!h.app.doc.editor.anim().playing);
}

/// Una fuente 16×16 con `a` y `´` dibujadas y la `á` armada, en la `á`, con Move.
fn font_with_a_composite(h: &mut LayoutHarness) {
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    e.add_glyphs(&['´']);
    for (ch, points) in [('a', vec![(1, 8), (4, 11)]), ('´', vec![(2, 1), (3, 0)])] {
        e.go_to_glyph(ch);
        let (layer, frame) = (e.layer, e.frame);
        e.execute(Command::DrawPixels {
            layer,
            frame,
            pixels: points
                .into_iter()
                .map(|(x, y)| (x, y, cs_core::ONE_BIT_INK))
                .collect(),
        });
    }
    assert_eq!(e.auto_build_accents(None, false).built, vec!['á']);
    e.go_to_glyph('á');
    e.tool = ToolKind::Move;
    h.app
        .doc
        .canvas
        .request_zoom(crate::view::ZoomRequest::CenterActive);
    h.frame();
    h.frame();
}

/// La esquina de la celda del glifo actual en pantalla y lo que mide un pixel (de la línea del
/// ancho, que va de arriba abajo de la celda a `advance` columnas del borde).
fn glyph_origin(h: &LayoutHarness) -> (Pos2, f32) {
    let line = h
        .app
        .doc
        .canvas
        .advance_line_rect()
        .expect("the advance line");
    let scale = h.app.doc.canvas.scale().x;
    let advance = h.app.doc.editor.glyph_advance().unwrap() as f32;
    (
        Pos2::new(line.center().x - advance * scale, line.top()),
        scale,
    )
}

#[test]
fn dragging_a_component_with_move_moves_it_in_one_step() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    font_with_a_composite(&mut h);
    let start = h.app.doc.editor.glyph_components()[1].offset;
    let (origin, scale) = glyph_origin(&h);
    // Un pixel de la marca: (3, 0) de la `´` más su offset.
    let at = |(x, y): (i32, i32)| {
        origin + egui::vec2((x as f32 + 0.5) * scale, (y as f32 + 0.5) * scale)
    };
    let from = at((3 + start.0, start.1));
    let to = from + egui::vec2(2.0 * scale, scale);
    h.drag(from, to);
    let e = &mut h.app.doc.editor;
    assert_eq!(e.selected_component(), Some(1));
    assert_eq!(e.glyph_components()[1].offset, (start.0 + 2, start.1 + 1));
    let frame = e.frame;
    assert!(
        e.sprite().cel(e.layer, frame).is_none(),
        "Move did not touch the pixels"
    );
    e.undo();
    assert_eq!(e.glyph_components()[1].offset, start);
    // Las flechas mueven el elegido.
    h.frame_with(key_tap(egui::Key::ArrowRight));
    let e = &h.app.doc.editor;
    assert_eq!(e.glyph_components()[1].offset, (start.0 + 1, start.1));
    assert_eq!(
        e.current_glyph(),
        Some('á'),
        "the arrow did not change the glyph"
    );
}

#[test]
fn generate_builds_the_accents_and_the_glyph_section_unlinks_them() {
    let mut h = font_panel_open(Vec2::new(1200.0, 1000.0));
    let e = &mut h.app.doc.editor;
    e.add_glyphs(&['´']);
    for (ch, points) in [('a', vec![(0, 4), (3, 6)]), ('´', vec![(1, 0)])] {
        e.go_to_glyph(ch);
        let (layer, frame) = (e.layer, e.frame);
        e.execute(Command::DrawPixels {
            layer,
            frame,
            pixels: points
                .into_iter()
                .map(|(x, y)| (x, y, cs_core::ONE_BIT_INK))
                .collect(),
        });
    }
    h.frame();
    let generate = h.button("Generate ▾");
    let build = h.button("Build ▾");
    assert_eq!(generate.top(), build.top());
    assert!(generate.right() < build.left());
    h.click(generate.center(), PointerButton::Primary);
    h.frame();
    h.click(
        h.button("Auto-build accents").center(),
        PointerButton::Primary,
    );
    h.frame();
    assert!(h.app.doc.editor.font().unwrap().index_of('á').is_some());
    h.app.message = None;
    h.app.doc.editor.go_to_glyph('á');
    h.frame();
    h.frame();
    assert!(!h.disabled.iter().any(|l| l == "Unlink"), "a composite");
    h.click(h.button("Unlink").center(), PointerButton::Primary);
    h.frame();
    let e = &h.app.doc.editor;
    assert!(e.glyph_components().is_empty());
    assert!(e.sprite().cel(e.layer, e.frame).is_some());
}

/// El panel FontForge con una fuente de 8×8 con `l` y `o` dibujadas, en la vista Kerning, con el
/// texto "lol".
fn kerning_view_open() -> LayoutHarness {
    let mut h = font_panel_open(Vec2::new(1200.0, 1000.0));
    let e = &mut h.app.doc.editor;
    for (ch, points) in [('l', vec![(0, 1), (0, 5)]), ('o', vec![(0, 3), (2, 5)])] {
        e.go_to_glyph(ch);
        let (layer, frame) = (e.layer, e.frame);
        e.execute(Command::DrawPixels {
            layer,
            frame,
            pixels: points
                .into_iter()
                .map(|(x, y)| (x, y, cs_core::ONE_BIT_INK))
                .collect(),
        });
    }
    h.app.font_panel.test_text = "lol".into();
    h.frame();
    h
}

#[test]
fn the_kerning_icon_opens_the_kerning_view() {
    let mut h = kerning_view_open();
    let flyout = font_flyout(&h);
    let kern = h.button("Kerning");
    assert_eq!(kern.min, flyout.min + vec2(275.0, 9.0));
    assert_eq!(h.drawn("icon.fontforge.kern").len(), 1);
    h.click(kern.center(), PointerButton::Primary);
    assert!(h.text_rects.iter().any(|(t, _, _)| t == "Pairs"));
    assert!(h.button("Kern lo").width() > 0.0);
    assert!(!h.buttons.iter().any(|(l, _)| l == "Glyph A"));
}

#[test]
fn clicking_a_letter_picks_its_pair_and_dragging_it_kerns_in_one_step() {
    let mut h = kerning_view_open();
    h.click(h.button("Kerning").center(), PointerButton::Primary);
    let letter = h.button("Kern lo");
    h.click(letter.center(), PointerButton::Primary);
    assert_eq!(h.app.font_panel.kern_pair, Some(('l', 'o')));
    // Cuatro pixeles de la fuente a la izquierda (a ×2, ocho puntos: más que lo que egui
    // todavía cuenta como clic).
    let from = h.button("Kern lo").center();
    h.drag(from, from - vec2(8.0, 0.0));
    let e = &mut h.app.doc.editor;
    assert_eq!(e.font().unwrap().kerning.value('l', 'o'), -4);
    e.undo();
    assert!(
        e.font().unwrap().kerning.is_empty(),
        "the drag was one step"
    );
    h.frame();
    // Agregar un par desde la fila de abajo.
    h.app.font_panel.kern_first = "o".into();
    h.app.font_panel.kern_second = "l".into();
    h.frame();
    h.click(h.button("Add").center(), PointerButton::Primary);
    let e = &h.app.doc.editor;
    let k = &e.font().unwrap().kerning;
    assert_eq!(k.pairs.len(), 1);
    assert_eq!(h.app.font_panel.kern_pair, Some(('o', 'l')));
    // Una clase nueva.
    h.click(h.button("New class").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.font().unwrap().kerning.classes.len(), 1);
}

#[test]
fn the_arrows_kern_the_picked_pair_one_step_per_press() {
    let mut h = kerning_view_open();
    let value = |h: &LayoutHarness| h.app.doc.editor.font().unwrap().kerning.value('l', 'o');
    h.click(h.button("Kerning").center(), PointerButton::Primary);
    // Sin par elegido, las flechas hacen lo de antes (el frame de al lado: otro glifo).
    let frame = h.app.doc.editor.frame;
    press(&mut h, egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert_ne!(h.app.doc.editor.frame, frame, "no pair: the next frame");
    assert_eq!(value(&h), 0);
    h.app.doc.editor.frame = frame;
    h.frame();
    h.click(h.button("Kern lo").center(), PointerButton::Primary);
    assert_eq!(h.app.font_panel.kern_pair, Some(('l', 'o')));
    press(&mut h, egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert_eq!(value(&h), 1);
    assert_eq!(h.app.doc.editor.frame, frame, "not the next frame");
    press(&mut h, egui::Key::ArrowLeft, egui::Modifiers::SHIFT);
    assert_eq!(value(&h), -4, "Shift: 5 px");
    press(&mut h, egui::Key::ArrowLeft, egui::Modifiers::NONE);
    assert_eq!(value(&h), -5);
    h.app.doc.editor.undo();
    assert_eq!(value(&h), -4, "each press is one step");
    h.app.doc.editor.undo();
    assert_eq!(value(&h), 1);
    h.frame();
    // Con el foco en un campo de texto, las flechas son del campo.
    let field = h
        .nodes
        .iter()
        .find(|(l, _)| l == "Test text")
        .map(|(_, r)| *r)
        .expect("the Test text field");
    h.click(field.center(), PointerButton::Primary);
    press(&mut h, egui::Key::ArrowLeft, egui::Modifiers::NONE);
    assert_eq!(value(&h), 1, "typing: the field gets the arrows");
}

#[test]
fn the_font_settings_show_the_family_and_new_variant_creates_the_bold() {
    let mut h = kerning_view_open();
    h.click(h.button("Font Settings").center(), PointerButton::Primary);
    assert!(h.text_rects.iter().any(|(t, _, _)| t == "Family \"Mine\""));
    assert!(h.text_rects.iter().any(|(t, _, _)| t == "Weight"));
    h.click(h.button("New Variant…").center(), PointerButton::Primary);
    assert!(h.app.new_variant.is_some());
    // Lo que hace Create (el diálogo no se dibuja en este arnés: ver dialog_tests).
    h.app.new_variant = None;
    let recipe = cs_core::VariantRecipe::new(cs_core::VariantKind::Bold);
    let style = cs_core::FontStyle {
        name: "Bold".into(),
        weight: 700,
        ..Default::default()
    };
    assert!(h.app.doc.editor.add_variant(&recipe, style));
    h.frame();
    let e = &h.app.doc.editor;
    assert_eq!(e.artboard().name, "Mine Bold");
    assert_eq!(e.workspace(), Some(e.active), "in its workspace");
    let bold = e.active;
    let family: Vec<String> = e.family_fonts().into_iter().map(|(_, s)| s.name).collect();
    assert_eq!(family, vec!["Regular", "Bold"]);
    for _ in 0..3 {
        h.frame();
    }
    // En los ajustes de la Bold: Rebuild en ella y un clic en Regular abre la otra.
    assert!(!h.disabled.iter().any(|l| l == "Rebuild"));
    h.click(h.button("Rebuild").center(), PointerButton::Primary);
    assert_eq!(
        h.app.doc.editor.history.done().last(),
        Some("Rebuild Variant")
    );
    let regular = h.nodes.iter().find(|(l, _)| l == "Regular").unwrap().1;
    h.click(regular.center(), PointerButton::Primary);
    let e = &h.app.doc.editor;
    assert_ne!(e.active, bold);
    assert_eq!(e.artboard().name, "Mine");
}
