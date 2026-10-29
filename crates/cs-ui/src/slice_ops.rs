//! Slices (hitboxes, pivotes) desde el editor y el panel.

use cs_commands::Command;
use cs_core::{Slice, SliceKey};
use cs_tools::Point;

use crate::editor::Editor;

/// Centro 9-slice inicial (relativo al slice): el tercio del medio en cada eje, al menos 1 px.
pub fn default_center(w: u32, h: u32) -> (i32, i32, u32, u32) {
    let axis = |size: u32| {
        let edge = size / 3;
        (edge as i32, size.saturating_sub(2 * edge).max(1))
    };
    let ((x, cw), (y, ch)) = (axis(w), axis(h));
    (x, y, cw, ch)
}

impl Editor {
    /// Slice de más arriba que contiene `p` en el frame actual.
    pub fn slice_at(&self, p: Point) -> Option<usize> {
        let frame = self.frame;
        self.sprite()
            .slices
            .iter()
            .enumerate()
            .rev()
            .find(|(_, s)| {
                s.key_at(frame).is_some_and(|k| {
                    let (x, y, w, h) = k.bounds;
                    p.0 >= x && p.1 >= y && p.0 < x + w as i32 && p.1 < y + h as i32
                })
            })
            .map(|(i, _)| i)
    }

    /// Selecciona el slice de más arriba que contiene `p` en el frame actual (si hay).
    pub fn select_slice_at(&mut self, p: Point) {
        if let Some(i) = self.slice_at(p) {
            if self.slice != Some(i) {
                self.end_group();
            }
            self.slice = Some(i);
        }
    }

    pub fn selected_slice(&self) -> Option<&Slice> {
        self.sprite().slices.get(self.slice?)
    }

    /// Cambia el slice seleccionado con `f` (un paso de undo si algo cambió).
    pub fn update_slice(&mut self, f: impl FnOnce(&mut Slice)) {
        self.update_slice_grouped(None, f);
    }

    /// `update_slice` como parte del gesto `group` (un solo paso de undo por gesto).
    pub fn update_slice_grouped(&mut self, group: Option<u64>, f: impl FnOnce(&mut Slice)) {
        let Some(index) = self.slice else {
            return;
        };
        let mut slices = self.sprite().slices.clone();
        let Some(slice) = slices.get_mut(index) else {
            return;
        };
        f(slice);
        if slices != self.sprite().slices {
            let cmd = Command::SetSlices(slices);
            match group {
                Some(g) => self.execute_grouped(cmd, g),
                None => self.execute(cmd),
            };
        }
    }

    /// Cambia la clave del slice seleccionado en el frame actual (la crea si no había).
    pub fn update_slice_key(&mut self, f: impl FnOnce(&mut SliceKey)) {
        self.update_slice_key_grouped(None, f);
    }

    /// `update_slice_key` como parte del gesto `group` (un solo paso de undo por gesto).
    pub fn update_slice_key_grouped(&mut self, group: Option<u64>, f: impl FnOnce(&mut SliceKey)) {
        let frame = self.frame;
        self.update_slice_grouped(group, |s| {
            let Some(mut key) = s.key_at(frame).copied() else {
                return;
            };
            key.frame = frame;
            f(&mut key);
            s.set_key(key);
        });
    }

    /// Quita la clave del frame actual (vuelve a valer la anterior). La primera no se quita.
    pub fn remove_slice_key(&mut self) {
        let frame = self.frame;
        self.update_slice(|s| {
            if s.keys.first().is_some_and(|k| k.frame != frame) {
                s.keys.retain(|k| k.frame != frame);
            }
        });
    }

    /// Copia del slice seleccionado en el mismo lugar, encima de todo y seleccionada.
    pub fn duplicate_slice(&mut self) {
        let Some(slice) = self.selected_slice().cloned() else {
            return;
        };
        let mut slices = self.sprite().slices.clone();
        slices.push(Slice {
            name: format!("{} Copy", slice.name),
            ..slice
        });
        let index = slices.len() - 1;
        if self.execute(Command::SetSlices(slices)) {
            self.end_group();
            self.slice = Some(index);
        }
    }

