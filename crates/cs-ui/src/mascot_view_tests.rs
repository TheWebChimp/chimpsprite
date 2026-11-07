//! Chimpy en la app entera (con Pixel UI y una pestaña), manejada con eventos crudos. Los botones
//! se buscan por su nombre en el árbol de accesibilidad.

use eframe::egui::{self, Event, PointerButton, Pos2, RawInput, Rect, Vec2};

use crate::ChimpApp;
use crate::mascot::{self, MascotSettings};
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::view::ViewSettings;

struct Harness {
    ctx: egui::Context,
    app: ChimpApp,
    time: f64,
    canvas: Option<Rect>,
    /// Lo que tiene nombre en el último frame: nombre y rect.
    nodes: Vec<(String, Rect)>,
}

impl Harness {
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
            canvas: None,
            nodes: Vec::new(),
        };
        h.frames(3);
        h
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
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
        let mut canvas = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            app.pixel.update(&ctx, &app.view);
            app.handle_shortcuts(&ctx);
            canvas = app.layout(ui, false);
        });
        output.textures_delta.clear();
        self.canvas = canvas;
        self.nodes = output
            .platform_output
            .accesskit_update
            .map(|update| update.nodes)
            .unwrap_or_default()
            .iter()
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let rect = Rect::from_min_max(
                    Pos2::new(b.x0 as f32, b.y0 as f32),
                    Pos2::new(b.x1 as f32, b.y1 as f32),
                );
                // Los textos (Label) llevan el suyo en `value`.
                let name = n.label().or_else(|| n.value())?;
                Some((name.to_string(), rect))
            })
            .collect();
    }

    fn node(&self, label: &str) -> Option<Rect> {
        self.nodes.iter().find(|(l, _)| l == label).map(|(_, r)| *r)
    }

    fn click(&mut self, pos: Pos2) {
        let press = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![press(true)]);
        self.frame(vec![press(false)]);
    }

    fn click_node(&mut self, label: &str) {
        let rect = self
            .node(label)
            .unwrap_or_else(|| panic!("no {label:?} in {:?}", self.nodes));
        self.click(rect.center());
    }

    /// Cinco clics al mono que corre.
    fn poke_five(&mut self) {
        let runner = self.app.mascot_view.runner.expect("the running chimp");
        for _ in 0..mascot::POKES {
            self.click(runner.center());
        }
    }
}

#[test]
fn five_clicks_on_the_running_chimp_wake_chimpy_up() {
    let mut h = Harness::new(true);
    assert!(h.node(mascot::NAME).is_none(), "asleep by default");
    h.poke_five();
    assert!(h.app.mascot.settings.enabled && h.app.mascot.settings.discovered);
    h.frames(10);
    let chimpy = h.node(mascot::NAME).expect("Chimpy hangs on the canvas");
    let canvas = h.canvas.expect("the canvas");
    // Colgado arriba a la izquierda del lienzo, debajo de la regla.
    assert!(canvas.contains(chimpy.center()), "{chimpy:?} in {canvas:?}");
    assert!(chimpy.top() >= canvas.top() + crate::pixel_rulers::RULER_PX - 0.5);
    assert!(chimpy.left() < canvas.left() + 40.0);
    let hello = h
        .nodes
        .iter()
        .find(|(l, _)| l.starts_with("Hi! I'm Chimpy"))
        .map(|(_, r)| *r)
        .expect("the greeting");
    assert!(
        hello.left() >= chimpy.right() - 1.0,
        "the bubble is on his right"
    );
    // OK lo despide.
    h.click_node("OK");
    h.frames(10);
    assert!(h.node(mascot::NAME).is_none());
    assert!(h.app.mascot.settings.enabled, "he stays awake");
}

#[test]
fn yes_runs_the_suggestion() {
    let mut h = Harness::new(true);
    assert!(!h.app.doc.editor.onion_enabled);
    let keys = h.app.keymap.clone();
    h.app.mascot.force("walk-cycle", h.time, &keys);
    h.frames(10);
    h.click_node("Yes");
    assert!(h.app.doc.editor.onion_enabled);
    h.frames(10);
    assert!(h.node(mascot::NAME).is_none(), "he leaves");
}

#[test]
fn mirror_selection_and_play_in_place_suggestions() {
    let mut h = Harness::new(true);
    let ctx = h.ctx.clone();
    h.app
        .accept_suggestion(&ctx, mascot::Suggestion::MirrorSelection);
    assert!(h.app.doc.editor.options.mirror_selection);
    // Con un solo frame no hay nada que reproducir.
    h.app
        .accept_suggestion(&ctx, mascot::Suggestion::PlayInPlace);
    let active = h.app.doc.editor.active;
    assert!(!h.app.doc.editor.is_playing_in_place(active));
    h.app.doc.editor.duplicate_frame();
    h.app
        .accept_suggestion(&ctx, mascot::Suggestion::PlayInPlace);
    assert!(h.app.doc.editor.is_playing_in_place(active));
    // Si ya se reproduce, no la apaga.
    h.app
        .accept_suggestion(&ctx, mascot::Suggestion::PlayInPlace);
    assert!(h.app.doc.editor.is_playing_in_place(active));
}

#[test]
fn clicking_chimpy_asks_for_another_tip() {
    let mut h = Harness::new(true);
    h.app.mascot = mascot::Mascot::new(MascotSettings {
        enabled: true,
        discovered: true,
        ..Default::default()
    });
    let keys = h.app.keymap.clone();
    h.app.mascot.ask(h.time, &keys);
    h.frames(10);
    let first = h.app.mascot.tip().unwrap().id.clone();
    h.click_node(mascot::NAME);
    let second = h.app.mascot.tip().unwrap().id.clone();
    assert_ne!(first, second);
    assert!(second.starts_with("dyk."), "{second}");
}

#[test]
fn without_pixel_ui_chimpy_stays_hidden() {
    let mut h = Harness::new(false);
    let keys = h.app.keymap.clone();
    h.app.mascot.force("hello", h.time, &keys);
    h.frames(10);
    assert!(h.node(mascot::NAME).is_none());
}

#[test]
fn turning_him_off_hides_him() {
    let mut h = Harness::new(true);
    h.poke_five();
    h.frames(10);
    assert!(h.node(mascot::NAME).is_some());
    h.app.mascot.set_enabled(false);
    h.frames(2);
    assert!(h.node(mascot::NAME).is_none());
    assert!(h.app.mascot.settings.discovered, "Help still offers him");
}

#[test]
fn yes_to_onion_skin_never_turns_it_off() {
    let mut h = Harness::new(true);
    let ctx = h.ctx.clone();
    h.app.doc.editor.onion_enabled = true;
    h.app.accept_suggestion(&ctx, mascot::Suggestion::OnionSkin);
    assert!(h.app.doc.editor.onion_enabled);
}

#[test]
fn help_offers_chimpy_only_once_he_was_found() {
    let mut h = Harness::new(true);
    h.click_node("Help");
    h.frames(4);
    assert!(h.node("About ChimpSprite").is_some(), "Help is open");
    assert!(h.node(mascot::NAME).is_none());

    let mut h = Harness::new(true);
    h.app.mascot = mascot::Mascot::new(MascotSettings {
        enabled: true,
        discovered: true,
        ..Default::default()
    });
    h.click_node("Help");
    h.frames(4);
    h.click_node(mascot::NAME);
    assert!(!h.app.mascot.settings.enabled, "the check turns him off");
    assert!(h.app.mascot.settings.discovered);
}
