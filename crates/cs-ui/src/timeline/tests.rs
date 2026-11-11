use cs_core::{ColorMode, Sprite};
use eframe::egui::{Event, Pos2, RawInput};

use super::*;
use crate::ChimpApp;
use crate::dock::Edge;
use crate::hosted_panel;
use crate::panel_registry::PanelId;
use crate::view::ViewSettings;

/// Los íconos de los botones de la barra, de 21×11.
const TOOLBAR_ICONS: [&str; 12] = [
    "timeline.icon.first",
    "timeline.icon.prev",
    "timeline.icon.play",
    "timeline.icon.next",
    "timeline.icon.last",
    "timeline.icon.frame_add",
    "timeline.icon.frame_remove",
    "timeline.icon.layer_add",
    "timeline.icon.layer_remove",
    "timeline.icon.group_add",
    "timeline.icon.tilemap_add",
    "timeline.icon.onion",
];

/// La línea de tiempo como en la app: una `ChimpApp` con una pestaña de 8×8 y el timeline solo
/// debajo del lienzo (`Workspace::solo`), dibujado por el workspace (con `timeline_frame`, el
/// marco del panel de abajo de antes), manejada con eventos crudos (50 ms por frame). Todo lo que
/// guarda y lo que se le pasa (rects, puntos de clics y arrastres) es relativo a la hoja del
/// timeline (su esquina de arriba a la izquierda es (0, 0)); las filas de `TimelineState::rows`
/// son de la ventana ([`Self::local`]).
struct Harness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    width: f32,
    /// El alto de la ventana.
    height: f32,
    /// La hoja del timeline en el último frame, en la ventana.
    leaf: Rect,
    /// Los nodos con nombre del último frame: los del timeline y sus menús
    /// (`hosted_panel::panel_nodes`).
    nodes: Vec<(String, Rect)>,
    /// Posición del texto "Layers" del encabezado de la grilla.
    grid: Pos2,
    /// Las texturas dibujadas en el último frame, con dónde.
    images: Vec<(egui::TextureId, Rect)>,
    /// Los colores de las mallas lisas (sin textura) del último frame.
    flat_colors: Vec<Color32>,
    /// Los textos dibujados en el último frame.
    texts: Vec<String>,
    /// Los rects lisos del último frame, en orden de dibujo, ya recortados.
    flat_rects: Vec<(Rect, Color32)>,
}

impl Harness {
    fn new(width: f32) -> Self {
        Self::with(width, false)
    }

    /// Con Pixel UI: el skin embebido (el de la app) y el marco del panel de abajo de antes.
    fn pixel(width: f32) -> Self {
        Self::with(width, true)
    }

    /// La ventana mide `width` × 400 y el timeline va a lo ancho, debajo del lienzo, de 190 de
    /// alto (`view.timeline_height`).
    fn with(width: f32, pixel: bool) -> Self {
        Self::sized(width, pixel, 400.0, 190.0)
    }

    /// La ventana mide `width` × `height` y el timeline, `timeline` de alto.
    fn sized(width: f32, pixel: bool, height: f32, timeline: f32) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let view = ViewSettings {
            pixel_ui: pixel,
            timeline_height: Some(timeline),
            ..Default::default()
        };
        let editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let app = hosted_panel::app(view, editor, PanelId::Timeline, Edge::Bottom);
        let mut h = Self {
            ctx,
            app,
            time: 0.0,
            width,
            height,
            leaf: Rect::ZERO,
            nodes: Vec::new(),
            grid: Pos2::ZERO,
            images: Vec::new(),
            flat_colors: Vec::new(),
            texts: Vec::new(),
            flat_rects: Vec::new(),
        };
        for _ in 0..3 {
            h.frame(vec![]);
        }
        assert!(h.app.workspace.timeline_below(), "below the canvas");
        h
    }

    fn editor(&mut self) -> &mut Editor {
        &mut self.app.doc.editor
    }

    /// El estado del timeline: el de la pestaña, el que usa la app.
    fn state(&mut self) -> &mut TimelineState {
        &mut self.app.doc.timeline
    }

    /// Con Pixel UI: el skin de la app.
    fn skin(&self) -> &Skin {
        self.app.pixel.skin().expect("pixel harness")
    }

    /// El ancho de lo que se ve del timeline en su hoja: debajo del lienzo, la tira de la
    /// derecha baja a su lado y le quita su ancho (`dock::strip_room`; en el panel de abajo de
    /// antes se veía la ventana entera).
    fn visible_width(&self) -> f32 {
        let pixel = self.app.pixel.skin().is_some();
        self.leaf.width() - crate::dock::strip_room(&self.ctx.global_style(), pixel)
    }

    /// La fila de la capa `layer` en el último frame (`TimelineState::rows`), relativa a la
    /// hoja.
    fn row(&self, layer: LayerId) -> Rect {
        let rows = &self.app.doc.timeline.rows;
        let row = rows.iter().find(|(l, _)| *l == layer).expect("the row").1;
        self.local(row)
    }

    /// `r` (de la ventana) relativo a la hoja.
    fn local(&self, r: Rect) -> Rect {
        r.translate(-self.leaf.min.to_vec2())
    }

    /// Un frame con `events`, cuyas posiciones son relativas a la hoja.
    fn frame(&mut self, events: Vec<Event>) {
        let events = hosted_panel::to_window(events, self.leaf);
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                vec2(self.width, self.height),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let output = hosted_panel::frame(&self.ctx, &mut self.app, input);
        let leaf = hosted_panel::leaf(&self.app, PanelId::Timeline);
        self.leaf = leaf;
        let back = -leaf.min.to_vec2();
        self.images.clear();
        self.flat_colors.clear();
        self.texts.clear();
        self.flat_rects.clear();
        // Lo del timeline y sus menús; no las barras de la ventana ni el lienzo.
        let shapes = output
            .shapes
            .iter()
            .filter(|c| hosted_panel::clipped_to(c.clip_rect, leaf));
        for clipped in shapes.clone() {
            collect_flat_rects(&clipped.shape, clipped.clip_rect, &mut self.flat_rects);
            collect_images(&clipped.shape, &mut self.images);
            collect_flat_colors(&clipped.shape, &mut self.flat_colors);
            collect_texts(&clipped.shape, &mut self.texts);
        }
        for (r, _) in &mut self.flat_rects {
            *r = r.translate(back);
        }
        for (_, r) in &mut self.images {
            *r = r.translate(back);
        }
        self.nodes = hosted_panel::panel_nodes(&self.ctx, &output, leaf)
            .into_iter()
            .map(|(t, r, _)| (t, r))
            .collect();
        for clipped in shapes {
            if let egui::Shape::Text(t) = &clipped.shape
                && t.galley.text() == "Layers"
            {
                self.grid = t.pos + back;
            }
        }
    }

    fn rect(&self, text: &str) -> Rect {
        self.nodes
            .iter()
            .find(|(t, _)| t == text)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {text:?} in {:?}", self.nodes))
    }

    /// Rect del campo de duración (el último control de la fila).
    fn duration(&self) -> Rect {
        self.nodes
            .iter()
            .filter(|(t, _)| t.contains("ms"))
            .map(|(_, r)| *r)
            .next_back()
            .unwrap_or_else(|| panic!("no duration in {:?}", self.nodes))
    }

    fn wheel(&mut self, dy: f32) {
        let over = pos2(100.0, self.rect("+ Frame").center().y);
        self.frame(vec![Event::PointerMoved(over)]);
        self.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..20 {
            self.frame(vec![]);
        }
    }

    /// Un clic en `at`, con `modifiers`.
    fn click_at_with(&mut self, at: Pos2, modifiers: egui::Modifiers) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        // egui toma las teclas apretadas de `ModifiersChanged`, no del evento del clic.
        self.frame(vec![
            Event::ModifiersChanged(modifiers),
            Event::PointerMoved(at),
        ]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![Event::ModifiersChanged(Default::default())]);
    }

    /// Un clic derecho en `at`: abre el menú contextual de lo que haya ahí.
    fn right_click_at(&mut self, at: Pos2) {
        let button = |pressed| Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(at)]);
        self.frame(vec![button(true)]);
        self.frame(vec![button(false)]);
        self.frame(vec![]);
    }

    /// Centro de la celda del frame `f` en el encabezado de la grilla clásica.
    fn header_cell(&self, f: usize) -> Pos2 {
        let row_left = self.grid.x - 6.0;
        let cell = Grid::CLASSIC.left_w + Grid::CLASSIC.cell_w * (f as f32 + 0.5);
        pos2(row_left + cell, self.grid.y + 6.0)
    }

    /// Un punto sobre el nombre de la fila `row` (0 = la de más arriba) de la grilla clásica.
    fn layer_name(&self, row: usize) -> Pos2 {
        let header = self.header_cell(0).y + Grid::CLASSIC.header_h / 2.0;
        let y = header + Grid::CLASSIC.row_h * (row as f32 + 0.5);
        pos2(self.grid.x - 6.0 + 100.0, y)
    }

    /// Un arrastre con el botón izquierdo de `from` a `to`, de a poco, con `modifiers`.
    fn drag_with(&mut self, from: Pos2, to: Pos2, modifiers: egui::Modifiers) {
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![button(from, true)]);
        for i in 1..=8 {
            self.frame(vec![Event::PointerMoved(from.lerp(to, i as f32 / 8.0))]);
        }
        self.frame(vec![Event::ModifiersChanged(modifiers), button(to, false)]);
        self.frame(vec![Event::ModifiersChanged(Default::default())]);
    }

    /// El centro del cel de la fila `row` y el frame `f` de la grilla clásica.
    fn cel(&self, row: usize, f: usize) -> Pos2 {
        pos2(self.header_cell(f).x, self.layer_name(row).y)
    }

    fn click_at(&mut self, at: Pos2) {
        self.click_at_with(at, Default::default());
    }

    /// Un clic en el centro del widget con nombre `text`.
    fn click(&mut self, text: &str) {
        let at = self.rect(text).center();
        self.click_at(at);
    }

    /// Dónde se dibujó la pieza `name` del skin en el último frame.
    fn drawn(&self, name: &str) -> Vec<Rect> {
        let id = self
            .skin()
            .piece(name, crate::skin::WidgetState::Normal)
            .unwrap_or_else(|| panic!("{name} in the skin"))
            .texture
            .id();
        self.images
            .iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| *r)
            .collect()
    }

    /// Cuántas veces se dibujó la pieza `name` (una pieza con centro son varias partes; acá
    /// se cuentan las que no tienen centro).
    fn count(&self, name: &str) -> usize {
        self.drawn(name).len()
    }

    fn layer(&self, i: usize) -> &cs_core::Layer {
        &self.app.doc.editor.sprite().layers[i]
    }

    /// El color liso que quedó en el píxel `(x, y)` (el último rect que lo cubre); las piezas,
    /// los textos y lo transparente no cuentan.
    fn color_at(&self, x: f32, y: f32) -> Option<Color32> {
        let p = pos2(x + 0.5, y + 0.5);
        self.flat_rects
            .iter()
            .rev()
            .find(|(r, c)| c.a() > 0 && r.contains(p))
            .map(|(_, c)| *c)
    }
}

