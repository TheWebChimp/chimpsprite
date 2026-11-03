//! El modo Texture de la pestaña de un modelo: el `Editor` pinta en la textura del modelo del
//! workspace (`active` = la textura, `workspace` = el modelo) y edita las UVs de las caras
//! elegidas. Cambiar de modo es estado de la UI: no toca el documento ni el undo.

use cs_3d::{Item, SelectMode, Selection, UvPoint};
use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Mask, Model3D};

use super::Editor;
use super::model_edit::{CadMode, UvOp};

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
/// usuario se queda hasta que cambien las caras o sus UVs.
#[derive(Clone, Debug)]
pub(crate) struct FaceMask {
    key: FaceMaskKey,
    mask: Option<Mask>,
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
        let object = self.model_object;
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
        self.model_object = object;
        self.revision += 1;
    }

    /// Elige la cara del modelo que se edita que se ve en el pixel `(x, y)` de su render (a la
    /// resolución de su artboard). `toggle` (Shift): la agrega o la quita; si no, queda sola, y
    /// un clic en vacío vacía la selección. La selección queda en modo Face.
    pub fn pick_model_face(&mut self, (x, y): (i32, i32), toggle: bool) {
        let Some(artboard) = self.document.artboard(self.model_id()) else {
            return;
        };
        let Some(model) = artboard.model.as_ref() else {
            return;
        };
        let size = (artboard.sprite.width.max(1), artboard.sprite.height.max(1));
        let hit = (x >= 0 && y >= 0)
            .then(|| {
                let at = (x as u32, y as u32);
                cs_render::model_pick_face(&self.document, artboard, model, size, at)
            })
            .flatten();
        let mut sel = self.model_edit.selection.clone();
        if sel.mode != SelectMode::Face {
            sel = Selection {
                mode: SelectMode::Face,
                items: Default::default(),
            };
        }
        match (hit, toggle) {
            (Some((o, f)), true) => sel.toggle(Item::Face(o, f)),
            (Some((o, f)), false) => sel.items = [Item::Face(o, f)].into(),
            (None, true) => return,
            (None, false) => sel.items.clear(),
        }
        // Con caras elegidas, la textura de abajo se encuadra en sus UVs.
        self.model_edit.focus_uvs = !sel.items.is_empty();
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
                self.selection = mask.clone();
                self.model_edit.face_mask = Some(FaceMask { key, mask });
                self.revision += 1;
            }
            None => {
                if old.is_some_and(|o| o.mask.is_some() && o.mask == self.selection) {
                    self.selection = None;
                    self.revision += 1;
                }
            }
        }
    }

    /// Si la selección de pixeles es la de las caras elegidas (la que puso el modo Texture).
    pub fn selection_is_face_mask(&self) -> bool {
        self.model_edit
            .face_mask
            .as_ref()
            .is_some_and(|m| m.mask.is_some() && m.mask == self.selection)
    }

    /// Si pasar la textura a `side` la achica y deja vértices UV de alguna cara fuera (antes de
    /// hacerlo se pide confirmación).
    pub fn texture_size_cuts_uvs(&self, side: u32) -> bool {
        let Some(sprite) = self
            .workspace_texture()
            .and_then(|id| self.document.artboard(id))
            .map(|a| &a.sprite)
        else {
            return false;
        };
        if side >= sprite.width && side >= sprite.height {
            return false;
        }
        let s = side as i32;
        self.workspace_model().is_some_and(|m| {
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

    /// El lado de la textura del modelo del workspace pasa a `side`, anclado arriba a la
    /// izquierda: lo pintado se queda donde estaba y al achicar se recorta. Las UVs no cambian.
    /// Un paso de undo. Solo lados de [`TEXTURE_SIDES`] (de 16 a 512).
    pub fn set_texture_size(&mut self, side: u32) -> bool {
        let Some(id) = self.workspace_texture() else {
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
    /// Shift, en una esquina los dos ejes con el mismo factor). Todo cae en pixeles enteros.
    pub fn update_uv_op(&mut self, pointer: (f32, f32), proportional: bool) {
        let Some(op) = self.model_edit.uv_op else {
            return;
        };
        let Some(m) = self.edited_model() else {
            return;
        };
        let faces = self.selected_faces();
        let delta = |from: (f32, f32)| {
            (
                (pointer.0 - from.0).round() as i32,
                (pointer.1 - from.1).round() as i32,
            )
        };
        let preview = match op {
            UvOp::Move { from } => cs_3d::move_uvs(m, &faces, delta(from)),
            UvOp::Scale { handle, bounds } => {
                let ((x0, y0), (x1, y1)) = bounds;
                // El ancla es el lado opuesto al asa; el borde arrastrado cae en el pixel.
                let side = |s: i8, lo: i32, hi: i32, at: f32| {
                    let (edge, anchor) = if s > 0 { (hi, lo) } else { (lo, hi) };
                    let f = if edge == anchor {
                        1.0
                    } else {
                        (at.round() - anchor as f32) / (edge - anchor) as f32
                    };
                    (anchor as f32, f)
                };
                let mid = ((x0 + x1) as f32 / 2.0, (y0 + y1) as f32 / 2.0);
                let (ax, fx) = if handle.u != 0 {
                    side(handle.u, x0, x1, pointer.0)
                } else {
                    (mid.0, 1.0)
                };
                let (ay, fy) = if handle.v != 0 {
                    side(handle.v, y0, y1, pointer.1)
                } else {
                    (mid.1, 1.0)
                };
                let (fx, fy) = if proportional && handle.u != 0 && handle.v != 0 {
                    let f = if (fx - 1.0).abs() >= (fy - 1.0).abs() {
                        fx
                    } else {
                        fy
                    };
                    (f, f)
                } else {
                    (fx, fy)
                };
                cs_3d::scale_uvs(m, &faces, (ax, ay), (fx, fy))
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
