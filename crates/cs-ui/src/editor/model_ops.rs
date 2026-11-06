//! Lo de los modelos 3D en el editor: crear uno, sus objetos, la cámara, la luz, el contorno y
//! la textura y la tabla de shading. Cada cambio es un `SetModel` con el modelo entero (un paso de undo).

use cs_3d::{Primitive, UvRect, primitive_mesh};
use cs_commands::model::texture_template;
use cs_commands::{Command, DocCommand};
use cs_core::{
    Artboard, ArtboardId, Camera3D, FIXED, LayerKind, Light3D, Model3D, Object3D, PixelValue,
    Projection, Rgba, ShadingTable, Transform3D, Vec3i,
};
use cs_render::BakeOptions;

use super::Editor;
use super::texture_mode::TEXTURE_SIDES;

/// El aire al encuadrar el modelo (Home): su esfera ocupa 1 / `FRAME_AIR` de lo que se ve.
const FRAME_AIR: f32 = 1.25;

/// El centro de la caja de todo `model` en el mundo (en 1/16, redondeado); `None` sin vértices.
pub(crate) fn model_center(model: &Model3D) -> Option<Vec3i> {
    let c = cs_3d::center(cs_3d::model_bounds(model)?);
    Some(Vec3i {
        x: c[0].round() as i32,
        y: c[1].round() as i32,
        z: c[2].round() as i32,
    })
}

