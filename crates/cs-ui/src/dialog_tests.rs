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
    /// Campos de texto del último frame: nombre y rect.
    inputs: Vec<(String, Rect)>,
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
            inputs: Vec::new(),
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
            app.update_font_dialog(&ctx);
            app.update_build_font_dialog(&ctx);
            app.update_new_variant_dialog(&ctx);
            app.update_import_font_dialog(&ctx);
            app.update_glyph_sheet_dialog(&ctx);
            app.update_model_dialog(&ctx);
            app.update_bake_dialog(&ctx);
            app.update_gif_dialog(&ctx);
            app.update_symbol_dialog(&ctx);
            app.update_sheet_dialog(&ctx);
            app.update_expand_dialog(&ctx);
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
            .filter(|(_, n)| {
                matches!(
                    n.role(),
                    egui::accesskit::Role::Button | egui::accesskit::Role::RadioButton
                )
            })
            .filter_map(|(_, n)| Some((n.label()?.to_string(), bounds(n)?)))
            .collect();
        self.numbers = nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::SpinButton)
            .filter_map(|(_, n)| bounds(n))
            .collect();
        self.numbers
            .sort_by(|a, b| (a.min.y, a.min.x).partial_cmp(&(b.min.y, b.min.x)).unwrap());
        self.inputs = nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::TextInput)
            .filter_map(|(_, n)| Some((n.label()?.to_string(), bounds(n)?)))
            .collect();
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
fn with_pixel_ui_the_anchor_grid_is_the_design_one() {
    let h = canvas_size(true);
    assert!(h.drew_piece("anchor.grid"), "the box");
    assert!(h.drew_piece("anchor.center"), "the monkey on the anchor");
    for arrow in ["anchor.n", "anchor.ne", "anchor.e", "anchor.se", "anchor.s"] {
        assert!(h.drew_piece(arrow), "the arrow {arrow}");
    }
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

// --- New Font ---

#[test]
fn new_font_creates_a_font_artboard() {
    let mut h = DialogHarness::new(true);
    h.app.new_font = Some(Default::default());
    h.settle();
    assert!(h.drew_text("New Font"));
    h.click("OK");
    h.settle();
    assert!(h.app.new_font.is_none());
    let font = h
        .app
        .doc
        .editor
        .font()
        .expect("the active artboard is a font");
    assert_eq!(font.name, "New Font");
    assert_eq!(h.app.doc.editor.sprite().width, 16);
    // Y se abre el panel Font del dock.
    assert_eq!(
        crate::dock::open_panel(&h.ctx),
        Some(crate::dock::DockPanel::Font)
    );
}

#[test]
fn new_font_from_home_opens_a_new_document() {
    let mut h = DialogHarness::new(true);
    h.app.request_close_tab(0);
    h.settle();
    assert!(!h.app.has_doc());
    h.app.new_font = Some(Default::default());
    h.settle();
    h.click("OK");
    h.settle();
    assert!(h.app.has_doc());
    let font = h
        .app
        .doc
        .editor
        .font()
        .expect("the new document's artboard is a font");
    assert_eq!(font.name, "New Font");
}

// --- New 3D Model ---

#[test]
fn new_model_creates_a_model_artboard() {
    let mut h = DialogHarness::new(true);
    h.app.new_model = Some(Default::default());
    h.settle();
    assert!(h.drew_text("New 3D Model"));
    h.click("OK");
    h.settle();
    assert!(h.app.new_model.is_none());
    let editor = &h.app.doc.editor;
    assert!(editor.is_model(), "the active artboard is a model");
    assert_eq!(editor.artboard().name, "Model");
    assert_eq!((editor.sprite().width, editor.sprite().height), (128, 128));
    // Y se abre su pestaña, con el panel 3D fijo: el panel abierto del dock (el del lienzo)
    // no cambia.
    assert!(editor.model_edit_on());
    assert_eq!(crate::dock::open_panel(&h.ctx), None);
}

#[test]
fn new_model_from_home_opens_a_new_document() {
    let mut h = DialogHarness::new(true);
    h.app.request_close_tab(0);
    h.settle();
    assert!(!h.app.has_doc());
    h.app.new_model = Some(Default::default());
    h.settle();
    h.click("OK");
    h.settle();
    assert!(h.app.has_doc());
    assert!(h.app.doc.editor.is_model());
}

// --- Bake ---

#[test]
fn bake_adds_the_baked_artboard_in_one_undo_step() {
    let mut h = DialogHarness::new(true);
    assert!(h.app.doc.editor.new_model("Model", (32, 32)));
    let before = h.app.doc.editor.document.artboards.len();
    h.app.bake = Some(crate::bake_dialog::BakeDialog::new(&h.app.doc.editor));
    h.settle();
    assert!(h.app.is_modal());
    assert!(h.drew_text("Sheet settings"), "{:?}", h.texts);
    for label in [
        "Frames",
        "Size (px)",
        "Zoom (%)",
        "Tilt (°)",
        "Height",
        "Ortho",
    ] {
        assert!(h.drew_text(label), "{label}: {:?}", h.texts);
    }
    h.button("Cancel");
    h.click("Export");
    h.settle();
    assert!(h.app.bake.is_none());
    let editor = &mut h.app.doc.editor;
    let doc = &editor.document;
    assert_eq!(doc.artboards.len(), before + 1);
    let baked = doc.artboards.last().unwrap();
    assert_eq!(baked.name, "Model · bake");
    assert!(baked.model.is_none(), "the bake is a normal artboard");
    assert_eq!(baked.sprite.frames.len(), 8, "8 angles by default");
    let baked = baked.id;
    assert_eq!(editor.active, baked, "the bake is shown on the canvas");
    assert_eq!(editor.workspace(), None, "out of the model's tab");
    editor.undo();
    assert_eq!(editor.document.artboards.len(), before);
}

#[test]
fn four_frames_in_the_sheet_settings_bake_four_frames() {
    let mut h = DialogHarness::new(true);
    assert!(h.app.doc.editor.new_model("Model", (32, 32)));
    h.app.bake = Some(crate::bake_dialog::BakeDialog::new(&h.app.doc.editor));
    h.settle();
    // El primer stepper (el de más arriba) es Frames: de 8 a 4.
    for _ in 0..4 {
        let minus = h
            .buttons
            .iter()
            .filter(|(l, _)| l == "Decrease")
            .map(|(_, r)| *r)
            .min_by(|a, b| a.min.y.partial_cmp(&b.min.y).unwrap())
            .expect("the Frames stepper");
        let press = |pressed| Event::PointerButton {
            pos: minus.center(),
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(minus.center())]);
        h.frame(vec![press(true)]);
        h.frame(vec![press(false)]);
        h.frame(vec![]);
    }
    assert_eq!(h.app.bake.as_ref().unwrap().options.frames, 4);
    h.click("Export");
    h.settle();
    assert!(h.app.bake.is_none());
    let baked = h.app.doc.editor.document.artboards.last().unwrap();
    assert_eq!(baked.name, "Model · bake");
    assert_eq!(baked.sprite.frames.len(), 4);
}

