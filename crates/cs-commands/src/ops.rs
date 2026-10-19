//! Operaciones compuestas: arman `Command`s a partir del estado actual del sprite.

use std::collections::HashMap;
use std::sync::Arc;

use cs_core::{
    ArtboardId, Cel, ImageId, InstanceKey, Layer, LayerId, LayerKind, LayerMask, Mask, PixelValue,
    Sprite,
};

use crate::Command;

/// Dónde soltar una capa respecto de otra (en términos visuales: arriba = más alto en la pila).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerDrop {
    Above(LayerId),
    Below(LayerId),
    /// Dentro de un grupo, como su hijo de más arriba.
    Into(LayerId),
}

/// Capa (o grupo) nueva justo encima de `above` (en su mismo grupo), o arriba de todo.
pub fn new_layer(sprite: &Sprite, above: Option<LayerId>, kind: LayerKind) -> Command {
    let id = sprite.next_layer_id();
    let prefix = match kind {
        LayerKind::Group => "Group",
        LayerKind::Instance => "Instance",
        LayerKind::Image => "Layer",
    };
    let mut layer = Layer::new_image(id, format!("{prefix} {}", id.0));
    layer.kind = kind;
    let (index, parent) = match above.and_then(|a| Some((sprite.subtree_range(a)?, a))) {
        Some((range, a)) => (range.end, sprite.layer(a).and_then(|l| l.parent)),
        None => (sprite.layers.len(), None),
    };
    layer.parent = parent;
    Command::AddLayer { index, layer }
}

/// Capa de instancia de `source` sobre `above`, con una key en `frame`.
pub fn new_instance_layer(
    sprite: &Sprite,
    above: Option<LayerId>,
    frame: usize,
    source: ArtboardId,
    name: &str,
    position: (i32, i32),
) -> Command {
    let Command::AddLayer { index, mut layer } = new_layer(sprite, above, LayerKind::Instance)
    else {
        unreachable!("new_layer devuelve AddLayer");
    };
    layer.name = name.to_string();
    layer.instance = Some(cs_core::InstanceTrack {
        keys: vec![InstanceKey::new(frame, source, position)],
    });
    Command::AddLayer { index, layer }
}

/// Cambia con `f` la key vigente de `layer` en `frame`; si `frame` no es keyframe, la copia ahí
/// primero. `None` si no hay key vigente o nada cambia.
pub fn update_instance_key(
    sprite: &Sprite,
    layer: LayerId,
    frame: usize,
    f: impl FnOnce(&mut InstanceKey),
) -> Option<Command> {
    let track = sprite.layer(layer)?.instance.as_ref()?;
    let mut key = *track.key_at(frame)?;
    key.frame = frame;
    f(&mut key);
    let mut new = track.clone();
    new.set_key(key);
    (new != *track).then_some(Command::SetInstanceTrack { layer, track: new })
}

