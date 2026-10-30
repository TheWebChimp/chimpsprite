//! Los objetos de las capas de objetos en el lienzo: tocarlos, seleccionarlos, moverlos con las
//! flechas, borrarlos y su recuadro.

use cs_commands::ops;
use cs_core::{LayerId, LayerKind, ObjectId, PixelValue};

use crate::editor::Editor;

impl Editor {
    /// El objeto visible de más arriba bajo el punto de mundo (en el artboard mostrado), de
    /// cualquier capa de objetos visible y no bloqueada: `(capa, objeto)`.
    pub fn object_at(&self, world: (f32, f32)) -> Option<(LayerId, ObjectId)> {
        if self.shown_artboard_at(world) != Some(self.active) {
            return None;
        }
        let (x, y) = self.to_local(world);
        let sprite = self.sprite();
        let layers = sprite.layers.iter().rev().filter(|l| {
            l.kind == LayerKind::Objects && !l.locked && l.opacity > 0 && sprite.is_visible(l.id)
        });
        for layer in layers {
            for object in layer.objects.iter().rev() {
                // Con opacidad 0 no se ve: tampoco se toca.
                if object
                    .track
                    .key_at(self.frame)
                    .is_none_or(|k| k.opacity == 0)
                {
                    continue;
                }
                let Some(((ox, oy), image)) = cs_render::object_image(
                    &self.document,
                    sprite,
                    layer.id,
                    object.id,
                    self.frame,
                ) else {
                    continue;
                };
                let (u, v) = (x - ox, y - oy);
                if u < 0 || v < 0 {
                    continue;
                }
                if let Some(PixelValue::Rgba(c)) = image.get(u as u32, v as u32)
                    && c.a > 0
                {
                    return Some((layer.id, object.id));
                }
            }
        }
        None
    }

    /// Selecciona `object` de `layer` y activa esa capa. Con `additive` (Shift) y en la capa
    /// activa, lo agrega o lo quita de la selección; si no, la selección pasa a ser solo él.
    pub fn select_object(&mut self, layer: LayerId, object: ObjectId, additive: bool) {
        if additive && layer == self.layer {
            match self.selected_objects.iter().position(|&o| o == object) {
                Some(i) => {
                    self.selected_objects.remove(i);
                }
                None => self.selected_objects.push(object),
            }
        } else {
            self.select_layer(layer);
            if self.layer != layer {
                return;
            }
            self.selected_objects = vec![object];
        }
        // Las ediciones agrupadas eran del objeto de antes.
        self.end_group();
        self.revision += 1;
    }

    pub fn clear_object_selection(&mut self) {
        if !self.selected_objects.is_empty() {
            self.selected_objects.clear();
            self.end_group();
            self.revision += 1;
        }
    }

    /// Borra los objetos seleccionados (un paso de undo). `false` si no había ninguno.
    pub fn delete_selected_objects(&mut self) -> bool {
        let Some(cmd) = ops::remove_objects(self.sprite(), self.layer, &self.selected_objects)
        else {
            return false;
        };
        if !self.execute_named(cmd, "Delete Objects") {
            return false;
        }
        self.selected_objects.clear();
        true
    }

    /// Corre los objetos seleccionados `(dx, dy)` en el frame actual (un paso de undo). `false`
    /// si no había ninguno que mover.
    pub fn nudge_selected_objects(&mut self, dx: i32, dy: i32) -> bool {
        if self.sprite().layer(self.layer).is_none_or(|l| l.locked) {
            return false;
        }
        let cmd = ops::move_objects(
            self.sprite(),
            self.layer,
            &self.selected_objects,
            self.frame,
            dx,
            dy,
        );
        cmd.is_some_and(|cmd| self.execute_named(cmd, "Move"))
    }

    /// La caja `(x, y, ancho, alto)` (en píxeles del sprite) de lo que se ve del objeto en el
    /// frame actual, para el recuadro.
    pub fn object_bounds(&self, layer: LayerId, object: ObjectId) -> Option<(i32, i32, u32, u32)> {
        object_bounds_in(&self.document, self, layer, object)
    }

    /// Doble clic con Move en `world`: si hay un objeto, lo selecciona y entra a su fuente (al
    /// modo símbolo si es de la biblioteca; si es del lienzo, la activa). `None` si no hay
    /// objeto; si no, si entró.
    pub fn open_object_at(&mut self, world: (f32, f32)) -> Option<bool> {
        let (layer, object) = self.object_at(world)?;
        self.select_object(layer, object, false);
        Some(self.enter_instance())
    }
}

