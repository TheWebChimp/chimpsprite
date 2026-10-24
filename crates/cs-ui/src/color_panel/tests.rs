//! El panel de paleta y color con Pixel UI, manejado con eventos crudos como en la app.

use cs_core::{ColorMode, Palette, PixelValue, Rgba, Sprite};
use eframe::egui::{
    self, Color32, Event, OutputCommand, PointerButton, Pos2, RawInput, Rect, Vec2, pos2, vec2,
};

use crate::editor::Editor;
use crate::skin::{Skin, WidgetState};
use crate::{panels, pixel_ui};

/// Los 21 colores de la paleta de prueba (como en el diseño, que muestra 21).
fn test_palette() -> Palette {
    Palette::new(
        (0..21u8)
            .map(|i| Rgba::rgb(i * 12, 250 - i * 10, 7 + i * 3))
            .collect(),
    )
}

/// El panel derecho como en la app (`lib.rs`): la paleta y el editor de color en el scroll del
/// panel. El reloj avanza 50 ms por frame.
struct Harness {
    ctx: egui::Context,
    editor: Editor,
    skin: Option<Skin>,
    time: f64,
    nodes: Vec<(String, Rect)>,
    shapes: Vec<egui::epaint::ClippedShape>,
    /// Lo copiado al portapapeles en cada frame.
    copied: Vec<String>,
    /// Lo pedido desde el panel en el último frame.
    action: Option<panels::PaletteAction>,
}

impl Harness {
    /// Con `pixel`, el estilo y la fuente de Pixel UI y el skin embebido.
    fn new(pixel: bool) -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = pixel.then(|| Skin::embedded(&ctx));
        if let Some(skin) = &skin {
            ctx.set_fonts(crate::appearance::font_definitions(true));
            crate::appearance::apply_style(&ctx, Some(skin.colors()));
        }
        let mut sprite = Sprite::new(8, 8, ColorMode::Rgba);
        sprite.palette = test_palette();
        let mut editor = Editor::with_sprite(sprite);
        editor.primary = PixelValue::Rgba(Rgba::rgb(10, 20, 30));
        let mut h = Self {
            ctx,
            editor,
            skin,
            time: 0.0,
            nodes: Vec::new(),
            shapes: Vec::new(),
            copied: Vec::new(),
            action: None,
        };
        h.frame(vec![]);
        h.frame(vec![]);
        h
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 800.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let editor = &mut self.editor;
        let skin = self.skin.as_ref();
        let mut action = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            egui::Panel::right("palette")
                .default_size(220.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        action = panels::palette(ui, editor, true, skin);
                    });
                });
        });
        self.action = action;
        output.textures_delta.clear();
        self.nodes = nodes(&output);
        self.copied = output
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        self.shapes = output.shapes;
    }

    fn node(&self, label: &str) -> Rect {
        self.nodes
            .iter()
            .find(|(t, _)| t == label)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {label:?} in {:?}", self.nodes))
    }

    fn has(&self, label: &str) -> bool {
        self.nodes.iter().any(|(t, _)| t == label)
    }

    fn texture(&self, piece: &str) -> egui::TextureId {
        let skin = self.skin.as_ref().expect("with Pixel UI");
        skin.piece(piece, WidgetState::Normal)
            .unwrap_or_else(|| panic!("{piece}"))
            .texture
            .id()
    }

    /// Las partes dibujadas con la pieza `piece`, de izquierda a derecha y de arriba abajo.
    fn drawn(&self, piece: &str) -> Vec<Rect> {
        let id = self.texture(piece);
        let mut meshes = Vec::new();
        for clipped in &self.shapes {
            collect_meshes(&clipped.shape, &mut meshes);
        }
        let mut rects: Vec<Rect> = meshes
            .into_iter()
            .filter(|(t, _)| *t == id)
            .map(|(_, r)| r)
            .collect();
        rects.sort_by(|a, b| {
            a.top()
                .total_cmp(&b.top())
                .then(a.left().total_cmp(&b.left()))
        });
        rects
    }

    /// La pieza `piece` dibujada adentro de `rect`, sus partes unidas.
    fn drawn_in(&self, piece: &str, rect: Rect) -> Option<Rect> {
        self.drawn(piece)
            .into_iter()
            .filter(|r| rect.contains_rect(*r))
            .reduce(|a, b| a.union(b))
    }

    /// Como `drawn`, con las partes de un 9-slice (que se tocan) unidas en un rect.
    fn drawn_boxes(&self, piece: &str) -> Vec<Rect> {
        let mut boxes: Vec<Rect> = Vec::new();
        for rect in self.drawn(piece) {
            match boxes.iter_mut().find(|b| b.intersects(rect)) {
                Some(b) => *b = b.union(rect),
                None => boxes.push(rect),
            }
        }
        boxes
    }

    fn texts(&self) -> Vec<(String, Rect, Color32)> {
        let mut texts = Vec::new();
        for clipped in &self.shapes {
            collect_texts(&clipped.shape, &mut texts);
        }
        texts
    }

    fn text(&self, text: &str) -> (Color32, Rect) {
        self.texts()
            .into_iter()
            .find(|(t, _, _)| t == text)
            .map(|(_, r, c)| (c, r))
            .unwrap_or_else(|| panic!("no text {text:?} in {:?}", self.texts()))
    }

    /// Los rects de color liso: los de `rect_filled` y los de malla sin textura.
    fn colored_rects(&self) -> Vec<(Color32, Rect)> {
        fn filled(shape: &egui::Shape, out: &mut Vec<(Color32, Rect)>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| filled(s, out)),
                egui::Shape::Rect(r) => out.push((r.fill, r.rect)),
                _ => out.extend(pixel_ui::colored_rects(shape)),
            }
        }
        let mut out = Vec::new();
        for clipped in &self.shapes {
            filled(&clipped.shape, &mut out);
        }
        out
    }

    fn press(&mut self, at: Pos2, button: PointerButton, pressed: bool) {
        self.frame(vec![Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Default::default(),
        }]);
    }

    fn click_with(&mut self, at: Pos2, button: PointerButton) {
        self.frame(vec![Event::PointerMoved(at)]);
        self.press(at, button, true);
        self.press(at, button, false);
        self.frame(vec![]);
    }

    fn click_at(&mut self, at: Pos2) {
        self.click_with(at, PointerButton::Primary);
    }

    /// Arrastre con el botón principal desde `from`, `dx` puntos a la derecha en 4 pasos.
    fn drag(&mut self, from: Pos2, dx: f32) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.press(from, PointerButton::Primary, true);
        let mut at = from;
        for _ in 0..4 {
            at.x += dx / 4.0;
            self.frame(vec![Event::PointerMoved(at)]);
        }
        self.press(at, PointerButton::Primary, false);
        self.frame(vec![]);
    }

    fn type_and_enter(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        let key = |pressed| Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Default::default(),
        };
        self.frame(vec![key(true)]);
        self.frame(vec![key(false)]);
        self.frame(vec![]);
    }

    fn primary(&self) -> Rgba {
        match self.editor.primary {
            PixelValue::Rgba(c) => c,
            other => panic!("{other:?}"),
        }
    }

    fn set_primary(&mut self, c: Rgba) {
        self.editor.primary = PixelValue::Rgba(c);
        self.frame(vec![]);
    }
}

