//! Layer → Duplicate, Merge Down y Flatten, como en Aseprite. Cada una arma el sprite nuevo y lo
//! deja en un solo paso de undo.

use std::collections::HashMap;
use std::sync::Arc;

use cs_core::{
    Cel, ColorMode, Image, ImageId, Layer, LayerId, LayerKind, PixelValue, Rgba, Sprite,
};

use crate::editor::Editor;

impl Editor {
    /// La capa de abajo de la actual, en su mismo grupo, si las dos son de imagen (Merge Down).
    pub fn merge_target(&self) -> Option<LayerId> {
        let sprite = self.sprite();
        let index = sprite.layer_index(self.layer)?;
        let upper = &sprite.layers[index];
        let lower = sprite.layers[..index]
            .iter()
            .rev()
            .find(|l| l.parent == upper.parent)?;
        (upper.kind == LayerKind::Image && lower.kind == LayerKind::Image && !lower.locked)
            .then_some(lower.id)
    }

    /// Layer → Duplicate: una copia de la capa actual (con sus hijos, si es un grupo) justo
    /// arriba de ella, con sus propias imágenes (los cels vinculados de la copia siguen
    /// vinculados entre sí). Queda elegida.
    pub fn duplicate_layer(&mut self) -> bool {
        self.commit_floating();
        let sprite = self.sprite();
        let Some(range) = sprite.subtree_range(self.layer) else {
            return false;
        };
        let mut out = sprite.clone();
        let mut next_image = sprite.next_image_id().0;
        let mut layer_ids: HashMap<LayerId, LayerId> = HashMap::new();
        let mut image_ids: HashMap<ImageId, ImageId> = HashMap::new();
        let mut copies: Vec<Layer> = Vec::new();
        let first = sprite.next_layer_id().0;
        for (id, layer) in (first..).zip(&sprite.layers[range.clone()]) {
            layer_ids.insert(layer.id, LayerId(id));
        }
        // Los hijos van al grupo copiado; la copia de arriba, al mismo grupo que la original.
        for layer in &sprite.layers[range.clone()] {
            let mut copy = layer.clone();
            copy.id = layer_ids[&layer.id];
            if layer.id == self.layer {
                copy.name = format!("{} Copy", layer.name);
            } else {
                copy.parent = layer
                    .parent
                    .map(|p| layer_ids.get(&p).copied().unwrap_or(p));
            }
            copies.push(copy);
        }
        for (&(layer, frame), cel) in &sprite.cels {
            let Some(&copy) = layer_ids.get(&layer) else {
                continue;
            };
            let image = *image_ids.entry(cel.image).or_insert_with(|| {
                let id = ImageId(next_image);
                next_image += 1;
                if let Some(img) = sprite.image(cel.image) {
                    out.images.insert(id, img.clone());
                }
                if let Some(grid) = sprite.tilemap(cel.image) {
                    out.tilemaps.insert(id, grid.clone());
                }
                id
            });
            out.cels.insert((copy, frame), Cel { image, ..*cel });
        }
        let new_layer = layer_ids[&self.layer];
        out.layers.splice(range.end..range.end, copies);
        self.named("Duplicate Layer", |e| {
            e.replace_sprite_with(out, Some(new_layer))
        })
    }

    /// Layer → Merge Down: la capa actual se compone sobre la de abajo (con su opacidad, su
    /// modo, su máscara y sus cels) en todos los frames y desaparece; queda elegida la de abajo.
    pub fn merge_down(&mut self) -> bool {
        self.commit_floating();
        let Some(lower) = self.merge_target() else {
            return false;
        };
        let sprite = self.sprite();
        let upper = self.layer;
        // Un sprite con solo las dos capas: la de abajo tal cual se pinta (sin su opacidad, su
        // modo ni su máscara, que se quedan en la capa) y la de arriba con todo lo suyo.
        let mut pair = sprite.clone();
        pair.layers = [lower, upper]
            .iter()
            .filter_map(|&id| sprite.layer(id).cloned())
            .map(|mut l| {
                l.parent = None;
                l.visible = true;
                if l.id == lower {
                    l.opacity = 255;
                    l.blend = cs_core::BlendMode::Normal;
                    l.mask = None;
                    l.clip = false;
                }
                l
            })
            .collect();
        let mut out = sprite.clone();
        let mut next_image = sprite.next_image_id().0;
        for frame in 0..sprite.frames.len() {
            out.cels.remove(&(upper, frame));
            let had = |id| sprite.cel(id, frame).is_some();
            if !had(lower) && !had(upper) {
                continue;
            }
            let pixels = cs_render::render_frame(&pair, frame);
            match trimmed_image(sprite, &pixels) {
                Some((position, image)) => {
                    let id = ImageId(next_image);
                    next_image += 1;
                    out.images.insert(id, Arc::new(image));
                    let opacity = 255;
                    out.cels.insert(
                        (lower, frame),
                        Cel {
                            position,
                            opacity,
                            image: id,
                        },
                    );
                }
                None => {
                    out.cels.remove(&(lower, frame));
                }
            }
        }
        out.layers.retain(|l| l.id != upper);
        // Las imágenes de las dos capas que ya no usa nadie.
        let before: Vec<ImageId> = sprite
            .cels
            .iter()
            .filter(|((l, _), _)| *l == upper || *l == lower)
            .map(|(_, c)| c.image)
            .collect();
        prune_images(&mut out, &before);
        self.named("Merge Down", |e| e.replace_sprite_with(out, Some(lower)))
    }

