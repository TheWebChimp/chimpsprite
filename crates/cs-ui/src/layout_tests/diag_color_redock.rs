//! Color flotando vuelve a la columna derecha arrastrándolo (el diagnóstico de 4036dbb, ahora
//! con el destino de orilla: ver `workspace::drop::edge_under` y `DockTarget::Edge`), y lo que
//! mide un lado del lienzo que parte egui_dock.

use super::*;
use crate::dock::Edge;
use crate::panel_registry::PanelId;
use crate::workspace::{DockTarget, EDGE_BAND};

/// Dónde queda flotando Color al empezar cada caso.
#[derive(Clone, Copy, Debug)]
enum Start {
    /// `Workspace::float` a mitad del lienzo (lejos de la orilla derecha).
    Middle,
    /// `Workspace::float` pegado a la orilla derecha (tapando la tira y el cuarto derecho del
    /// lienzo), como queda si se saca de la columna y se suelta cerca.
    Right,
    /// Como lo haría el usuario: la franja de agarre de Color (sin barra, en la columna)
    /// arrastrada al lienzo, al anillo de "ventana nueva" de egui_dock a la derecha del centro.
    Dragged,
}

/// Essentials con Pixel UI y una pestaña nueva, en una ventana de `size`.
fn essentials(size: Vec2) -> LayoutHarness {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
    app.new_document();
    LayoutHarness::start_in(app, size)
}

/// Essentials en una ventana de `size`, con Color flotando sobre el lienzo.
fn color_floating(size: Vec2, start: Start) -> LayoutHarness {
    let mut h = essentials(size);
    let canvas = h.leaf(PanelId::Canvas);
    match start {
        Start::Middle => {
            let at = pos2(canvas.center().x - 60.0, canvas.top() + 30.0);
            h.app.workspace.float(PanelId::Color, at);
        }
        Start::Right => {
            let at = pos2(h.screen().right() - 230.0, canvas.top() + 10.0);
            h.app.workspace.float(PanelId::Color, at);
        }
        Start::Dragged => {
            let leaf = h.leaf(PanelId::Color);
            let from = pos2(leaf.left() + 20.0, leaf.top() + 1.0);
            let to = pos2(canvas.center().x + canvas.width() * 0.18, canvas.center().y);
            dock_drag(&mut h, from, to);
        }
    }
    for _ in 0..5 {
        h.frame();
    }
    assert!(
        h.app.workspace.floating(PanelId::Color),
        "{start:?}: floats"
    );
    h
}

/// Lo que se ve del acomodo.
fn describe(h: &LayoutHarness) -> String {
    let w = &h.app.workspace;
    let mut out = String::new();
    for id in [
        PanelId::Tools,
        PanelId::Canvas,
        PanelId::Timeline,
        PanelId::Color,
    ] {
        out += &format!(
            "  {id:?}: at {:?} rect {:?} floating {}\n",
            w.leaf_of(id),
            w.leaf_rect(id),
            w.floating(id)
        );
    }
    out += &format!(
        "  column {:?} timeline_below {} palette_width {:?} natural {:?}\n",
        w.column_rect(),
        w.timeline_below(),
        h.app.view.palette_width,
        h.app.palette_natural_width
    );
    out
}

/// Si `h` es Essentials como `reference`: el mismo árbol, los mismos rects de cada grupo, de la
/// columna y de la tira, y los mismos anchos de la columna.
fn assert_like(h: &LayoutHarness, reference: &LayoutHarness, case: &str) {
    let (w, r) = (&h.app.workspace, &reference.app.workspace);
    let why = || format!("{case}:\n{}vs\n{}", describe(h), describe(reference));
    assert_eq!(w.main_shape(), r.main_shape(), "{}", why());
    assert!(!w.floating(PanelId::Color), "{}", why());
    for id in [
        PanelId::Tools,
        PanelId::Canvas,
        PanelId::Timeline,
        PanelId::Color,
    ] {
        assert_eq!(w.leaf_rect(id), r.leaf_rect(id), "{id:?} {}", why());
    }
    assert_eq!(w.column_rect(), r.column_rect(), "{}", why());
    assert_eq!(h.panel("dock"), reference.panel("dock"), "{}", why());
    assert_eq!(h.app.view.palette_width, reference.app.view.palette_width);
    assert_eq!(
        h.app.palette_natural_width,
        reference.app.palette_natural_width,
        "{}",
        why()
    );
}

/// Si `a` y `b` son el mismo rect a medio punto (las zonas pintadas se miden de sus piezas).
fn near(a: Rect, b: Rect) -> bool {
    (a.min - b.min).length() <= 0.5 && (a.max - b.max).length() <= 0.5
}

#[test]
fn docking_color_at_the_right_edge_draws_exactly_essentials() {
    for size in [vec2(1200.0, 800.0), vec2(960.0, 505.0)] {
        let reference = essentials(size);
        let mut h = essentials(size);
        h.app.workspace.float(PanelId::Color, pos2(300.0, 100.0));
        for _ in 0..3 {
            h.frame();
        }
        h.app
            .workspace
            .dock(PanelId::Color, DockTarget::Edge(Edge::Right));
        for _ in 0..3 {
            h.frame();
        }
        assert_like(&h, &reference, &format!("{size:?}"));
    }
}

