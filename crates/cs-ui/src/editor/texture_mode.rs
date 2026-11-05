//! El modo Texture de la pestaña de un modelo: el `Editor` pinta en la textura del modelo del
//! workspace (`active` = la textura, `workspace` = el modelo) y edita las UVs de las caras
//! elegidas. Cambiar de modo es estado de la UI: no toca el documento ni el undo.

use cs_3d::{Item, SelectMode, Selection, UvPoint};
use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Mask, Model3D};

use super::model_edit::{CadMode, UvOp};
use super::{Editor, Row};

/// Los lados de la textura que se pueden elegir (Texture size).
pub const TEXTURE_SIDES: std::ops::RangeInclusive<u32> = 16..=512;

/// De qué sale la selección de pixeles de las caras elegidas: la textura, su tamaño y las UVs de
/// cada cara.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FaceMaskKey {
    texture: ArtboardId,
    size: (u32, u32),
    faces: Vec<((usize, usize), Vec<UvPoint>)>,
}

/// La selección de pixeles que puso el modo Texture (la unión de los polígonos UV de las caras
/// elegidas) y de qué salió. Solo se vuelve a poner cuando eso cambia: una marquesina del
/// usuario se queda hasta que cambien las caras o sus UVs. `on`: la selección de ahora es la
/// suya (la apaga cualquier otra selección: `set_selection`, confirmar lo flotante…).
#[derive(Clone, Debug)]
pub(crate) struct FaceMask {
    key: FaceMaskKey,
    on: bool,
}

impl Editor {
    pub fn cad_mode(&self) -> CadMode {
        self.model_edit.mode
    }

    /// El modelo del workspace (en los dos modos), aunque el activo sea su textura.
    pub fn workspace_model(&self) -> Option<&Model3D> {
        self.document.artboard(self.workspace?)?.model.as_ref()
    }

    /// La textura del modelo del workspace, si es un artboard de pixeles que existe (no un
    /// modelo ni una fuente).
    pub(crate) fn workspace_texture(&self) -> Option<ArtboardId> {
        let id = self.workspace_model()?.texture?;
        self.document
            .artboard(id)
            .filter(|a| a.model.is_none() && a.font.is_none())
            .map(|a| a.id)
    }

    /// La textura del modelo que se edita ([`Self::edited_model`]), si es un artboard de pixeles
    /// que existe. En el workspace es la misma que [`Self::workspace_texture`]; fuera de él, la
    /// del modelo activo.
    pub(crate) fn edited_texture(&self) -> Option<ArtboardId> {
        let id = self.edited_model()?.texture?;
        self.document
            .artboard(id)
            .filter(|a| a.model.is_none() && a.font.is_none())
            .map(|a| a.id)
    }

    /// Si `id` es la textura del modelo del workspace en modo Texture (activarla no sale del
    /// workspace).
    pub(crate) fn is_workspace_texture(&self, id: ArtboardId) -> bool {
        self.model_edit.mode == CadMode::Texture && self.workspace_texture() == Some(id)
    }

    /// Si los atajos de pixeles (pegar, borrar, rellenar, seleccionar…) están bloqueados: en el
    /// workspace de un modelo con el modelo activo (el modo Model, o Texture sin textura)
    /// tocarían su sprite escondido. Con la textura activa actúan sobre ella.
    pub fn pixel_actions_blocked(&self) -> bool {
        self.model_edit_on() && self.is_model()
    }

