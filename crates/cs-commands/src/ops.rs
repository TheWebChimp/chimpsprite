//! Operaciones compuestas: arman `Command`s a partir del estado actual del sprite.

use std::collections::HashMap;
use std::sync::Arc;

use cs_core::{
    ArtboardId, Cel, ColorMode, Image, ImageId, InstanceKey, InstanceTrack, Layer, LayerId,
    LayerKind, LayerMask, MAX_TILESET_TILES, Mask, ObjectId, PixelBuffer, PixelValue, SceneObject,
    Sprite, Tile, TileGrid, Tileset,
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
        LayerKind::Objects => "Objects",
        LayerKind::Tilemap => "Tilemap",
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

/// Capa de objetos sobre `above` con un objeto que muestra `source`, con una key en `frame`.
/// La capa y su objeto se llaman `name`.
pub fn new_object_layer(
    sprite: &Sprite,
    above: Option<LayerId>,
    frame: usize,
    source: ArtboardId,
    name: &str,
    position: (i32, i32),
) -> Command {
    let Command::AddLayer { index, mut layer } = new_layer(sprite, above, LayerKind::Objects)
    else {
        unreachable!("new_layer devuelve AddLayer");
    };
    layer.name = name.to_string();
    layer.objects = vec![SceneObject {
        id: ObjectId(1),
        name: name.to_string(),
        track: InstanceTrack {
            keys: vec![InstanceKey::new(frame, source, position)],
        },
    }];
    Command::AddLayer { index, layer }
}

/// Cambia con `f` la key vigente de `object` (de `layer`) en `frame`; si `frame` no es keyframe,
/// la copia ahí primero. `None` si no hay key vigente o nada cambia.
pub fn update_object_key(
    sprite: &Sprite,
    layer: LayerId,
    object: ObjectId,
    frame: usize,
    f: impl FnOnce(&mut InstanceKey),
) -> Option<Command> {
    let mut f = Some(f);
    update_objects_key(sprite, layer, &[object], frame, |k| {
        if let Some(f) = f.take() {
            f(k);
        }
    })
}

/// Como `update_object_key`, sobre cada uno de `objects` en un solo comando (un paso de undo).
/// Los que no existen o no tienen key vigente en `frame` se saltean; `None` si nada cambia.
pub fn update_objects_key(
    sprite: &Sprite,
    layer: LayerId,
    objects: &[ObjectId],
    frame: usize,
    mut f: impl FnMut(&mut InstanceKey),
) -> Option<Command> {
    let old = &sprite.layer(layer)?.objects;
    let mut new = old.clone();
    for o in new.iter_mut().filter(|o| objects.contains(&o.id)) {
        let Some(&key) = o.track.key_at(frame) else {
            continue;
        };
        let mut key = key;
        key.frame = frame;
        f(&mut key);
        o.track.set_key(key);
    }
    (new != *old).then_some(Command::SetObjects {
        layer,
        objects: new,
    })
}

/// Le pone `name` a `object` de `layer`. `None` si no existe o ya se llama así.
pub fn rename_object(
    sprite: &Sprite,
    layer: LayerId,
    object: ObjectId,
    name: &str,
) -> Option<Command> {
    let old = &sprite.layer(layer)?.objects;
    let i = old.iter().position(|o| o.id == object)?;
    if old[i].name == name {
        return None;
    }
    let mut new = old.clone();
    new[i].name = name.to_string();
    Some(Command::SetObjects {
        layer,
        objects: new,
    })
}

/// Quita la key de `object` que empieza en `frame`. Si es la única, quita el objeto; y si era el
/// único objeto, quita la capa.
pub fn remove_object_key(
    sprite: &Sprite,
    layer: LayerId,
    object: ObjectId,
    frame: usize,
) -> Option<Command> {
    let objects = &sprite.layer(layer)?.objects;
    let i = objects.iter().position(|o| o.id == object)?;
    let track = &objects[i].track;
    if !track.keys.iter().any(|k| k.frame == frame) {
        return None;
    }
    let mut new = objects.clone();
    if track.keys.len() == 1 {
        if objects.len() == 1 {
            return remove_layer_tree(sprite, layer);
        }
        new.remove(i);
    } else {
        new[i].track.remove_key(frame);
    }
    Some(Command::SetObjects {
        layer,
        objects: new,
    })
}

/// Hacia dónde se manda un grupo de objetos en la pila de su capa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reorder {
    Forward,
    Backward,
    Front,
    Back,
}

