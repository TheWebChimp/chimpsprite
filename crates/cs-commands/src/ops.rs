//! Operaciones compuestas: arman `Command`s a partir del estado actual del sprite.

use cs_core::{Cel, ImageId, Layer, LayerId, LayerKind, Sprite};

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
    let prefix = if kind == LayerKind::Group {
        "Group"
    } else {
        "Layer"
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

/// Vincula el cel `(layer, frame)` al de `(layer, source)`: pasan a compartir imagen.
pub fn link_cel(sprite: &Sprite, layer: LayerId, frame: usize, source: usize) -> Option<Command> {
    let cel = *sprite.cel(layer, source)?;
    Some(Command::SetCel {
        layer,
        frame,
        cel: Some(cel),
    })
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
