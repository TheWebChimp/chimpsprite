//! Sprite de demostración (`CHIMPSPRITE_DEMO=1`), dibujado a través de comandos normales.

use cs_commands::{Command, DocCommand};
use cs_core::{ColorMode, PixelValue, Sprite};
use cs_raster::{ellipse_filled, ellipse_outline};

use crate::editor::Editor;

/// Una variable de entorno de desarrollo (`CHIMPSPRITE_DEMO_*`), leída una sola vez: las que se
/// consultan en cada frame no van al sistema cada vez.
pub(crate) fn dev_var(name: &'static str) -> Option<&'static str> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static VARS: OnceLock<Mutex<HashMap<&'static str, Option<&'static str>>>> = OnceLock::new();
    let vars = VARS.get_or_init(Default::default);
    let mut vars = vars.lock().unwrap_or_else(|e| e.into_inner());
    *vars.entry(name).or_insert_with(|| {
        std::env::var(name)
            .ok()
            .map(|v| &*Box::leak(v.into_boxed_str()))
    })
}

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

/// El techo de la casita del demo: un cono de 4 lados girado 45° (lados paralelos a los del
/// cubo) y aplastado en Y a 5/8, con la base sobre la tapa del cubo: el centro sube 1 + 5/8
/// unidades (con el centro en y = 1 la base quedaba en y = 0.4, enterrada en el cubo, y en Persp
/// solo asomaba la punta).
fn house_roof() -> cs_core::Transform3D {
    let mut t = cs_core::Transform3D::default();
    t.scale.y = 625;
    t.position.y = cs_core::FIXED + cs_core::FIXED * t.scale.y / 1000;
    t.rotation.y = 450;
    t
}

/// La casita del demo animada (CHIMPSPRITE_DEMO_ANIM): el techo en el grupo "Hinge" cuyo origen
/// es la bisagra (arriba atrás del cubo) se abre con un rebote y se cierra; la casa da saltitos.
/// Queda en modo Anim con la pista del grupo y su primer bloque elegidos, en `time` ms.
fn animate_house(e: &mut Editor, time: u32) {
    use cs_core::{Axis3, Clip, ClipMotion, Curve, FIXED, Group3D, Node, Transform3D, Vec3i};
    let Some(mut m) = e.model().cloned() else {
        return;
    };
    if m.objects.len() < 2 {
        return;
    }
    let hinge = Vec3i {
        x: 0,
        y: FIXED,
        z: -FIXED,
    };
    m.objects[1].name = "Roof".into();
    m.objects[1].transform.position.y -= hinge.y;
    m.objects[1].transform.position.z -= hinge.z;
    m.groups = vec![Group3D {
        name: "Hinge".into(),
        transform: Transform3D {
            position: hinge,
            ..Transform3D::default()
        },
        hidden: false,
        locked: false,
        open: true,
        anim: vec![
            Clip {
                curve: Curve::Bounce,
                ..Clip::new(
                    ClipMotion::Rotate {
                        axis: Axis3::X,
                        turns: -150,
                    },
                    0,
                    900,
                )
            },
            Clip {
                curve: Curve::EaseIn,
                ..Clip::new(
                    ClipMotion::Rotate {
                        axis: Axis3::X,
                        turns: 150,
                    },
                    1300,
                    500,
                )
            },
        ],
    }];
    m.objects[0].name = "House".into();
    m.objects[0].anim = vec![Clip {
        curve: Curve::EaseOut,
        ping_pong: true,
        repeat: 2,
        ..Clip::new(
            ClipMotion::Move {
                offset: Vec3i {
                    x: 0,
                    y: FIXED / 4,
                    z: 0,
                },
            },
            1000,
            1000,
        )
    }];
    m.tree = vec![
        Node::Object(0),
        Node::Group {
            group: 0,
            children: vec![Node::Object(1)],
        },
    ];
    let id = e.model_id();
    e.execute_doc(DocCommand::SetModel {
        id,
        model: Some(Box::new(m)),
    });
    e.set_cad_mode(crate::editor::CadMode::Anim);
    e.select_clip(Some((crate::editor::Row::Group(0), 0)));
    e.set_anim_time(time);
}

