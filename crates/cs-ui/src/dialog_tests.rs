//! Diálogos de la app con y sin Pixel UI: la app entera con una pestaña, manejada con eventos
//! crudos. Los botones y campos se buscan por su nombre en el árbol de accesibilidad.

use eframe::egui::{
    self, Color32, Event, Key, PointerButton, Pos2, RawInput, Rect, Shape, TextureId, Vec2,
};

use crate::ChimpApp;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::skin::WidgetState;
use crate::view::ViewSettings;

struct DialogHarness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    /// Botones del último frame: nombre y rect (en puntos).
    buttons: Vec<(String, Rect)>,
    /// Campos numéricos del último frame, de arriba abajo y de izquierda a derecha.
    numbers: Vec<Rect>,
    /// Texturas dibujadas en el último frame.
    textures: Vec<TextureId>,
    /// Textos dibujados en el último frame.
    texts: Vec<String>,
    /// Colores de los rectángulos rellenos del último frame.
    fills: Vec<Color32>,
}

impl DialogHarness {
    /// La app con Pixel UI encendido o apagado y una pestaña nueva de 64×64.
    fn new(pixel_ui: bool) -> Self {
        let view = ViewSettings {
            pixel_ui,
            ..Default::default()
        };
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
        app.new_document();
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let mut h = Self {
            ctx,
            app,
            time: 0.0,
            buttons: Vec::new(),
            numbers: Vec::new(),
            textures: Vec::new(),
            texts: Vec::new(),
            fills: Vec::new(),
        };
        // La escala de Pixel UI rige desde el segundo frame.
        h.settle();
        h
    }

    /// Unos frames sin eventos: los diálogos se miden en el primero y después aparecen de a poco.
    fn settle(&mut self) {
        for _ in 0..6 {
            self.frame(vec![]);
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.pixel.update(&ctx, &app.view);
            app.update_canvas_size(&ctx);
            app.update_tilemap_dialogs(&ctx);
            app.show_dialogs(&ctx);
            app.update_palette_windows(&ctx, false);
            app.update_grid_dialog(&ctx);
            app.show_slice_properties(&ctx);
        });
        // Las texturas (fuentes y skin) no se suben a ninguna GPU.
        output.textures_delta.clear();
        self.textures.clear();
        self.texts.clear();
        self.fills.clear();
        for clipped in &output.shapes {
            self.collect(&clipped.shape);
        }
        let nodes = output
            .platform_output
            .accesskit_update
            .map(|update| update.nodes)
            .unwrap_or_default();
        let bounds = |n: &egui::accesskit::Node| {
            let b = n.bounds()?;
            Some(Rect::from_min_max(
                Pos2::new(b.x0 as f32, b.y0 as f32),
                Pos2::new(b.x1 as f32, b.y1 as f32),
            ))
        };
        self.buttons = nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
            .filter_map(|(_, n)| Some((n.label()?.to_string(), bounds(n)?)))
            .collect();
        self.numbers = nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::SpinButton)
            .filter_map(|(_, n)| bounds(n))
            .collect();
        self.numbers
            .sort_by(|a, b| (a.min.y, a.min.x).partial_cmp(&(b.min.y, b.min.x)).unwrap());
    }

    fn collect(&mut self, shape: &Shape) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|s| self.collect(s)),
            Shape::Mesh(mesh) => self.textures.push(mesh.texture_id),
            Shape::Text(text) => self.texts.push(text.galley.text().to_owned()),
            Shape::Rect(rect) => self.fills.push(rect.fill),
            _ => {}
        }
    }

    fn button(&self, label: &str) -> Rect {
        self.buttons
            .iter()
            .find(|(l, _)| l == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
    }

    fn click(&mut self, label: &str) {
        let pos = self.button(label).center();
        let press = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![press(true)]);
        self.frame(vec![press(false)]);
        self.frame(vec![]);
    }

    /// Arrastra el campo numérico `index` `dx` puntos a la derecha.
    fn drag_number(&mut self, index: usize, dx: f32) {
        let from = self.numbers[index].center();
        let to = from + Vec2::new(dx, 0.0);
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        for i in 1..=4 {
            let at = from + Vec2::new(dx * i as f32 / 4.0, 0.0);
            self.frame(vec![Event::PointerMoved(at)]);
        }
        self.frame(vec![press(to, false)]);
        self.frame(vec![]);
    }

    fn key(&mut self, key: Key) {
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        self.frame(vec![event(true), event(false)]);
        self.frame(vec![]);
    }

    /// Si se dibujó la pieza `name` del skin en el último frame.
    fn drew_piece(&self, name: &str) -> bool {
        let skin = self.app.pixel.skin().expect("Pixel UI is on");
        let id = skin.piece(name, WidgetState::Normal).unwrap().texture.id();
        self.textures.contains(&id)
    }

    fn drew_text(&self, text: &str) -> bool {
        self.texts.iter().any(|t| t == text)
    }

    /// Si hay un área de egui con `id` (los diálogos pixel lo son).
    fn has_area(&self, id: &str) -> bool {
        self.ctx
            .memory(|m| m.area_rect(egui::Id::new(id)))
            .is_some()
    }

    /// El rect del diálogo pixel `id` en el último frame.
    fn dialog_rect(&self, id: &str) -> Rect {
        self.ctx
            .memory(|m| m.area_rect(egui::Id::new(id)))
            .unwrap_or_else(|| panic!("no dialog {id:?}"))
    }

    /// Aprieta en `from`, arrastra en 4 pasos hasta `to` y suelta.
    fn drag(&mut self, from: Pos2, to: Pos2) {
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![press(from, true)]);
        for i in 1..=4 {
            let at = from + (to - from) * (i as f32 / 4.0);
            self.frame(vec![Event::PointerMoved(at)]);
        }
        self.frame(vec![press(to, false)]);
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn sprite_size(&self) -> (u32, u32) {
        let s = self.app.doc.editor.sprite();
        (s.width, s.height)
    }
}

