//! Los artboards 3D en el documento: su sprite se compone con el render del modelo.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, LazyLock, Mutex};

use cs_3d::{RenderInput, Texture, render_model};
use cs_core::{Artboard, ArtboardId, Cel, Document, Image, ImageId, PixelBuffer, Rgba, Sprite};

/// Renders guardados como mucho (al llenarse se vacía).
const CACHE_LIMIT: usize = 64;

/// Lo que define un render: si nada de esto cambió, el render es el mismo.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    artboard: ArtboardId,
    /// Hash del `Model3D` y de la paleta (la rampa de la luz).
    model: u64,
    /// Hash del tamaño y los pixeles del frame de la textura (0 sin textura).
    texture: u64,
    size: (u32, u32),
}

static CACHE: LazyLock<Mutex<HashMap<Key, Arc<Vec<Rgba>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn hash_of(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// Copia del sprite de un artboard con `model` cuya capa tiene en el frame 0 el render del
/// modelo, con la textura en `texture_frame` (módulo sus frames). `None` si no es un modelo.
/// Sin textura, o si la textura es otro modelo, las caras son grises.
pub fn model_sprite(doc: &Document, artboard: &Artboard, texture_frame: usize) -> Option<Sprite> {
    let mut stack = vec![artboard.id];
    model_sprite_at(doc, artboard, texture_frame, &mut stack)
}

/// El render más chico, con el cuadro `size` en su centro, que llega al pixel `(x, y)` del cuadro
/// (que puede caer afuera): su tamaño y dónde queda ese pixel en él. `None` si queda demasiado
/// lejos.
fn around(size: (u32, u32), (x, y): (i32, i32)) -> Option<((u32, u32), (u32, u32))> {
    // Lo que falta de cada lado, igual en los dos para que el cuadro siga centrado.
    let side = |v: i32, n: u32| (-v).max(v - (n as i32 - 1)).max(0);
    let (ox, oy) = (side(x, size.0), side(y, size.1));
    if ox > 1 << 14 || oy > 1 << 14 {
        return None;
    }
    let (ox, oy) = (ox as u32, oy as u32);
    Some((
        (size.0 + 2 * ox, size.1 + 2 * oy),
        ((x + ox as i32) as u32, (y + oy as i32) as u32),
    ))
}

/// El objeto del modelo de `artboard` que se ve en su pixel `(x, y)` en el lienzo (con el frame
/// 0 de la textura: sus pixeles transparentes no tapan lo de atrás). El pixel puede caer fuera
/// del artboard: cuenta lo que se vería ahí si el render siguiera (como la vista Persp que llena
/// su cuadro). `None` si no hay ninguno o si no es un modelo.
pub fn model_pick(doc: &Document, artboard: &Artboard, x: i32, y: i32) -> Option<usize> {
    let model = artboard.model.as_ref()?;
    let sprite = &artboard.sprite;
    if !sprite.color_mode.stores_rgba() || model.check(sprite).is_err() {
        return None;
    }
    let frame = (sprite.width, sprite.height);
    let (size, at) = around(frame, (x, y))?;
    let mut stack = vec![artboard.id];
    let texture = texture_frame_of(doc, model, 0, &mut stack);
    cs_3d::pick_object(
        &RenderInput {
            model,
            size,
            texture: texture.as_ref().map(|(width, height, pixels)| Texture {
                width: *width,
                height: *height,
                pixels,
            }),
            palette: sprite.palette.colors(),
            camera: None,
            extra_yaw: 0,
            frame: Some(frame),
            time_ms: None,
        },
        at.0,
        at.1,
    )
}

/// El objeto de `model` que se ve en el pixel `(x, y)` de su render a `size` en el tiempo
/// `time_ms` de la animación (con la textura, frame 0, y la paleta de `artboard`), como
/// [`model_render_at`]. El pixel puede caer fuera de `size`, como en [`model_pick`]. Los
/// bloqueados no se eligen. `None` si no hay ninguno.
pub fn model_pick_at(
    doc: &Document,
    artboard: &Artboard,
    model: &cs_core::Model3D,
    size: (u32, u32),
    at: (i32, i32),
    time_ms: Option<u32>,
) -> Option<usize> {
    let (render, at) = around(size, at)?;
    let mut stack = vec![artboard.id];
    let texture = texture_frame_of(doc, model, 0, &mut stack);
    cs_3d::pick_object(
        &RenderInput {
            model,
            size: render,
            texture: texture.as_ref().map(|(width, height, pixels)| Texture {
                width: *width,
                height: *height,
                pixels,
            }),
            palette: artboard.sprite.palette.colors(),
            camera: None,
            extra_yaw: 0,
            frame: Some(size),
            time_ms,
        },
        at.0,
        at.1,
    )
}

/// La cara `(objeto, cara)` de `model` que se ve en el pixel `(x, y)` de su render a `size` con
/// la textura (frame 0) y la paleta de `artboard`, como [`model_render`]. El pixel puede caer
/// fuera de `size`, como en [`model_pick`]. `None` si no hay ninguna.
pub fn model_pick_face(
    doc: &Document,
    artboard: &Artboard,
    model: &cs_core::Model3D,
    size: (u32, u32),
    at: (i32, i32),
) -> Option<(usize, usize)> {
    let (render, at) = around(size, at)?;
    let mut stack = vec![artboard.id];
    let texture = texture_frame_of(doc, model, 0, &mut stack);
    cs_3d::pick_face(
        &RenderInput {
            model,
            size: render,
            texture: texture.as_ref().map(|(width, height, pixels)| Texture {
                width: *width,
                height: *height,
                pixels,
            }),
            palette: artboard.sprite.palette.colors(),
            camera: None,
            extra_yaw: 0,
            frame: Some(size),
            time_ms: None,
        },
        at.0,
        at.1,
    )
}

/// El contorno de 1 px de las caras `faces` en el render de `model` a `size` con el cuadro
/// `frame` (ver [`cs_3d::face_outline`] y [`RenderInput::frame`]), fila por fila.
pub fn model_face_outline(
    model: &cs_core::Model3D,
    size: (u32, u32),
    frame: Option<(u32, u32)>,
    faces: &[(usize, usize)],
) -> Vec<bool> {
    cs_3d::face_outline(
        &RenderInput {
            model,
            size,
            texture: None,
            palette: &[],
            camera: None,
            extra_yaw: 0,
            frame,
            time_ms: None,
        },
        faces,
    )
}

/// El render de `model` (p. ej. la vista previa del modo Edit, no el guardado en `artboard`)
/// con la textura (frame 0) y la paleta de `artboard`, a `size` con el cuadro `frame` (ver
/// [`RenderInput::frame`]; `None`: el cuadro es `size`). Sin caché.
pub fn model_render(
    doc: &Document,
    artboard: &Artboard,
    model: &cs_core::Model3D,
    size: (u32, u32),
    frame: Option<(u32, u32)>,
) -> Vec<Rgba> {
    model_render_at(doc, artboard, model, size, frame, None)
}

/// Como [`model_render`], en el tiempo `time_ms` de la animación (`None`: en reposo).
pub fn model_render_at(
    doc: &Document,
    artboard: &Artboard,
    model: &cs_core::Model3D,
    size: (u32, u32),
    frame: Option<(u32, u32)>,
    time_ms: Option<u32>,
) -> Vec<Rgba> {
    let mut stack = vec![artboard.id];
    let texture = texture_frame_of(doc, model, 0, &mut stack);
    render_model(&RenderInput {
        model,
        size,
        texture: texture.as_ref().map(|(width, height, pixels)| Texture {
            width: *width,
            height: *height,
            pixels,
        }),
        palette: artboard.sprite.palette.colors(),
        camera: None,
        extra_yaw: 0,
        frame,
        time_ms,
    })
}

/// El frame `texture_frame` (módulo sus frames) de la textura de `model`, como se ve en el
/// lienzo: con sus instancias y tilemaps resueltos. `None` sin textura, si la textura es otro
/// modelo o si está en `stack` (un ciclo).
pub(crate) fn texture_frame_of(
    doc: &Document,
    model: &cs_core::Model3D,
    texture_frame: usize,
    stack: &mut Vec<ArtboardId>,
) -> Option<(u32, u32, Vec<Rgba>)> {
    let t = texture_of(doc, model, stack)?;
    let frame = texture_frame % t.sprite.frames.len();
    stack.push(t.id);
    let pixels = crate::materialize::visible_frame(doc, &t.sprite, frame, stack);
    stack.pop();
    Some((t.sprite.width, t.sprite.height, pixels))
}

/// El artboard de la textura de `model`, si cuenta como textura: existe, no es otro modelo, no
/// está en `stack` (un ciclo) y tiene frames.
pub(crate) fn texture_of<'a>(
    doc: &'a Document,
    model: &cs_core::Model3D,
    stack: &[ArtboardId],
) -> Option<&'a Artboard> {
    model
        .texture
        .and_then(|id| doc.artboard(id))
        .filter(|t| t.model.is_none() && !stack.contains(&t.id) && !t.sprite.frames.is_empty())
}