#[test]
fn without_pixel_ui_the_zoom_field_moves_in_steps_of_ten() {
    let mut h = DialogHarness::new(false);
    assert!(h.app.doc.editor.new_model("Model", (32, 32)));
    h.app.bake = Some(crate::bake_dialog::BakeDialog::new(&h.app.doc.editor));
    h.settle();
    // Frames, Size, Zoom, Tilt, Height: el tercero es Zoom.
    assert_eq!(h.numbers.len(), 5, "{:?}", h.numbers);
    h.drag_number(2, 12.0);
    let zoom = h.app.bake.as_ref().unwrap().options.zoom;
    assert!(zoom > 100 && zoom.is_multiple_of(10), "zoom {zoom}");
}

#[test]
fn export_bakes_the_dialogs_own_model_even_if_another_is_active() {
    let mut h = DialogHarness::new(true);
    assert!(h.app.doc.editor.new_model("First", (32, 32)));
    h.app.bake = Some(crate::bake_dialog::BakeDialog::new(&h.app.doc.editor));
    assert!(h.app.doc.editor.new_model("Second", (32, 32)));
    h.settle();
    assert!(h.app.bake.is_some(), "the dialog stays open");
    h.click("Export");
    h.settle();
    assert!(h.app.bake.is_none());
    let baked = h.app.doc.editor.document.artboards.last().unwrap();
    assert_eq!(baked.name, "First · bake");
}

// --- GIF settings ---

/// Los frames (RGBA), su tamaño, las demoras y si repite de un GIF.
fn decode_gif(bytes: &[u8]) -> ((u16, u16), Vec<Vec<u8>>, Vec<u16>, bool) {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(std::io::Cursor::new(bytes)).unwrap();
    let size = (decoder.width(), decoder.height());
    let (mut frames, mut delays) = (Vec::new(), Vec::new());
    while let Some(frame) = decoder.read_next_frame().unwrap() {
        frames.push(frame.buffer.to_vec());
        delays.push(frame.delay);
    }
    let looping = decoder.repeat() == gif::Repeat::Infinite;
    (size, frames, delays, looping)
}

