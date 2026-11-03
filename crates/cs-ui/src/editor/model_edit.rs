//! El modo Edit del modelo 3D (picoCAD), siempre prendido en su workspace: la selección
//! (objetos, vértices, aristas o caras), las vistas ortográficas (centro y zoom compartidos), la
//! cuadrícula y los gestos de mover y escalar. Durante un gesto solo cambia la vista previa; al
//! terminar es un paso de undo.

use cs_3d::{GizmoKind, GizmoPart, Handle, OrthoView, P3, SelectMode, Selection, ViewMap};
use cs_core::{ArtboardId, Model3D};

use super::Editor;

/// La cuadrícula por defecto: 1/4 de unidad.
pub const DEFAULT_GRID: i32 = 4;
/// Las cuadrículas del panel, en 1/16 de unidad.
pub const GRIDS: [i32; 5] = [1, 2, 4, 8, 16];
/// Cuánto cambia el zoom de las vistas por paso de rueda.
const ZOOM_STEP: f32 = 1.25;
const ZOOM_MIN: f32 = 0.02;
const ZOOM_MAX: f32 = 64.0;
/// Cuánto (en puntos) hay que mover el puntero para que un arrastre con el mouse empiece a mover
/// o escalar: un clic sin arrastre no toca el modelo.
pub const DRAG_THRESHOLD: f32 = 3.0;
/// El tamaño del gizmo en pantalla (en puntos; no cambia con el zoom): el largo de sus ejes y el
/// radio de sus aros.
pub const GIZMO_PX: f32 = 32.0;
/// Los pasos del aro del gizmo, en grados (con Ctrl, de 1°).
const ROTATE_STEP: f32 = 15.0;

/// El gizmo de las vistas (como Blender): uno a la vez, o ninguno.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Gizmo {
    None,
    #[default]
    Move,
    Rotate,
    Scale,
}

impl Gizmo {
    pub fn kind(self) -> Option<GizmoKind> {
        match self {
            Gizmo::None => None,
            Gizmo::Move => Some(GizmoKind::Move),
            Gizmo::Rotate => Some(GizmoKind::Rotate),
            Gizmo::Scale => Some(GizmoKind::Scale),
        }
    }
}

/// Lo que pidió el teclado y empieza el lienzo (que sabe dónde está el puntero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOp {
    Grab,
    Scale,
}

/// Un gesto en curso en una vista ortográfica (puntos de pantalla).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditOp {
    /// Mover lo elegido; `grab` es el punto agarrado (el que cae en la cuadrícula). `keyboard`:
    /// empezó con G (sigue al puntero sin botón; clic o Enter confirma). Con el mouse no hay
    /// vista previa hasta que el puntero se aleja [`DRAG_THRESHOLD`] de `from`.
    Move {
        map: ViewMap,
        from: (f32, f32),
        grab: P3,
        keyboard: bool,
    },
    /// Arrastrar un asa de la caja de la selección (desde `from`, con el mismo umbral que Move).
    Handle {
        map: ViewMap,
        from: (f32, f32),
        handle: Handle,
        bounds: (P3, P3),
    },
    /// S: igual en los tres ejes desde `center`; el factor es la distancia del puntero al centro
    /// entre `from_dist`.
    Scale {
        map: ViewMap,
        center: P3,
        from_dist: f32,
    },
    /// Caja de selección (`add`: con Shift, agrega).
    Box {
        map: ViewMap,
        from: (f32, f32),
        to: (f32, f32),
        add: bool,
    },
    /// Arrastrar el asa `part` del gizmo (desde `from`, con el mismo umbral que Move) de
    /// `center` (el centro de la caja de lo elegido al empezar). Para los aros, `last` es el
    /// último punto del puntero y `turned` lo girado hasta ahí (grados, en el sentido positivo
    /// del eje).
    Gizmo {
        map: ViewMap,
        part: GizmoPart,
        from: (f32, f32),
        center: P3,
        last: (f32, f32),
        turned: f32,
    },
    /// Desplazar las vistas (botón derecho o central).
    Pan {
        map: ViewMap,
        from: (f32, f32),
        center: P3,
    },
}

