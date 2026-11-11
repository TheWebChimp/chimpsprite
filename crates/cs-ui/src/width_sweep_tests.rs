//! Barrido de anchos (fase D, pendiente de la fase C): cada panel acoplado, dibujado como lo
//! hospeda el workspace (`ChimpApp::draw_panel` en un `Ui` del ancho de su cuerpo, con Pixel UI y
//! la escala de la app), a cada ancho de su mínimo a 600, de a un punto: sin medios pixeles, nada
//! fuera de su ancho y nada encimado (lo de `forms::testing`). El mismo contexto va de un ancho
//! al siguiente, como al arrastrar el divisor de su grupo, y cada ancho se mide en su segundo
//! frame (taffy mide con lo del frame anterior).
//!
//! Lo que un panel no cumple en un tramo y pide rediseñarlo va en [`KNOWN_GAPS`] (con el panel y
//! el tramo), no aflojando lo que se revisa. Lo que va así a propósito, a cualquier ancho, va en
//! [`problems`] con su porqué (los mismos casos que ya separan los tests de cada panel, como
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

/// Tramos conocidos que no cumplen (panel, de, a, por qué).
const KNOWN_GAPS: &[(PanelId, f32, f32, &str)] = &[
    // La fila de botones de arriba del timeline (los de reproducir, los de frames y los de capas)
    // no se parte: más angosto se sale. Pide rediseñarla (que se parta o que vaya a un menú);
    // de fábrica el timeline va a lo ancho, debajo del lienzo.
    (
        PanelId::Timeline,
        64.0,
        126.0,
        "its top row of buttons does not wrap",
    ),
    // Lospec no se migró a `forms` (es de ancho fijo, 364): desde 568, las filas de cada paleta
    // (su "CREATED BY …" y sus Apply, Append y Save) caen a medio punto. Pide migrarlo.
    (
        PanelId::Lospec,
        568.0,
        WIDEST,
        "not migrated to forms: half points from 568",
    ),
];

/// Lo que no cumple de `probe` a `width` (sin medios pixeles, nada fuera, nada encimado), menos
/// lo que va así a propósito:
/// - Los controles pegados del kit comparten la columna (o la fila) de su borde con el de al lado
///   (los botones del timeline, los presets de la cámara, los cuadros de la paleta, el campo "Add
///   char" con su Add): se tocan en un punto.
/// - Una fila que es un botón de lado a lado con los suyos adentro (la del outliner del 3D, con su
///   ojo y su candado) o un campo con el cuadro de su texto: uno contiene al otro.
/// - El botón de cambiar los colores de la toolbar va encima de la esquina de los dos (como en el
///   mockup).
/// - El cuadro del texto de un campo (el `DragValue` o el `TextEdit` sin marco de adentro, más bajo
///   que el campo) va donde caen las mayúsculas de su texto en la fila del campo (ver
///   `color_panel`, `text_rect`): su alto es el de la fuente y puede empezar a medio punto. Su x sí
///   cuenta (y el campo, entero).
fn problems(probe: &Probe, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    let text_box = |i: usize| {
        matches!(probe.roles[i], Role::SpinButton | Role::TextInput)
            && probe.widgets[i].1.height() < crate::kit::input::HEIGHT
    };
    for (i, (name, r)) in probe.widgets.iter().enumerate() {
        let xs = [r.min.x, r.max.x];
        let ys = [r.min.y, r.max.y];
        let half = |v: &f32| *v != v.round();
        if xs.iter().any(half) || (!text_box(i) && ys.iter().any(half)) {
            out.push(format!("half pixel: {name:?} at {r:?}"));
        }
        if r.min.x < 0.0 || r.max.x > width {
            out.push(format!("outside: {name:?} at {r:?}"));
        }
    }
    let swap =
        |a: &str, b: &str| a == "Swap colors" && ["Primary color", "Secondary color"].contains(&b);
    for (i, (a, ra)) in probe.widgets.iter().enumerate() {
        for (b, rb) in &probe.widgets[i + 1..] {
            let inter = ra.intersect(*rb);
            let glued = inter.width() <= 1.0 || inter.height() <= 1.0;
            // (El cuadro del texto de un campo puede asomar medio punto arriba de él.)
            let nested = ra.expand(0.5).contains_rect(*rb) || rb.expand(0.5).contains_rect(*ra);
            if inter.is_positive() && !glued && !nested && !swap(a, b) && !swap(b, a) {
                out.push(format!("overlap: {a:?} {ra:?} and {b:?} {rb:?}"));
            }
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
        _ => {}
    }
    app
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
fn every_docked_panel_reflows_from_its_minimum_to_600() {
    let started = std::time::Instant::now();
    let mut failures: Vec<String> = Vec::new();
    let mut drawn = 0usize;
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
            let known = KNOWN_GAPS
                .iter()
                .any(|&(p, a, b, _)| p == id && (a..=b).contains(&width));
            if !known && let Some(first) = problems(&probe, width).into_iter().next() {
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
    eprintln!(
        "width sweep: {drawn} widths in {:.1} s",
        started.elapsed().as_secs_f32()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
