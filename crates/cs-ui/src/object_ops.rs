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
            let mut ids = self.selected_objects().to_vec();
            match ids.iter().position(|&o| o == object) {
                Some(i) => {
                    ids.remove(i);
                }
                None => ids.push(object),
            }
            self.set_selected_objects(ids);
        } else {
            self.select_layer(layer);
            if self.layer != layer {
                return;
            }
            self.set_selected_objects(vec![object]);
        }
        // Las ediciones agrupadas eran del objeto de antes.
        self.end_group();
        self.revision += 1;
    }

    pub fn clear_object_selection(&mut self) {
        if !self.selected_objects().is_empty() {
            self.set_selected_objects(Vec::new());
            self.end_group();
            self.revision += 1;
        }
    }

    /// Borra los objetos seleccionados (un paso de undo). `false` si no había ninguno.
    pub fn delete_selected_objects(&mut self) -> bool {
        if self.sprite().layer(self.layer).is_none_or(|l| l.locked) {
            return false;
        }
        let Some(cmd) = ops::remove_objects(self.sprite(), self.layer, self.selected_objects())
        else {
            return false;
        };
        if !self.execute_named(cmd, "Delete Objects") {
            return false;
        }
        self.set_selected_objects(Vec::new());
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
            self.selected_objects(),
            self.frame,
            dx,
            dy,
        );
        cmd.is_some_and(|cmd| self.execute_named(cmd, "Move"))
    }

    /// La caja `(x, y, ancho, alto)` (en píxeles del sprite) de lo que se ve del objeto en el
    /// frame actual, para el recuadro. Se recuerda hasta que cambia `revision`.
    pub fn object_bounds(&self, layer: LayerId, object: ObjectId) -> Option<(i32, i32, u32, u32)> {
        let mut cache = self.object_bounds_cache.borrow_mut();
        if cache.0 != self.revision {
            *cache = (self.revision, Default::default());
        }
        *cache
            .1
            .entry((self.active, self.frame, layer, object))
            .or_insert_with(|| object_bounds_in(&self.document, self, layer, object))
    }

    /// Las cajas de los recuadros a dibujar sobre `document` (el que se muestra): los objetos
    /// seleccionados, solo con Move. Durante Alt+arrastrar, las copias que se mueven.
    pub(crate) fn object_outlines(
        &self,
        document: &cs_core::Document,
    ) -> Vec<(i32, i32, u32, u32)> {
        if self.tool != crate::editor::ToolKind::Move || self.selected_objects().is_empty() {
            return Vec::new();
        }
        let preview = !std::ptr::eq(document, &self.document);
        let shown = document
            .artboard(self.active)
            .and_then(|a| a.sprite.layer(self.layer));
        let copies: Vec<ObjectId> = match (self.object_copy && preview, shown) {
            (true, Some(l)) => {
                let real = self.sprite().layer(self.layer);
                l.objects
                    .iter()
                    .map(|o| o.id)
                    .filter(|&o| real.is_none_or(|r| r.object(o).is_none()))
                    .collect()
            }
            _ => Vec::new(),
        };
        let ids = if copies.is_empty() {
            self.selected_objects()
        } else {
            &copies
        };
        ids.iter()
            .filter_map(|&o| {
                if preview {
                    object_bounds_in(document, self, self.layer, o)
                } else {
                    self.object_bounds(self.layer, o)
                }
            })
            .collect()
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
            (e.layer, e.selected_objects().to_vec()),
            (a, vec![first(&e, a)])
        );
        // Otro objeto en la capa de arriba, en (0, 0).
        let (cmd, other) = ops::add_object(e.sprite(), b, 0, ArtboardId(2), "o", (0, 0)).unwrap();
        e.execute(cmd);
        click(&mut e, (9.5, 9.5), false);
        assert_eq!(
            (e.layer, e.selected_objects().to_vec()),
            (b, vec![first(&e, b)])
        );
        assert_eq!(e.current_object(), Some(first(&e, b)));
        click(&mut e, (1.5, 1.5), true);
        assert_eq!(e.selected_objects(), vec![first(&e, b), other]);
        click(&mut e, (9.5, 9.5), true);
        assert_eq!(
            e.selected_objects(),
            vec![other],
            "shift again takes it out"
        );
        assert_eq!(e.current_object(), Some(other));
        // Un clic donde no hay objetos vacía la selección.
        click(&mut e, (14.5, 1.5), false);
        assert!(e.selected_objects().is_empty());
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
        assert_eq!(e.selected_objects(), vec![o]);
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
        assert_eq!(e.selected_objects(), vec![copy]);
        assert_eq!(position(&e, b, o, 0), (8, 8), "the original stays");
        assert_eq!(position(&e, b, copy, 0), (11, 8));
        assert_eq!(e.history.done_count(), 1, "one undo step");
        e.undo();
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        assert_eq!(e.selected_objects(), Vec::<ObjectId>::new());
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
        assert!(e.selected_objects().is_empty());
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
        e.select_object(b, o, false);
        e.update_instance_key_grouped(Some(7), |k| k.position = (1, 1));
        e.select_object(b, other, false);
        e.update_instance_key_grouped(Some(7), |k| k.position = (2, 2));
        e.undo();
        assert_eq!(position(&e, b, other, 0), (0, 0));
        assert_eq!(
            position(&e, b, o, 0),
            (1, 1),
            "the first object's edit stays"
        );
    }

    #[test]
    fn the_selection_does_not_jump_to_another_layer_with_the_same_ids() {
        let (mut e, a, b) = scene();
        assert_eq!(first(&e, a), first(&e, b), "both layers have ObjectId(1)");
        click(&mut e, (9.5, 9.5), false);
        assert_eq!((e.layer, e.selected_objects().len()), (b, 1));
        e.step_layer(false);
        assert_eq!(e.layer, a);
        assert!(e.selected_objects().is_empty());
        crate::editor_key(&mut e, Key::Delete, false);
        assert_eq!(e.sprite().layer(a).unwrap().objects.len(), 1);
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        // Tampoco vuelve con ensure_valid (un comando cualquiera) ni con capas nuevas.
        e.add_layer(LayerKind::Image);
        e.layer = b;
        e.ensure_valid();
        assert!(e.selected_objects().is_empty());
    }

    #[test]
    fn delete_respects_the_lock_and_the_palette_selection() {
        let (mut e, _, b) = scene();
        click(&mut e, (9.5, 9.5), false);
        e.document.artboards[0].sprite.layer_mut(b).unwrap().locked = true;
        assert!(!e.delete_selected_objects());
        crate::editor_key(&mut e, Key::Delete, false);
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1, "locked");
        e.document.artboards[0].sprite.layer_mut(b).unwrap().locked = false;
        e.palette_selection = Some((0, 2));
        assert!(e.palette_range().is_some_and(|r| r.end() > r.start()));
        assert!(
            !crate::editor_key(&mut e, Key::Delete, false),
            "goes to the palette"
        );
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        e.palette_selection = None;
        assert!(crate::editor_key(&mut e, Key::Delete, false));
        assert!(e.sprite().layer(b).unwrap().objects.is_empty());
    }

    #[test]
    fn arrows_skip_objects_with_a_marquee_and_keys_need_the_move_tool() {
        let (mut e, _, b) = scene();
        let o = first(&e, b);
        click(&mut e, (9.5, 9.5), false);
        e.selection = Some(cs_core::Mask::from_rect(16, 16, (0, 0), (1, 1)));
        crate::editor_key(&mut e, Key::ArrowRight, false);
        assert_eq!(position(&e, b, o, 0), (8, 8), "the marquee wins");
        e.selection = None;
        e.set_tool(ToolKind::Pencil);
        assert_eq!(e.selected_objects(), vec![o], "the selection stays");
        crate::editor_key(&mut e, Key::ArrowRight, false);
        crate::editor_key(&mut e, Key::Delete, false);
        assert_eq!(e.sprite().layer(b).unwrap().objects.len(), 1);
        assert_eq!(position(&e, b, o, 0), (8, 8));
        assert!(e.object_outlines(&e.document).is_empty(), "no outline");
        e.set_tool(ToolKind::Move);
        assert_eq!(e.object_outlines(&e.document), vec![(8, 8, 4, 4)]);
    }

    #[test]
    fn during_alt_drag_the_outline_follows_the_copy() {
        let (mut e, _, _) = scene();
        e.pointer_press((9.5, 9.5), false, true, 0.1);
        e.pointer_drag((12.5, 9.5));
        let view = e.view();
        assert_eq!(e.object_outlines(&view.document), vec![(11, 8, 4, 4)]);
        drop(view);
        e.pointer_release();
        assert_eq!(e.object_outlines(&e.document), vec![(11, 8, 4, 4)]);
    }
}
