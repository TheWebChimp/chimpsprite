//! Instancias: antes de componer, cada capa de instancia se convierte en una capa de imagen con
//! el artboard fuente ya renderizado en su frame.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use cs_core::{
    ArtboardId, Cel, ColorMode, Document, Image, ImageId, InstanceKey, InstanceTrack, LayerKind,
    PixelValue, Rgba, Sprite, source_frame,
};

use crate::render_frame;

/// Profundidad máxima de instancias anidadas (defensa contra ciclos en datos corruptos).
const MAX_DEPTH: u32 = 16;

/// Copia del sprite donde las capas de instancia son capas de imagen con un cel en cada frame de
/// `frames` en que se ve algo. Sin capas de instancia devuelve el mismo sprite. Es para mostrar:
/// las capas de instancia ocultas quedan sin cels.
pub fn materialize<'a>(
    doc: &Document,
    sprite: &'a Sprite,
    frames: Range<usize>,
) -> Cow<'a, Sprite> {
    materialize_root(doc, sprite, frames, true)
}

/// `materialize` de todos los frames, para formatos sin instancias. `None` si no hay. Hornea
/// también las capas de instancia ocultas, para que sus píxeles se guarden como los de cualquier
/// capa oculta.
pub fn bake_instances(doc: &Document, sprite: &Sprite) -> Option<Sprite> {
    match materialize_root(doc, sprite, 0..sprite.frames.len(), false) {
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
    let pixels = render_frame(&materialize_at(doc, source, sf..sf + 1, stack, true), sf);
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
        let baked = bake_instances(&d, &d.artboards[0].sprite).unwrap();
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
        let baked = bake_instances(&d, &d.artboards[0].sprite).unwrap();
        assert!(
            baked.cel(LayerId(2), 0).is_none(),
            "nothing before the first key"
        );
        assert!((1..4).all(|f| baked.cel(LayerId(2), f).is_some()));
        assert!(bake_instances(&d, &d.artboards[1].sprite).is_none());
    }
}
