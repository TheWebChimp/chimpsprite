//! Los paneles de la derecha que editan el documento: artboards, slices (y sus propiedades)
//! y objetos.

use super::*;

/// Lista de artboards: activar, crear, renombrar y eliminar.
/// Devuelve `true` si se pidió abrir el diálogo de tamaño.
pub fn artboards(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
) -> bool {
    let mut resize = false;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.heading("Artboards");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_remove = editor.can_remove_active_artboard();
            if ui
                .add_enabled(can_remove, egui::Button::new("🗑").small())
                .tip("Delete artboard")
                .clicked()
            {
                editor.remove_active_artboard();
            }
            if ui.small_button("➕").tip("New 64×64 artboard").clicked() {
                editor.add_artboard(64, 64);
            }
        });
    });
    ui.separator();

    let ids: Vec<(ArtboardId, String)> = editor
        .document
        .canvas_artboards()
        .rev()
        .map(|a| (a.id, a.name.clone()))
        .collect();
    for (id, name) in ids {
        if ui.selectable_label(id == editor.active, name).clicked() {
            editor.activate(id);
        }
    }

    ui.add_space(4.0);
    if name_buffer.0 != editor.active {
        *name_buffer = (editor.active, editor.artboard().name.clone());
    }
    let response = ui.add(egui::TextEdit::singleline(&mut name_buffer.1).hint_text("Name"));
    if response.lost_focus() {
        let name = name_buffer.1.trim().to_string();
        if name.is_empty() {
            name_buffer.1 = editor.artboard().name.clone();
        } else {
            editor.rename_active_artboard(name);
        }
    } else if !response.has_focus() {
        name_buffer.1 = editor.artboard().name.clone();
    }
    if ui.button("Canvas Size").clicked() {
        resize = true;
    }
    resize
}

/// Con Pixel UI, la sección Artboards del diseño: el título con + y tacho, los artboards como
/// pestañas (sin marca) y, debajo, el nombre del activo y el botón de Canvas Size. Devuelve
/// `true` si se pidió abrir el diálogo de tamaño.
pub fn pixel_artboards(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
    skin: &Skin,
) -> bool {
    let can_remove = editor.can_remove_active_artboard();
    match pixel_sections::header(
        ui,
        skin,
        "Artboards",
        "New 64×64 artboard",
        "Delete artboard",
        can_remove,
    ) {
        Some(HeaderAction::Add) => editor.add_artboard(64, 64),
        Some(HeaderAction::Delete) => editor.remove_active_artboard(),
        None => {}
    }
    let ids: Vec<ArtboardId> = editor.document.canvas_artboards().map(|a| a.id).collect();
    let tabs: Vec<pixel_sections::Tab> = editor
        .document
        .canvas_artboards()
        .map(|a| pixel_sections::Tab {
            name: a.name.clone(),
            color: None,
        })
        .collect();
    let active = ids.iter().position(|id| *id == editor.active);
    if let Some(i) = pixel_sections::tabs(ui, skin, &tabs, active) {
        editor.activate(ids[i]);
    }
    if name_buffer.0 != editor.active {
        *name_buffer = (editor.active, editor.artboard().name.clone());
    }
    let (response, resize) = pixel_sections::field_row(
        ui,
        |ui, width| {
            crate::kit::input::text_input(
                ui,
                skin,
                &mut name_buffer.1,
                width,
                "Artboard name",
                "Name",
            )
        },
        |ui| {
            pixel_sections::icon_button(
                ui,
                skin,
                pixel_sections::ICON_CANVAS_SIZE,
                "Canvas Size",
                true,
            )
            .clicked()
        },
    );
    if response.lost_focus() {
        let name = name_buffer.1.trim().to_string();
        if name.is_empty() {
            name_buffer.1 = editor.artboard().name.clone();
        } else {
            editor.rename_active_artboard(name);
        }
    } else if !response.has_focus() {
        name_buffer.1 = editor.artboard().name.clone();
    }
    resize
}