/// La capa de objetos `layer`, o `None` si no existe o es de otro tipo.
fn object_layer(sprite: &Sprite, layer: LayerId) -> Option<&Layer> {
    sprite.layer(layer).filter(|l| l.kind == LayerKind::Objects)
}

/// Objeto nuevo arriba de todos en `layer`, con una key en `frame` que muestra `source`.
pub fn add_object(
    sprite: &Sprite,
    layer: LayerId,
    frame: usize,
    source: ArtboardId,
    name: &str,
    position: (i32, i32),
) -> Option<(Command, ObjectId)> {
    let l = object_layer(sprite, layer)?;
    let id = l.next_object_id();
    let mut objects = l.objects.clone();
    objects.push(SceneObject {
        id,
        name: name.to_string(),
        track: InstanceTrack {
            keys: vec![InstanceKey::new(frame, source, position)],
        },
    });
    Some((Command::SetObjects { layer, objects }, id))
}

/// Quita los objetos `ids`. Si no queda ninguno, la capa queda vacía (no se borra).
pub fn remove_objects(sprite: &Sprite, layer: LayerId, ids: &[ObjectId]) -> Option<Command> {
    let l = object_layer(sprite, layer)?;
    let mut objects = l.objects.clone();
    objects.retain(|o| !ids.contains(&o.id));
    (objects.len() != l.objects.len()).then_some(Command::SetObjects { layer, objects })
}

/// Desplaza los objetos `ids` en `frame` (crea la key ahí si no existe, copiando la vigente).
/// Los objetos sin key vigente en `frame` se saltan.
pub fn move_objects(
    sprite: &Sprite,
    layer: LayerId,
    ids: &[ObjectId],
    frame: usize,
    dx: i32,
    dy: i32,
) -> Option<Command> {
    let l = object_layer(sprite, layer)?;
    let mut objects = l.objects.clone();
    shift_objects(&mut objects, ids, frame, (dx, dy))
        .then_some(Command::SetObjects { layer, objects })
}

/// Corre `offset` la key en `frame` de los objetos `ids` (la crea copiando la vigente). Devuelve
/// si alguno cambió.
fn shift_objects(
    objects: &mut [SceneObject],
    ids: &[ObjectId],
    frame: usize,
    offset: (i32, i32),
) -> bool {
    if offset == (0, 0) {
        return false;
    }
    let mut changed = false;
    for o in objects.iter_mut().filter(|o| ids.contains(&o.id)) {
        let Some(mut key) = o.track.key_at(frame).copied() else {
            continue;
        };
        key.frame = frame;
        key.position = (key.position.0 + offset.0, key.position.1 + offset.1);
        o.track.set_key(key);
        changed = true;
    }
    changed
}

/// Copias de los objetos `ids` (como `duplicate_objects` sin desplazar) corridas `(dx, dy)` en
/// `frame`, en un solo comando: Alt+arrastrar con Move. Los originales no cambian.
pub fn duplicate_and_move_objects(
    sprite: &Sprite,
    layer: LayerId,
    ids: &[ObjectId],
    frame: usize,
    dx: i32,
    dy: i32,
) -> Option<(Command, Vec<ObjectId>)> {
    if (dx, dy) == (0, 0) {
        return None;
    }
    let (Command::SetObjects { layer, mut objects }, new_ids) =
        duplicate_objects(sprite, layer, ids, (0, 0))?
    else {
        unreachable!("duplicate_objects devuelve SetObjects");
    };
    shift_objects(&mut objects, &new_ids, frame, (dx, dy));
    Some((Command::SetObjects { layer, objects }, new_ids))
}

