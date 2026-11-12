//! Diagnóstico (sin arreglo): Color flotando sobre el lienzo, ¿se puede volver a acoplar
//! arrastrándolo? Cada caso arrastra su pestaña (o su ventana) a un lugar y dice qué quedó.

use super::*;
use crate::panel_registry::PanelId;

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

/// Essentials en una ventana de `size`, con Color flotando sobre el lienzo.
fn color_floating(size: Vec2, start: Start) -> LayoutHarness {
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), pixel_ui(), palettes);
    app.new_document();
    let mut h = LayoutHarness::start_in(app, size);
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
    if w.floating(PanelId::Color) {
        out += &format!(
            "  Color window {:?}\n",
            w.window_rect(&h.ctx, PanelId::Color)
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

/// Arrastra de `from` a `to` (como `dock_drag`, quieto al final), dice qué zona se pintó a
/// mitad y al final, y suelta.
fn try_drop(h: &mut LayoutHarness, from: Pos2, to: Pos2) -> String {
    hold_drag(h, from, to);
    let zones = drop_zones(h);
    let window_before = h.app.workspace.window_rect(&h.ctx, PanelId::Color);
    release_at(h, to);
    for _ in 0..3 {
        h.frame();
    }
    format!(
        "  zones while held {zones:?}\n  window while held {window_before:?}\n{}",
        describe(h)
    )
}

fn run_cases(size: Vec2, start: Start) {
    let h = color_floating(size, start);
    let screen = h.screen();
    let canvas = h.leaf(PanelId::Canvas);
    let timeline = h.leaf(PanelId::Timeline);
    let tools = h.leaf(PanelId::Tools);
    let strip = h.panel("dock");
    let tab = h.button("Color");
    let window = h.window(PanelId::Color);
    eprintln!(
        "=== size {size:?} screen {screen:?} ppp {}\n  strip {strip:?} tab {tab:?} window {window:?}\n{}",
        h.ctx.pixels_per_point(),
        describe(&h)
    );
    let cases: Vec<(&str, Pos2, Pos2)> = vec![
        ("a strip middle", tab.center(), strip.center()),
        (
            "a strip top (over canvas)",
            tab.center(),
            pos2(strip.center().x, canvas.top() + 40.0),
        ),
        (
            "a strip bottom (over timeline)",
            tab.center(),
            pos2(strip.center().x, timeline.center().y),
        ),
        (
            "b canvas right 12%",
            tab.center(),
            pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y),
        ),
        (
            "b canvas right 30%",
            tab.center(),
            pos2(canvas.right() - canvas.width() * 0.30, canvas.center().y),
        ),
        (
            "b canvas right 40% (ring)",
            tab.center(),
            pos2(canvas.center().x + canvas.width() * 0.15, canvas.center().y),
        ),
        (
            "b canvas center",
            tab.center(),
            pos2(canvas.center().x + canvas.width() * 0.05, canvas.center().y),
        ),
        (
            "c between canvas and strip",
            tab.center(),
            pos2(strip.left() - 2.0, canvas.center().y),
        ),
        (
            "c screen right edge",
            tab.center(),
            pos2(screen.right() - 1.0, canvas.center().y),
        ),
        ("d tools center", tab.center(), tools.center()),
        (
            "d tools right edge",
            tab.center(),
            pos2(tools.right() - 2.0, tools.center().y),
        ),
        ("d timeline center", tab.center(), timeline.center()),
        (
            "d timeline right 10%",
            tab.center(),
            pos2(
                timeline.right() - timeline.width() * 0.10,
                timeline.center().y,
            ),
        ),
        (
            "d timeline top edge",
            tab.center(),
            pos2(timeline.center().x, timeline.top() + 3.0),
        ),
        (
            "e empty tab bar -> strip",
            pos2(tab.right() + 12.0, tab.center().y),
            strip.center(),
        ),
        (
            "e empty tab bar -> canvas right 12%",
            pos2(tab.right() + 12.0, tab.center().y),
            pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y),
        ),
    ];
    for (name, from, to) in cases {
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut h = color_floating(size, start);
            try_drop(&mut h, from, to)
        }));
        match run {
            Ok(report) => eprintln!("--- {name}: {from:?} -> {to:?}\n{report}"),
            Err(_) => eprintln!("--- {name}: {from:?} -> {to:?}\n  PANICKED (see above)"),
        }
    }
    // Soltar en la tira sin quedarse quieto (soltar en cuanto llega).
    let mut h = color_floating(size, start);
    let press = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let (from, to) = (tab.center(), strip.center());
    h.frame_with(vec![Event::PointerMoved(from)]);
    h.frame_with(vec![press(from, true)]);
    for t in [0.1, 0.4, 0.7, 1.0] {
        h.frame_with(vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    h.frame_with(vec![press(to, false)]);
    for _ in 0..3 {
        h.frame();
    }
    eprintln!("--- a strip, quick release\n{}", describe(&h));
}

#[test]
#[ignore = "diagnostic: prints what each drop does with Color floating"]
fn diag_color_redock_report_1200x800() {
    run_cases(vec2(1200.0, 800.0), Start::Middle);
}

#[test]
#[ignore = "diagnostic: prints what each drop does with Color floating"]
fn diag_color_redock_report_user_size() {
    run_cases(vec2(960.0, 505.0), Start::Middle);
}

#[test]
#[ignore = "diagnostic: prints what each drop does with Color floating"]
fn diag_color_redock_report_user_size_right() {
    run_cases(vec2(960.0, 505.0), Start::Right);
}

#[test]
#[ignore = "diagnostic: prints what each drop does with Color floating"]
fn diag_color_redock_report_user_size_dragged() {
    run_cases(vec2(960.0, 505.0), Start::Dragged);
}

// --- Lo de hoy (08d04df), fijado: estos tests dicen lo que pasa, no lo que debería pasar. ---

/// Si Color quedó como la columna derecha de Essentials: a la derecha del lienzo y del timeline
/// (de alto completo), con el timeline todavía justo debajo del lienzo.
fn full_height_column(h: &LayoutHarness) -> bool {
    let w = &h.app.workspace;
    let (Some(color), Some(timeline)) =
        (w.leaf_rect(PanelId::Color), w.leaf_rect(PanelId::Timeline))
    else {
        return false;
    };
    w.docked(PanelId::Color) && w.timeline_below() && color.bottom() >= timeline.bottom()
}

#[test]
fn diag_no_drop_of_the_floating_color_tab_restores_the_full_height_column() {
    let size = vec2(960.0, 505.0);
    let probe = color_floating(size, Start::Middle);
    let canvas = probe.leaf(PanelId::Canvas);
    let timeline = probe.leaf(PanelId::Timeline);
    let strip = probe.panel("dock");
    let tab = probe.button("Color").center();
    let targets = [
        strip.center(),
        pos2(strip.left() - 2.0, canvas.center().y),
        pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y),
        pos2(canvas.center().x + canvas.width() * 0.05, canvas.center().y),
        pos2(
            timeline.right() - timeline.width() * 0.10,
            timeline.center().y,
        ),
        pos2(strip.center().x, timeline.center().y),
    ];
    for to in targets {
        let mut h = color_floating(size, Start::Middle);
        hold_drag(&mut h, tab, to);
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        // Se acopla (a un lado del lienzo o del timeline), pero nunca en la columna de alto
        // completo: ningún destino de egui_dock es el split V[Canvas / Timeline].
        assert!(h.app.workspace.docked(PanelId::Color), "{to:?}");
        assert!(!full_height_column(&h), "{to:?}:\n{}", describe(&h));
    }
}