    /// Cambia de modo: en Texture activa la textura del modelo del workspace (si tiene; si no,
    /// el modelo sigue activo y el lienzo avisa) y pasa la selección a modo Face; en Model
    /// vuelve a activar el modelo. No toca el documento.
    pub fn set_cad_mode(&mut self, mode: CadMode) {
        if mode == self.model_edit.mode || self.workspace_model().is_none() {
            return;
        }
        self.cancel_model_op();
        self.cancel_uv_op();
        // Activar la textura o el modelo olvida el objeto del panel: es el mismo modelo.
        let (object, group) = (self.model_object, self.model_group);
        self.model_edit.mode = mode;
        match mode {
            CadMode::Texture => {
                self.model_edit.texture_view = None;
                if self.model_edit.selection.mode != SelectMode::Face {
                    self.set_select_mode(SelectMode::Face);
                }
                if let Some(t) = self.workspace_texture() {
                    self.activate(t);
                }
            }
            CadMode::Model => {
                if let Some(w) = self.workspace {
                    self.activate(w);
                }
            }
        }
        (self.model_object, self.model_group) = (object, group);
        self.revision += 1;
    }

    /// Elige la cara del modelo que se edita que se ve en el pixel `(x, y)` de su render (a la
    /// resolución de su artboard), con toda su cara completa. `toggle` (Shift): la agrega o la
    /// quita; si no, queda sola, y un clic en vacío vacía la selección. La selección queda en
    /// modo Face.
    pub fn pick_model_face(&mut self, (x, y): (i32, i32), toggle: bool) {
        let Some(artboard) = self.document.artboard(self.model_id()) else {
            return;
        };
        let Some(model) = artboard.model.as_ref() else {
            return;
        };
        let size = (artboard.sprite.width.max(1), artboard.sprite.height.max(1));
        // Afuera del artboard también: el render de arriba llena su panel.
        let hit = cs_render::model_pick_face(&self.document, artboard, model, size, (x, y));
        let mut sel = self.model_edit.selection.clone();
        if sel.mode != SelectMode::Face {
            sel = Selection {
                mode: SelectMode::Face,
                items: Default::default(),
            };
        }
        // La cara completa en bloque.
        match (hit, toggle) {
            (Some((o, f)), true) => sel.toggle_part(model, Item::Face(o, f)),
            (Some((o, f)), false) => {
                sel.items = cs_3d::part_items(model, o, f).into_iter().collect()
            }
            (None, true) => return,
            (None, false) => sel.items.clear(),
        }
        // Con caras elegidas, la textura de abajo se encuadra en sus UVs.
        self.model_edit.focus_uvs = !sel.items.is_empty();
        // Elegir caras (aunque sean las mismas) vuelve a poner su selección, aunque otra (Ctrl+D,
        // Ctrl+A, lo flotante confirmado) la haya reemplazado: se olvida la de antes para que se
        // recalcule.
        if self.model_edit.mode == CadMode::Texture {
            self.commit_floating();
        }
        if !sel.items.is_empty() && self.model_edit.face_mask.as_ref().is_some_and(|m| !m.on) {
            self.model_edit.face_mask = None;
        }
        self.set_model_selection(sel);
    }

    /// De qué sale la selección de las caras ahora: en modo Texture, con la textura activa y
    /// caras elegidas.
    fn face_mask_key(&self) -> Option<FaceMaskKey> {
        if self.model_edit.mode != CadMode::Texture {
            return None;
        }
        let texture = self.workspace_texture().filter(|&t| t == self.active)?;
        let faces = self.selected_faces();
        if faces.is_empty() {
            return None;
        }
        let model = self.workspace_model()?;
        let sprite = &self.document.artboard(texture)?.sprite;
        let mut faces: Vec<_> = faces
            .into_iter()
            .filter_map(|(o, f)| {
                let uvs = model.objects.get(o)?.mesh.faces.get(f)?.uvs.clone();
                Some(((o, f), uvs))
            })
            .collect();
        faces.sort_by_key(|(id, _)| *id);
        Some(FaceMaskKey {
            texture,
            size: (sprite.width, sprite.height),
            faces,
        })
    }