/// Los rects lisos de una forma (cada cuadro de 4 vértices de una malla sin textura), recortados
/// a `clip`.
fn collect_flat_rects(shape: &egui::Shape, clip: Rect, out: &mut Vec<(Rect, Color32)>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_flat_rects(s, clip, out);
            }
        }
        egui::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::default() => {
            for quad in mesh.vertices.as_chunks::<4>().0 {
                let r = Rect::from_points(&quad.iter().map(|v| v.pos).collect::<Vec<_>>());
                out.push((r.intersect(clip), quad[0].color));
            }
        }
        egui::Shape::Rect(r) => out.push((r.rect.intersect(clip), r.fill)),
        _ => {}
    }
}

/// Los colores de las mallas lisas (los rects de las líneas y de lo elegido).
fn collect_flat_colors(shape: &egui::Shape, out: &mut Vec<Color32>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_flat_colors(s, out);
            }
        }
        egui::Shape::Mesh(mesh) if mesh.texture_id == egui::TextureId::default() => {
            out.extend(mesh.vertices.iter().map(|v| v.color));
        }
        egui::Shape::Rect(r) => out.push(r.fill),
        _ => {}
    }
}

/// Los textos de una forma.
fn collect_texts(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_texts(s, out);
            }
        }
        egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
        _ => {}
    }
}

/// Las imágenes (mallas con textura) de una forma, con su rect.
fn collect_images(shape: &egui::Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_images(s, out);
            }
        }
        egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() => {
            out.push((mesh.texture_id, mesh.calc_bounds()));
        }
        _ => {}
    }
}

#[test]
fn with_pixel_ui_the_toolbar_draws_every_group_and_field_of_the_design() {
    let h = Harness::pixel(1200.0);
    for icon in TOOLBAR_ICONS {
        assert_eq!(h.count(icon), 1, "{icon}");
    }
    // Los cuatro grupos de botones son grupos del UI kit: 12 botones con ícono (las piezas
    // viejas del timeline ya no se usan).
    assert!(h.count("button") >= 12);
    for old in ["timeline.group", "timeline.button", "timeline.button.sep"] {
        assert_eq!(h.count(old), 0, "{old}");
    }
    for name in [
        "timeline.onion_range",
        "timeline.icon.clock",
        "field.text",
        "timeline.divider",
        "workspace.box",
    ] {
        assert!(h.count(name) >= 1, "{name}");
    }
    // − y + de la opacidad y de la duración: los botones del stepper del UI kit.
    assert_eq!(h.count("stepper.left"), 2);
    assert_eq!(h.count("stepper.right"), 2);
    for label in [
        "First frame",
        "Previous frame",
        "Play",
        "Next frame",
        "Last frame",
        "New frame",
        "Delete frame",
        "New layer",
        "Delete layer",
        "New group",
        "New tilemap layer",
        "Onion skin",
        "Opacity",
        "Less opacity",
        "More opacity",
        "Normal",
        "Shorter frame",
        "Longer frame",
    ] {
        h.rect(label);
    }
    // Todo en una fila, como en el diseño: los grupos arriba a 3 del interior de la caja.
    let first = h.rect("First frame");
    let longer = h.rect("Longer frame");
    assert_eq!(first.top(), longer.top(), "one row");
    let divider = h.drawn("timeline.divider")[0];
    assert_eq!(
        divider.top() - first.top(),
        18.0,
        "group top 562, divider 580"
    );
}

#[test]
fn with_pixel_ui_play_new_frame_new_layer_and_onion_work() {
    let mut h = Harness::pixel(1200.0);
    h.click("Play");
    assert!(h.editor().playing);
    assert!(
        h.rect("Pause").is_positive(),
        "the button says what it does now"
    );
    h.click("Pause");
    assert!(!h.editor().playing);

    h.click("New frame");
    assert_eq!(h.editor().frame_count(), 2);
    h.click("Delete frame");
    assert_eq!(h.editor().frame_count(), 1);

    let layers = h.editor().sprite().layers.len();
    h.click("New layer");
    assert_eq!(h.editor().sprite().layers.len(), layers + 1);
    h.click("New group");
    assert_eq!(h.editor().sprite().layers.len(), layers + 2);
    h.click("New tilemap layer");
    // La app toma el pedido del timeline (`TimelineState::new_tilemap`) en el mismo frame y
    // abre su diálogo; se cierra para seguir (con él, modal, el timeline queda apagado).
    assert!(h.app.new_tilemap.is_some(), "the app opened the dialog");
    h.app.new_tilemap = None;

    assert!(!h.editor().onion_enabled);
    assert_eq!(h.count("timeline.icon.onion"), 1, "off: the cut onion");
    h.click("Onion skin");
    assert!(h.editor().onion_enabled);
    assert_eq!(h.count("timeline.icon.onion.on"), 1, "on: the whole onion");
    assert_eq!(h.count("timeline.icon.onion"), 0);
}

#[test]
fn with_pixel_ui_the_playback_buttons_move_through_the_frames() {
    let mut h = Harness::pixel(1200.0);
    for _ in 0..3 {
        h.editor().duplicate_frame();
    }
    h.click("First frame");
    assert_eq!(h.editor().frame, 0);
    h.click("Next frame");
    assert_eq!(h.editor().frame, 1);
    h.click("Last frame");
    assert_eq!(h.editor().frame, 3);
    h.click("Previous frame");
    assert_eq!(h.editor().frame, 2);
}

