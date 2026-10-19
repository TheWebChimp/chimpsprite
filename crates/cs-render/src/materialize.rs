//! Materializar: antes de componer, las capas que no son de imagen (instancias y tilemaps) se
//! convierten en capas de imagen con cels temporales; el compositor solo ve píxeles.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cs_core::{
    ArtboardId, Cel, ColorMode, Document, Image, ImageId, InstanceKey, InstanceTrack, LayerKind,
    PixelValue, Rgba, Sprite, Tile, TileGrid, Tileset, source_frame,
};

use crate::render_frame;

/// Profundidad máxima de instancias anidadas (defensa contra ciclos en datos corruptos).
const MAX_DEPTH: u32 = 16;

/// Qué capas convierte `bake_layers` en capas de imagen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bake {
    /// Instancias y tilemaps: para formatos de solo píxeles (PNG, GIF, hojas, atlas, motores).
    Everything,
    /// Solo instancias: para `.aseprite` (y el `.aseprite` interno del `.chimp`), que guarda los
    /// tilemaps como tilemaps.
    KeepTilemaps,
}

/// Copia del sprite donde las capas de instancia y las tilemap son capas de imagen con cels en
/// los frames de `frames`. Sin esas capas devuelve el mismo sprite. Es para mostrar: las capas
/// ocultas quedan sin cels.
pub fn materialize<'a>(
    doc: &Document,
    sprite: &'a Sprite,
    frames: Range<usize>,
) -> Cow<'a, Sprite> {
    let instances = materialize_root(doc, sprite, frames.clone(), true);
    with_tilemaps(instances, frames, true)
}

/// Como `materialize`, pero solo tilemaps y sin documento: para herramientas que leen lo visible.
pub fn materialize_tilemaps<'a>(sprite: &'a Sprite, frames: Range<usize>) -> Cow<'a, Sprite> {
    with_tilemaps(Cow::Borrowed(sprite), frames, true)
}

/// Todos los frames, incluidas las capas ocultas (sus píxeles se guardan como los de cualquier
/// capa oculta). `None` si no hay nada que hornear.
pub fn bake_layers(doc: &Document, sprite: &Sprite, bake: Bake) -> Option<Sprite> {
    let all = 0..sprite.frames.len();
    let baked = materialize_root(doc, sprite, all.clone(), false);
    let baked = match bake {
        Bake::Everything => with_tilemaps(baked, all, false),
        Bake::KeepTilemaps => baked,
    };
    match baked {
        Cow::Borrowed(_) => None,
        Cow::Owned(s) => Some(s),
    }
}

fn materialize_root<'a>(
    doc: &Document,
    sprite: &'a Sprite,
    frames: Range<usize>,
    skip_hidden: bool,
) -> Cow<'a, Sprite> {
    // Si el sprite es el de un artboard del documento, ese artboard ya está "en curso".
    let mut stack: Vec<ArtboardId> = doc
        .artboards
        .iter()
        .find(|a| std::ptr::eq(&a.sprite, sprite))
        .map(|a| a.id)
        .into_iter()
        .collect();
    materialize_at(doc, sprite, frames, &mut stack, skip_hidden)
}

fn materialize_at<'a>(
    doc: &Document,
    sprite: &'a Sprite,
    frames: Range<usize>,
    // Artboards que se están materializando (de afuera hacia adentro).
    stack: &mut Vec<ArtboardId>,
    // Dejar sin cels las capas de instancia ocultas (no se ven al componer).
    skip_hidden: bool,
) -> Cow<'a, Sprite> {
    if !sprite.layers.iter().any(|l| l.kind == LayerKind::Instance) {
        return Cow::Borrowed(sprite);
    }
    let mut out = sprite.clone();
    let mut next = sprite.next_image_id().0;
    let can_draw = sprite.color_mode == ColorMode::Rgba && (stack.len() as u32) < MAX_DEPTH;
    for layer in &mut out.layers {
        if layer.kind != LayerKind::Instance {
            continue;
        }
        layer.kind = LayerKind::Image;
        let Some(track) = layer.instance.take() else {
            continue;
        };
        // Una capa oculta no se ve: no hace falta renderizar su fuente.
        if !can_draw || (skip_hidden && !layer.visible) {
            continue;
        }
        for frame in frames.clone() {
            let Some((position, image)) = instance_image(doc, sprite, &track, frame, stack) else {
                continue;
            };
            let id = ImageId(next);
            next += 1;
            out.images.insert(id, Arc::new(image));
            let opacity = track.key_at(frame).map_or(255, |k| k.opacity);
            out.cels.insert(
                (layer.id, frame),
                Cel {
                    position,
                    opacity,
                    image: id,
                },
            );
        }
    }
    Cow::Owned(out)
}