/// `object_bounds` sobre `document` (el que se muestra, con el gesto en curso aplicado).
pub(crate) fn object_bounds_in(
    document: &cs_core::Document,
    editor: &Editor,
    layer: LayerId,
    object: ObjectId,
) -> Option<(i32, i32, u32, u32)> {
    let sprite = &document.artboard(editor.active)?.sprite;
    let ((x, y), image) = cs_render::object_image(document, sprite, layer, object, editor.frame)?;
    Some((x, y, image.width(), image.height()))
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ArtboardId, ColorMode, Rgba, Sprite};
    use eframe::egui::Key;

    use super::*;
    use crate::editor::ToolKind;

    /// Host de 16×16 con 2 frames y, en la biblioteca, un símbolo de 4×4 todo rojo. Dos capas de
    /// objetos con un objeto cada una: `a` en (6, 6) y `b` (arriba) en (8, 8).
    fn scene() -> (Editor, LayerId, LayerId) {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.insert_empty_frame();
        e.set_frame(0);
        e.add_artboard(4, 4);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        let pixels = (0..16).map(|i| (i % 4, i / 4, red)).collect();
        e.execute(Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels,
        });
        assert!(e.move_active_to_library());
        e.activate(ArtboardId(1));
        e.add_instance_layer(ArtboardId(2));
        let a = e.layer;
        e.add_instance_layer(ArtboardId(2));
        let b = e.layer;
        e.update_instance_key(|k| k.position = (8, 8));
        e.history = Default::default();
        e.set_tool(ToolKind::Move);
        (e, a, b)
    }

    fn first(e: &Editor, layer: LayerId) -> ObjectId {
        e.sprite().layer(layer).unwrap().objects[0].id
    }

    fn click(e: &mut Editor, at: (f32, f32), shift: bool) {
        e.options.constrain = shift;
        e.pointer_press(at, false, false, 0.1);
        e.pointer_release();
        e.options.constrain = false;
    }

    fn position(e: &Editor, layer: LayerId, object: ObjectId, frame: usize) -> (i32, i32) {
        let track = &e
            .sprite()
            .layer(layer)
            .unwrap()
            .object(object)
            .unwrap()
            .track;
        track.key_at(frame).unwrap().position
    }

    #[test]
    fn object_at_picks_the_top_one_and_skips_hidden_or_locked_layers() {
        let (mut e, a, b) = scene();
        assert_eq!(e.object_at((9.5, 9.5)), Some((b, first(&e, b))));
        assert_eq!(e.object_at((6.5, 6.5)), Some((a, first(&e, a))));
        assert_eq!(e.object_at((1.5, 1.5)), None, "nothing drawn there");
        assert_eq!(e.object_at((40.5, 1.5)), None, "outside the artboard");
        e.document.artboards[0].sprite.layer_mut(b).unwrap().visible = false;
        assert_eq!(e.object_at((9.5, 9.5)), Some((a, first(&e, a))));
        let layer = e.document.artboards[0].sprite.layer_mut(b).unwrap();
        (layer.visible, layer.locked) = (true, true);
        assert_eq!(e.object_at((9.5, 9.5)), Some((a, first(&e, a))));
    }

    #[test]
    fn a_click_selects_the_object_and_its_layer_and_shift_adds() {
        let (mut e, a, b) = scene();
        e.select_layer(LayerId(1));
        click(&mut e, (6.5, 6.5), false);
        assert_eq!(
            (e.layer, e.selected_objects.clone()),
            (a, vec![first(&e, a)])
        );
        // Otro objeto en la capa de arriba, en (0, 0).
        let (cmd, other) = ops::add_object(e.sprite(), b, 0, ArtboardId(2), "o", (0, 0)).unwrap();
        e.execute(cmd);
        click(&mut e, (9.5, 9.5), false);
        assert_eq!(
            (e.layer, e.selected_objects.clone()),
            (b, vec![first(&e, b)])
        );
        assert_eq!(e.current_object(), Some(first(&e, b)));
        click(&mut e, (1.5, 1.5), true);
        assert_eq!(e.selected_objects, vec![first(&e, b), other]);
        click(&mut e, (9.5, 9.5), true);
        assert_eq!(e.selected_objects, vec![other], "shift again takes it out");
        assert_eq!(e.current_object(), Some(other));
        // Un clic donde no hay objetos vacía la selección.
        click(&mut e, (14.5, 1.5), false);
        assert!(e.selected_objects.is_empty());
    }

    #[test]
    fn dragging_moves_the_selection_on_the_current_frame_in_one_undo() {
        let (mut e, _, b) = scene();
        e.set_frame(1);
        let o = first(&e, b);
        e.pointer_press((9.5, 9.5), false, false, 0.1);
        e.pointer_drag((10.5, 9.5));
        e.pointer_drag((11.5, 10.5));
        assert_eq!(e.view().document.artboards[0].sprite, {
            let mut s = e.sprite().clone();
            ops::move_objects(&s, b, &[o], 1, 2, 1)
                .unwrap()
                .apply(&mut s)
                .unwrap();
            s
        });
        e.pointer_release();
        assert_eq!(position(&e, b, o, 1), (10, 9));
        assert_eq!(position(&e, b, o, 0), (8, 8), "frame 0 keeps its key");
        assert_eq!(e.history.done_count(), 1);
        e.undo();
        assert_eq!(position(&e, b, o, 1), (8, 8));
        assert_eq!(e.selected_objects, vec![o]);
    }

    #[test]
    fn alt_dragging_moves_a_copy_and_selects_it() {
        let (mut e, _, b) = scene();
        let o = first(&e, b);
        // Alt sin arrastrar no copia.
        e.pointer_press((9.5, 9.5), false, true, 0.1);
        e.pointer_release();
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        e.pointer_press((9.5, 9.5), false, true, 0.1);
        e.pointer_drag((12.5, 9.5));
        e.pointer_release();
        let objects = &e.sprite().layer(b).unwrap().objects;
        assert_eq!(objects.len(), 2);
        let copy = objects[1].id;
        assert_eq!(e.selected_objects, vec![copy]);
        assert_eq!(position(&e, b, o, 0), (8, 8), "the original stays");
        assert_eq!(position(&e, b, copy, 0), (11, 8));
        assert_eq!(e.history.done_count(), 1, "one undo step");
        e.undo();
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        assert_eq!(e.selected_objects, Vec::<ObjectId>::new());
    }

    #[test]
    fn arrows_nudge_and_delete_removes_the_selected_objects() {
        let (mut e, _, b) = scene();
        let o = first(&e, b);
        assert!(!crate::editor_key(&mut e, Key::ArrowRight, false));
        click(&mut e, (9.5, 9.5), false);
        assert!(crate::editor_key(&mut e, Key::ArrowRight, false));
        assert!(crate::editor_key(&mut e, Key::ArrowUp, true));
        assert_eq!(position(&e, b, o, 0), (9, -2));
        assert_eq!(e.object_bounds(b, o), Some((9, -2, 4, 4)));
        assert!(crate::editor_key(&mut e, Key::Delete, false));
        assert!(e.sprite().layer(b).unwrap().objects.is_empty());
        assert!(e.selected_objects.is_empty());
        e.undo();
        assert_eq!(position(&e, b, o, 0), (9, -2));
    }

    #[test]
    fn double_clicking_a_library_object_enters_its_symbol() {
        let (mut e, a, _) = scene();
        assert_eq!(e.open_object_at((1.5, 1.5)), None);
        assert_eq!(e.open_object_at((6.5, 6.5)), Some(true));
        assert!(e.in_symbol_mode());
        assert_eq!(e.active, ArtboardId(2));
        e.exit_symbol_to(0);
        assert_eq!(e.active, ArtboardId(1));
        assert_eq!(e.layer, a);
    }

    #[test]
    fn grouped_edits_of_different_objects_are_separate_undo_steps() {
        let (mut e, _, b) = scene();
        let o = first(&e, b);
        let (cmd, other) = ops::add_object(e.sprite(), b, 0, ArtboardId(2), "o", (0, 0)).unwrap();
        e.execute(cmd);
        e.selected_objects = vec![o];
        e.update_instance_key_grouped(Some(7), |k| k.position = (1, 1));
        e.selected_objects = vec![other];
        e.update_instance_key_grouped(Some(7), |k| k.position = (2, 2));
        e.undo();
        assert_eq!(position(&e, b, other, 0), (0, 0));
        assert_eq!(
            position(&e, b, o, 0),
            (1, 1),
            "the first object's edit stays"
        );
    }
}