/// Quita la key que empieza en `frame`. Si es la única, quita la capa.
pub fn remove_instance_key(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<Command> {
    let track = sprite.layer(layer)?.instance.as_ref()?;
    if !track.keys.iter().any(|k| k.frame == frame) {
        return None;
    }
    if track.keys.len() == 1 {
        return remove_layer_tree(sprite, layer);
    }
    let mut new = track.clone();
    new.remove_key(frame);
    Some(Command::SetInstanceTrack { layer, track: new })
}

/// Quita la capa y, si es un grupo, todo su contenido.
pub fn remove_layer_tree(sprite: &Sprite, id: LayerId) -> Option<Command> {
    let range = sprite.subtree_range(id)?;
    let removals = sprite.layers[range]
        .iter()
        .rev()
        .map(|l| Command::RemoveLayer { id: l.id })
        .collect();
    Some(Command::Batch(removals))
}

/// Mueve `id` (con sus descendientes) según `drop`. `None` si el destino es inválido.
pub fn move_layer(sprite: &Sprite, id: LayerId, drop: LayerDrop) -> Option<Command> {
    let block = sprite.subtree_range(id)?;
    let rest: Vec<&Layer> = sprite
        .layers
        .iter()
        .enumerate()
        .filter(|(i, _)| !block.contains(i))
        .map(|(_, l)| l)
        .collect();
    let target = match drop {
        LayerDrop::Above(t) | LayerDrop::Below(t) | LayerDrop::Into(t) => t,
    };
    let pos = rest.iter().position(|l| l.id == target)?;
    let subtree_end = pos
        + 1
        + rest[pos + 1..]
            .iter()
            .take_while(|l| sprite.is_descendant(l.id, target))
            .count();
    let target_parent = rest[pos].parent;
    let (index, parent) = match drop {
        LayerDrop::Above(_) => (subtree_end, target_parent),
        LayerDrop::Below(_) => (pos, target_parent),
        LayerDrop::Into(_) => {
            if rest[pos].kind != LayerKind::Group {
                return None;
            }
            (subtree_end, Some(target))
        }
    };
    Some(Command::MoveLayer { id, index, parent })
}

/// Inserta una copia del frame `index` justo después. Con `linked`, los cels nuevos comparten
/// imagen con los originales; si no, cada uno recibe su propia imagen.
pub fn duplicate_frame(sprite: &Sprite, index: usize, linked: bool) -> Option<Command> {
    let frame = *sprite.frames.get(index)?;
    let mut cmds = vec![Command::AddFrame {
        index: index + 1,
        frame,
    }];
    let mut next_id = sprite.next_image_id().0;
    for (&(layer, f), cel) in &sprite.cels {
        if f != index {
            continue;
        }
        let cel = if linked {
            *cel
        } else {
            let id = ImageId(next_id);
            next_id += 1;
            cmds.push(Command::SetImage {
                id,
                image: sprite.image(cel.image).cloned(),
            });
            Cel { image: id, ..*cel }
        };
        cmds.push(Command::SetCel {
            layer,
            frame: index + 1,
            cel: Some(cel),
        });
    }
    Some(Command::Batch(cmds))
}

/// Máscara nueva del tamaño del lienzo: todo visible, todo oculto, o visible solo en `selection`.
pub fn new_mask(sprite: &Sprite, hide_all: bool, selection: Option<&Mask>) -> LayerMask {
    let (w, h) = (sprite.width, sprite.height);
    match selection {
        Some(sel) => {
            let mut mask = LayerMask::filled(w, h, 0);
            let data = Arc::make_mut(&mut mask.data);
            for (x, y) in sel.points() {
                data[(y as u32 * w + x as u32) as usize] = 255;
            }
            mask
        }
        None => LayerMask::filled(w, h, if hide_all { 0 } else { 255 }),
    }
}

/// Aplica la máscara de una capa de imagen a sus píxeles (todos los frames) y la quita.
/// Los cels vinculados siguen compartiendo imagen.
pub fn apply_mask(sprite: &Sprite, id: LayerId) -> Option<Command> {
    let layer = sprite.layer(id)?;
    let mask = layer.mask.as_ref()?;
    if layer.kind != LayerKind::Image {
        return None;
    }
    let mut cmds = Vec::new();
    let mut next = sprite.next_image_id().0;
    let mut done: HashMap<(ImageId, (i32, i32)), ImageId> = HashMap::new();
    for frame in 0..sprite.frames.len() {
        let Some(cel) = sprite.cel(id, frame).copied() else {
            continue;
        };
        let new_id = match done.get(&(cel.image, cel.position)) {
            Some(&new_id) => new_id,
            None => {
                let Some(image) = sprite.image(cel.image) else {
                    continue;
                };
                let mut out = cs_core::Image::clone(image);
                for iy in 0..image.height() {
                    for ix in 0..image.width() {
                        let (x, y) = (cel.position.0 + ix as i32, cel.position.1 + iy as i32);
                        let m = mask.get(x, y) as u32;
                        let value = match image.get(ix, iy).expect("in bounds") {
                            PixelValue::Rgba(c) => PixelValue::Rgba(cs_core::Rgba {
                                a: ((c.a as u32 * m + 127) / 255) as u8,
                                ..c
                            }),
                            PixelValue::Index(_) if m < 128 => sprite.color_mode.transparent(),
                            v => v,
                        };
                        out.set(ix, iy, value).expect("same mode");
                    }
                }
                let new_id = ImageId(next);
                next += 1;
                cmds.push(Command::SetImage {
                    id: new_id,
                    image: Some(Arc::new(out)),
                });
                done.insert((cel.image, cel.position), new_id);
                new_id
            }
        };
        cmds.push(Command::SetCel {
            layer: id,
            frame,
            cel: Some(Cel {
                image: new_id,
                ..cel
            }),
        });
    }
    cmds.push(Command::SetLayerMask { id, mask: None });
    Some(Command::Batch(cmds))
}

/// Selección con lo que deja ver la máscara (valor ≥ 128).
pub fn mask_to_selection(sprite: &Sprite, id: LayerId) -> Option<Mask> {
    let mask = sprite.layer(id)?.mask.as_ref()?;
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    let points = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| mask.get(x, y) >= 128);
    Some(Mask::from_points(sprite.width, sprite.height, points))
}