/// Lo que piden las variables de desarrollo (`CHIMPSPRITE_DEMO_*`) al abrir la app: diálogos,
/// herramientas, rangos y paneles listos para las capturas.
pub(crate) fn startup(app: &mut crate::ChimpApp, ctx: &eframe::egui::Context) {
    use crate::*;
    // Desarrollo: Chimpy colgado con el consejo `id` (p. ej. `walk-cycle`, `tool.pencil`,
    // `hello`), sin tocar lo guardado ni irse solo, para capturas.
    if let Some(id) = dev_var("CHIMPSPRITE_DEMO_MASCOT") {
        let now = ctx.input(|i| i.time);
        app.mascot.force(id, now, &app.keymap);
    }
    // Desarrollo: la pestaña UI Catalog a la vista, para capturas.
    if dev_var("CHIMPSPRITE_DEMO_CATALOG").is_some() {
        app.show_page(tabs::Page::Catalog);
    }
    // Desarrollo: abre el diálogo con un ancho de ejemplo para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_CANVAS_SIZE").is_some() && app.has_doc() {
        app.open_canvas_size();
        if let Some(d) = &mut app.canvas_size {
            d.set_anchor(0, 1);
            d.set_width(d.width() + 24);
        }
    }
    // Desarrollo: la herramienta de selección (sus opciones en la barra) para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_SELECT").is_some() && app.has_doc() {
        app.doc.editor.set_tool(ToolKind::RectSelect);
    }
    // Desarrollo: la selección espejo con los ojos y la boca del chimpancé seleccionados y los
    // ojos acercados al eje (siguen flotando), para capturas.
    if dev_var("CHIMPSPRITE_DEMO_MIRROR").is_some() && app.has_doc() {
        let editor = &mut app.doc.editor;
        editor.set_tool(ToolKind::RectSelect);
        editor.options.mirror_selection = true;
        let (w, h) = (editor.sprite().width, editor.sprite().height);
        let mut mask = cs_core::Mask::from_rect(w, h, (26, 30), (27, 31));
        mask = mask.union(&cs_core::Mask::from_rect(w, h, (36, 30), (37, 31)));
        mask = mask.union(&cs_core::Mask::from_rect(w, h, (26, 40), (37, 41)));
        editor.set_selection(Some(mask));
        editor.mirror_anchor = Some((2 * 26 + 1, 2 * 30 + 1));
        editor.nudge(2, 0);
    }
    // Desarrollo: Palette Presets con una paleta de Coolors pegada (el texto de la variable).
    if let Ok(text) = std::env::var("CHIMPSPRITE_DEMO_COOLORS")
        && app.has_doc()
    {
        app.paste_palette_text(&text);
    }
    // Desarrollo: cuatro colores de la paleta seleccionados, para capturas.
    // (`a,b` elige de la a a la b.)
    if let Ok(range) = std::env::var("CHIMPSPRITE_DEMO_PALETTE_SELECTION")
        && app.has_doc()
    {
        let mut ends = range.split(',').filter_map(|n| n.trim().parse().ok());
        let a = ends.next().unwrap_or(2);
        app.doc.editor.palette_selection = Some((a, ends.next().unwrap_or(a + 3)));
    }
    // Desarrollo: el diálogo de Replace Color (o, con "hue", "curve" u "outline", el de ese
    // filtro).
    if let Ok(which) = std::env::var("CHIMPSPRITE_DEMO_FILTER")
        && app.has_doc()
    {
        let editor = &app.doc.editor;
        let filter = match which.as_str() {
            "curve" => {
                let mut curve = filters::Curve::identity();
                curve.insert(96, 160);
                filters::Filter::ColorCurve {
                    curve,
                    channels: Default::default(),
                }
            }
            "outline" => {
                filters::Filter::Outline(filters::OutlineFx::new(editor.resolve(editor.primary)))
            }
            "hue" => filters::Filter::HueSaturation {
                hue: 90,
                saturation: 0,
                lightness: 0,
            },
            _ => filters::Filter::ReplaceColor {
                from: editor.resolve(editor.primary),
                to: editor.resolve(editor.secondary),
                tolerance: 0,
            },
        };
        app.open_filter(filter);
    }
    // Desarrollo: una herramienta elegida (por su nombre), para capturas; Rectangle con radio.
    if let Ok(name) = std::env::var("CHIMPSPRITE_DEMO_TOOL")
        && app.has_doc()
        && let Some(&tool) = ToolKind::GROUPS
            .iter()
            .flat_map(|g| g.iter())
            .find(|t| t.label() == name)
    {
        app.doc.editor.set_tool(tool);
        app.doc.editor.options.corner_radius = 3;
    }
    // Desarrollo: dos tags más (encimados), para capturas de la fila de tags.
    if std::env::var_os("CHIMPSPRITE_DEMO_TAGS").is_some() && app.has_doc() {
        let editor = &mut app.doc.editor;
        for (range, name) in [((1, 2), "Blink"), ((3, 3), "Hold")] {
            editor.frame_range = Some(range);
            editor.add_tag();
            let i = editor.sprite().tags.len() - 1;
            if let Some(mut tag) = editor.sprite().tags.get(i).cloned() {
                tag.name = name.into();
                editor.set_tag(i, tag);
            }
        }
        editor.frame_range = None;
    }
    // Desarrollo: píxeles al doble de ancho (`2:1`) o de alto (`1:2`), para capturas.
    if let Ok(ratio) = std::env::var("CHIMPSPRITE_DEMO_RATIO")
        && app.has_doc()
    {
        let ratio = if ratio == "1:2" { (1, 2) } else { (2, 1) };
        app.doc.editor.set_pixel_ratio(ratio);
    }
    // Desarrollo: Import Sprite Sheet con una tira de los frames del chango, para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_SHEET").is_some() && app.has_doc() {
        let sprite = app.doc.editor.sprite();
        let (w, h, n) = (sprite.width, sprite.height, sprite.frames.len() as u32);
        let mut strip = cs_io::export::Bitmap::new(w * n, h);
        for f in 0..n {
            let frame = cs_io::export::Bitmap::from_frame(sprite, f as usize, 1);
            for y in 0..h {
                for x in 0..w {
                    strip.pixels[(y * w * n + f * w + x) as usize] =
                        frame.pixels[(y * w + x) as usize];
                }
            }
        }
        app.sheet_dialog = Some(sheet_dialog::SheetDialog::new("walk".into(), strip));
    }
    // Desarrollo: un script de ejemplo (por su archivo, p. ej. `outline.rhai`), para capturas.
    if let Ok(name) = std::env::var("CHIMPSPRITE_DEMO_SCRIPT")
        && app.has_doc()
        && let Some((name, source)) = cs_script::EXAMPLES.iter().find(|(n, _)| *n == name)
    {
        app.doc.editor.primary = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(34, 32, 52));
        app.run_script(name, source);
    }
    // Desarrollo: la capa actual convertida en símbolo y el panel Library abierto, para capturas.
    // Con CHIMPSPRITE_DEMO_SYMBOL=edit, además dentro del modo símbolo; con =inplace, editándolo
    // en su lugar (el resto atenuado) con un trazo nuevo encima; con =tween, con un tween.
    if let Some(mode) = dev_var("CHIMPSPRITE_DEMO_SYMBOL")
        && app.has_doc()
    {
        let editor = &mut app.doc.editor;
        // El sombrero: el cuerpo tiene una capa recortada encima y no se puede partir.
        if let Some(hat) = editor.sprite().layers.iter().find(|l| l.name == "Hat") {
            let hat = hat.id;
            editor.select_layer(hat);
        }
        let name = editor.suggested_symbol_name();
        if let Ok(symbol) =
            editor.convert_to_symbol(&name, cs_commands::symbol::SymbolTiming::Graphic)
        {
            if !matches!(mode, "objects" | "inplace" | "tween") {
                dock::set_open(ctx, Some(dock::DockPanel::Library));
            }
            // Con =objects, dos sombreros más en la misma capa, la capa desplegada en el
            // timeline y el último seleccionado.
            if mode == "objects" {
                let layer = (editor.active, editor.layer);
                let (x, y) = editor.artboard().position;
                for at in [(4, 30), (38, 40)] {
                    let _ = editor.drop_symbol(symbol, (x + at.0, y + at.1));
                }
                app.doc.timeline.expanded_objects.insert(layer);
            }
            // Con =tween, el sombrero va del primer frame al último corriéndose, creciendo y
            // girando (classic tween con ease out), la capa desplegada y un frame del medio.
            if mode == "tween"
                && let Some(object) = editor.current_object()
            {
                let layer = editor.layer;
                let last = editor.sprite().frames.len().saturating_sub(1);
                editor.set_frame(last);
                editor.update_instance_key(|k| {
                    k.position = (k.position.0 + 16, k.position.1 + 6);
                    k.scale = (1500, 1500);
                    k.rotation = 30;
                });
                editor.set_frame(0);
                editor.create_tweens(layer, &[object], 0);
                editor.select_object(layer, object, false);
                editor.update_current_tween(None, |t| t.ease = cs_core::Ease::Classic(100));
                editor.set_frame(last.div_ceil(2));
                app.doc
                    .timeline
                    .expanded_objects
                    .insert((editor.active, layer));
            }
            if mode == "edit" && editor.enter_symbol(symbol) {
                app.doc.canvas.request_zoom(crate::view::ZoomRequest::Fit);
            }
            if mode == "inplace"
                && let Some(object) = editor.current_object()
                && editor.enter_symbol_in_place(editor.layer, object)
            {
                // Un trazo en el símbolo: se ve en su lugar (y en la Library).
                let pink = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 80, 160));
                let (w, h) = (editor.sprite().width as i32, editor.sprite().height as i32);
                let pixels = (0..w.min(h)).map(|i| (i, i, pink)).collect();
                editor.execute(cs_commands::Command::DrawPixels {
                    layer: editor.sprite().layers.last().map_or(editor.layer, |l| l.id),
                    frame: editor.frame,
                    pixels,
                });
            }
        }
    }
    // Desarrollo: el panel Groom abierto sin imagen (`=empty`, la ilustración) o con una imagen
    // (su ruta), para capturas.
    if dev_var("CHIMPSPRITE_DEMO_GROOM") == Some("empty") && app.has_doc() {
        dock::set_open(ctx, Some(dock::DockPanel::Groom));
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(path) = std::env::var("CHIMPSPRITE_DEMO_GROOM")
        && app.has_doc()
        && let Ok(bytes) = std::fs::read(&path)
        && let Ok(image) = groom::decode_image(&bytes)
    {
        dock::set_open(ctx, Some(dock::DockPanel::Groom));
        app.groom.load("groomed".into(), image);
        // Con CHIMPSPRITE_DEMO_GROOM_PICK=n, los n colores más usados elegidos (la vista previa
        // prueba juntarlos).
        // Con CHIMPSPRITE_DEMO_GROOM_FIT=<paleta>, ajustado a ella.
        if let Some(name) = dev_var("CHIMPSPRITE_DEMO_GROOM_FIT") {
            app.groom.fit_to(name);
        }
        if let Some(n) = dev_var("CHIMPSPRITE_DEMO_GROOM_PICK").and_then(|n| n.parse().ok()) {
            let primary = app.doc.editor.resolve(app.doc.editor.primary);
            app.groom.pick_most_used(primary, n);
        }
    }
    // Desarrollo: el panel Lumen abierto con una imagen de albedo (su ruta), para capturas. Con
    // CHIMPSPRITE_DEMO_LUMEN_MODIFY=normal|luminance, con los ajustes de ese mapa abiertos.
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(path) = std::env::var("CHIMPSPRITE_DEMO_LUMEN")
        && app.has_doc()
        && let Ok(bytes) = std::fs::read(&path)
        && let Ok(image) = groom::decode_image(&bytes)
    {
        dock::set_open(ctx, Some(dock::DockPanel::Lumen));
        app.lumen.load("lumen".into(), image);
        match dev_var("CHIMPSPRITE_DEMO_LUMEN_MODIFY") {
            Some("normal") => app.lumen.open_settings(crate::lumen::MapKind::Normal),
            Some("luminance") => app.lumen.open_settings(crate::lumen::MapKind::Luminance),
            _ => {}
        }
        // CHIMPSPRITE_DEMO_LUMEN_PICKER=1: el selector del color de la luz abierto.
        if dev_var("CHIMPSPRITE_DEMO_LUMEN_PICKER").is_some() {
            ctx.data_mut(|d| {
                d.insert_temp(
                    eframe::egui::Id::new(crate::kit::color_button::DEMO_PICKER),
                    eframe::egui::Id::NULL,
                )
            });
        }
    }
    // Desarrollo: Tiled Mode en los dos ejes, para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_TILED").is_some() && app.has_doc() {
        app.doc.editor.tiled = editor::TiledMode::Both;
    }
    // Desarrollo: Keyboard Shortcuts o Preferences abiertas, para capturas.
    match std::env::var("CHIMPSPRITE_DEMO_WINDOW").as_deref() {
        Ok("shortcuts") => app.shortcuts = Some(Default::default()),
        Ok("preferences") => app.preferences = true,
        // Con CHIMPSPRITE_DEMO_TILEMAP para que haya capa tilemap.
        Ok("tiled") if app.has_doc() => app.open_tiled_export(),
        Ok("color_range") if app.has_doc() => {
            app.select_dialog = Some(select_dialogs::SelectDialog::color_range(&app.doc.editor));
        }
        _ => {}
    }
    // Desarrollo: la ventana Undo History con unos pasos, para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_UNDO_HISTORY").is_some() && app.has_doc() {
        let editor = &mut app.doc.editor;
        editor.add_layer(cs_core::LayerKind::Image);
        editor.insert_empty_frame();
        editor.undo();
        app.undo_history = true;
    }
    // Desarrollo: el campo de renombrar de la capa activa abierto, para capturas.
    if dev_var("CHIMPSPRITE_DEMO_LAYER_RENAME").is_some() && app.has_doc() {
        let layer = app.doc.editor.layer;
        if let Some(name) = app.doc.editor.sprite().layer(layer).map(|l| l.name.clone()) {
            app.doc.timeline.start_rename(layer, name);
        }
    }
    // Desarrollo: un rango de cels del timeline ("fila,frame,fila,frame"), para capturas.
    if let Ok(range) = std::env::var("CHIMPSPRITE_DEMO_CELS")
        && app.has_doc()
    {
        let n: Vec<usize> = range
            .split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        let order = app.doc.editor.timeline_layers();
        if let [ra, fa, rb, fb] = n[..]
            && let (Some(&a), Some(&b)) = (order.get(ra), order.get(rb))
        {
            app.doc.editor.select_layer(a);
            app.doc.editor.select_cels((a, fa), (b, fb));
        }
    }
    // Desarrollo: una nota con texto, para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_NOTE").is_some() && app.has_doc() {
        // A la derecha del primer artboard (la vista todavía no se acomodó).
        let at = app.doc.editor.document.artboards[0].rect().1;
        if let Some(id) = app.doc.editor.add_note((at.0 + 4, 0)) {
            app.doc
                .editor
                .set_note_text(id, "Revisar la animación del mono 🐒".into());
            app.doc.editor.set_note_color(id, cs_core::NoteColor::Blue);
        }
        // Con `many`, una amarilla de varias líneas encima del artboard y una rosa duplicada; con
        // `edit`, además se escribe en la amarilla (con su barra y la tira de colores).
        let mode = std::env::var("CHIMPSPRITE_DEMO_NOTE").unwrap_or_default();
        if (mode == "many" || mode == "edit")
            && let Some(id) = app.doc.editor.add_note((-40, 8))
        {
            app.doc
                .editor
                .set_note_text(id, "TODO:\n- walk cycle\n- idle blink".into());
            if let Some(copy) = app.doc.editor.duplicate_note(id, 12) {
                app.doc
                    .editor
                    .set_note_text(copy, "Ask about palette".into());
                app.doc
                    .editor
                    .set_note_color(copy, cs_core::NoteColor::Pink);
            }
            if mode == "edit" {
                app.doc.editor.note_to_front(id);
                app.doc.canvas.notes.start_editing(&mut app.doc.editor, id);
            }
        }
    }
    // Desarrollo: el panel Dafont del dock abierto, para capturas.
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("CHIMPSPRITE_DEMO_DAFONT").is_some() && app.has_doc() {
        dock::set_open(ctx, Some(dock::DockPanel::Dafont));
        // Con `detail`, el detalle de la fuente de la app.
        if std::env::var("CHIMPSPRITE_DEMO_DAFONT").as_deref() == Ok("detail")
            && let Ok(font) =
                cs_tools::text::FontChoice::load("ChimpSprite", appearance::PIXEL_FONT)
        {
            app.dafont.demo_detail(font);
        }
    }
    // Desarrollo: el panel Lospec del dock abierto, para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_LOSPEC").is_some() && app.has_doc() {
        dock::set_open(ctx, Some(dock::DockPanel::Lospec));
    }
    // Desarrollo: el panel Artboards con muchos artboards más (uno de nombre largo), para
    // capturas de sus pestañas.
    if std::env::var_os("CHIMPSPRITE_DEMO_ARTBOARDS").is_some() && app.has_doc() {
        dock::set_open(ctx, Some(dock::DockPanel::Artboards));
        let editor = &mut app.doc.editor;
        let names = [
            "Artboard 3",
            "Artboard 4",
            "A very long artboard name",
            "Walk",
            "Run",
        ];
        // Con un número, solo esos (si no, todos).
        let count = std::env::var("CHIMPSPRITE_DEMO_ARTBOARDS")
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(usize::MAX);
        let names = names.iter().chain(&["Jump", "Idle", "Hurt", "Win"]);
        for name in names.take(count) {
            editor.add_artboard(64, 64);
            editor.rename_active_artboard((*name).into());
        }
        // Con CHIMPSPRITE_DEMO_ARTBOARD_RENAME, el campo de renombrar del activo abierto.
        if dev_var("CHIMPSPRITE_DEMO_ARTBOARD_RENAME").is_some() {
            app.doc.canvas.start_rename(&app.doc.editor);
        }
    }
    // Desarrollo: dos grupos (uno colapsado), para capturas de sus carpetitas.
    if std::env::var_os("CHIMPSPRITE_DEMO_GROUP").is_some() && app.has_doc() {
        let editor = &mut app.doc.editor;
        editor.add_layer(cs_core::LayerKind::Group);
        if let Some(id) = editor.sprite().layers.last().map(|l| l.id) {
            editor.toggle_collapsed(id);
        }
        editor.add_layer(cs_core::LayerKind::Group);
    }
    // Desarrollo: abre Sprite Size para capturas.
    if std::env::var_os("CHIMPSPRITE_DEMO_SPRITE_SIZE").is_some() && app.has_doc() {
        app.open_sprite_dialog(|a| {
            sprite_dialogs::SpriteDialog::Size(sprite_dialogs::SpriteSizeDialog::new(a))
        });
    }
    // Desarrollo: abre el diálogo de nueva capa tilemap para capturas (con --demo).
    // Desarrollo: la capa activa pasada a tilemap de 8×8 (con su panel del tileset) y el volteo X
    // del pincel prendido, para capturas.
    // Con CHIMPSPRITE_DEMO_TILEMAP=pixels (o pixels-manual, pixels-stack): en modo Pixels con ese
    // modo de tileset, el lápiz elegido y una línea pintada a través de los tiles.
    if let Some(value) = dev_var("CHIMPSPRITE_DEMO_TILEMAP")
        && app.has_doc()
    {
        let editor = &mut app.doc.editor;
        editor.convert_to_tilemap((8, 8));
        editor.toggle_brush_flip(cs_core::Tile::FLIP_X);
        if let Some(mode) = value.strip_prefix("pixels") {
            use cs_commands::tiles::TilesetMode;
            editor.set_tilemap_pixels(true);
            editor.options.tileset_mode = match mode {
                "-manual" => TilesetMode::Manual,
                "-stack" => TilesetMode::Stack,
                _ => TilesetMode::Auto,
            };
            editor.set_tool(editor::ToolKind::Pencil);
            let color = editor.primary;
            let width = editor.sprite().width as i32;
            let pixels = (4..width - 4)
                .map(|x| (x, 40 + (x / 6) % 3, color))
                .collect();
            let cmd = Command::DrawPixels {
                layer: editor.layer,
                frame: editor.frame,
                pixels,
            };
            editor.execute_paint(cmd, "Pencil");
        }
    }
    // Desarrollo: una fuente de 8×8 con unas letras y el panel Font abierto, para capturas.
    if dev_var("CHIMPSPRITE_DEMO_FONT").is_some() && app.has_doc() {
        // Con CHIMPSPRITE_DEMO_FONT_CONFIG, en lugar de la de 8×8, una fuente nueva de 16×16 de
        // un solo estilo con la vista de ajustes (la del mockup de los ajustes). Con `=sample`,
        // una H dibujada (el glifo actual, "!", sigue vacío: la vista previa muestra la H).
        if let Some(config) = dev_var("CHIMPSPRITE_DEMO_FONT_CONFIG") {
            if app.create_font(ctx, "New Font", (16, 16), cs_core::FontMode::OneBit) {
                app.font_panel.view = crate::font_panel::PanelView::Config;
                if config.trim() == "sample" {
                    let e = &mut app.doc.editor;
                    let current = e.current_glyph();
                    e.go_to_glyph('H');
                    let (layer, frame) = (e.layer, e.frame);
                    // Las dos astas (filas 2 a 11, sobre la baseline 12) y el travesaño.
                    let pixels = (2..12)
                        .flat_map(|y| [(3, y), (4, y), (9, y), (10, y)])
                        .chain((5..9).map(|x| (x, 6)))
                        .map(|(x, y)| (x, y, cs_core::ONE_BIT_INK))
                        .collect();
                    e.execute(Command::DrawPixels {
                        layer,
                        frame,
                        pixels,
                    });
                    if let Some(ch) = current {
                        e.go_to_glyph(ch);
                    }
                }
            }
        }
        // Como el OK de File → New Font…: con su panel abierto y encuadrada al lado de él.
        else if app.create_font(ctx, "Demo Font", (8, 8), cs_core::FontMode::OneBit) {
            let e = &mut app.doc.editor;
            type Points = &'static [(i32, i32)];
            let letters: [(char, Points); 3] = [
                (
                    'A',
                    &[
                        (1, 1),
                        (2, 1),
                        (0, 2),
                        (3, 2),
                        (0, 3),
                        (1, 3),
                        (2, 3),
                        (3, 3),
                        (0, 4),
                        (3, 4),
                        (0, 5),
                        (3, 5),
                    ],
                ),
                (
                    'B',
                    &[
                        (0, 1),
                        (1, 1),
                        (2, 1),
                        (0, 2),
                        (3, 2),
                        (0, 3),
                        (1, 3),
                        (2, 3),
                        (0, 4),
                        (3, 4),
                        (0, 5),
                        (1, 5),
                        (2, 5),
                    ],
                ),
                (
                    'C',
                    &[
                        (1, 1),
                        (2, 1),
                        (3, 1),
                        (0, 2),
                        (0, 3),
                        (0, 4),
                        (1, 5),
                        (2, 5),
                        (3, 5),
                    ],
                ),
            ];
            for (ch, points) in letters {
                e.go_to_glyph(ch);
                let (layer, frame) = (e.layer, e.frame);
                e.execute(Command::DrawPixels {
                    layer,
                    frame,
                    pixels: points
                        .iter()
                        .map(|&(x, y)| (x, y, cs_core::ONE_BIT_INK))
                        .collect(),
                });
            }
            e.go_to_glyph('A');
            app.font_panel.test_text = "ABC CAB".into();
            // Con CHIMPSPRITE_DEMO_FONT_COMPOSITE, unas minúsculas y marcas, los acentos armados
            // con Auto-build accents y la `á` abierta con su `´` elegida (con Move).
            if dev_var("CHIMPSPRITE_DEMO_FONT_COMPOSITE").is_some() {
                demo_accents(&mut app.doc.editor);
                app.font_panel.test_text = "àéîõüçñå ÀÇÑ".into();
                app.font_panel.page = crate::font_panel::Page::Latin1;
            }
            // Con CHIMPSPRITE_DEMO_FONT_KERN, unos pares y una clase, y la vista Kerning con el
            // par A·B elegido.
            if dev_var("CHIMPSPRITE_DEMO_FONT_KERN").is_some() {
                use cs_core::KernSide::{Char, Class};
                let e = &mut app.doc.editor;
                let mut k = e.font().map(|f| f.kerning.clone()).unwrap_or_default();
                k.classes.push(cs_core::KernClass {
                    name: "round".into(),
                    side: cs_core::ClassSide::Second,
                    chars: vec!['C', 'O'],
                });
                k.set(Char('A'), Char('B'), -1);
                k.set(Char('B'), Class("round".into()), 1);
                e.set_kerning(k);
                app.font_panel.view = crate::font_panel::PanelView::Kerning;
                app.font_panel.kern_pair = Some(('A', 'B'));
                app.font_panel.test_text = "ABC CAB".into();
            }
            // Con CHIMPSPRITE_DEMO_FONT_STYLES, la Bold y la Italic generadas y los ajustes de la
            // Italic con la familia (en su pestaña).
            if dev_var("CHIMPSPRITE_DEMO_FONT_STYLES").is_some() {
                use cs_core::{FontStyle, VariantKind, VariantRecipe};
                for kind in [VariantKind::Bold, VariantKind::Italic] {
                    let e = &mut app.doc.editor;
                    let regular = e.family_fonts().first().map(|(id, _)| *id);
                    if let Some(id) = regular {
                        e.enter_workspace(id);
                    }
                    let style = FontStyle {
                        name: kind.style_name().into(),
                        weight: kind.weight(),
                        italic: kind.italic(),
                        slant: if kind.italic() { 3 } else { 0 },
                        source: None,
                    };
                    if e.add_variant(&VariantRecipe::new(kind), style)
                        && let Some(id) = e.workspace()
                    {
                        app.tabs.open_view(&mut app.doc, id);
                    }
                }
                app.doc.editor.go_to_glyph('A');
                app.font_panel.view = crate::font_panel::PanelView::Config;
            }
            // Con CHIMPSPRITE_DEMO_FONT_VARIANT, el diálogo New Variant… abierto (`=outline`,
            // `=italic`… elige el tipo).
            if let Some(kind) = dev_var("CHIMPSPRITE_DEMO_FONT_VARIANT") {
                app.open_new_variant();
                let kind = cs_core::VariantKind::ALL
                    .into_iter()
                    .find(|k| k.style_name().eq_ignore_ascii_case(kind.trim()));
                if let (Some(dialog), Some(kind)) = (app.new_variant.as_mut(), kind) {
                    dialog.set_kind(kind);
                }
            }
            // Con CHIMPSPRITE_DEMO_FONT_BUILD, el diálogo Build Font abierto.
            if dev_var("CHIMPSPRITE_DEMO_FONT_BUILD").is_some() {
                app.open_build_font(None);
            }
        }
    }
    // Desarrollo: Glyphs from image… con un PNG, para capturas (con CHIMPSPRITE_DEMO_FONT).
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = dev_var("CHIMPSPRITE_DEMO_FONT_SHEET")
        && app.doc.editor.is_font()
    {
        match std::fs::read(path) {
            Ok(bytes) => app.opened_glyph_sheet(&bytes),
            Err(e) => log::warn!("CHIMPSPRITE_DEMO_FONT_SHEET: {e}"),
        }
    }
    // Desarrollo: el diálogo Import Font de un TTF/OTF (o un JSON de BitFontMaker2 importado),
    // para capturas.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = dev_var("CHIMPSPRITE_DEMO_FONT_IMPORT") {
        match std::fs::read(path) {
            Ok(bytes) => app.opened_font_file(ctx, path, bytes),
            Err(e) => log::warn!("CHIMPSPRITE_DEMO_FONT_IMPORT: {e}"),
        }
    }
    // Desarrollo: una casita 3D (cubo y cono de techo) con su textura pintada, cámara iso, luz y
    // contorno, y el panel 3D abierto, para capturas.
    if dev_var("CHIMPSPRITE_DEMO_MODEL").is_some()
        && app.has_doc()
        && app.create_model("House", (128, 128))
    {
        let e = &mut app.doc.editor;
        if e.add_primitive(cs_3d::Primitive::Cone { sides: 4 })
            && let Some(i) = e.selected_object()
            && let Some(mut roof) = e.model().and_then(|m| m.objects.get(i).cloned())
        {
            roof.transform = house_roof();
            e.set_object(i, roof);
        }
        // Ventana y puerta en la cara de enfrente del cubo (la celda (0,0)-(16,16) de la textura).
        let model = e.model().cloned();
        if let Some(texture) = model.as_ref().and_then(|m| m.texture) {
            let home = e.active;
            let rect = |x0: i32, y0: i32, x1: i32, y1: i32, c: cs_core::Rgba| {
                (y0..y1)
                    .flat_map(|y| (x0..x1).map(move |x| (x, y, PixelValue::Rgba(c))))
                    .collect::<Vec<_>>()
            };
            let mut pixels = rect(2, 3, 7, 8, cs_core::Rgba::rgb(110, 190, 240));
            pixels.extend(rect(10, 7, 14, 16, cs_core::Rgba::rgb(120, 70, 40)));
            e.activate(texture);
            let (layer, frame) = (e.layer, e.frame);
            e.execute(Command::DrawPixels {
                layer,
                frame,
                pixels,
            });
            // Pintar la textura (en el lienzo) sale del workspace: se vuelve a él.
            e.enter_workspace(home);
        }
        if let Some(m) = e.model() {
            // Con CHIMPSPRITE_DEMO_ZOOM=<n>: la cámara n veces más cerca (como con la rueda).
            let zoom = dev_var("CHIMPSPRITE_DEMO_ZOOM")
                .and_then(|z| z.parse::<f32>().ok())
                .filter(|z| *z > 0.0)
                .unwrap_or(1.0);
            let distance = m.camera.distance.max(cs_core::FIXED) as f32 / zoom;
            let camera = cs_core::CameraPreset::Iso
                .camera((distance.round() as i32).max(1), m.camera.target);
            e.set_camera(camera);
        }
        // Con CHIMPSPRITE_DEMO_DITHER: las sombras con tramado.
        e.set_light(cs_core::Light3D {
            dither: dev_var("CHIMPSPRITE_DEMO_DITHER").is_some(),
            ..cs_core::Light3D::default()
        });
        e.set_outline(Some(cs_core::Rgba::rgb(0, 0, 0)));
        // El panel sin objeto elegido (abajo, solo la tira); con CHIMPSPRITE_DEMO_MODEL_TEXTURE,
        // con el cubo.
        e.select_model_object(None);
        // Con CHIMPSPRITE_DEMO_ADD_MENU: el menú del + de la tira abierto.
        if dev_var("CHIMPSPRITE_DEMO_ADD_MENU").is_some() {
            app.model_panel.open_add_menu = true;
        }
        // Con CHIMPSPRITE_DEMO_SHADING: la ventana Shading abierta.
        if dev_var("CHIMPSPRITE_DEMO_SHADING").is_some() {
            app.model_panel.shading = true;
        }
        // Con CHIMPSPRITE_DEMO_MODEL_EDIT: en vértices y con uno elegido (el workspace siempre
        // está en modo Edit).
        if dev_var("CHIMPSPRITE_DEMO_MODEL_EDIT").is_some() {
            e.set_select_mode(cs_3d::SelectMode::Vertex);
            e.set_model_selection(cs_3d::Selection {
                mode: cs_3d::SelectMode::Vertex,
                items: [cs_3d::Item::Vertex(0, 7)].into(),
            });
        }
        // Con CHIMPSPRITE_DEMO_EXTRUDE: la cara de arriba del cubo extruida una unidad (en modo
        // Face, con la cara extruida elegida).
        if dev_var("CHIMPSPRITE_DEMO_EXTRUDE").is_some() {
            let top = e.model().and_then(|m| {
                let mesh = &m.objects.first()?.mesh;
                mesh.faces.iter().position(|f| {
                    f.verts
                        .iter()
                        .all(|&v| mesh.vertices.get(v as usize).is_some_and(|p| p.y > 0))
                })
            });
            if let Some(f) = top {
                e.set_select_mode(cs_3d::SelectMode::Face);
                e.set_model_selection(cs_3d::Selection {
                    mode: cs_3d::SelectMode::Face,
                    items: [cs_3d::Item::Face(0, f)].into(),
                });
                e.extrude_moved([0.0, cs_core::FIXED as f32, 0.0]);
            }
        }
        // Con CHIMPSPRITE_DEMO_GIZMO=move|rotate|scale: el cubo elegido en modo Object con ese
        // gizmo.
        if let Some(name) = dev_var("CHIMPSPRITE_DEMO_GIZMO") {
            use crate::editor::Gizmo;
            let gizmo = match name {
                "rotate" => Gizmo::Rotate,
                "scale" => Gizmo::Scale,
                "none" => Gizmo::None,
                _ => Gizmo::Move,
            };
            e.select_model_object(Some(0));
            e.set_gizmo(gizmo);
        }
        // Con CHIMPSPRITE_DEMO_MODEL_TEXTURE: en modo Texture, con la cara de enfrente del cubo
        // (la de z+) elegida.
        if dev_var("CHIMPSPRITE_DEMO_MODEL_TEXTURE").is_some() {
            let front = e.model().and_then(|m| {
                let mesh = &m.objects.first()?.mesh;
                mesh.faces.iter().position(|f| {
                    f.verts
                        .iter()
                        .all(|&v| mesh.vertices.get(v as usize).is_some_and(|p| p.z > 0))
                })
            });
            e.select_model_object(Some(0));
            e.set_cad_mode(crate::editor::CadMode::Texture);
            if let Some(f) = front {
                e.set_model_selection(cs_3d::Selection {
                    mode: cs_3d::SelectMode::Face,
                    items: [cs_3d::Item::Face(0, f)].into(),
                });
                // Como al elegirla con un clic: la textura de abajo se encuadra en ella.
                e.model_edit.focus_uvs = true;
            }
        }
        // Con CHIMPSPRITE_DEMO_ANIM[=<ms>]: la casita animada en modo Anim (ver
        // `animate_house`), en ese tiempo (600 ms si no dice).
        if let Some(t) = dev_var("CHIMPSPRITE_DEMO_ANIM") {
            animate_house(&mut app.doc.editor, t.parse().unwrap_or(600));
        }
        // Con CHIMPSPRITE_DEMO_GIF=<preset> (native, nintendo_ds, gameboy, pico-8): GIF
        // settings abierto con ese preset, escala 2, fondo y scanlines. Con
        // CHIMPSPRITE_DEMO_GIF_MOTION=<Turntable|Animation|Both>, ese Motion (12 frames); con
        // CHIMPSPRITE_DEMO_GIF_FILE=<ruta>, además escribe ahí el GIF.
        if let Some(name) = dev_var("CHIMPSPRITE_DEMO_GIF") {
            let mut gif = crate::gif_dialog::GifDialog::new(&app.doc.editor);
            if let Some(motion) = dev_var("CHIMPSPRITE_DEMO_GIF_MOTION")
                .and_then(|m| cs_render::Motion::ALL.into_iter().find(|x| x.label() == m))
            {
                gif.options.bake.motion = motion;
                gif.options.bake.frames = 12;
            }
            if let Some(preset) = cs_render::RenderPreset::ALL
                .into_iter()
                .find(|p| p.label() == name)
            {
                gif.options.preset = preset;
            }
            gif.options.scale = 2;
            gif.options.background = Some(cs_core::Rgba::rgb(0x1d, 0x2b, 0x53));
            gif.options.scanlines = Some(30);
            if let Some(path) = dev_var("CHIMPSPRITE_DEMO_GIF_FILE") {
                match gif.build(&app.doc.editor.document) {
                    Ok(bytes) => {
                        if let Err(e) = std::fs::write(path, bytes) {
                            log::error!("demo gif: {e}");
                        }
                    }
                    Err(e) => log::error!("demo gif: {e}"),
                }
            }
            app.gif = Some(gif);
        }
    }
    // Desarrollo: el panel Library abierto, de vuelta en el lienzo (con _MODEL y _FONT, con un
    // modelo y una fuente en sus secciones), para capturas.
    if dev_var("CHIMPSPRITE_DEMO_LIBRARY").is_some() && app.has_doc() {
        app.doc.editor.exit_workspace();
        dock::set_open(ctx, Some(dock::DockPanel::Library));
    }
    if std::env::var_os("CHIMPSPRITE_DEMO_NEW_TILEMAP").is_some() && app.has_doc() {
        app.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&app.doc.editor));
    }
    // Desarrollo: una copia del chango y el original animándose en el lugar mientras se edita la
    // copia, para capturas del ▶.
    if dev_var("CHIMPSPRITE_DEMO_IN_PLACE").is_some() && app.has_doc() {
        let editor = &mut app.doc.editor;
        let original = editor.active;
        let name = format!("{} Copy", editor.artboard().name);
        editor.duplicate_artboard(name);
        editor.toggle_play_in_place(original);
    }
    // Desarrollo: Expand to Spritesheet del chango abierto; con CHIMPSPRITE_DEMO_EXPAND=add, la
    // hoja ya agregada (con spacing 0 y la grilla de las celdas).
    if let Some(mode) = dev_var("CHIMPSPRITE_DEMO_EXPAND")
        && app.has_doc()
    {
        app.open_expand_sheet();
        if mode == "add"
            && let Some(dialog) = app.expand_dialog.take()
            && let Ok(sheet) = dialog.sheet()
        {
            let name = format!("{} Sheet", dialog.name);
            app.doc.editor.add_artboard_with_sprite(name, sheet);
        }
    }
    // Desarrollo: la hoja del chango (Expand) y Slice into Frames abierto sobre ella.
    if dev_var("CHIMPSPRITE_DEMO_SLICE").is_some() && app.has_doc() {
        app.open_expand_sheet();
        if let Some(dialog) = app.expand_dialog.take()
            && let Ok(sheet) = dialog.sheet()
        {
            let name = format!("{} Sheet", dialog.name);
            app.doc.editor.add_artboard_with_sprite(name, sheet);
            app.open_slice_frames();
        }
    }
    demo_iso(app);
}