// --- Canvas Size ---

fn canvas_size(pixel_ui: bool) -> DialogHarness {
    let mut h = DialogHarness::new(pixel_ui);
    h.app.open_canvas_size();
    h.settle();
    assert!(h.app.canvas_size.is_some());
    h
}

#[test]
fn with_pixel_ui_canvas_size_is_a_pixel_dialog() {
    let h = canvas_size(true);
    assert!(h.has_area("canvas_size"), "the pixel dialog");
    assert!(
        !h.buttons.iter().any(|(l, _)| l == "Close window"),
        "no egui window"
    );
    assert!(h.drew_piece("window"), "the dialog frame");
    assert!(h.drew_piece("button"), "pixel buttons");
    assert!(h.drew_text("Canvas Size"), "the title: {:?}", h.texts);
    for label in ["OK", "Cancel", "Close"] {
        h.button(label);
    }
    assert_eq!(h.numbers.len(), 6, "width, height and the four borders");
}

#[test]
fn with_pixel_ui_canvas_size_anchors_top_right_like_the_plain_window() {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0));
    let h = canvas_size(true);
    let pixel = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("canvas_size")))
        .expect("el diálogo pixel");
    // El ancla de `plain_window`: RIGHT_TOP con desfase (-240, 40).
    let (right, top) = (screen.right() - 240.0, screen.top() + 40.0);
    assert!(
        (pixel.right() - right).abs() < 1.0,
        "borde derecho: {pixel:?}"
    );
    assert!((pixel.top() - top).abs() < 1.0, "borde superior: {pixel:?}");
    assert!(pixel.right() <= screen.right() - 200.0, "{pixel:?}");
    assert!(
        !pixel.contains(screen.center()),
        "no tapa el centro (la vista previa): {pixel:?}"
    );
}

#[test]
fn with_pixel_ui_the_anchor_grid_uses_the_skin_colors() {
    let h = canvas_size(true);
    let colors = *h.app.pixel.skin().unwrap().colors();
    assert!(
        h.fills.contains(&colors.panel),
        "the anchor cells: {:?}",
        h.fills
    );
    assert!(h.fills.contains(&colors.accent), "the anchor square");
    assert!(
        !h.fills.contains(&crate::theme::ACCENT),
        "no dark theme accent"
    );
}

