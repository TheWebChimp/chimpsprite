//! Lo de los modelos 3D en el editor: crear uno, sus objetos, la cámara, la luz, el contorno y
//! la textura. Cada cambio es un `SetModel` con el modelo entero (un paso de undo).

use cs_3d::{Primitive, primitive_mesh};
use cs_commands::DocCommand;
use cs_core::{Camera3D, Light3D, Model3D, Object3D, Rgba, Transform3D};

use super::Editor;

impl Editor {
    /// Un modelo 3D nuevo (con su textura) a la derecha de todo, activo y con el cubo elegido.
    pub fn new_model(&mut self, name: &str, size: (u32, u32)) -> bool {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let (id, _texture, cmd) =
            cs_commands::model::new_model_artboard(&self.document, name, size, (x1 + 16, y0));
        if !self.execute_doc(cmd) {
            return false;
        }
        self.activate(id);
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

    /// Fuente o modelo: sus frames, capas, modo de color y tamaño no se tocan con las acciones
    /// de siempre.
    pub fn is_structure_locked(&self) -> bool {
        self.is_font() || self.is_model()
    }

    /// El objeto elegido del modelo activo.
    pub fn selected_object(&self) -> Option<usize> {
        let count = self.model()?.objects.len();
        self.model_object.filter(|&i| i < count)
    }

    pub fn select_model_object(&mut self, i: Option<usize>) {
        self.model_object = i;
        self.revision += 1;
    }

    pub fn add_primitive(&mut self, p: Primitive) -> bool {
        let Some(mut model) = self.model().cloned() else {
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
        let Some(mut model) = self.model().cloned() else {
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
        let Some(mut model) = self.model().cloned() else {
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
        if self.model() == Some(&model) {
            return false;
        }
        let cmd = set_model_cmd(self.active, model);
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

    pub fn set_texture(&mut self, t: Option<cs_core::ArtboardId>) -> bool {
        self.edit_model(|m| m.texture = t)
    }

    fn edit_model(&mut self, f: impl FnOnce(&mut Model3D)) -> bool {
        let Some(mut model) = self.model().cloned() else {
            return false;
        };
        f(&mut model);
        self.set_model(model)
    }

    fn set_model(&mut self, model: Model3D) -> bool {
        if self.model() == Some(&model) {
            return false;
        }
        self.execute_doc(set_model_cmd(self.active, model))
    }
}

fn set_model_cmd(id: cs_core::ArtboardId, model: Model3D) -> DocCommand {
    DocCommand::SetModel {
        id,
        model: Some(Box::new(model)),
    }
}