    /// Layer → Flatten: todo lo que se ve (instancias y tilemaps incluidos) queda en una sola
    /// capa de imagen, frame por frame.
    pub fn flatten(&mut self) -> bool {
        self.commit_floating();
        let sprite = self.sprite();
        if sprite.layers.len() <= 1 && sprite.layers.iter().all(|l| l.kind == LayerKind::Image) {
            return false;
        }
        let frames = 0..sprite.frames.len();
        let visible = cs_render::materialize(&self.document, sprite, frames.clone());
        let id = LayerId(1);
        let mut out = sprite.clone();
        out.layers = vec![Layer::new_image(id, "Flattened")];
        out.cels.clear();
        out.images.clear();
        out.tilemaps.clear();
        let mut next_image = 1;
        for frame in frames {
            let pixels = cs_render::render_frame(&visible, frame);
            if let Some((position, image)) = trimmed_image(sprite, &pixels) {
                let image_id = ImageId(next_image);
                next_image += 1;
                out.images.insert(image_id, Arc::new(image));
                out.cels.insert(
                    (id, frame),
                    Cel {
                        position,
                        opacity: 255,
                        image: image_id,
                    },
                );
            }
        }
        self.named("Flatten", |e| e.replace_sprite_with(out, Some(id)))
    }

    /// Deja `sprite` como el del artboard activo (un paso de undo) y elige `layer`.
    fn replace_sprite_with(&mut self, sprite: Sprite, layer: Option<LayerId>) -> bool {
        let done = self.execute_doc(cs_commands::DocCommand::SetSprite {
            id: self.active,
            sprite: Box::new(sprite),
        });
        if done && let Some(layer) = layer {
            self.select_layer(layer);
        }
        done
    }
}

/// Lo pintado de `pixels` (el lienzo entero de `sprite`) recortado a su caja, en el modo de
/// color del sprite. `None` si no hay nada.
fn trimmed_image(sprite: &Sprite, pixels: &[Rgba]) -> Option<((i32, i32), Image)> {
    let w = sprite.width as usize;
    let painted = |i: usize| pixels[i].a > 0;
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for i in (0..pixels.len()).filter(|&i| painted(i)) {
        let (x, y) = (i % w, i / w);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    if x0 == usize::MAX {
        return None;
    }
    let (iw, ih) = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
    let mut image = Image::new(iw, ih, sprite.color_mode);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let c = pixels[y * w + x];
            let value = match sprite.color_mode {
                _ if c.a == 0 => sprite.color_mode.transparent(),
                ColorMode::Rgba => PixelValue::Rgba(c),
                ColorMode::Indexed { transparent_index } => {
                    PixelValue::Index(cs_io::import::nearest_index(sprite, c, transparent_index))
                }
            };
            image
                .set((x - x0) as u32, (y - y0) as u32, value)
                .expect("same mode");
        }
    }
    Some(((x0 as i32, y0 as i32), image))
}