#[test]
fn the_floating_color_tab_dropped_on_the_right_edge_band_or_strip_gives_back_essentials() {
    // Antes (4036dbb) ningún soltar dejaba la columna de alto completo: egui_dock solo parte la
    // hoja bajo el puntero. Ahora la franja junto a la tira derecha y la tira misma (Color no va
    // a la tira) son la orilla: la columna de Essentials, del ancho de siempre. Mientras se
    // sostiene, la zona pintada es esa columna (y la de egui_dock no se ve). Con la ventana
    // pegada a la orilla (tapando la tira), igual: su propia ventana no le tapa la orilla.
    for size in [vec2(1200.0, 800.0), vec2(960.0, 505.0)] {
        let reference = essentials(size);
        let column = reference.app.workspace.column_rect().unwrap();
        for start in [Start::Middle, Start::Dragged, Start::Right] {
            let probe = color_floating(size, start);
            let strip = probe.panel("dock");
            let canvas = probe.leaf(PanelId::Canvas);
            let timeline = probe.leaf(PanelId::Timeline);
            let screen = probe.screen();
            let tab = probe.button("Color").center();
            let band = |y| pos2(strip.left() - EDGE_BAND / 2.0, y);
            let targets = [
                ("band, canvas", band(canvas.center().y)),
                ("band, timeline", band(timeline.center().y)),
                (
                    "between canvas and strip",
                    pos2(strip.left() - 2.0, canvas.center().y),
                ),
                ("strip middle", strip.center()),
                ("strip top", pos2(strip.center().x, canvas.top() + 40.0)),
                (
                    "strip over the timeline",
                    pos2(strip.center().x, timeline.center().y),
                ),
                (
                    "screen right edge",
                    pos2(screen.right() - 1.0, canvas.center().y),
                ),
            ];
            for (name, to) in targets {
                let case = format!("{size:?} {start:?} {name} {to:?}");
                let mut h = color_floating(size, start);
                hold_drag(&mut h, tab, to);
                let zones = drop_zones(&h);
                assert!(
                    zones.len() == 1 && near(zones[0], column),
                    "{case}: {zones:?} vs {column:?}"
                );
                release_at(&mut h, to);
                for _ in 0..3 {
                    h.frame();
                }
                assert_like(&h, &reference, &case);
                assert_eq!(drop_zones(&h), [], "{case}: nothing painted after");
            }
        }
    }
}