/// Un campo del diseño que edita `current`: mientras tiene el foco guarda lo escrito (en la
/// memoria de egui, con `id`); al perderlo devuelve el texto si cambió (Esc lo descarta).
/// Un campo de una línea (`widgets::text_line`) que edita una copia de `current`: lo escrito se
/// guarda mientras tiene el foco y se devuelve al terminar (Enter o al salir) si cambió; Escape
/// lo descarta.
fn buffered_line(ui: &mut egui::Ui, id: egui::Id, current: &str, label: &str) -> Option<String> {
    crate::kit::input::buffered(ui, id, current, |ui, text| {
        crate::widgets::text_line_named(ui, text, label)
    })
}

fn pixel_buffered_field(
    ui: &mut egui::Ui,
    skin: &Skin,
    id: egui::Id,
    current: &str,
    width: f32,
    label: &str,
    hint: &str,
) -> Option<String> {
    crate::kit::input::buffered(ui, id, current, |ui, text| {
        crate::kit::input::text_input(ui, skin, text, width, label, hint)
    })
}

/// Con Pixel UI, la sección Slices como la de Artboards: el título con + (un slice del tamaño de
/// la selección o del lienzo) y tacho, los slices como pestañas con su color y, del elegido, el
/// nombre con el botón de color, los datos y la clave del frame actual.
pub fn pixel_slices(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin) {
    let selected = editor.slice;
    match pixel_sections::header(
        ui,
        skin,
        "Slices",
        "New slice",
        "Delete slice",
        selected.is_some(),
    ) {
        Some(HeaderAction::Add) => editor.add_slice(),
        Some(HeaderAction::Delete) => editor.delete_slice(),
        None => {}
    }
    let tabs: Vec<pixel_sections::Tab> = editor
        .sprite()
        .slices
        .iter()
        .map(|s| pixel_sections::Tab {
            name: s.name.clone(),
            color: Some(Color32::from_rgb(s.color.r, s.color.g, s.color.b)),
        })
        .collect();
    if tabs.is_empty() {
        pixel_sections::note(
            ui,
            skin,
            "No slices. Use the Slice tool (Shift+C) to mark hitboxes and pivots.",
        );
        return;
    }
    if let Some(i) = pixel_sections::tabs(ui, skin, &tabs, editor.slice) {
        editor.end_group();
        editor.slice = Some(i);
        editor.set_tool(ToolKind::Slice);
    }
    let Some(index) = editor.slice else {
        return;
    };
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    let base = ui.id().with(("pixel_slice", index));
    let (name, color_button) = pixel_sections::field_row(
        ui,
        |ui, width| {
            pixel_buffered_field(
                ui,
                skin,
                base.with("name"),
                &slice.name,
                width,
                "Slice name",
                "Name",
            )
        },
        |ui| {
            let c = slice.color;
            pixel_sections::color_button(ui, skin, Color32::from_rgb(c.r, c.g, c.b), "Slice color")
        },
    );
    if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
        editor.update_slice(|s| s.name = name.trim().to_owned());
    }
    // El color: el selector de la columna derecha, en el panel de los menús, colgado del botón.
    let look = menus::MenuLook::new(skin, color_button.rect.top());
    let current = cs_core::Rgba::rgb(slice.color.r, slice.color.g, slice.color.b);
    let picked = menus::dropdown(&look, &color_button, color_button.rect, |ui| {
        ui.set_width(SLICE_PICKER_W);
        let id = ui.id().with("slice_color");
        crate::color_panel::picker(ui, skin, id, current)
    });
    if let Some(color) = picked.filter(|c| *c != current) {
        let rgb = cs_core::Rgba::rgb(color.r, color.g, color.b);
        editor.update_slice(|s| s.color = rgb);
    }
    let data = pixel_sections::field_alone(ui, |ui, width| {
        pixel_buffered_field(
            ui,
            skin,
            base.with("data"),
            &slice.text,
            width,
            "Slice data",
            "Data (e.g. damage=3)",
        )
    });
    if let Some(text) = data {
        editor.update_slice(|s| s.text = text);
    }
    slice_key_properties(ui, editor);
}

