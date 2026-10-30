use std::collections::BTreeMap;
use std::sync::Arc;

use cs_core::{
    ArtboardId, BlendMode, Cel, Frame, GridSettings, Image, ImageId, InstanceKey, Layer, LayerId,
    LayerKind, LayerMask, Palette, PixelValue, SceneObject, Slice, Sprite, Tag, TileGrid, Tileset,
};
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
    #[error("the move would break the layer tree")]
    InvalidLayerTree,
    #[error("layer {0:?} has no mask")]
    NoMask(LayerId),
    #[error("layer {0:?} is not an object layer")]
    NotObjectLayer(LayerId),
    #[error("object id {0:?} is repeated in the layer")]
    DuplicateObject(cs_core::ObjectId),
    #[error("an artboard cannot contain itself")]
    InstanceCycle,
    #[error("tileset {0} is used by a layer")]
    TilesetInUse(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerProps {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub opacity: Option<u8>,
    pub blend: Option<BlendMode>,
    #[serde(default)]
    pub clip: Option<bool>,
    #[serde(default)]
    pub reference: Option<bool>,
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
    /// Pone, cambia o quita la máscara de una capa.
    SetLayerMask {
        id: LayerId,
        mask: Option<LayerMask>,
    },
    /// Pinta valores (0 oculta, 255 muestra) en la máscara de una capa; la agranda si hace falta.
    DrawMask {
        layer: LayerId,
        pixels: Vec<(i32, i32, u8)>,
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
    /// Recorta las imágenes de los cels a los límites del lienzo; quita los cels que quedan fuera.
    TrimToCanvas,
    /// Reemplaza la lista completa de tags.
    SetTags(Vec<Tag>),
    /// Reemplaza todos los slices (hitboxes, pivotes).
    SetSlices(Vec<Slice>),
    /// Reemplaza los objetos (y sus keys) de una capa de objetos.
    SetObjects {
        layer: LayerId,
        objects: Vec<SceneObject>,
    },
    /// Primitiva: agrega (`index == len`), reemplaza (`index < len`) o quita (`None`) un tileset.
    /// Solo se quita el último (quitar otro correría los índices de las capas) y solo si ninguna
    /// capa lo usa.
    SetTileset {
        index: usize,
        tileset: Option<Tileset>,
    },
    /// Primitiva: pone (o quita, con `None`) una grilla de tiles del pool.
    SetTilemap {
        id: ImageId,
        grid: Option<Arc<TileGrid>>,
    },
    SetGrid(GridSettings),
    /// Reemplaza la paleta completa.
    SetPalette(Palette),
    /// Mueve la capa y sus descendientes; `index` es la posición en la lista sin ese bloque.
    MoveLayer {
        id: LayerId,
        index: usize,
        parent: Option<LayerId>,
    },
    MoveFrame {
        from: usize,
        to: usize,
    },
    SetFrameDuration {
        index: usize,
        duration_ms: u16,
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
            Command::SetLayerMask { id, mask } => {
                let layer = doc.layer_mut(*id).ok_or(CmdError::LayerNotFound(*id))?;
                let old = std::mem::replace(&mut layer.mask, mask.clone());
                Ok(Command::SetLayerMask { id: *id, mask: old })
            }
            Command::DrawMask { layer, pixels } => draw_mask(doc, *layer, pixels),
            Command::AddFrame { index, frame } => add_frame(doc, *index, *frame),
            Command::RemoveFrame { index } => remove_frame(doc, *index),
            Command::ResizeCanvas {
                width,
                height,
                offset,
            } => resize_canvas(doc, *width, *height, *offset),
            Command::TrimToCanvas => Ok(trim_to_canvas(doc)),
            Command::SetPalette(palette) => Ok(Command::SetPalette(std::mem::replace(
                &mut doc.palette,
                palette.clone(),
            ))),
            Command::SetGrid(grid) => {
                if grid.size.0 == 0 || grid.size.1 == 0 {
                    return Err(CmdError::InvalidSize(grid.size.0, grid.size.1));
                }
                Ok(Command::SetGrid(std::mem::replace(&mut doc.grid, *grid)))
            }
            Command::SetTags(tags) => Ok(Command::SetTags(std::mem::replace(
                &mut doc.tags,
                tags.clone(),
            ))),
            Command::SetSlices(slices) => Ok(Command::SetSlices(std::mem::replace(
                &mut doc.slices,
                slices.clone(),
            ))),
            Command::SetObjects { layer, objects } => set_objects(doc, *layer, objects),
            Command::SetTileset { index, tileset } => set_tileset(doc, *index, tileset.as_ref()),
            Command::SetTilemap { id, grid } => {
                let old = match grid {
                    Some(g) => doc.tilemaps.insert(*id, g.clone()),
                    None => doc.tilemaps.remove(id),
                };
                Ok(Command::SetTilemap { id: *id, grid: old })
            }
            Command::MoveLayer { id, index, parent } => move_layer(doc, *id, *index, *parent),
            Command::MoveFrame { from, to } => move_frame(doc, *from, *to),
            Command::SetFrameDuration { index, duration_ms } => {
                let frame = doc
                    .frames
                    .get_mut(*index)
                    .ok_or(CmdError::FrameOutOfRange(*index))?;
                let old = std::mem::replace(&mut frame.duration_ms, *duration_ms);
                Ok(Command::SetFrameDuration {
                    index: *index,
                    duration_ms: old,
                })
            }
            Command::Batch(cmds) => apply_batch(doc, cmds),
        }
    }
}

impl crate::Undoable for Command {
    type Target = Sprite;

    fn apply(&self, target: &mut Sprite) -> Result<Self, CmdError> {
        Command::apply(self, target)
    }

    fn label(&self) -> &'static str {
        Command::label(self)
    }
}