/// Lo que muestra la instancia en `frame`: posición del cel e imagen recortada a lo opaco.
fn instance_image(
    doc: &Document,
    parent: &Sprite,
    track: &InstanceTrack,
    frame: usize,
    stack: &mut Vec<ArtboardId>,
) -> Option<((i32, i32), Image)> {
    let key: &InstanceKey = track.key_at(frame)?;
    let source_id = key.source?;
    // Un artboard que ya se está materializando no se dibuja dentro de sí mismo (ciclo).
    if stack.contains(&source_id) {
        return None;
    }
    let source = &doc.artboard(source_id)?.sprite;
    let sf = source_frame(key, parent, frame, source);
    stack.push(source_id);
    // Adentro del símbolo solo importa lo que se ve.
    // El símbolo puede tener tilemaps: también se convierten antes de componerlo.
    let inner = with_tilemaps(
        materialize_at(doc, source, sf..sf + 1, stack, true),
        sf..sf + 1,
        true,
    );
    let pixels = render_frame(&inner, sf);
    stack.pop();
    let (w, h) = (source.width, source.height);
    let at = |x: u32, y: u32| -> Rgba {
        let sx = if key.flip_h { w - 1 - x } else { x };
        let sy = if key.flip_v { h - 1 - y } else { y };
        pixels[(sy * w + sx) as usize]
    };
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if at(x, y).a > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
            }
        }
    }
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let mut image = Image::new(x1 - x0, y1 - y0, ColorMode::Rgba);
    for y in y0..y1 {
        for x in x0..x1 {
            image
                .set(x - x0, y - y0, PixelValue::Rgba(at(x, y)))
                .expect("in bounds and RGBA");
        }
    }
    let position = (key.position.0 + x0 as i32, key.position.1 + y0 as i32);
    Some((position, image))
}

/// `sprite` con las capas tilemap convertidas, o el mismo si no tiene.
fn with_tilemaps<'a>(
    sprite: Cow<'a, Sprite>,
    frames: Range<usize>,
    skip_hidden: bool,
) -> Cow<'a, Sprite> {
    match tilemaps_baked(&sprite, frames, skip_hidden) {
        Some(baked) => Cow::Owned(baked),
        None => sprite,
    }
}