/// El ancho del selector de color de los slices.
const SLICE_PICKER_W: f32 = 180.0;

/// Gesto sobre campos numéricos o sliders: un solo paso de undo por arrastre o edición.
/// Llamar antes de aplicar el cambio; devuelve el grupo (id del widget que cambió) si cambió.
/// El gesto anterior del mismo widget ya terminó en `gesture_end` (al soltar o perder el foco),
/// así que acá solo se corta al tomar foco; cortar en `drag_started` partiría en dos el
/// "clic (el slider salta) + arrastre".
pub(crate) fn gesture_group(editor: &mut Editor, responses: &[&egui::Response]) -> Option<u64> {
    if responses.iter().any(|r| r.gained_focus()) {
        editor.end_group();
    }
    responses.iter().find(|r| r.changed()).map(|r| r.id.value())
}

/// Llamar después de aplicar: cierra el gesto si terminó (al soltar el botón, al perder el foco,
/// o si el cambio no vino de un botón apretado ni de un campo con foco).
pub(crate) fn gesture_end(editor: &mut Editor, responses: &[&egui::Response]) {
    let ended = responses.iter().any(|r| {
        r.drag_stopped()
            || r.clicked()
            || r.lost_focus()
            || (r.changed() && !r.is_pointer_button_down_on() && !r.has_focus())
    });
    if ended {
        editor.end_group();
    }
}

/// Rango de los campos de posición del objeto.
const POSITION_RANGE: std::ops::RangeInclusive<i32> = -16384..=16384;