#[test]
fn the_floating_color_tab_dropped_quickly_on_the_strip_gives_back_essentials() {
    // Soltar en cuanto llega (sin quedarse quieto, sin el `max_preference_time` de egui_dock).
    let size = vec2(960.0, 505.0);
    let reference = essentials(size);
    let mut h = color_floating(size, Start::Middle);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (from, to) = (h.button("Color").center(), h.panel("dock").center());
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from, true)]);
    for t in [0.1, 0.4, 0.7, 1.0] {
        h.frame_with(vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    h.frame_with(vec![press(to, false)]);
    for _ in 0..3 {
        h.frame();
    }
    assert_like(&h, &reference, "quick release");
}

#[test]
fn a_strip_icon_dropped_on_the_left_edge_band_is_a_full_height_column_left_of_the_toolbar() {
    // Library, de la tira derecha a la franja de la orilla izquierda: una columna de alto
    // completo en la orilla (la toolbar queda entre ella y el lienzo, del ancho de su caja), del
    // ancho de siempre de Library. Mientras se sostiene, se pinta esa columna.
    let mut h = essentials(vec2(1200.0, 800.0));
    let tools = h.leaf(PanelId::Tools);
    let canvas = h.leaf(PanelId::Canvas);
    let dock = tools.union(h.app.workspace.column_rect().unwrap());
    let to = pos2(dock.left() + EDGE_BAND / 2.0, canvas.center().y);
    let width = h
        .app
        .workspace
        .preferred_side(PanelId::Library, true)
        .min(dock.width() * 0.45);
    let from = h.button("Library").center();
    hold_drag(&mut h, from, to);
    let zones = drop_zones(&h);
    assert_eq!(zones.len(), 1, "{zones:?}");
    let zone = zones[0];
    let column = Rect::from_min_size(
        dock.min,
        vec2(width + crate::pixel_ui::ETCHED_SIZE / 2.0, dock.height()),
    );
    assert!(near(zone, column), "{zone:?} vs {column:?}");
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    let library = h.leaf(PanelId::Library);
    assert!(w.docked(PanelId::Library) && w.strip_of(PanelId::Library).is_none());
    assert_eq!(library.left(), dock.left());
    assert_eq!(
        (library.top(), library.bottom()),
        (dock.top(), dock.bottom())
    );
    assert!(
        (library.width() - zone.width()).abs() <= 2.0,
        "{library:?} vs {zone:?}"
    );
    // La toolbar, a su derecha y del ancho de su caja; el lienzo, después.
    let after = h.leaf(PanelId::Tools);
    assert!(after.left() > library.right(), "{after:?} {library:?}");
    assert_eq!(after.width(), tools.width());
    assert_eq!(w.tools_side(), Some(true));
    assert!(h.leaf(PanelId::Canvas).left() > after.right());
    assert!(w.timeline_below());
    assert_eq!(w.column_panels(), [PanelId::Color]);
}

#[test]
fn a_tab_dropped_on_a_strip_goes_into_it_where_it_was_dropped() {
    // Library (pestaña junto a Color) soltada sobre la tira derecha, entre sus dos primeros
    // íconos: a la tira, en ese lugar (no se acopla en ninguna columna).
    let mut h = library_beside_color();
    let strip = h.app.workspace.strip_at(Edge::Right).to_vec();
    let first = h.button(strip[0].label());
    let second = h.button(strip[1].label());
    let to = pos2(first.center().x, (first.bottom() + second.top()) / 2.0);
    let from = h.button("Library").center();
    hold_drag(&mut h, from, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    assert_eq!(w.strip_of(PanelId::Library), Some(Edge::Right));
    assert_eq!(w.strip_at(Edge::Right)[1], PanelId::Library);
    assert_eq!(w.strip_at(Edge::Right)[0], strip[0]);
    assert!(!w.docked(PanelId::Library) && !w.floating(PanelId::Library));
    assert_eq!(w.places(PanelId::Library), 1);
}

#[test]
fn a_canvas_split_made_by_egui_dock_gives_the_panel_its_usual_width() {
    // Antes (4036dbb), soltado en el cuarto derecho del lienzo (lo parte egui_dock, fracción
    // 0.5), el lado nuevo se quedaba con la mitad del lienzo, y la columna lo guardaba como su
    // ancho natural. Ahora: Color, la columna de su ancho de siempre (el natural no cambia).
    let size = vec2(960.0, 505.0);
    let reference = essentials(size);
    let column = reference.app.workspace.column_rect().unwrap();
    let mut h = color_floating(size, Start::Middle);
    let before = h.app.palette_natural_width;
    let canvas = h.leaf(PanelId::Canvas);
    let tab = h.button("Color").center();
    let to = pos2(canvas.right() - canvas.width() * 0.2, canvas.center().y);
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    assert!(w.docked(PanelId::Color), "{}", describe(&h));
    let got = w.column_rect().expect("a column");
    assert_eq!(got.width(), column.width(), "{}", describe(&h));
    assert_eq!(h.app.palette_natural_width, before);
    assert_eq!(h.app.view.palette_width, None);
    // Library (de junto a Color), igual: su lado mide lo de siempre de Library, no la mitad.
    let mut h = library_beside_color();
    let natural = h.app.palette_natural_width;
    let canvas = h.leaf(PanelId::Canvas);
    let tab = h.button("Library").center();
    let to = pos2(canvas.right() - canvas.width() * 0.15, canvas.center().y);
    // (A lo más el 45 % de lo que parte, como cualquier lado nuevo: el del lienzo.)
    let width = h
        .app
        .workspace
        .preferred_side(PanelId::Library, true)
        .min(canvas.width() * 0.45);
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let library = h.leaf(PanelId::Library);
    let canvas_after = h.leaf(PanelId::Canvas);
    assert!(library.left() > canvas_after.left(), "{}", describe(&h));
    assert!(
        (library.width() + crate::pixel_ui::ETCHED_SIZE / 2.0 - width).abs() <= 1.5,
        "{library:?} vs {width} (canvas {canvas:?})"
    );
    assert_eq!(h.app.palette_natural_width, natural);
    assert_eq!(h.app.view.palette_width, None);
}

#[test]
fn the_floating_color_tab_dropped_in_the_new_window_ring_floats_where_it_was_dropped() {
    // Causa 3 de 4036dbb: soltada en el anillo de "ventana nueva" de egui_dock, la ventana nueva
    // de lo que ya flotaba aparecía en un lugar fijo (~360,105), no donde se soltó.
    let size = vec2(960.0, 505.0);
    let mut h = color_floating(size, Start::Middle);
    let canvas = h.leaf(PanelId::Canvas);
    let tab = h.button("Color").center();
    let to = pos2(canvas.center().x + canvas.width() * 0.15, canvas.center().y);
    assert!(!h.window(PanelId::Color).contains(to));
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.workspace.floating(PanelId::Color), "{}", describe(&h));
    let window = h.window(PanelId::Color);
    assert!(
        (window.min - to.round()).length() <= 1.0,
        "{window:?} vs {to:?}"
    );
}

#[test]
fn a_docked_group_dragged_a_little_from_its_grab_strip_stays_where_it_was() {
    // La franja de agarre de un grupo de hasta arriba está en la franja de la orilla de arriba:
    // mientras el puntero siga sobre su propio grupo, no cuenta (si no, moverlo un poco lo
    // mandaría a una fila de ancho completo arriba).
    let size = vec2(1200.0, 800.0);
    let reference = essentials(size);
    let mut h = essentials(size);
    let leaf = h.leaf(PanelId::Color);
    let from = pos2(leaf.left() + 20.0, leaf.top() + 1.0);
    let to = from + vec2(12.0, 6.0);
    hold_drag(&mut h, from, to);
    assert!(
        !drop_zones(&h).iter().any(|z| z.width() > leaf.width()),
        "{:?}",
        drop_zones(&h)
    );
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert_like(&h, &reference, "grab strip wiggle");
}

// --- Lo de la revisión de la tarea 1 ---

#[test]
fn color_dragged_from_the_column_onto_the_right_strip_stays_the_essentials_column() {
    // La tira derecha, con Color en la columna, va entre el lienzo y la columna: soltar ahí la
    // franja de agarre de Color lo deja donde estaba (egui_dock lo mete junto al lienzo y el
    // workspace lo regresa a la orilla: la misma forma de antes, y sus lados en puntos).
    for size in [vec2(1200.0, 800.0), vec2(960.0, 505.0)] {
        let reference = essentials(size);
        let mut h = essentials(size);
        let leaf = h.leaf(PanelId::Color);
        let from = pos2(leaf.left() + 20.0, leaf.top() + 1.0);
        let to = h.panel("dock").center();
        hold_drag(&mut h, from, to);
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        assert_like(&h, &reference, &format!("{size:?}"));
    }
}

#[test]
fn a_tab_joining_a_floating_window_does_not_move_that_window() {
    let mut h = essentials(vec2(1200.0, 800.0));
    h.app.workspace.float(PanelId::Library, pos2(600.0, 150.0));
    h.app.workspace.float(PanelId::Color, pos2(150.0, 150.0));
    for _ in 0..5 {
        h.frame();
    }
    let window = h.window(PanelId::Library);
    let tab = h.button("Color").center();
    let to = h.leaf(PanelId::Library).center();
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    assert_eq!(w.leaf_of(PanelId::Color), w.leaf_of(PanelId::Library));
    assert_eq!(h.window(PanelId::Library).min, window.min);
}

#[test]
fn the_right_columns_width_preference_is_colors_only() {
    // Library en la orilla derecha (Color flotando): la columna es suya, de su ancho de siempre
    // (como cualquier lado nuevo, a lo más el 45 % de lo que parte), no del de Color; arrastrar
    // su divisor no cambia el de Color, y Color de vuelta en la orilla derecha toma el suyo.
    for size in [vec2(1200.0, 800.0), vec2(960.0, 505.0)] {
        let reference = essentials(size);
        let column = reference.app.workspace.column_rect().unwrap();
        let mut h = color_floating(size, Start::Middle);
        let (palette, natural) = (h.app.view.palette_width, h.app.palette_natural_width);
        let canvas = h.leaf(PanelId::Canvas);
        let stack = canvas.union(h.leaf(PanelId::Timeline));
        let strip = h.panel("dock");
        let to = pos2(strip.left() - EDGE_BAND / 2.0, canvas.center().y);
        let width = h
            .app
            .workspace
            .preferred_side(PanelId::Library, true)
            .min(stack.width() * 0.45);
        let from = h.button("Library").center();
        hold_drag(&mut h, from, to);
        let zones = drop_zones(&h);
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        let case = format!("{size:?}:\n{}", describe(&h));
        let w = &h.app.workspace;
        assert_eq!(w.column_panels(), [PanelId::Library], "{case}");
        let got = w.column_rect().unwrap();
        let sep = crate::pixel_ui::ETCHED_SIZE;
        assert!(
            (got.width() - (width + sep / 2.0)).abs() <= 1.0,
            "{got:?} vs {width} {case}"
        );
        assert!(
            zones.len() == 1 && near(zones[0], got),
            "{zones:?} vs {got:?} {case}"
        );
        assert_eq!(h.app.view.palette_width, palette, "{case}");
        assert_eq!(h.app.palette_natural_width, natural, "{case}");
        // Su divisor, 40 más a la izquierda: es suyo.
        let edge = pos2(got.left(), got.center().y);
        drag(&mut h, edge, edge - vec2(40.0, 0.0));
        let dragged = h.app.workspace.column_rect().unwrap();
        assert!(
            (dragged.width() - got.width() - 40.0).abs() <= 1.0,
            "{dragged:?}"
        );
        assert_eq!(h.app.view.palette_width, palette, "{case}");
        assert_eq!(h.app.palette_natural_width, natural, "{case}");
        // Color de vuelta a la orilla derecha (por fuera de Library): del ancho de la columna de
        // Essentials.
        let tab = h.button("Color").center();
        let to = pos2(h.screen().right() - EDGE_BAND / 2.0, canvas.center().y);
        hold_drag(&mut h, tab, to);
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        let w = &h.app.workspace;
        assert_eq!(w.column_panels(), [PanelId::Color], "{}", describe(&h));
        assert_eq!(
            w.column_rect().unwrap().width(),
            column.width(),
            "{}",
            describe(&h)
        );
        assert_eq!(h.app.view.palette_width, palette);
        assert_eq!(h.app.palette_natural_width, natural);
    }
}

// --- Tarea 2: la ventana que se arrastra no tapa sus destinos ---

/// La opacidad de la ventana flotante de `id` en el último frame (1: opaca): con Pixel UI, la
/// del centro del 9-slice de su marco (`menu.side`, la malla más grande dentro de la ventana);
/// sin él, la del relleno de su `Frame::window`.
fn window_opacity(h: &LayoutHarness, id: PanelId) -> f32 {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::Shape>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            s => out.push(s.clone()),
        }
    }
    let window = h.window(id);
    let mut shapes = Vec::new();
    for clipped in &h.shapes {
        walk(&clipped.shape, &mut shapes);
    }
    let alpha = match h.app.pixel.skin() {
        Some(_) => {
            let frame = h.piece("menu.side");
            shapes.iter().find_map(|s| match s {
                egui::Shape::Mesh(mesh) if mesh.texture_id == frame => {
                    let bounds = mesh.calc_bounds();
                    let inside = window.expand(0.5).contains_rect(bounds);
                    (inside && bounds.area() > window.area() * 0.5)
                        .then(|| mesh.vertices.iter().map(|v| v.color.a()).max().unwrap_or(0))
                }
                _ => None,
            })
        }
        None => shapes.iter().find_map(|s| match s {
            egui::Shape::Rect(r) if near(r.rect, window) && r.fill.a() > 0 => Some(r.fill.a()),
            _ => None,
        }),
    };
    let alpha = alpha.unwrap_or_else(|| panic!("no frame for {id:?}'s window {window:?}"));
    f32::from(alpha) / 255.0
}

