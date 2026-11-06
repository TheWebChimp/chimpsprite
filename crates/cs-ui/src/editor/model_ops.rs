//! Lo de los modelos 3D en el editor: crear uno, sus objetos, la cámara, la luz, el contorno y
//! la textura y la tabla de shading. Cada cambio es un `SetModel` con el modelo entero (un paso de undo).

use cs_3d::{Primitive, primitive_mesh};
use cs_commands::DocCommand;
use cs_commands::model::texture_template;
use cs_core::{
    Artboard, ArtboardId, Camera3D, Light3D, Model3D, Object3D, Rgba, ShadingTable, Transform3D,
};
use cs_render::BakeOptions;

use super::Editor;

impl Editor {
    /// Un modelo 3D nuevo (con su textura) a la derecha de todo, en su workspace y con el cubo
    /// elegido.
    pub fn new_model(&mut self, name: &str, size: (u32, u32)) -> bool {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let (id, _texture, cmd) =
            cs_commands::model::new_model_artboard(&self.document, name, size, (x1 + 16, y0));
        if !self.execute_doc(cmd) {
            return false;
        }
        // Se edita en su workspace (su pestaña), fuera del lienzo.
        self.enter_workspace(id);
        self.set_frame(0);
        self.model_object = Some(0);
        true
    }

    /// El modelo del artboard activo.
    pub fn model(&self) -> Option<&Model3D> {
        self.artboard().model.as_ref()
    }

    pub fn is_model(&self) -> bool {
        self.model().is_some()
    }

    /// El artboard del modelo que se edita: el del workspace en modo Texture (ahí el activo es
    /// su textura); si no, el activo.
    pub fn model_id(&self) -> ArtboardId {
        match self.workspace {
            Some(w) if self.model_edit.mode == super::CadMode::Texture => w,
            _ => self.active,
        }
    }

    /// El modelo que se edita (el de [`Self::model_id`]): el que tocan el panel 3D y el modo
    /// Edit, aunque el activo sea su textura.
    pub fn edited_model(&self) -> Option<&Model3D> {
        self.document.artboard(self.model_id())?.model.as_ref()
    }

    /// Fuente o modelo: sus frames, capas, modo de color y tamaño no se tocan con las acciones
    /// de siempre.
    pub fn is_structure_locked(&self) -> bool {
        self.is_font() || self.is_model()
    }

    /// El objeto elegido del modelo que se edita.
    pub fn selected_object(&self) -> Option<usize> {
        let count = self.edited_model()?.objects.len();
        self.model_object.filter(|&i| i < count)
    }

    pub fn select_model_object(&mut self, i: Option<usize>) {
        self.model_object = i;
        // En el modo Edit, elegir un objeto (panel o perspectiva) pasa a modo Object con ese
        // objeto: convertirlo a sus aristas o vértices elegiría todo el bloque (solo si el
        // modelo del modo Edit es el que se edita: la selección es de ese modelo). En modo
        // Texture la selección se queda en caras.
        if self.model_edit.artboard == Some(self.model_id())
            && self.model_edit.mode == super::CadMode::Model
            && let Some(m) = self.edited_model()
        {
            self.model_edit.selection = cs_3d::Selection {
                mode: cs_3d::SelectMode::Object,
                items: i
                    .filter(|&i| i < m.objects.len())
                    .map(cs_3d::Item::Object)
                    .into_iter()
                    .collect(),
            };
        }
        self.revision += 1;
    }

