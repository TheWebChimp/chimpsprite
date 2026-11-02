//! Lo de los modelos 3D en el editor: crear uno, sus objetos, la cámara, la luz, el contorno y
//! la textura. Cada cambio es un `SetModel` con el modelo entero (un paso de undo).

use cs_3d::{Primitive, primitive_mesh};
use cs_commands::DocCommand;
use cs_commands::model::texture_template;
use cs_core::{Artboard, Camera3D, Light3D, Model3D, Object3D, Rgba, Transform3D};
use cs_render::BakeOptions;

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

    /// Una textura nueva (la plantilla de las primitivas, con la paleta del modelo), a la derecha
    /// de todo, y asignada al modelo activo: un solo paso de undo. El modelo sigue activo.
    pub fn new_texture(&mut self) -> bool {
        let Some(mut model) = self.model().cloned() else {
            return false;
        };
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            id,
            name: format!("{} Texture", self.artboard().name),
            position: (x1 + 16, y0),
            sprite: texture_template(&self.sprite().palette),
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
            set_model_cmd(self.active, model),
        ]);
        self.execute_doc(cmd)
    }

    /// Cuántos frames tiene la textura del modelo activo (0 sin textura o si no es un modelo).
    pub fn model_texture_frames(&self) -> usize {
        self.model()
            .and_then(|m| m.texture)
            .filter(|&id| id != self.active)
            .and_then(|id| self.document.artboard(id))
            .filter(|t| t.model.is_none())
            .map_or(0, |t| t.sprite.frames.len())
    }

    /// Hornea el modelo activo (ver [`cs_render::bake_model`]) en un artboard normal nuevo,
    /// `"{modelo} · bake"`, a la derecha del modelo: un solo paso de undo. El modelo sigue
    /// activo.
    pub fn bake(&mut self, options: BakeOptions) -> bool {
        let Some((mut sprite, tags)) = cs_render::bake_model(&self.document, self.active, &options)
        else {
            return false;
        };
        sprite.tags = tags;
        let model = self.artboard();
        let position = (
            model.position.0 + model.sprite.width as i32 + 16,
            model.position.1,
        );
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

/// Qué hacen las flechas sobre el objeto elegido de un modelo 3D (G, R y S las cambian).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelArrows {
    /// 1/4 de unidad (4/16) por flecha.
    #[default]
    Move,
    /// 15° por flecha.
    Rotate,
    /// ±10 % por flecha.
    Scale,
}

impl ModelArrows {
    pub fn label(self) -> &'static str {
        match self {
            ModelArrows::Move => "Move",
            ModelArrows::Rotate => "Rotate",
            ModelArrows::Scale => "Scale",
        }
    }
}

/// Lo que mueve una flecha en el modo Move, en 1/16 de unidad.
const ARROW_MOVE: i32 = 4;
/// Lo que gira una flecha en el modo Rotate, en décimas de grado.
const ARROW_ROTATE: i32 = 150;

impl Editor {
    pub fn model_arrows(&self) -> ModelArrows {
        self.model_arrows
    }

    pub fn set_model_arrows(&mut self, mode: ModelArrows) {
        self.model_arrows = mode;
        self.revision += 1;
    }

    /// Una flecha sobre el objeto elegido, según [`ModelArrows`]: `dx` a la derecha, `dy` hacia
    /// abajo (en pantalla). Move corre el objeto en X e Y; Rotate lo gira en Y (izquierda y
    /// derecha) y en X (arriba y abajo); Scale lo agranda con → o ↑ y lo achica con ← o ↓.
    pub fn arrow_on_object(&mut self, dx: i32, dy: i32) -> bool {
        let Some(i) = self.selected_object() else {
            return false;
        };
        let Some(mut obj) = self.model().map(|m| m.objects[i].clone()) else {
            return false;
        };
        let t = &mut obj.transform;
        match self.model_arrows {
            ModelArrows::Move => {
                t.position.x += dx * ARROW_MOVE;
                t.position.y -= dy * ARROW_MOVE;
            }
            ModelArrows::Rotate => {
                t.rotation.y += dx * ARROW_ROTATE;
                t.rotation.x += dy * ARROW_ROTATE;
            }
            ModelArrows::Scale => {
                let grow = dx - dy;
                let step = |s: i32| match grow.signum() {
                    1 => (s * 11 + 5) / 10,
                    -1 => ((s * 10 + 5) / 11).max(1),
                    _ => s,
                };
                t.scale.x = step(t.scale.x);
                t.scale.y = step(t.scale.y);
                t.scale.z = step(t.scale.z);
            }
        }
        self.set_object(i, obj)
    }
}

impl Editor {
    /// Elige el objeto del modelo activo que se ve en el pixel local `(x, y)` (ninguno si ahí
    /// no hay nada).
    pub fn pick_model_object(&mut self, (x, y): (i32, i32)) {
        if !self.is_model() {
            return;
        }
        let hit = (x >= 0 && y >= 0)
            .then(|| cs_render::model_pick(&self.document, self.artboard(), x as u32, y as u32))
            .flatten();
        self.select_model_object(hit);
    }
}
