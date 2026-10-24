//! Sprite de demostración (`CHIMPSPRITE_DEMO=1`), dibujado a través de comandos normales.

use cs_commands::{Command, DocCommand};
use cs_core::{ColorMode, PixelValue, Sprite};
use cs_raster::{ellipse_filled, ellipse_outline};

use crate::editor::Editor;

/// Documento de demo: un chimpancé y un plátano en artboards separados.
pub fn editor() -> Editor {
    let mut editor = Editor::with_sprite(Sprite::new(64, 64, ColorMode::Rgba));
    editor.execute_doc(DocCommand::SetArtboardProps {
        id: editor.active,
        name: Some("chimp".into()),
        position: None,
    });
    let chimp = editor.active;
    draw_chimp(&mut editor);
    animate_chimp(&mut editor);
    shade_chimp(&mut editor);
    slice_chimp(&mut editor);

    editor.add_artboard(32, 32);
    editor.execute_doc(DocCommand::SetArtboardProps {
        id: editor.active,
        name: Some("banana".into()),
        position: Some((80, 16)),
    });
    draw_banana(&mut editor);

    editor.activate(chimp);
    editor.set_frame(0);
    // Guías al centro del chimpancé.
    editor.set_guides(vec![
        cs_core::Guide {
            axis: cs_core::Axis::Vertical,
            position: 32,
        },
        cs_core::Guide {
            axis: cs_core::Axis::Horizontal,
            position: 32,
        },
    ]);
    // Simetría horizontal para probar dibujando sobre la cara.
    editor.set_symmetry(cs_core::Symmetry {
        axes: cs_core::SymmetryAxes {
            vertical: true,
            ..cs_core::SymmetryAxes::NONE
        },
        center2: None,
    });
    editor
}

/// Capa "Hat", 4 frames (gorro vinculado en todos), parpadeo en el frame 3 y tag "Idle".
fn animate_chimp(editor: &mut Editor) {
    let body = editor.layer;
    editor.rename_layer(body, "Body".into());
    editor.add_layer(cs_core::LayerKind::Image);
    let hat = editor.layer;
    editor.rename_layer(hat, "Hat".into());
    let (w, h) = (editor.sprite().width as i32, editor.sprite().height as i32);
    let (cx, top) = (w / 2, (h as f32 / 2.0 - h as f32 * 0.3) as i32);
    let red = editor.palette_value(27);
    let dark = editor.palette_value(2);
    let white = editor.palette_value(21);
    let mut pixels: Vec<(i32, i32, PixelValue)> =
        ellipse_filled((cx - 9, top - 5), (cx + 8, top + 6))
            .into_iter()
            .filter(|&(_, y)| y <= top + 1)
            .map(|(x, y)| (x, y, red))
            .collect();
    pixels.extend((cx - 10..=cx + 9).map(|x| (x, top + 2, dark)));
    pixels.extend([
        (cx - 1, top - 7, white),
        (cx, top - 7, white),
        (cx - 1, top - 6, white),
        (cx, top - 6, white),
    ]);
    editor.execute(Command::DrawPixels {
        layer: hat,
        frame: 0,
        pixels,
    });

    for _ in 0..3 {
        editor.duplicate_frame();
    }
    for f in 1..4 {
        editor.link_to_previous(hat, f);
    }

    // Parpadeo: ojos cerrados en el frame 3.
    let r = (w.min(h) as f32) * 0.3;
    let (cxf, cyf) = (w as f32 / 2.0, h as f32 / 2.0);
    let eye_y = (cyf - r * 0.1) as i32;
    let face = editor.palette_value(7);
    let lid = editor.palette_value(1);
    let mut blink = Vec::new();
    for ex in [(cxf - r * 0.3) as i32, (cxf + r * 0.3) as i32 - 1] {
        blink.extend([
            (ex, eye_y, face),
            (ex + 1, eye_y, face),
            (ex, eye_y + 1, lid),
            (ex + 1, eye_y + 1, lid),
        ]);
    }
    editor.execute(Command::DrawPixels {
        layer: body,
        frame: 2,
        pixels: blink,
    });
    editor.set_duration_of(0, 300);
    editor.frame_range = Some((0, 3));
    editor.add_tag();
    if let Some(mut tag) = editor.sprite().tags.first().cloned() {
        tag.name = "Idle".into();
        editor.set_tag(0, tag);
    }
    editor.frame_range = None;
    editor.select_layer(body);
}

/// Capa "Shade" recortada al cuerpo (máscara de recorte) con una máscara de capa en degradado:
/// la sombra solo cae sobre el chimpancé y se desvanece hacia el centro.
fn shade_chimp(editor: &mut Editor) {
    let body = editor.layer;
    editor.add_layer(cs_core::LayerKind::Image);
    let shade = editor.layer;
    editor.rename_layer(shade, "Shade".into());
    let (w, h) = (editor.sprite().width as i32, editor.sprite().height as i32);
    let dark = PixelValue::Rgba(cs_core::Rgba::new(40, 20, 60, 120));
    let pixels = (0..h)
        .flat_map(|y| (w / 2..w).map(move |x| (x, y, dark)))
        .collect();
    editor.execute(Command::DrawPixels {
        layer: shade,
        frame: 0,
        pixels,
    });
    for f in 1..editor.sprite().frames.len() {
        editor.link_to_previous(shade, f);
    }
    editor.toggle_clip(shade);
    let mut mask = cs_core::LayerMask::filled(w as u32, h as u32, 255);
    mask.data = std::sync::Arc::new(
        (0..h)
            .flat_map(|_| (0..w).map(|x| ((x - w / 2).clamp(0, 12) * 255 / 12) as u8))
            .collect(),
    );
    editor.execute(Command::SetLayerMask {
        id: shade,
        mask: Some(mask),
    });
    editor.select_layer(body);
}