/// Los dos modos de la pestaña de un modelo: las 4 vistas (*Model*) o el modelo arriba y su
/// textura abajo (*Texture*).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CadMode {
    #[default]
    Model,
    Texture,
}

/// El zoom (entero) y el desplazamiento de la textura de abajo en modo Texture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureView {
    pub zoom: u32,
    pub pan: (f32, f32),
}

/// Un gesto sobre las UVs de las caras elegidas, en pixeles de la textura (`f32`: el puntero).
/// Hasta soltar solo cambia la vista previa; al soltar es un paso de undo.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UvOp {
    /// Mover las UVs de las caras elegidas en pixeles enteros; `from` es el punto agarrado.
    Move { from: (f32, f32) },
    /// Mover solo el vértice UV `vertex` de la cara `face`; `from` es el punto agarrado.
    Vertex {
        face: (usize, usize),
        vertex: usize,
        from: (f32, f32),
    },
}

/// El estado del modo Edit (de la UI: no se guarda ni pasa por undo). Es del modelo de
/// `artboard`: al entrar al workspace de otro, empieza de cero.
#[derive(Clone, Debug)]
pub struct ModelEdit {
    pub artboard: Option<ArtboardId>,
    pub selection: Selection,
    pub grid: i32,
    pub center: P3,
    /// Puntos de pantalla por 1/16 de unidad en las vistas ortográficas.
    pub zoom: f32,
    /// Encuadrar el modelo en las vistas en cuanto el lienzo conozca el área (al entrar al
    /// workspace). Ver `fit_model_views`.
    pub fit: bool,
    pub last_view: OrthoView,
    pub preview: Option<Model3D>,
    pub op: Option<EditOp>,
    pub key_request: Option<KeyOp>,
    /// Lo que se presionó estando ya elegido: si el gesto de mover termina sin arrastre, queda
    /// como la única selección.
    pub click_item: Option<cs_3d::Item>,
    /// El modo de la pestaña (al abrir otro modelo, *Model*).
    pub mode: CadMode,
    /// La vista de la textura en modo Texture (`None`: encuadrarla cuando el lienzo sepa el área).
    pub texture_view: Option<TextureView>,
    /// El gesto de UV en curso (su vista previa va en `preview`).
    pub uv_op: Option<UvOp>,
    /// Encuadrar abajo las UVs de las caras elegidas en cuanto el lienzo sepa el área (al
    /// elegirlas con un clic arriba).
    pub focus_uvs: bool,
    /// La selección de pixeles que puso el modo Texture con las caras elegidas (ver
    /// `sync_face_mask`).
    pub(crate) face_mask: Option<super::texture_mode::FaceMask>,
    /// El gizmo de las vistas en modo *Model* (se queda al cambiar de modelo).
    pub gizmo: Gizmo,
}