/// Propiedades del objeto actual (ver `Editor::current_object`) en el frame actual. Con varios
/// seleccionados, el campo que cambia se aplica a todos (un paso de undo). Devuelve `true` si se
/// entró al símbolo (el lienzo debe centrarlo).
/// En otras capas no muestra nada (el lugar es de la paleta, ver `palette_and_editor`).
pub fn instance(ui: &mut egui::Ui, editor: &mut Editor) -> bool {
    if !editor.on_object_layer() {
        return false;
    }
    ui.heading("Object");
    let Some(key) = editor.instance_key() else {
        // Sin objeto (con varios en la capa y ninguno seleccionado), o uno que todavía no empieza.
        let hint = if editor.current_object().is_none() {
            "Select an object on the canvas"
        } else {
            "No keyframe at or before this frame"
        };
        // Con wrap: no ensancha el panel.
        let hint = egui::RichText::new(hint).weak();
        ui.add(egui::Label::new(hint).wrap());
        ui.separator();
        return false;
    };
    let count = editor.edited_objects().len();
    if count > 1 {
        ui.weak(format!("{count} objects selected"));
    }
    // El nombre es solo del objeto actual (renombrar varios a la vez los dejaría iguales).
    let mut name = editor.current_object_name().unwrap_or_default().to_string();
    // La etiqueta primero: el campo toma lo que queda (después de él, ensancharía el panel).
    let r = ui
        .horizontal(|ui| {
            ui.label("Name");
            widgets::text_line_named(ui, &mut name, "Object name")
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.rename_current_object(&name, Some(group));
    }
    gesture_end(editor, &[&r]);
    let is_key = editor.is_instance_keyframe(editor.layer, editor.frame);
    if !is_key {
        ui.weak("Changes create a keyframe on this frame.");
    }
    let names: Vec<(cs_core::ArtboardId, String)> = editor.instance_candidates();
    let current = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or("(missing)".to_string(), |a| a.name.clone());
    ui.horizontal(|ui| {
        let mut source = key.source;
        widgets::combo(ui, "instance_source", &current, |ui| {
            for (id, name) in names {
                widgets::choice(ui, &mut source, Some(id), &name);
            }
        });
        ui.label("Source");
        if source != key.source {
            editor.update_instance_key(|k| k.source = source);
        }
    });
    let (mut x, mut y) = key.position;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("Position");
            let rx = widgets::number(ui, &mut x, POSITION_RANGE, "");
            let ry = widgets::number(ui, &mut y, POSITION_RANGE, "");
            (rx, ry)
        })
        .inner;
    // Solo la coordenada que cambió (con varios objetos, la otra queda la de cada uno).
    if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
        let (cx, cy) = (x != key.position.0, y != key.position.1);
        editor.update_instance_key_grouped(Some(group), |k| {
            if cx {
                k.position.0 = x;
            }
            if cy {
                k.position.1 = y;
            }
        });
    }
    gesture_end(editor, &[&rx, &ry]);
    transform_rows(ui, editor, &key);
    let (mut fh, mut fv) = (key.flip_h, key.flip_v);
    ui.horizontal(|ui| {
        if crate::widgets::checkbox(ui, &mut fh, "Flip H").changed() {
            editor.update_instance_key(|k| k.flip_h = fh);
        }
        if crate::widgets::checkbox(ui, &mut fv, "Flip V").changed() {
            editor.update_instance_key(|k| k.flip_v = fv);
        }
    });
    let mut opacity = key.opacity;
    let r = ui
        .horizontal(|ui| {
            ui.label("Opacity");
            widgets::number(ui, &mut opacity, 0..=255, "")
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
    }
    gesture_end(editor, &[&r]);
    use cs_core::{GraphicPlay, Timing};
    let (mut first, mut mode) = match key.timing {
        Timing::Graphic { first_frame, play } => (first_frame, Some(play)),
        Timing::MovieClip { first_frame } => (first_frame, None),
    };
    let before = (first, mode);
    // El combo del diseño con los cuatro modos.
    let modes = [
        (Some(GraphicPlay::Loop), "Loop"),
        (Some(GraphicPlay::Once), "Once"),
        (Some(GraphicPlay::Single), "Single"),
        (None, "Movie clip"),
    ];
    let shown = modes
        .iter()
        .find(|(m, _)| *m == mode)
        .map_or("Loop", |(_, name)| *name);
    ui.horizontal(|ui| {
        ui.label("Play");
        widgets::combo(ui, "instance_play", shown, |ui| {
            for (value, name) in modes {
                widgets::choice(ui, &mut mode, value, name);
            }
        })
        .tip(
            "Loop, Once and Single follow this artboard's frames; Movie clip plays on its own              clock with its own frame durations",
        );
    });
    let frames = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or(1, |a| a.sprite.frames.len());
    let first_response = ui
        .horizontal(|ui| {
            ui.label("First frame");
            let mut shown = first + 1;
            let r = widgets::number(ui, &mut shown, 1..=frames, "");
            if r.changed() {
                first = shown - 1;
            }
            r
        })
        .inner;
    // El modo es un clic (paso propio); el primer frame se arrastra (un paso por gesto).
    let group = gesture_group(editor, &[&first_response]).filter(|_| mode == before.1);
    if (first, mode) != before {
        // Solo lo que cambió: el modo o el primer frame (con varios objetos, el resto queda el
        // de cada uno).
        let (new_first, new_mode) = (first != before.0, mode != before.1);
        editor.update_instance_key_grouped(group, |k| {
            let (old_first, old_mode) = match k.timing {
                Timing::Graphic { first_frame, play } => (first_frame, Some(play)),
                Timing::MovieClip { first_frame } => (first_frame, None),
            };
            let first_frame = if new_first { first } else { old_first };
            k.timing = match if new_mode { mode } else { old_mode } {
                Some(play) => Timing::Graphic { first_frame, play },
                None => Timing::MovieClip { first_frame },
            }
        });
    }
    gesture_end(editor, &[&first_response]);
    tween_rows(ui, editor);
    let entered = crate::pixel_ui::text_button(ui, "Edit Symbol")
        .tip("Edit the source symbol (double-click with Move)")
        .clicked()
        && editor.enter_instance();
    ui.separator();
    entered
}