/// La app con un modelo de 32×32 ("Ship") y GIF settings abierto.
fn gif_open(pixel_ui: bool) -> DialogHarness {
    let mut h = DialogHarness::new(pixel_ui);
    assert!(h.app.doc.editor.new_model("Ship", (32, 32)));
    h.app.gif = Some(crate::gif_dialog::GifDialog::new(&h.app.doc.editor));
    h.settle();
    h
}

#[test]
fn gif_settings_shows_its_fields_and_is_modal() {
    for pixel_ui in [true, false] {
        let h = gif_open(pixel_ui);
        assert!(h.app.is_modal());
        assert!(h.drew_text("GIF settings"), "{:?}", h.texts);
        for label in [
            "Frames",
            "Size (px)",
            "Zoom (%)",
            "Tilt (°)",
            "Height",
            "Ortho",
            "FOV (°)",
            "Render",
            "Scale",
            "Outline",
            "Scanlines",
            "Background",
            "Frame (ms)",
            "Loop",
            "Format",
            "32×32, 8 frames",
        ] {
            assert!(h.drew_text(label), "{label}: {:?}", h.texts);
        }
        h.button("Export");
        h.button("Cancel");
    }
}

#[test]
fn gif_settings_builds_a_gif_with_a_frame_per_angle() {
    let mut h = gif_open(true);
    let gif = h.app.gif.as_mut().unwrap();
    assert_eq!(gif.file_name(), "Ship.gif");
    gif.options.scale = 2;
    gif.options.frame_ms = 120;
    gif.options.bake.frames = 6;
    let bytes = gif.build(&h.app.doc.editor.document).unwrap();
    let (size, frames, delays, looping) = decode_gif(&bytes);
    assert_eq!(size, (64, 64));
    assert_eq!(frames.len(), 6);
    assert_eq!(delays, vec![12; 6]);
    assert!(looping);
    assert_ne!(frames[0], frames[1], "the model turns");
    // Sin fondo, la esquina es transparente.
    assert_eq!(frames[0][3], 0);
}

#[test]
fn gif_settings_presets_and_png_strip() {
    let mut h = gif_open(true);
    let gif = h.app.gif.as_mut().unwrap();
    gif.options.preset = cs_render::RenderPreset::GameBoy;
    gif.options.looping = false;
    gif.options.bake.frames = 4;
    let bytes = gif.build(&h.app.doc.editor.document).unwrap();
    let (size, frames, _, looping) = decode_gif(&bytes);
    assert_eq!(size, (160, 144));
    assert_eq!(frames.len(), 4);
    assert!(!looping);
    gif.format = crate::gif_dialog::GifFormat::Png;
    assert_eq!(gif.file_name(), "Ship.png");
    let png = gif.build(&h.app.doc.editor.document).unwrap();
    let strip = cs_io::import::decode_png(&png).unwrap();
    assert_eq!((strip.width, strip.height), (160 * 4, 144));
}

#[test]
fn gif_settings_refuses_a_huge_file() {
    let mut h = gif_open(true);
    let gif = h.app.gif.as_mut().unwrap();
    gif.options.preset = cs_render::RenderPreset::NintendoDs;
    gif.options.scale = 8;
    gif.options.bake.frames = 64;
    let error = gif.build(&h.app.doc.editor.document).unwrap_err();
    assert!(error.contains("too big"), "{error}");
    // Export lo dice y deja el diálogo abierto para cambiarlo.
    h.click("Export");
    h.settle();
    assert!(h.app.message.is_some());
    assert!(h.app.gif.is_some());
}

#[test]
fn gif_settings_cancel_closes_and_the_dialog_follows_its_model() {
    let mut h = gif_open(true);
    h.click("Cancel");
    h.settle();
    assert!(h.app.gif.is_none());
    // Si el modelo deja de existir, se cierra solo.
    h.app.gif = Some(crate::gif_dialog::GifDialog::new(&h.app.doc.editor));
    let model = h.app.gif.as_ref().unwrap().model();
    h.app.doc.editor.document.artboard_mut(model).unwrap().model = None;
    h.settle();
    assert!(h.app.gif.is_none());
}

