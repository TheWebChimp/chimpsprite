//! Barrido de anchos (fase D, pendiente de la fase C): cada panel acoplado, dibujado como lo
//! hospeda el workspace (`ChimpApp::draw_panel` en un `Ui` del ancho de su cuerpo, con Pixel UI y
//! la escala de la app), a cada ancho de su mínimo a 600, de a un punto: sin medios pixeles, nada
//! fuera de su ancho y nada encimado (lo de `forms::testing`). El mismo contexto va de un ancho
//! al siguiente, como al arrastrar el divisor de su grupo, y cada ancho se mide en su segundo
//! frame (taffy mide con lo del frame anterior).
//!
//! Lo que un panel no cumple en un tramo y pide rediseñarlo va en [`KNOWN_GAPS`] (con el panel, el
//! tramo y la clase de falla que calla; si deja de pasar en todo su tramo, el test falla: se
//! borra), no aflojando lo que se revisa. Lo que va así a propósito, a cualquier ancho, va en
//! [`problems`] con su porqué y nombrado (los grupos de controles pegados de [`GLUED`] y los
//! pares de roles de [`nested`]; los mismos casos que ya separan los tests de cada panel, como
//! `without_glued` del panel 3D).

use eframe::egui;
use eframe::egui::accesskit::Role;

use crate::ChimpApp;
use crate::app_panels::PanelCtx;
use crate::forms::testing::Probe;
use crate::palette_library::PaletteLibrary;
use crate::palette_library::test_util::NoPresets;
use crate::panel_registry::{Body, PanelId};
use crate::view::ViewSettings;

/// El ancho más grande del barrido.
const WIDEST: f32 = 600.0;

/// Lo que no cumple un control.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    /// Empieza o termina a medio punto.
    HalfPoint,
    /// Se sale del ancho del panel.
    Outside,
    /// Tapa a otro.
    Overlap,
}

/// Un tramo conocido que no cumple: el panel, de qué ancho a cuál, la clase de falla que calla
/// (solo esa) y por qué.
struct Gap {
    panel: PanelId,
    from: f32,
    to: f32,
    kind: Kind,
    why: &'static str,
}

const KNOWN_GAPS: &[Gap] = &[
    // La fila de botones de arriba del timeline (los de reproducir, los de frames y los de capas)
    // no se parte: más angosto se sale. Pide rediseñarla (que se parta o que vaya a un menú);
    // de fábrica el timeline va a lo ancho, debajo del lienzo.
    Gap {
        panel: PanelId::Timeline,
        from: 64.0,
        to: 126.0,
        kind: Kind::Outside,
        why: "its top row of buttons does not wrap",
    },
    // Lospec no se migró a `forms` (filas de `ui.horizontal` con textos de alturas distintas): las
    // filas de cada paleta de la lista (su título y "CREATED BY …", sus Apply, Append y Save) caen
    // a medio punto a lo alto, a cualquier ancho. Pide migrarlo. (Antes el barrido dependía de
    // cuándo respondía la red y solo lo veía desde 568; ahora la lista ya llegó: ver `app_for`.)
    Gap {
        panel: PanelId::Lospec,
        from: crate::palette_ui::LOSPEC_PANEL_W,
        to: WIDEST,
        kind: Kind::HalfPoint,
        why: "not migrated to forms: its result rows fall on half points",
    },
];

/// Los controles del kit que van pegados a propósito, por panel: cada uno comparte la columna (o
/// la fila) de su borde con el de al lado, así que se tocan en un punto (una intersección de 1
/// punto de ancho o de alto), como en el mockup. Solo cuenta entre controles del mismo grupo.
/// `"#n"`: los cuadros de la paleta (`#0`, `#1`...).
const GLUED: &[(PanelId, &[&str])] = &[
    // Los botones de arriba del timeline, en tres filas de botones de borde compartido.
    (
        PanelId::Timeline,
        &[
            "First frame",
            "Previous frame",
            "Play",
            "Next frame",
            "Last frame",
        ],
    ),
    (PanelId::Timeline, &["New frame", "Delete frame"]),
    (
        PanelId::Timeline,
        &[
            "New layer",
            "Delete layer",
            "New group",
            "New tilemap layer",
        ],
    ),
    // La fila de la cabecera de la paleta y la rejilla de sus cuadros (con el + del final).
    (
        PanelId::Color,
        &["Lock palette", "Palette menu", "New palette"],
    ),
    (PanelId::Color, &["#n", "Add to palette"]),
    // El campo "Add char" de FontForge y su Add.
    (PanelId::Font, &["Add char", "Add"]),
    // El select de la textura del 3D con su New y los presets de la cámara (como
    // `model_panel::tests::without_glued`).
    (PanelId::Model, &["Ship Texture", "New"]),
    (PanelId::Model, &["Iso", "Top", "Side", "Front"]),
];