#[test]
fn with_pixel_ui_opacity_minus_and_plus_change_the_layer_opacity() {
    let mut h = Harness::pixel(1200.0);
    let layer = h.editor().layer;
    let opacity = |h: &Harness| h.app.doc.editor.sprite().layer(layer).unwrap().opacity;
    assert_eq!(opacity(&h), 255);
    h.click("Less opacity");
    assert_eq!(opacity(&h), 254);
    h.click("Less opacity");
    assert_eq!(opacity(&h), 253);
    h.click("More opacity");
    assert_eq!(opacity(&h), 254);
    // Con Shift, de a 10.
    let at = h.rect("Less opacity").center();
    h.click_at_with(at, egui::Modifiers::SHIFT);
    assert_eq!(opacity(&h), 244);
    // Lo muestra el campo.
    h.rect("244");
}

#[test]
fn with_pixel_ui_duration_minus_and_plus_change_the_frame_duration() {
    let mut h = Harness::pixel(1200.0);
    let ms = |h: &Harness| h.app.doc.editor.sprite().frames[h.app.doc.editor.frame].duration_ms;
    assert_eq!(ms(&h), 100);
    // El número se arrastra o se escribe; "ms" va dibujado al lado.
    h.rect("100");
    h.click("Longer frame");
    assert_eq!(ms(&h), 110);
    h.click("Shorter frame");
    h.click("Shorter frame");
    assert_eq!(ms(&h), 90);
    h.rect("90");
}

#[test]
fn with_pixel_ui_the_blend_combo_picks_a_mode() {
    let mut h = Harness::pixel(1200.0);
    let layer = h.editor().layer;
    assert!(h.count("field.text") >= 1, "the select of the UI kit");
    h.click("Normal");
    h.click("Multiply");
    assert_eq!(
        h.editor().sprite().layer(layer).unwrap().blend,
        BlendMode::Multiply
    );
    h.rect("Multiply");
}

#[test]
fn with_pixel_ui_the_onion_range_shows_the_frames_before_and_after() {
    let mut h = Harness::pixel(1200.0);
    h.editor().onion.before = 2;
    h.editor().onion.after = 5;
    h.frame(vec![]);
    let range = h.drawn("timeline.onion_range")[0];
    let (before, after) = (h.rect("2"), h.rect("5"));
    let mid = range.left() + RANGE_MID_FOR_TESTS;
    assert!(range.x_range().contains(before.center().x) && before.center().x < mid);
    assert!(range.x_range().contains(after.center().x) && after.center().x > mid);
}

/// La línea del medio del rango del onion skin, en la columna 29 de su pieza.
const RANGE_MID_FOR_TESTS: f32 = 29.0;

#[test]
fn with_pixel_ui_the_table_shows_eye_lock_and_cel_pieces_by_state() {
    let mut h = Harness::pixel(1200.0);
    // La capa de abajo con un cel; arriba, una nueva sin cel, oculta y bloqueada.
    let layer = h.layer(0).id;
    h.editor().execute(cs_commands::Command::DrawPixels {
        layer,
        frame: 0,
        pixels: vec![(0, 0, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(9, 9, 9)))],
    });
    h.editor().add_layer(LayerKind::Image);
    let top = h.editor().layer;
    h.editor().toggle_visible(top);
    h.editor().toggle_locked(top);
    h.frame(vec![]);
    // El encabezado tiene el ojo abierto y el candado cerrado de las columnas. La capa de
    // arriba es la activa: sus íconos van en la versión para lo elegido.
    assert_eq!(h.count("timeline.eye"), 2, "header + the visible layer");
    assert_eq!(
        h.count("timeline.eye.closed.selected"),
        1,
        "the hidden layer"
    );
    assert_eq!(h.count("timeline.eye.closed"), 0);
    assert_eq!(h.count("palette.icon.lock"), 1, "the header");
    assert_eq!(h.count("palette.icon.lock.selected"), 1, "the locked layer");
    assert_eq!(h.count("palette.icon.lock.open"), 1, "the unlocked layer");
    assert_eq!(h.count("timeline.cel"), 1);
    assert_eq!(
        h.count("timeline.cel.empty.selected"),
        1,
        "the active layer's cel"
    );
    // Las filas miden 13 y los íconos van donde en el diseño.
    let eyes = h.drawn("timeline.eye");
    let closed = h.drawn("timeline.eye.closed.selected")[0];
    let header_eye = eyes.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
    assert_eq!(closed.top() - header_eye, 13.0, "the first layer row");
    let cel = h.drawn("timeline.cel")[0];
    let empty = h.drawn("timeline.cel.empty.selected")[0];
    assert_eq!(
        cel.top() - empty.top(),
        13.0,
        "the cel layer is the second row"
    );
    assert_eq!(cel.left(), empty.left());
}

#[test]
fn with_pixel_ui_the_active_layer_and_frame_are_pink_like_the_design() {
    let h = Harness::pixel(1200.0);
    let colors = *h.skin().colors();
    let drawn = &h.flat_colors;
    for (color, what) in [
        (colors.timeline_selection, "the fill"),
        (colors.timeline_selection_dark, "the dark lines"),
        (colors.timeline_selection_light, "the light lines"),
    ] {
        assert!(drawn.contains(&color), "{what}");
    }
    assert!(
        !drawn.contains(&colors.selection),
        "not the old purple selection"
    );
    // El ojo y el candado de la capa activa, en su versión para lo elegido.
    assert_eq!(h.count("timeline.eye.selected"), 1);
    assert_eq!(h.count("palette.icon.lock.open.selected"), 1);
}

#[test]
fn with_pixel_ui_groups_show_the_open_or_closed_folder() {
    let mut h = Harness::pixel(1200.0);
    h.editor().add_layer(LayerKind::Group);
    let group = h.editor().layer;
    h.frame(vec![]);
    // La capa activa: la carpetita en su versión para lo elegido; abierta.
    assert_eq!(h.count("timeline.folder.open.selected"), 1);
    assert_eq!(h.count("timeline.folder.selected"), 0);
    h.editor().toggle_collapsed(group);
    h.frame(vec![]);
    assert_eq!(h.count("timeline.folder.selected"), 1, "collapsed: closed");
    // Otra capa activa: la carpetita de siempre.
    h.editor().add_layer(LayerKind::Image);
    h.frame(vec![]);
    assert_eq!(h.count("timeline.folder"), 1);
}

#[test]
fn with_pixel_ui_clicking_the_eye_and_the_lock_toggles_them() {
    let mut h = Harness::pixel(1200.0);
    let id = h.editor().layer;
    let name = h.editor().sprite().layer(id).unwrap().name.clone();
    let layer = |h: &Harness| h.app.doc.editor.sprite().layer(id).unwrap().clone();
    assert!(layer(&h).visible && !layer(&h).locked);
    h.click(&format!("Visible: {name}"));
    assert!(!layer(&h).visible);
    assert_eq!(
        h.count("timeline.eye.closed.selected"),
        1,
        "the active layer"
    );
    h.click(&format!("Visible: {name}"));
    assert!(layer(&h).visible);
    h.click(&format!("Locked: {name}"));
    assert!(layer(&h).locked);
    h.click(&format!("Locked: {name}"));
    assert!(!layer(&h).locked);
}

#[test]
fn with_pixel_ui_a_narrow_timeline_wraps_the_toolbar_and_nothing_overflows() {
    let mut h = Harness::pixel(420.0);
    let (first, longer) = (h.rect("First frame"), h.rect("Longer frame"));
    assert!(longer.top() > first.top(), "the duration went down a row");
    // Lo que se ve del timeline: su hoja (420, la ventana) sin la tira, que baja a su derecha.
    let seen = h.visible_width();
    assert_eq!(seen, 420.0 - 26.0);
    for label in ["Onion skin", "Less opacity", "Normal", "Longer frame"] {
        let r = h.rect(label);
        assert!(r.left() >= 0.0 && r.right() <= seen, "{label}: {r:?}");
    }
    // Y todavía se usa.
    h.click("Longer frame");
    assert_eq!(h.editor().sprite().frames[0].duration_ms, 110);
}