/// Reordena los objetos `ids` conservando su orden relativo.
pub fn reorder_objects(
    sprite: &Sprite,
    layer: LayerId,
    ids: &[ObjectId],
    to: Reorder,
) -> Option<Command> {
    let l = object_layer(sprite, layer)?;
    let mut objects = l.objects.clone();
    let n = objects.len();
    let sel = |o: &SceneObject| ids.contains(&o.id);
    match to {
        Reorder::Forward => {
            for i in (0..n.saturating_sub(1)).rev() {
                if sel(&objects[i]) && !sel(&objects[i + 1]) {
                    objects.swap(i, i + 1);
                }
            }
        }
        Reorder::Backward => {
            for i in 1..n {
                if sel(&objects[i]) && !sel(&objects[i - 1]) {
                    objects.swap(i, i - 1);
                }
            }
        }
        Reorder::Front => objects.sort_by_key(sel),
        Reorder::Back => objects.sort_by_key(|o| !sel(o)),
    }
    (objects != l.objects).then_some(Command::SetObjects { layer, objects })
}

/// Copias de los objetos `ids` (arriba de todos, con ids nuevos y nombre "<nombre> copy"),
/// desplazadas `offset` en todas sus keys.
pub fn duplicate_objects(
    sprite: &Sprite,
    layer: LayerId,
    ids: &[ObjectId],
    offset: (i32, i32),
) -> Option<(Command, Vec<ObjectId>)> {
    let l = object_layer(sprite, layer)?;
    let mut objects = l.objects.clone();
    let picked = l.objects.iter().filter(|o| ids.contains(&o.id));
    let mut new_ids = Vec::new();
    for (next, o) in (l.next_object_id().0..).zip(picked) {
        let mut copy = o.clone();
        copy.id = ObjectId(next);
        copy.name = format!("{} copy", o.name);
        for k in &mut copy.track.keys {
            k.position = (k.position.0 + offset.0, k.position.1 + offset.1);
        }
        new_ids.push(copy.id);
        objects.push(copy);
    }
    (!new_ids.is_empty()).then_some((Command::SetObjects { layer, objects }, new_ids))
}

/// Capa tilemap nueva sobre `above` que dibuja con el tileset `tileset`.
pub fn new_tilemap_layer(sprite: &Sprite, above: Option<LayerId>, tileset: u32) -> Command {
    let Command::AddLayer { index, mut layer } = new_layer(sprite, above, LayerKind::Tilemap)
    else {
        unreachable!("new_layer devuelve AddLayer");
    };
    layer.tileset = Some(tileset);
    Command::AddLayer { index, layer }
}

/// Lado máximo de una grilla de tilemap, en tiles (protege contra pedidos absurdos).
pub const MAX_TILEMAP_SIDE: u32 = 4096;

/// ¿Una grilla de `width`×`height` tiles de `tile_size` cabe en `MAX_TILEMAP_PIXELS` al dibujarse?
pub fn tilemap_fits(width: u64, height: u64, tile_size: (u32, u32)) -> bool {
    let w = width.saturating_mul(tile_size.0 as u64);
    let h = height.saturating_mul(tile_size.1 as u64);
    w.saturating_mul(h) <= cs_core::MAX_TILEMAP_PIXELS
}