/// `camera` mirando al centro de `model`, desde la distancia en la que todo el modelo (su esfera)
/// se ve con aire en un render de `size`; el giro, la altura y la proyección no cambian. `None`
/// si el modelo no tiene vértices.
pub(crate) fn framed_camera(
    camera: Camera3D,
    model: &Model3D,
    size: (u32, u32),
) -> Option<Camera3D> {
    let (lo, hi) = cs_3d::model_bounds(model)?;
    let target = model_center(model)?;
    let half = [0, 1, 2].map(|i| (hi[i] - lo[i]) / 2.0);
    let radius = (half[0] * half[0] + half[1] * half[1] + half[2] * half[2]).sqrt();
    let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
    let need = radius * FRAME_AIR;
    let distance = match camera.projection {
        // Lo que se ve: `distance` hacia arriba y abajo del objetivo, y eso por `aspect` a los
        // lados.
        Projection::Orthographic => need / aspect.min(1.0),
        // La esfera entra en el cono más angosto (el vertical o el horizontal).
        Projection::Perspective { fov } => {
            let vertical = (fov.clamp(10, 1700) as f32 / 20.0).to_radians();
            let horizontal = (vertical.tan() * aspect).atan();
            need / vertical.min(horizontal).sin()
        }
    };
    Some(Camera3D {
        distance: (distance.round() as i32).max(FIXED),
        target,
        ..camera
    })
}

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
        self.model_group = None;
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
        // Durante un Extrude la selección es la de lo extruido: no cambia.
        if self.extruding() {
            return;
        }
        self.model_object = i;
        self.model_group = None;
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
        // Entra en el grupo elegido en el outliner o, si no, en el del objeto elegido.
        let groups = model.groups.len();
        let parent = self.model_group.filter(|&g| g < groups).or_else(|| {
            self.selected_object()
                .and_then(|o| model.object_parents(o).first().copied())
        });
        model.push_object(
            Object3D {
                name: p.name().to_owned(),
                mesh: primitive_mesh(&p),
                transform: Transform3D::default(),
                double_sided: false,
                kind: Some(p.kind()),
                hidden: false,
                locked: false,
            },
            parent,
        );
        let last = model.objects.len() - 1;
        if !self.named("Add Object", |e| {
            e.set_placed_model(model, &[last], NewPixels::Template)
        }) {
            return false;
        }
        self.model_object = Some(last);
        self.model_group = None;
        true
    }

    /// Los objetos que duplica Duplicate Object: en modo Object, los elegidos en las vistas; si
    /// no, el de la pestaña elegida.
    fn objects_to_duplicate(&self) -> Vec<usize> {
        let Some(model) = self.edited_model() else {
            return Vec::new();
        };
        let sel = &self.model_edit.selection;
        let chosen: Vec<usize> = match sel.mode {
            cs_3d::SelectMode::Object if self.model_edit.artboard == Some(self.model_id()) => sel
                .items
                .iter()
                .filter_map(|it| match it {
                    cs_3d::Item::Object(o) => Some(*o),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        let chosen = if chosen.is_empty() {
            self.selected_object().into_iter().collect()
        } else {
            chosen
        };
        chosen
            .into_iter()
            .filter(|&o| o < model.objects.len())
            .collect()
    }

    /// Si hay algo que duplicar (ver [`Self::duplicate_model_objects`]).
    pub fn can_duplicate_model_objects(&self) -> bool {
        !self.extruding() && !self.objects_to_duplicate().is_empty()
    }

    /// Duplicate Object (Ctrl+J o el botón de la tira de pestañas): una copia de cada objeto
    /// elegido en su mismo lugar, al final de la lista y con " copy" en el nombre, en un paso de
    /// undo. Quedan elegidas las copias (la última, en su pestaña).
    pub fn duplicate_model_objects(&mut self) -> bool {
        if !self.can_duplicate_model_objects() {
            return false;
        }
        let chosen = self.objects_to_duplicate();
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        let first = model.objects.len();
        for &o in &chosen {
            let mut copy = model.objects[o].clone();
            copy.name = format!("{} copy", copy.name);
            // La copia queda en el mismo grupo que su original.
            let parent = model.object_parents(o).first().copied();
            model.push_object(copy, parent);
        }
        let last = model.objects.len() - 1;
        let new: Vec<usize> = (first..=last).collect();
        if !self.named("Duplicate Object", |e| {
            e.set_placed_model(model, &new, NewPixels::Copy)
        }) {
            return false;
        }
        self.model_object = Some(last);
        self.model_group = None;
        if self.model_edit.mode == super::CadMode::Model
            && self.model_edit.artboard == Some(self.model_id())
        {
            self.set_model_selection(cs_3d::Selection {
                mode: cs_3d::SelectMode::Object,
                items: (first..=last).map(cs_3d::Item::Object).collect(),
            });
        }
        true
    }

    /// Duplicate Group: copia el grupo con todo lo de adentro (objetos y subgrupos, en el mismo
    /// orden) justo después del original, en un paso de undo. Solo la copia del grupo lleva
    /// " copy" en el nombre (lo de adentro conserva el suyo). La copia queda como fila activa.
    pub fn duplicate_group(&mut self, g: usize) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        model.normalize_tree();
        if g >= model.groups.len() {
            return false;
        }
        let Some(children) = group_children(&model.tree, g) else {
            return false;
        };
        let mut copy = model.groups[g].clone();
        copy.name = format!("{} copy", copy.name);
        model.groups.push(copy);
        let ng = model.groups.len() - 1;
        let first = model.objects.len();
        let children = copy_nodes(&mut model, &children);
        let new: Vec<usize> = (first..model.objects.len()).collect();
        let _ = insert_after_group(
            &mut model.tree,
            g,
            cs_core::Node::Group {
                group: ng,
                children,
            },
        );
        if !self.named("Duplicate Group", |e| {
            e.set_placed_model(model, &new, NewPixels::Copy)
        }) {
            return false;
        }
        self.set_active_row(Some(super::Row::Group(ng)), false);
        true
    }

    pub fn remove_object(&mut self, i: usize) -> bool {
        let Some(mut model) = self.edited_model().cloned() else {
            return false;
        };
        // Lo bloqueado no se borra.
        if i >= model.objects.len() || model.object_locked(i) {
            return false;
        }
        model.remove_object(i);
        let left = model.objects.len();
        // La selección guarda índices de objeto: se acomoda ANTES del paso (que la revalida contra
        // el modelo nuevo): lo del objeto quitado se va y lo de los posteriores se corre un lugar.
        let old = self.model_edit.selection.items.clone();
        self.model_edit.selection.items = old
            .iter()
            .filter(|it| it.object() != i)
            .map(|&it| match it.object() {
                o if o > i => it.with_object(o - 1),
                _ => it,
            })
            .collect();
        // Los conteos de la selección se corren igual (ver `sync_model_edit`).
        let made_on = self.model_edit.made_on.clone();
        if i < self.model_edit.made_on.len() {
            self.model_edit.made_on.remove(i);
        }
        if !self.named("Remove Object", |e| e.set_model(model)) {
            self.model_edit.selection.items = old;
            self.model_edit.made_on = made_on;
            return false;
        }
        // Deshacer el paso vuelve a lo elegido de antes (no a lo acomodado para él).
        self.step_selection_before(old);
        // Quitar uno anterior a la elegida la deja en el mismo objeto (un lugar antes); quitar la
        // elegida (o una posterior) acomoda el índice dentro de lo que quedó.
        let selected = self.model_object.unwrap_or(0);
        let selected = if i < selected { selected - 1 } else { selected };
        self.model_object = (left > 0).then(|| selected.min(left - 1));
        self.model_group = None;
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

    /// Home: la cámara encuadra todo el modelo que se edita (ver [`framed_camera`]) en el render
    /// de la perspectiva, del tamaño del artboard del modelo. Un paso de undo.
    pub fn frame_model(&mut self) -> bool {
        let Some(model) = self.edited_model() else {
            return false;
        };
        let size = self
            .document
            .artboard(self.model_id())
            .map_or((1, 1), |a| (a.sprite.width, a.sprite.height));
        let Some(camera) = framed_camera(model.camera, model, size) else {
            return false;
        };
        self.end_group();
        self.set_camera(camera)
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

    /// Lleva las UVs de los objetos `new` de `model` a lugares libres de la textura del modelo
    /// que se edita (sin encimarse con las de los demás objetos, con [`cs_3d::MARGIN`] de aire),
    /// en orden. Si uno no cabe, prueba los lados siguientes de [`TEXTURE_SIDES`] (de doble en
    /// doble); si ni en el más grande cabe, se queda donde está. Devuelve el lado final de la
    /// textura y, por cada objeto movido, de qué caja a qué lugar se corrió (para copiar sus
    /// pixeles). Sin textura no toca nada.
    fn place_new_objects(&self, model: &mut Model3D, new: &[usize]) -> (u32, Vec<UvMove>) {
        let Some(texture) = self
            .edited_texture()
            .and_then(|id| self.document.artboard(id))
        else {
            return (0, Vec::new());
        };
        let (w, h) = (texture.sprite.width, texture.sprite.height);
        let mut side = w.min(h);
        // Los lados a los que puede crecer: los de la lista del panel, más grandes que la textura.
        let bigger = |s: u32| {
            TEXTURE_SIDES
                .filter(|v| v.is_power_of_two())
                .filter(move |&v| v > s.max(w).max(h))
        };
        let mut taken: Vec<UvRect> = (0..model.objects.len())
            .filter(|o| !new.contains(o))
            .filter_map(|o| cs_3d::object_uv_rect(model, o))
            .collect();
        let mut moves = Vec::new();
        for &o in new {
            let Some(rect) = cs_3d::object_uv_rect(model, o) else {
                continue;
            };
            let spot = std::iter::once(side)
                .chain(bigger(side))
                .find_map(|s| cs_3d::find_free(&taken, rect.2, rect.3, s as i32).map(|at| (s, at)));
            let Some((s, at)) = spot else {
                taken.push(rect);
                continue;
            };
            side = s;
            *model = cs_3d::place_object(model, o, at);
            taken.push((at.0, at.1, rect.2, rect.3));
            moves.push((rect, at));
        }
        (side.max(w.min(h)), moves)
    }

    /// Pone `model` (con objetos nuevos `new`) en el modelo que se edita, cada objeto nuevo en
    /// su lugar de la textura (ver [`Self::place_new_objects`]): si no cupo, la textura crece
    /// (conserva sus pixeles), y su caja nueva se llena según `fill`. Todo en un comando.
    fn set_placed_model(&mut self, mut model: Model3D, new: &[usize], fill: NewPixels) -> bool {
        let (side, moves) = self.place_new_objects(&mut model, new);
        self.set_model_moving_pixels(model, side, &moves, fill)
    }

    /// Pone `model` en el modelo que se edita y, en el mismo comando, su textura pasa a lado
    /// `side` si es más chica (conserva sus pixeles, anclada arriba a la izquierda) y cada caja
    /// de `moves` se llena en su lugar nuevo según `fill` (los pixeles se leen de la textura de
    /// antes de cualquier cambio; los de las cajas viejas se quedan). Sin textura, solo el modelo.
    pub(super) fn set_model_moving_pixels(
        &mut self,
        mut model: Model3D,
        side: u32,
        moves: &[UvMove],
        fill: NewPixels,
    ) -> bool {
        let Some(texture) = self
            .edited_texture()
            .and_then(|id| self.document.artboard(id))
        else {
            return self.set_model(model);
        };
        let sprite = &texture.sprite;
        let mut cmds = Vec::new();
        if side > sprite.width.min(sprite.height) {
            cmds.push(DocCommand::OnArtboard {
                id: texture.id,
                cmd: Command::ResizeCanvas {
                    width: side,
                    height: side,
                    offset: (0, 0),
                },
            });
        }
        let mut images = sprite
            .layers
            .iter()
            .filter(|l| l.kind == LayerKind::Image && !l.locked);
        // De qué sprite se leen los pixeles y en qué capas se escriben.
        let template;
        let (source, layers): (&cs_core::Sprite, Vec<_>) = match fill {
            NewPixels::Copy => (sprite, images.map(|l| (l.id, l.id)).collect()),
            NewPixels::Template => {
                // Con la paleta del modelo, como una textura nueva (ver `new_texture`).
                let palette = self
                    .document
                    .artboard(self.model_id())
                    .map_or(&sprite.palette, |a| &a.sprite.palette);
                template = texture_template(palette);
                let from = template.layers[0].id;
                (
                    &template,
                    images.next().map(|l| (from, l.id)).into_iter().collect(),
                )
            }
        };
        for (from, to) in layers {
            for frame in 0..sprite.frames.len() {
                let read = if fill == NewPixels::Copy { frame } else { 0 };
                // Todos los pixeles de la caja, también los transparentes: queda igual.
                let pixels: Vec<(i32, i32, PixelValue)> = moves
                    .iter()
                    .flat_map(|&((x, y, w, h), (nx, ny))| {
                        (0..h).flat_map(move |dy| (0..w).map(move |dx| (x, y, nx, ny, dx, dy)))
                    })
                    .map(|(x, y, nx, ny, dx, dy)| {
                        (nx + dx, ny + dy, source.pixel(from, read, x + dx, y + dy))
                    })
                    .filter(|&(_, _, v)| sprite.color_mode.accepts(v))
                    .collect();
                if !pixels.is_empty() {
                    cmds.push(DocCommand::OnArtboard {
                        id: texture.id,
                        cmd: Command::DrawPixels {
                            layer: to,
                            frame,
                            pixels,
                        },
                    });
                }
            }
        }
        if cmds.is_empty() {
            return self.set_model(model);
        }
        model.normalize_tree();
        cmds.push(set_model_cmd(self.model_id(), model));
        self.execute_doc(DocCommand::Batch(cmds))
    }

    /// El modelo nuevo del que se edita (el del workspace en modo Texture, no la textura activa).
    pub(super) fn set_model(&mut self, mut model: Model3D) -> bool {
        model.normalize_tree();
        if self.edited_model() == Some(&model) {
            return false;
        }
        self.execute_doc(set_model_cmd(self.model_id(), model))
    }
}

/// De qué caja de la textura a qué lugar se corrieron las UVs de un objeto.
pub(super) type UvMove = (UvRect, (i32, i32));

/// Con qué se llena la caja nueva de un objeto en la textura (ver `Editor::set_placed_model`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NewPixels {
    /// La plantilla de las primitivas (lo que tiene una textura nueva en su zona): un objeto
    /// agregado se ve como en un modelo nuevo y no transparente.
    Template,
    /// Los pixeles de su caja vieja (en todas las capas de imagen y frames): un duplicado se ve
    /// igual que su original, y un objeto reacomodado (Pack UVs) igual que antes. Las capas
    /// bloqueadas se saltan (no se escribe en ellas): un duplicado con una capa bloqueada no
    /// lleva lo de esa capa a su caja nueva. Pack UVs no llega aquí con capas bloqueadas (ver
    /// [`Editor::texture_layers_locked`]).
    Copy,
}

/// Los hijos del grupo `g` en el árbol.
fn group_children(nodes: &[cs_core::Node], g: usize) -> Option<Vec<cs_core::Node>> {
    nodes.iter().find_map(|n| match n {
        cs_core::Node::Group { group, children } if *group == g => Some(children.clone()),
        cs_core::Node::Group { children, .. } => group_children(children, g),
        cs_core::Node::Object(_) => None,
    })
}

/// Copia `nodes` al modelo (objetos y grupos nuevos al final, con el mismo nombre) y devuelve
/// los nodos de las copias.
fn copy_nodes(model: &mut Model3D, nodes: &[cs_core::Node]) -> Vec<cs_core::Node> {
    nodes
        .iter()
        .map(|n| match n {
            cs_core::Node::Object(o) => {
                model.objects.push(model.objects[*o].clone());
                cs_core::Node::Object(model.objects.len() - 1)
            }
            cs_core::Node::Group { group, children } => {
                model.groups.push(model.groups[*group].clone());
                let ng = model.groups.len() - 1;
                cs_core::Node::Group {
                    group: ng,
                    children: copy_nodes(model, children),
                }
            }
        })
        .collect()
}

/// Mete `new` justo después del nodo del grupo `g` (en su misma lista). Si no lo encuentra, lo
/// devuelve.
fn insert_after_group(
    nodes: &mut Vec<cs_core::Node>,
    g: usize,
    new: cs_core::Node,
) -> Option<cs_core::Node> {
    if let Some(at) = nodes
        .iter()
        .position(|n| matches!(n, cs_core::Node::Group { group, .. } if *group == g))
    {
        nodes.insert(at + 1, new);
        return None;
    }
    let mut new = new;
    for n in nodes.iter_mut() {
        if let cs_core::Node::Group { children, .. } = n {
            new = insert_after_group(children, g, new)?;
        }
    }
    Some(new)
}

fn set_model_cmd(id: cs_core::ArtboardId, model: Model3D) -> DocCommand {
    DocCommand::SetModel {
        id,
        model: Some(Box::new(model)),
    }
}

/// Hasta dónde llegan, en la barrita del objeto, la posición (en 1/16: ±256 unidades), la
/// rotación (en grados) y la escala (en %).
pub const POSITION_MAX: i32 = 256 * cs_core::FIXED;
pub const ROTATION_MAX: i32 = 360;
pub const SCALE_MAX: i32 = 1000;

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
        // Afuera del artboard también: la vista Persp llena su cuadro.
        let hit = cs_render::model_pick(&self.document, artboard, x, y);
        self.select_model_object(hit);
    }
}