    /// Con caras elegidas en modo Texture, la selección de pixeles de la textura es la unión de
    /// sus polígonos UV (las herramientas solo pintan ahí); se recalcula cuando cambian las caras,
    /// sus UVs o la textura. Sin caras (o fuera de Texture) se quita, si sigue siendo la suya. No
    /// pasa por el undo.
    pub(crate) fn sync_face_mask(&mut self) {
        // Con algo flotando la selección es la suya: se espera a que se confirme.
        if self.floating.is_some() {
            return;
        }
        let key = self.face_mask_key();
        if self.model_edit.face_mask.as_ref().map(|m| &m.key) == key.as_ref() {
            return;
        }
        let old = self.model_edit.face_mask.take();
        match key {
            Some(key) => {
                let model = self.workspace_model().expect("the key has a model");
                let ids: Vec<(usize, usize)> = key.faces.iter().map(|(id, _)| *id).collect();
                let (w, h) = key.size;
                let covered = cs_3d::uv_coverage(model, &ids, key.size);
                let mut mask = Mask::new(w, h);
                for (i, _) in covered.iter().enumerate().filter(|(_, on)| **on) {
                    mask.set((i % w as usize) as i32, (i / w as usize) as i32, true);
                }
                let mask = (!mask.is_empty()).then_some(mask);
                let on = mask.is_some();
                self.selection = mask;
                self.model_edit.face_mask = Some(FaceMask { key, on });
                self.revision += 1;
            }
            None => {
                if old.is_some_and(|o| o.on) {
                    self.selection = None;
                    self.revision += 1;
                }
            }
        }
    }

    /// Si la selección de pixeles es la de las caras elegidas (la que puso el modo Texture).
    /// Con algo flotando, no: la selección es la de lo flotante.
    pub fn selection_is_face_mask(&self) -> bool {
        self.floating.is_none() && self.model_edit.face_mask.as_ref().is_some_and(|m| m.on)
    }

    /// La selección de pixeles dejó de ser la de las caras (otra la reemplazó).
    pub(crate) fn face_mask_replaced(&mut self) {
        if let Some(m) = &mut self.model_edit.face_mask {
            m.on = false;
        }
    }

    /// Si pasar la textura a `side` la achica y deja vértices UV de alguna cara fuera (antes de
    /// hacerlo se pide confirmación).
    pub fn texture_size_cuts_uvs(&self, side: u32) -> bool {
        let Some(sprite) = self
            .edited_texture()
            .and_then(|id| self.document.artboard(id))
            .map(|a| &a.sprite)
        else {
            return false;
        };
        if side >= sprite.width && side >= sprite.height {
            return false;
        }
        let s = side as i32;
        self.edited_model().is_some_and(|m| {
            m.objects
                .iter()
                .flat_map(|o| &o.mesh.faces)
                .flat_map(|f| &f.uvs)
                .any(|&(x, y)| x < 0 || y < 0 || x > s || y > s)
        })
    }

    /// Las caras elegidas `(objeto, cara)` (solo en modo Face).
    pub fn selected_faces(&self) -> Vec<(usize, usize)> {
        let sel = &self.model_edit.selection;
        if sel.mode != SelectMode::Face {
            return Vec::new();
        }
        sel.items
            .iter()
            .filter_map(|&i| match i {
                Item::Face(o, f) => Some((o, f)),
                _ => None,
            })
            .collect()
    }

    /// Las caras que abarca la caja de UVs: con caras elegidas (modo Face), esas; si no, todas las
    /// de los objetos elegidos en modo Object o, sin ellos, las del objeto o el grupo de la fila
    /// activa del outliner. Los objetos ocultos (ellos o un grupo suyo) quedan fuera, como de la
    /// selección 3D en `set_active_row`: la fila de un objeto oculto no tiene caja. Sin nada de
    /// eso, ninguna (no hay caja).
    pub fn uv_box_faces(&self) -> Vec<(usize, usize)> {
        let faces = self.selected_faces();
        if !faces.is_empty() {
            return faces;
        }
        let Some(model) = self.edited_model() else {
            return Vec::new();
        };
        let sel = &self.model_edit.selection;
        let mut objects: Vec<usize> = if sel.mode == SelectMode::Object {
            sel.items.iter().map(|i| i.object()).collect()
        } else {
            Vec::new()
        };
        if objects.is_empty() {
            objects = match self.active_row() {
                Some(Row::Object(o)) => vec![o],
                Some(Row::Group(g)) => model.group_objects(g),
                None => Vec::new(),
            };
        }
        objects.sort_unstable();
        objects.dedup();
        objects
            .into_iter()
            .filter(|&o| !model.object_hidden(o))
            .filter_map(|o| Some((o, model.objects.get(o)?.mesh.faces.len())))
            .flat_map(|(o, n)| (0..n).map(move |f| (o, f)))
            .collect()
    }