/// Copia con las capas tilemap convertidas en capas de imagen: un cel por frame de `frames` con
/// la grilla dibujada. `None` si no hay capas tilemap. Con `skip_hidden`, las ocultas quedan sin
/// cels. La copia ya no tiene tilesets ni grillas.
fn tilemaps_baked(sprite: &Sprite, frames: Range<usize>, skip_hidden: bool) -> Option<Sprite> {
    if !sprite.layers.iter().any(|l| l.kind == LayerKind::Tilemap) {
        return None;
    }
    let mut out = sprite.clone();
    let mut next = sprite.next_image_id().0;
    // Los cels vinculados (misma grilla, mismo tileset) siguen compartiendo imagen.
    let mut done: HashMap<(ImageId, u32), ImageId> = HashMap::new();
    for layer in &mut out.layers {
        if layer.kind != LayerKind::Tilemap {
            continue;
        }
        layer.kind = LayerKind::Image;
        let index = layer.tileset.take();
        let tileset = index.and_then(|i| sprite.tilesets.get(i as usize));
        let draw = !skip_hidden || layer.visible;
        for frame in 0..sprite.frames.len() {
            let Some(cel) = out.cels.remove(&(layer.id, frame)) else {
                continue;
            };
            if !draw || !frames.contains(&frame) {
                continue;
            }
            let (Some(i), Some(tileset), Some(grid)) = (index, tileset, sprite.tilemap(cel.image))
            else {
                continue;
            };
            let key = (cel.image, i);
            let image = match done.get(&key) {
                Some(&id) => id,
                None => {
                    let id = ImageId(next);
                    next += 1;
                    let pixels = tilemap_image(sprite.color_mode, tileset, grid);
                    out.images.insert(id, Arc::new(pixels));
                    done.insert(key, id);
                    id
                }
            };
            out.cels.insert((layer.id, frame), Cel { image, ..cel });
        }
    }
    out.tilesets.clear();
    out.tilemaps.clear();
    Some(out)
}

/// Imagen de una grilla: cada tile se copia como valores (sin mezclar), con sus flips.
fn tilemap_image(mode: ColorMode, tileset: &Tileset, grid: &TileGrid) -> Image {
    let (tw, th) = tileset.tile_size;
    let mut image = Image::new(grid.width * tw, grid.height * th, mode);
    for gy in 0..grid.height {
        for gx in 0..grid.width {
            let Some(tile) = grid.get(gx, gy) else {
                continue;
            };
            let Some(src) = tile_image(tileset, tile) else {
                continue;
            };
            for y in 0..th {
                for x in 0..tw {
                    let (sx, sy) = tile_source(tile, tw, th, x, y);
                    if let Some(v) = src.get(sx, sy) {
                        // Un tile de otro modo de color (datos corruptos) no se dibuja.
                        let _ = image.set(gx * tw + x, gy * th + y, v);
                    }
                }
            }
        }
    }
    image
}

/// Avisado una sola vez: flip D en tiles no cuadrados.
static WARNED_DIAGONAL: AtomicBool = AtomicBool::new(false);

/// Imagen del tile, o `None` si la celda no dibuja nada: tile vacío, índice fuera de rango, imagen
/// de otro tamaño o flip D en un tile no cuadrado.
fn tile_image(tileset: &Tileset, tile: Tile) -> Option<&Image> {
    let index = tile.index() as usize;
    if index == 0 {
        return None;
    }
    let image = tileset.tiles.get(index)?;
    let (tw, th) = tileset.tile_size;
    if (image.width(), image.height()) != (tw, th) {
        return None;
    }
    if tile.flip_d() && tw != th {
        if !WARNED_DIAGONAL.swap(true, Ordering::Relaxed) {
            log::warn!("diagonal flip on non-square tiles ({tw}×{th}) is not drawn");
        }
        return None;
    }
    Some(image)
}