/// Si `a` es la opacidad de la ventana que se arrastra (el 40 %).
fn see_through(a: f32) -> bool {
    (a - 0.4).abs() < 0.02
}

/// Si en `h` Color quedó acoplado a la derecha del lienzo (solo del lienzo: el timeline sigue
/// debajo de los dos), del ancho de la columna de Essentials (`column`).
fn assert_split_right_of_the_canvas(h: &LayoutHarness, column: Rect, case: &str) {
    let w = &h.app.workspace;
    let why = || format!("{case}:\n{}", describe(h));
    assert!(w.docked(PanelId::Color), "{}", why());
    let (color, canvas) = (h.leaf(PanelId::Color), h.leaf(PanelId::Canvas));
    assert!(color.left() > canvas.right(), "{}", why());
    assert_eq!(
        (color.top(), color.bottom()),
        (canvas.top(), canvas.bottom()),
        "{}",
        why()
    );
    assert!(
        h.leaf(PanelId::Timeline).top() > canvas.bottom(),
        "{}",
        why()
    );
    let got = w.column_rect().expect("a column");
    assert_eq!(got.width(), column.width(), "{}", why());
}

#[test]
fn the_floating_color_tab_dropped_over_its_own_window_counts_what_is_under_it() {
    // Como la captura del usuario (960×505): Color flotando pegado a la orilla derecha, tapando
    // la tira y el cuarto derecho del lienzo. Mientras se arrastra su única pestaña, su ventana
    // se ve al 40 % y no es destino: con el puntero encima de ella, cuenta lo de debajo (y se
    // pinta su zona). Antes, la tira ya funcionaba (tarea 1); el cuarto derecho del lienzo la
    // volvía a flotar.
    let size = vec2(960.0, 505.0);
    let reference = essentials(size);
    let column = reference.app.workspace.column_rect().unwrap();
    let probe = color_floating(size, Start::Right);
    let window = probe.window(PanelId::Color);
    let canvas = probe.leaf(PanelId::Canvas);
    let tab = probe.button("Color").center();
    assert_eq!(
        window_opacity(&probe, PanelId::Color),
        1.0,
        "opaque at rest"
    );
    // (i) Sobre la tira: Essentials.
    let to = probe.panel("dock").center();
    assert!(window.contains(to), "{window:?} covers {to:?}");
    let mut h = color_floating(size, Start::Right);
    hold_drag(&mut h, tab, to);
    let opacity = window_opacity(&h, PanelId::Color);
    assert!(see_through(opacity), "strip: {opacity}");
    let zones = drop_zones(&h);
    assert!(
        zones.len() == 1 && near(zones[0], column),
        "strip: {zones:?} vs {column:?}"
    );
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert_like(&h, &reference, "strip under its own window");
    // (i') En la franja de la orilla, bajo su propia barra de pestañas (la barra de un grupo le
    // gana a la franja, pero no la de la ventana que no es destino): Essentials.
    let strip = probe.panel("dock");
    let to = pos2(strip.left() - EDGE_BAND / 2.0, tab.y);
    assert!(window.contains(to), "{window:?} covers {to:?}");
    let mut h = color_floating(size, Start::Right);
    hold_drag(&mut h, tab, to);
    let zones = drop_zones(&h);
    assert!(
        zones.len() == 1 && near(zones[0], column),
        "band: {zones:?} vs {column:?}"
    );
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert_like(&h, &reference, "band under its own tab bar");
    // (ii) Sobre el cuarto derecho del lienzo: partido a la derecha del lienzo (como sin la
    // ventana encima), con la zona de egui_dock de esa mitad.
    let to = pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y);
    assert!(window.contains(to), "{window:?} covers {to:?}");
    let mut h = color_floating(size, Start::Right);
    hold_drag(&mut h, tab, to);
    let opacity = window_opacity(&h, PanelId::Color);
    assert!(see_through(opacity), "canvas: {opacity}");
    let half = canvas.with_min_x(canvas.center().x);
    let zones = drop_zones(&h);
    assert!(
        zones.len() == 1 && near(zones[0], half),
        "canvas: {zones:?} vs {half:?}"
    );
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert_split_right_of_the_canvas(&h, column, "canvas under its own window");
    assert_eq!(drop_zones(&h), [], "nothing painted after");
}