fn nodes(output: &egui::FullOutput) -> Vec<(String, Rect)> {
    let Some(update) = &output.platform_output.accesskit_update else {
        return Vec::new();
    };
    update
        .nodes
        .iter()
        .filter_map(|(_, n)| {
            let b = n.bounds()?;
            let text = n.label().or_else(|| n.value())?.to_string();
            let rect = Rect::from_min_max(
                pos2(b.x0 as f32, b.y0 as f32),
                pos2(b.x1 as f32, b.y1 as f32),
            );
            Some((text, rect))
        })
        .collect()
}

fn collect_meshes(shape: &egui::Shape, out: &mut Vec<(egui::TextureId, Rect)>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_meshes(s, out)),
        egui::Shape::Mesh(mesh) => out.push((mesh.texture_id, mesh.calc_bounds())),
        _ => {}
    }
}

fn collect_texts(shape: &egui::Shape, out: &mut Vec<(String, Rect, Color32)>) {
    match shape {
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_texts(s, out)),
        egui::Shape::Text(text) => out.push((
            text.galley.text().to_owned(),
            text.visual_bounding_rect(),
            text.fallback_color,
        )),
        _ => {}
    }
}

fn to_color32(c: Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

#[test]
fn the_pieces_of_the_palette_and_the_color_editor_are_in_the_embedded_skin() {
    let ctx = egui::Context::default();
    let skin = Skin::embedded(&ctx);
    let size = |name: &str| skin.piece(name, WidgetState::Normal).map(|p| p.size);
    for (name, piece) in [
        ("palette.box", [9, 9]),
        ("palette.swatch", [13, 13]),
        ("palette.add", [6, 7]),
        ("picker.copy", [25, 16]),
        ("picker.sv", [7, 7]),
        ("picker.bars", [7, 6]),
        ("picker.cursor", [9, 9]),
        ("picker.cursor.bar", [7, 7]),
        ("channel.r", [4, 16]),
        ("channel.g", [1, 16]),
        ("channel.b", [1, 16]),
        ("channel.a", [4, 16]),
        ("channel.hex", [5, 16]),
        ("channel.sep", [1, 16]),
        ("channel.dot.r", [6, 6]),
        ("channel.dot.g", [6, 6]),
        ("channel.dot.b", [6, 6]),
        ("channel.dot.a", [6, 6]),
    ] {
        assert_eq!(size(name), Some(piece), "{name}");
    }
}

#[test]
fn with_pixel_ui_the_swatches_are_the_design_cells_inside_the_palette_box() {
    let h = Harness::new(true);
    let boxes = h.drawn_boxes("palette.box");
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    let pbox = boxes[0];
    // La caja del diseño mide 207 de alto (x 784..972, y 120..326).
    assert!(pbox.height() >= 207.0, "{pbox:?}");
    // El editor, 5 filas debajo de la caja (y 327..331 del diseño), con el botón de copiar 2 px
    // adentro.
    let copy = h.drawn("picker.copy")[0];
    assert_eq!(copy.min, pos2(pbox.left() + 2.0, pbox.bottom() + 5.0));
    // Los 21 colores: cuadritos de 13×13 (borde de 1 compartido), uno cada 12, desde 4 px
    // adentro de la caja (el Mask es el botón del damero, arriba).
    let cells: Vec<Rect> = (0..21).map(|i| h.node(&format!("#{i}"))).collect();
    assert_eq!(cells[0].min, pbox.min + vec2(4.0, 4.0));
    let columns = ((pbox.width() - 9.0) / 12.0).floor() as usize;
    for (i, cell) in cells.iter().enumerate() {
        let at = cells[0].min + vec2((i % columns) as f32 * 12.0, (i / columns) as f32 * 12.0);
        assert_eq!(
            *cell,
            Rect::from_min_size(at, Vec2::splat(13.0)),
            "cell {i}"
        );
        assert!(pbox.contains_rect(*cell));
        // El borde es la pieza; adentro, el color en 11×11.
        assert_eq!(h.drawn_in("palette.swatch", *cell), Some(*cell), "cell {i}");
    }
    let rects = h.colored_rects();
    for (i, cell) in cells.iter().enumerate() {
        let color = to_color32(h.editor.sprite().palette.colors()[i]);
        let inside = cell.shrink(1.0);
        assert!(
            rects.iter().any(|&(c, r)| c == color && r == inside),
            "#{i} {color:?} in {inside:?}"
        );
    }
}

#[test]
fn with_pixel_ui_clicking_a_swatch_picks_the_primary_and_right_click_the_secondary() {
    let mut h = Harness::new(true);
    h.click_at(h.node("#3").center());
    assert_eq!(h.editor.primary, h.editor.palette_value(3));
    h.click_with(h.node("#5").center(), PointerButton::Secondary);
    assert_eq!(h.editor.secondary, h.editor.palette_value(5));
}

#[test]
fn with_pixel_ui_the_add_mark_follows_the_last_swatch_and_adds_the_primary_color() {
    let mut h = Harness::new(true);
    // En el diseño, la marca va 3 px después del borde del último cuadrito y 3 abajo de su borde
    // de arriba (x 863, y 139): en la celda siguiente, a (3,3).
    let last = h.node("#20");
    let add = h.node("Add to palette");
    let at = last.min + vec2(12.0 + 3.0, 3.0);
    assert_eq!(add, Rect::from_min_size(at, vec2(6.0, 7.0)));
    assert_eq!(h.drawn("palette.add"), vec![add]);
    h.click_at(add.center());
    let palette = h.editor.sprite().palette.colors().to_vec();
    assert_eq!(palette.len(), 22);
    assert_eq!(palette[21], Rgba::rgb(10, 20, 30));
    // La marca pasa al lado del nuevo.
    assert_eq!(
        h.node("Add to palette").min,
        h.node("#21").min + vec2(15.0, 3.0)
    );
    // Se deshace como cualquier cambio de paleta.
    h.editor.undo();
    assert_eq!(h.editor.sprite().palette.len(), 21);
}

#[test]
fn with_pixel_ui_the_channel_strip_shows_the_four_values_and_the_hex() {
    let mut h = Harness::new(true);
    h.set_primary(Rgba::new(10, 20, 30, 200));
    let text_color = h.skin.as_ref().unwrap().colors().field_text;
    assert_eq!(text_color, Color32::from_rgb(89, 77, 77));
    // La tira: 16 de alto, 2 px después del botón de copiar (25×16), en el orden del diseño.
    let copy = h.drawn("picker.copy");
    assert_eq!(copy.len(), 1);
    assert_eq!(copy[0].size(), vec2(25.0, 16.0));
    let r = h.drawn_boxes("channel.r")[0];
    let g = h.drawn_boxes("channel.g")[0];
    let b = h.drawn_boxes("channel.b")[0];
    let hex = h.drawn_boxes("channel.hex")[0];
    let a = h.drawn("channel.a");
    assert_eq!(r.min, pos2(copy[0].right() + 2.0, copy[0].top()));
    assert_eq!(r.height(), 16.0);
    // Con la fuente pixel, "255" mide 14 de tinta como en el diseño: los tramos son los del
    // diseño (x 813..842, 844..870, 872..898).
    assert_eq!((r.width(), g.width(), b.width()), (30.0, 27.0, 27.0));
    assert_eq!(g.left(), r.right() + 1.0, "the separator between");
    assert_eq!(b.left(), g.right() + 1.0);
    assert_eq!(a[0].left(), b.right() + 1.0, "the alpha checker, tiled");
    assert!(a.len() >= 6 && a.iter().all(|t| t.size() == vec2(4.0, 16.0)));
    assert!(hex.left() > a[0].left() && hex.height() == 16.0);
    assert_eq!(h.drawn("channel.sep").len(), 4);
    // Cada canal con su punto y su número, y el hex.
    for (dot, segment, value) in [
        ("channel.dot.r", r, "10"),
        ("channel.dot.g", g, "20"),
        ("channel.dot.b", b, "30"),
        (
            "channel.dot.a",
            Rect::from_min_max(a[0].min, hex.left_bottom()),
            "200",
        ),
    ] {
        let drawn = h.drawn(dot);
        assert_eq!(drawn.len(), 1, "{dot}");
        assert_eq!(drawn[0].size(), vec2(6.0, 6.0));
        assert!(segment.contains_rect(drawn[0]), "{dot}");
        let (color, ink) = h.text(value);
        assert_eq!(color, text_color, "{value}");
        assert!(segment.contains_rect(ink), "{value} {ink:?} in {segment:?}");
        assert!(ink.left() > drawn[0].right(), "{value} after its dot");
    }
    let (color, ink) = h.text("#0a141ec8");
    assert_eq!(color, text_color);
    // Como en el diseño: el punto rojo en x 818 (5 después del borde) y el "2" en la 825, el
    // "#" 3 después del comienzo del hex, y los dígitos de la fila 4 a la 9 de la tira.
    assert_eq!(h.drawn("channel.dot.r")[0].min, r.min + vec2(5.0, 4.0));
    assert_eq!(h.text("10").1.min, r.min + vec2(12.0, 4.0));
    assert_eq!(h.text("10").1.height(), 6.0);
    assert_eq!(ink.min, hex.min + vec2(3.0, 4.0));
}

#[test]
fn with_pixel_ui_dragging_or_typing_red_changes_the_color() {
    let mut h = Harness::new(true);
    let red = h.node("R");
    h.drag(red.center(), 40.0);
    let dragged = h.primary();
    assert!(dragged.r > 10, "{dragged:?}");
    assert_eq!((dragged.g, dragged.b, dragged.a), (20, 30, 255));
    h.click_at(h.node("R").center());
    h.type_and_enter("200");
    assert_eq!(h.primary(), Rgba::rgb(200, 20, 30));
    // El hex se escribe.
    let hex = h.node("Hex");
    h.click_at(hex.center());
    h.frame(vec![Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    h.type_and_enter("#336699");
    assert_eq!(h.primary(), Rgba::rgb(0x33, 0x66, 0x99));
}

#[test]
fn with_pixel_ui_the_copy_button_copies_the_hex_of_the_color() {
    let mut h = Harness::new(true);
    let copy = h.node("Copy color");
    assert_eq!(h.drawn("picker.copy"), vec![copy]);
    h.frame(vec![Event::PointerMoved(copy.center())]);
    h.press(copy.center(), PointerButton::Primary, true);
    h.press(copy.center(), PointerButton::Primary, false);
    assert_eq!(h.copied, vec!["#0a141e".to_owned()]);
    // Con alfa, los 8 dígitos (como el hex de la tira).
    h.set_primary(Rgba::new(255, 0, 0, 128));
    h.press(copy.center(), PointerButton::Primary, true);
    h.press(copy.center(), PointerButton::Primary, false);
    assert_eq!(h.copied, vec!["#ff000080".to_owned()]);
}

/// El color en HSV "gamma", como lo ve el selector.
fn hsv(c: Rgba) -> egui::ecolor::HsvaGamma {
    egui::ecolor::Hsva::from_srgba_unmultiplied([c.r, c.g, c.b, c.a]).into()
}

#[test]
fn with_pixel_ui_dragging_in_the_sv_square_changes_s_and_v_and_the_cursor_follows() {
    let mut h = Harness::new(true);
    h.set_primary(Rgba::rgb(255, 0, 0));
    let sv = h.node("Saturation and value");
    // El cuadro del diseño: 86 de alto, 3 px adentro de su caja; el ancho, el del panel con el
    // máximo del selector.
    assert_eq!(sv.height(), 86.0);
    assert!(sv.width() <= panels::COLOR_PICKER_MAX, "{sv:?}");
    let sv_box = h.drawn_boxes("picker.sv")[0];
    assert_eq!(sv.min, sv_box.min + vec2(3.0, 3.0));
    // Al medio: la mitad de saturación y de brillo, el mismo tono.
    let p = sv.min + vec2(91.5, 43.5);
    h.frame(vec![Event::PointerMoved(p)]);
    h.press(p, PointerButton::Primary, true);
    let picked = hsv(h.primary());
    assert!((picked.s - p_s(sv, p)).abs() < 0.02, "{picked:?}");
    assert!((picked.v - p_v(sv, p)).abs() < 0.02, "{picked:?}");
    assert!(picked.h.abs() < 0.01 || (picked.h - 1.0).abs() < 0.01);
    // El cursor del diseño, centrado donde se apretó.
    let cursor = h.drawn("picker.cursor");
    assert_eq!(cursor.len(), 1);
    assert_eq!(cursor[0].size(), vec2(9.0, 9.0));
    assert!((cursor[0].center() - p).length() <= 1.0, "{cursor:?} {p:?}");
    // Arrastrando arriba a la izquierda: blanco.
    let corner = sv.min + vec2(-10.0, -10.0);
    h.frame(vec![Event::PointerMoved(corner)]);
    h.press(corner, PointerButton::Primary, false);
    assert_eq!(h.primary(), Rgba::rgb(255, 255, 255));
    assert_eq!(
        h.drawn("picker.cursor")[0].center(),
        sv.min + vec2(0.5, 0.5)
    );
}

fn p_s(sv: Rect, p: Pos2) -> f32 {
    (p.x - sv.left()) / sv.width()
}

fn p_v(sv: Rect, p: Pos2) -> f32 {
    1.0 - (p.y - sv.top()) / sv.height()
}

#[test]
fn with_pixel_ui_the_hue_and_alpha_bars_work_with_their_cursors() {
    let mut h = Harness::new(true);
    h.set_primary(Rgba::rgb(255, 0, 0));
    let hue = h.node("Hue");
    let alpha = h.node("Alpha");
    let sv = h.node("Saturation and value");
    // Las dos barras de 13, una sobre la otra, del ancho del cuadro, en la caja de abajo.
    assert_eq!(hue.height(), 13.0);
    assert_eq!(alpha.height(), 13.0);
    assert_eq!(alpha.top(), hue.bottom());
    assert_eq!((hue.left(), hue.width()), (sv.left(), sv.width()));
    let bars = h.drawn_boxes("picker.bars")[0];
    assert_eq!(hue.top(), bars.top() + 2.0);
    // Los cursores: con tono 0, a 1 px de la punta izquierda; con alfa 255, a 1 px de la
    // derecha. 3 filas abajo del borde de cada barra.
    let cursors = h.drawn("picker.cursor.bar");
    assert_eq!(
        cursors,
        vec![
            Rect::from_min_size(hue.min + vec2(1.0, 3.0), Vec2::splat(7.0)),
            Rect::from_min_size(
                pos2(alpha.right() - 8.0, alpha.top() + 3.0),
                Vec2::splat(7.0)
            ),
        ]
    );
    // Un clic al medio del tono: cian.
    let middle = pos2(hue.left() + 4.5 + (hue.width() - 9.0) / 2.0, hue.center().y);
    h.click_at(middle);
    assert_eq!(h.primary(), Rgba::rgb(0, 255, 255));
    let cursor = h.drawn("picker.cursor.bar")[0];
    assert!((cursor.center().x - middle.x).abs() <= 1.0, "{cursor:?}");
    // Un clic al principio del alfa: transparente.
    h.click_at(pos2(alpha.left() + 2.0, alpha.center().y));
    assert_eq!(h.primary(), Rgba::new(0, 255, 255, 0));
    assert_eq!(
        h.drawn("picker.cursor.bar")[1],
        Rect::from_min_size(alpha.min + vec2(1.0, 3.0), Vec2::splat(7.0))
    );
}

#[test]
fn without_pixel_ui_the_palette_and_the_picker_are_the_egui_ones() {
    let mut h = Harness::new(false);
    assert!(!h.has("Add to palette") && !h.has("Copy color") && !h.has("Hue"));
    // Sin los botones del diseño: el combo Presets y el menú de siempre.
    for label in ["Lock palette", "Palette menu", "New palette"] {
        assert!(!h.has(label), "{label}");
    }
    assert!(h.has("Presets ⏷") && h.has("…"), "{:?}", h.nodes);
    assert!(h.editor.palette_locked);
    // Los cuadritos de 20 de siempre y el selector de egui con sus campos "R ".
    let color = to_color32(h.editor.sprite().palette.colors()[0]);
    assert!(
        h.colored_rects()
            .iter()
            .any(|&(c, r)| c == color && r.size() == Vec2::splat(20.0)),
    );
    let texts: Vec<String> = h.texts().into_iter().map(|(t, _, _)| t).collect();
    for text in ["R ", "10", "G ", "20", "📋", "#0a141e", "+"] {
        assert!(texts.iter().any(|t| t == text), "{text} in {texts:?}");
    }
    // Un clic en un cuadrito elige el color, como siempre.
    let first = h
        .colored_rects()
        .into_iter()
        .find(|&(c, r)| c == color && r.size() == Vec2::splat(20.0))
        .unwrap()
        .1;
    h.click_at(first.center());
    assert_eq!(h.editor.primary, h.editor.palette_value(0));
}

fn key(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers,
    }
}

impl Harness {
    /// Clic en el hex y todo su texto seleccionado (para escribir encima).
    fn focus_hex(&mut self) {
        self.click_at(self.node("Hex").center());
        self.frame(vec![key(egui::Key::A, true, egui::Modifiers::COMMAND)]);
        self.frame(vec![key(egui::Key::A, false, egui::Modifiers::COMMAND)]);
    }

    fn press_key(&mut self, k: egui::Key) {
        self.frame(vec![key(k, true, Default::default())]);
        self.frame(vec![key(k, false, Default::default())]);
        self.frame(vec![]);
    }

    /// Clic en el canal `label` y escribir `text` con Enter.
    fn type_into(&mut self, label: &str, text: &str) {
        self.click_at(self.node(label).center());
        self.frame(vec![key(egui::Key::A, true, egui::Modifiers::COMMAND)]);
        self.type_and_enter(text);
    }

    fn has_text(&self, text: &str) -> bool {
        self.texts().iter().any(|(t, _, _)| t == text)
    }
}

#[test]
fn with_pixel_ui_escape_in_the_hex_cancels_and_restores_the_hex_of_the_color() {
    let mut h = Harness::new(true);
    h.focus_hex();
    h.frame(vec![Event::Text("#336699".to_owned())]);
    h.press_key(egui::Key::Escape);
    assert_eq!(h.primary(), Rgba::rgb(10, 20, 30), "Esc does not apply");
    h.frame(vec![]);
    assert!(h.has_text("#0a141e"), "{:?}", h.texts());
    assert!(!h.has_text("#336699"));
    // Al volver a entrar, el texto es el del color, no lo escrito antes.
    h.click_at(h.node("Hex").center());
    assert!(h.has_text("#0a141e"), "{:?}", h.texts());
}

#[test]
fn losing_the_hex_focus_does_not_undo_an_edit_of_another_channel_in_the_same_frame() {
    let current = Rgba::rgb(10, 20, 30);
    let edited = Rgba::rgb(99, 20, 30);
    // Con otro canal editado en el mismo frame, gana esa edición.
    assert_eq!(
        super::strip_result(current, edited, Some("#ffffff")),
        edited
    );
    // Sin otra edición, el hex escrito se aplica; si no es válido, queda el color.
    assert_eq!(
        super::strip_result(current, current, Some("#ffffff")),
        Rgba::rgb(255, 255, 255)
    );
    assert_eq!(super::strip_result(current, current, Some("zz")), current);
    assert_eq!(super::strip_result(current, current, None), current);
}

#[test]
fn with_pixel_ui_the_focused_hex_follows_the_color_until_the_user_types() {
    let mut h = Harness::new(true);
    h.focus_hex();
    // El color cambia por otro lado (el S/V, el gotero) con el hex enfocado: sin escribir, el
    // texto sigue al color, y Enter no vuelve al de antes.
    h.set_primary(Rgba::rgb(1, 2, 3));
    h.frame(vec![]);
    assert!(h.has_text("#010203"), "{:?}", h.texts());
    h.press_key(egui::Key::Enter);
    assert_eq!(h.primary(), Rgba::rgb(1, 2, 3));
    // Con algo escrito, lo escrito no se pisa.
    h.focus_hex();
    h.frame(vec![Event::Text("#4455".to_owned())]);
    h.set_primary(Rgba::rgb(7, 8, 9));
    h.frame(vec![]);
    assert!(h.has_text("#4455"), "{:?}", h.texts());
    h.frame(vec![Event::Text("66".to_owned())]);
    h.press_key(egui::Key::Enter);
    assert_eq!(h.primary(), Rgba::rgb(0x44, 0x55, 0x66));
}

#[test]
fn with_pixel_ui_a_channel_clamps_what_is_typed_to_0_255() {
    let mut h = Harness::new(true);
    h.type_into("R", "300");
    assert_eq!(h.primary(), Rgba::rgb(255, 20, 30));
    h.type_into("G", "-5");
    assert_eq!(h.primary(), Rgba::rgb(255, 0, 30));
}

#[test]
fn with_pixel_ui_an_invalid_hex_keeps_the_color() {
    let mut h = Harness::new(true);
    h.focus_hex();
    h.type_and_enter("#zz12");
    assert_eq!(h.primary(), Rgba::rgb(10, 20, 30));
    h.frame(vec![]);
    assert!(h.has_text("#0a141e"), "{:?}", h.texts());
}

#[test]
fn with_pixel_ui_typing_into_g_b_and_a_changes_the_color() {
    let mut h = Harness::new(true);
    h.type_into("G", "100");
    assert_eq!(h.primary(), Rgba::rgb(10, 100, 30));
    h.type_into("B", "200");
    assert_eq!(h.primary(), Rgba::rgb(10, 100, 200));
    h.type_into("A", "128");
    assert_eq!(h.primary(), Rgba::new(10, 100, 200, 128));
}

/// Si el nodo de accesibilidad `label` se anuncia elegido (prendido).
fn selected(output: &egui::FullOutput, label: &str) -> Option<bool> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update
        .nodes
        .iter()
        .find(|(_, n)| n.label().is_some_and(|l| l == label))
        .and_then(|(_, n)| n.toggled())
        .map(|t| t == egui::accesskit::Toggled::True)
}

impl Harness {
    /// Si el botón `label` se anuncia elegido (prendido), en un frame sin eventos.
    fn is_selected(&mut self, label: &str) -> Option<bool> {
        self.time += 0.05;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 800.0))),
            time: Some(self.time),
            ..Default::default()
        };
        let editor = &mut self.editor;
        let skin = self.skin.as_ref();
        let output = self.ctx.run_ui(input, |ui| {
            egui::Panel::right("palette")
                .default_size(220.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        panels::palette(ui, editor, true, skin);
                    });
                });
        });
        selected(&output, label)
    }
}