impl Command {
    /// El nombre del paso, para el historial.
    pub fn label(&self) -> &'static str {
        match self {
            Command::DrawPixels { .. } => "Draw",
            Command::SetCel { cel: None, .. } => "Clear Cel",
            Command::SetCel { .. } => "Cel",
            Command::SetImage { .. } => "Image",
            Command::AddLayer { .. } => "New Layer",
            Command::RemoveLayer { .. } => "Delete Layer",
            Command::SetLayerProps { .. } => "Layer Properties",
            Command::SetLayerMask { .. } => "Layer Mask",
            Command::DrawMask { .. } => "Draw Mask",
            Command::AddFrame { .. } => "New Frame",
            Command::RemoveFrame { .. } => "Delete Frame",
            Command::ResizeCanvas { .. } => "Canvas Size",
            Command::TrimToCanvas => "Trim",
            Command::SetTags(_) => "Tags",
            Command::SetSlices(_) => "Slices",
            Command::SetObjects { .. } => "Objects",
            Command::SetTileset { .. } => "Tileset",
            Command::SetTilemap { .. } => "Tilemap",
            Command::SetGrid(_) => "Grid",
            Command::SetPalette(_) => "Palette",
            Command::MoveLayer { .. } => "Move Layer",
            Command::MoveFrame { .. } => "Move Frame",
            Command::SetFrameDuration { .. } => "Frame Duration",
            // Un lote: el de su primer paso (las imágenes y tilemaps nuevos acompañan a otro).
            Command::Batch(cmds) => cmds
                .iter()
                .find(|c| !matches!(c, Command::SetImage { .. } | Command::SetTilemap { .. }))
                .or(cmds.first())
                .map_or("Change", Command::label),
        }
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
    for mask in doc.layers.iter_mut().filter_map(|l| l.mask.as_mut()) {
        mask.position.0 += offset.0;
        mask.position.1 += offset.1;
    }
    // Los objetos se mueven con el contenido.
    for object in doc.layers.iter_mut().flat_map(|l| l.objects.iter_mut()) {
        for key in &mut object.track.keys {
            key.position.0 += offset.0;
            key.position.1 += offset.1;
        }
    }
    Ok(inverse)
}