#[test]
fn the_floating_color_tab_dropped_over_its_own_window_with_nothing_under_it_floats_there() {
    // Arrastrada desde la columna, la ventana llega hasta abajo de la pantalla y tapa la barra de
    // estado: soltada ahí (fuera del árbol y de las demás ventanas) va donde se soltó, como
    // fuera de todo. Antes su propia ventana contaba y no pasaba nada.
    let size = vec2(960.0, 505.0);
    let mut h = color_floating(size, Start::Dragged);
    let window = h.window(PanelId::Color);
    let tab = h.button("Color").center();
    let to = pos2(window.center().x + 20.0, h.screen().bottom() - 8.0);
    assert!(window.contains(to), "{window:?} covers {to:?}");
    assert!(to.y > h.leaf(PanelId::Timeline).bottom(), "below the tree");
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.workspace.floating(PanelId::Color), "{}", describe(&h));
    let after = h.window(PanelId::Color);
    assert!(
        (after.left() - to.x.round()).abs() <= 1.0,
        "{after:?} vs {to:?} (was {window:?})"
    );
}

#[test]
fn the_floating_color_tab_dropped_quickly_over_its_own_window_counts_what_is_under_it() {
    // Sin quedarse quieto: egui_dock recuerda el grupo por el que pasó el puntero
    // `max_preference_time` (0.3 s), y al empezar a arrastrar ese grupo es el de su propia
    // ventana. Encima de ella, lo de debajo cuenta en cuanto llega.
    let size = vec2(960.0, 505.0);
    let reference = essentials(size);
    let column = reference.app.workspace.column_rect().unwrap();
    let mut h = color_floating(size, Start::Right);
    let canvas = h.leaf(PanelId::Canvas);
    let from = h.button("Color").center();
    let to = pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y);
    assert!(h.window(PanelId::Color).contains(to));
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from, true)]);
    for t in [0.1, 0.4, 0.7, 1.0] {
        h.frame_with(vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    h.frame_with(vec![press(to, false)]);
    for _ in 0..3 {
        h.frame();
    }
    assert_split_right_of_the_canvas(&h, column, "quick release");
}

#[test]
fn the_dragged_window_is_opaque_again_when_the_drag_is_cancelled() {
    // Escape corta el arrastre: la ventana vuelve a verse opaca (y no se pinta ninguna zona), y
    // al soltar no pasa nada.
    let size = vec2(960.0, 505.0);
    let mut h = color_floating(size, Start::Right);
    let window = h.window(PanelId::Color);
    let canvas = h.leaf(PanelId::Canvas);
    let tab = h.button("Color").center();
    let to = pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y);
    hold_drag(&mut h, tab, to);
    assert!(see_through(window_opacity(&h, PanelId::Color)));
    tap_key(&mut h, egui::Key::Escape);
    assert_eq!(window_opacity(&h, PanelId::Color), 1.0, "cancelled");
    assert_eq!(drop_zones(&h), [], "no zone once cancelled");
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.workspace.floating(PanelId::Color), "{}", describe(&h));
    assert_eq!(h.window(PanelId::Color), window);
    assert_eq!(window_opacity(&h, PanelId::Color), 1.0, "after");
}