/// Desarrollo: las herramientas isométricas. `CHIMPSPRITE_DEMO_ISO=box` abre Edit → Iso Box…,
/// `=blocks` lo abre en Blocks y `=placed` arma un artboard de 96×72 con la cuadrícula
/// isométrica, unas pilas y una caja ya colocadas y otra caja flotando. Con
/// `CHIMPSPRITE_DEMO_ISO_GRID`, la cuadrícula del artboard activo es isométrica de 16×8 (con
/// `=dialog`, además Grid Settings abierto).
fn demo_iso(app: &mut crate::ChimpApp) {
    use crate::iso_dialog::IsoMode;
    use cs_raster::{IsoBox, IsoCorner};
    if !app.has_doc() {
        return;
    }
    let mode = dev_var("CHIMPSPRITE_DEMO_ISO");
    if mode == Some("placed") {
        let sprite = cs_core::Sprite::new(96, 72, cs_core::ColorMode::Rgba);
        app.doc
            .editor
            .add_artboard_with_sprite("Iso".into(), sprite);
        let place = |app: &mut crate::ChimpApp,
                     settings: &crate::iso_dialog::IsoSettings,
                     base: cs_core::Rgba,
                     at: Option<(i32, i32)>,
                     commit: bool| {
            let d = crate::iso_dialog::IsoDialog::new(settings.clone(), base, &[], false);
            let image = d.image();
            let editor = &mut app.doc.editor;
            let origin = at.unwrap_or_else(|| editor.centered_origin((image.width, image.height)));
            if let Some(clip) = crate::clipboard::Clip::from_shape(
                origin,
                (image.width, image.height),
                &image.pixels,
            ) {
                editor.place_floating(&clip, d.label());
                if commit {
                    editor.commit_floating();
                }
            }
        };
        let mut blocks = crate::iso_dialog::IsoSettings {
            mode: IsoMode::Blocks,
            ..Default::default()
        };
        blocks.blocks.resize(4, 3);
        blocks.blocks.heights = vec![3, 2, 1, 1, 2, 1, 0, 1, 1, 1, 1, 2];
        blocks.blocks.block_height = 5;
        place(
            app,
            &blocks,
            cs_core::Rgba::rgb(0x6a, 0xbe, 0x30),
            Some((4, 4)),
            true,
        );
        let mut cube = crate::iso_dialog::IsoSettings {
            shape: IsoBox {
                left: 5,
                right: 3,
                height: 10,
                corner: IsoCorner::Two,
            },
            ..Default::default()
        };
        place(
            app,
            &cube,
            cs_core::Rgba::rgb(0xdf, 0x71, 0x26),
            Some((60, 8)),
            true,
        );
        cube.shape = IsoBox {
            left: 4,
            right: 4,
            height: 6,
            corner: IsoCorner::Three,
        };
        place(
            app,
            &cube,
            cs_core::Rgba::rgb(0x5b, 0x6e, 0xe1),
            Some((56, 40)),
            false,
        );
    }
    if mode == Some("placed") || dev_var("CHIMPSPRITE_DEMO_ISO_GRID").is_some() {
        let mut grid = app.doc.editor.sprite().grid;
        grid.size = (16, 8);
        grid.kind = cs_core::GridKind::Isometric;
        app.doc.editor.execute(cs_commands::Command::SetGrid(grid));
        app.view.grid = true;
        if dev_var("CHIMPSPRITE_DEMO_ISO_GRID") == Some("dialog") {
            app.grid_dialog = Some(crate::view::GridDialog::new(&app.doc.editor, &app.view));
        }
    }
    if mode == Some("box") || mode == Some("blocks") {
        if mode == Some("blocks") {
            app.iso_settings.mode = IsoMode::Blocks;
        }
        app.open_iso_box();
    }
}

