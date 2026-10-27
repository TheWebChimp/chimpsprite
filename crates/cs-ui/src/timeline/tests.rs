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
        for clipped in &output.shapes {
            collect_images(&clipped.shape, &mut self.images);
            collect_flat_colors(&clipped.shape, &mut self.flat_colors);
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
    // Los cuatro grupos de botones, sus 12 celdas y las 8 líneas entre celdas.
    assert!(h.count("timeline.group") >= 4);
    assert_eq!(h.count("timeline.button"), 12);
    assert_eq!(h.count("timeline.button.sep"), 8);
    for name in [
        "timeline.onion_range",
        "timeline.icon.clock",
        "timeline.combo",
        "timeline.divider",
        "workspace.box",
    ] {
        assert!(h.count(name) >= 1, "{name}");
    }
    // − y + de la opacidad y de la duración.
    assert_eq!(h.count("timeline.stepper.minus"), 2);
    assert_eq!(h.count("timeline.stepper.plus"), 2);
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
    let group = h.drawn("timeline.button")[0];
    let divider = h.drawn("timeline.divider")[0];
    assert_eq!(
        divider.top() - group.top(),
        17.0,
        "button top 563, divider 580"
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
    assert_eq!(
        h.count("timeline.combo"),
        3,
        "the combo of the design, in 9-slice"
    );
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
    // La capa activa, de imagen y sin máscara: el botón para agregarle una.
    assert!(h.drawn("timeline.mask").is_empty());
    assert_eq!(h.drawn("timeline.mask.add").len(), 1);
    h.click("Add Layer Mask");
    h.frame(vec![]);
    let layer = h.editor.layer;
    assert!(
        h.editor.sprite().layer(layer).unwrap().mask.is_some(),
        "added"
    );
    assert!(h.drawn("timeline.mask.add").is_empty());
    // La cara; editándola, la del borde rosa.
    h.editor.set_edit_mask(false);
    h.frame(vec![]);
    let face = h.drawn("timeline.mask");
    assert_eq!(face.len(), 1);
    assert_eq!(face[0].size(), Vec2::new(9.0, 9.0));
    h.editor.set_edit_mask(true);
    h.frame(vec![]);
    assert_eq!(h.drawn("timeline.mask.editing").len(), 1);
    assert!(h.drawn("timeline.mask").is_empty());
}

#[test]
fn with_pixel_ui_the_layer_menu_is_the_menus_panel() {
    let mut h = Harness::new(1200.0);
    let skin = std::sync::Arc::new(Skin::embedded(&h.ctx));
    let panel = skin
        .piece("menu.panel", crate::skin::WidgetState::Normal)
        .expect("menu.panel")
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
    h.click("Delete tag");
    h.frame(vec![]);
    assert_eq!(h.editor.sprite().tags.len(), 1);
}