#[test]
fn with_pixel_ui_changing_the_width_and_ok_resizes_the_canvas() {
    let mut h = canvas_size(true);
    assert_eq!(h.sprite_size(), (64, 64));
    // Ancla al centro: 20 puntos de arrastre a 0.25 por punto son 5 px más.
    h.drag_number(0, 20.0);
    let width = h.app.canvas_size.as_ref().unwrap().width();
    assert!(width > 64, "the width field works: {width}");
    let ab = h.app.doc.editor.artboard();
    let (x, y) = ab.position;
    let left = h.app.canvas_size.as_ref().unwrap().left;
    h.click("OK");
    assert!(h.app.canvas_size.is_none(), "the dialog is gone");
    assert_eq!(h.sprite_size(), (width as u32, 64));
    assert_eq!(
        h.app.doc.editor.artboard().position,
        (x - left, y),
        "the content stays in place in the world"
    );
    assert!(h.app.doc.editor.history.can_undo());
}

#[test]
fn with_pixel_ui_enter_confirms_canvas_size() {
    let mut h = canvas_size(true);
    h.app.canvas_size.as_mut().unwrap().set_height(70);
    h.key(Key::Enter);
    assert!(h.app.canvas_size.is_none());
    assert_eq!(h.sprite_size(), (64, 70));
}

#[test]
fn with_pixel_ui_close_cancel_and_escape_leave_the_canvas_alone() {
    for how in ["Close", "Cancel", "Escape"] {
        let mut h = canvas_size(true);
        h.app.canvas_size.as_mut().unwrap().set_width(80);
        match how {
            "Escape" => h.key(Key::Escape),
            label => h.click(label),
        }
        assert!(h.app.canvas_size.is_none(), "{how} closes it");
        assert_eq!(h.sprite_size(), (64, 64), "{how} changes nothing");
        assert!(!h.app.doc.editor.history.can_undo(), "{how}");
    }
}

#[test]
fn without_pixel_ui_canvas_size_is_the_egui_window_as_always() {
    let mut h = canvas_size(false);
    assert!(h.app.pixel.skin().is_none());
    assert!(h.drew_text("Canvas Size"), "{:?}", h.texts);
    h.button("Close window");
    assert!(!h.has_area("canvas_size"), "no pixel dialog");
    h.app.canvas_size.as_mut().unwrap().set_width(80);
    h.click("OK");
    assert!(h.app.canvas_size.is_none());
    assert_eq!(h.sprite_size(), (80, 64));
    let mut h = canvas_size(false);
    h.app.canvas_size.as_mut().unwrap().set_width(80);
    h.key(Key::Escape);
    assert!(h.app.canvas_size.is_none());
    assert_eq!(h.sprite_size(), (64, 64));
}

// --- New Tilemap Layer ---

fn new_tilemap(pixel_ui: bool) -> DialogHarness {
    let mut h = DialogHarness::new(pixel_ui);
    h.app.new_tilemap = Some(crate::tilemap_dialog::NewTilemapDialog::new(
        &h.app.doc.editor,
    ));
    h.settle();
    assert!(h.app.new_tilemap.is_some());
    h
}

/// Cuántas capas tilemap hay y los tilesets del sprite (nombre y tamaño de tile).
type Tilemaps = (usize, Vec<(String, (u32, u32))>);

fn tilemaps(h: &DialogHarness) -> Tilemaps {
    let s = h.app.doc.editor.sprite();
    let layers = s
        .layers
        .iter()
        .filter(|l| l.kind == cs_core::LayerKind::Tilemap)
        .count();
    let sets = s
        .tilesets
        .iter()
        .map(|t| (t.name.clone(), t.tile_size))
        .collect();
    (layers, sets)
}

#[test]
fn with_pixel_ui_new_tilemap_layer_is_a_pixel_dialog() {
    let h = new_tilemap(true);
    assert!(h.has_area("new_tilemap_layer"), "the pixel dialog");
    assert!(
        !h.buttons.iter().any(|(l, _)| l == "Close window"),
        "no egui window"
    );
    assert!(h.drew_piece("window"), "the dialog frame");
    assert!(h.drew_piece("button"), "pixel buttons");
    assert!(h.drew_text("New Tilemap Layer"), "the title: {:?}", h.texts);
    for label in ["OK", "Cancel", "Close"] {
        h.button(label);
    }
    assert!(h.drew_text("Tileset 1"), "the name field: {:?}", h.texts);
    assert_eq!(h.numbers.len(), 3, "tile width, tile height and base index");
}