/// Para la captura de los compuestos: unas minúsculas y marcas en la fuente de ejemplo (8×8,
/// baseline 6), Auto-build accents y la `á` abierta con la `´` elegida, con Move.
fn demo_accents(e: &mut crate::editor::Editor) {
    type Rows = &'static [&'static str];
    // Cada glifo: su carácter, la fila de arriba y sus filas ('X' = tinta).
    let glyphs: [(char, i32, Rows); 16] = [
        ('a', 3, &[".XXX", "X..X", ".XXX"]),
        ('c', 3, &[".XXX", "X...", ".XXX"]),
        ('e', 3, &[".XX.", "XXXX", ".XX."]),
        ('i', 1, &["X", ".", "X", "X", "X"]),
        ('ı', 3, &["X", "X", "X"]),
        ('n', 3, &["XXX.", "X..X", "X..X"]),
        ('o', 3, &[".XX.", "X..X", ".XX."]),
        ('u', 3, &["X..X", "X..X", ".XXX"]),
        ('´', 0, &["..X", ".X."]),
        ('`', 0, &[".X.", "..X"]),
        ('ˆ', 0, &[".X.", "X.X"]),
        ('˜', 0, &[".X.X", "X.X."]),
        ('¨', 0, &["X.X"]),
        ('¸', 0, &[".X", "X."]),
        ('˚', 0, &[".X.", "X.X", ".X."]),
        ('C', 1, &[".XXX", "X...", "X...", "X...", ".XXX"]),
    ];
    let chars: Vec<char> = glyphs.iter().map(|g| g.0).collect();
    e.add_glyphs(&chars);
    for (ch, top, rows) in glyphs {
        e.go_to_glyph(ch);
        let (layer, frame) = (e.layer, e.frame);
        let pixels = rows
            .iter()
            .enumerate()
            .flat_map(|(dy, row)| {
                row.chars()
                    .enumerate()
                    .filter(|(_, c)| *c == 'X')
                    .map(move |(x, _)| (x as i32 + 1, top + dy as i32, cs_core::ONE_BIT_INK))
            })
            .collect();
        e.execute(Command::Batch(vec![
            Command::SetCel {
                layer,
                frame,
                cel: None,
            },
            Command::DrawPixels {
                layer,
                frame,
                pixels,
            },
        ]));
    }
    e.auto_build_accents(None, false);
    e.go_to_glyph('á');
    e.select_component(Some(1));
    e.tool = crate::ToolKind::Move;
}