#[test]
fn a_window_with_two_tabs_is_not_faded_and_takes_what_is_dropped_on_it() {
    // Library junto a Color en una ventana: arrastrar una de las dos no atenúa la ventana, que
    // sigue siendo destino (soltada en su cuarto de abajo, la parte en dos).
    let mut h = essentials(vec2(1200.0, 800.0));
    h.app.workspace.float(PanelId::Color, pos2(500.0, 150.0));
    for _ in 0..3 {
        h.frame();
    }
    let (s, n) = h.app.workspace.leaf_of(PanelId::Color).unwrap();
    h.app
        .workspace
        .dock(PanelId::Library, DockTarget::Leaf(s, n));
    for _ in 0..3 {
        h.frame();
    }
    let leaf = h.leaf(PanelId::Color);
    let tab = h.button("Library").center();
    let to = pos2(leaf.center().x, leaf.bottom() - leaf.height() * 0.1);
    hold_drag(&mut h, tab, to);
    assert_eq!(window_opacity(&h, PanelId::Color), 1.0, "two tabs: opaque");
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    let (color, library) = (
        w.leaf_of(PanelId::Color).unwrap(),
        w.leaf_of(PanelId::Library).unwrap(),
    );
    assert!(
        !color.0.is_main() && color.0 == library.0,
        "{color:?} {library:?}"
    );
    assert_ne!(color.1, library.1, "split in its window");
    assert!(h.leaf(PanelId::Library).top() > h.leaf(PanelId::Color).top());
}

#[test]
fn the_dragged_window_is_translucent_without_pixel_ui_too() {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), plain(), palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, vec2(1200.0, 800.0));
    let canvas = h.leaf(PanelId::Canvas);
    let at = pos2(canvas.right() - 230.0, canvas.top() + 10.0);
    h.app.workspace.float(PanelId::Color, at);
    for _ in 0..5 {
        h.frame();
    }
    assert_eq!(window_opacity(&h, PanelId::Color), 1.0, "opaque at rest");
    let tab = h.button("Color").center();
    let to = pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y);
    assert!(h.window(PanelId::Color).contains(to));
    hold_drag(&mut h, tab, to);
    let opacity = window_opacity(&h, PanelId::Color);
    assert!(see_through(opacity), "{opacity}");
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.workspace.docked(PanelId::Color), "{}", describe(&h));
    assert!(h.leaf(PanelId::Color).left() > h.leaf(PanelId::Canvas).right());
}

// --- Tarea 3: lo que dejó la revisión de la tarea 2 ---

#[test]
fn a_long_press_or_a_short_slide_on_a_floating_lone_tab_off_the_tree_leaves_its_window() {
    // egui da el arrastre por decidido (apretado más de 0.8 s, o 6 a 30 pt a lo ancho) sin que
    // egui_dock haya empezado el suyo: soltar fuera del árbol (aquí, sobre la barra de menús) no
    // es soltar en nada, y la ventana se queda donde estaba (no salta con su esquina al puntero).
    let size = vec2(960.0, 505.0);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    for slide in [0.0, 20.0] {
        let mut h = essentials(size);
        h.app.workspace.float(PanelId::Color, pos2(300.0, 0.0));
        for _ in 0..5 {
            h.frame();
        }
        let window = h.window(PanelId::Color);
        let tab = h.button("Color").center();
        assert!(
            tab.y < h.leaf(PanelId::Tools).top(),
            "the tab is off the tree"
        );
        h.frame_with(vec![Event::PointerMoved(tab)]);
        h.frame_with(vec![press(tab, true)]);
        let to = tab + vec2(slide, 0.0);
        for t in [0.25, 0.5, 0.75, 1.0] {
            h.frame_with(vec![Event::PointerMoved(tab + (to - tab) * t)]);
        }
        // Más de 0.8 s quieto (cada frame del arnés son 0.05 s).
        for _ in 0..20 {
            h.frame();
        }
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        assert!(h.app.workspace.floating(PanelId::Color), "slide {slide}");
        assert_eq!(h.window(PanelId::Color), window, "slide {slide}");
    }
}

