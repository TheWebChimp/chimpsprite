//! El modo Edit del artboard 3D (picoCAD): la selección (objetos, vértices, aristas o caras),
//! las vistas ortográficas (centro y zoom compartidos), la cuadrícula y los gestos de mover y
//! escalar. Durante un gesto solo cambia la vista previa; al terminar es un paso de undo.

use cs_3d::{Handle, OrthoView, P3, SelectMode, Selection, ViewMap};
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
    /// Desplazar las vistas (botón derecho o central).
    Pan {
        map: ViewMap,
        from: (f32, f32),
        center: P3,
    },
}

/// El estado del modo Edit (de la UI: no se guarda ni pasa por undo).
#[derive(Clone, Debug)]
pub struct ModelEdit {
    pub on: bool,
    pub artboard: Option<ArtboardId>,
    pub selection: Selection,
    pub grid: i32,
    pub center: P3,
    /// Pixeles del artboard por 1/16 de unidad en las vistas ortográficas.
    pub zoom: f32,
    pub last_view: OrthoView,
    pub preview: Option<Model3D>,
    pub op: Option<EditOp>,
    pub key_request: Option<KeyOp>,
}

impl Default for ModelEdit {
    fn default() -> Self {
        Self {
            on: false,
            artboard: None,
            selection: Selection::default(),
            grid: DEFAULT_GRID,
            center: [0.0; 3],
            zoom: 1.0,
            last_view: OrthoView::Front,
            preview: None,
            op: None,
            key_request: None,
        }
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl Editor {
    pub fn model_edit(&self) -> &ModelEdit {
        &self.model_edit
    }

    /// Si el modo Edit está prendido en el artboard activo (y es un modelo).
    pub fn model_edit_on(&self) -> bool {
        self.model_edit.on && self.model_edit.artboard == Some(self.active) && self.is_model()
    }

    /// Prende o apaga el modo Edit. Al prenderlo en otro modelo, la selección empieza vacía;
    /// siempre encuadra el modelo entero en las vistas.
    pub fn toggle_model_edit(&mut self) {
        if self.model_edit_on() {
            self.model_edit.on = false;
            self.model_edit.preview = None;
            self.model_edit.op = None;
            self.model_edit.key_request = None;
        } else if self.is_model() {
            if self.model_edit.artboard != Some(self.active) {
                let mode = self.model_edit.selection.mode;
                self.model_edit.selection = Selection {
                    mode,
                    items: Default::default(),
                };
            }
            self.model_edit.on = true;
            self.model_edit.artboard = Some(self.active);
            self.fit_model_views();
        }
        self.revision += 1;
    }

    /// Centro y zoom para que el modelo entero quepa en un cuadrante (la mitad del artboard),
    /// con aire.
    fn fit_model_views(&mut self) {
        let Some(b) = self.model().and_then(cs_3d::model_bounds) else {
            return;
        };
        let extent = (0..3).map(|i| b.1[i] - b.0[i]).fold(16.0_f32, f32::max);
        let side = self.sprite().width.min(self.sprite().height) as f32 / 2.0;
        self.model_edit.center = cs_3d::center(b);
        self.model_edit.zoom = (0.8 * side / extent).clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn set_select_mode(&mut self, mode: SelectMode) {
        let next = match self.model() {
            Some(m) => self.model_edit.selection.converted(m, mode),
            None => Selection {
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
    }

    pub fn set_model_grid(&mut self, grid: i32) {
        self.model_edit.grid = grid.max(1);
    }

    /// El modelo que se ve en el modo Edit: la vista previa del gesto o el del documento.
    pub fn edit_model_shown(&self) -> Option<&Model3D> {
        self.model_edit.preview.as_ref().or(self.model())
    }

    pub fn request_model_key_op(&mut self, op: KeyOp) {
        if self.model_edit_on() && !self.model_edit.selection.is_empty() {
            self.model_edit.key_request = Some(op);
        }
    }

    pub fn start_model_op(&mut self, op: EditOp) {
        if let EditOp::Move { map, .. } | EditOp::Handle { map, .. } = op {
            self.model_edit.last_view = map.view;
        }
        self.model_edit.preview = None;
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
            | EditOp::Handle { from, .. } => Some(from),
            _ => None,
        };
        if from.is_some_and(|f| dist(pointer, f) < DRAG_THRESHOLD)
            && self.model_edit.preview.is_none()
        {
            return;
        }
        let grid = (!free).then_some(self.model_edit.grid);
        let sel = &self.model_edit.selection;
        let preview = match (op, self.model()) {
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
            if let Some(m) = self.model() {
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
        let label = match op {
            EditOp::Move { .. } => "Move",
            _ => "Scale",
        };
        if let Some(model) = self.model_edit.preview.take() {
            self.named(label, |e| e.set_model(model));
        }
    }

    /// Cancela el gesto sin tocar el documento. `false` si no había ninguno.
    pub fn cancel_model_op(&mut self) -> bool {
        self.model_edit.key_request = None;
        let had = self.model_edit.op.take().is_some();
        self.model_edit.preview = None;
        had
    }

    /// Una flecha: lo elegido un paso de cuadrícula en la última vista usada (`dx` a la derecha,
    /// `dy` hacia abajo en pantalla). Un paso de undo.
    pub fn nudge_model_selection(&mut self, dx: i32, dy: i32) -> bool {
        let Some(m) = self.model() else {
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
            None => {
                edit.on = false;
                edit.selection.items.clear();
            }
        }
        edit.op = None;
        edit.preview = None;
    }
}