/// Slices de ejemplo: la hitbox de la cabeza (más chica en el parpadeo) con pivote en la barbilla.
fn slice_chimp(editor: &mut Editor) {
    let key = |frame, bounds, pivot| cs_core::SliceKey {
        frame,
        bounds,
        center: None,
        pivot,
    };
    editor.execute(Command::SetSlices(vec![cs_core::Slice {
        name: "hitbox".into(),
        color: cs_core::Rgba::rgb(230, 60, 80),
        text: "damage=1".into(),
        keys: vec![
            key(0, (13, 14, 38, 38), Some((19, 38))),
            key(2, (15, 18, 34, 34), Some((17, 34))),
            key(3, (13, 14, 38, 38), Some((19, 38))),
        ],
    }]));
}

/// Plátano: una media luna (elipse menos elipse desplazada) con contorno.
fn draw_banana(editor: &mut Editor) {
    let yellow = editor.palette_value(8);
    let outline = editor.palette_value(30);
    let stem = editor.palette_value(4);
    let body: Vec<_> = ellipse_filled((3, 4), (28, 27))
        .into_iter()
        .filter(|p| !ellipse_filled((3, -2), (28, 21)).contains(p))
        .collect();
    let mut pixels: Vec<(i32, i32, PixelValue)> =
        body.iter().map(|&(x, y)| (x, y, yellow)).collect();
    pixels.extend(
        ellipse_outline((3, 4), (28, 27))
            .into_iter()
            .filter(|p| body.contains(p))
            .map(|(x, y)| (x, y, outline)),
    );
    pixels.extend([
        (2, 15, stem),
        (2, 16, stem),
        (1, 15, stem),
        (29, 15, outline),
    ]);
    editor.execute(Command::DrawPixels {
        layer: editor.layer,
        frame: editor.frame,
        pixels,
    });
}

/// Dibuja una cara de chimpancé centrada usando índices de la paleta DB32.
fn draw_chimp(editor: &mut Editor) {
    let (w, h) = (editor.sprite().width as i32, editor.sprite().height as i32);
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let r = (w.min(h) as f32) * 0.3;
    let fur = editor.palette_value(4);
    let fur_dark = editor.palette_value(3);
    let face = editor.palette_value(7);
    let eye = editor.palette_value(1);
    let shine = editor.palette_value(21);
    let mouth = editor.palette_value(2);

    let mut pixels: Vec<(i32, i32, PixelValue)> = Vec::new();
    let inside = |x: i32, y: i32, ox: f32, oy: f32, rx: f32, ry: f32| {
        let dx = (x as f32 + 0.5 - ox) / rx;
        let dy = (y as f32 + 0.5 - oy) / ry;
        dx * dx + dy * dy <= 1.0
    };
    for y in 0..h {
        for x in 0..w {
            let ear = inside(x, y, cx - r, cy, r * 0.35, r * 0.35)
                || inside(x, y, cx + r, cy, r * 0.35, r * 0.35);
            let head = inside(x, y, cx, cy, r, r);
            let muzzle = inside(x, y, cx, cy + r * 0.35, r * 0.6, r * 0.42);
            let brow = inside(x, y, cx - r * 0.3, cy - r * 0.1, r * 0.32, r * 0.3)
                || inside(x, y, cx + r * 0.3, cy - r * 0.1, r * 0.32, r * 0.3);
            let color = if muzzle || brow {
                Some(face)
            } else if head {
                Some(fur)
            } else if ear {
                Some(fur_dark)
            } else {
                None
            };
            if let Some(c) = color {
                pixels.push((x, y, c));
            }
        }
    }
    // Ojos, brillo y boca.
    let eye_y = (cy - r * 0.1) as i32;
    for ex in [(cx - r * 0.3) as i32, (cx + r * 0.3) as i32 - 1] {
        pixels.extend([
            (ex, eye_y, eye),
            (ex + 1, eye_y, eye),
            (ex, eye_y + 1, eye),
            (ex + 1, eye_y + 1, eye),
        ]);
        pixels.push((ex, eye_y, shine));
    }
    let mouth_y = (cy + r * 0.5) as i32;
    for x in (cx - r * 0.25) as i32..=(cx + r * 0.25) as i32 {
        pixels.push((x, mouth_y, mouth));
    }
    pixels.push(((cx - r * 0.3) as i32, mouth_y - 1, mouth));
    pixels.push(((cx + r * 0.3) as i32, mouth_y - 1, mouth));

    editor.execute(Command::DrawPixels {
        layer: editor.layer,
        frame: editor.frame,
        pixels,
    });
}