/// Píxel del tile original que se ve en `(x, y)` del tile volteado. Equivale a aplicar a la imagen
/// D (transponer), después X y después Y; el orden lo confirma `tilemap_file_renders_like_aseprite`
/// en `cs-io/tests/aseprite_interop.rs`.
fn tile_source(tile: Tile, w: u32, h: u32, x: u32, y: u32) -> (u32, u32) {
    let mut sx = if tile.flip_x() { w - 1 - x } else { x };
    let mut sy = if tile.flip_y() { h - 1 - y } else { y };
    if tile.flip_d() {
        std::mem::swap(&mut sx, &mut sy);
    }
    (sx, sy)
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::*;

    use super::*;
    use crate::render_frame;

    const RED: Rgba = Rgba::rgb(255, 0, 0);
    const BLUE: Rgba = Rgba::rgb(0, 0, 255);

    /// Símbolo de 3×2 con un píxel rojo en (0, 0) y uno azul en (2, 1), en 2 frames: en el
    /// frame 1 el azul pasa a (1, 1).
    fn symbol() -> Sprite {
        let mut s = Sprite::new(3, 2, ColorMode::Rgba);
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        for (frame, bx) in [(0, 2), (1, 1)] {
            Command::DrawPixels {
                layer: LayerId(1),
                frame,
                pixels: vec![
                    (0, 0, PixelValue::Rgba(RED)),
                    (bx, 1, PixelValue::Rgba(BLUE)),
                ],
            }
            .apply(&mut s)
            .unwrap();
        }
        s
    }

    fn artboard(id: u32, sprite: Sprite) -> Artboard {
        Artboard {
            id: ArtboardId(id),
            name: format!("a{id}"),
            position: (0, 0),
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
        }
    }

    /// Host de 8×8 con 4 frames y una capa de instancia (id 2) con `keys`.
    fn host(keys: Vec<InstanceKey>) -> Sprite {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        for i in 1..4 {
            Command::AddFrame {
                index: i,
                frame: Frame { duration_ms: 100 },
            }
            .apply(&mut s)
            .unwrap();
        }
        let mut layer = Layer::new_image(LayerId(2), "inst");
        layer.kind = LayerKind::Instance;
        layer.instance = Some(InstanceTrack { keys });
        s.layers.push(layer);
        s
    }

    fn doc(host: Sprite) -> Document {
        Document {
            artboards: vec![artboard(1, host), artboard(2, symbol())],
        }
    }

    fn px(pixels: &[Rgba], x: usize, y: usize) -> Rgba {
        pixels[y * 8 + x]
    }

    #[test]
    fn instance_renders_at_position_and_follows_timeline() {
        let d = doc(host(vec![InstanceKey::new(0, ArtboardId(2), (4, 5))]));
        let sprite = &d.artboards[0].sprite;
        let m = materialize(&d, sprite, 0..4);
        let f0 = render_frame(&m, 0);
        assert_eq!(px(&f0, 4, 5), RED);
        assert_eq!(px(&f0, 6, 6), BLUE);
        // Frame 1 del padre = frame 1 del símbolo; frame 2 = vuelve al 0 (loop).
        assert_eq!(px(&render_frame(&m, 1), 5, 6), BLUE);
        assert_eq!(px(&render_frame(&m, 2), 6, 6), BLUE);
        // El cel se recorta a lo opaco: empieza en la esquina del rojo.
        assert_eq!(m.cel(LayerId(2), 0).unwrap().position, (4, 5));
        assert_eq!(m.layer(LayerId(2)).unwrap().kind, LayerKind::Image);
        // Sin instancias, no copia.
        let plain = Sprite::new(2, 2, ColorMode::Rgba);
        assert!(matches!(materialize(&d, &plain, 0..1), Cow::Borrowed(_)));
    }

    #[test]
    fn flip_and_opacity_apply() {
        let mut k = InstanceKey::new(0, ArtboardId(2), (0, 0));
        k.flip_h = true;
        k.flip_v = true;
        k.opacity = 128;
        let mut h = host(vec![k]);
        h.layers[1].opacity = 128;
        let d = doc(h);
        let m = materialize(&d, &d.artboards[0].sprite, 0..1);
        let f0 = render_frame(&m, 0);
        // Rojo de (0, 0) → (2, 1); azul de (2, 1) → (0, 0). Opacidad 128 × 128 ≈ 64.
        let red = px(&f0, 2, 1);
        assert_eq!((red.r, red.g, red.b), (255, 0, 0));
        assert!((60..=68).contains(&red.a), "alpha {}", red.a);
        assert_eq!(px(&f0, 0, 0).b, 255);
    }

    #[test]
    fn nested_instances_render_and_missing_sources_do_not() {
        // 1 contiene a 3, que contiene a 2.
        let middle = host(vec![InstanceKey::new(0, ArtboardId(2), (1, 1))]);
        let top = host(vec![InstanceKey::new(0, ArtboardId(3), (2, 0))]);
        let mut d = doc(top);
        d.artboards.push(artboard(3, middle));
        let m = materialize(&d, &d.artboards[0].sprite, 0..1);
        assert_eq!(px(&render_frame(&m, 0), 3, 1), RED);
        // Fuente inexistente: no dibuja ni falla.
        let d = doc(host(vec![InstanceKey::new(0, ArtboardId(9), (0, 0))]));
        let m = materialize(&d, &d.artboards[0].sprite, 0..1);
        assert!(render_frame(&m, 0).iter().all(|c| c.a == 0));
    }

    #[test]
    fn cycles_stop_at_the_depth_limit() {
        let mut d = doc(host(vec![InstanceKey::new(0, ArtboardId(1), (0, 0))]));
        // Algo visible en el propio host para que el ciclo tenga píxeles.
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(RED))],
        }
        .apply(&mut d.artboards[0].sprite)
        .unwrap();
        let m = materialize(&d, &d.artboards[0].sprite, 0..1);
        assert_eq!(px(&render_frame(&m, 0), 0, 0), RED);
    }

    #[test]
    fn branching_cycles_stop_at_the_first_repeat() {
        // Dos capas de instancia del artboard 1 que lo muestran a él mismo: con solo el límite
        // de profundidad serían 2^16 renders.
        let mut h = host(vec![InstanceKey::new(0, ArtboardId(1), (0, 0))]);
        let mut second = h.layers[1].clone();
        second.id = LayerId(3);
        h.layers.push(second);
        let mut d = doc(h);
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Rgba(RED))],
        }
        .apply(&mut d.artboards[0].sprite)
        .unwrap();
        let m = materialize(&d, &d.artboards[0].sprite, 0..4);
        assert_eq!(px(&render_frame(&m, 0), 1, 1), RED);
        assert!(m.cels.keys().all(|(l, _)| *l == LayerId(1)));
    }

    #[test]
    fn hidden_instance_layers_are_not_materialized() {
        let mut h = host(vec![InstanceKey::new(0, ArtboardId(2), (0, 0))]);
        h.layers[1].visible = false;
        let d = doc(h);
        let m = materialize(&d, &d.artboards[0].sprite, 0..4);
        let layer = m.layer(LayerId(2)).unwrap();
        assert_eq!(layer.kind, LayerKind::Image);
        assert!(m.cels.is_empty());
        assert!(render_frame(&m, 0).iter().all(|c| c.a == 0));
        // Al hornear (guardar/exportar) se conservan, como en cualquier capa oculta.
        let baked = bake_layers(&d, &d.artboards[0].sprite, Bake::Everything).unwrap();
        assert!((0..4).all(|f| baked.cel(LayerId(2), f).is_some()));
        assert!(!baked.layer(LayerId(2)).unwrap().visible);
        assert!(render_frame(&baked, 0).iter().all(|c| c.a == 0));
    }

    #[test]
    fn instances_respect_layer_masks() {
        let mut h = host(vec![InstanceKey::new(0, ArtboardId(2), (0, 0))]);
        h.layers[1].mask = Some(LayerMask::filled(8, 8, 0));
        let d = doc(h);
        let m = materialize(&d, &d.artboards[0].sprite, 0..1);
        assert_eq!(px(&render_frame(&m, 0), 0, 0).a, 0);
    }

    #[test]
    fn bake_covers_every_frame() {
        let d = doc(host(vec![InstanceKey::new(1, ArtboardId(2), (0, 0))]));
        let baked = bake_layers(&d, &d.artboards[0].sprite, Bake::Everything).unwrap();
        assert!(
            baked.cel(LayerId(2), 0).is_none(),
            "nothing before the first key"
        );
        assert!((1..4).all(|f| baked.cel(LayerId(2), f).is_some()));
        assert!(bake_layers(&d, &d.artboards[1].sprite, Bake::Everything).is_none());
    }

    // --- Tilemaps ---

    /// Color distinto en cada píxel de un tile de 3×3 (asimétrico en todas las direcciones).
    fn asymmetric(x: u32, y: u32) -> Rgba {
        Rgba::rgb(40 + x as u8 * 70, 40 + y as u8 * 70, 200)
    }

    fn rgba_tileset() -> Tileset {
        let mut ts = Tileset::new("t", (3, 3), ColorMode::Rgba);
        let mut tile = Image::new(3, 3, ColorMode::Rgba);
        for y in 0..3 {
            for x in 0..3 {
                tile.set(x, y, PixelValue::Rgba(asymmetric(x, y))).unwrap();
            }
        }
        ts.tiles.push(Arc::new(tile));
        ts
    }

    /// Sprite cuya capa 1 es tilemap con `tileset` (índice 0) y `grid` en el frame 0, con 2 px de
    /// margen a la derecha y abajo.
    fn tiled(mode: ColorMode, tileset: Tileset, grid: TileGrid, position: (i32, i32)) -> Sprite {
        let (tw, th) = tileset.tile_size;
        let mut s = Sprite::new(grid.width * tw + 2, grid.height * th + 2, mode);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        s.tilesets.push(tileset);
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position,
                opacity: 255,
                image: ImageId(1),
            },
        );
        s
    }

    #[test]
    fn tilemaps_draw_every_flip_combination() {
        // 8 celdas (4×2): la celda i tiene flip X si i&1, Y si i&2 y D si i&4.
        let mut grid = TileGrid::new(4, 2);
        for i in 0..8u32 {
            grid.set(
                i % 4,
                i / 4,
                Tile::new(1, i & 1 != 0, i & 2 != 0, i & 4 != 0),
            );
        }
        let s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        let m = materialize_tilemaps(&s, 0..1);
        assert_eq!(m.layer(LayerId(1)).unwrap().kind, LayerKind::Image);
        let pixels = render_frame(&m, 0);
        let w = s.width as usize;
        for i in 0..8u32 {
            // Referencia independiente, como operaciones de imagen: transponer (D), espejar en X y
            // después en Y.
            let mut t: [[Rgba; 3]; 3] =
                std::array::from_fn(|y| std::array::from_fn(|x| asymmetric(x as u32, y as u32)));
            if i & 4 != 0 {
                let src = t;
                t = std::array::from_fn(|y| std::array::from_fn(|x| src[x][y]));
            }
            if i & 1 != 0 {
                for row in &mut t {
                    row.reverse();
                }
            }
            if i & 2 != 0 {
                t.reverse();
            }
            let (cx, cy) = ((i % 4) as usize * 3, (i / 4) as usize * 3);
            for (y, row) in t.iter().enumerate() {
                for (x, expected) in row.iter().enumerate() {
                    assert_eq!(
                        pixels[(cy + y) * w + cx + x],
                        *expected,
                        "cell {i} pixel ({x}, {y})"
                    );
                }
            }
        }
    }

    #[test]
    fn indexed_tilemaps_copy_palette_indices() {
        let mode = ColorMode::Indexed {
            transparent_index: 0,
        };
        let mut ts = Tileset::new("t", (2, 2), mode);
        let mut tile = Image::new(2, 2, mode);
        tile.set(0, 0, PixelValue::Index(5)).unwrap();
        ts.tiles.push(Arc::new(tile));
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, true, false, false));
        let s = tiled(mode, ts, grid, (0, 0));
        let m = materialize_tilemaps(&s, 0..1);
        // Celda (1, 0) espejada en X: el índice 5 pasa de (0, 0) a (1, 0) del tile, (3, 0) del lienzo.
        assert_eq!(m.pixel(LayerId(1), 0, 3, 0), PixelValue::Index(5));
        assert_eq!(m.pixel(LayerId(1), 0, 2, 0), PixelValue::Index(0));
        assert_eq!(render_frame(&m, 0)[3], s.palette.get(5).unwrap());
    }

    #[test]
    fn tilemap_cels_keep_position_and_opacity() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (1, 2));
        s.cels.get_mut(&(LayerId(1), 0)).unwrap().opacity = 128;
        s.layers[0].opacity = 128;
        let m = materialize_tilemaps(&s, 0..1);
        let cel = m.cel(LayerId(1), 0).unwrap();
        assert_eq!((cel.position, cel.opacity), ((1, 2), 128));
        let px = render_frame(&m, 0)[2 * s.width as usize + 1];
        assert_eq!((px.r, px.g, px.b), (40, 40, 200));
        assert!((60..=68).contains(&px.a), "alpha {}", px.a);
    }

    #[test]
    fn missing_tilesets_and_tiles_draw_nothing() {
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(9, false, false, false));
        grid.set(1, 0, Tile::new(1, false, false, false));
        let s = tiled(ColorMode::Rgba, rgba_tileset(), grid.clone(), (0, 0));
        let pixels = render_frame(&materialize_tilemaps(&s, 0..1), 0);
        assert_eq!(pixels[0].a, 0, "tile 9 does not exist");
        assert_eq!(pixels[3], asymmetric(0, 0), "tile 1 does");
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        s.layers[0].tileset = Some(7);
        let m = materialize_tilemaps(&s, 0..1);
        assert!(m.cels.is_empty());
        assert!(render_frame(&m, 0).iter().all(|c| c.a == 0));
    }

    #[test]
    fn diagonal_flip_needs_square_tiles() {
        let mut ts = Tileset::new("t", (2, 1), ColorMode::Rgba);
        let mut tile = Image::new(2, 1, ColorMode::Rgba);
        tile.set(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))
            .unwrap();
        ts.tiles.push(Arc::new(tile));
        let mut grid = TileGrid::new(2, 1);
        grid.set(0, 0, Tile::new(1, false, false, true));
        grid.set(1, 0, Tile::new(1, false, false, false));
        let s = tiled(ColorMode::Rgba, ts, grid, (0, 0));
        let pixels = render_frame(&materialize_tilemaps(&s, 0..1), 0);
        assert_eq!(pixels[0].a, 0, "D flip on a 2×1 tile is not drawn");
        assert_eq!(pixels[2], Rgba::rgb(1, 2, 3));
    }

    #[test]
    fn hidden_tilemaps_are_not_drawn_but_are_baked() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        s.layers[0].visible = false;
        let d = Document::with_sprite("t", s);
        let sprite = &d.artboards[0].sprite;
        assert!(materialize(&d, sprite, 0..1).cels.is_empty());
        let baked = bake_layers(&d, sprite, Bake::Everything).unwrap();
        assert!(baked.cel(LayerId(1), 0).is_some());
        assert!(baked.tilesets.is_empty() && baked.tilemaps.is_empty());
        assert!(
            bake_layers(&d, sprite, Bake::KeepTilemaps).is_none(),
            "no instances to bake"
        );
    }

    #[test]
    fn linked_tilemap_cels_stay_linked_and_instances_show_tilemaps() {
        let mut grid = TileGrid::new(1, 1);
        grid.set(0, 0, Tile::new(1, false, false, false));
        let mut s = tiled(ColorMode::Rgba, rgba_tileset(), grid, (0, 0));
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        let cel = *s.cel(LayerId(1), 0).unwrap();
        s.cels.insert((LayerId(1), 1), cel);
        let m = materialize_tilemaps(&s, 0..2);
        assert_eq!(
            m.cel(LayerId(1), 0).unwrap().image,
            m.cel(LayerId(1), 1).unwrap().image
        );
        // Un artboard con tilemaps visto como instancia.
        let mut d = doc(host(vec![InstanceKey::new(0, ArtboardId(3), (2, 2))]));
        d.artboards.push(artboard(3, s));
        let px = render_frame(&materialize(&d, &d.artboards[0].sprite, 0..1), 0);
        assert_eq!(px[2 * 8 + 2], asymmetric(0, 0));
    }
}