#[test]
fn without_pixel_ui_the_timeline_does_not_use_the_skin() {
    let h = Harness::new(1200.0);
    let skin = Skin::embedded(&h.ctx);
    let ours: Vec<egui::TextureId> = [
        "timeline.group",
        "timeline.button",
        "timeline.divider",
        "timeline.eye",
        "timeline.cel",
        "workspace.box",
        "palette.icon.lock",
    ]
    .iter()
    .map(|n| {
        skin.piece(n, crate::skin::WidgetState::Normal)
            .unwrap()
            .texture
            .id()
    })
    .collect();
    assert!(h.images.iter().all(|(t, _)| !ours.contains(t)));
    // Los controles de siempre, con sus textos.
    for label in ["+ Frame", "+ Layer", "+ Group", "+ Tilemap", "Onion"] {
        h.rect(label);
    }
    assert_ne!(h.grid, Pos2::ZERO, "the \"Layers\" header");
}

#[test]
fn the_last_control_can_be_scrolled_into_view_in_a_narrow_window() {
    let mut h = Harness::new(1000.0);
    // Lo que se ve: la hoja (1000, la ventana) sin la tira de la derecha.
    let seen = h.visible_width();
    assert_eq!(seen, 1000.0 - 25.0);
    assert!(h.duration().right() > seen, "it starts clipped");
    for _ in 0..6 {
        h.wheel(-300.0);
    }
    let d = h.duration();
    assert!(d.right() <= seen && d.left() >= 0.0, "{d:?}");
}

#[test]
fn the_controls_stay_on_one_line_and_the_grid_does_not_move() {
    let wide = Harness::new(2000.0);
    let mut h = Harness::new(1000.0);
    let (frame_y, grid) = (h.rect("+ Frame").center().y, h.grid);
    assert_eq!(
        grid, wide.grid,
        "same grid position as when everything fits"
    );
    assert_eq!(frame_y, wide.rect("+ Frame").center().y);
    for _ in 0..3 {
        h.wheel(-300.0);
    }
    assert_eq!(h.rect("+ Frame").center().y, frame_y, "one line");
    assert_eq!(h.rect("+ Layer").center().y, frame_y, "one line");
    assert_eq!(h.duration().center().y, frame_y, "one line");
    assert_eq!(h.grid, grid, "the grid did not move");
}
#[test]
fn deleting_a_frame_from_its_header_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor().duplicate_frame();
    h.editor().duplicate_frame();
    h.frame(vec![]);
    assert_eq!(h.editor().frame_count(), 3);
    h.right_click_at(h.header_cell(1));
    // Con la grilla clásica, "Delete frame" es solo el del menú (la barra dice "− Frame").
    h.click("Delete frame");
    h.frame(vec![]);
    assert_eq!(h.editor().frame_count(), 2);
}

#[test]
fn deleting_the_last_frame_from_its_header_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor().duplicate_frame();
    h.editor().duplicate_frame();
    h.frame(vec![]);
    h.right_click_at(h.header_cell(2));
    h.click("Delete frame");
    h.frame(vec![]);
    assert_eq!(h.editor().frame_count(), 2);
    assert_eq!(h.editor().frame, 1);
}

#[test]
fn dragging_over_cels_selects_layers_by_frames_and_dragging_the_range_moves_it() {
    let mut h = Harness::new(1200.0);
    h.editor().add_layer(LayerKind::Image);
    for _ in 0..3 {
        h.editor().insert_empty_frame();
    }
    let order = h.editor().timeline_layers();
    let (top, bottom) = (order[0], order[1]);
    let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    for (layer, frame) in [(top, 0), (bottom, 1)] {
        h.editor().execute(cs_commands::Command::DrawPixels {
            layer,
            frame,
            pixels: vec![(0, 0, red)],
        });
    }
    h.frame(vec![]);
    // De la capa de arriba, frame 1, a la de abajo, frame 2: 2 capas × 2 frames.
    h.drag_with(h.cel(0, 0), h.cel(1, 1), Default::default());
    assert_eq!(h.editor().range_layers(), vec![top, bottom]);
    assert_eq!(h.editor().selected_frames(), (0, 1));
    assert_eq!(h.editor().selected_cels().len(), 4);
    // Arrastrándolo dos frames a la derecha, los cels se mueven y el rango con ellos.
    h.drag_with(h.cel(0, 0), h.cel(0, 2), Default::default());
    let sprite = h.editor().sprite();
    assert!(sprite.cel(top, 0).is_none() && sprite.cel(bottom, 1).is_none());
    assert!(sprite.cel(top, 2).is_some() && sprite.cel(bottom, 3).is_some());
    assert_eq!(h.editor().selected_frames(), (2, 3));
    // Con Ctrl, se copia.
    let ctrl = egui::Modifiers::COMMAND;
    h.drag_with(h.cel(0, 2), h.cel(0, 0), ctrl);
    let sprite = h.editor().sprite();
    assert!(
        sprite.cel(top, 0).is_some() && sprite.cel(top, 2).is_some(),
        "copied"
    );
    // Un clic suelto deja un solo cel.
    h.click_at(h.cel(1, 3));
    assert!(!h.editor().has_cel_range());
}

#[test]
fn overlapping_tags_go_on_different_floors() {
    let tag = |from, to| Tag {
        name: "t".into(),
        from,
        to,
        direction: AnimDirection::Forward,
        color: cs_core::Rgba::rgb(0, 0, 0),
        repeat: 0,
    };
    // 0–3 y 5–6 caben en el mismo piso; 1–2 se encima con el primero; 3–5, con 0–3 y 5–6
    // pero no con 1–2: va en su piso.
    let tags = [tag(0, 3), tag(1, 2), tag(5, 6), tag(3, 5), tag(2, 4)];
    assert_eq!(tag_levels(&tags), vec![0, 1, 0, 1, 2]);
    assert!(tag_levels(&[]).is_empty());
}

#[test]
fn with_pixel_ui_the_layer_mask_is_the_face_of_the_design() {
    let mut h = Harness::pixel(1200.0);
    h.frame(vec![]);
    // La capa activa, de imagen y sin máscara: el botón para agregarle una, en los colores de lo
    // elegido.
    assert!(h.drawn("timeline.mask").is_empty());
    assert_eq!(h.drawn("timeline.mask.add.selected").len(), 1);
    assert!(h.drawn("timeline.mask.add").is_empty());
    h.click("Add Layer Mask");
    h.frame(vec![]);
    let layer = h.editor().layer;
    assert!(
        h.editor().sprite().layer(layer).unwrap().mask.is_some(),
        "added"
    );
    assert!(h.drawn("timeline.mask.add.selected").is_empty());
    // La cara; editándola, la del borde rosa. En la capa activa, las de los colores de lo
    // elegido.
    h.editor().set_edit_mask(false);
    h.frame(vec![]);
    let face = h.drawn("timeline.mask.selected");
    assert_eq!(face.len(), 1);
    assert_eq!(face[0].size(), Vec2::new(9.0, 9.0));
    assert!(h.drawn("timeline.mask").is_empty());
    h.editor().set_edit_mask(true);
    h.frame(vec![]);
    assert_eq!(h.drawn("timeline.mask.editing.selected").len(), 1);
    assert!(h.drawn("timeline.mask.selected").is_empty());
}

#[test]
fn with_pixel_ui_the_layer_menu_is_the_menus_panel() {
    // Con Pixel UI de verdad: la app pone el skin de los menús (`pixel_ui::set_bubble`) en cada
    // frame según `view.pixel_ui`, así que ya no se puede ponerlo a mano sobre la grilla clásica.
    let mut h = Harness::pixel(1200.0);
    let panel = h
        .skin()
        .piece("menu.list", crate::skin::WidgetState::Normal)
        .expect("menu.list")
        .texture
        .id();
    h.editor().add_layer(LayerKind::Image);
    h.frame(vec![]);
    let layers = h.editor().sprite().layers.len();
    // Sobre el nombre de la fila de arriba de la grilla del diseño (después del ojo y el candado).
    let top = h.editor().timeline_layers()[0];
    let row = h.row(top);
    let name = pos2(
        row.left() + pixel::EYE_W + pixel::LOCK_W + 20.0,
        row.center().y,
    );
    h.right_click_at(name);
    assert!(
        h.images.iter().any(|(t, _)| *t == panel),
        "the panel of the skin"
    );
    h.click("Delete");
    h.frame(vec![]);
    assert_eq!(h.editor().sprite().layers.len(), layers - 1);
}