#[test]
fn the_fov_stepper_is_disabled_in_ortho_and_changes_the_fov() {
    let mut h = gif_open(true);
    let fov = h.app.gif.as_ref().unwrap().options.bake.fov;
    // El stepper del FOV es el séptimo de la primera columna (Frames, Size, Zoom, Tilt, Height,
    // y FOV después de Ortho): el más abajo de la columna de la izquierda.
    let left = h
        .buttons
        .iter()
        .filter(|(l, _)| l == "Increase")
        .map(|(_, r)| *r)
        .min_by(|a, b| a.min.x.partial_cmp(&b.min.x).unwrap())
        .unwrap()
        .min
        .x;
    let plus = h
        .buttons
        .iter()
        .filter(|(l, r)| l == "Increase" && (r.min.x - left).abs() < 1.0)
        .map(|(_, r)| *r)
        .max_by(|a, b| a.min.y.partial_cmp(&b.min.y).unwrap())
        .unwrap();
    let press = |pressed| Event::PointerButton {
        pos: plus.center(),
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(plus.center())]);
    h.frame(vec![press(true)]);
    h.frame(vec![press(false)]);
    h.frame(vec![]);
    let o = h.app.gif.as_ref().unwrap().options.bake;
    if o.ortho {
        assert_eq!(o.fov, fov, "disabled in ortho");
    } else {
        assert_eq!(o.fov, fov + 50);
    }
}

// --- Export Tiled Map ---