fn trim_to_canvas(doc: &mut Sprite) -> Command {
    // Agrupa los cels por imagen: los vinculados se recortan juntos.
    let mut by_image: BTreeMap<ImageId, Vec<(LayerId, usize)>> = BTreeMap::new();
    for (key, cel) in &doc.cels {
        by_image.entry(cel.image).or_default().push(*key);
    }
    let (w, h) = (doc.width as i32, doc.height as i32);
    let mut inverse = Vec::new();
    for (image_id, keys) in by_image {
        let positions: Vec<_> = keys.iter().map(|k| doc.cels[k].position).collect();
        if positions.iter().any(|p| *p != positions[0]) {
            log::warn!("linked cels of {image_id:?} have different positions; not trimmed");
            continue;
        }
        let Some(image) = doc.images.get(&image_id).cloned() else {
            continue;
        };
        let (px, py) = positions[0];
        let (x0, y0) = (px.max(0), py.max(0));
        let (x1, y1) = (
            (px + image.width() as i32).min(w),
            (py + image.height() as i32).min(h),
        );
        let cropped = if x0 >= x1 || y0 >= y1 {
            None
        } else if (x1 - x0, y1 - y0) == (image.width() as i32, image.height() as i32) {
            continue;
        } else {
            Some(image.crop(
                (x0 - px) as u32,
                (y0 - py) as u32,
                (x1 - x0) as u32,
                (y1 - y0) as u32,
            ))
        };
        for key in keys {
            let old = doc.cels[&key];
            inverse.push(Command::SetCel {
                layer: key.0,
                frame: key.1,
                cel: Some(old),
            });
            match &cropped {
                Some(_) => {
                    doc.cels.get_mut(&key).expect("key exists").position = (x0, y0);
                }
                None => {
                    doc.cels.remove(&key);
                }
            }
        }
        inverse.push(Command::SetImage {
            id: image_id,
            image: Some(image),
        });
        match cropped {
            Some(img) => doc.images.insert(image_id, Arc::new(img)),
            None => doc.images.remove(&image_id),
        };
    }
    // Primero restaurar imágenes y luego cels.
    inverse.reverse();
    Command::Batch(inverse)
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

fn set_tileset(
    doc: &mut Sprite,
    index: usize,
    tileset: Option<&Tileset>,
) -> Result<Command, CmdError> {
    let len = doc.tilesets.len();
    match tileset {
        Some(t) if index < len => {
            let old = std::mem::replace(&mut doc.tilesets[index], t.clone());
            Ok(Command::SetTileset {
                index,
                tileset: Some(old),
            })
        }
        Some(t) if index == len => {
            doc.tilesets.push(t.clone());
            Ok(Command::SetTileset {
                index,
                tileset: None,
            })
        }
        Some(_) => Err(CmdError::IndexOutOfRange(index)),
        None => {
            if index + 1 != len {
                return Err(CmdError::IndexOutOfRange(index));
            }
            if doc.layers.iter().any(|l| l.tileset == Some(index as u32)) {
                return Err(CmdError::TilesetInUse(index));
            }
            Ok(Command::SetTileset {
                index,
                tileset: doc.tilesets.pop(),
            })
        }
    }
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
    let mut freed = Vec::new();
    for key in keys {
        let cel = doc.cels.remove(&key);
        freed.extend(cel.map(|c| c.image));
        inverse.push(Command::SetCel {
            layer: key.0,
            frame: key.1,
            cel,
        });
    }
    // Las imágenes y grillas que ya no usa ningún cel salen del pool (el inverso las restaura).
    freed.sort();
    freed.dedup();
    for image in freed {
        if doc.cels.values().any(|c| c.image == image) {
            continue;
        }
        if let Some(old) = doc.images.remove(&image) {
            inverse.push(Command::SetImage {
                id: image,
                image: Some(old),
            });
        }
        if let Some(old) = doc.tilemaps.remove(&image) {
            inverse.push(Command::SetTilemap {
                id: image,
                grid: Some(old),
            });
        }
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
    if let Some(v) = props.blend {
        old.blend = Some(std::mem::replace(&mut layer.blend, v));
    }
    if let Some(v) = props.reference {
        old.reference = Some(std::mem::replace(&mut layer.reference, v));
    }
    if let Some(v) = props.clip {
        old.clip = Some(std::mem::replace(&mut layer.clip, v));
    }
    Ok(Command::SetLayerProps { id, props: old })
}

fn draw_mask(
    doc: &mut Sprite,
    id: LayerId,
    pixels: &[(i32, i32, u8)],
) -> Result<Command, CmdError> {
    let layer = doc.layer_mut(id).ok_or(CmdError::LayerNotFound(id))?;
    if layer.locked {
        return Err(CmdError::LayerLocked(id));
    }
    let old = layer.mask.clone().ok_or(CmdError::NoMask(id))?;
    if pixels.is_empty() {
        return Ok(Command::Batch(Vec::new()));
    }
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for &(x, y, _) in pixels {
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
    }
    let mut mask = old.grown_to(x0, y0, x1, y1);
    let (px, py, w) = (mask.position.0, mask.position.1, mask.width);
    let data = std::sync::Arc::make_mut(&mut mask.data);
    for &(x, y, v) in pixels {
        data[((y - py) as u32 * w + (x - px) as u32) as usize] = v;
    }
    layer.mask = Some(mask);
    Ok(Command::SetLayerMask {
        id,
        mask: Some(old),
    })
}

fn move_layer(
    doc: &mut Sprite,
    id: LayerId,
    index: usize,
    parent: Option<LayerId>,
) -> Result<Command, CmdError> {
    let range = doc.subtree_range(id).ok_or(CmdError::LayerNotFound(id))?;
    let old_index = range.start;
    let old_parent = doc.layers[old_index].parent;
    let backup = doc.layers.clone();
    let block: Vec<Layer> = doc.layers.drain(range).collect();
    let parent_ok = parent.is_none_or(|p| doc.layer(p).is_some_and(|l| l.kind == LayerKind::Group));
    if index > doc.layers.len() || !parent_ok {
        doc.layers = backup;
        return Err(if parent_ok {
            CmdError::IndexOutOfRange(index)
        } else {
            CmdError::InvalidLayerTree
        });
    }
    doc.layers.splice(index..index, block);
    doc.layers[index].parent = parent;
    if !doc.is_valid_tree() {
        doc.layers = backup;
        return Err(CmdError::InvalidLayerTree);
    }
    Ok(Command::MoveLayer {
        id,
        index: old_index,
        parent: old_parent,
    })
}

fn move_frame(doc: &mut Sprite, from: usize, to: usize) -> Result<Command, CmdError> {
    let len = doc.frames.len();
    for i in [from, to] {
        if i >= len {
            return Err(CmdError::FrameOutOfRange(i));
        }
    }
    let frame = doc.frames.remove(from);
    doc.frames.insert(to, frame);
    remap_frames(doc, |f| {
        if f == from {
            to
        } else if from < to && f > from && f <= to {
            f - 1
        } else if to < from && f >= to && f < from {
            f + 1
        } else {
            f
        }
    });
    Ok(Command::MoveFrame { from: to, to: from })
}

/// Tags después de insertar un frame en `index`.
fn tags_after_insert(tags: &[Tag], index: usize) -> Vec<Tag> {
    tags.iter()
        .map(|t| Tag {
            from: if t.from >= index { t.from + 1 } else { t.from },
            to: if t.to >= index { t.to + 1 } else { t.to },
            ..t.clone()
        })
        .collect()
}

/// Tags después de quitar el frame `index`; desaparecen los que solo lo contenían a él.
fn tags_after_remove(tags: &[Tag], index: usize) -> Vec<Tag> {
    tags.iter()
        .filter(|t| !(t.from == index && t.to == index))
        .map(|t| Tag {
            from: if t.from > index { t.from - 1 } else { t.from },
            to: if t.to >= index { t.to - 1 } else { t.to },
            ..t.clone()
        })
        .collect()
}

/// Slices con las claves corridas por `f` (si dos caen en el mismo frame gana la última).
fn remap_slices(slices: &[Slice], f: impl Fn(usize) -> usize) -> Vec<Slice> {
    slices
        .iter()
        .map(|s| {
            let mut out = Slice {
                keys: Vec::new(),
                ..s.clone()
            };
            for key in &s.keys {
                out.set_key(cs_core::SliceKey {
                    frame: f(key.frame),
                    ..*key
                });
            }
            out
        })
        .collect()
}

/// Reindexa los cels: `f(frame)` devuelve el nuevo índice.
fn remap_frames(doc: &mut Sprite, f: impl Fn(usize) -> usize) {
    let cels = std::mem::take(&mut doc.cels);
    doc.cels = cels
        .into_iter()
        .map(|((l, fr), c)| ((l, f(fr)), c))
        .collect();
}

/// Reemplaza los objetos de una capa de objetos (con sus tracks normalizados); los ids no se
/// pueden repetir.
fn set_objects(
    doc: &mut Sprite,
    layer: LayerId,
    objects: &[SceneObject],
) -> Result<Command, CmdError> {
    let l = doc.layer_mut(layer).ok_or(CmdError::LayerNotFound(layer))?;
    if l.kind != LayerKind::Objects {
        return Err(CmdError::NotObjectLayer(layer));
    }
    let mut seen = std::collections::HashSet::new();
    if let Some(o) = objects.iter().find(|o| !seen.insert(o.id)) {
        return Err(CmdError::DuplicateObject(o.id));
    }
    let mut objects = objects.to_vec();
    for object in &mut objects {
        object.track.normalize();
    }
    let old = std::mem::replace(&mut l.objects, objects);
    Ok(Command::SetObjects {
        layer,
        objects: old,
    })
}

/// Corre las keys de todos los objetos por `f`; devuelve los comandos que los restauran (si dos
/// keys de un objeto caen en el mismo frame gana la última).
fn remap_instances(doc: &mut Sprite, f: impl Fn(usize) -> usize) -> Vec<Command> {
    let mut inverse = Vec::new();
    for layer in doc.layers.iter_mut().filter(|l| !l.objects.is_empty()) {
        let old = layer.objects.clone();
        for object in &mut layer.objects {
            let keys = std::mem::take(&mut object.track.keys);
            for key in &keys {
                object.track.set_key(InstanceKey {
                    frame: f(key.frame),
                    ..*key
                });
            }
        }
        inverse.push(Command::SetObjects {
            layer: layer.id,
            objects: old,
        });
    }
    inverse
}

fn add_frame(doc: &mut Sprite, index: usize, frame: Frame) -> Result<Command, CmdError> {
    if index > doc.frames.len() {
        return Err(CmdError::IndexOutOfRange(index));
    }
    remap_frames(doc, |f| if f >= index { f + 1 } else { f });
    doc.frames.insert(index, frame);
    let old_tags = std::mem::take(&mut doc.tags);
    doc.tags = tags_after_insert(&old_tags, index);
    let old_slices = std::mem::take(&mut doc.slices);
    // Las claves en el frame insertado o después se corren, salvo la del frame 0 (vale desde el
    // principio).
    doc.slices = remap_slices(&old_slices, |f| if f >= index && f > 0 { f + 1 } else { f });
    let mut inverse = vec![
        Command::RemoveFrame { index },
        Command::SetTags(old_tags),
        Command::SetSlices(old_slices),
    ];
    // Igual que los slices: la key del frame 0 vale desde el principio y no se corre.
    inverse.extend(remap_instances(doc, |f| {
        if f >= index && f > 0 { f + 1 } else { f }
    }));
    Ok(Command::Batch(inverse))
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
    let old_tags = std::mem::take(&mut doc.tags);
    doc.tags = tags_after_remove(&old_tags, index);
    inverse.push(Command::SetTags(old_tags));
    let old_slices = std::mem::take(&mut doc.slices);
    doc.slices = remap_slices(&old_slices, |f| if f > index { f - 1 } else { f });
    inverse.push(Command::SetSlices(old_slices));
    inverse.extend(remap_instances(doc, |f| if f > index { f - 1 } else { f }));
    // Una key en el último frame quitado queda fuera de rango: pasa al nuevo último frame si
    // ahí no hay otra (si la hay, esa sigue mandando y la quitada se descarta).
    let last = doc.frames.len() - 1;
    for track in doc
        .layers
        .iter_mut()
        .flat_map(|l| l.objects.iter_mut())
        .map(|o| &mut o.track)
    {
        let taken = track.keys.iter().any(|k| k.frame == last);
        if taken {
            track.keys.retain(|k| k.frame <= last);
        } else if let Some(key) = track.keys.iter_mut().find(|k| k.frame > last) {
            key.frame = last;
        }
    }
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
mod tests;