/// Vincula el cel `(layer, frame)` al de `(layer, source)`: pasan a compartir imagen.
pub fn link_cel(sprite: &Sprite, layer: LayerId, frame: usize, source: usize) -> Option<Command> {
    let cel = *sprite.cel(layer, source)?;
    Some(Command::SetCel {
        layer,
        frame,
        cel: Some(cel),
    })
}

/// Vincula todos los frames `from..=to` de una capa al primer cel del rango que exista.
pub fn link_range(sprite: &Sprite, layer: LayerId, from: usize, to: usize) -> Option<Command> {
    let (from, to) = (from.min(to), from.max(to));
    let source = (from..=to).find(|&f| sprite.cel(layer, f).is_some())?;
    let cel = *sprite.cel(layer, source)?;
    let cmds: Vec<Command> = (from..=to)
        .filter(|&f| f != source && sprite.cel(layer, f) != Some(&cel))
        .map(|frame| Command::SetCel {
            layer,
            frame,
            cel: Some(cel),
        })
        .collect();
    (!cmds.is_empty()).then_some(Command::Batch(cmds))
}

/// Desvincula todos los cels vinculados del rango (cada uno recibe su propia copia).
pub fn unlink_range(sprite: &Sprite, layer: LayerId, from: usize, to: usize) -> Option<Command> {
    let (from, to) = (from.min(to), from.max(to));
    let mut next_id = sprite.next_image_id().0;
    let mut cmds = Vec::new();
    // El primero de cada grupo vinculado conserva la imagen original.
    let mut kept = std::collections::HashSet::new();
    for frame in from..=to {
        let Some(cel) = sprite.cel(layer, frame).copied() else {
            continue;
        };
        if !is_linked(sprite, layer, frame) || kept.insert(cel.image) {
            continue;
        }
        let id = ImageId(next_id);
        next_id += 1;
        cmds.push(Command::SetImage {
            id,
            image: sprite.image(cel.image).cloned(),
        });
        cmds.push(Command::SetCel {
            layer,
            frame,
            cel: Some(Cel { image: id, ..cel }),
        });
    }
    (!cmds.is_empty()).then_some(Command::Batch(cmds))
}