#[test]
fn with_pixel_ui_ok_and_enter_create_the_tilemap_layer() {
    for how in ["OK", "Enter"] {
        let mut h = new_tilemap(true);
        assert_eq!(tilemaps(&h), (0, vec![]));
        // Arrastrar el campo del ancho del tile lo agranda.
        h.drag_number(0, 20.0);
        let size = h.app.new_tilemap.as_ref().unwrap().tile_size;
        assert!(size.0 > 16, "the tile width field works: {size:?}");
        match how {
            "Enter" => h.key(Key::Enter),
            label => h.click(label),
        }
        assert!(h.app.new_tilemap.is_none(), "{how} closes it");
        assert_eq!(
            tilemaps(&h),
            (1, vec![("Tileset 1".to_string(), size)]),
            "{how}"
        );
        assert!(h.app.message.is_none(), "{how}: no error");
    }
}

#[test]
fn with_pixel_ui_close_cancel_and_escape_create_no_tilemap_layer() {
    for how in ["Close", "Cancel", "Escape"] {
        let mut h = new_tilemap(true);
        match how {
            "Escape" => h.key(Key::Escape),
            label => h.click(label),
        }
        assert!(h.app.new_tilemap.is_none(), "{how} closes it");
        assert_eq!(tilemaps(&h), (0, vec![]), "{how} changes nothing");
        assert!(!h.app.doc.editor.history.can_undo(), "{how}");
    }
}

#[test]
fn without_pixel_ui_new_tilemap_layer_is_the_egui_window_as_always() {
    let mut h = new_tilemap(false);
    assert!(h.drew_text("New Tilemap Layer"), "{:?}", h.texts);
    h.button("Close window");
    assert!(!h.has_area("new_tilemap_layer"), "no pixel dialog");
    h.click("OK");
    assert!(h.app.new_tilemap.is_none());
    assert_eq!(tilemaps(&h).0, 1);
    let mut h = new_tilemap(false);
    h.key(Key::Escape);
    assert!(h.app.new_tilemap.is_none());
    assert_eq!(tilemaps(&h), (0, vec![]));
}

// --- Convert to Tilemap ---

fn convert_tilemap(pixel_ui: bool) -> DialogHarness {
    let mut h = DialogHarness::new(pixel_ui);
    let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    h.app.doc.editor.execute(cs_commands::Command::DrawPixels {
        layer: cs_core::LayerId(1),
        frame: 0,
        pixels: vec![(0, 0, red)],
    });
    h.app.convert_tilemap = Some(crate::tilemap_dialog::TileSizeDialog::new(
        &h.app.doc.editor,
    ));
    h.settle();
    assert!(h.app.convert_tilemap.is_some());
    h
}

/// La capa activa es tilemap y los tamaños de tile de los tilesets.
fn converted(h: &DialogHarness) -> (bool, Vec<(u32, u32)>) {
    let e = &h.app.doc.editor;
    let s = e.sprite();
    let tilemap = s
        .layers
        .iter()
        .any(|l| l.id == e.layer && l.kind == cs_core::LayerKind::Tilemap);
    (tilemap, s.tilesets.iter().map(|t| t.tile_size).collect())
}

#[test]
fn with_pixel_ui_convert_to_tilemap_is_a_pixel_dialog() {
    let h = convert_tilemap(true);
    assert!(h.has_area("convert_to_tilemap"), "the pixel dialog");
    assert!(
        !h.buttons.iter().any(|(l, _)| l == "Close window"),
        "no egui window"
    );
    assert!(h.drew_piece("window"), "the dialog frame");
    assert!(h.drew_piece("button"), "pixel buttons");
    assert!(
        h.drew_text("Convert to Tilemap"),
        "the title: {:?}",
        h.texts
    );
    for label in ["OK", "Cancel", "Close"] {
        h.button(label);
    }
    assert_eq!(h.numbers.len(), 2, "tile width and height");
}

#[test]
fn with_pixel_ui_ok_and_enter_convert_the_layer() {
    for how in ["OK", "Enter"] {
        let mut h = convert_tilemap(true);
        assert_eq!(converted(&h), (false, vec![]));
        h.drag_number(1, 20.0);
        let size = h.app.convert_tilemap.as_ref().unwrap().tile_size;
        assert!(size.1 > 16, "the tile height field works: {size:?}");
        match how {
            "Enter" => h.key(Key::Enter),
            label => h.click(label),
        }
        assert!(h.app.convert_tilemap.is_none(), "{how} closes it");
        assert_eq!(converted(&h), (true, vec![size]), "{how}");
        assert!(h.app.message.is_none(), "{how}: no error");
    }
}