#[test]
fn the_palette_buttons_are_in_the_embedded_skin() {
    let ctx = egui::Context::default();
    let skin = Skin::embedded(&ctx);
    let size = |name: &str| skin.piece(name, WidgetState::Normal).map(|p| p.size);
    for (name, piece) in [
        ("palette.buttons", [5, 6]),
        ("palette.buttons.sep", [3, 16]),
        ("palette.button", [5, 6]),
        ("palette.button.active", [3, 3]),
        ("palette.icon.lock", [7, 9]),
        ("palette.icon.menu", [7, 7]),
        ("palette.icon.new", [9, 9]),
        ("palette.icon.mask", [18, 9]),
    ] {
        assert_eq!(size(name), Some(piece), "{name}");
    }
}

#[test]
fn with_pixel_ui_the_top_buttons_are_the_design_group_and_the_checker() {
    let h = Harness::new(true);
    let pbox = h.drawn_boxes("palette.box")[0];
    // El grupo (x 786..858, y 99..114): 2 px adentro de la columna y 5 filas arriba de la caja.
    let group = h.drawn_boxes("palette.buttons");
    assert_eq!(group.len(), 1, "{group:?}");
    let group = group[0];
    assert_eq!(group.size(), vec2(73.0, 16.0));
    assert_eq!(
        group.left_bottom(),
        pos2(pbox.left() + 2.0, pbox.top() - 5.0)
    );
    // Los separadores, en la 809 y la 833.
    let seps: Vec<f32> = h
        .drawn("palette.buttons.sep")
        .iter()
        .map(|r| r.left())
        .collect();
    assert_eq!(seps, vec![group.left() + 23.0, group.left() + 47.0]);
    // Un botón cada 24, con su ícono donde está en el diseño.
    for (k, (label, icon, at)) in [
        ("Lock palette", "palette.icon.lock", vec2(9.0, 3.0)),
        ("Palette menu", "palette.icon.menu", vec2(33.0, 4.0)),
        ("New palette", "palette.icon.new", vec2(56.0, 3.0)),
    ]
    .into_iter()
    .enumerate()
    {
        let button = h.node(label);
        assert_eq!(button.left(), group.left() + 24.0 * k as f32, "{label}");
        assert_eq!((button.top(), button.height()), (group.top(), 16.0));
        assert_eq!(h.drawn(icon)[0].min, group.min + at, "{icon}");
    }
    // El damero (x 947..970): 24 de ancho, 2 px antes del borde derecho, a la altura del grupo.
    let checker = h.drawn_boxes("palette.button")[0];
    assert_eq!(checker.size(), vec2(24.0, 16.0));
    assert_eq!(checker.right_top(), pos2(pbox.right() - 2.0, group.top()));
    assert_eq!(h.node("Mask"), checker);
    assert_eq!(
        h.drawn("palette.icon.mask")[0].min,
        checker.min + vec2(3.0, 3.0)
    );
    // Nada prendido: el diseño, con la paleta bloqueada y sin el Mask.
    assert!(h.drawn("palette.button.active").is_empty());
}