    pub fn add_primitive(&mut self, p: Primitive) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.objects.push(Object3D {
            name: p.name().to_owned(),
            mesh: primitive_mesh(&p),
            transform: Transform3D::default(),
            double_sided: false,
        });
        let last = model.objects.len() - 1;
        if !self.named("Add Object", |e| e.set_model(model)) {
            return false;
        }
        self.model_object = Some(last);
        true
    }

    pub fn remove_object(&mut self, i: usize) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        if i >= model.objects.len() {
            return false;
        }
        model.objects.remove(i);
        let left = model.objects.len();
        if !self.named("Remove Object", |e| e.set_model(model)) {
            return false;
        }
        self.model_object = (left > 0).then(|| self.model_object.unwrap_or(0).min(left - 1));
        true
    }

    pub fn set_object(&mut self, i: usize, obj: Object3D) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let Some(slot) = model.objects.get_mut(i) else {
            return false;
        };
        *slot = obj;
        self.set_model(model)
    }

    /// Como `set_model`, pero los cambios seguidos del mismo `group` (un arrastre) son un solo
    /// paso de undo.
    pub fn set_model_grouped(&mut self, model: Model3D, group: eframe::egui::Id) -> bool {
        if self.edited_model() == Some(&model) {
            return false;
        }
        let cmd = set_model_cmd(self.model_id(), model);
        self.execute_doc_grouped(cmd, group.value())
    }

    pub fn set_camera(&mut self, c: Camera3D) -> bool {
        self.edit_model(|m| m.camera = c)
    }

    pub fn set_light(&mut self, l: Light3D) -> bool {
        self.edit_model(|m| m.light = l)
    }

    pub fn set_outline(&mut self, o: Option<Rgba>) -> bool {
        self.edit_model(|m| m.outline = o)
    }

    /// La tabla de shading del modelo activo (las rampas de luz fijadas por color).
    pub fn set_shading(&mut self, table: ShadingTable) -> bool {
        self.edit_model(|m| m.shading = table)
    }

    pub fn set_texture(&mut self, t: Option<cs_core::ArtboardId>) -> bool {
        self.edit_model(|m| m.texture = t)
    }

    /// Una textura nueva (la plantilla de las primitivas, con la paleta del modelo), a la derecha
    /// de todo, y asignada al modelo que se edita: un solo paso de undo. El modelo sigue activo
    /// (en modo Texture se activa la textura nueva).
    pub fn new_texture(&mut self) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let model_id = self.model_id();
        let Some(owner) = self.document.artboard(model_id) else {
            return false;
        };
        let artboard = Artboard {
            id,
            name: format!("{} Texture", owner.name),
            position: (x1 + 16, y0),
            sprite: texture_template(&owner.sprite.palette),
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: None,
            model: None,
        };
        model.texture = Some(id);
        let cmd = DocCommand::Batch(vec![
            DocCommand::AddArtboard {
                index: self.document.artboards.len(),
                artboard: Box::new(artboard),
            },
            set_model_cmd(model_id, model),
        ]);
        self.execute_doc(cmd)
    }

    /// Cuántos frames tiene la textura del modelo que se edita (0 sin textura o si no es un
    /// modelo).
    pub fn model_texture_frames(&self) -> usize {
        self.edited_model()
            .and_then(|m| m.texture)
            .filter(|&id| id != self.model_id())
            .and_then(|id| self.document.artboard(id))
            .filter(|t| t.model.is_none())
            .map_or(0, |t| t.sprite.frames.len())
    }

    /// Hornea el modelo que se edita (ver [`cs_render::bake_model`]) en un artboard normal
    /// nuevo, `"{modelo} · bake"`, a la derecha de todo: un solo paso de undo. El activo no
    /// cambia.
    pub fn bake(&mut self, options: BakeOptions) -> bool {
        self.bake_artboard(self.model_id(), options)
    }

    /// Como [`Self::bake`], pero del modelo `id` aunque no sea el activo.
    pub fn bake_artboard(&mut self, id: ArtboardId, options: BakeOptions) -> bool {
        let Some((mut sprite, tags)) = cs_render::bake_model(&self.document, id, &options) else {
            return false;
        };
        sprite.tags = tags;
        let Some(model) = self.document.artboard(id) else {
            return false;
        };
        // A la derecha de todo (como un modelo nuevo): los bakes seguidos no se pisan.
        let right = self.document.bounds().map_or(0, |(_, (x1, _))| x1);
        let position = (right + 16, model.position.1);
        let artboard = Artboard {
            id: self.document.next_artboard_id(),
            name: format!("{} · bake", model.name),
            position,
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: None,
            model: None,
        };
        let cmd = DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        };
        self.named("Bake", |e| e.execute_doc(cmd))
    }

    fn edit_model(&mut self, f: impl FnOnce(&mut Model3D)) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        f(&mut model);
        self.set_model(model)
    }

    /// El modelo nuevo del que se edita (el del workspace en modo Texture, no la textura activa).
    pub(super) fn set_model(&mut self, model: Model3D) -> bool {
        if self.edited_model() == Some(&model) {
            return false;
        }
        self.execute_doc(set_model_cmd(self.model_id(), model))
    }
}

fn set_model_cmd(id: cs_core::ArtboardId, model: Model3D) -> DocCommand {
    DocCommand::SetModel {
        id,
        model: Some(Box::new(model)),
    }
}

/// Un ángulo en décimas de grado llevado a `-1800..1800` (la misma orientación).
pub fn wrap_angle(tenths: i32) -> i32 {
    (tenths + 1800).rem_euclid(3600) - 1800
}

impl Editor {
    /// Elige el objeto del modelo que se edita que se ve en el pixel local `(x, y)` (ninguno si
    /// ahí no hay nada).
    pub fn pick_model_object(&mut self, (x, y): (i32, i32)) {
        let Some(artboard) = self
            .document
            .artboard(self.model_id())
            .filter(|a| a.model.is_some())
        else {
            return;
        };
        let hit = (x >= 0 && y >= 0)
            .then(|| cs_render::model_pick(&self.document, artboard, x as u32, y as u32))
            .flatten();
        self.select_model_object(hit);
    }
}