#[test]
fn with_pixel_ui_close_cancel_and_escape_do_not_convert() {
    for how in ["Close", "Cancel", "Escape"] {
        let mut h = convert_tilemap(true);
        match how {
            "Escape" => h.key(Key::Escape),
            label => h.click(label),
        }
        assert!(h.app.convert_tilemap.is_none(), "{how} closes it");
        assert_eq!(converted(&h), (false, vec![]), "{how} changes nothing");
    }
}

#[test]
fn without_pixel_ui_convert_to_tilemap_is_the_egui_window_as_always() {
    let mut h = convert_tilemap(false);
    assert!(h.drew_text("Convert to Tilemap"), "{:?}", h.texts);
    h.button("Close window");
    assert!(!h.has_area("convert_to_tilemap"), "no pixel dialog");
    h.click("OK");
    assert!(h.app.convert_tilemap.is_none());
    assert_eq!(converted(&h), (true, vec![(16, 16)]));
    let mut h = convert_tilemap(false);
    h.key(Key::Escape);
    assert!(h.app.convert_tilemap.is_none());
    assert_eq!(converted(&h), (false, vec![]));
}

// --- Mover los diálogos pixel desde la barra de título ---

/// "Unsaved changes" con Pixel UI: la pestaña con un cambio, a punto de cerrarse.
fn unsaved() -> DialogHarness {
    let mut h = DialogHarness::new(true);
    h.app.doc.editor.execute(cs_commands::Command::DrawPixels {
        layer: cs_core::LayerId(1),
        frame: 0,
        pixels: vec![(
            0,
            0,
            cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0)),
        )],
    });
    h.app.request_close_tab(0);
    h.settle();
    assert!(h.app.pending.is_some());
    h
}

/// Un punto de la barra de título rosa de `rect`, lejos del botón de cerrar.
fn title_bar_point(rect: Rect) -> Pos2 {
    rect.left_top() + Vec2::new(20.0, 4.0)
}

fn assert_near(a: Rect, b: Rect, what: &str) {
    assert!(
        (a.min - b.min).length() < 0.5 && (a.size() - b.size()).length() < 0.5,
        "{what}: {a:?} != {b:?}"
    );
}

#[test]
fn with_pixel_ui_dragging_the_title_bar_moves_the_dialog_by_the_drag_delta() {
    for (id, mut h) in [
        ("unsaved_changes", unsaved()),
        ("canvas_size", canvas_size(true)),
    ] {
        let before = h.dialog_rect(id);
        let from = title_bar_point(before);
        let delta = Vec2::new(-150.0, 60.0);
        h.drag(from, from + delta);
        assert_near(h.dialog_rect(id), before.translate(delta), id);
        // Se queda donde quedó mientras siga abierto.
        h.settle();
        assert_near(h.dialog_rect(id), before.translate(delta), id);
        // Y se puede volver a mover desde ahí.
        let from = title_bar_point(h.dialog_rect(id));
        h.drag(from, from + Vec2::new(30.0, -20.0));
        assert_near(
            h.dialog_rect(id),
            before.translate(delta + Vec2::new(30.0, -20.0)),
            id,
        );
    }
}

#[test]
fn with_pixel_ui_dragging_inside_the_dialog_body_does_not_move_it() {
    let mut h = unsaved();
    let before = h.dialog_rect("unsaved_changes");
    // El margen izquierdo y el texto de la pregunta: cuerpo, no barra.
    for from in [
        Pos2::new(before.left() + 3.0, before.center().y),
        Pos2::new(before.center().x, before.top() + 22.0),
    ] {
        h.drag(from, from + Vec2::new(-120.0, 80.0));
        assert_near(h.dialog_rect("unsaved_changes"), before, "body");
    }
    assert!(h.app.pending.is_some(), "still open");
    // Canvas Size: arrastrar en el margen tampoco lo mueve ni cambia los campos.
    let mut h = canvas_size(true);
    let before = h.dialog_rect("canvas_size");
    let width = h.app.canvas_size.as_ref().unwrap().width();
    let from = Pos2::new(before.left() + 3.0, before.center().y);
    h.drag(from, from + Vec2::new(-120.0, 80.0));
    assert_near(h.dialog_rect("canvas_size"), before, "canvas size body");
    assert_eq!(h.app.canvas_size.as_ref().unwrap().width(), width);
}

