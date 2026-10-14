use std::sync::Arc;

use cs_core::{ArtboardId, Cel, Frame, Image, ImageId, Layer, LayerId, PixelValue, Sprite};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CmdError {
    #[error("layer {0:?} not found")]
    LayerNotFound(LayerId),
    #[error("layer {0:?} is locked")]
    LayerLocked(LayerId),
    #[error("layer {0:?} is not an image layer")]
    NotImageLayer(LayerId),
    #[error("layer {0:?} has child layers")]
    LayerHasChildren(LayerId),
    #[error("layer id {0:?} already exists")]
    DuplicateLayerId(LayerId),
    #[error("frame {0} out of range")]
    FrameOutOfRange(usize),
    #[error("index {0} out of range")]
    IndexOutOfRange(usize),
    #[error("cannot remove the last frame")]
    LastFrame,
    #[error("pixel value does not match the sprite color mode")]
    ColorModeMismatch,
    #[error("invalid size {0}×{1}")]
    InvalidSize(u32, u32),
    #[error("artboard {0:?} not found")]
    ArtboardNotFound(ArtboardId),
    #[error("artboard id {0:?} already exists")]
    DuplicateArtboardId(ArtboardId),
    #[error("cannot remove the last artboard")]
    LastArtboard,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerProps {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub opacity: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Pinta píxeles en coordenadas del lienzo; crea o expande el cel si hace falta.
    DrawPixels {
        layer: LayerId,
        frame: usize,
        pixels: Vec<(i32, i32, PixelValue)>,
    },
    /// Primitiva: pone (o quita, con `None`) el cel de una capa en un frame.
    SetCel {
        layer: LayerId,
        frame: usize,
        cel: Option<Cel>,
    },
    /// Primitiva: pone (o quita, con `None`) una imagen del pool.
    SetImage {
        id: ImageId,
        image: Option<Arc<Image>>,
    },
    AddLayer {
        index: usize,
        layer: Layer,
    },
    RemoveLayer {
        id: LayerId,
    },
    SetLayerProps {
        id: LayerId,
        props: LayerProps,
    },
    AddFrame {
        index: usize,
        frame: Frame,
    },
    RemoveFrame {
        index: usize,
    },
    /// Cambia el tamaño del lienzo y desplaza todos los cels por `offset`.
    ResizeCanvas {
        width: u32,
        height: u32,
        offset: (i32, i32),
    },
    /// Transacción: se aplica completa o nada; cuenta como un paso de undo.
    Batch(Vec<Command>),
}

impl Command {
    /// Aplica el comando y devuelve su inverso. Si falla, el documento no cambia.
    pub fn apply(&self, doc: &mut Sprite) -> Result<Command, CmdError> {
        match self {
            Command::DrawPixels {
                layer,
                frame,
                pixels,
            } => crate::draw::draw_pixels(doc, *layer, *frame, pixels),
            Command::SetCel { layer, frame, cel } => set_cel(doc, *layer, *frame, *cel),
            Command::SetImage { id, image } => {
                let old = match image {
                    Some(img) => doc.images.insert(*id, img.clone()),
                    None => doc.images.remove(id),
                };
                Ok(Command::SetImage {
                    id: *id,
                    image: old,
                })
            }
            Command::AddLayer { index, layer } => add_layer(doc, *index, layer),
            Command::RemoveLayer { id } => remove_layer(doc, *id),
            Command::SetLayerProps { id, props } => set_layer_props(doc, *id, props),
            Command::AddFrame { index, frame } => add_frame(doc, *index, *frame),
            Command::RemoveFrame { index } => remove_frame(doc, *index),
            Command::ResizeCanvas {
                width,
                height,
                offset,
            } => resize_canvas(doc, *width, *height, *offset),
            Command::Batch(cmds) => apply_batch(doc, cmds),
        }
    }
}

impl crate::Undoable for Command {
    type Target = Sprite;

    fn apply(&self, target: &mut Sprite) -> Result<Self, CmdError> {
        Command::apply(self, target)
    }
}

fn resize_canvas(
    doc: &mut Sprite,
    width: u32,
    height: u32,
    offset: (i32, i32),
) -> Result<Command, CmdError> {
    if width == 0 || height == 0 {
        return Err(CmdError::InvalidSize(width, height));
    }
    let inverse = Command::ResizeCanvas {
        width: doc.width,
        height: doc.height,
        offset: (-offset.0, -offset.1),
    };
    doc.width = width;
    doc.height = height;
    for cel in doc.cels.values_mut() {
        cel.position.0 += offset.0;
        cel.position.1 += offset.1;
    }
    Ok(inverse)
}

fn set_cel(
    doc: &mut Sprite,
    layer: LayerId,
    frame: usize,
    cel: Option<Cel>,
) -> Result<Command, CmdError> {
    let old = match cel {
        Some(c) => {
            doc.layer(layer).ok_or(CmdError::LayerNotFound(layer))?;
            if frame >= doc.frames.len() {
                return Err(CmdError::FrameOutOfRange(frame));
            }
            doc.cels.insert((layer, frame), c)
        }
        None => doc.cels.remove(&(layer, frame)),
    };
    Ok(Command::SetCel {
        layer,
        frame,
        cel: old,
    })
}

fn add_layer(doc: &mut Sprite, index: usize, layer: &Layer) -> Result<Command, CmdError> {
    if index > doc.layers.len() {
        return Err(CmdError::IndexOutOfRange(index));
    }
    if doc.layer(layer.id).is_some() {
        return Err(CmdError::DuplicateLayerId(layer.id));
    }
    doc.layers.insert(index, layer.clone());
    Ok(Command::RemoveLayer { id: layer.id })
}

fn remove_layer(doc: &mut Sprite, id: LayerId) -> Result<Command, CmdError> {
    let index = doc.layer_index(id).ok_or(CmdError::LayerNotFound(id))?;
    if doc.layers.iter().any(|l| l.parent == Some(id)) {
        return Err(CmdError::LayerHasChildren(id));
    }
    let layer = doc.layers.remove(index);
    let keys: Vec<_> = doc.cels.keys().filter(|(l, _)| *l == id).copied().collect();
    let mut inverse = vec![Command::AddLayer { index, layer }];
    for key in keys {
        let cel = doc.cels.remove(&key);
        inverse.push(Command::SetCel {
            layer: key.0,
            frame: key.1,
            cel,
        });
    }
    Ok(Command::Batch(inverse))
}

fn set_layer_props(doc: &mut Sprite, id: LayerId, props: &LayerProps) -> Result<Command, CmdError> {
    let layer = doc.layer_mut(id).ok_or(CmdError::LayerNotFound(id))?;
    let mut old = LayerProps::default();
    if let Some(v) = &props.name {
        old.name = Some(std::mem::replace(&mut layer.name, v.clone()));
    }
    if let Some(v) = props.visible {
        old.visible = Some(std::mem::replace(&mut layer.visible, v));
    }
    if let Some(v) = props.locked {
        old.locked = Some(std::mem::replace(&mut layer.locked, v));
    }
    if let Some(v) = props.opacity {
        old.opacity = Some(std::mem::replace(&mut layer.opacity, v));
    }
    Ok(Command::SetLayerProps { id, props: old })
}

/// Reindexa los cels: `f(frame)` devuelve el nuevo índice.
fn remap_frames(doc: &mut Sprite, f: impl Fn(usize) -> usize) {
    let cels = std::mem::take(&mut doc.cels);
    doc.cels = cels
        .into_iter()
        .map(|((l, fr), c)| ((l, f(fr)), c))
        .collect();
}

fn add_frame(doc: &mut Sprite, index: usize, frame: Frame) -> Result<Command, CmdError> {
    if index > doc.frames.len() {
        return Err(CmdError::IndexOutOfRange(index));
    }
    remap_frames(doc, |f| if f >= index { f + 1 } else { f });
    doc.frames.insert(index, frame);
    Ok(Command::RemoveFrame { index })
}

fn remove_frame(doc: &mut Sprite, index: usize) -> Result<Command, CmdError> {
    if index >= doc.frames.len() {
        return Err(CmdError::FrameOutOfRange(index));
    }
    if doc.frames.len() == 1 {
        return Err(CmdError::LastFrame);
    }
    let keys: Vec<_> = doc
        .cels
        .keys()
        .filter(|(_, f)| *f == index)
        .copied()
        .collect();
    let mut inverse = vec![Command::AddFrame {
        index,
        frame: doc.frames.remove(index),
    }];
    for key in keys {
        let cel = doc.cels.remove(&key);
        inverse.push(Command::SetCel {
            layer: key.0,
            frame: key.1,
            cel,
        });
    }
    remap_frames(doc, |f| if f > index { f - 1 } else { f });
    Ok(Command::Batch(inverse))
}

fn apply_batch(doc: &mut Sprite, cmds: &[Command]) -> Result<Command, CmdError> {
    let mut inverses = Vec::with_capacity(cmds.len());
    for cmd in cmds {
        match cmd.apply(doc) {
            Ok(inv) => inverses.push(inv),
            Err(e) => {
                for inv in inverses.iter().rev() {
                    if let Err(rollback) = inv.apply(doc) {
                        log::error!("batch rollback failed: {rollback}");
                    }
                }
                return Err(e);
            }
        }
    }
    inverses.reverse();
    Ok(Command::Batch(inverses))
}

#[cfg(test)]
mod tests {
    use cs_core::*;

    use crate::{CmdError, Command, LayerProps};

    fn dot(layer: u32) -> Command {
        Command::DrawPixels {
            layer: LayerId(layer),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))],
        }
    }

    #[test]
    fn add_and_remove_layer_roundtrip() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let before = s.clone();
        let id = s.next_layer_id();
        let inv = Command::AddLayer {
            index: 1,
            layer: Layer::new_image(id, "Layer 2"),
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.layers[1].id, id);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn remove_layer_restores_cels_on_undo() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        dot(1).apply(&mut s).unwrap();
        let before = s.clone();
        let inv = Command::RemoveLayer { id: LayerId(1) }
            .apply(&mut s)
            .unwrap();
        assert!(s.layers.is_empty() && s.cels.is_empty());
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn frames_shift_cels() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        dot(1).apply(&mut s).unwrap();
        let before = s.clone();
        let inv = Command::AddFrame {
            index: 0,
            frame: Frame { duration_ms: 50 },
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.frames.len(), 2);
        assert!(s.cel(LayerId(1), 0).is_none() && s.cel(LayerId(1), 1).is_some());
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        assert_eq!(
            Command::RemoveFrame { index: 0 }.apply(&mut s),
            Err(CmdError::LastFrame)
        );
    }

    #[test]
    fn set_layer_props_roundtrip() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let before = s.clone();
        let inv = Command::SetLayerProps {
            id: LayerId(1),
            props: LayerProps {
                visible: Some(false),
                opacity: Some(10),
                ..Default::default()
            },
        }
        .apply(&mut s)
        .unwrap();
        assert!(!s.layers[0].visible);
        assert_eq!(s.layers[0].opacity, 10);
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn failing_batch_rolls_back() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let before = s.clone();
        let r = Command::Batch(vec![
            Command::AddFrame {
                index: 1,
                frame: Frame { duration_ms: 100 },
            },
            Command::RemoveLayer { id: LayerId(99) },
        ])
        .apply(&mut s);
        assert_eq!(r, Err(CmdError::LayerNotFound(LayerId(99))));
        assert_eq!(s, before);
    }

    #[test]
    fn resize_canvas_shifts_cels_and_undoes() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        dot(1).apply(&mut s).unwrap();
        let before = s.clone();
        let inv = Command::ResizeCanvas {
            width: 6,
            height: 5,
            offset: (2, 1),
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!((s.width, s.height), (6, 5));
        assert_eq!(s.cel(LayerId(1), 0).unwrap().position, (2, 1));
        assert_eq!(
            s.pixel(LayerId(1), 0, 2, 1),
            PixelValue::Rgba(Rgba::rgb(1, 2, 3))
        );
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
        assert_eq!(
            Command::ResizeCanvas {
                width: 0,
                height: 3,
                offset: (0, 0)
            }
            .apply(&mut s),
            Err(CmdError::InvalidSize(0, 3))
        );
    }

    #[test]
    fn commands_serialize() {
        let c = Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Index(2))],
        };
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), c);
    }
}