/// Escala, rotación y tinte de `key` (lo que se ve del objeto actual en este frame): cada campo
/// cambia solo lo suyo en los objetos que se editan (un paso de undo por gesto).
fn transform_rows(ui: &mut egui::Ui, editor: &mut Editor, key: &cs_core::InstanceKey) {
    // La key guarda milésimas; el panel muestra porcentajes enteros.
    let percent = |milli: i32| (milli as f32 / 10.0).round() as i32;
    let (mut sx, mut sy) = (percent(key.scale.0), percent(key.scale.1));
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("Scale %");
            let rx = widgets::number(ui, &mut sx, 1..=2000, "");
            let ry = widgets::number(ui, &mut sy, 1..=2000, "");
            (rx, ry)
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
        let (cx, cy) = (sx != percent(key.scale.0), sy != percent(key.scale.1));
        editor.update_instance_key_grouped(Some(group), |k| {
            if cx {
                k.scale.0 = sx * 10;
            }
            if cy {
                k.scale.1 = sy * 10;
            }
        });
    }
    gesture_end(editor, &[&rx, &ry]);
    let mut rotation = key.rotation;
    let r = ui
        .horizontal(|ui| {
            ui.label("Rotation");
            widgets::number(ui, &mut rotation, -3600..=3600, "")
                .tip("Degrees, clockwise. 720 in the end keyframe of a tween spins twice")
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_instance_key_grouped(Some(group), |k| k.rotation = rotation);
    }
    gesture_end(editor, &[&r]);
    // El tinte: el color (opaco) y cuánto se mezcla (0 = nada; el color se conserva).
    let tint = key.tint.unwrap_or(cs_core::Tint {
        color: cs_core::Rgba::rgb(255, 255, 255),
        amount: 0,
    });
    let mut color = cs_core::Rgba {
        a: 255,
        ..tint.color
    };
    let mut amount = tint.amount;
    let r = ui
        .horizontal(|ui| {
            ui.label("Tint");
            if widgets::color_edit(ui, &mut color, false, "Tint color").changed() {
                // Elegir un color sin cantidad lo deja entero (como el Tint de Flash).
                editor.update_instance_key(|k| {
                    let amount = k.tint.map_or(0, |t| t.amount);
                    let amount = if amount == 0 { 255 } else { amount };
                    k.tint = Some(cs_core::Tint { color, amount });
                });
            }
            widgets::number(ui, &mut amount, 0..=255, "")
                .tip("How much of the tint color: 0 is none, 255 is all")
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_instance_key_grouped(Some(group), |k| {
            let color = k.tint.map_or(color, |t| t.color);
            k.tint = Some(cs_core::Tint { color, amount });
        });
    }
    gesture_end(editor, &[&r]);
}

/// Las curvas del Ease (además de Classic), con su nombre en el select.
const EASE_CURVES: [(cs_core::Curve, &str); 7] = [
    (cs_core::Curve::EaseIn, "Ease In"),
    (cs_core::Curve::EaseOut, "Ease Out"),
    (cs_core::Curve::EaseInOut, "Ease In Out"),
    (cs_core::Curve::Bounce, "Bounce"),
    (cs_core::Curve::Elastic, "Elastic"),
    (cs_core::Curve::Back, "Back"),
    (cs_core::Curve::Step, "Step"),
];

/// El tween del tramo actual (Ease y Amount, Remove Tween) o, si se puede, Create Tween.
fn tween_rows(ui: &mut egui::Ui, editor: &mut Editor) {
    use cs_core::Ease;
    let Some((tween, span)) = editor.current_tween() else {
        if editor.can_create_current_tween()
            && crate::pixel_ui::text_button(ui, "Create Tween")
                .tip("Tween from this keyframe to the next one")
                .clicked()
        {
            editor.create_current_tween();
        }
        return;
    };
    // `None` = Classic.
    let (mut curve, mut amount): (Option<cs_core::Curve>, i32) = match tween.ease {
        Ease::Classic(e) => (None, e as i32),
        Ease::Curve { curve, amount } => (Some(curve), amount as i32),
    };
    let before = curve;
    let shown = EASE_CURVES
        .iter()
        .find(|(c, _)| Some(*c) == curve)
        .map_or("Classic", |(_, name)| *name);
    ui.horizontal(|ui| {
        ui.label("Ease");
        widgets::combo(ui, "tween_ease", shown, |ui| {
            widgets::choice(ui, &mut curve, None, "Classic");
            for (c, name) in EASE_CURVES {
                widgets::choice(ui, &mut curve, Some(c), name);
            }
        })
        .tip("Classic is Flash's ease: -100 speeds up, 100 slows down");
    });
    if curve != before {
        // Classic empieza lineal; una curva, entera.
        editor.update_current_tween(None, |t| {
            t.ease = match curve {
                None => Ease::Classic(0),
                Some(curve) => Ease::Curve { curve, amount: 100 },
            }
        });
    }
    let range = if curve.is_none() { -100..=100 } else { 0..=100 };
    let r = ui
        .horizontal(|ui| {
            ui.label("Amount");
            widgets::number(ui, &mut amount, range, "")
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_current_tween(Some(group), |t| {
            t.ease = match t.ease {
                Ease::Classic(_) => Ease::Classic(amount.clamp(-100, 100) as i8),
                Ease::Curve { curve, .. } => Ease::Curve {
                    curve,
                    amount: amount.clamp(0, 100) as u8,
                },
            }
        });
    }
    gesture_end(editor, &[&r]);
    if span.broken {
        let hint = egui::RichText::new("Tween needs a next keyframe with the same symbol").weak();
        ui.add(egui::Label::new(hint).wrap());
    }
    if crate::pixel_ui::text_button(ui, "Remove Tween").clicked() {
        editor.remove_current_tween();
    }
}

/// Panel de slices: lista, nombre, color, datos y la clave del frame actual (rect y pivote).
pub fn slices(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.horizontal(|ui| {
        ui.heading("Slices");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(editor.slice.is_some(), egui::Button::new("🗑").small())
                .tip("Delete slice")
                .clicked()
            {
                editor.delete_slice();
            }
        });
    });
    ui.separator();
    let names: Vec<(String, cs_core::Rgba)> = editor
        .sprite()
        .slices
        .iter()
        .map(|s| (s.name.clone(), s.color))
        .collect();
    if names.is_empty() {
        ui.weak("No slices. Use the Slice tool (Shift+C) to mark hitboxes and pivots.");
        return;
    }
    for (i, (name, color)) in names.iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 2.0, Color32::from_rgb(color.r, color.g, color.b));
            if ui.selectable_label(editor.slice == Some(i), name).clicked() {
                editor.end_group();
                editor.slice = Some(i);
                editor.set_tool(ToolKind::Slice);
            }
        });
    }
    if editor.slice.is_some() {
        ui.separator();
        slice_properties(ui, editor);
    }
}