#[test]
fn with_pixel_ui_dragging_from_the_close_button_does_not_move_and_it_still_closes() {
    let mut h = unsaved();
    let before = h.dialog_rect("unsaved_changes");
    let from = h.button("Close").center();
    h.drag(from, from + Vec2::new(-150.0, 60.0));
    assert_near(h.dialog_rect("unsaved_changes"), before, "close button");
    assert!(h.app.pending.is_some(), "released away from the button");
    h.click("Close");
    assert_eq!(h.app.pending, None, "closed");
    assert!(h.app.any_dirty(), "the tab is still there, unsaved");
}

#[test]
fn with_pixel_ui_a_dialog_cannot_be_dragged_off_the_screen() {
    let mut h = canvas_size(true);
    let screen = h.ctx.content_rect();
    let size = h.dialog_rect("canvas_size").size();
    // Muy arriba a la izquierda: queda contra la esquina.
    let from = title_bar_point(h.dialog_rect("canvas_size"));
    h.drag(from, from - Vec2::new(3000.0, 3000.0));
    let rect = h.dialog_rect("canvas_size");
    assert_near(rect, Rect::from_min_size(screen.min, size), "top left");
    // Muy abajo a la derecha: queda contra la otra esquina.
    let from = title_bar_point(rect);
    h.drag(from, from + Vec2::new(3000.0, 3000.0));
    let rect = h.dialog_rect("canvas_size");
    assert_near(
        rect,
        Rect::from_min_size(screen.max - size, size),
        "bottom right",
    );
    // Desde ahí se mueve enseguida: la posición guardada no quedó afuera.
    let from = title_bar_point(rect);
    h.drag(from, from - Vec2::new(50.0, 50.0));
    assert_near(
        h.dialog_rect("canvas_size"),
        Rect::from_min_size(screen.max - size - Vec2::splat(50.0), size),
        "back in",
    );
}

#[test]
fn with_pixel_ui_a_reopened_dialog_starts_at_its_default_place() {
    let mut h = canvas_size(true);
    let home = h.dialog_rect("canvas_size");
    let from = title_bar_point(home);
    h.drag(from, from + Vec2::new(-300.0, 200.0));
    assert!(h.dialog_rect("canvas_size").min != home.min, "moved");
    h.click("Cancel");
    assert!(h.app.canvas_size.is_none());
    h.settle();
    h.app.open_canvas_size();
    h.settle();
    assert_near(h.dialog_rect("canvas_size"), home, "top right again");
    // El de Unsaved, centrado.
    let mut h = unsaved();
    let home = h.dialog_rect("unsaved_changes");
    let from = title_bar_point(home);
    h.drag(from, from + Vec2::new(100.0, 100.0));
    h.click("Cancel");
    assert_eq!(h.app.pending, None);
    h.settle();
    h.app.request_close_tab(0);
    h.settle();
    assert_near(h.dialog_rect("unsaved_changes"), home, "centered again");
}

// --- "New palette" en un sprite indexado ---

const REPLACE_PALETTE: &str = "Replace the palette? Pixels using removed colors will change.";

/// Un sprite indexado con un píxel del color #5, la paleta desbloqueada y "New palette" pedida.
fn new_palette_on_indexed(pixel_ui: bool) -> DialogHarness {
    use cs_core::{ColorMode, LayerId, PixelValue, Sprite};
    let mut h = DialogHarness::new(pixel_ui);
    let sprite = Sprite::new(
        8,
        8,
        ColorMode::Indexed {
            transparent_index: 0,
        },
    );
    let editor = &mut h.app.doc.editor;
    *editor = crate::editor::Editor::with_sprite(sprite);
    editor.execute(cs_commands::Command::DrawPixels {
        layer: LayerId(1),
        frame: 0,
        pixels: vec![(1, 1, PixelValue::Index(5))],
    });
    editor.primary = PixelValue::Index(3);
    editor.palette_locked = false;
    h.app.view.palette_locked = false;
    let ctx = h.ctx.clone();
    h.app
        .palette_action(&ctx, crate::panels::PaletteAction::NewPalette);
    h.settle();
    h
}