/// Desvincula un cel que comparte imagen con otros: recibe una copia propia.
pub fn unlink_cel(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<Command> {
    let cel = *sprite.cel(layer, frame)?;
    let shared = sprite
        .cels
        .iter()
        .any(|(k, c)| *k != (layer, frame) && c.image == cel.image);
    if !shared {
        return None;
    }
    let id = sprite.next_image_id();
    Some(Command::Batch(vec![
        Command::SetImage {
            id,
            image: sprite.image(cel.image).cloned(),
        },
        Command::SetCel {
            layer,
            frame,
            cel: Some(Cel { image: id, ..cel }),
        },
    ]))
}

/// `true` si el cel comparte imagen con otro cel.
pub fn is_linked(sprite: &Sprite, layer: LayerId, frame: usize) -> bool {
    sprite.cel(layer, frame).is_some_and(|cel| {
        sprite
            .cels
            .iter()
            .any(|(k, c)| *k != (layer, frame) && c.image == cel.image)
    })
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use super::*;

    fn dot(s: &mut Sprite, layer: u32, frame: usize) {
        Command::DrawPixels {
            layer: LayerId(layer),
            frame,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))],
        }
        .apply(s)
        .unwrap();
    }

    /// Abajo → arriba: L1, G2 { L3 }, L4.
    fn tree() -> Sprite {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        new_layer(&s.clone(), Some(LayerId(1)), LayerKind::Group)
            .apply(&mut s)
            .unwrap();
        let mut child = Layer::new_image(LayerId(3), "c");
        child.parent = Some(LayerId(2));
        Command::AddLayer {
            index: 2,
            layer: child,
        }
        .apply(&mut s)
        .unwrap();
        new_layer(&s.clone(), None, LayerKind::Image)
            .apply(&mut s)
            .unwrap();
        s
    }

    fn order(s: &Sprite) -> Vec<(u32, Option<u32>)> {
        s.layers
            .iter()
            .map(|l| (l.id.0, l.parent.map(|p| p.0)))
            .collect()
    }

    #[test]
    fn instance_layer_keys_update_and_remove() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        for i in 1..4 {
            crate::Command::AddFrame {
                index: i,
                frame: Frame { duration_ms: 100 },
            }
            .apply(&mut s)
            .unwrap();
        }
        let cmd = new_instance_layer(&s, Some(LayerId(1)), 1, ArtboardId(2), "Hero", (3, 4));
        cmd.apply(&mut s).unwrap();
        let layer = s.layers.last().unwrap().clone();
        assert_eq!(
            (layer.kind, layer.name.as_str()),
            (LayerKind::Instance, "Hero")
        );
        let id = layer.id;
        let track = |s: &Sprite| s.layer(id).unwrap().instance.clone().unwrap();
        assert_eq!(
            track(&s).keys,
            vec![InstanceKey::new(1, ArtboardId(2), (3, 4))]
        );
        // Editar en un frame sin key crea una copia de la vigente ahí.
        update_instance_key(&s, id, 3, |k| k.flip_h = true)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        let t = track(&s);
        assert_eq!(t.keys.len(), 2);
        assert!(!t.key_at(2).unwrap().flip_h && t.key_at(3).unwrap().flip_h);
        assert_eq!(t.key_at(3).unwrap().position, (3, 4));
        // Sin cambios, no hay comando; antes de la primera key, tampoco.
        assert!(update_instance_key(&s, id, 3, |_| {}).is_none());
        assert!(update_instance_key(&s, id, 0, |k| k.opacity = 1).is_none());
        // Borrar una key; borrar la única quita la capa.
        remove_instance_key(&s, id, 3)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(track(&s).keys.len(), 1);
        assert!(remove_instance_key(&s, id, 2).is_none(), "no key there");
        remove_instance_key(&s, id, 1)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert!(s.layer(id).is_none());
    }

    #[test]
    fn new_layer_goes_above_target_in_same_group() {
        let mut s = tree();
        new_layer(&s.clone(), Some(LayerId(3)), LayerKind::Image)
            .apply(&mut s)
            .unwrap();
        assert_eq!(
            order(&s),
            vec![(1, None), (2, None), (3, Some(2)), (5, Some(2)), (4, None)]
        );
        assert!(s.is_valid_tree());
    }

    #[test]
    fn moving_group_moves_children_and_undo_restores() {
        let mut s = tree();
        let before = s.clone();
        let inv = move_layer(&s, LayerId(2), LayerDrop::Above(LayerId(4)))
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(
            order(&s),
            vec![(1, None), (4, None), (2, None), (3, Some(2))]
        );
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn move_into_and_out_of_group() {
        let mut s = tree();
        move_layer(&s, LayerId(4), LayerDrop::Into(LayerId(2)))
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(
            order(&s),
            vec![(1, None), (2, None), (3, Some(2)), (4, Some(2))]
        );
        move_layer(&s, LayerId(3), LayerDrop::Below(LayerId(1)))
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(
            order(&s),
            vec![(3, None), (1, None), (2, None), (4, Some(2))]
        );
        assert!(s.is_valid_tree());
    }

    #[test]
    fn invalid_moves_are_rejected() {
        let s = tree();
        // Un grupo no puede entrar en sí mismo ni en sus hijos; una capa normal no es destino "Into".
        assert_eq!(
            move_layer(&s, LayerId(2), LayerDrop::Into(LayerId(2))),
            None
        );
        assert_eq!(
            move_layer(&s, LayerId(2), LayerDrop::Above(LayerId(3))),
            None
        );
        assert_eq!(
            move_layer(&s, LayerId(4), LayerDrop::Into(LayerId(1))),
            None
        );
        let mut s2 = s.clone();
        assert_eq!(
            Command::MoveLayer {
                id: LayerId(4),
                index: 2,
                parent: None
            }
            .apply(&mut s2),
            Err(crate::CmdError::InvalidLayerTree)
        );
        assert_eq!(s2, s);
    }

    #[test]
    fn remove_group_removes_children() {
        let mut s = tree();
        dot(&mut s, 3, 0);
        let before = s.clone();
        let inv = remove_layer_tree(&s, LayerId(2))
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(order(&s), vec![(1, None), (4, None)]);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn duplicate_frame_linked_and_unlinked() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        dot(&mut s, 1, 0);
        let mut linked = s.clone();
        duplicate_frame(&linked.clone(), 0, true)
            .unwrap()
            .apply(&mut linked)
            .unwrap();
        assert_eq!(linked.frames.len(), 2);
        assert!(is_linked(&linked, LayerId(1), 1));
        let before = s.clone();
        let inv = duplicate_frame(&s.clone(), 0, false)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert!(!is_linked(&s, LayerId(1), 1));
        assert_eq!(
            s.pixel(LayerId(1), 1, 0, 0),
            PixelValue::Rgba(Rgba::rgb(1, 2, 3))
        );
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn link_and_unlink() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        dot(&mut s, 1, 0);
        link_cel(&s, LayerId(1), 1, 0)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert!(is_linked(&s, LayerId(1), 0) && is_linked(&s, LayerId(1), 1));
        // Pintar en uno se ve en el otro.
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 1,
            pixels: vec![(2, 2, PixelValue::Rgba(Rgba::rgb(9, 9, 9)))],
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(
            s.pixel(LayerId(1), 0, 2, 2),
            PixelValue::Rgba(Rgba::rgb(9, 9, 9))
        );
        unlink_cel(&s, LayerId(1), 1)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert!(!is_linked(&s, LayerId(1), 0));
        assert_eq!(unlink_cel(&s, LayerId(1), 1), None);
    }

    #[test]
    fn new_apply_and_select_masks() {
        let mut s = Sprite::new(4, 1, ColorMode::Rgba);
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: (0..4).map(|x| (x, 0, red)).collect(),
        }
        .apply(&mut s)
        .unwrap();
        duplicate_frame(&s, 0, true).unwrap().apply(&mut s).unwrap();
        let sel = Mask::from_rect(4, 1, (1, 0), (2, 0));
        let mask = new_mask(&s, false, Some(&sel));
        assert_eq!(
            (mask.get(0, 0), mask.get(1, 0), mask.get(9, 9)),
            (0, 255, 0)
        );
        assert_eq!(new_mask(&s, true, None).get(0, 0), 0);
        Command::SetLayerMask {
            id: LayerId(1),
            mask: Some(mask),
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(mask_to_selection(&s, LayerId(1)).unwrap(), sel);
        let before = s.clone();
        let inv = apply_mask(&s, LayerId(1)).unwrap().apply(&mut s).unwrap();
        assert!(s.layers[0].mask.is_none());
        assert_eq!(
            s.pixel(LayerId(1), 1, 0, 0),
            PixelValue::Rgba(Rgba::new(255, 0, 0, 0))
        );
        assert_eq!(s.pixel(LayerId(1), 0, 1, 0), red);
        assert!(is_linked(&s, LayerId(1), 1), "links survive");
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        assert!(apply_mask(&s, LayerId(9)).is_none());
    }

    #[test]
    fn link_and_unlink_ranges() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        for i in 1..4 {
            Command::AddFrame {
                index: i,
                frame: Frame { duration_ms: 100 },
            }
            .apply(&mut s)
            .unwrap();
        }
        dot(&mut s, 1, 1); // el primer cel del rango 0..=3 está en el frame 1
        let before = s.clone();
        let inv = link_range(&s, LayerId(1), 3, 0)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        for f in 0..4 {
            assert!(is_linked(&s, LayerId(1), f), "frame {f}");
        }
        assert_eq!(link_range(&s, LayerId(1), 0, 3), None, "already linked");
        let linked = s.clone();
        let inv2 = unlink_range(&s, LayerId(1), 0, 3)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        for f in 0..4 {
            assert!(!is_linked(&s, LayerId(1), f), "frame {f}");
        }
        inv2.apply(&mut s).unwrap();
        assert_eq!(s, linked);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        let empty = Sprite::new(2, 2, ColorMode::Rgba);
        assert_eq!(link_range(&empty, LayerId(1), 0, 0), None);
    }

    #[test]
    fn set_palette_roundtrip() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        let before = s.clone();
        let mut colors = s.palette.colors().to_vec();
        colors.push(Rgba::rgb(1, 2, 3));
        let inv = Command::SetPalette(Palette::new(colors))
            .apply(&mut s)
            .unwrap();
        assert_eq!(s.palette.len(), 33);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn frames_adjust_tags_and_undo_restores_them() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        for i in 1..4 {
            Command::AddFrame {
                index: i,
                frame: Frame { duration_ms: 100 },
            }
            .apply(&mut s)
            .unwrap();
        }
        let tag = |name: &str, from, to| Tag {
            name: name.into(),
            from,
            to,
            direction: AnimDirection::Forward,
            color: Rgba::rgb(1, 1, 1),
            repeat: 0,
        };
        Command::SetTags(vec![tag("walk", 1, 2), tag("solo", 3, 3)])
            .apply(&mut s)
            .unwrap();
        let before = s.clone();
        let inv = Command::AddFrame {
            index: 2,
            frame: Frame { duration_ms: 50 },
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!((s.tags[0].from, s.tags[0].to), (1, 3));
        assert_eq!((s.tags[1].from, s.tags[1].to), (4, 4));
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        let inv = Command::RemoveFrame { index: 3 }.apply(&mut s).unwrap();
        assert_eq!(s.tags.len(), 1, "el tag de un solo frame desaparece");
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn move_frame_and_duration_roundtrip() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        for i in 1..3 {
            Command::AddFrame {
                index: i,
                frame: Frame { duration_ms: 100 },
            }
            .apply(&mut s)
            .unwrap();
        }
        dot(&mut s, 1, 0);
        let before = s.clone();
        let inv = Command::MoveFrame { from: 0, to: 2 }.apply(&mut s).unwrap();
        assert!(s.cel(LayerId(1), 2).is_some() && s.cel(LayerId(1), 0).is_none());
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        let inv = Command::SetFrameDuration {
            index: 1,
            duration_ms: 33,
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.frames[1].duration_ms, 33);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }
}