    /// El lado de la textura del modelo que se edita pasa a `side`, anclado arriba a la
    /// izquierda: lo pintado se queda donde estaba y al achicar se recorta. Las UVs no cambian.
    /// Un paso de undo. Solo lados de [`TEXTURE_SIDES`] (de 16 a 512).
    pub fn set_texture_size(&mut self, side: u32) -> bool {
        let Some(id) = self.edited_texture() else {
            return false;
        };
        let Some(sprite) = self.document.artboard(id).map(|a| &a.sprite) else {
            return false;
        };
        if !TEXTURE_SIDES.contains(&side) || (sprite.width, sprite.height) == (side, side) {
            return false;
        }
        let shrink = side < sprite.width || side < sprite.height;
        let mut cmds = vec![DocCommand::OnArtboard {
            id,
            cmd: Command::ResizeCanvas {
                width: side,
                height: side,
                offset: (0, 0),
            },
        }];
        if shrink {
            cmds.push(DocCommand::OnArtboard {
                id,
                cmd: Command::TrimToCanvas,
            });
        }
        self.named("Texture Size", |e| e.execute_doc(DocCommand::Batch(cmds)))
    }

    /// Empieza un gesto de UV (sin vista previa hasta que el puntero se mueva un pixel).
    pub fn start_uv_op(&mut self, op: UvOp) {
        self.model_edit.preview = None;
        self.model_edit.uv_op = Some(op);
    }

    /// El gesto de UV sigue al puntero (`pointer` en pixeles de la textura; `proportional`:
    /// Shift, al escalar el mismo factor en los dos ejes). Todo cae en pixeles enteros.
    pub fn update_uv_op(&mut self, pointer: (f32, f32), proportional: bool) {
        let Some(op) = self.model_edit.uv_op else {
            return;
        };
        let Some(m) = self.edited_model() else {
            return;
        };
        let faces = self.uv_box_faces();
        let delta = |from: (f32, f32)| {
            (
                (pointer.0 - from.0).round() as i32,
                (pointer.1 - from.1).round() as i32,
            )
        };
        let preview = match op {
            UvOp::Move { from } => cs_3d::move_uvs(m, &faces, delta(from)),
            UvOp::Scale {
                handle,
                from,
                bounds,
            } => {
                let (anchor, f) = uv_handle_scale(handle, from, bounds, pointer, proportional);
                cs_3d::scale_uvs(m, &faces, anchor, f)
            }
            UvOp::Vertex { face, vertex, from } => {
                let Some(&(x, y)) = m
                    .objects
                    .get(face.0)
                    .and_then(|o| o.mesh.faces.get(face.1))
                    .and_then(|f| f.uvs.get(vertex))
                else {
                    return;
                };
                let (dx, dy) = delta(from);
                cs_3d::move_uv_vertex(m, face, vertex, (x + dx, y + dy))
            }
        };
        self.model_edit.preview = Some(preview);
    }

    /// Termina el gesto de UV: si cambió algo, un paso de undo.
    pub fn finish_uv_op(&mut self) {
        let Some(op) = self.model_edit.uv_op.take() else {
            return;
        };
        let label = match op {
            UvOp::Scale { .. } => "Scale UV",
            UvOp::Move { .. } | UvOp::Vertex { .. } => "Move UV",
        };
        if let Some(model) = self.model_edit.preview.take() {
            self.named(label, |e| e.set_model(model));
        }
    }