/// El documento con la capa pasada a tilemap de 8×8 (con un píxel pintado: un tile).
fn with_a_tilemap(pixel_ui: bool) -> DialogHarness {
    use cs_core::{PixelValue, Rgba};
    let mut h = DialogHarness::new(pixel_ui);
    let editor = &mut h.app.doc.editor;
    editor.execute(cs_commands::Command::DrawPixels {
        layer: editor.layer,
        frame: 0,
        pixels: vec![(1, 1, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
    });
    assert!(editor.convert_to_tilemap((8, 8)));
    h
}

#[test]
fn tiled_export_writes_the_map_into_its_folder() {
    let dir = std::env::temp_dir().join(format!("chimpsprite-tiled-ui-{}", std::process::id()));
    let mut h = with_a_tilemap(true);
    h.app.open_tiled_export();
    h.app.tiled_export.as_mut().unwrap().folder = dir.display().to_string();
    h.app.tiled_export.as_mut().unwrap().name = "level".into();
    h.settle();
    assert!(h.app.is_modal());
    for label in [
        "Export Tiled Map",
        "Format",
        "Tilesets",
        "Layer data",
        "Frames",
        "Name",
        "Folder",
        "TMX (XML)",
        "External .tsx",
        "Base64 + zlib",
        "Current (1)",
    ] {
        assert!(h.drew_text(label), "{label}: {:?}", h.texts);
    }
    h.button("Browse…");
    h.click("TMJ (JSON)");
    h.settle();
    let dialog = h.app.tiled_export.as_ref().unwrap();
    assert_eq!(dialog.format, cs_io::tiled::MapFormat::Tmj);
    assert!(h.drew_text("External .tsj"), "{:?}", h.texts);
    h.click("Export");
    h.settle();
    assert!(h.app.tiled_export.is_none());
    let message = h.app.message.as_ref().expect("the files written");
    assert_eq!(message.title, "Exported");
    assert!(message.body.contains("level.tmj"), "{}", message.body);
    let map = std::fs::read_to_string(dir.join("level.tmj")).unwrap();
    let map = cs_io::tiled::read_tmj(&map).unwrap();
    assert_eq!(map.tilesets.len(), 1);
    assert!(dir.join(map.tilesets[0].source.as_ref().unwrap()).exists());
    // La próxima vez abre igual.
    assert_eq!(h.app.last_tiled.format, cs_io::tiled::MapFormat::Tmj);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn exported_tiled_maps_open_as_tilemap_sprites() {
    let dir = std::env::temp_dir().join(format!("chimpsprite-tiled-open-{}", std::process::id()));
    let mut h = with_a_tilemap(false);
    let dialog = crate::tiled_dialog::TiledDialog::default();
    let platform = h.app.platform.clone();
    dialog
        .export_to(&mut h.app.doc.editor, Some(dir.clone()), "level", &platform)
        .unwrap();
    h.app.platform.open_path(
        &dir.join("level.tmx"),
        crate::platform::OpenPurpose::Dropped,
    );
    let ctx = h.ctx.clone();
    h.app.handle_platform_events(&ctx);
    h.settle();
    assert!(
        h.app.message.is_none(),
        "{:?}",
        h.app.message.as_ref().map(|m| &m.body)
    );
    let sprite = h.app.doc.editor.sprite();
    assert_eq!(sprite.layers[0].kind, cs_core::LayerKind::Tilemap);
    assert_eq!(sprite.tilesets[0].tiles.len(), 2);
    assert_eq!(h.app.doc.location.name, "level.chimp");
    assert!(h.app.message.is_none(), "no warnings");
    std::fs::remove_dir_all(dir).ok();
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
    // Los campos son los del UI kit: el select y el de texto (`field.text`) y los numéricos (con
    // `px`).
    for piece in ["field.text", "field.unit"] {
        assert!(h.drew_piece(piece), "{piece}");
    }
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

/// Cada ventana se abre en los dos modos; con Pixel UI es el diálogo pixel (su área y el marco
/// `window` del skin).
#[test]
fn with_pixel_ui_a_slice_can_be_renamed_in_slice_properties() {
    let mut h = DialogHarness::new(true);
    h.app.doc.editor.add_slice();
    h.app.doc.canvas.slice_properties = true;
    h.settle();
    let field = h
        .inputs
        .iter()
        .find(|(l, _)| l == "Slice name")
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("no Slice name in {:?}", h.inputs));
    let press = |pressed| Event::PointerButton {
        pos: field.center(),
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame(vec![Event::PointerMoved(field.center())]);
    h.frame(vec![press(true)]);
    h.frame(vec![press(false)]);
    h.frame(vec![Event::Text("y".into())]);
    // Varios frames escribiendo: lo escrito no se pierde.
    h.frame(vec![]);
    h.frame(vec![Event::Text("z".into())]);
    h.key(Key::Enter);
    assert_eq!(h.app.doc.editor.sprite().slices[0].name, "Slice 1yz");
}

#[test]
fn every_window_is_the_app_window() {
    type Open = fn(&mut ChimpApp, &egui::Context);
    let windows: [(&str, &str, Open); 6] = [
        ("palette_presets_window", "Palette Presets", |app, ctx| {
            app.palette_action(ctx, crate::panels::PaletteAction::OpenPresets)
        }),
        ("export_window", "Export", |app, _| app.open_export()),
        ("tiled_export_window", "Export Tiled Map", |app, _| {
            app.open_tiled_export()
        }),
        ("grid_settings_window", "Grid Settings", |app, _| {
            app.grid_dialog = Some(crate::view::GridDialog::new(&app.doc.editor, &app.view))
        }),
        ("message_window", "Hello", |app, _| {
            app.show_message("Hello", "World")
        }),
        ("slice_properties_window", "Slice Properties", |app, _| {
            app.doc.editor.add_slice();
            app.doc.canvas.slice_properties = true;
        }),
    ];
    for (id, title, open) in windows {
        for pixel_ui in [true, false] {
            let mut h = DialogHarness::new(pixel_ui);
            let ctx = h.ctx.clone();
            open(&mut h.app, &ctx);
            h.settle();
            assert!(h.drew_text(title), "{id}: pixel_ui {pixel_ui}");
            // El área del diálogo pixel, solo con Pixel UI (sin él, la ventana de egui).
            assert_eq!(h.has_area(id), pixel_ui, "{id}: pixel_ui {pixel_ui}");
            if pixel_ui {
                assert!(h.drew_piece("window"), "{id}");
            }
        }
    }
}

#[test]
fn convert_to_symbol_dialog_creates_with_the_chosen_type() {
    use cs_commands::symbol::SymbolTiming;
    use cs_core::{PixelValue, Rgba};
    for pixel in [false, true] {
        let mut h = DialogHarness::new(pixel);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let editor = &mut h.app.doc.editor;
        editor.execute(cs_commands::Command::DrawPixels {
            layer: editor.layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        h.app.open_convert_to_symbol();
        h.settle();
        assert!(h.drew_text("Convert to Symbol"), "{:?}", h.texts);
        h.click("Movie Clip");
        h.key(egui::Key::Enter);
        assert!(h.app.symbol_dialog.is_none());
        assert!(h.app.message.is_none(), "pixel ui {pixel}: no error");
        let e = &h.app.doc.editor;
        let key = e.sprite().layer(e.layer).unwrap().objects[0].track.keys[0];
        assert_eq!(
            key.timing,
            SymbolTiming::MovieClip.timing(),
            "pixel ui {pixel}"
        );
    }
}

#[test]
fn shrinking_the_texture_with_uvs_outside_asks_and_resize_applies() {
    for pixel_ui in [true, false] {
        let mut h = DialogHarness::new(pixel_ui);
        let e = &mut h.app.doc.editor;
        assert!(e.new_model("Ship", (32, 32)));
        e.set_cad_mode(crate::editor::CadMode::Texture);
        let texture = e.workspace_model().unwrap().texture.unwrap();
        let side = |h: &DialogHarness| {
            let s = &h.app.doc.editor.document.artboard(texture).unwrap().sprite;
            (s.width, s.height)
        };
        // Lo que pide Texture size en el panel 3D (ver `ModelAction::TextureSize`).
        h.app.palette_confirm = Some(crate::palette_ui::Confirm::TextureSize(32));
        h.settle();
        assert!(
            h.texts
                .iter()
                .any(|t| t.contains("Some faces use pixels outside the new size")),
            "pixel_ui {pixel_ui}: {:?}",
            h.texts
        );
        assert_eq!(side(&h), (64, 64), "not yet");
        h.click("Cancel");
        assert!(h.app.palette_confirm.is_none());
        assert_eq!(side(&h), (64, 64));
        h.app.palette_confirm = Some(crate::palette_ui::Confirm::TextureSize(32));
        h.settle();
        h.click("Resize");
        assert!(h.app.palette_confirm.is_none());
        assert_eq!(side(&h), (32, 32));
    }
}

// --- Build e Import Font ---

#[test]
fn build_font_opens_with_the_format_and_cancels() {
    let mut h = DialogHarness::new(true);
    h.app
        .doc
        .editor
        .new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.app
        .open_build_font(Some(crate::font_io::BuildFormat::BmFont));
    h.settle();
    assert!(h.drew_text("Build Font"));
    assert!(h.drew_text("Mine.fnt + Mine.png"));
    let dialog = h.app.build_font.as_ref().unwrap();
    assert_eq!(dialog.format, crate::font_io::BuildFormat::BmFont);
    h.click("Cancel");
    h.settle();
    assert!(h.app.build_font.is_none());
    // Sin fuente activa no hay nada que construir; en 1 bit, el TTF de color pasa a TTF.
    h.app
        .open_build_font(Some(crate::font_io::BuildFormat::ColorTtf));
    assert_eq!(
        h.app.build_font.as_ref().map(|d| d.format),
        Some(crate::font_io::BuildFormat::Ttf)
    );
}

#[test]
fn importing_a_ttf_shows_its_dialog_and_adds_the_font_in_its_tab() {
    let mut h = DialogHarness::new(true);
    let ctx = h.ctx.clone();
    h.app
        .opened_font_file(&ctx, "tiny.ttf", crate::font_io::sample_ttf());
    h.settle();
    assert!(h.drew_text("Import Font"));
    assert_eq!(h.app.import_font.as_ref().unwrap().px, 8);
    let before = h.app.doc.editor.document.artboards.len();
    h.click("Import");
    h.settle();
    assert!(h.app.import_font.is_none());
    let editor = &h.app.doc.editor;
    assert_eq!(editor.document.artboards.len(), before + 1);
    let font = editor.font().expect("the imported font is active");
    assert_eq!(font.name, "Tiny");
    assert_eq!((editor.sprite().width, editor.sprite().height), (6, 8));
    assert_eq!(
        crate::dock::open_panel(&h.ctx),
        Some(crate::dock::DockPanel::Font)
    );
    // Un paso de undo.
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.document.artboards.len(), before);
}

#[test]
fn a_bitfontmaker_json_is_imported_without_a_dialog_even_from_home() {
    let mut h = DialogHarness::new(true);
    h.app.request_close_tab(0);
    h.settle();
    assert!(!h.app.has_doc());
    let ctx = h.ctx.clone();
    let json = br#"{"65":[0,0,0,12,12,0,0,0,0,0,0,0,0,0,0,0],"name":"BFM"}"#.to_vec();
    h.app.opened_font_file(&ctx, "bfm.json", json);
    h.settle();
    assert!(h.app.import_font.is_none());
    assert!(h.app.has_doc());
    let font = h.app.doc.editor.font().expect("a font");
    assert_eq!(font.name, "BFM");
    assert_eq!(font.glyphs.len(), 1);
    // Lo que no es una fuente avisa.
    h.app.opened_font_file(&ctx, "bad.json", b"[]".to_vec());
    h.settle();
    assert!(h.app.message.is_some());
}

#[test]
fn fill_from_ttf_fills_the_missing_glyphs_of_the_active_font_in_one_step() {
    let mut h = DialogHarness::new(true);
    // Una fuente 6×8 como la de prueba, con la `A` vacía.
    assert!(
        h.app
            .doc
            .editor
            .new_font("Mine", (6, 8), cs_core::FontMode::OneBit)
    );
    h.app.opened_fill_font(crate::font_io::sample_ttf());
    h.settle();
    assert!(h.drew_text("Fill from TTF"));
    assert!(h.app.import_font.as_ref().unwrap().fill.is_some());
    h.click("Fill");
    h.settle();
    assert!(h.app.import_font.is_none());
    let body = &h.app.message.as_ref().expect("a notice").body;
    assert_eq!(body, "1 glyph filled.");
    let editor = &mut h.app.doc.editor;
    let font = editor.font().unwrap().clone();
    let a = font.index_of('A').unwrap();
    assert!(editor.sprite().cel(editor.layer, a).is_some(), "A filled");
    editor.undo();
    assert!(editor.sprite().cel(editor.layer, a).is_none());
}

#[test]
fn auto_build_accents_tells_what_it_did() {
    let mut h = DialogHarness::new(true);
    let e = &mut h.app.doc.editor;
    assert!(e.new_font("Mine", (8, 10), cs_core::FontMode::OneBit));
    e.add_glyphs(&['´']);
    for (ch, points) in [('a', vec![(0, 5), (3, 7)]), ('´', vec![(1, 0)])] {
        e.go_to_glyph(ch);
        let (layer, frame) = (e.layer, e.frame);
        e.execute(cs_commands::Command::DrawPixels {
            layer,
            frame,
            pixels: points
                .into_iter()
                .map(|(x, y)| (x, y, cs_core::ONE_BIT_INK))
                .collect(),
        });
    }
    h.app.auto_build_accents();
    let body = &h.app.message.as_ref().expect("a notice").body;
    assert!(body.starts_with("1 accented letter built."), "{body}");
    assert!(body.contains("need a base letter or a mark"), "{body}");
    assert!(h.app.doc.editor.font().unwrap().index_of('á').is_some());
}

#[test]
fn glyphs_from_an_image_go_to_the_active_font() {
    let mut h = DialogHarness::new(true);
    assert!(
        h.app
            .doc
            .editor
            .new_font("Mine", (4, 4), cs_core::FontMode::OneBit)
    );
    let mut sheet = cs_io::export::Bitmap::new(8, 4);
    sheet.pixels.fill(cs_core::Rgba::new(255, 255, 255, 255));
    sheet.pixels[1] = cs_core::Rgba::new(0, 0, 0, 255);
    let png = sheet.to_png().unwrap();
    h.app.opened_glyph_sheet(&png);
    h.settle();
    assert!(h.drew_text("Glyphs from Image"));
    h.click("Import");
    h.settle();
    assert!(h.app.glyph_sheet.is_none());
    assert_eq!(h.app.message.as_ref().unwrap().body, "1 glyph filled.");
    let e = &h.app.doc.editor;
    let bang = e.font().unwrap().index_of('!').unwrap();
    assert!(e.sprite().cel(e.layer, bang).is_some());
}

// --- Expand to Spritesheet / Slice into Frames ---

/// La app con un artboard "Walk" de 8×8 y 4 frames activo.
fn walk(pixel_ui: bool) -> DialogHarness {
    let mut h = DialogHarness::new(pixel_ui);
    let mut sprite = cs_core::Sprite::new(8, 8, cs_core::ColorMode::Rgba);
    sprite.frames = vec![cs_core::Frame { duration_ms: 100 }; 4];
    assert!(
        h.app
            .doc
            .editor
            .add_artboard_with_sprite("Walk".into(), sprite)
    );
    h.settle();
    h
}

#[test]
fn expand_to_spritesheet_shows_the_preview_and_adds_the_sheet() {
    for pixel_ui in [true, false] {
        let mut h = walk(pixel_ui);
        let before = h.app.doc.editor.document.artboards.len();
        h.app.doc.canvas.artboard_request = Some(crate::canvas::ArtboardRequest::ExpandSheet);
        h.app.artboard_request();
        h.settle();
        assert!(h.drew_text("Expand to Spritesheet"), "{:?}", h.texts);
        assert!(
            h.texts.iter().any(|t| t.contains("2 × 2 cells, 16×16 px")),
            "{:?}",
            h.texts
        );
        h.click("Columns");
        assert_eq!(
            h.app.expand_dialog.as_ref().unwrap().shape,
            crate::expand_dialog::SheetShape::Columns
        );
        h.click("Expand");
        assert!(h.app.expand_dialog.is_none(), "pixel_ui {pixel_ui}");
        let e = &h.app.doc.editor;
        assert_eq!(e.document.artboards.len(), before + 1);
        assert_eq!(e.artboard().name, "Walk Sheet");
        // Columns empieza en las automáticas (2): 2 × 2.
        assert_eq!((e.sprite().width, e.sprite().height), (16, 16));
        h.app.doc.editor.undo();
        assert_eq!(h.app.doc.editor.document.artboards.len(), before);
    }
}

#[test]
fn slice_into_frames_reuses_the_sheet_dialog_with_its_own_title() {
    let mut h = walk(true);
    h.app.doc.canvas.artboard_request = Some(crate::canvas::ArtboardRequest::SliceFrames);
    h.app.artboard_request();
    h.settle();
    assert!(h.drew_text("Slice into Frames"), "{:?}", h.texts);
    h.click("Columns × Rows");
    let dialog = h.app.sheet_dialog.as_mut().unwrap();
    assert_eq!(dialog.mode, crate::sheet_dialog::CutMode::Counts);
    dialog.counts = (2, 2);
    h.click("Slice");
    assert!(h.app.sheet_dialog.is_none());
    let e = &h.app.doc.editor;
    assert_eq!(e.artboard().name, "Walk Frames");
    // Un frame vacío: las celdas vacías del final no se agregan (queda la primera).
    assert_eq!((e.sprite().width, e.sprite().height), (4, 4));
    assert_eq!(h.app.tabs.docs(&h.app.doc).len(), 1, "no new tab");
}

#[test]
fn new_variant_creates_the_style_and_opens_its_tab() {
    let mut h = DialogHarness::new(true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    e.go_to_glyph('A');
    let (layer, frame) = (e.layer, e.frame);
    e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(0, 2, cs_core::ONE_BIT_INK)],
    });
    h.app.open_new_variant();
    h.settle();
    assert!(h.drew_text("New Variant"));
    assert!(h.drew_text("From Mine"));
    h.click("Create");
    h.settle();
    assert!(h.app.new_variant.is_none());
    let e = &h.app.doc.editor;
    assert_eq!(e.artboard().name, "Mine Bold");
    assert!(h.app.doc.views.contains(&e.active), "its tab");
    // Otra vez Bold: la familia ya lo tiene, el diálogo lo dice y no se cierra.
    let regular = e.family_fonts()[0].0;
    h.app.tabs.open_view(&mut h.app.doc, regular);
    h.app.open_new_variant();
    h.settle();
    assert!(h.drew_text("The family has a \"Bold\" style already."));
    h.click("Create");
    h.settle();
    assert!(h.app.new_variant.is_some());
    assert_eq!(h.app.doc.editor.family_fonts().len(), 2);
}

#[test]
fn build_font_offers_the_whole_family() {
    let mut h = DialogHarness::new(true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    let recipe = cs_core::VariantRecipe::new(cs_core::VariantKind::Italic);
    let style = cs_core::FontStyle {
        name: "Italic".into(),
        italic: true,
        slant: 3,
        ..Default::default()
    };
    assert!(e.add_variant(&recipe, style));
    h.app.open_build_font(None);
    h.settle();
    assert!(h.drew_text("Mine-Italic.ttf"));
    assert!(h.drew_text("Whole family (2 styles)"));
    let dialog = h.app.build_font.as_mut().unwrap();
    dialog.whole_family = true;
    assert_eq!(dialog.targets().len(), 2);
    h.settle();
    assert!(h.drew_text("Mine-Regular.ttf\nMine-Italic.ttf"));
}

#[test]
fn a_pasted_coolors_palette_shows_up_and_applies() {
    for pixel_ui in [true, false] {
        let mut h = DialogHarness::new(pixel_ui);
        h.app
            .paste_palette_text("https://coolors.co/palette/264653-2a9d8f-e9c46a");
        h.settle();
        assert!(
            h.drew_text("Coolors 264653-2a9d8f-e9c46a"),
            "pixel_ui {pixel_ui}"
        );
        h.click("Apply");
        let colors = h.app.doc.editor.sprite().palette.colors().to_vec();
        assert_eq!(
            colors,
            [0x264653, 0x2a9d8f, 0xe9c46a].map(cs_core::Rgba::from_hex),
            "pixel_ui {pixel_ui}"
        );
    }
}

#[test]
fn grid_settings_has_the_grid_type() {
    for pixel_ui in [true, false] {
        let mut h = DialogHarness::new(pixel_ui);
        h.app.grid_dialog = Some(crate::view::GridDialog::new(&h.app.doc.editor, &h.app.view));
        h.settle();
        assert!(h.drew_text("Type:"), "pixel_ui {pixel_ui}: {:?}", h.texts);
        assert!(
            h.drew_text("Orthogonal"),
            "pixel_ui {pixel_ui}: {:?}",
            h.texts
        );
    }
}
