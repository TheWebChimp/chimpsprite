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