/// Pone tiles en el cel tilemap `(layer, frame)`. Las coordenadas son celdas de la grilla del
/// lienzo: la celda `(x, y)` empieza en el píxel `(x · ancho_tile, y · alto_tile)` (corrida por el
/// resto de la posición del cel si no está alineado). Sin cel, crea uno en (0, 0). Si las celdas
/// caen fuera de la grilla, la agranda con vacíos y corre la posición del cel si crece hacia la
/// izquierda o hacia arriba. Un cel vinculado recibe una grilla propia. `None` si la capa no es
/// tilemap, su tileset no existe, no hay celdas, nada cambia o la grilla pasaría
/// `MAX_TILEMAP_SIDE` de lado o `MAX_TILEMAP_PIXELS` dibujada (ver `tilemap_fits`).
pub fn set_tiles(
    sprite: &Sprite,
    layer: LayerId,
    frame: usize,
    cells: &[(i32, i32, Tile)],
) -> Option<Command> {
    let l = sprite.layer(layer)?;
    if l.kind != LayerKind::Tilemap || cells.is_empty() || frame >= sprite.frames.len() {
        return None;
    }
    let tileset = sprite.tilesets.get(l.tileset? as usize)?;
    let (tw, th) = (tileset.tile_size.0 as i32, tileset.tile_size.1 as i32);
    let old_cel = sprite.cel(layer, frame).copied();
    let old_grid: Option<&TileGrid> = old_cel.and_then(|c| sprite.tilemap(c.image)).map(|g| &**g);
    let (ow, oh) = old_grid.map_or((0, 0), |g| (g.width as i32, g.height as i32));
    let position = old_cel.map_or((0, 0), |c| c.position);
    // Celda del lienzo donde empieza la grilla actual.
    let origin = (position.0.div_euclid(tw), position.1.div_euclid(th));
    let (mut x0, mut y0, mut x1, mut y1) = (0i64, 0i64, ow as i64, oh as i64);
    for &(x, y, _) in cells {
        // Usa aritmética saturada para evitar overflow en i32::MAX.
        let (rx, ry) = (
            (x as i64).saturating_sub(origin.0 as i64),
            (y as i64).saturating_sub(origin.1 as i64),
        );
        x0 = x0.min(rx);
        y0 = y0.min(ry);
        x1 = x1.max(rx.saturating_add(1));
        y1 = y1.max(ry.saturating_add(1));
    }

    // Valida el tamaño de la grilla antes de asignar.
    let width = x1 - x0;
    let height = y1 - y0;
    if width <= 0
        || height <= 0
        || width > MAX_TILEMAP_SIDE as i64
        || height > MAX_TILEMAP_SIDE as i64
    {
        return None;
    }
    // Ni una grilla que, dibujada, pase el límite que el lector de `.aseprite` acepta (se
    // descartaría al abrir el archivo guardado).
    if !tilemap_fits(width as u64, height as u64, tileset.tile_size) {
        return None;
    }
    let x0_i32 = x0 as i32;
    let y0_i32 = y0 as i32;
    let mut grid = TileGrid::new(width as u32, height as u32);
    if let Some(old) = old_grid {
        for y in 0..old.height {
            for x in 0..old.width {
                let tile = old.get(x, y).unwrap_or(Tile::EMPTY);
                grid.set((x as i32 - x0_i32) as u32, (y as i32 - y0_i32) as u32, tile);
            }
        }
    }
    for &(x, y, tile) in cells {
        grid.set(
            (x - origin.0 - x0_i32) as u32,
            (y - origin.1 - y0_i32) as u32,
            tile,
        );
    }
    let new_position = (position.0 + x0_i32 * tw, position.1 + y0_i32 * th);
    if old_grid == Some(&grid) && new_position == position {
        return None;
    }
    // Un cel vinculado recibe una grilla nueva; si no, se reemplaza la suya.
    let id = match old_cel {
        Some(c) if !is_linked(sprite, layer, frame) => c.image,
        _ => sprite.next_image_id(),
    };
    Some(Command::Batch(vec![
        Command::SetTilemap {
            id,
            grid: Some(Arc::new(grid)),
        },
        Command::SetCel {
            layer,
            frame,
            cel: Some(Cel {
                position: new_position,
                opacity: old_cel.map_or(255, |c| c.opacity),
                image: id,
            }),
        },
    ]))
}

/// Reemplaza la imagen del tile `index` del tileset `tileset`, o la agrega si
/// `index == tiles.len()`. `None` si es el tile 0 (el vacío), el índice o el tileset no existen,
/// la imagen no tiene el tamaño o el modo de color del tileset, o nada cambia.
pub fn set_tile_image(
    sprite: &Sprite,
    tileset: usize,
    index: usize,
    image: Image,
) -> Option<Command> {
    let ts = sprite.tilesets.get(tileset)?;
    if index == 0 || index > ts.tiles.len() || (image.width(), image.height()) != ts.tile_size {
        return None;
    }
    let same_mode = matches!(
        (image.pixels(), sprite.color_mode),
        (PixelBuffer::Rgba(_), ColorMode::Rgba)
            | (PixelBuffer::Indexed(_), ColorMode::Indexed { .. })
    );
    if !same_mode {
        return None;
    }
    let mut new = ts.clone();
    if index == new.tiles.len() {
        new.tiles.push(Arc::new(image));
    } else {
        new.tiles[index] = Arc::new(image);
    }
    (new != *ts).then_some(Command::SetTileset {
        index: tileset,
        tileset: Some(new),
    })
}