/// Nombre, color, datos y clave del frame actual del slice seleccionado (panel y diálogo).
pub fn slice_properties(ui: &mut egui::Ui, editor: &mut Editor) {
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    // El nombre y los datos se guardan mientras se escriben (si no, cada frame volverían a ser
    // los del slice) y se aplican al terminar.
    let base = ui.id().with(("slice_props", editor.slice));
    ui.horizontal(|ui| {
        ui.label("Name");
        if let Some(name) = buffered_line(ui, base.with("name"), &slice.name, "Slice name")
            .filter(|n| !n.trim().is_empty())
        {
            editor.update_slice(|s| s.name = name.trim().to_owned());
        }
    });
    let mut color = cs_core::Rgba {
        a: 255,
        ..slice.color
    };
    ui.horizontal(|ui| {
        ui.label("Color");
        if crate::widgets::color_edit(ui, &mut color, false, "Slice color").changed() {
            editor.update_slice(|s| s.color = color);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Data");
        if let Some(text) = buffered_line(ui, base.with("data"), &slice.text, "Slice data") {
            editor.update_slice(|s| s.text = text);
        }
    });
    slice_key_properties(ui, editor);
}

/// La clave del frame actual del slice seleccionado: rect, pivote, 9-slice y ocultar.
fn slice_key_properties(ui: &mut egui::Ui, editor: &mut Editor) {
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    let frame = editor.frame;
    let Some(key) = slice.key_at(frame).copied() else {
        ui.weak(format!("Hidden on frame {}", frame + 1));
        return;
    };
    let own_key = key.frame == frame;
    ui.label(if own_key {
        format!("Frame {} (own key)", frame + 1)
    } else {
        format!("Frame {} (from frame {})", frame + 1, key.frame + 1)
    });
    let (mut x, mut y, mut w, mut h) = key.bounds;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("X");
            let rx = widgets::number(ui, &mut x, -4096..=4096, "");
            ui.label("Y");
            let ry = widgets::number(ui, &mut y, -4096..=4096, "");
            (rx, ry)
        })
        .inner;
    let (rw, rh) = ui
        .horizontal(|ui| {
            ui.label("W");
            let rw = widgets::number(ui, &mut w, 1..=4096, "");
            ui.label("H");
            let rh = widgets::number(ui, &mut h, 1..=4096, "");
            (rw, rh)
        })
        .inner;
    let rect_fields = [&rx, &ry, &rw, &rh];
    if let Some(group) = gesture_group(editor, &rect_fields) {
        editor.update_slice_key_grouped(Some(group), |k| k.bounds = (x, y, w, h));
    }
    gesture_end(editor, &rect_fields);
    let mut has_pivot = key.pivot.is_some();
    if crate::widgets::checkbox(ui, &mut has_pivot, "Pivot").changed() {
        let default = (w as i32 / 2, h as i32);
        editor.update_slice_key(|k| k.pivot = has_pivot.then_some(default));
    }
    if let Some((mut px, mut py)) = key.pivot {
        let (rx, ry) = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut px, -4096..=4096, "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut py, -4096..=4096, "");
                (rx, ry)
            })
            .inner;
        if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
            editor.update_slice_key_grouped(Some(group), |k| k.pivot = Some((px, py)));
        }
        gesture_end(editor, &[&rx, &ry]);
    }
    // 9-slice: el rect central (relativo al slice) se estira; las esquinas quedan fijas.
    let mut nine = key.center.is_some();
    if crate::widgets::checkbox(ui, &mut nine, "9-Slice")
        .tip("Center area that stretches; corners keep their size")
        .changed()
    {
        editor
            .update_slice_key(|k| k.center = nine.then(|| crate::slice_ops::default_center(w, h)));
    }
    if let Some((mut cx, mut cy, mut cw, mut ch)) = key.center {
        let fields = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut cx, 0..=(w as i32 - 1).max(0), "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut cy, 0..=(h as i32 - 1).max(0), "");
                [rx, ry]
            })
            .inner;
        let sizes = ui
            .horizontal(|ui| {
                ui.label("W");
                let rw = widgets::number(ui, &mut cw, 1..=w.max(1), "");
                ui.label("H");
                let rh = widgets::number(ui, &mut ch, 1..=h.max(1), "");
                [rw, rh]
            })
            .inner;
        let all = [&fields[0], &fields[1], &sizes[0], &sizes[1]];
        if let Some(group) = gesture_group(editor, &all) {
            // El centro no puede salirse del slice.
            let cw = cw.min(w.saturating_sub(cx as u32)).max(1);
            let ch = ch.min(h.saturating_sub(cy as u32)).max(1);
            editor.update_slice_key_grouped(Some(group), |k| k.center = Some((cx, cy, cw, ch)));
        }
        gesture_end(editor, &all);
    }
    if own_key
        && slice.keys.first().is_some_and(|k| k.frame != frame)
        && crate::pixel_ui::text_button(ui, "Remove key on this frame").clicked()
    {
        editor.remove_slice_key();
    }
    let mut hide = false;
    if crate::pixel_ui::text_button(ui, "Hide from this frame")
        .tip("Adds an empty key: the slice disappears from here on")
        .clicked()
    {
        hide = true;
    }
    if hide {
        editor.update_slice(|s| {
            s.set_key(cs_core::SliceKey {
                frame,
                bounds: (x, y, 0, 0),
                center: None,
                pivot: None,
            })
        });
    }
}