#[test]
fn deleting_a_layer_from_its_row_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor().add_layer(LayerKind::Image);
    h.editor().add_layer(LayerKind::Image);
    h.frame(vec![]);
    let layers = h.editor().sprite().layers.len();
    assert!(layers >= 3);
    let top = h.editor().timeline_layers()[0];
    h.right_click_at(h.layer_name(0));
    h.click("Delete");
    h.frame(vec![]);
    assert_eq!(h.editor().sprite().layers.len(), layers - 1);
    assert!(
        h.editor().sprite().layer(top).is_none(),
        "deleted the row's layer"
    );
}

#[test]
fn deleting_a_tag_from_its_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor().duplicate_frame();
    h.editor().add_tag();
    h.editor().add_tag();
    h.frame(vec![]);
    assert_eq!(h.editor().sprite().tags.len(), 2);
    // Los dos tags van sobre el frame actual (el 2).
    let tag = pos2(h.header_cell(1).x, h.grid.y - Grid::CLASSIC.tag_h / 2.0);
    h.right_click_at(tag);
    // El menú (alto, cerca del borde de abajo) se acomoda en el frame siguiente.
    h.frame(vec![]);
    h.click("Delete tag");
    h.frame(vec![]);
    assert_eq!(h.editor().sprite().tags.len(), 1);
}

/// Una capa de objetos ("Scene") con dos objetos: "Alpha" (el primero) y "Beta" (arriba), en un
/// sprite de dos frames. Devuelve la capa y los dos objetos.
fn with_two_objects(h: &mut Harness) -> (LayerId, [cs_core::ObjectId; 2]) {
    use cs_core::ArtboardId;
    let e = h.editor();
    e.duplicate_frame();
    e.set_frame(0);
    e.add_artboard(4, 4);
    e.activate(ArtboardId(1));
    e.add_instance_layer(ArtboardId(2));
    let layer = e.layer;
    let (cmd, b) =
        cs_commands::ops::add_object(e.sprite(), layer, 0, ArtboardId(2), "Beta", (0, 0)).unwrap();
    e.execute(cmd);
    let board = e.document.artboard_mut(ArtboardId(1)).unwrap();
    let l = board.sprite.layer_mut(layer).unwrap();
    l.name = "Scene".into();
    l.objects[0].name = "Alpha".into();
    let a = l.objects[0].id;
    h.frame(vec![]);
    (layer, [a, b])
}

fn has_node(h: &Harness, text: &str) -> bool {
    h.nodes.iter().any(|(t, _)| t == text)
}

#[test]
fn an_object_layer_expands_into_one_row_per_object() {
    let mut h = Harness::new(1200.0);
    let (layer, _) = with_two_objects(&mut h);
    assert!(!has_node(&h, "Alpha"), "collapsed at first");
    h.click("Objects: Scene");
    h.frame(vec![]);
    assert!(
        h.app
            .doc
            .timeline
            .expanded_objects
            .contains(&(h.app.doc.editor.active, layer))
    );
    let (alpha, beta) = (h.rect("Alpha"), h.rect("Beta"));
    // De arriba hacia abajo: el último de la lista primero, cada uno de una fila.
    assert!(beta.top() < alpha.top(), "{beta:?} above {alpha:?}");
    // Con la altura de fila de siempre: Beta en la fila 1, Alpha en la 2 y la capa de abajo en
    // la 3.
    let h_row = Grid::CLASSIC.row_h;
    let scene = h.row(layer);
    assert_eq!(
        beta,
        Rect::from_min_max(
            pos2(beta.left(), scene.bottom()),
            pos2(beta.right(), scene.bottom() + h_row)
        )
    );
    assert_eq!(alpha.top(), beta.bottom());
    assert_eq!(alpha.height(), h_row);
    let below = h.editor().timeline_layers()[1];
    assert_eq!(h.row(below).top(), alpha.bottom());
    // Con sangría: más adentro que el nombre de la capa.
    let name_left = h.grid.x - 6.0 + Grid::CLASSIC.eye_w + Grid::CLASSIC.lock_w + 4.0;
    assert!(alpha.left() > name_left, "{alpha:?}");
    h.click("Objects: Scene");
    h.frame(vec![]);
    assert!(!has_node(&h, "Alpha"), "collapsed again");
}

#[test]
fn clicking_an_object_row_selects_the_object() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_two_objects(&mut h);
    let active = h.editor().active;
    h.state().expanded_objects.insert((active, layer));
    let below = h.editor().timeline_layers()[1];
    h.editor().select_layer(below);
    h.frame(vec![]);
    h.click("Beta");
    assert_eq!(h.editor().layer, layer);
    assert_eq!(h.editor().selected_objects(), [b]);
    h.click("Alpha");
    assert_eq!(h.editor().selected_objects(), [a]);
}

#[test]
fn the_keyframe_menu_of_an_object_row_works_on_that_object() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_two_objects(&mut h);
    let active = h.editor().active;
    h.state().expanded_objects.insert((active, layer));
    h.frame(vec![]);
    let keys = |h: &Harness, o: cs_core::ObjectId| -> Vec<usize> {
        let l = h.app.doc.editor.sprite().layer(layer).unwrap();
        l.object(o)
            .unwrap()
            .track
            .keys
            .iter()
            .map(|k| k.frame)
            .collect()
    };
    // Aunque Alpha esté seleccionado, el menú de la sub-fila de Beta va a Beta.
    let at = pos2(h.header_cell(1).x, h.rect("Beta").center().y);
    h.right_click_at(at);
    h.click("Insert Keyframe");
    h.frame(vec![]);
    assert_eq!(keys(&h, b), [0, 1]);
    assert_eq!(keys(&h, a), [0]);
    h.right_click_at(at);
    h.click("Delete Keyframe");
    h.frame(vec![]);
    assert_eq!(keys(&h, b), [0]);
    assert_eq!(keys(&h, a), [0]);
}

#[test]
fn the_collapsed_row_menu_acts_on_the_selected_objects_only() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_two_objects(&mut h);
    let keys = |h: &Harness, o: cs_core::ObjectId| -> Vec<usize> {
        let l = h.app.doc.editor.sprite().layer(layer).unwrap();
        l.object(o)
            .unwrap()
            .track
            .keys
            .iter()
            .map(|k| k.frame)
            .collect()
    };
    let row = h.row(layer);
    // Los dos tienen key en 0 y ninguno está seleccionado: la fila no elige uno cualquiera.
    h.editor().clear_object_selection();
    h.frame(vec![]);
    let at = |h: &Harness, f| pos2(h.header_cell(f).x, row.center().y);
    h.right_click_at(at(&h, 0));
    assert!(has_node(&h, "Delete Keyframe"), "the menu is open");
    h.click("Delete Keyframe");
    h.frame(vec![]);
    assert_eq!((keys(&h, a), keys(&h, b)), (vec![0], vec![0]), "disabled");
    // Con Beta seleccionado, Insert va a Beta.
    h.editor().select_object(layer, b, false);
    h.frame(vec![]);
    h.right_click_at(at(&h, 1));
    h.click("Insert Keyframe");
    h.frame(vec![]);
    assert_eq!((keys(&h, a), keys(&h, b)), (vec![0], vec![0, 1]));
}

