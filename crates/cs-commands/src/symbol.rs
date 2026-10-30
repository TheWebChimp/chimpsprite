//! Símbolos de la biblioteca: convertir capas o píxeles en un símbolo (F8), mandar artboards a
//! la biblioteca y sacarlos al lienzo.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use cs_core::{
    Artboard, ArtboardId, ColorMode, Document, GraphicPlay, Image, LayerId, LayerKind, Mask,
    PixelValue, Rgba, Sprite, Timing,
};

use crate::{Command, DocCommand, ops};

#[cfg(test)]
mod tests;

/// Separación entre un artboard nuevo y los demás, como en `Editor::add_artboard`.
const GAP: i32 = 16;

/// Lugar libre para un artboard nuevo en el lienzo: a la derecha de todos.
pub fn free_position(doc: &Document) -> (i32, i32) {
    match doc.bounds() {
        Some(((_, y0), (x1, _))) => (x1 + GAP, y0),
        None => (0, 0),
    }
}

/// Manda `id` a la biblioteca. `None` si ya está ahí o si es el último artboard del lienzo.
pub fn move_to_library(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    (!artboard.in_library && doc.canvas_count() > 1).then_some(DocCommand::SetArtboardInLibrary {
        id,
        in_library: true,
    })
}

/// Saca `id` de la biblioteca a su lugar o, si choca con otro artboard del lienzo, a la derecha
/// de todos.
pub fn place_on_canvas(doc: &Document, id: ArtboardId) -> Option<DocCommand> {
    let artboard = doc.artboard(id)?;
    if !artboard.in_library {
        return None;
    }
    let show = DocCommand::SetArtboardInLibrary {
        id,
        in_library: false,
    };
    if !collides(doc, artboard) {
        return Some(show);
    }
    Some(DocCommand::Batch(vec![
        DocCommand::SetArtboardProps {
            id,
            name: None,
            position: Some(free_position(doc)),
        },
        show,
    ]))
}

/// ¿El rectángulo de `artboard` se encima con el de algún artboard del lienzo?
fn collides(doc: &Document, artboard: &Artboard) -> bool {
    let ((ax0, ay0), (ax1, ay1)) = artboard.rect();
    doc.canvas_artboards().any(|other| {
        let ((bx0, by0), (bx1, by1)) = other.rect();
        ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1
    })
}

/// Lo que F8 convierte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolSource {
    /// Las capas completas (con sus hijas, si son grupos).
    Layers(Vec<LayerId>),
    /// Los píxeles de `selection` en esas capas, en todos los frames (solo capas de imagen).
    Pixels {
        layers: Vec<LayerId>,
        selection: Mask,
    },
}

/// El "Type" del diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolTiming {
    Graphic,
    MovieClip,
}