/// Si `name` es del grupo `group` (ver [`GLUED`]).
fn in_group(group: &[&str], name: &str) -> bool {
    group.iter().any(|&g| match g {
        "#n" => name
            .strip_prefix('#')
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        _ => g == name,
    })
}

/// Si `a` y `b` de `panel` van pegados a propósito: del mismo grupo de [`GLUED`] y tocándose en
/// un punto (`inter`, su intersección, de 1 punto o menos de ancho o de alto).
fn glued(panel: PanelId, a: &str, b: &str, inter: egui::Rect) -> bool {
    (inter.width() <= 1.0 || inter.height() <= 1.0)
        && GLUED
            .iter()
            .any(|(p, g)| *p == panel && in_group(g, a) && in_group(g, b))
}

/// Si uno va adentro del otro a propósito (`outer` con `inner`, de roles dados; nunca del mismo
/// rect):
/// - Una fila que es un botón de lado a lado con los suyos adentro (la del outliner del 3D, con
///   su ojo y su candado): un `Button` dentro de otro.
/// - Un campo (`SpinButton` o `TextInput`) con el cuadro de su texto (otro de esos, más bajo que
///   el campo; ver `color_panel::text_rect`): adentro a lo ancho, y a lo alto puede asomar medio
///   punto arriba (va donde caen las mayúsculas de su texto).
fn nested((outer, ro): (egui::Rect, Role), (inner, ri): (egui::Rect, Role)) -> bool {
    if outer == inner {
        return false;
    }
    let field = |r: Role| matches!(r, Role::SpinButton | Role::TextInput);
    match (ro, ri) {
        (Role::Button, Role::Button) => outer.contains_rect(inner),
        (a, b) if field(a) && field(b) => {
            let poke = egui::Rect::from_min_max(outer.min - egui::vec2(0.0, 0.5), outer.max);
            inner.height() < crate::kit::input::HEIGHT && poke.contains_rect(inner)
        }
        _ => false,
    }
}

/// Lo que no cumple de `probe` (el panel `panel`) a `width` (sin medios pixeles, nada fuera,
/// nada encimado), con su clase, menos lo que va así a propósito:
/// - Los controles pegados del kit de [`GLUED`].
/// - Uno dentro del otro, de [`nested`].
/// - El botón de cambiar los colores de la toolbar va encima de la esquina de los dos (como en el
///   mockup).
/// - El cuadro del texto de un campo (el `DragValue` o el `TextEdit` sin marco de adentro, más bajo
///   que el campo) va donde caen las mayúsculas de su texto en la fila del campo (ver
///   `color_panel`, `text_rect`): su alto es el de la fuente y puede empezar a medio punto. Su x sí
///   cuenta (y el campo, entero).
fn problems(panel: PanelId, probe: &Probe, width: f32) -> Vec<(Kind, String)> {
    let mut out = Vec::new();
    let text_box = |role: Role, r: &egui::Rect| {
        matches!(role, Role::SpinButton | Role::TextInput) && r.height() < crate::kit::input::HEIGHT
    };
    for (name, r, role) in &probe.widgets {
        let xs = [r.min.x, r.max.x];
        let ys = [r.min.y, r.max.y];
        let half = |v: &f32| *v != v.round();
        if xs.iter().any(half) || (!text_box(*role, r) && ys.iter().any(half)) {
            out.push((Kind::HalfPoint, format!("half pixel: {name:?} at {r:?}")));
        }
        if r.min.x < 0.0 || r.max.x > width {
            out.push((Kind::Outside, format!("outside: {name:?} at {r:?}")));
        }
    }
    let swap = |a: &str, b: &str| {
        panel == PanelId::Tools
            && a == "Swap colors"
            && ["Primary color", "Secondary color"].contains(&b)
    };
    for (i, (a, ra, roa)) in probe.widgets.iter().enumerate() {
        for (b, rb, rob) in &probe.widgets[i + 1..] {
            let inter = ra.intersect(*rb);
            if !inter.is_positive()
                || glued(panel, a, b, inter)
                || nested((*ra, *roa), (*rb, *rob))
                || nested((*rb, *rob), (*ra, *roa))
                || swap(a, b)
                || swap(b, a)
            {
                continue;
            }
            out.push((
                Kind::Overlap,
                format!("overlap: {a:?} {ra:?} and {b:?} {rb:?}"),
            ));
        }
    }
    out
}