#[test]
fn with_pixel_ui_an_object_layer_has_no_picture_label() {
    let mut h = Harness::pixel(1200.0);
    let (layer, _) = with_two_objects(&mut h);
    let active = h.editor().active;
    h.state().expanded_objects.insert((active, layer));
    h.frame(vec![]);
    assert!(h.texts.iter().any(|t| t == "Scene"), "{:?}", h.texts);
    assert!(h.texts.iter().all(|t| !t.contains('🖼')), "{:?}", h.texts);
    assert!(has_node(&h, "Alpha") && has_node(&h, "Beta"));
    // En su lugar, el ícono del skin (en la capa activa, su versión para lo elegido).
    assert_eq!(h.drawn("icon.layer.objects.selected").len(), 1);
    assert!(h.drawn("icon.layer.objects").is_empty());
    h.editor().select_layer(LayerId(1));
    h.frame(vec![]);
    assert_eq!(h.drawn("icon.layer.objects").len(), 1);
    // Sin Pixel UI, el texto sigue.
    let mut h = Harness::new(1200.0);
    with_two_objects(&mut h);
    assert!(h.texts.iter().any(|t| t == "🖼 Scene"), "{:?}", h.texts);
}

/// `with_two_objects` con una key más de Beta en el frame 1 (en (2, 2)).
fn with_tweenable_beta(h: &mut Harness) -> (LayerId, [cs_core::ObjectId; 2]) {
    let (layer, [a, b]) = with_two_objects(h);
    let cmd =
        ops::update_object_key(h.editor().sprite(), layer, b, 1, |k| k.position = (2, 2)).unwrap();
    h.editor().execute(cmd);
    h.frame(vec![]);
    (layer, [a, b])
}

fn first_tween(h: &Harness, layer: LayerId, o: cs_core::ObjectId) -> Option<cs_core::Tween> {
    let l = h.app.doc.editor.sprite().layer(layer).unwrap();
    l.object(o).unwrap().track.keys[0].tween
}

#[test]
fn the_tween_menu_of_an_object_row_creates_and_removes_its_tween() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_tweenable_beta(&mut h);
    let active = h.editor().active;
    h.state().expanded_objects.insert((active, layer));
    h.frame(vec![]);
    let fill = theme::ACCENT.gamma_multiply(0.25);
    assert!(!h.flat_colors.contains(&fill));
    let at = pos2(h.header_cell(0).x, h.rect("Beta").center().y);
    h.right_click_at(at);
    h.click("Create Tween");
    h.frame(vec![]);
    assert!(first_tween(&h, layer, b).is_some());
    assert!(first_tween(&h, layer, a).is_none());
    assert!(h.flat_colors.contains(&fill), "the span is painted");
    // Alpha no tiene key siguiente: Create Tween está deshabilitado.
    let alpha = pos2(h.header_cell(0).x, h.rect("Alpha").center().y);
    h.right_click_at(alpha);
    h.click("Create Tween");
    h.frame(vec![]);
    assert!(first_tween(&h, layer, a).is_none());
    h.right_click_at(at);
    h.click("Remove Tween");
    h.frame(vec![]);
    assert!(first_tween(&h, layer, b).is_none());
    // Cada uno, un paso de undo.
    h.editor().undo();
    assert!(first_tween(&h, layer, b).is_some());
}

#[test]
fn the_collapsed_row_tween_menu_acts_on_the_selected_object() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_tweenable_beta(&mut h);
    h.editor().select_object(layer, b, false);
    h.frame(vec![]);
    let row = h.row(layer);
    h.right_click_at(pos2(h.header_cell(0).x, row.center().y));
    h.click("Create Tween");
    h.frame(vec![]);
    assert!(first_tween(&h, layer, b).is_some());
    assert!(first_tween(&h, layer, a).is_none());
}

#[test]
fn tween_cells_carry_the_arrow_or_the_dashed_line() {
    let span = TweenSpan {
        start: 2,
        end: Some(5),
        broken: false,
    };
    let marks = |f| tween_marks(span, f, 8);
    assert_eq!(
        marks(2),
        TweenMarks {
            from_left: false,
            to_right: true,
            arrow: false,
            dashed: false
        }
    );
    assert!(marks(3).from_left && !marks(3).arrow);
    assert!(marks(4).arrow, "the cell before the end key");
    let broken = TweenSpan {
        start: 2,
        end: None,
        broken: true,
    };
    let last = tween_marks(broken, 7, 8);
    assert!(last.dashed && !last.arrow && !last.to_right && last.from_left);
}

// ---- La tabla y los tags del mockup (`.refs/timeline/mockup.png`, 236×138 a 1x) ----
//
// Medidas del mockup con su caja en x = 10 (interior desde 11): el primer grupo de la barra en
// x = 14, la tabla desde x = 12 con el brillo de su borde en x = 11, el contenido del frame 1 en
// L = 129 y el final de la celda del frame 4 en R = 181; la cajita del tag "Idle" arriba en T = 49
// y el encabezado (los números) desde B = 66.

/// Dónde quedaron la tabla y el tag en el último frame: lo que el mockup llama 12 (`left`, el
/// principio de las filas), L, B, T y la última columna de la cajita (`label_right`).
struct TagMarks {
    left: f32,
    l: f32,
    b: f32,
    t: f32,
    label_right: f32,
}

/// Un sprite de 4 frames con el tag "Idle" de todos, como el demo y el mockup, con el frame 2
/// actual (el frame elegido taparía el pie de la pata izquierda). Devuelve el color del tag.
fn with_idle_tag(h: &mut Harness) -> Color32 {
    for _ in 0..3 {
        h.editor().duplicate_frame();
    }
    h.editor().frame_range = Some((0, 3));
    h.editor().add_tag();
    let mut tag = h.editor().sprite().tags[0].clone();
    tag.name = "Idle".into();
    h.editor().set_tag(0, tag);
    h.editor().frame_range = None;
    h.editor().set_frame(1);
    h.frame(vec![]);
    to_color32(h.editor().sprite().tags[0].color)
}

/// Las medidas del mockup en el último frame: el ojo del encabezado va 3 a la derecha y 2 abajo
/// del principio de su celda; la cajita del tag es el rect del color del tag de 11 de alto (si
/// no hay tag, T y `label_right` no dicen nada).
fn tag_marks(h: &Harness, color: Option<Color32>) -> TagMarks {
    let eye = h.drawn("timeline.eye")[0];
    let (left, b) = (eye.left() - 3.0, eye.top() - 2.0);
    let label = color.map_or(Rect::NOTHING, |color| {
        h.flat_rects
            .iter()
            .find(|(r, c)| *c == color && r.height() == 11.0)
            .map(|(r, _)| *r)
            .expect("the tag label")
    });
    TagMarks {
        left,
        l: left + 117.0,
        b,
        t: label.top(),
        label_right: label.right() - 1.0,
    }
}

/// El negro de los contornos del mockup.
const TAG_INK: Color32 = Color32::from_rgb(0x05, 0x04, 0x03);

#[test]
fn with_pixel_ui_the_table_starts_one_column_into_the_box_with_a_light_edge() {
    let mut h = Harness::pixel(1200.0);
    // Dos capas con la de abajo activa (como Body en el mockup).
    let bottom = h.layer(0).id;
    h.editor().add_layer(LayerKind::Image);
    h.editor().layer = bottom;
    h.frame(vec![]);
    let colors = *h.skin().colors();
    let m = tag_marks(&h, None);
    // El primer grupo de la barra está a 3 del interior de la caja (x = 14 en el mockup); la
    // tabla, a 1 (x = 12), y la columna de en medio (x = 11) es el brillo de su borde.
    let inner = h.rect("First frame").left() - 3.0;
    assert_eq!(m.left, inner + 1.0, "the table starts at x = 12");
    // El encabezado (contenido de 66 a 76): el brillo del borde (11), el contenido del ojo
    // (12..26), su línea oscura (27) y su brillo (28).
    let y = m.b + 5.0;
    assert_eq!(h.color_at(inner, y), Some(colors.highlight), "x = 11");
    assert_eq!(h.color_at(m.left, y), Some(colors.background), "x = 12");
    assert_eq!(h.color_at(m.left + 15.0, y), Some(colors.canvas), "x = 27");
    assert_eq!(
        h.color_at(m.left + 16.0, y),
        Some(colors.highlight),
        "x = 28"
    );
    // Las líneas de abajo del encabezado (77 oscura, 78 brillo) empiezan en x = 11.
    assert_eq!(h.color_at(inner, m.b + 11.0), Some(colors.canvas), "y = 77");
    assert_eq!(
        h.color_at(inner, m.b + 12.0),
        Some(colors.highlight),
        "y = 78"
    );
    // La capa activa (la segunda fila, de 90 a 102 como Shade) en rosa también en esa columna:
    // la oscura arriba (90) y abajo (103) y el brillo en medio (91..102), como Body en 103..116.
    let dark = Some(colors.timeline_selection_dark);
    assert_eq!(h.color_at(inner, m.b + 24.0), dark, "y = 90");
    for dy in 25..=36 {
        let y = m.b + dy as f32;
        assert_eq!(
            h.color_at(inner, y),
            Some(colors.timeline_selection_light),
            "y = {y}"
        );
    }
    assert_eq!(h.color_at(inner, m.b + 37.0), dark, "y = 103");
    assert_eq!(h.color_at(m.left + 15.0, m.b + 30.0), dark, "x = 27");
}