/// Copia del contenido de un cel (imagen o grilla de tiles) con el id `to`.
fn copy_content(sprite: &Sprite, from: ImageId, to: ImageId) -> Command {
    match sprite.tilemap(from) {
        Some(grid) => Command::SetTilemap {
            id: to,
            grid: Some(grid.clone()),
        },
        None => Command::SetImage {
            id: to,
            image: sprite.image(from).cloned(),
        },
    }
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
            cmds.push(copy_content(sprite, cel.image, id));
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

/// Un cel: su capa y su frame.
pub type CelAt = (LayerId, usize);

/// Mueve (o copia, con `copy`) cels: cada par va de `(capa, frame)` a `(capa, frame)`. Lo que
/// había en el destino se reemplaza (un cel vacío deja el destino vacío) y, al mover, los
/// orígenes que no son destino de otro quedan vacíos. Las copias llevan su propia imagen. `None`
/// si no cambia nada.
pub fn move_cels(sprite: &Sprite, moves: &[(CelAt, CelAt)], copy: bool) -> Option<Command> {
    let mut cmds = Vec::new();
    if !copy {
        let dests: std::collections::HashSet<_> = moves.iter().map(|m| m.1).collect();
        for &(from, _) in moves {
            if !dests.contains(&from) && sprite.cel(from.0, from.1).is_some() {
                cmds.push(Command::SetCel {
                    layer: from.0,
                    frame: from.1,
                    cel: None,
                });
            }
        }
    }
    let mut next_id = sprite.next_image_id().0;
    for &(from, to) in moves {
        let cel = match sprite.cel(from.0, from.1).copied() {
            Some(cel) if copy => {
                let id = ImageId(next_id);
                next_id += 1;
                cmds.push(copy_content(sprite, cel.image, id));
                Some(Cel { image: id, ..cel })
            }
            cel => cel,
        };
        if from == to && !copy {
            continue;
        }
        if cel.is_some() || sprite.cel(to.0, to.1).is_some() {
            cmds.push(Command::SetCel {
                layer: to.0,
                frame: to.1,
                cel,
            });
        }
    }
    (!cmds.is_empty()).then_some(Command::Batch(cmds))
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
        cmds.push(copy_content(sprite, cel.image, id));
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
        copy_content(sprite, cel.image, id),
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

/// Posición de un cel en el lienzo.
type Pos = (i32, i32);

/// Lado máximo de un tile al crear tilesets desde la UI o al convertir una capa.
pub const MAX_TILE_SIDE: u32 = 256;

/// Clave para comparar tiles por igualdad exacta de píxeles.
fn pixel_key(image: &Image) -> Vec<u8> {
    match image.pixels() {
        PixelBuffer::Rgba(px) => px.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect(),
        PixelBuffer::Indexed(px) => px.clone(),
    }
}

/// Un píxel que no se ve: alfa 0 en RGBA, el índice transparente en indexado.
fn is_clear(value: PixelValue, mode: ColorMode) -> bool {
    match value {
        PixelValue::Rgba(c) => c.a == 0,
        v => v == mode.transparent(),
    }
}

/// Cambia el tipo de una capa conservando id, índice y propiedades: la quita (con sus cels; las
/// imágenes y grillas que quedan sin uso salen del pool) y agrega `layer` en el mismo índice. El
/// inverso de las dos restaura todo exacto.
fn replace_layer(sprite: &Sprite, layer: Layer) -> Option<[Command; 2]> {
    let index = sprite.layer_index(layer.id)?;
    Some([
        Command::RemoveLayer { id: layer.id },
        Command::AddLayer { index, layer },
    ])
}

/// Convierte la capa de imagen `layer` en una capa tilemap con un tileset nuevo (con el nombre de
/// la capa) de tiles de `tile_size`. Cada cel se corta en celdas de la grilla del lienzo (la celda
/// `(cx, cy)` son los píxeles `[cx·tw, (cx+1)·tw)`); solo cuentan las celdas que tocan la imagen
/// del cel. Una celda transparente es el tile vacío; las demás se comparan por igualdad exacta de
/// píxeles (sin flips) y las nuevas se agregan al tileset. Cada cel pasa a una grilla cuya
/// `position` es la esquina de su primera celda, con la misma opacidad; los cels vinculados siguen
/// vinculados. Los cels de 0×0 (que no dibujan nada) se omiten. Todo es un solo `Batch`. `None` si
/// la capa no es de imagen, algún cel apunta a una imagen que no existe, el tamaño de tile no es
/// válido (0, más de `MAX_TILE_SIDE` o más de `MAX_TILE_PIXELS`), harían falta más de
/// `MAX_TILESET_TILES` tiles o alguna grilla pasaría `MAX_TILEMAP_SIDE` / `tilemap_fits`.
pub fn convert_to_tilemap(
    sprite: &Sprite,
    layer: LayerId,
    tile_size: (u32, u32),
) -> Option<Command> {
    let l = sprite.layer(layer)?;
    let (tw, th) = tile_size;
    if l.kind != LayerKind::Image
        || !(1..=MAX_TILE_SIDE).contains(&tw)
        || !(1..=MAX_TILE_SIDE).contains(&th)
        || tw as u64 * th as u64 > cs_core::MAX_TILE_PIXELS
    {
        return None;
    }
    let mode = sprite.color_mode;
    let index = sprite.tilesets.len();
    let mut tileset = Tileset::new(l.name.clone(), tile_size, mode);
    let mut known: HashMap<Vec<u8>, u32> = HashMap::new();
    let mut next = sprite.next_image_id().0;
    let (mut grids, mut cels) = (Vec::new(), Vec::new());
    // Cels vinculados (misma imagen y posición) → la misma grilla.
    let mut done: HashMap<(ImageId, Pos), (ImageId, Pos)> = HashMap::new();
    let (tw_i, th_i) = (tw as i32, th as i32);
    for frame in 0..sprite.frames.len() {
        let Some(cel) = sprite.cel(layer, frame).copied() else {
            continue;
        };
        let (grid_id, position) = match done.get(&(cel.image, cel.position)) {
            Some(&found) => found,
            None => {
                // Un cel sin imagen es un documento roto: no se convierte (borraría el cel).
                let image = sprite.image(cel.image)?;
                let (px, py) = cel.position;
                let (iw, ih) = (image.width() as i32, image.height() as i32);
                if iw == 0 || ih == 0 {
                    // Un cel de 0×0 no dibuja nada: se omite a propósito.
                    continue;
                }
                let (cx0, cy0) = (px.div_euclid(tw_i), py.div_euclid(th_i));
                let (cx1, cy1) = (
                    (px + iw - 1).div_euclid(tw_i),
                    (py + ih - 1).div_euclid(th_i),
                );
                let (gw, gh) = ((cx1 - cx0 + 1) as u32, (cy1 - cy0 + 1) as u32);
                if gw > MAX_TILEMAP_SIDE
                    || gh > MAX_TILEMAP_SIDE
                    || !tilemap_fits(gw as u64, gh as u64, tile_size)
                {
                    return None;
                }
                let mut grid = TileGrid::new(gw, gh);
                for gy in 0..gh {
                    for gx in 0..gw {
                        let (x0, y0) = ((cx0 + gx as i32) * tw_i, (cy0 + gy as i32) * th_i);
                        let mut tile = Image::new(tw, th, mode);
                        let mut clear = true;
                        for ty in 0..th {
                            for tx in 0..tw {
                                let (lx, ly) = (x0 + tx as i32 - px, y0 + ty as i32 - py);
                                if lx < 0 || ly < 0 {
                                    continue;
                                }
                                let Some(v) = image.get(lx as u32, ly as u32) else {
                                    continue;
                                };
                                // Los píxeles invisibles se normalizan para que tiles que se
                                // ven igual (alfa 0 con distinto RGB) se deduplicen.
                                let v = if is_clear(v, mode) {
                                    mode.transparent()
                                } else {
                                    clear = false;
                                    v
                                };
                                tile.set(tx, ty, v).ok()?;
                            }
                        }
                        if clear {
                            continue;
                        }
                        let key = pixel_key(&tile);
                        let i = match known.get(&key) {
                            Some(&i) => i,
                            None => {
                                if tileset.tiles.len() as u32 >= MAX_TILESET_TILES {
                                    return None;
                                }
                                let i = tileset.tiles.len() as u32;
                                tileset.tiles.push(Arc::new(tile));
                                known.insert(key, i);
                                i
                            }
                        };
                        grid.set(gx, gy, Tile::new(i, false, false, false));
                    }
                }
                let id = ImageId(next);
                next += 1;
                let position = (cx0 * tw_i, cy0 * th_i);
                grids.push(Command::SetTilemap {
                    id,
                    grid: Some(Arc::new(grid)),
                });
                done.insert((cel.image, cel.position), (id, position));
                (id, position)
            }
        };
        cels.push(Command::SetCel {
            layer,
            frame,
            cel: Some(Cel {
                position,
                opacity: cel.opacity,
                image: grid_id,
            }),
        });
    }
    let mut new_layer = l.clone();
    new_layer.kind = LayerKind::Tilemap;
    new_layer.tileset = Some(index as u32);
    let mut cmds = vec![Command::SetTileset {
        index,
        tileset: Some(tileset),
    }];
    cmds.extend(replace_layer(sprite, new_layer)?);
    cmds.extend(grids);
    cmds.extend(cels);
    Some(Command::Batch(cmds))
}

/// Convierte la capa tilemap `layer` en una capa de imagen: cada cel pasa a su grilla dibujada
/// entera (sin recortar al lienzo, para no perder contenido), en la misma posición y con la misma
/// opacidad; los cels vinculados siguen vinculados. El tileset queda en el sprite. Un solo
/// `Batch`. Las grillas sin celdas (que no dibujan nada) se omiten. `None` si la capa no es
/// tilemap, su tileset no existe, algún cel apunta a una grilla que no existe o una grilla dibujada
/// pasaría `MAX_TILEMAP_PIXELS` (nunca se pierde un cel en silencio).
pub fn convert_to_image(sprite: &Sprite, layer: LayerId) -> Option<Command> {
    let l = sprite.layer(layer)?;
    if l.kind != LayerKind::Tilemap {
        return None;
    }
    let tileset = sprite.tilesets.get(l.tileset? as usize)?;
    let mut next = sprite.next_image_id().0;
    let (mut images, mut cels) = (Vec::new(), Vec::new());
    let mut done: HashMap<ImageId, ImageId> = HashMap::new();
    for frame in 0..sprite.frames.len() {
        let Some(cel) = sprite.cel(layer, frame).copied() else {
            continue;
        };
        let id = match done.get(&cel.image) {
            Some(&id) => id,
            None => {
                // Una grilla faltante es un documento roto: no se convierte (borraría el cel).
                let grid = sprite.tilemap(cel.image)?;
                if grid.width == 0 || grid.height == 0 {
                    // Una grilla sin celdas no dibuja nada: el cel se omite a propósito.
                    continue;
                }
                // Si no cabe (`MAX_TILEMAP_PIXELS`) se rechaza todo en vez de perder el cel.
                let image = cs_core::draw_tile_grid(sprite.color_mode, tileset, grid)?;
                let id = ImageId(next);
                next += 1;
                images.push(Command::SetImage {
                    id,
                    image: Some(Arc::new(image)),
                });
                done.insert(cel.image, id);
                id
            }
        };
        cels.push(Command::SetCel {
            layer,
            frame,
            cel: Some(Cel { image: id, ..cel }),
        });
    }
    let mut new_layer = l.clone();
    new_layer.kind = LayerKind::Image;
    new_layer.tileset = None;
    let mut cmds = Vec::from(replace_layer(sprite, new_layer)?);
    cmds.extend(images);
    cmds.extend(cels);
    Some(Command::Batch(cmds))
}

#[cfg(test)]
mod tests;