#[test]
fn a_drag_brought_back_within_the_threshold_and_released_off_the_tree_leaves_its_window() {
    // egui_dock vuelve a mirar su umbral (30/6) cada frame: arrastrada 40 pt y de vuelta a 3 pt
    // de donde se apretó, para egui_dock ya no hay arrastre y no suelta nada. El workspace
    // tampoco: soltar fuera del árbol (sobre las barras de arriba: una ventana no baja hasta la
    // barra de estado) no pone la ventana bajo el puntero.
    let size = vec2(960.0, 505.0);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let mut h = essentials(size);
    h.app.workspace.float(PanelId::Color, pos2(300.0, 0.0));
    for _ in 0..5 {
        h.frame();
    }
    let window = h.window(PanelId::Color);
    let tab = h.button("Color").center();
    assert!(
        tab.y < h.leaf(PanelId::Tools).top(),
        "the tab is off the tree"
    );
    h.frame_with(vec![Event::PointerMoved(tab)]);
    h.frame_with(vec![press(tab, true)]);
    // egui lo da por arrastre a los 10 pt (pasada la distancia de un clic): de ahí cuenta
    // egui_dock, y a los 50 ya va 40 pasado.
    for k in 1..=5 {
        h.frame_with(vec![Event::PointerMoved(tab + vec2(10.0 * k as f32, 0.0))]);
    }
    h.frame();
    // Empezó (la ventana se ve translúcida).
    let opacity = window_opacity(&h, PanelId::Color);
    assert!(see_through(opacity), "{opacity}");
    let back = tab + vec2(3.0, 0.0);
    h.frame_with(vec![Event::PointerMoved(back)]);
    h.frame();
    release_at(&mut h, back);
    for _ in 0..3 {
        h.frame();
    }
    assert!(h.app.workspace.floating(PanelId::Color), "{}", describe(&h));
    assert_eq!(h.window(PanelId::Color), window);
}

#[test]
fn starting_to_drag_a_floating_lone_tab_paints_no_zone_on_its_own_window() {
    // El primer frame del arrastre de egui_dock, su destino es todavía la propia ventana (antes
    // de que haya arrastre no pregunta `allowed_in_windows`): su zona no se pinta (la ventana no
    // es destino). Se arrastra a lo largo de su barra, de a 4 pt.
    let size = vec2(1200.0, 800.0);
    let mut h = color_floating(size, Start::Middle);
    let window = h.window(PanelId::Color);
    let from = h.button("Color").center();
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers: Default::default(),
    }]);
    for k in 1..=16 {
        let at = from + vec2(4.0 * k as f32, 0.0);
        h.frame_with(vec![Event::PointerMoved(at)]);
        let own: Vec<Rect> = drop_zones(&h)
            .into_iter()
            .filter(|z| window.expand(1.0).contains_rect(*z))
            .collect();
        assert_eq!(own, [], "frame {k}: a zone on its own window {window:?}");
    }
}

#[test]
fn a_two_tab_window_over_the_right_strip_takes_its_own_tab_dropped_on_it() {
    // Con dos pestañas, la ventana sí es destino (regla B): soltar una de ellas sobre la propia
    // ventana, encima de la tira, la deja en la ventana (no la manda a la orilla de la tira).
    let size = vec2(960.0, 505.0);
    let mut h = color_floating(size, Start::Right);
    let (s, n) = h.app.workspace.leaf_of(PanelId::Color).unwrap();
    h.app
        .workspace
        .dock(PanelId::Library, DockTarget::Leaf(s, n));
    for _ in 0..3 {
        h.frame();
    }
    let window = h.window(PanelId::Color);
    let strip = h.panel("dock");
    let to = pos2(strip.center().x, window.center().y);
    assert!(
        window.contains(to) && strip.contains(to),
        "{window:?} {strip:?}"
    );
    let tab = h.button("Color").center();
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let w = &h.app.workspace;
    let (color, library) = (
        w.leaf_of(PanelId::Color).unwrap(),
        w.leaf_of(PanelId::Library).unwrap(),
    );
    assert!(w.floating(PanelId::Color), "{}", describe(&h));
    assert_eq!(color.0, library.0, "{}", describe(&h));
}

// --- Tarea 3: "Dock" en la pestaña de una ventana flotante ---

/// Clic derecho en `at` (la pestaña de una ventana flotante, o su franja de agarre) y, en su
/// menú, "Dock".
fn dock_from_menu(h: &mut LayoutHarness, at: Pos2) {
    h.click(at, PointerButton::Secondary);
    let dock = h.button(crate::dock::MENU_DOCK).center();
    h.click(dock, PointerButton::Primary);
    for _ in 0..3 {
        h.frame();
    }
}