impl SymbolTiming {
    /// El `Timing` de la key de la instancia nueva: desde el primer frame del símbolo.
    pub fn timing(self) -> Timing {
        match self {
            SymbolTiming::Graphic => Timing::Graphic {
                first_frame: 0,
                play: GraphicPlay::Loop,
            },
            SymbolTiming::MovieClip => Timing::MovieClip { first_frame: 0 },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConvertError {
    #[error("Symbols need an RGBA sprite")]
    NotRgba,
    #[error("Tilemap layers can't go into a symbol")]
    Tilemap,
    #[error("Nothing to convert")]
    Empty,
    #[error("Select the layers to convert")]
    NoLayers,
    /// Capas de grupos distintos o con otras capas en el medio.
    #[error("Select layers from the same group")]
    Scattered,
    /// Con marquesina, alguna capa de imagen está bloqueada.
    #[error("Unlock the layers first")]
    Locked,
    /// Separaría una capa que recorta de su base.
    #[error("Clipping layers can't be split into a symbol")]
    Clipping,
}

/// Lo que arma `convert_to_symbol`: un solo comando (un Ctrl+Z) y los ids nuevos.
#[derive(Clone, Debug, PartialEq)]
pub struct Converted {
    pub command: DocCommand,
    pub symbol: ArtboardId,
    pub instance: LayerId,
}

/// Convierte capas o píxeles del artboard `host` en un símbolo nuevo de la biblioteca y deja en
/// su lugar una capa de instancia que se ve igual en todos los frames.
///
/// Los grupos del host que siguen ahí (con otras capas) no se copian al símbolo: la instancia
/// queda adentro de ellos y le aplican una sola vez. Los que quedan vacíos se borran del host,
/// viajan al símbolo y la instancia ocupa su lugar.
pub fn convert_to_symbol(
    doc: &Document,
    host: ArtboardId,
    source: &SymbolSource,
    name: &str,
    timing: SymbolTiming,
) -> Result<Converted, ConvertError> {
    let sprite = &doc.artboard(host).ok_or(ConvertError::NoLayers)?.sprite;
    let (listed, selection) = match source {
        SymbolSource::Layers(layers) => (layers, None),
        SymbolSource::Pixels { layers, selection } => (layers, Some(selection)),
    };
    let roots = roots(sprite, listed);
    if roots.is_empty() {
        return Err(ConvertError::NoLayers);
    }
    if sprite.color_mode != ColorMode::Rgba {
        return Err(ConvertError::NotRgba);
    }
    let converted: BTreeSet<LayerId> = roots
        .iter()
        .flat_map(|&id| sprite.layers[sprite.subtree_range(id).expect("root exists")].iter())
        .map(|l| l.id)
        .collect();
    let layer = |id: LayerId| sprite.layer(id).expect("converted layer exists");
    if converted
        .iter()
        .any(|&id| layer(id).kind == LayerKind::Tilemap)
    {
        return Err(ConvertError::Tilemap);
    }
    // Lo que no se podría convertir sin cambiar el render se rechaza.
    let (cut, removal) = match selection {
        None => {
            together(sprite, &roots, &converted)?;
            let removal = remove_layers(sprite, &roots);
            unclipped(sprite, &(&converted | &removal.groups))?;
            (converted.clone(), Some(removal))
        }
        Some(_) => {
            // Con marquesina solo se cortan las capas de imagen.
            let cut: BTreeSet<LayerId> = converted
                .iter()
                .copied()
                .filter(|&id| layer(id).kind == LayerKind::Image)
                .collect();
            if cut.iter().any(|&id| layer(id).locked) {
                return Err(ConvertError::Locked);
            }
            let units: Vec<LayerId> = cut.iter().copied().collect();
            together(sprite, &units, &cut)?;
            unclipped(sprite, &cut)?;
            (cut, None)
        }
    };
    let rect = content_rect(doc, sprite, &cut, selection).ok_or(ConvertError::Empty)?;

    let symbol_id = doc.next_artboard_id();
    // Id libre en el host original, para no reusar el de una capa que se borra.
    let Command::AddLayer { mut layer, .. } =
        ops::new_instance_layer(sprite, None, 0, symbol_id, name, (rect.0, rect.1))
    else {
        unreachable!("new_instance_layer devuelve AddLayer");
    };
    if let Some(track) = &mut layer.instance {
        track.keys[0].timing = timing.timing();
    }
    let instance = layer.id;

    let (mut host_cmds, keep, index, parent) = match removal {
        Some(removal) => {
            // La instancia ocupa el lugar del bloque borrado de más arriba.
            let top = roots
                .iter()
                .map(|&root| removal.outermost(sprite, root))
                .max_by_key(|&id| sprite.layer_index(id))
                .expect("there are roots");
            let start = sprite.layer_index(top);
            let index = removal
                .after
                .layers
                .iter()
                .filter(|l| sprite.layer_index(l.id) < start)
                .count();
            let keep = &converted | &removal.groups;
            (removal.cmds, keep, index, layer_parent(sprite, top))
        }
        None => {
            let mask = selection.expect("only pixel mode removes nothing");
            // Encima de la capa cortada de más arriba, en su mismo grupo.
            let top = *cut
                .iter()
                .max_by_key(|&&id| sprite.layer_index(id))
                .expect("content implies a cut layer");
            let range = sprite.subtree_range(top).expect("top exists");
            let cmds = erase_pixels(sprite, &cut, mask);
            (cmds, cut.clone(), range.end, layer_parent(sprite, top))
        }
    };
    layer.parent = parent;
    host_cmds.push(Command::AddLayer { index, layer });

    let symbol = Artboard {
        id: symbol_id,
        name: name.to_string(),
        position: free_position(doc),
        sprite: symbol_sprite(sprite, &keep, &cut, selection, rect),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: true,
    };
    let command = DocCommand::Batch(vec![
        DocCommand::AddArtboard {
            index: doc.artboards.len(),
            artboard: Box::new(symbol),
        },
        DocCommand::OnArtboard {
            id: host,
            cmd: Command::Batch(host_cmds),
        },
    ]);
    Ok(Converted {
        command,
        symbol: symbol_id,
        instance,
    })
}

fn layer_parent(sprite: &Sprite, id: LayerId) -> Option<LayerId> {
    sprite.layer(id).and_then(|l| l.parent)
}

/// `units` (con sus descendientes, todos en `members`) son hermanas seguidas: mismo padre y sin
/// otras capas en el medio. Si no, la instancia (que va en un solo lugar) cambiaría el render.
fn together(
    sprite: &Sprite,
    units: &[LayerId],
    members: &BTreeSet<LayerId>,
) -> Result<(), ConvertError> {
    let Some(&first) = units.first() else {
        return Ok(());
    };
    let parent = layer_parent(sprite, first);
    if units.iter().any(|&id| layer_parent(sprite, id) != parent) {
        return Err(ConvertError::Scattered);
    }
    let ranges = units.iter().filter_map(|&id| sprite.subtree_range(id));
    let start = ranges.clone().map(|r| r.start).min().expect("units exist");
    let end = ranges.map(|r| r.end).max().expect("units exist");
    if sprite.layers[start..end]
        .iter()
        .any(|l| !members.contains(&l.id))
    {
        return Err(ConvertError::Scattered);
    }
    Ok(())
}

/// Ninguna capa que recorta queda de un lado (`moved` o no) y su base del otro.
fn unclipped(sprite: &Sprite, moved: &BTreeSet<LayerId>) -> Result<(), ConvertError> {
    let split = sprite.layers.iter().any(|l| {
        sprite
            .clip_base(l.id)
            .is_some_and(|base| moved.contains(&l.id) != moved.contains(&base))
    });
    if split {
        return Err(ConvertError::Clipping);
    }
    Ok(())
}

/// Las capas pedidas que existen, sin repetir y sin las que ya están dentro de otra pedida, de
/// abajo hacia arriba.
fn roots(sprite: &Sprite, listed: &[LayerId]) -> Vec<LayerId> {
    let exists: Vec<LayerId> = listed
        .iter()
        .copied()
        .filter(|&id| sprite.layer(id).is_some())
        .collect();
    let mut roots: Vec<LayerId> = exists
        .iter()
        .copied()
        .filter(|&id| !exists.iter().any(|&other| sprite.is_descendant(id, other)))
        .collect();
    roots.sort_by_key(|&id| sprite.layer_index(id));
    roots.dedup();
    roots
}

/// El rectángulo `(x, y, w, h)` con lo que se convierte en algún frame, recortado al lienzo.
/// De una capa de instancia cuenta el rectángulo entero de su fuente en cada key.
fn content_rect(
    doc: &Document,
    sprite: &Sprite,
    layers: &BTreeSet<LayerId>,
    selection: Option<&Mask>,
) -> Option<(i32, i32, u32, u32)> {
    let (cw, ch) = (sprite.width as i32, sprite.height as i32);
    let mut acc: Option<(i32, i32, i32, i32)> = None;
    let mut add = |x0: i32, y0: i32, x1: i32, y1: i32| {
        let (x0, y0, x1, y1) = (x0.max(0), y0.max(0), x1.min(cw - 1), y1.min(ch - 1));
        if x0 > x1 || y0 > y1 {
            return;
        }
        acc = Some(match acc {
            None => (x0, y0, x1, y1),
            Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
        });
    };
    for &id in layers {
        let layer = sprite.layer(id).expect("listed layer exists");
        match layer.kind {
            LayerKind::Image => {
                for frame in 0..sprite.frames.len() {
                    let Some(cel) = sprite.cel(id, frame) else {
                        continue;
                    };
                    let Some(image) = sprite.image(cel.image) else {
                        continue;
                    };
                    for (u, v) in visible_pixels(image) {
                        let (x, y) = (cel.position.0 + u, cel.position.1 + v);
                        if selection.is_none_or(|m| m.contains(x, y)) {
                            add(x, y, x, y);
                        }
                    }
                }
            }
            LayerKind::Instance => {
                for key in layer.instance.iter().flat_map(|t| t.keys.iter()) {
                    let Some(src) = key.source.and_then(|s| doc.artboard(s)) else {
                        continue;
                    };
                    let (w, h) = (src.sprite.width as i32, src.sprite.height as i32);
                    let (x, y) = key.position;
                    add(x, y, x + w - 1, y + h - 1);
                }
            }
            LayerKind::Group | LayerKind::Tilemap => {}
        }
    }
    acc.map(|(x0, y0, x1, y1)| (x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
}

/// Las coordenadas de los píxeles con alfa > 0 de una imagen RGBA.
fn visible_pixels(image: &Image) -> impl Iterator<Item = (i32, i32)> + '_ {
    (0..image.height()).flat_map(move |v| {
        (0..image.width()).filter_map(move |u| match image.get(u, v) {
            Some(PixelValue::Rgba(c)) if c.a > 0 => Some((u as i32, v as i32)),
            _ => None,
        })
    })
}

/// El sprite del símbolo: copia del host con las capas `keep` (las que no tienen su grupo ahí
/// quedan sueltas) y los cels de `cut`, desplazado para que `rect` empiece en (0, 0).
fn symbol_sprite(
    host: &Sprite,
    keep: &BTreeSet<LayerId>,
    cut: &BTreeSet<LayerId>,
    selection: Option<&Mask>,
    rect: (i32, i32, u32, u32),
) -> Sprite {
    let (dx, dy) = (-rect.0, -rect.1);
    let mut s = host.clone();
    s.width = rect.2;
    s.height = rect.3;
    s.layers.retain(|l| keep.contains(&l.id));
    s.cels.retain(|(layer, _), _| cut.contains(layer));
    s.slices.clear();
    s.tilesets.clear();
    s.tilemaps.clear();
    s.extra_chunks.clear();
    if let Some(mask) = selection {
        // Cada imagen una sola vez: los cels vinculados la comparten y también comparten
        // posición (`draw_pixels` los mueve juntos), así que la máscara cae igual en todos.
        // Por imagen: ¿quedó sin nada visible?
        let mut blank = BTreeMap::new();
        let mut at = BTreeMap::new();
        for cel in s.cels.values() {
            if let Some(&position) = at.get(&cel.image) {
                debug_assert_eq!(position, cel.position, "linked cels share position");
                continue;
            }
            at.insert(cel.image, cel.position);
            let Some(image) = s.images.get(&cel.image) else {
                continue;
            };
            let mut kept = (**image).clone();
            for v in 0..kept.height() {
                for u in 0..kept.width() {
                    let (x, y) = (cel.position.0 + u as i32, cel.position.1 + v as i32);
                    if !mask.contains(x, y) {
                        kept.set(u, v, PixelValue::Rgba(Rgba::TRANSPARENT))
                            .expect("in bounds and RGBA");
                    }
                }
            }
            blank.insert(cel.image, visible_pixels(&kept).next().is_none());
            s.images.insert(cel.image, Arc::new(kept));
        }
        // Los cels que quedaron sin nada no hacen falta.
        s.cels
            .retain(|_, cel| blank.get(&cel.image) == Some(&false));
    }
    for layer in &mut s.layers {
        if layer.parent.is_some_and(|p| !keep.contains(&p)) {
            layer.parent = None;
        }
        if let Some(mask) = &mut layer.mask {
            mask.position = (mask.position.0 + dx, mask.position.1 + dy);
        }
        if let Some(track) = &mut layer.instance {
            for key in &mut track.keys {
                key.position = (key.position.0 + dx, key.position.1 + dy);
            }
        }
    }
    for cel in s.cels.values_mut() {
        cel.position = (cel.position.0 + dx, cel.position.1 + dy);
    }
    let used: BTreeSet<_> = s.cels.values().map(|c| c.image).collect();
    s.images.retain(|id, _| used.contains(id));
    s
}

/// Lo que queda de borrar las capas convertidas del host.
struct Removal {
    cmds: Vec<Command>,
    /// Los grupos ancestros que quedaron vacíos y también se borraron.
    groups: BTreeSet<LayerId>,
    /// El host después de borrar.
    after: Sprite,
}

impl Removal {
    /// El bloque que se fue con `root`: su grupo ancestro borrado de más afuera, o ella misma.
    fn outermost(&self, sprite: &Sprite, root: LayerId) -> LayerId {
        let mut top = root;
        while let Some(p) = layer_parent(sprite, top).filter(|p| self.groups.contains(p)) {
            top = p;
        }
        top
    }
}

/// Borra del host las capas convertidas y, después, los grupos que queden vacíos.
fn remove_layers(sprite: &Sprite, roots: &[LayerId]) -> Removal {
    let mut after = sprite.clone();
    let mut cmds = Vec::new();
    let mut groups = BTreeSet::new();
    let run = |cmd: Command, after: &mut Sprite, cmds: &mut Vec<Command>| {
        cmd.apply(after).expect("removal is valid");
        cmds.push(cmd);
    };
    for &root in roots.iter().rev() {
        if let Some(cmd) = ops::remove_layer_tree(&after, root) {
            run(cmd, &mut after, &mut cmds);
        }
    }
    let mut parents: Vec<LayerId> = roots
        .iter()
        .filter_map(|&r| layer_parent(sprite, r))
        .collect();
    while let Some(group) = parents.pop() {
        let empty =
            after.layer(group).is_some() && !after.layers.iter().any(|l| l.parent == Some(group));
        if empty {
            let up = layer_parent(&after, group);
            run(Command::RemoveLayer { id: group }, &mut after, &mut cmds);
            groups.insert(group);
            parents.extend(up);
        }
    }
    Removal {
        cmds,
        groups,
        after,
    }
}

/// Borra los píxeles de `mask` en las capas `cut`, en todos los frames.
fn erase_pixels(sprite: &Sprite, cut: &BTreeSet<LayerId>, mask: &Mask) -> Vec<Command> {
    let clear = PixelValue::Rgba(Rgba::TRANSPARENT);
    let mut done = BTreeSet::new();
    let mut cmds = Vec::new();
    for &layer in cut {
        for frame in 0..sprite.frames.len() {
            let Some(cel) = sprite.cel(layer, frame) else {
                continue;
            };
            // Borrar en una imagen vinculada ya la borra en todos sus cels.
            if !done.insert(cel.image) {
                continue;
            }
            let Some(image) = sprite.image(cel.image) else {
                continue;
            };
            let pixels: Vec<_> = visible_pixels(image)
                .map(|(u, v)| (cel.position.0 + u, cel.position.1 + v))
                .filter(|&(x, y)| mask.contains(x, y))
                .map(|(x, y)| (x, y, clear))
                .collect();
            if !pixels.is_empty() {
                cmds.push(Command::DrawPixels {
                    layer,
                    frame,
                    pixels,
                });
            }
        }
    }
    cmds
}