/// Quita de `candidates` las imágenes que ya no usa ningún cel.
fn prune_images(sprite: &mut Sprite, candidates: &[ImageId]) {
    let used: std::collections::HashSet<ImageId> = sprite.cels.values().map(|c| c.image).collect();
    for id in candidates.iter().filter(|id| !used.contains(id)) {
        sprite.images.remove(id);
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::{Command, LayerProps};
    use cs_core::{ColorMode, LayerKind};

    use super::*;

    const RED: Rgba = Rgba::rgb(255, 0, 0);
    const BLUE: Rgba = Rgba::rgb(0, 0, 255);

    fn paint(e: &mut Editor, layer: LayerId, x: i32, c: Rgba) {
        e.execute(Command::DrawPixels {
            layer,
            frame: 0,
            pixels: vec![(x, 0, PixelValue::Rgba(c))],
        });
    }

    fn at(e: &Editor, layer: LayerId, x: i32) -> PixelValue {
        e.sprite().pixel(layer, 0, x, 0)
    }

    /// Dos capas: la 1 (abajo) con rojo en x = 0 y la 2 (arriba, elegida) con azul en x = 1.
    fn two_layers() -> (Editor, LayerId, LayerId) {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let lower = e.layer;
        paint(&mut e, lower, 0, RED);
        e.add_layer(LayerKind::Image);
        let upper = e.layer;
        paint(&mut e, upper, 1, BLUE);
        (e, lower, upper)
    }

    #[test]
    fn duplicate_copies_the_layer_above_it_with_its_own_images() {
        let (mut e, lower, _) = two_layers();
        e.select_layer(lower);
        let steps = e.steps_done();
        assert!(e.duplicate_layer());
        let copy = e.layer;
        assert_ne!(copy, lower);
        let names: Vec<_> = e.sprite().layers.iter().map(|l| l.name.as_str()).collect();
        let original = &e.sprite().layer(lower).unwrap().name.clone();
        assert_eq!(
            names[1],
            format!("{original} Copy"),
            "right above it: {names:?}"
        );
        assert_eq!(at(&e, copy, 0), PixelValue::Rgba(RED));
        paint(&mut e, copy, 0, BLUE);
        assert_eq!(at(&e, lower, 0), PixelValue::Rgba(RED), "not linked");
        e.undo();
        e.undo();
        assert_eq!(e.steps_done(), steps);
        assert_eq!(e.sprite().layers.len(), 2);
    }

    #[test]
    fn duplicating_a_group_copies_its_children_into_the_copy() {
        let (mut e, lower, _) = two_layers();
        e.add_layer(LayerKind::Group);
        let group = e.layer;
        e.move_layer(lower, cs_commands::ops::LayerDrop::Into(group));
        e.select_layer(group);
        assert!(e.duplicate_layer());
        let sprite = e.sprite();
        let copy = e.layer;
        let children: Vec<_> = sprite
            .layers
            .iter()
            .filter(|l| l.parent == Some(copy))
            .collect();
        assert_eq!(children.len(), 1, "{:?}", sprite.layers);
        assert!(sprite.is_valid_tree());
    }

    #[test]
    fn merge_down_composes_the_layer_over_the_one_below() {
        let (mut e, lower, upper) = two_layers();
        e.set_layer_props(
            upper,
            LayerProps {
                opacity: Some(128),
                ..Default::default()
            },
        );
        assert_eq!(e.merge_target(), Some(lower));
        assert!(e.merge_down());
        assert_eq!(e.sprite().layers.len(), 1);
        assert_eq!(e.layer, lower);
        assert_eq!(at(&e, lower, 0), PixelValue::Rgba(RED));
        let PixelValue::Rgba(half) = at(&e, lower, 1) else {
            panic!("rgba");
        };
        assert_eq!((half.b, half.a), (255, 128), "with its opacity");
        assert_eq!(e.history.done().last(), Some("Merge Down"));
        e.undo();
        assert_eq!(e.sprite().layers.len(), 2);
    }

    #[test]
    fn nothing_to_merge_on_the_bottom_layer() {
        let (mut e, lower, _) = two_layers();
        e.select_layer(lower);
        assert_eq!(e.merge_target(), None);
        assert!(!e.merge_down());
    }

    #[test]
    fn flatten_leaves_what_is_seen_in_one_layer() {
        let (mut e, lower, upper) = two_layers();
        e.add_layer(LayerKind::Image);
        let hidden = e.layer;
        paint(&mut e, hidden, 2, BLUE);
        e.toggle_visible(hidden);
        assert!(e.flatten());
        let sprite = e.sprite();
        assert_eq!(sprite.layers.len(), 1);
        let only = sprite.layers[0].id;
        assert_eq!(sprite.pixel(only, 0, 0, 0), PixelValue::Rgba(RED));
        assert_eq!(sprite.pixel(only, 0, 1, 0), PixelValue::Rgba(BLUE));
        assert_eq!(
            sprite.pixel(only, 0, 2, 0),
            sprite.color_mode.transparent(),
            "hidden"
        );
        e.undo();
        assert!(e.sprite().layer(lower).is_some() && e.sprite().layer(upper).is_some());
    }
}