#[test]
fn with_pixel_ui_tags_close_the_header_with_the_line_of_the_rows() {
    let mut h = Harness::pixel(1200.0);
    let colors = *h.skin().colors();
    let color = with_idle_tag(&mut h);
    let m = tag_marks(&h, Some(color));
    assert_eq!(m.b - m.t, 17.0, "B = 66, T = 49");
    // La línea de arriba del encabezado: oscura en y = 64 (B − 2) y brillo en y = 65 (B − 1),
    // desde el brillo del borde (x = 11) por el nombre y el contenido de los frames.
    for x in [m.left - 1.0, m.left + 50.0, m.l + 6.0, m.l + 26.0 + 6.0] {
        assert_eq!(h.color_at(x, m.b - 2.0), Some(colors.canvas), "dark at {x}");
        assert_eq!(
            h.color_at(x, m.b - 1.0),
            Some(colors.highlight),
            "light at {x}"
        );
    }
    // Las columnas del encabezado suben hasta el brillo (x = 27 en y = 65).
    assert_eq!(h.color_at(m.left + 15.0, m.b - 1.0), Some(colors.canvas));
}

#[test]
fn with_pixel_ui_the_tag_label_has_a_black_outline_with_cut_corners_like_the_mockup() {
    let mut h = Harness::pixel(1200.0);
    let color = with_idle_tag(&mut h);
    let m = tag_marks(&h, Some(color));
    let ink = Some(TAG_INK);
    // La cajita: de L + 1 (x = 130) a la tinta + 3 (x = 151), de T (49) a T + 10 (59).
    let (x0, x1) = (m.l + 1.0, m.label_right);
    // Arriba (y = 48) y abajo (y = 60, donde la pata izquierda sigue naranja en x = 130).
    for x in (x0 as i32)..=(x1 as i32) {
        let x = x as f32;
        assert_eq!(h.color_at(x, m.t - 1.0), ink, "top at {x}");
        let bottom = if x == x0 { Some(color) } else { ink };
        assert_eq!(h.color_at(x, m.t + 11.0), bottom, "bottom at {x}");
    }
    // Los lados (x = 129 y x = 152); el derecho lo atraviesa el corchete en y = 54.
    for y in 0..11 {
        let y = m.t + y as f32;
        assert_eq!(h.color_at(m.l, y), ink, "left at {y}");
        let right = if y == m.t + 5.0 { Some(color) } else { ink };
        assert_eq!(h.color_at(x1 + 1.0, y), right, "right at {y}");
    }
    // Las esquinas recortadas: (129, 48), (152, 48) y (152, 60); en (129, 60) sigue el contorno
    // de la pata.
    assert_ne!(h.color_at(m.l, m.t - 1.0), ink);
    assert_ne!(h.color_at(x1 + 1.0, m.t - 1.0), ink);
    assert_ne!(h.color_at(x1 + 1.0, m.t + 11.0), ink);
    assert_eq!(h.color_at(m.l, m.t + 11.0), ink);
}

#[test]
fn with_pixel_ui_the_tag_bracket_is_orange_with_a_black_outline_like_the_mockup() {
    let mut h = Harness::pixel(1200.0);
    let colors = *h.skin().colors();
    let color = with_idle_tag(&mut h);
    let m = tag_marks(&h, Some(color));
    let (ink, orange) = (Some(TAG_INK), Some(color));
    let r = m.l + 4.0 * 13.0;
    let mid = m.t + 5.0;
    // Arriba: naranja en y = 54 de la cajita a R − 3 (x = 178), contorno en y = 53 hasta R − 3 y
    // en y = 55 hasta R − 4 (177); la esquina de afuera (179, 53), recortada.
    for x in (m.label_right as i32 + 1)..=(r as i32 - 3) {
        let x = x as f32;
        assert_eq!(h.color_at(x, mid), orange, "bar at {x}");
        assert_eq!(h.color_at(x, mid - 1.0), ink, "bar top at {x}");
        let below = if x == r - 3.0 { orange } else { ink };
        assert_eq!(h.color_at(x, mid + 1.0), below, "bar bottom at {x}");
    }
    assert_ne!(h.color_at(r - 2.0, mid - 1.0), ink, "cut corner");
    // La pata derecha: naranja en x = 178 hasta B − 2 (64), contorno en 177 y 179 (sobre la
    // línea oscura de la tabla); abierta abajo: en y = 65, el brillo de la línea.
    for y in (mid as i32)..=(m.b as i32 - 2) {
        let y = y as f32;
        assert_eq!(h.color_at(r - 3.0, y), orange, "right leg at {y}");
        assert_eq!(h.color_at(r - 2.0, y), ink, "right leg outside at {y}");
        if y > mid {
            assert_eq!(h.color_at(r - 4.0, y), ink, "right leg inside at {y}");
        }
    }
    assert_eq!(h.color_at(r - 3.0, m.b - 1.0), Some(colors.highlight));
    // La pata izquierda: naranja en x = 130 de debajo de la cajita (60) a B − 2 (64), contorno
    // en 129 y 131.
    for y in (m.t as i32 + 11)..=(m.b as i32 - 2) {
        let y = y as f32;
        assert_eq!(h.color_at(m.l + 1.0, y), orange, "left leg at {y}");
        assert_eq!(h.color_at(m.l, y), ink, "left leg outside at {y}");
        assert_eq!(h.color_at(m.l + 2.0, y), ink, "left leg inside at {y}");
    }
    assert_eq!(h.color_at(m.l + 1.0, m.b - 1.0), Some(colors.highlight));
}

#[test]
fn with_pixel_ui_a_tag_narrower_than_its_label_drops_both_legs_below_it() {
    let mut h = Harness::pixel(1200.0);
    h.editor().add_tag();
    let mut tag = h.editor().sprite().tags[0].clone();
    tag.name = "A long name".into();
    h.editor().set_tag(0, tag);
    h.frame(vec![]);
    let color = to_color32(h.editor().sprite().tags[0].color);
    let m = tag_marks(&h, Some(color));
    let r = m.l + 13.0;
    assert!(m.label_right > r, "the label is wider than the frame");
    // La pata derecha (x = R − 3) sale de debajo de la cajita, como la izquierda; el frame
    // elegido (el único) tapa su pie.
    for y in (m.t as i32 + 11)..(m.b as i32 - 2) {
        let y = y as f32;
        assert_eq!(h.color_at(r - 3.0, y), Some(color), "right leg at {y}");
        assert_eq!(h.color_at(r - 2.0, y), Some(TAG_INK), "outside at {y}");
        assert_eq!(h.color_at(r - 4.0, y), Some(TAG_INK), "inside at {y}");
    }
    // Y nada sale a la derecha de la cajita.
    assert_ne!(h.color_at(m.label_right + 2.0, m.t + 5.0), Some(color));
}

// ---- La fila de botones de la barra en anchos chicos ----

/// Los controles con nombre de la barra con Pixel UI: los botones de los grupos, "Opacity" y los
/// − y + de sus steppers, y el combo.
const BAR_CONTROLS: [&str; 18] = [
    "First frame",
    "Previous frame",
    "Play",
    "Next frame",
    "Last frame",
    "New frame",
    "Delete frame",
    "New layer",
    "Delete layer",
    "New group",
    "New tilemap layer",
    "Onion skin",
    "Opacity",
    "Less opacity",
    "More opacity",
    "Normal",
    "Shorter frame",
    "Longer frame",
];

