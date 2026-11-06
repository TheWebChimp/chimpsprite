//! La barrita del objeto de ChimpCAD: una fila fija debajo del encabezado, del alto de la barra
//! de opciones y con su separador grabado, con la posición (en 1/16), la rotación (en grados) y
//! la escala (en %) de la fila activa del outliner (un objeto o un grupo) en campos numéricos
//! compactos que se escriben o se arrastran. Con la fila bloqueada, sus valores deshabilitados;
//! sin fila activa o con otras partes elegidas en las vistas, los campos vacíos y deshabilitados.

use std::ops::RangeInclusive;

use cs_core::{Transform3D, Vec3i};
use eframe::egui::{self, Rect};

use crate::bar_fields::{self, BarLook};
use crate::editor::{Editor, POSITION_MAX, ROTATION_MAX, SCALE_MAX, wrap_angle};
use crate::panels::{CONTROL_GAP, GROUP_GAP, LABEL_FIELD_GAP, gap};

/// Lo que muestra cada grupo de la barrita: su título, de qué parte de la transform, cómo se ve
/// lo guardado (pasado antes por `show` y dividido entre `per`) y hasta dónde llega lo mostrado.
struct Part {
    title: &'static str,
    axes: fn(&mut Transform3D) -> &mut Vec3i,
    show: fn(i32) -> i32,
    per: i32,
    range: RangeInclusive<i32>,
}

fn position(t: &mut Transform3D) -> &mut Vec3i {
    &mut t.position
}

fn rotation(t: &mut Transform3D) -> &mut Vec3i {
    &mut t.rotation
}

fn scale(t: &mut Transform3D) -> &mut Vec3i {
    &mut t.scale
}

fn parts() -> [Part; 3] {
    [
        Part {
            title: "Position",
            axes: position,
            show: std::convert::identity,
            per: 1,
            range: -POSITION_MAX..=POSITION_MAX,
        },
        // En décimas de grado: se muestra en grados, dado la vuelta a ±180°.
        Part {
            title: "Rotation",
            axes: rotation,
            show: wrap_angle,
            per: 10,
            range: -ROTATION_MAX..=ROTATION_MAX,
        },
        // En milésimas: se muestra en %.
        Part {
            title: "Scale",
            axes: scale,
            show: std::convert::identity,
            per: 10,
            range: 1..=SCALE_MAX,
        },
    ]
}

/// La barrita en `rect` (debajo de la línea del encabezado): va después del fondo del
/// workspace, así queda encima. Todo lo cambiado en un frame (o en un arrastre) es un paso de
/// undo.
pub(super) fn object_bar(ui: &mut egui::Ui, rect: Rect, editor: &mut Editor) {
    let skin = crate::pixel_ui::frame_skin(ui.ctx());
    let skin = skin.as_deref();
    let builder = egui::UiBuilder::new()
        .max_rect(rect)
        .id_salt("cad_object_bar");
    let mut bar_ui = ui.new_child(builder);
    bar_ui.set_clip_rect(rect);
    crate::panels::bar_panel("cad_object_bar", bar_ui.style(), skin).show(&mut bar_ui, |ui| {
        crate::panels::bar_row(ui, skin, "cad_object_bar_scroll", |ui, look| {
            fields(ui, editor, look);
        });
    });
}

/// Los tres grupos (título y campos X, Y y Z) y lo que cambió, a la fila activa.
fn fields(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    let (current, editable) = match editor.active_transform() {
        Some((t, editable)) => (Some(t), editable),
        None => (None, false),
    };
    let mut next = current;
    ui.add_enabled_ui(editable, |ui| {
        for (k, part) in parts().iter().enumerate() {
            if k > 0 {
                match look {
                    Some(_) => gap(ui, GROUP_GAP),
                    None => {
                        ui.separator();
                    }
                }
            }
            match look {
                Some(look) => {
                    bar_fields::label(ui, look, part.title);
                    gap(ui, LABEL_FIELD_GAP);
                }
                None => {
                    ui.label(part.title);
                }
            }
            for (a, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
                if a > 0 && look.is_some() {
                    gap(ui, CONTROL_GAP);
                }
                let name = format!("{} {axis}", part.title);
                let value = next.as_mut().map(|t| {
                    let v = (part.axes)(t);
                    match a {
                        0 => &mut v.x,
                        1 => &mut v.y,
                        _ => &mut v.z,
                    }
                });
                axis_field(ui, look, (&name, axis), value, part);
            }
        }
    });
    // Un arrastre (o lo escrito en un campo) es un solo paso de undo: se juntan los cambios
    // mientras el botón siga apretado.
    let group = egui::Id::new("cad_object_bar_transform");
    if let Some(t) = next
        && editable
        && next != current
    {
        editor.set_active_transform(t, group);
    }
    if !ui.input(|input| input.pointer.any_down()) {
        editor.end_group_of(group.value());
    }
}

/// El campo de un eje (`name`, con la letra `axis`) de `part`: lo guardado en `value` se muestra
/// en la unidad de la barrita y solo se escribe si el usuario lo cambia (un valor fuera de
/// `part.range` no se recorta solo). Sin `value`, vacío y deshabilitado.
fn axis_field(
    ui: &mut egui::Ui,
    look: Option<BarLook>,
    (name, axis): (&str, &str),
    value: Option<&mut i32>,
    part: &Part,
) {
    let shown = value.as_deref().map(|v| (part.show)(*v) / part.per);
    let mut v = shown;
    match look {
        Some(look) => {
            bar_fields::axis_number(ui, look, name, axis, v.as_mut(), part.range.clone());
        }
        None => {
            ui.label(axis);
            match v.as_mut() {
                Some(v) => {
                    let drag = egui::DragValue::new(v)
                        .range(part.range.clone())
                        .clamp_existing_to_range(false);
                    ui.add(drag).on_hover_text(name);
                }
                None => {
                    ui.add_enabled(false, egui::Label::new("–"));
                }
            }
        }
    }
    if let (Some(value), Some(now)) = (value, v)
        && v != shown
    {
        *value = now * part.per;
    }
}
