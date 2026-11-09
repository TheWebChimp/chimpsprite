use cs_core::{ColorMode, Sprite};
use eframe::egui::{Event, Pos2, RawInput};

use super::*;

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

/// La línea de tiempo en un panel de abajo, con eventos crudos (50 ms por frame).
struct Harness {
    ctx: egui::Context,
    editor: Editor,
    state: TimelineState,
    time: f64,
    width: f32,
    nodes: Vec<(String, Rect)>,
    /// Posición del texto "Layers" del encabezado de la grilla.
    grid: Pos2,
    /// Con Pixel UI: el skin embebido.
    skin: Option<Skin>,
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

    /// Con Pixel UI: el skin embebido y el marco del panel de la app.
    fn pixel(width: f32) -> Self {
        Self::with(width, true)
    }

    fn with(width: f32, pixel: bool) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = pixel.then(|| Skin::embedded(&ctx));
        let mut h = Self {
            ctx,
            editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
            state: TimelineState::default(),
            time: 0.0,
            width,
            nodes: Vec::new(),
            grid: Pos2::ZERO,
            skin,
            images: Vec::new(),
            flat_colors: Vec::new(),
            texts: Vec::new(),
            flat_rects: Vec::new(),
        };
        for _ in 0..3 {
            h.frame(vec![]);
        }
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(self.width, 400.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let (editor, state, skin) = (&mut self.editor, &mut self.state, self.skin.as_ref());
        let mut output = self.ctx.run_ui(input, |ui| {
            let mut panel = egui::Panel::bottom("timeline").exact_size(190.0);
            if let Some(skin) = skin {
                panel = panel
                    .frame(pixel_panel_frame(skin))
                    .show_separator_line(false);
            }
            panel.show(ui, |ui| show(ui, editor, state, skin));
        });
        output.textures_delta.clear();
        self.images.clear();
        self.flat_colors.clear();
        self.texts.clear();
        self.flat_rects.clear();
        for clipped in &output.shapes {
            collect_flat_rects(&clipped.shape, clipped.clip_rect, &mut self.flat_rects);
            collect_images(&clipped.shape, &mut self.images);
            collect_flat_colors(&clipped.shape, &mut self.flat_colors);
            collect_texts(&clipped.shape, &mut self.texts);
        }
        self.nodes = output
            .platform_output
            .accesskit_update
            .map(|u| {
                u.nodes
                    .iter()
                    .filter_map(|(_, n)| {
                        let b = n.bounds()?;
                        let text = n.label().or_else(|| n.value())?.to_string();
                        Some((
                            text,
                            Rect::from_min_max(
                                pos2(b.x0 as f32, b.y0 as f32),
                                pos2(b.x1 as f32, b.y1 as f32),
                            ),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for clipped in &output.shapes {
            if let egui::Shape::Text(t) = &clipped.shape
                && t.galley.text() == "Layers"
            {
                self.grid = t.pos;
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
        let skin = self.skin.as_ref().expect("pixel harness");
        let id = skin
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
        &self.editor.sprite().layers[i]
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
    assert!(h.editor.playing);
    assert!(
        h.rect("Pause").is_positive(),
        "the button says what it does now"
    );
    h.click("Pause");
    assert!(!h.editor.playing);

    h.click("New frame");
    assert_eq!(h.editor.frame_count(), 2);
    h.click("Delete frame");
    assert_eq!(h.editor.frame_count(), 1);

    let layers = h.editor.sprite().layers.len();
    h.click("New layer");
    assert_eq!(h.editor.sprite().layers.len(), layers + 1);
    h.click("New group");
    assert_eq!(h.editor.sprite().layers.len(), layers + 2);
    h.click("New tilemap layer");
    assert!(h.state.new_tilemap, "asks the app for the dialog");

    assert!(!h.editor.onion_enabled);
    assert_eq!(h.count("timeline.icon.onion"), 1, "off: the cut onion");
    h.click("Onion skin");
    assert!(h.editor.onion_enabled);
    assert_eq!(h.count("timeline.icon.onion.on"), 1, "on: the whole onion");
    assert_eq!(h.count("timeline.icon.onion"), 0);
}

#[test]
fn with_pixel_ui_the_playback_buttons_move_through_the_frames() {
    let mut h = Harness::pixel(1200.0);
    for _ in 0..3 {
        h.editor.duplicate_frame();
    }
    h.click("First frame");
    assert_eq!(h.editor.frame, 0);
    h.click("Next frame");
    assert_eq!(h.editor.frame, 1);
    h.click("Last frame");
    assert_eq!(h.editor.frame, 3);
    h.click("Previous frame");
    assert_eq!(h.editor.frame, 2);
}

#[test]
fn with_pixel_ui_opacity_minus_and_plus_change_the_layer_opacity() {
    let mut h = Harness::pixel(1200.0);
    let layer = h.editor.layer;
    let opacity = |h: &Harness| h.editor.sprite().layer(layer).unwrap().opacity;
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
    let ms = |h: &Harness| h.editor.sprite().frames[h.editor.frame].duration_ms;
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
    let layer = h.editor.layer;
    assert!(h.count("field.text") >= 1, "the select of the UI kit");
    h.click("Normal");
    h.click("Multiply");
    assert_eq!(
        h.editor.sprite().layer(layer).unwrap().blend,
        BlendMode::Multiply
    );
    h.rect("Multiply");
}

#[test]
fn with_pixel_ui_the_onion_range_shows_the_frames_before_and_after() {
    let mut h = Harness::pixel(1200.0);
    h.editor.onion.before = 2;
    h.editor.onion.after = 5;
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
    h.editor.execute(cs_commands::Command::DrawPixels {
        layer: h.layer(0).id,
        frame: 0,
        pixels: vec![(0, 0, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(9, 9, 9)))],
    });
    h.editor.add_layer(LayerKind::Image);
    let top = h.editor.layer;
    h.editor.toggle_visible(top);
    h.editor.toggle_locked(top);
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
    let colors = *h.skin.as_ref().unwrap().colors();
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
    h.editor.add_layer(LayerKind::Group);
    let group = h.editor.layer;
    h.frame(vec![]);
    // La capa activa: la carpetita en su versión para lo elegido; abierta.
    assert_eq!(h.count("timeline.folder.open.selected"), 1);
    assert_eq!(h.count("timeline.folder.selected"), 0);
    h.editor.toggle_collapsed(group);
    h.frame(vec![]);
    assert_eq!(h.count("timeline.folder.selected"), 1, "collapsed: closed");
    // Otra capa activa: la carpetita de siempre.
    h.editor.add_layer(LayerKind::Image);
    h.frame(vec![]);
    assert_eq!(h.count("timeline.folder"), 1);
}

#[test]
fn with_pixel_ui_clicking_the_eye_and_the_lock_toggles_them() {
    let mut h = Harness::pixel(1200.0);
    let id = h.editor.layer;
    let name = h.editor.sprite().layer(id).unwrap().name.clone();
    let layer = |h: &Harness| h.editor.sprite().layer(id).unwrap().clone();
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
    for label in ["Onion skin", "Less opacity", "Normal", "Longer frame"] {
        let r = h.rect(label);
        assert!(r.left() >= 0.0 && r.right() <= 420.0, "{label}: {r:?}");
    }
    // Y todavía se usa.
    h.click("Longer frame");
    assert_eq!(h.editor.sprite().frames[0].duration_ms, 110);
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
    assert!(h.duration().right() > 1000.0, "it starts clipped");
    for _ in 0..6 {
        h.wheel(-300.0);
    }
    let d = h.duration();
    assert!(d.right() <= 1000.0 && d.left() >= 0.0, "{d:?}");
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
    h.editor.duplicate_frame();
    h.editor.duplicate_frame();
    h.frame(vec![]);
    assert_eq!(h.editor.frame_count(), 3);
    h.right_click_at(h.header_cell(1));
    // Con la grilla clásica, "Delete frame" es solo el del menú (la barra dice "− Frame").
    h.click("Delete frame");
    h.frame(vec![]);
    assert_eq!(h.editor.frame_count(), 2);
}

#[test]
fn deleting_the_last_frame_from_its_header_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor.duplicate_frame();
    h.editor.duplicate_frame();
    h.frame(vec![]);
    h.right_click_at(h.header_cell(2));
    h.click("Delete frame");
    h.frame(vec![]);
    assert_eq!(h.editor.frame_count(), 2);
    assert_eq!(h.editor.frame, 1);
}

#[test]
fn dragging_over_cels_selects_layers_by_frames_and_dragging_the_range_moves_it() {
    let mut h = Harness::new(1200.0);
    h.editor.add_layer(LayerKind::Image);
    for _ in 0..3 {
        h.editor.insert_empty_frame();
    }
    let order = h.editor.timeline_layers();
    let (top, bottom) = (order[0], order[1]);
    let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    for (layer, frame) in [(top, 0), (bottom, 1)] {
        h.editor.execute(cs_commands::Command::DrawPixels {
            layer,
            frame,
            pixels: vec![(0, 0, red)],
        });
    }
    h.frame(vec![]);
    // De la capa de arriba, frame 1, a la de abajo, frame 2: 2 capas × 2 frames.
    h.drag_with(h.cel(0, 0), h.cel(1, 1), Default::default());
    assert_eq!(h.editor.range_layers(), vec![top, bottom]);
    assert_eq!(h.editor.selected_frames(), (0, 1));
    assert_eq!(h.editor.selected_cels().len(), 4);
    // Arrastrándolo dos frames a la derecha, los cels se mueven y el rango con ellos.
    h.drag_with(h.cel(0, 0), h.cel(0, 2), Default::default());
    let sprite = h.editor.sprite();
    assert!(sprite.cel(top, 0).is_none() && sprite.cel(bottom, 1).is_none());
    assert!(sprite.cel(top, 2).is_some() && sprite.cel(bottom, 3).is_some());
    assert_eq!(h.editor.selected_frames(), (2, 3));
    // Con Ctrl, se copia.
    let ctrl = egui::Modifiers::COMMAND;
    h.drag_with(h.cel(0, 2), h.cel(0, 0), ctrl);
    let sprite = h.editor.sprite();
    assert!(
        sprite.cel(top, 0).is_some() && sprite.cel(top, 2).is_some(),
        "copied"
    );
    // Un clic suelto deja un solo cel.
    h.click_at(h.cel(1, 3));
    assert!(!h.editor.has_cel_range());
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
    let layer = h.editor.layer;
    assert!(
        h.editor.sprite().layer(layer).unwrap().mask.is_some(),
        "added"
    );
    assert!(h.drawn("timeline.mask.add.selected").is_empty());
    // La cara; editándola, la del borde rosa. En la capa activa, las de los colores de lo
    // elegido.
    h.editor.set_edit_mask(false);
    h.frame(vec![]);
    let face = h.drawn("timeline.mask.selected");
    assert_eq!(face.len(), 1);
    assert_eq!(face[0].size(), Vec2::new(9.0, 9.0));
    assert!(h.drawn("timeline.mask").is_empty());
    h.editor.set_edit_mask(true);
    h.frame(vec![]);
    assert_eq!(h.drawn("timeline.mask.editing.selected").len(), 1);
    assert!(h.drawn("timeline.mask.selected").is_empty());
}

#[test]
fn with_pixel_ui_the_layer_menu_is_the_menus_panel() {
    let mut h = Harness::new(1200.0);
    let skin = std::sync::Arc::new(Skin::embedded(&h.ctx));
    let panel = skin
        .piece("menu.list", crate::skin::WidgetState::Normal)
        .expect("menu.list")
        .texture
        .id();
    crate::pixel_ui::set_bubble(&h.ctx, Some(skin));
    h.editor.add_layer(LayerKind::Image);
    h.frame(vec![]);
    let layers = h.editor.sprite().layers.len();
    h.right_click_at(h.layer_name(0));
    assert!(
        h.images.iter().any(|(t, _)| *t == panel),
        "the panel of the skin"
    );
    h.click("Delete");
    h.frame(vec![]);
    assert_eq!(h.editor.sprite().layers.len(), layers - 1);
}

#[test]
fn deleting_a_layer_from_its_row_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor.add_layer(LayerKind::Image);
    h.editor.add_layer(LayerKind::Image);
    h.frame(vec![]);
    let layers = h.editor.sprite().layers.len();
    assert!(layers >= 3);
    let top = h.editor.timeline_layers()[0];
    h.right_click_at(h.layer_name(0));
    h.click("Delete");
    h.frame(vec![]);
    assert_eq!(h.editor.sprite().layers.len(), layers - 1);
    assert!(
        h.editor.sprite().layer(top).is_none(),
        "deleted the row's layer"
    );
}

#[test]
fn deleting_a_tag_from_its_menu_does_not_crash() {
    let mut h = Harness::new(1200.0);
    h.editor.duplicate_frame();
    h.editor.add_tag();
    h.editor.add_tag();
    h.frame(vec![]);
    assert_eq!(h.editor.sprite().tags.len(), 2);
    // Los dos tags van sobre el frame actual (el 2).
    let tag = pos2(h.header_cell(1).x, h.grid.y - Grid::CLASSIC.tag_h / 2.0);
    h.right_click_at(tag);
    // El menú (alto, cerca del borde de abajo) se acomoda en el frame siguiente.
    h.frame(vec![]);
    h.click("Delete tag");
    h.frame(vec![]);
    assert_eq!(h.editor.sprite().tags.len(), 1);
}

/// Una capa de objetos ("Scene") con dos objetos: "Alpha" (el primero) y "Beta" (arriba), en un
/// sprite de dos frames. Devuelve la capa y los dos objetos.
fn with_two_objects(h: &mut Harness) -> (LayerId, [cs_core::ObjectId; 2]) {
    use cs_core::ArtboardId;
    let e = &mut h.editor;
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
    assert!(h.state.expanded_objects.contains(&(h.editor.active, layer)));
    let (alpha, beta) = (h.rect("Alpha"), h.rect("Beta"));
    // De arriba hacia abajo: el último de la lista primero, cada uno de una fila.
    assert!(beta.top() < alpha.top(), "{beta:?} above {alpha:?}");
    // Con la altura de fila de siempre: Beta en la fila 1, Alpha en la 2 y la capa de abajo en
    // la 3.
    let h_row = Grid::CLASSIC.row_h;
    let top = |h: &Harness, l: LayerId| h.state.rows.iter().find(|(r, _)| *r == l).unwrap().1;
    let scene = top(&h, layer);
    assert_eq!(
        beta,
        Rect::from_min_max(
            pos2(beta.left(), scene.bottom()),
            pos2(beta.right(), scene.bottom() + h_row)
        )
    );
    assert_eq!(alpha.top(), beta.bottom());
    assert_eq!(alpha.height(), h_row);
    let below = h.editor.timeline_layers()[1];
    assert_eq!(top(&h, below).top(), alpha.bottom());
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
    h.state.expanded_objects.insert((h.editor.active, layer));
    h.editor.select_layer(h.editor.timeline_layers()[1]);
    h.frame(vec![]);
    h.click("Beta");
    assert_eq!(h.editor.layer, layer);
    assert_eq!(h.editor.selected_objects(), [b]);
    h.click("Alpha");
    assert_eq!(h.editor.selected_objects(), [a]);
}

#[test]
fn the_keyframe_menu_of_an_object_row_works_on_that_object() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_two_objects(&mut h);
    h.state.expanded_objects.insert((h.editor.active, layer));
    h.frame(vec![]);
    let keys = |h: &Harness, o: cs_core::ObjectId| -> Vec<usize> {
        let l = h.editor.sprite().layer(layer).unwrap();
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
        let l = h.editor.sprite().layer(layer).unwrap();
        l.object(o)
            .unwrap()
            .track
            .keys
            .iter()
            .map(|k| k.frame)
            .collect()
    };
    let row = h.state.rows.iter().find(|(l, _)| *l == layer).unwrap().1;
    // Los dos tienen key en 0 y ninguno está seleccionado: la fila no elige uno cualquiera.
    h.editor.clear_object_selection();
    h.frame(vec![]);
    let at = |h: &Harness, f| pos2(h.header_cell(f).x, row.center().y);
    h.right_click_at(at(&h, 0));
    assert!(has_node(&h, "Delete Keyframe"), "the menu is open");
    h.click("Delete Keyframe");
    h.frame(vec![]);
    assert_eq!((keys(&h, a), keys(&h, b)), (vec![0], vec![0]), "disabled");
    // Con Beta seleccionado, Insert va a Beta.
    h.editor.select_object(layer, b, false);
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
    h.state.expanded_objects.insert((h.editor.active, layer));
    h.frame(vec![]);
    assert!(h.texts.iter().any(|t| t == "Scene"), "{:?}", h.texts);
    assert!(h.texts.iter().all(|t| !t.contains('🖼')), "{:?}", h.texts);
    assert!(has_node(&h, "Alpha") && has_node(&h, "Beta"));
    // En su lugar, el ícono del skin (en la capa activa, su versión para lo elegido).
    assert_eq!(h.drawn("icon.layer.objects.selected").len(), 1);
    assert!(h.drawn("icon.layer.objects").is_empty());
    h.editor.select_layer(LayerId(1));
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
        ops::update_object_key(h.editor.sprite(), layer, b, 1, |k| k.position = (2, 2)).unwrap();
    h.editor.execute(cmd);
    h.frame(vec![]);
    (layer, [a, b])
}

fn first_tween(h: &Harness, layer: LayerId, o: cs_core::ObjectId) -> Option<cs_core::Tween> {
    let l = h.editor.sprite().layer(layer).unwrap();
    l.object(o).unwrap().track.keys[0].tween
}

#[test]
fn the_tween_menu_of_an_object_row_creates_and_removes_its_tween() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_tweenable_beta(&mut h);
    h.state.expanded_objects.insert((h.editor.active, layer));
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
    h.editor.undo();
    assert!(first_tween(&h, layer, b).is_some());
}

#[test]
fn the_collapsed_row_tween_menu_acts_on_the_selected_object() {
    let mut h = Harness::new(1200.0);
    let (layer, [a, b]) = with_tweenable_beta(&mut h);
    h.editor.select_object(layer, b, false);
    h.frame(vec![]);
    let row = h.state.rows.iter().find(|(l, _)| *l == layer).unwrap().1;
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
        h.editor.duplicate_frame();
    }
    h.editor.frame_range = Some((0, 3));
    h.editor.add_tag();
    let mut tag = h.editor.sprite().tags[0].clone();
    tag.name = "Idle".into();
    h.editor.set_tag(0, tag);
    h.editor.frame_range = None;
    h.editor.set_frame(1);
    h.frame(vec![]);
    to_color32(h.editor.sprite().tags[0].color)
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
    h.editor.add_layer(LayerKind::Image);
    h.editor.layer = bottom;
    h.frame(vec![]);
    let colors = *h.skin.as_ref().unwrap().colors();
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
    let colors = *h.skin.as_ref().unwrap().colors();
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
    let colors = *h.skin.as_ref().unwrap().colors();
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
    h.editor.add_tag();
    let mut tag = h.editor.sprite().tags[0].clone();
    tag.name = "A long name".into();
    h.editor.set_tag(0, tag);
    h.frame(vec![]);
    let color = to_color32(h.editor.sprite().tags[0].color);
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