/// La app con Pixel UI, una pestaña nueva y lo que necesita `id` para mostrar su forma entera: una
/// fuente para FontForge y un modelo para el panel 3D.
fn app_for(id: PanelId) -> ChimpApp {
    let view = ViewSettings {
        pixel_ui: true,
        ..Default::default()
    };
    let palettes = PaletteLibrary::new(Box::new(NoPresets));
    let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
    app.new_document();
    let editor = &mut app.doc.editor;
    match id {
        PanelId::Font => {
            editor.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
        }
        PanelId::Model => {
            let skin = cs_core::Sprite::new(16, 16, cs_core::ColorMode::Rgba);
            assert!(editor.add_artboard_with_sprite("Skin".into(), skin));
            assert!(editor.new_model("Ship", (32, 32)));
        }
        // Las listas de Lospec y de dafont, ya llegadas y sin red (si no, llegan cuando la red
        // quiere, a la mitad del barrido).
        PanelId::Lospec => app.open_lospec_offline(lospec_page()),
        #[cfg(not(target_arch = "wasm32"))]
        PanelId::Dafont => app.dafont = crate::dafont::DafontPanel::offline(),
        _ => {}
    }
    app
}

/// Una página de Lospec como las de verdad: paletas de varios tamaños, con autor, likes y
/// descargas (sin imágenes de ejemplo, que se pedirían a la red).
fn lospec_page() -> cs_io::lospec::LospecPage {
    let palette = |k: usize, colors: usize| cs_io::lospec::LospecPalette {
        slug: format!("palette-{k}"),
        title: format!("Palette Number {k}"),
        author: Some(format!("Somebody {k}")),
        palette: cs_core::Palette::new(
            (0..colors)
                .map(|c| cs_core::Rgba::from_hex(0x10_20_30 + (c as u32) * 0x05_07_09))
                .collect(),
        ),
        likes: Some(12 * k as u32),
        downloads: Some(345 * k as u32),
        examples: Vec::new(),
        source: None,
    };
    let palettes: Vec<_> = [4, 8, 16, 32, 64, 5]
        .iter()
        .enumerate()
        .map(|(k, &n)| palette(k + 1, n))
        .collect();
    cs_io::lospec::LospecPage {
        raw_count: palettes.len(),
        total: 120,
        palettes,
    }
}

/// El panel `id` como lo hospeda el workspace acoplado: con todo el alto (los de cuerpo fijo) y
/// hasta el fondo de su grupo (la lista de Lospec).
fn draw(app: &mut ChimpApp, ui: &mut egui::Ui, id: PanelId) {
    app.pixel.update(ui.ctx(), &app.view);
    app.begin_panel_frame();
    let panel = PanelCtx {
        modal: false,
        bottom: Some(ui.max_rect().bottom()),
        window: None,
    };
    if matches!(id.spec().body, Body::Fixed) {
        ui.set_min_height(ui.available_height());
    }
    let _ = app.draw_panel(ui, id, &panel);
}

/// El ancho más chico de `id` acoplado: el de su ficha (Color: el de su contenido con Pixel UI,
/// el que pide su grupo sin su margen).
fn narrowest(ctx: &egui::Context, app: &ChimpApp, id: PanelId) -> f32 {
    if id == PanelId::Color
        && let Some(skin) = app.pixel.skin()
    {
        let mut width = id.spec().min_width;
        let _ = ctx.run_ui(Default::default(), |ui| {
            width = crate::color_panel::min_width(ui, skin);
        });
        return width;
    }
    id.spec().min_width
}