#[test]
fn with_pixel_ui_the_lock_starts_on_and_blocks_editing_a_palette_color() {
    let mut h = Harness::new(true);
    assert!(h.editor.palette_locked);
    assert_eq!(h.is_selected("Lock palette"), Some(true));
    let before = h.editor.sprite().palette.clone();
    // Bloqueada: elegir #3 y cambiar el rojo cambia el primario, no la paleta.
    h.click_at(h.node("#3").center());
    h.type_into("R", "200");
    assert_eq!(h.primary().r, 200);
    assert_eq!(h.editor.sprite().palette, before);
    // Desbloqueada: el botón se ve apretado y el mismo cambio edita la entrada #3.
    h.click_at(h.node("Lock palette").center());
    assert!(!h.editor.palette_locked);
    assert_eq!(h.is_selected("Lock palette"), Some(false));
    let lock = h.node("Lock palette");
    assert_eq!(
        h.drawn_boxes("palette.button.active"),
        vec![Rect::from_min_size(
            lock.min + vec2(1.0, 1.0),
            vec2(23.0, 13.0)
        )]
    );
    h.click_at(h.node("#3").center());
    h.type_into("R", "201");
    let edited = h.editor.sprite().palette.colors()[3];
    assert_eq!(edited.r, 201);
    assert_eq!(h.primary(), edited, "the primary follows the entry");
    // Arrastrar el S/V también la edita, en un solo paso de undo.
    let sv = h.node("Saturation and value");
    h.drag(sv.center(), 30.0);
    assert_ne!(h.editor.sprite().palette.colors()[3], edited);
    assert_eq!(h.editor.sprite().palette.colors()[3], h.primary());
    h.editor.undo();
    assert_eq!(h.editor.sprite().palette.colors()[3], edited);
    h.editor.undo();
    assert_eq!(h.editor.sprite().palette, before);
    // Agregar con la marca se puede siempre, también bloqueada.
    h.click_at(h.node("Lock palette").center());
    assert!(h.editor.palette_locked);
    h.click_at(h.node("Add to palette").center());
    assert_eq!(h.editor.sprite().palette.len(), 22);
}