/// Como `model_sprite`, con los artboards que ya se están materializando (`stack`): una textura
/// que está en la pila (un ciclo) cuenta como sin textura.
pub(crate) fn model_sprite_at(
    doc: &Document,
    artboard: &Artboard,
    texture_frame: usize,
    stack: &mut Vec<ArtboardId>,
) -> Option<Sprite> {
    let model = artboard.model.as_ref()?;
    let sprite = &artboard.sprite;
    let layer = sprite.layers.first()?.id;
    // Un modelo roto (caras con vértices que no existen, varias capas) no se pinta.
    if !sprite.color_mode.stores_rgba() || model.check(sprite).is_err() {
        return None;
    }
    let texture = texture_frame_of(doc, model, texture_frame, stack);
    let size = (sprite.width, sprite.height);
    let palette = sprite.palette.colors();
    let key = Key {
        artboard: artboard.id,
        model: hash_of(&(model, palette)),
        texture: texture.as_ref().map_or(0, hash_of),
        size,
    };
    let cached = CACHE.lock().ok().and_then(|c| c.get(&key).cloned());
    let pixels = match cached {
        Some(pixels) => pixels,
        None => {
            let pixels = Arc::new(render_model(&RenderInput {
                model,
                size,
                texture: texture.as_ref().map(|(width, height, pixels)| Texture {
                    width: *width,
                    height: *height,
                    pixels,
                }),
                palette,
                camera: None,
                extra_yaw: 0,
                frame: None,
                time_ms: None,
            }));
            if let Ok(mut cache) = CACHE.lock() {
                if cache.len() >= CACHE_LIMIT {
                    cache.clear();
                }
                cache.insert(key, pixels.clone());
            }
            pixels
        }
    };
    let image = Image::from_buffer(size.0, size.1, PixelBuffer::Rgba(pixels.to_vec()))?;
    // El sprite del modelo no aporta pixeles: su única capa muestra el render.
    let mut out = sprite.clone();
    out.cels.clear();
    out.images.clear();
    let id = ImageId(1);
    out.images.insert(id, Arc::new(image));
    out.cels.insert(
        (layer, 0),
        Cel {
            position: (0, 0),
            opacity: 255,
            image: id,
        },
    );
    Some(out)
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_commands::model::new_model_artboard;
    use cs_core::*;

    use super::*;
    use crate::{Bake, bake_layers, materialize, render_frame};

    const SIZE: u32 = 32;

    /// Documento con un host de 40×40 y un modelo (cubo) de 32×32 con su textura.
    fn with_model() -> (Document, ArtboardId, ArtboardId) {
        let mut doc = Document::with_sprite("host", Sprite::new(40, 40, ColorMode::Rgba));
        let (model, texture, cmd) = new_model_artboard(&doc, "Cube", (SIZE, SIZE), (0, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, model, texture)
    }

    fn center(pixels: &[Rgba], width: u32) -> Rgba {
        pixels[(width / 2 * width + width / 2) as usize]
    }

    fn model_pixels(doc: &Document, model: ArtboardId) -> Vec<Rgba> {
        let sprite = &doc.artboard(model).unwrap().sprite;
        render_frame(&materialize(doc, sprite, 0..1), 0)
    }

    #[test]
    fn a_model_artboard_composes_its_render() {
        let (doc, model, _) = with_model();
        let pixels = model_pixels(&doc, model);
        assert_eq!(center(&pixels, SIZE).a, 255, "the cube covers the center");
        assert_eq!(pixels[0].a, 0, "the background is transparent");
        let a = doc.artboard(model).unwrap();
        let s = model_sprite(&doc, a, 0).unwrap();
        assert_eq!(render_frame(&s, 0), pixels);
        assert!(model_sprite(&doc, &doc.artboards[0], 0).is_none());
    }

    #[test]
    fn an_instance_of_a_model_shows_the_render() {
        let (mut doc, model, _) = with_model();
        let host = &mut doc.artboards[0].sprite;
        let mut layer = Layer::new_image(LayerId(2), "inst");
        layer.kind = LayerKind::Objects;
        layer.objects = vec![SceneObject {
            id: ObjectId(1),
            name: "cube".into(),
            track: InstanceTrack {
                keys: vec![InstanceKey::new(0, model, (4, 4))],
            },
        }];
        host.layers.push(layer);
        let host = &doc.artboards[0].sprite;
        let pixels = render_frame(&materialize(&doc, host, 0..1), 0);
        let expected = model_pixels(&doc, model);
        let at = |x: u32, y: u32| pixels[(y * 40 + x) as usize];
        assert_eq!(at(4 + SIZE / 2, 4 + SIZE / 2), center(&expected, SIZE));
        assert_eq!(at(4 + SIZE / 2, 4 + SIZE / 2).a, 255);
    }

    #[test]
    fn baking_a_model_gives_its_pixels() {
        let (doc, model, _) = with_model();
        let sprite = &doc.artboard(model).unwrap().sprite;
        let baked = bake_layers(&doc, sprite, Bake::Everything).expect("a model is baked");
        assert_eq!(render_frame(&baked, 0), model_pixels(&doc, model));
        assert!(bake_layers(&doc, sprite, Bake::KeepTilemaps).is_some());
    }

    #[test]
    fn painting_the_texture_changes_the_render() {
        let (mut doc, model, texture) = with_model();
        let before = model_pixels(&doc, model);
        let t = &mut doc.artboard_mut(texture).unwrap().sprite;
        let layer = t.layers[0].id;
        let (w, h) = (t.width as i32, t.height as i32);
        let pixels = (0..w)
            .flat_map(|x| (0..h).map(move |y| (x, y)))
            .map(|(x, y)| (x, y, PixelValue::Rgba(Rgba::rgb(200, 30, 30))))
            .collect();
        Command::DrawPixels {
            layer,
            frame: 0,
            pixels,
        }
        .apply(t)
        .unwrap();
        let after = model_pixels(&doc, model);
        assert_ne!(before, after);
        assert_eq!(center(&after, SIZE).a, 255);
    }

    #[test]
    fn a_model_used_as_texture_counts_as_no_texture() {
        let (mut doc, model, _) = with_model();
        let none = {
            let a = doc.artboard_mut(model).unwrap();
            a.model.as_mut().unwrap().texture = None;
            model_pixels(&doc, model)
        };
        // Otro modelo como textura (y el propio): gris, como sin textura.
        let (other, _, cmd) = new_model_artboard(&doc, "Other", (8, 8), (100, 0));
        cmd.apply(&mut doc).unwrap();
        for texture in [other, model] {
            doc.artboard_mut(model)
                .unwrap()
                .model
                .as_mut()
                .unwrap()
                .texture = Some(texture);
            assert_eq!(model_pixels(&doc, model), none);
        }
    }

    #[test]
    fn model_render_matches_the_canvas_render_at_the_same_size() {
        let (doc, model, _) = with_model();
        let artboard = doc.artboard(model).unwrap();
        let m = artboard.model.as_ref().unwrap();
        assert_eq!(
            model_render(&doc, artboard, m, (SIZE, SIZE), None),
            model_pixels(&doc, model)
        );
        let half = model_render(&doc, artboard, m, (SIZE / 2, SIZE / 2), None);
        assert_eq!(half.len(), (SIZE * SIZE / 4) as usize);
        assert_eq!(center(&half, SIZE / 2).a, 255);
    }

    #[test]
    fn the_render_and_the_pick_at_a_time_use_the_animation() {
        let (doc, model, _) = with_model();
        let artboard = doc.artboard(model).unwrap();
        let mut m = artboard.model.clone().unwrap();
        // El cubo se va 4 unidades a la derecha en 1 s: al final ya no está en el centro.
        m.objects[0].anim.push(Clip::new(
            ClipMotion::Move {
                offset: Vec3i {
                    x: 4 * FIXED,
                    y: 0,
                    z: 0,
                },
            },
            0,
            1000,
        ));
        let size = (SIZE, SIZE);
        let rest = model_render(&doc, artboard, &m, size, None);
        assert_eq!(
            model_render_at(&doc, artboard, &m, size, None, Some(0)),
            rest
        );
        let moved = model_render_at(&doc, artboard, &m, size, None, Some(1000));
        assert_ne!(moved, rest);
        let c = (SIZE as i32 / 2, SIZE as i32 / 2);
        assert_eq!(model_pick_at(&doc, artboard, &m, size, c, Some(0)), Some(0));
        assert_eq!(model_pick_at(&doc, artboard, &m, size, c, Some(1000)), None);
        // Bloqueado no se elige.
        m.objects[0].locked = true;
        assert_eq!(model_pick_at(&doc, artboard, &m, size, c, Some(0)), None);
    }
}