#[test]
fn the_allowances_name_their_controls() {
    let r = |x0: f32, y0: f32, x1: f32, y1: f32| {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    };
    // Pegados: solo en su grupo y su panel, y solo tocándose en un punto.
    let line = r(24.0, 0.0, 25.0, 17.0);
    assert!(glued(PanelId::Timeline, "Play", "Next frame", line));
    assert!(glued(PanelId::Color, "#3", "#12", line));
    assert!(glued(PanelId::Color, "#3", "Add to palette", line));
    assert!(!glued(PanelId::Color, "#", "Add to palette", line));
    assert!(!glued(PanelId::Timeline, "Play", "New frame", line));
    assert!(!glued(PanelId::Model, "Play", "Next frame", line));
    assert!(!glued(
        PanelId::Timeline,
        "Play",
        "Next frame",
        r(20.0, 0.0, 25.0, 17.0)
    ));
    // Uno dentro de otro: solo de sus roles, y nunca del mismo rect.
    let row = r(0.0, 0.0, 186.0, 13.0);
    let eye = r(164.0, 0.0, 175.0, 13.0);
    assert!(nested((row, Role::Button), (eye, Role::Button)));
    assert!(!nested((row, Role::Button), (row, Role::Button)));
    assert!(!nested((row, Role::Label), (eye, Role::Button)));
    assert!(!nested((eye, Role::Button), (row, Role::Button)));
    let field = r(33.0, 240.0, 60.0, 256.0);
    let text = r(42.0, 239.5, 59.0, 253.5);
    assert!(nested((field, Role::SpinButton), (text, Role::SpinButton)));
    assert!(!nested((field, Role::SpinButton), (text, Role::Button)));
    assert!(!nested(
        (field, Role::SpinButton),
        (text.translate(egui::vec2(1.5, 0.0)), Role::SpinButton)
    ));
    assert!(!nested(
        (field, Role::SpinButton),
        (text.translate(egui::vec2(0.0, -0.5)), Role::SpinButton)
    ));
}

#[test]
fn every_docked_panel_reflows_from_its_minimum_to_600() {
    let started = std::time::Instant::now();
    let mut failures: Vec<String> = Vec::new();
    let mut drawn = 0usize;
    // Si cada tramo conocido todavía pasa en algún ancho de su tramo.
    let mut seen = vec![false; KNOWN_GAPS.len()];
    for &id in PanelId::ALL {
        if id == PanelId::Canvas {
            continue;
        }
        let mut app = app_for(id);
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        // Pixel UI (fuentes, escala y skin) rige desde el segundo frame.
        for _ in 0..3 {
            crate::forms::testing::frame_at(&ctx, WIDEST, |ui| draw(&mut app, ui, id));
        }
        let from = narrowest(&ctx, &app, id).ceil();
        let mut bad: Vec<(f32, String)> = Vec::new();
        let mut width = from;
        while width <= WIDEST {
            crate::forms::testing::frame_at(&ctx, width, |ui| draw(&mut app, ui, id));
            let probe = crate::forms::testing::frame_at(&ctx, width, |ui| draw(&mut app, ui, id));
            drawn += 1;
            let mut first = None;
            for (kind, what) in problems(id, &probe, width) {
                let known = KNOWN_GAPS.iter().position(|g| {
                    g.panel == id && g.kind == kind && (g.from..=g.to).contains(&width)
                });
                match known {
                    Some(k) => seen[k] = true,
                    None => {
                        first.get_or_insert(what);
                    }
                }
            }
            if let Some(first) = first {
                bad.push((width, first));
            }
            width += 1.0;
        }
        // Por tramos: el primer ancho de cada tramo que falla, con lo primero que falló.
        let mut start: Option<(f32, f32, String)> = None;
        for (w, what) in bad {
            start = match start {
                Some((a, b, first)) if w == b + 1.0 => Some((a, w, first)),
                Some((a, b, first)) => {
                    failures.push(format!("{id:?} {a}..={b}: {first}"));
                    Some((w, w, what))
                }
                None => Some((w, w, what)),
            };
        }
        if let Some((a, b, first)) = start {
            failures.push(format!("{id:?} {a}..={b}: {first}"));
        }
    }
    // Un tramo conocido que ya no pasa en ningún ancho de su tramo: se arregló (o cambió), se borra.
    for (gap, seen) in KNOWN_GAPS.iter().zip(seen) {
        if !seen {
            failures.push(format!(
                "stale known gap: {:?} {}..={} {:?} ({})",
                gap.panel, gap.from, gap.to, gap.kind, gap.why
            ));
        }
    }
    eprintln!(
        "width sweep: {drawn} widths in {:.1} s",
        started.elapsed().as_secs_f32()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
