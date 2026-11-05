//! Extruir, dividir, unir y borrar lo elegido en el modo Edit (modo *Model* del workspace del
//! modelo): las teclas y los botones de la pestaña del objeto llegan aquí. Dividir, unir y borrar
//! son un paso de undo cada uno; extruir sigue al puntero hasta confirmar y extruir + mover es un
//! solo paso (cancelar no deja nada).

use cs_3d::{Item, P3, SelectMode, Selection};
use cs_core::Model3D;

use super::{CadMode, EditOp, Editor, GizmoView, KeyOp};

/// Un Extrude en curso: el modelo ya extruido (las caras en su lugar, sin mover) y la selección
/// de antes de extruir.
#[derive(Clone, Debug)]
pub struct Extrusion {
    pub model: Model3D,
    pub before: Selection,
}

/// La normal promedio (del mundo, unitaria; 0 si no hay) de las caras elegidas de `model`.
fn average_normal(model: &Model3D, sel: &Selection) -> P3 {
    let mut sum = [0.0_f32; 3];
    for item in &sel.items {
        let Item::Face(o, f) = *item else {
            continue;
        };
        let Some(obj) = model.objects.get(o) else {
            continue;
        };
        let Some(face) = obj.mesh.faces.get(f) else {
            continue;
        };
        let world = cs_3d::world_points(model, o);
        let Some(pts) = face
            .verts
            .iter()
            .map(|&v| world.get(v as usize).copied())
            .collect::<Option<Vec<P3>>>()
        else {
            continue;
        };
        // Newell, como el rasterizador: hacia afuera (una escala negativa en un número impar
        // de ejes invierte el sentido de las caras).
        let mut n = [0.0_f32; 3];
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let mirrored = model.object_chain(o).iter().fold(false, |m, t| {
            let s = t.scale;
            m ^ (s.x < 0) ^ (s.y < 0) ^ (s.z < 0)
        });
        if mirrored {
            n = n.map(|v| -v);
        }
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 0.0 {
            for i in 0..3 {
                sum[i] += n[i] / len;
            }
        }
    }
    let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    if len < 1e-3 {
        return [0.0; 3];
    }
    sum.map(|v| v / len)
}

impl Editor {
    /// El modelo, si se puede cambiar su topología: en modo *Model* del workspace y sin gesto en
    /// curso.
    pub(super) fn topology_model(&self) -> Option<&Model3D> {
        let e = &self.model_edit;
        if !self.model_edit_on()
            || e.mode != CadMode::Model
            || e.op.is_some()
            || e.uv_op.is_some()
            || e.key_request.is_some()
        {
            return None;
        }
        self.edited_model()
    }

    /// Lo elegido que se puede editar: sin lo de los objetos ocultos o bloqueados (ellos o sus
    /// grupos), como lo que mueve el gizmo.
    fn editable_selection(&self) -> Selection {
        let mut sel = self.model_edit.selection.clone();
        if let Some(m) = self.edited_model() {
            sel.items.retain(|it| cs_3d::pickable(m, it.object()));
        }
        sel
    }

    /// Si lo elegido involucra caras (en modo Object no: el objeto entero no se extruye ni se
    /// divide desde aquí).
    fn faces_involved(&self) -> bool {
        let Some(m) = self.topology_model() else {
            return false;
        };
        let sel = &self.editable_selection();
        match sel.mode {
            SelectMode::Object => false,
            SelectMode::Face => sel.items.iter().any(|i| matches!(i, Item::Face(..))),
            _ => !sel.converted(m, SelectMode::Face).is_empty(),
        }
    }

    pub fn can_extrude(&self) -> bool {
        self.faces_involved()
    }

    pub fn can_subdivide(&self) -> bool {
        self.faces_involved()
    }

    /// Unir: 2 o más vértices involucrados de un mismo objeto (no se une entre objetos), y no en
    /// modo Object.
    pub fn can_merge(&self) -> bool {
        let Some(m) = self.topology_model() else {
            return false;
        };
        let sel = &self.editable_selection();
        sel.mode != SelectMode::Object && sel.vertices(m).values().any(|vs| vs.len() >= 2)
    }

    /// Borrar: hay algo elegido que se puede editar (en modo Object, los objetos).
    pub fn can_delete_model_selection(&self) -> bool {
        self.topology_model().is_some() && !self.editable_selection().is_empty()
    }

    /// Ctrl+E o el botón Extrude: el lienzo empieza el Extrude en la vista bajo el puntero (o en
    /// la última usada), como G.
    pub fn request_extrude(&mut self) {
        if self.can_extrude() {
            self.model_edit.key_request = Some(KeyOp::Extrude);
        }
    }