/// El timeline con Pixel UI con `w` de ancho a la vista: la ventana mide `w` más lo que le quita
/// la tira de la derecha (medido en un arnés de 600, no adivinado). Alto como acoplado junto al
/// lienzo (450 de 700): a 64 la barra tiene 12 filas (264) y en 190 no se verían todas.
fn pixel_seen(w: f32) -> Harness {
    let probe = Harness::pixel(600.0);
    let strip = 600.0 - probe.visible_width();
    let h = Harness::sized(w + strip, true, 700.0, 450.0);
    assert_eq!(h.visible_width(), w, "the timeline is {w} wide");
    h
}

/// Los nodos de la barra (arriba de su línea de abajo, `timeline.divider`): los de
/// [`BAR_CONTROLS`] y los campos (su nombre es su número). Los íconos de la tira, que bajan a la
/// derecha del timeline, no. Ordenados por fila y de izquierda a derecha.
fn bar_nodes(h: &Harness) -> Vec<(String, Rect)> {
    let divider = h.drawn("timeline.divider")[0].top();
    let mut bar: Vec<(String, Rect)> = h
        .nodes
        .iter()
        .filter(|(t, r)| {
            r.bottom() <= divider
                && (BAR_CONTROLS.contains(&t.as_str()) || t.parse::<u32>().is_ok())
        })
        .cloned()
        .collect();
    bar.sort_by(|(_, a), (_, b)| {
        (a.top(), a.left())
            .partial_cmp(&(b.top(), b.left()))
            .unwrap()
    });
    bar
}

#[test]
fn the_button_row_wraps_down_to_the_minimum() {
    let mut rows = Vec::new();
    for w in [64.0, 69.0, 70.0, 80.0, 100.0, 126.0, 127.0, 200.0] {
        let h = pixel_seen(w);
        let bar = bar_nodes(&h);
        for label in BAR_CONTROLS {
            assert!(
                bar.iter().any(|(t, _)| t == label),
                "{w}: {label} in the bar {bar:?}"
            );
        }
        // Nada se sale de lo que se ve del timeline. A 64 la barra mide 49 (64 − 15: la caja,
        // su aire a la derecha y el de la barra) y lo que no se parte y es más ancho se sale: el
        // combo (59) a 64 y el stepper de la duración (64: el campo de "100ms" mide 34) hasta
        // 69 (ver `KNOWN_GAPS` del barrido de anchos).
        let outside: Vec<&str> = bar
            .iter()
            .filter(|(_, r)| r.left() < 0.0 || r.right() > w)
            .map(|(t, _)| t.as_str())
            .collect();
        let expected: &[&str] = if w == 64.0 {
            &["Normal", "Longer frame"]
        } else if w == 69.0 {
            &["Longer frame"]
        } else {
            &[]
        };
        assert_eq!(outside, expected, "{w}: {bar:?}");
        // Los controles, en puntos enteros y sin encimarse: los botones de un grupo solo
        // comparten la columna de su borde (se cruzan en 1 punto de ancho).
        let controls: Vec<&(String, Rect)> = bar
            .iter()
            .filter(|(t, _)| BAR_CONTROLS.contains(&t.as_str()))
            .collect();
        for (t, r) in &controls {
            let corners = [r.min.x, r.min.y, r.max.x, r.max.y];
            assert!(
                corners.iter().all(|v| *v == v.round()),
                "{w}: {t:?} at {r:?}"
            );
        }
        for (i, (a, ra)) in controls.iter().enumerate() {
            for (b, rb) in &controls[i + 1..] {
                let both = ra.intersect(*rb);
                assert!(
                    both.width() <= 1.0 || both.height() <= 1.0,
                    "{w}: {a:?} {ra:?} and {b:?} {rb:?}"
                );
            }
        }
        let mut tops: Vec<f32> = controls.iter().map(|(_, r)| r.top()).collect();
        tops.sort_by(f32::total_cmp);
        tops.dedup();
        rows.push((w, tops.len()));
    }
    let of = |w: f32| rows.iter().find(|(x, _)| *x == w).unwrap().1;
    assert!(of(64.0) > of(127.0), "more rows when narrower: {rows:?}");
}

/// Con `w` a la vista y 4 frames en el primero: un clic en "Last frame" y uno en "Play". Devuelve
/// el frame y si se reproduce, y el arnés.
fn last_then_play(w: f32) -> (usize, bool, Harness) {
    let mut h = pixel_seen(w);
    for _ in 0..3 {
        h.editor().duplicate_frame();
    }
    h.editor().set_frame(0);
    h.frame(vec![]);
    h.click("Last frame");
    let frame = h.editor().frame;
    h.click("Play");
    let playing = h.editor().playing;
    (frame, playing, h)
}

#[test]
fn a_split_group_keeps_its_buttons_working() {
    let wide = last_then_play(600.0);
    assert_eq!((wide.0, wide.1), (3, true));
    let (frame, playing, h) = last_then_play(64.0);
    // A 64, Play y Last frame van en filas de abajo de las del primero del grupo.
    let first = h.rect("First frame").top();
    assert!(h.rect("Pause").top() > first, "Play went down a row");
    assert!(
        h.rect("Last frame").top() > first,
        "Last frame went down a row"
    );
    assert_eq!((frame, playing), (wide.0, wide.1), "the same as at 600");
}

#[test]
fn the_button_row_is_unchanged_where_it_fit() {
    // Los rects (relativos a la hoja) de antes de partirse por botón, en una fila más la
    // duración en la de abajo a 600 y en una sola fila al ancho del timeline en Essentials: su
    // hoja de 37 a 937 (`essentials_at_1200_by_800`) sin la tira (26).
    type Want = (&'static str, [f32; 4]);
    let row: [Want; 19] = [
        ("1", [334.0, 5.5, 355.0, 19.5]),
        ("1", [356.0, 5.5, 377.0, 19.5]),
        ("255", [444.0, 5.5, 472.0, 19.5]),
        ("First frame", [6.0, 6.0, 31.0, 23.0]),
        ("Previous frame", [30.0, 6.0, 55.0, 23.0]),
        ("Play", [54.0, 6.0, 79.0, 23.0]),
        ("Next frame", [78.0, 6.0, 103.0, 23.0]),
        ("Last frame", [102.0, 6.0, 127.0, 23.0]),
        ("New frame", [135.0, 6.0, 160.0, 23.0]),
        ("Delete frame", [159.0, 6.0, 184.0, 23.0]),
        ("New layer", [192.0, 6.0, 217.0, 23.0]),
        ("Delete layer", [216.0, 6.0, 241.0, 23.0]),
        ("New group", [240.0, 6.0, 265.0, 23.0]),
        ("New tilemap layer", [264.0, 6.0, 289.0, 23.0]),
        ("Onion skin", [297.0, 6.0, 322.0, 23.0]),
        ("Opacity", [393.0, 6.0, 423.0, 22.0]),
        ("Less opacity", [429.0, 6.0, 444.0, 22.0]),
        ("More opacity", [472.0, 6.0, 487.0, 22.0]),
        ("Normal", [491.0, 6.0, 550.0, 23.0]),
    ];
    let at_600: [Want; 3] = [
        ("100", [42.0, 27.5, 57.0, 41.5]),
        ("Shorter frame", [24.0, 28.0, 39.0, 44.0]),
        ("Longer frame", [73.0, 28.0, 88.0, 44.0]),
    ];
    let at_essentials: [Want; 3] = [
        ("100", [594.0, 5.5, 609.0, 19.5]),
        ("Shorter frame", [576.0, 6.0, 591.0, 22.0]),
        ("Longer frame", [625.0, 6.0, 640.0, 22.0]),
    ];
    for (w, duration) in [(600.0, at_600), (900.0 - 26.0, at_essentials)] {
        let h = pixel_seen(w);
        let want: Vec<(String, Rect)> = row
            .iter()
            .chain(&duration)
            .map(|(t, [x0, y0, x1, y1])| {
                let r = Rect::from_min_max(pos2(*x0, *y0), pos2(*x1, *y1));
                (t.to_string(), r)
            })
            .collect();
        let mut want = want;
        want.sort_by(|(_, a), (_, b)| {
            (a.top(), a.left())
                .partial_cmp(&(b.top(), b.left()))
                .unwrap()
        });
        assert_eq!(bar_nodes(&h), want, "at {w}");
    }
}