#[cfg(test)]
mod tests {
    use cs_3d::{Primitive, RenderInput, TEMPLATE_SIZE, Texture, primitive_mesh, render_model};
    use cs_core::{CameraPreset, FIXED, Model3D, Object3D, Palette, Rgba, Transform3D};

    use super::*;

    /// Render iso de 64×64 de `objects` con la plantilla de textura de un modelo nuevo.
    fn render_iso(objects: Vec<Object3D>) -> Vec<Rgba> {
        let palette = Palette::db32();
        let tex = cs_commands::model::texture_template(&palette);
        let img = &tex.images[&tex.cels.values().next().unwrap().image];
        let (w, h) = TEMPLATE_SIZE;
        let pixels: Vec<Rgba> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| match img.get(x, y) {
                Some(PixelValue::Rgba(c)) => c,
                _ => Rgba::TRANSPARENT,
            })
            .collect();
        let model = Model3D {
            objects,
            camera: CameraPreset::Iso.camera(5 * FIXED, Default::default()),
            ..Default::default()
        };
        render_model(&RenderInput {
            model: &model,
            size: (64, 64),
            texture: Some(Texture {
                width: w,
                height: h,
                pixels: &pixels,
            }),
            palette: palette.colors(),
            camera: None,
            extra_yaw: 0,
            frame: None,
            time_ms: None,
        })
    }

    fn object(p: Primitive, transform: Transform3D) -> Object3D {
        Object3D {
            name: p.name().into(),
            mesh: primitive_mesh(&p),
            transform,
            double_sided: false,
            kind: Some(p.kind()),
            hidden: false,
            locked: false,
            anim: Vec::new(),
        }
    }

    #[test]
    fn el_techo_de_la_casita_sale_arriba_del_cubo() {
        let cube = object(Primitive::Cube, Transform3D::default());
        let roof = object(Primitive::Cone { sides: 4 }, house_roof());
        let alone = render_iso(vec![cube.clone()]);
        let house = render_iso(vec![cube, roof]);
        // El techo pinta pixeles arriba de la silueta del cubo solo.
        let extra = (0..alone.len())
            .filter(|&i| alone[i].a == 0 && house[i].a > 0)
            .count();
        assert!(extra > 10, "el techo agrega {extra} pixeles");
    }

    #[test]
    fn la_base_del_techo_queda_sobre_la_tapa_del_cubo() {
        let t = house_roof();
        // La base del cono (y = −1) escalada y trasladada cae en y = +1, la tapa del cubo.
        assert_eq!(t.position.y * 1000 - FIXED * t.scale.y, FIXED * 1000);
    }
}