#[test]
fn with_pixel_ui_the_menu_button_opens_the_presets_and_palette_menu() {
    let mut h = Harness::new(true);
    // El combo Presets y el menú de antes ya no están: sus funciones van en el botón del menú.
    assert!(!h.has("Presets ⏷") && !h.has("…"), "{:?}", h.nodes);
    assert!(!h.has("Presets…"));
    h.click_at(h.node("Palette menu").center());
    for label in [
        "Presets…",
        "Load Palette…",
        "Save Palette…",
        "Save as Preset…",
        "Set as Default",
        "Load Default",
        "Open Folder",
    ] {
        assert!(h.has(label), "{label} in {:?}", h.nodes);
    }
    let presets = h.node("Presets…").center();
    h.frame(vec![Event::PointerMoved(presets)]);
    h.press(presets, PointerButton::Primary, true);
    h.press(presets, PointerButton::Primary, false);
    assert_eq!(h.action, Some(panels::PaletteAction::OpenPresets));
    h.frame(vec![]);
    assert!(!h.has("Presets…"), "choosing closes the menu");
}

#[test]
fn with_pixel_ui_the_page_button_makes_a_new_palette() {
    let mut h = Harness::new(true);
    let before = h.editor.sprite().palette.clone();
    h.click_at(h.node("New palette").center());
    assert_eq!(h.editor.sprite().palette.colors(), &[Rgba::rgb(10, 20, 30)]);
    assert!(h.has("#0") && !h.has("#1"));
    h.editor.undo();
    assert_eq!(h.editor.sprite().palette, before);
}

#[test]
fn with_pixel_ui_the_checker_button_toggles_the_mask() {
    let mut h = Harness::new(true);
    assert_eq!(h.is_selected("Mask"), Some(false));
    h.click_at(h.node("Mask").center());
    assert!(h.editor.is_mask(h.editor.primary));
    assert_eq!(h.is_selected("Mask"), Some(true));
    let checker = h.node("Mask");
    assert_eq!(
        h.drawn_boxes("palette.button.active"),
        vec![Rect::from_min_size(
            checker.min + vec2(1.0, 1.0),
            vec2(22.0, 13.0)
        )]
    );
    h.click_at(h.node("Mask").center());
    assert_eq!(h.primary(), Rgba::rgb(10, 20, 30));
    assert_eq!(h.is_selected("Mask"), Some(false));
    // Clic derecho: el Mask como secundario.
    h.click_with(h.node("Mask").center(), PointerButton::Secondary);
    assert!(h.editor.is_mask(h.editor.secondary));
}