#[test]
fn diag_the_floating_color_window_over_the_strip_hides_every_target_under_it() {
    // Color flotando pegado a la orilla derecha (tapa la tira y casi todo el alto del lienzo a
    // 960×505): su pestaña soltada sobre la tira o el cuarto derecho del lienzo cae en su propio
    // grupo (egui_dock: `deserted_node`, sin splits) y vuelve a flotar, sin zona pintada.
    let size = vec2(960.0, 505.0);
    let probe = color_floating(size, Start::Right);
    let canvas = probe.leaf(PanelId::Canvas);
    let window = probe.window(PanelId::Color);
    let tab = probe.button("Color").center();
    let strip = probe.panel("dock");
    assert!(window.top() < canvas.top() + 20.0 && window.bottom() >= canvas.bottom());
    for to in [
        strip.center(),
        pos2(canvas.right() - canvas.width() * 0.12, canvas.center().y),
    ] {
        assert!(window.contains(to), "{to:?} under {window:?}");
        let mut h = color_floating(size, Start::Right);
        hold_drag(&mut h, tab, to);
        assert_eq!(drop_zones(&h), [], "{to:?}: no zone painted");
        release_at(&mut h, to);
        for _ in 0..3 {
            h.frame();
        }
        assert!(h.app.workspace.floating(PanelId::Color), "{to:?}");
    }
}

#[test]
fn diag_a_canvas_split_made_by_egui_dock_gives_color_half_the_canvas() {
    // Soltado en la tira (el cuarto derecho del lienzo para egui_dock): Color a la derecha del
    // lienzo a la mitad (0.5 de egui_dock), y ese ancho se queda como el "natural" de la columna.
    // En el centro del lienzo (lo parte `tidy_canvas`, después de medir) va del ancho de antes.
    let size = vec2(960.0, 505.0);
    let mut h = color_floating(size, Start::Middle);
    let before = h.app.palette_natural_width;
    let canvas = h.leaf(PanelId::Canvas);
    let (tab, to) = (h.button("Color").center(), h.panel("dock").center());
    hold_drag(&mut h, tab, to);
    release_at(&mut h, to);
    for _ in 0..3 {
        h.frame();
    }
    let column = h.app.workspace.column_rect().expect("a column");
    assert!(
        (column.width() - (canvas.width() / 2.0)).abs() < 3.0,
        "{column:?} vs canvas {canvas:?}"
    );
    assert_ne!(h.app.palette_natural_width, before);
    let mut c = color_floating(size, Start::Middle);
    let (tab, to) = (
        c.button("Color").center(),
        pos2(canvas.center().x + canvas.width() * 0.05, canvas.center().y),
    );
    hold_drag(&mut c, tab, to);
    release_at(&mut c, to);
    for _ in 0..3 {
        c.frame();
    }
    let column = c.app.workspace.column_rect().expect("a column");
    assert!(column.width() < canvas.width() / 3.0, "{column:?}");
}