/// Si el último frame de `h` tiene un botón "Dock" (el del menú abierto).
fn dock_offered(h: &LayoutHarness) -> bool {
    h.buttons.iter().any(|(l, _)| l == crate::dock::MENU_DOCK)
}

#[test]
fn dock_on_the_tab_of_floating_color_gives_back_essentials() {
    // El menú de la pestaña "Color" de su ventana: "Dock" la regresa a la columna derecha de
    // Essentials (la orilla derecha, del ancho de la columna), esté donde esté la ventana.
    for size in [vec2(1200.0, 800.0), vec2(960.0, 505.0)] {
        let reference = essentials(size);
        for start in [Start::Middle, Start::Dragged, Start::Right] {
            let mut h = color_floating(size, start);
            let tab = h.button("Color").center();
            dock_from_menu(&mut h, tab);
            assert_like(&h, &reference, &format!("{size:?} {start:?}"));
        }
    }
}

#[test]
fn dock_on_a_floating_tab_puts_the_toolbar_the_timeline_or_a_dock_panel_back_in_place() {
    // La toolbar (sola en su ventana, con Pixel UI, sin barra: el menú es de su franja de
    // agarre), el timeline y Library: cada uno a su lugar de Essentials (la toolbar a la
    // izquierda del lienzo, el timeline debajo, Library a la tira derecha en su lugar).
    let size = vec2(1200.0, 800.0);
    let reference = essentials(size);
    for id in [PanelId::Tools, PanelId::Timeline, PanelId::Library] {
        let mut h = essentials(size);
        h.app.workspace.float(id, pos2(400.0, 150.0));
        for _ in 0..5 {
            h.frame();
        }
        assert!(h.app.workspace.floating(id), "{id:?}");
        let at = if id == PanelId::Tools {
            assert!(h.app.workspace.bare_tools().is_some());
            let leaf = h.leaf(id);
            pos2(leaf.center().x, leaf.top() + 1.0)
        } else {
            // Su pestaña, la de su ventana (la tira también tiene un botón "Timeline").
            let window = h.window(id);
            h.buttons
                .iter()
                .find(|(l, r)| l == id.spec().title && window.contains_rect(*r))
                .map(|(_, r)| r.center())
                .unwrap_or_else(|| panic!("{id:?}'s tab in {window:?}"))
        };
        dock_from_menu(&mut h, at);
        let case = format!("{id:?}");
        assert_like(&h, &reference, &case);
        assert_eq!(
            h.app.workspace.strip_at(Edge::Right),
            reference.app.workspace.strip_at(Edge::Right),
            "{case}"
        );
    }
}

#[test]
fn the_preview_tab_offers_no_dock() {
    // El Preview flota por diseño: su pestaña (aquí junto a Library en su ventana) no lleva
    // "Dock"; la de Library, sí.
    let mut h = essentials(vec2(1200.0, 800.0));
    h.app.view.preview = true;
    for _ in 0..3 {
        h.frame();
    }
    let (s, n) = h.app.workspace.leaf_of(PanelId::Preview).unwrap();
    assert!(!s.is_main());
    h.app
        .workspace
        .dock(PanelId::Library, DockTarget::Leaf(s, n));
    for _ in 0..3 {
        h.frame();
    }
    h.click(h.button("Preview").center(), PointerButton::Secondary);
    assert!(!dock_offered(&h), "{:?}", h.buttons);
    h.click(h.button("Library").center(), PointerButton::Secondary);
    assert!(dock_offered(&h));
}

// --- Tarea 3: el crash en debug al soltar a la derecha de la toolbar ---

#[test]
fn a_tab_dropped_on_the_right_of_the_toolbar_docks_beside_it() {
    // (d) de la investigación: la pestaña de Color soltada en el borde derecho de la toolbar.
    // egui_dock parte su hoja (35 pt) por la mitad y dibuja ese frame así, antes de que el
    // workspace le dé a cada lado su mínimo: Color se acomodaba en 16.5 pt (0 sin sus márgenes) y
    // en debug tronaba (egui `placer.rs`, "Negative child size": la caja de la paleta, de 0 de
    // ancho, sin sus bordes). Ahora se acomoda en su mínimo, y queda entre la toolbar y el lienzo.
    for size in [vec2(960.0, 505.0), vec2(1200.0, 800.0)] {
        let mut h = color_floating(size, Start::Middle);
        let tools = h.leaf(PanelId::Tools);
        let tab = h.button("Color").center();
        let to = pos2(tools.right() - 2.0, h.leaf(PanelId::Canvas).center().y);
        hold_drag(&mut h, tab, to);
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        let case = format!("{size:?}:\n{}", describe(&h));
        assert!(h.app.workspace.docked(PanelId::Color), "{case}");
        let (tools, color) = (h.leaf(PanelId::Tools), h.leaf(PanelId::Color));
        let canvas = h.leaf(PanelId::Canvas);
        assert!(tools.right() < color.left(), "{case}");
        assert!(color.right() < canvas.left(), "{case}");
        // Su mínimo (no la mitad de la toolbar), el que cuenta el workspace.
        let skin = h.app.pixel.skin();
        let mut min = Vec2::ZERO;
        let mut output = h.ctx.run_ui(RawInput::default(), |ui| {
            min = crate::workspace::panel_minimum(ui, skin, PanelId::Color);
        });
        output.textures_delta.clear();
        assert!(min.x > 100.0, "{min:?}");
        assert!(color.width() >= min.x, "{min:?} {case}");
    }
}