    /// Cancela el gesto de UV sin tocar el documento. `false` si no había ninguno.
    pub fn cancel_uv_op(&mut self) -> bool {
        let had = self.model_edit.uv_op.take().is_some();
        if had {
            self.model_edit.preview = None;
        }
        had
    }

    /// Gira un cuarto de vuelta las UVs de cada cara elegida. Un paso de undo.
    pub fn rotate_selected_uvs(&mut self) -> bool {
        self.edit_selected_uvs("Rotate UV", cs_3d::rotate_uvs_90)
    }

    /// Voltea las UVs de cada cara elegida (`horizontal`: en x). Un paso de undo.
    pub fn flip_selected_uvs(&mut self, horizontal: bool) -> bool {
        self.edit_selected_uvs("Flip UV", |m, faces| cs_3d::flip_uvs(m, faces, horizontal))
    }

    fn edit_selected_uvs(
        &mut self,
        label: &'static str,
        f: impl FnOnce(&Model3D, &[(usize, usize)]) -> Model3D,
    ) -> bool {
        let faces = self.selected_faces();
        if faces.is_empty() {
            return false;
        }
        let Some(m) = self.edited_model() else {
            return false;
        };
        let next = f(m, &faces);
        self.named(label, |e| e.set_model(next))
    }
}

/// Arrastrar el asa `handle` de la caja `bounds` (inclusiva) agarrada en `from` hasta `pointer`
/// (en pixeles de la textura): el ancla (el lado o la esquina opuesta; en un eje sin asa, el
/// medio) y el factor por eje. El borde arrastrado cae en el pixel entero bajo el puntero y la
/// caja no baja de 1 pixel ni se voltea. Con `proportional` (Shift), los dos ejes usan el mismo
/// factor: en una esquina el que más cambió; en un lado, el de ese lado (el otro eje tampoco
/// baja de 1 pixel).
fn uv_handle_scale(
    handle: (i8, i8),
    from: (f32, f32),
    ((x0, y0), (x1, y1)): ((i32, i32), (i32, i32)),
    pointer: (f32, f32),
    proportional: bool,
) -> ((f32, f32), (f32, f32)) {
    // Por eje: el ancla y el factor (`None`: el eje no tiene asa).
    let axis = |s: i8, lo: i32, hi: i32, from: f32, at: f32| -> (f32, Option<f32>) {
        if s == 0 {
            return ((lo + hi) as f32 / 2.0, None);
        }
        let (edge, anchor) = if s > 0 { (hi, lo) } else { (lo, hi) };
        let old = edge - anchor;
        if old == 0 {
            return (anchor as f32, Some(1.0));
        }
        let moved = (edge as f32 + at - from).round() as i32 - anchor;
        let moved = if old > 0 { moved.max(1) } else { moved.min(-1) };
        (anchor as f32, Some(moved as f32 / old as f32))
    };
    let (ax, fx) = axis(handle.0, x0, x1, from.0, pointer.0);
    let (ay, fy) = axis(handle.1, y0, y1, from.1, pointer.1);
    // El factor más chico que deja `extent` pixeles en al menos 1.
    let at_least_1px = |f: f32, extent: i32| {
        if extent == 0 {
            f
        } else {
            f.max(1.0 / extent.abs() as f32)
        }
    };
    let (w, h) = (x1 - x0, y1 - y0);
    let f = match (fx, fy, proportional) {
        (Some(fx), Some(fy), true) => {
            let f = if (fx - 1.0).abs() >= (fy - 1.0).abs() {
                fx
            } else {
                fy
            };
            (at_least_1px(f, w), at_least_1px(f, h))
        }
        (Some(f), None, true) => (f, at_least_1px(f, h)),
        (None, Some(f), true) => (at_least_1px(f, w), f),
        (fx, fy, _) => (fx.unwrap_or(1.0), fy.unwrap_or(1.0)),
    };
    ((ax, ay), f)
}