    /// Extruye lo elegido y empieza a moverlo con el puntero desde `from` en `view` (ver
    /// [`EditOp::Extrude`]). El documento no cambia hasta confirmar.
    pub fn start_extrude(&mut self, view: GizmoView, from: (f32, f32)) -> bool {
        if !self.can_extrude() {
            return false;
        }
        let editable = self.editable_selection();
        let Some(m) = self.edited_model() else {
            return false;
        };
        let before = self.model_edit.selection.clone();
        let (model, sel) = cs_3d::extrude_faces(m, &editable);
        let Some(b) = cs_3d::bounds(&model, &sel) else {
            return false;
        };
        let normal = average_normal(&model, &sel);
        self.start_model_op(EditOp::Extrude {
            view,
            from,
            center: cs_3d::center(b),
            normal,
        });
        // (`set_model_selection` ya no cambia nada a media extrusión.)
        if let Some(o) = sel.first_object() {
            self.model_object = Some(o);
            self.model_group = None;
        }
        self.model_edit.selection = sel;
        // Lo extruido se ve desde ya, aunque el puntero no se haya movido.
        self.model_edit.preview = Some(model.clone());
        self.model_edit.extrusion = Some(Extrusion { model, before });
        true
    }

    /// Confirma el Extrude en curso (lo de la vista previa): un paso de undo. Lo llama
    /// `finish_model_op`, que ya quitó el gesto.
    pub(super) fn finish_extrude(&mut self) {
        let Some(x) = self.model_edit.extrusion.take() else {
            return;
        };
        let model = self.model_edit.preview.take().unwrap_or(x.model);
        let sel = self.model_edit.selection.clone();
        if self.named("Extrude", |e| e.set_model(model)) {
            self.set_model_selection(sel);
        } else {
            self.model_edit.selection = x.before;
        }
    }

    /// Extruye lo elegido y lo mueve `d` (en el mundo) de una vez: un paso de undo (la demo).
    pub fn extrude_moved(&mut self, d: P3) -> bool {
        if !self.can_extrude() {
            return false;
        }
        self.apply_topology("Extrude", |m, sel| {
            let (m, sel) = cs_3d::extrude_faces(m, sel);
            (cs_3d::translate(&m, &sel, d), sel)
        })
    }

    /// Ctrl+D o el botón Subdivide: las caras elegidas en 4. Un paso de undo.
    pub fn subdivide_model_selection(&mut self) -> bool {
        self.can_subdivide() && self.apply_topology("Subdivide", cs_3d::subdivide_faces)
    }

    /// M o el botón Merge: los vértices involucrados de cada objeto en uno. Un paso de undo.
    pub fn merge_model_selection(&mut self) -> bool {
        self.can_merge() && self.apply_topology("Merge", cs_3d::merge_selection)
    }

    /// Supr, Retroceso o el botón Delete: lo elegido (en modo Object, los objetos). Un paso de
    /// undo.
    pub fn delete_model_selection(&mut self) -> bool {
        self.can_delete_model_selection() && self.apply_topology("Delete", cs_3d::delete_selection)
    }

    /// El modelo y la selección que da `op` con lo elegido que se puede editar, en un paso de
    /// undo `label`. La selección nueva va antes del paso (el paso revalida la selección contra
    /// el modelo nuevo).
    fn apply_topology(
        &mut self,
        label: &'static str,
        op: impl FnOnce(&Model3D, &Selection) -> (Model3D, Selection),
    ) -> bool {
        let editable = self.editable_selection();
        let Some(m) = self.edited_model() else {
            return false;
        };
        let (model, sel) = op(m, &editable);
        // En modo Face lo que queda elegido son caras completas (Subdivide desde Vertex o
        // Edge deja solo los pedazos de una parte).
        let sel = cs_3d::expand_parts(&model, &sel);
        let left = model.objects.len();
        let old = std::mem::replace(&mut self.model_edit.selection, sel.clone());
        if !self.named(label, |e| e.set_model(model)) {
            self.model_edit.selection = old;
            return false;
        }
        // Deshacer el paso vuelve a lo elegido de antes (no a lo que ya se puso para él).
        if let Some(Some(step)) = self.step_selections.last_mut() {
            step.set_before(old);
        }
        self.set_model_selection(sel);
        // Si se borraron objetos, el del panel queda dentro de lo que quedó.
        self.model_object = self
            .model_object
            .map(|i| i.min(left.saturating_sub(1)))
            .filter(|_| left > 0);
        true
    }
}