    /// Slice nuevo ("Slice N", el primer N libre) del tamaño de la selección o, sin selección,
    /// de todo el lienzo; queda seleccionado.
    pub fn add_slice(&mut self) {
        let sprite = self.sprite();
        let bounds = match self.selection.as_ref().and_then(|m| m.bounds()) {
            Some(((x0, y0), (x1, y1))) => (x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32),
            None => (0, 0, sprite.width, sprite.height),
        };
        let mut slices = sprite.slices.clone();
        let n = (1..)
            .find(|n| !slices.iter().any(|s| s.name == format!("Slice {n}")))
            .expect("free name");
        slices.push(Slice {
            name: format!("Slice {n}"),
            color: cs_tools::SLICE_COLOR,
            text: String::new(),
            keys: vec![SliceKey {
                frame: 0,
                bounds,
                center: None,
                pivot: None,
            }],
        });
        let index = slices.len() - 1;
        if self.execute(Command::SetSlices(slices)) {
            self.end_group();
            self.slice = Some(index);
        }
    }

    pub fn delete_slice(&mut self) {
        let Some(index) = self.slice else {
            return;
        };
        let mut slices = self.sprite().slices.clone();
        if index < slices.len() {
            slices.remove(index);
            self.execute(Command::SetSlices(slices));
            self.slice = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;
    use crate::editor::ToolKind;

    #[test]
    fn default_center_is_the_middle_third() {
        assert_eq!(default_center(9, 6), (3, 2, 3, 2));
        assert_eq!(default_center(10, 2), (3, 0, 4, 2));
        assert_eq!(default_center(1, 1), (0, 0, 1, 1));
    }

    fn drag(e: &mut Editor, a: Point, b: Point) {
        let at = |p: Point| (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
        e.pointer_press(at(a), false, false, 0.1);
        e.pointer_drag(at(b));
        e.pointer_release();
    }

    #[test]
    fn create_select_edit_and_delete_slices() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.insert_empty_frame();
        e.set_frame(0);
        e.set_tool(ToolKind::Slice);
        drag(&mut e, (1, 1), (4, 4));
        assert_eq!(e.slice, Some(0), "new slice is selected");
        drag(&mut e, (8, 8), (10, 12));
        assert_eq!(e.slice, Some(1));
        // Clic sobre el primero lo selecciona y arrastrarlo lo mueve.
        drag(&mut e, (2, 2), (3, 2));
        assert_eq!(e.slice, Some(0));
        assert_eq!(e.selected_slice().unwrap().keys[0].bounds, (2, 1, 4, 4));
        // Pivote solo en el frame 1.
        e.set_frame(1);
        e.update_slice_key(|k| k.pivot = Some((2, 4)));
        let s = e.selected_slice().unwrap().clone();
        assert_eq!(s.key_at(0).unwrap().pivot, None);
        assert_eq!(s.key_at(1).unwrap().pivot, Some((2, 4)));
        e.remove_slice_key();
        assert_eq!(e.selected_slice().unwrap().keys.len(), 1);
        e.update_slice(|s| s.name = "hitbox".into());
        assert_eq!(e.sprite().slices[0].name, "hitbox");
        e.delete_slice();
        assert_eq!(e.sprite().slices.len(), 1);
        assert_eq!(e.slice, None);
        e.undo();
        assert_eq!(e.sprite().slices.len(), 2);
    }

    #[test]
    fn slice_at_finds_the_topmost_and_duplicate_selects_the_copy() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.set_tool(ToolKind::Slice);
        drag(&mut e, (1, 1), (6, 6));
        drag(&mut e, (10, 10), (4, 4));
        assert_eq!(e.slice_at((5, 5)), Some(1));
        assert_eq!(e.slice_at((2, 2)), Some(0));
        assert_eq!(e.slice_at((12, 12)), None);
        e.slice = Some(0);
        e.duplicate_slice();
        assert_eq!(e.slice, Some(2));
        let slices = &e.sprite().slices;
        assert_eq!(slices[2].name, format!("{} Copy", slices[0].name));
        assert_eq!(slices[2].keys, slices[0].keys);
        assert_eq!(e.slice_at((2, 2)), Some(2), "the copy is on top");
        e.undo();
        assert_eq!(e.sprite().slices.len(), 2);
    }

    #[test]
    fn grouped_slice_key_edits_undo_as_one_step() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.set_tool(ToolKind::Slice);
        drag(&mut e, (1, 1), (4, 4));
        let original = e.sprite().slices.clone();
        for x in 2..5 {
            e.update_slice_key_grouped(Some(1), |k| k.bounds.0 = x);
        }
        assert_eq!(e.selected_slice().unwrap().keys[0].bounds.0, 4);
        e.undo();
        assert_eq!(e.sprite().slices, original);
    }
}