fn color_at_1_1(h: &DialogHarness) -> cs_core::Rgba {
    cs_render::composite_pixel(h.app.doc.editor.sprite(), 0, 1, 1)
}

#[test]
fn new_palette_on_an_indexed_sprite_asks_and_cancel_keeps_everything() {
    for pixel_ui in [true, false] {
        let mut h = new_palette_on_indexed(pixel_ui);
        let palette = cs_core::Palette::db32();
        assert!(
            h.texts.iter().any(|t| t == REPLACE_PALETTE),
            "pixel_ui {pixel_ui}: {:?}",
            h.texts
        );
        h.button("OK");
        h.button("Cancel");
        assert_eq!(h.has_area("palette_confirm"), pixel_ui, "the pixel dialog");
        if pixel_ui {
            assert!(h.drew_piece("window") && h.drew_piece("button"));
        }
        assert_eq!(h.app.doc.editor.sprite().palette, palette, "not yet");
        h.click("Cancel");
        assert!(h.app.palette_confirm.is_none());
        assert_eq!(h.app.doc.editor.sprite().palette, palette);
        assert_eq!(color_at_1_1(&h), palette.colors()[5]);
        assert!(!h.texts.iter().any(|t| t == REPLACE_PALETTE));
        // Esc también cancela.
        let ctx = h.ctx.clone();
        h.app
            .palette_action(&ctx, crate::panels::PaletteAction::NewPalette);
        h.settle();
        h.key(Key::Escape);
        assert!(h.app.palette_confirm.is_none());
        assert_eq!(h.app.doc.editor.sprite().palette, palette);
    }
}

#[test]
fn new_palette_on_an_indexed_sprite_ok_applies_and_undo_restores() {
    for pixel_ui in [true, false] {
        let mut h = new_palette_on_indexed(pixel_ui);
        let palette = cs_core::Palette::db32();
        h.click("OK");
        assert!(h.app.palette_confirm.is_none());
        assert_eq!(
            h.app.doc.editor.sprite().palette.colors(),
            &[palette.colors()[3]]
        );
        assert_eq!(color_at_1_1(&h), cs_core::Rgba::TRANSPARENT, "#5 is gone");
        h.app.doc.editor.undo();
        assert_eq!(h.app.doc.editor.sprite().palette, palette);
        assert_eq!(color_at_1_1(&h), palette.colors()[5]);
    }
}

#[test]
fn new_palette_does_nothing_with_the_palette_locked() {
    let mut h = new_palette_on_indexed(true);
    h.click("Cancel");
    h.app.doc.editor.palette_locked = true;
    h.app.view.palette_locked = true;
    let ctx = h.ctx.clone();
    h.app
        .palette_action(&ctx, crate::panels::PaletteAction::NewPalette);
    h.settle();
    assert!(h.app.palette_confirm.is_none());
    assert!(!h.texts.iter().any(|t| t == REPLACE_PALETTE));
    assert_eq!(h.app.doc.editor.sprite().palette, cs_core::Palette::db32());
}

/// Cada ventana se abre con su id en los dos modos; con Pixel UI, con el marco `window` del skin.
#[test]
fn every_window_is_the_app_window() {
    type Open = fn(&mut ChimpApp, &egui::Context);
    let windows: [(&str, Open); 5] = [
        ("palette_presets_window", |app, ctx| {
            app.palette_action(ctx, crate::panels::PaletteAction::OpenPresets)
        }),
        ("export_window", |app, _| app.open_export()),
        ("grid_settings_window", |app, _| {
            app.grid_dialog = Some(crate::view::GridDialog::new(&app.doc.editor, &app.view))
        }),
        ("message_window", |app, _| {
            app.show_message("Hello", "World")
        }),
        ("slice_properties_window", |app, _| {
            app.doc.editor.add_slice();
            app.doc.canvas.slice_properties = true;
        }),
    ];
    for (id, open) in windows {
        for pixel_ui in [true, false] {
            let mut h = DialogHarness::new(pixel_ui);
            let ctx = h.ctx.clone();
            open(&mut h.app, &ctx);
            h.settle();
            assert!(h.has_area(id), "{id}: pixel_ui {pixel_ui}");
            if pixel_ui {
                assert!(h.drew_piece("window"), "{id}");
            }
        }
    }
}