impl Default for ModelEdit {
    fn default() -> Self {
        Self {
            artboard: None,
            selection: Selection::default(),
            grid: DEFAULT_GRID,
            center: [0.0; 3],
            zoom: 1.0,
            fit: false,
            last_view: OrthoView::Front,
            preview: None,
            op: None,
            key_request: None,
            click_item: None,
            mode: CadMode::Model,
            texture_view: None,
            uv_op: None,
            focus_uvs: false,
            face_mask: None,
            gizmo: Gizmo::default(),
        }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// El factor más chico (en valor absoluto) de los gizmos de escala.
const MIN_SCALE: f32 = 0.001;

/// El factor de escala `f` de un gizmo en milésimas (como las escalas), nunca 0: lo más chico
/// es una milésima, con su signo (al pasar del centro, voltea).
fn scale_factor(f: f32) -> f32 {
    let f = (f * 1000.0).round() / 1000.0;
    if f.abs() < MIN_SCALE {
        MIN_SCALE.copysign(f)
    } else {
        f
    }
}

/// Cuánto giró el puntero de `a` a `b` alrededor de `center` en pantalla, en grados y en el
/// sentido positivo del eje `axis` (el del aro); `project` lleva el mundo a la pantalla. 0 si el
/// aro se ve de canto o el puntero pasa por el centro.
pub fn ring_turn(
    project: impl Fn(P3) -> (f32, f32),
    center: P3,
    axis: usize,
    a: (f32, f32),
    b: (f32, f32),
) -> f32 {
    let c = project(center);
    let along = |k: usize| {
        let mut p = center;
        p[k] += 1.0;
        let s = project(p);
        (s.0 - c.0, s.1 - c.1)
    };
    // El giro positivo lleva el eje que sigue hacia el otro: así se ve su sentido en pantalla.
    let (u, v) = (along((axis + 1) % 3), along((axis + 2) % 3));
    let sense = u.0 * v.1 - u.1 * v.0;
    let (pa, pb) = ((a.0 - c.0, a.1 - c.1), (b.0 - c.0, b.1 - c.1));
    if sense.abs() < 1e-6 || pa == (0.0, 0.0) || pb == (0.0, 0.0) {
        return 0.0;
    }
    let cross = pa.0 * pb.1 - pa.1 * pb.0;
    let dot = pa.0 * pb.0 + pa.1 * pb.1;
    cross.atan2(dot).to_degrees() * sense.signum()
}

impl Editor {
    pub fn model_edit(&self) -> &ModelEdit {
        &self.model_edit
    }

    /// Si el modo Edit está prendido: en el workspace de un modelo, siempre (en los dos modos; en
    /// Texture el activo es su textura).
    pub fn model_edit_on(&self) -> bool {
        self.workspace_model().is_some()
            && (self.workspace == Some(self.active) || self.model_edit.mode == CadMode::Texture)
    }

    /// Centro y zoom para que el modelo entero quepa, con aire, en un cuadrante de `side` puntos
    /// de pantalla (el lado más corto). Ya no queda encuadre pendiente.
    pub fn fit_model_views(&mut self, side: f32) {
        self.model_edit.fit = false;
        let Some(b) = self.edited_model().and_then(cs_3d::model_bounds) else {
            return;
        };
        let extent = (0..3).map(|i| b.1[i] - b.0[i]).fold(16.0_f32, f32::max);
        self.model_edit.center = cs_3d::center(b);
        self.model_edit.zoom = (0.8 * side / extent).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn set_select_mode(&mut self, mode: SelectMode) {
        // En modo Texture la selección es siempre de caras.
        if self.model_edit.mode == CadMode::Texture && mode != SelectMode::Face {
            return;
        }
        let from = self.model_edit.selection.mode;
        let next = match self.edited_model() {
            // De Object a un subelemento se empieza vacío: convertir elegiría todo el objeto y
            // arrastrar una arista movería el bloque entero.
            Some(m) if from != SelectMode::Object || mode == SelectMode::Object => {
                self.model_edit.selection.converted(m, mode)
            }
            _ => Selection {
                mode,
                items: Default::default(),
            },
        };
        self.set_model_selection(next);
    }

    /// La selección nueva; el objeto del panel pasa a ser el primero de ella (si hay).
    pub fn set_model_selection(&mut self, sel: Selection) {
        if let Some(o) = sel.first_object() {
            self.model_object = Some(o);
        }
        self.model_edit.selection = sel;
        // En modo Texture la selección de pixeles sigue a las caras (lo flotante se confirma
        // antes: la selección de debajo cambia).
        if self.model_edit.mode == CadMode::Texture {
            self.commit_floating();
            self.sync_face_mask();
        }
    }

    pub fn set_model_grid(&mut self, grid: i32) {
        self.model_edit.grid = grid.max(1);
    }

    /// El modelo que se ve en el modo Edit: la vista previa del gesto o el del documento.
    pub fn edit_model_shown(&self) -> Option<&Model3D> {
        self.model_edit.preview.as_ref().or(self.edited_model())
    }

    pub fn request_model_key_op(&mut self, op: KeyOp) {
        if self.model_edit_on() && !self.model_edit.selection.is_empty() {
            self.model_edit.key_request = Some(op);
        }
    }

    /// El gizmo `g` (estado de la UI: no es un paso de undo).
    pub fn set_gizmo(&mut self, g: Gizmo) {
        self.model_edit.gizmo = g;
    }

    /// El gizmo `g`, o ninguno si ya era ese (los botones del encabezado y W, E, R).
    pub fn toggle_gizmo(&mut self, g: Gizmo) {
        let next = if self.model_edit.gizmo == g {
            Gizmo::None
        } else {
            g
        };
        self.set_gizmo(next);
    }

    /// El gizmo que se ve en las vistas: el elegido, en modo *Model* y con algo elegido.
    pub fn shown_gizmo(&self) -> Option<GizmoKind> {
        let e = &self.model_edit;
        if e.mode != CadMode::Model || e.selection.is_empty() || !self.model_edit_on() {
            return None;
        }
        e.gizmo.kind()
    }

    pub fn start_model_op(&mut self, op: EditOp) {
        if let EditOp::Move { map, .. } | EditOp::Handle { map, .. } | EditOp::Gizmo { map, .. } =
            op
        {
            self.model_edit.last_view = map.view;
        }
        self.model_edit.preview = None;
        self.model_edit.click_item = None;
        self.model_edit.op = Some(op);
    }

    /// El gesto en curso sigue al puntero (`free`: Ctrl, sin cuadrícula; `proportional`: Shift).
    ///
    /// El estado del modo Edit (gesto, vista previa, selección, centro y zoom) no cambia
    /// `revision`: las vistas se dibujan cada frame desde él y `revision` recompondría todos los
    /// artboards. Los cambios del documento ya la suben al confirmar.
    pub fn update_model_op(&mut self, pointer: (f32, f32), free: bool, proportional: bool) {
        let Some(op) = self.model_edit.op else {
            return;
        };
        // Un arrastre con el mouse empieza pasado el umbral: antes no hay vista previa (y al
        // soltar no hay paso de undo).
        let from = match op {
            EditOp::Move {
                from,
                keyboard: false,
                ..
            }
            | EditOp::Handle { from, .. }
            | EditOp::Gizmo { from, .. } => Some(from),
            _ => None,
        };
        if from.is_some_and(|f| dist(pointer, f) < DRAG_THRESHOLD)
            && self.model_edit.preview.is_none()
        {
            return;
        }
        // El aro suma lo que gira el puntero desde el punto anterior (así pasa de media vuelta
        // sin saltar); el giro sale siempre del modelo del inicio.
        let op = match op {
            EditOp::Gizmo {
                map,
                part: part @ GizmoPart::Ring(axis),
                from,
                center,
                last,
                turned,
            } => {
                let turned = turned + ring_turn(|p| map.to_screen(p), center, axis, last, pointer);
                let op = EditOp::Gizmo {
                    map,
                    part,
                    from,
                    center,
                    last: pointer,
                    turned,
                };
                self.model_edit.op = Some(op);
                op
            }
            op => op,
        };
        let grid = (!free).then_some(self.model_edit.grid);
        let sel = &self.model_edit.selection;
        let preview = match (op, self.edited_model()) {
            (
                EditOp::Move {
                    map, from, grab, ..
                },
                Some(m),
            ) => {
                let d = map.drag_delta((pointer.0 - from.0, pointer.1 - from.1));
                let d = cs_3d::snapped_delta(map.view, grab, d, grid);
                Some(cs_3d::translate(m, sel, d))
            }
            (
                EditOp::Handle {
                    map,
                    handle,
                    bounds,
                    ..
                },
                Some(m),
            ) => {
                let to = map.to_plane(pointer);
                let (anchor, f) =
                    cs_3d::handle_scale(map.view, bounds, handle, to, grid, proportional);
                Some(cs_3d::scale(m, sel, anchor, f))
            }
            (
                EditOp::Scale {
                    map,
                    center,
                    from_dist,
                },
                Some(m),
            ) if from_dist > 0.0 => {
                let f =
                    (dist(pointer, map.to_screen(center)) / from_dist * 1000.0).round() / 1000.0;
                Some(cs_3d::scale(m, sel, center, [f; 3]))
            }
            (
                EditOp::Gizmo {
                    map,
                    part,
                    from,
                    center,
                    turned,
                    ..
                },
                Some(m),
            ) => gizmo_preview(
                m,
                sel,
                GizmoDrag {
                    map,
                    part,
                    center,
                    from,
                    turned,
                },
                pointer,
                grid,
            ),
            (EditOp::Box { map, from, add, .. }, _) => {
                self.model_edit.op = Some(EditOp::Box {
                    map,
                    from,
                    to: pointer,
                    add,
                });
                None
            }
            (EditOp::Pan { map, from, center }, _) => {
                let d = map.drag_delta((pointer.0 - from.0, pointer.1 - from.1));
                self.model_edit.center = [center[0] - d[0], center[1] - d[1], center[2] - d[2]];
                None
            }
            _ => None,
        };
        if preview.is_some() {
            self.model_edit.preview = preview;
        }
    }

    /// Termina el gesto: mover o escalar es un paso de undo; la caja cambia la selección.
    pub fn finish_model_op(&mut self) {
        let Some(op) = self.model_edit.op.take() else {
            return;
        };
        if let EditOp::Box { map, from, to, add } = op {
            if let Some(m) = self.edited_model() {
                let mode = self.model_edit.selection.mode;
                let hit = cs_3d::box_select(m, &map, mode, from, to);
                let mut next = if add {
                    self.model_edit.selection.clone()
                } else {
                    Selection {
                        mode,
                        items: Default::default(),
                    }
                };
                next.items.extend(hit);
                self.set_model_selection(next);
            }
            return;
        }
        // Un clic (sin arrastre) en algo ya elegido lo deja como la única selección.
        let click = self.model_edit.click_item.take();
        if let (EditOp::Move { .. }, Some(item), None) = (op, click, &self.model_edit.preview) {
            let mode = self.model_edit.selection.mode;
            self.set_model_selection(Selection {
                mode,
                items: [item].into(),
            });
            return;
        }
        let label = match op {
            EditOp::Move { .. }
            | EditOp::Gizmo {
                part: GizmoPart::Axis(_) | GizmoPart::Plane,
                ..
            } => "Move",
            EditOp::Gizmo {
                part: GizmoPart::Ring(_),
                ..
            } => "Rotate",
            _ => "Scale",
        };
        if let Some(model) = self.model_edit.preview.take() {
            self.named(label, |e| e.set_model(model));
        }
    }

    /// Cancela el gesto sin tocar el documento. `false` si no había ninguno.
    pub fn cancel_model_op(&mut self) -> bool {
        self.model_edit.key_request = None;
        self.model_edit.click_item = None;
        let had = self.model_edit.op.take().is_some();
        self.model_edit.preview = None;
        had
    }

    /// Una flecha: lo elegido un paso de cuadrícula en la última vista usada (`dx` a la derecha,
    /// `dy` hacia abajo en pantalla). Un paso de undo.
    pub fn nudge_model_selection(&mut self, dx: i32, dy: i32) -> bool {
        let Some(m) = self.edited_model() else {
            return false;
        };
        if self.model_edit.selection.is_empty() {
            return false;
        }
        let g = self.model_edit.grid as f32;
        let d = self
            .model_edit
            .last_view
            .world_delta((dx as f32 * g, -dy as f32 * g));
        let next = cs_3d::translate(m, &self.model_edit.selection, d);
        self.named("Move", |e| e.set_model(next))
    }

    /// `steps` pasos de rueda sobre la vista de `map`, alrededor del punto de pantalla `at`.
    pub fn zoom_model_views(&mut self, map: &ViewMap, at: (f32, f32), steps: i32) {
        let old = self.model_edit.zoom;
        let zoom = (old * ZOOM_STEP.powi(steps)).clamp(ZOOM_MIN, ZOOM_MAX);
        // El punto bajo el puntero se queda ahí: `scale` del mapa crece igual que el zoom.
        let before = map.to_plane(at);
        let after = ViewMap {
            scale: map.scale * zoom / old,
            ..*map
        }
        .to_plane(at);
        let shift = map
            .view
            .world_delta((before.0 - after.0, before.1 - after.1));
        let c = &mut self.model_edit.center;
        for i in 0..3 {
            c[i] += shift[i];
        }
        self.model_edit.zoom = zoom;
    }

    /// Después de cada cambio del documento (y al cambiar de artboard): no queda gesto ni vista
    /// previa (un deshacer a media mano no se pisa al soltar) y la selección pierde lo que ya no
    /// existe.
    pub(crate) fn sync_model_edit(&mut self) {
        let edit = &mut self.model_edit;
        let model = edit
            .artboard
            .and_then(|id| self.document.artboard(id))
            .and_then(|a| a.model.as_ref());
        match model {
            Some(m) => edit.selection.retain_valid(m),
            None => edit.selection.items.clear(),
        }
        edit.op = None;
        edit.uv_op = None;
        edit.preview = None;
    }
}

/// Un arrastre del gizmo: el asa `part` del gizmo de `center`, desde `from` en la vista de
/// `map`; `turned`, lo girado por el aro.
struct GizmoDrag {
    map: ViewMap,
    part: GizmoPart,
    center: P3,
    from: (f32, f32),
    turned: f32,
}

/// La vista previa de arrastrar el gizmo hasta `pointer` (`grid`: la cuadrícula, `None` con
/// Ctrl), siempre desde el modelo del inicio `m`.
fn gizmo_preview(
    m: &Model3D,
    sel: &Selection,
    drag: GizmoDrag,
    pointer: (f32, f32),
    grid: Option<i32>,
) -> Option<Model3D> {
    let GizmoDrag {
        map,
        part,
        center,
        from,
        turned,
    } = drag;
    let c = map.to_screen(center);
    match part {
        // Las flechas, solo en su eje del mundo; el cuadrito, en el plano de la vista. El centro
        // del gizmo cae en la cuadrícula, como lo agarrado al mover.
        GizmoPart::Axis(_) | GizmoPart::Plane => {
            let d = map.drag_delta((pointer.0 - from.0, pointer.1 - from.1));
            let mut d = cs_3d::snapped_delta(map.view, center, d, grid);
            if let GizmoPart::Axis(axis) = part {
                for (i, v) in d.iter_mut().enumerate() {
                    if i != axis {
                        *v = 0.0;
                    }
                }
            }
            Some(cs_3d::translate(m, sel, d))
        }
        // En pasos de 15° (con Ctrl, de 1°).
        GizmoPart::Ring(axis) => {
            let step = if grid.is_some() { ROTATE_STEP } else { 1.0 };
            let tenths = ((turned / step).round() * step * 10.0).round() as i32;
            Some(cs_3d::rotate_selection(m, sel, center, axis, tenths))
        }
        // La distancia al centro sobre el eje (en pantalla) entre la del inicio.
        GizmoPart::ScaleAxis(axis) => {
            let mut tip = center;
            tip[axis] += 1.0;
            let t = map.to_screen(tip);
            let dir = (t.0 - c.0, t.1 - c.1);
            let len = dir.0.hypot(dir.1);
            if len < 1e-6 {
                return None;
            }
            let along = |p: (f32, f32)| ((p.0 - c.0) * dir.0 + (p.1 - c.1) * dir.1) / len;
            let start = along(from);
            if start.abs() < 1.0 {
                return None;
            }
            let mut f = [1.0; 3];
            f[axis] = scale_factor(along(pointer) / start);
            Some(cs_3d::scale(m, sel, center, f))
        }
        // El cubito del centro se agarra en el centro (la distancia del inicio es casi 0): el
        // factor crece con lo que el puntero va a la derecha o arriba, 2× a un gizmo de
        // distancia, y baja hacia la izquierda o abajo (nunca a 0 ni menos).
        GizmoPart::ScaleUniform => {
            let moved = (pointer.0 - from.0) - (pointer.1 - from.1);
            let f = scale_factor(1.0 + moved / GIZMO_PX).max(MIN_SCALE);
            Some(cs_3d::scale(m, sel, center, [f; 3]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gizmo_scale_factor_is_never_zero_and_keeps_its_sign() {
        assert_eq!(scale_factor(0.0), 0.001);
        assert_eq!(scale_factor(0.0003), 0.001);
        assert_eq!(scale_factor(-0.0003), -0.001);
        assert_eq!(scale_factor(-0.5), -0.5);
        assert_eq!(scale_factor(1.23456), 1.235);
    }
}
