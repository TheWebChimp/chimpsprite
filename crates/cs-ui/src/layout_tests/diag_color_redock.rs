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
