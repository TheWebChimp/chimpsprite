//! Hornear un modelo 3D: renderizarlo en varios ángulos (y con los frames de su textura) a un
//! sprite normal, como frames con tags o como una hoja de sprites.

use std::sync::Arc;

use cs_3d::{RenderInput, Texture, render_model};
use cs_core::{
    AnimDirection, ArtboardId, CameraPreset, Cel, ColorMode, Document, Frame, Image, ImageId,
    PixelBuffer, Rgba, Sprite, Tag,
};

use crate::model3d::{texture_frame_of, texture_of};

/// Cómo sale el horneado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BakeOutput {
    /// Un frame por ángulo y frame de textura (ángulo mayor), con un tag por ángulo si la
    /// textura está animada.
    #[default]
    Frames,
    /// Un solo frame: fila = ángulo, columna = frame de textura.
    Spritesheet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakeOptions {
    /// Cuántos ángulos alrededor del eje Y (1, 4, 8, 16 o 32).
    pub angles: u32,
    /// `None`: la cámara del modelo.
    pub camera: Option<CameraPreset>,
    /// Usar todos los frames de la textura (si no, solo el 0).
    pub animate_texture: bool,
    pub output: BakeOutput,
}

/// Color de los tags de los ángulos.
const TAG_COLOR: Rgba = Rgba::rgb(0x5b, 0x6e, 0xe1);

/// El sprite horneado del modelo `model` (RGBA, una capa, con la paleta del modelo) y sus tags
/// (el sprite sale sin ellos). El ángulo `a` gira todos los objetos `a·360/angles` grados en Y.
/// Con `animate_texture`, un render por frame de la textura (cada uno dura lo que su frame); si
/// no, solo con el frame 0. `None` si no es un modelo (o está roto).
pub fn bake_model(
    doc: &Document,
    model: ArtboardId,
    options: &BakeOptions,
) -> Option<(Sprite, Vec<Tag>)> {
    let artboard = doc.artboard(model)?;
    let m = artboard.model.as_ref()?;
    let source = &artboard.sprite;
    if !source.color_mode.stores_rgba() || m.check(source).is_err() {
        return None;
    }
    let angles = options.angles.max(1);
    let (w, h) = (source.width, source.height);
    // Los frames de la textura que se usan, con su duración.
    let texture = texture_of(doc, m, &[artboard.id]);
    let durations: Vec<u16> = match texture {
        Some(t) if options.animate_texture => {
            t.sprite.frames.iter().map(|f| f.duration_ms).collect()
        }
        _ => vec![source.frames.first().map_or(100, |f| f.duration_ms)],
    };
    let camera = options
        .camera
        .map(|p| p.camera(m.camera.distance, m.camera.target));
    // Los renders, en orden ángulo-mayor.
    let mut stack = vec![artboard.id];
    let textures: Vec<_> = (0..durations.len())
        .map(|f| texture_frame_of(doc, m, f, &mut stack))
        .collect();
    let renders: Vec<Vec<Rgba>> = (0..angles)
        .flat_map(|a| textures.iter().map(move |t| (a, t)))
        .map(|(a, t)| {
            render_model(&RenderInput {
                model: m,
                size: (w, h),
                texture: t.as_ref().map(|(width, height, pixels)| Texture {
                    width: *width,
                    height: *height,
                    pixels,
                }),
                palette: source.palette.colors(),
                camera,
                extra_yaw: (a * 3600 / angles) as i32,
            })
        })
        .collect();
    let columns = durations.len();
    let mut tags = Vec::new();
    let sprite = match options.output {
        BakeOutput::Frames => {
            if columns > 1 {
                tags = (0..angles as usize)
                    .map(|a| Tag {
                        name: format!("{}°", a * 360 / angles as usize),
                        from: a * columns,
                        to: a * columns + columns - 1,
                        direction: AnimDirection::Forward,
                        color: TAG_COLOR,
                        repeat: 0,
                    })
                    .collect();
            }
            let frames = durations
                .iter()
                .cycle()
                .take(renders.len())
                .map(|&duration_ms| Frame { duration_ms })
                .collect();
            sprite_with(source, (w, h), frames, renders)?
        }
        BakeOutput::Spritesheet => {
            let sw = w * columns as u32;
            let mut sheet = vec![Rgba::TRANSPARENT; (sw * h * angles) as usize];
            for (i, render) in renders.iter().enumerate() {
                let (row, col) = ((i / columns) as u32, (i % columns) as u32);
                for y in 0..h {
                    let from = (y * w) as usize;
                    let to = ((row * h + y) * sw + col * w) as usize;
                    sheet[to..to + w as usize].copy_from_slice(&render[from..from + w as usize]);
                }
            }
            let frames = vec![Frame {
                duration_ms: durations[0],
            }];
            sprite_with(source, (sw, h * angles), frames, vec![sheet])?
        }
    };
    Some((sprite, tags))
}

/// Un sprite RGBA de `size` con la paleta de `source`, una capa y un frame por imagen.
fn sprite_with(
    source: &Sprite,
    size: (u32, u32),
    frames: Vec<Frame>,
    images: Vec<Vec<Rgba>>,
) -> Option<Sprite> {
    let mut sprite = Sprite::new(size.0, size.1, ColorMode::Rgba);
    sprite.palette = source.palette.clone();
    sprite.frames = frames;
    let layer = sprite.layers.first()?.id;
    for (frame, pixels) in images.into_iter().enumerate() {
        let image = Image::from_buffer(size.0, size.1, PixelBuffer::Rgba(pixels))?;
        let id = ImageId(frame as u32 + 1);
        sprite.images.insert(id, Arc::new(image));
        sprite.cels.insert(
            (layer, frame),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
    }
    Some(sprite)
}

#[cfg(test)]
mod tests {
    use cs_commands::model::new_model_artboard;

    use super::*;
    use crate::{materialize, render_frame};

    const SIZE: u32 = 32;

    /// Documento con un host de 40×40 y un modelo (cubo) de 32×32 con su textura.
    fn with_model() -> (Document, ArtboardId, ArtboardId) {
        let mut doc = Document::with_sprite("host", Sprite::new(40, 40, ColorMode::Rgba));
        let (model, texture, cmd) = new_model_artboard(&doc, "Cube", (SIZE, SIZE), (0, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, model, texture)
    }

    /// La textura con `n` frames (los nuevos, vacíos) de 40, 50, 60… ms.
    fn texture_frames(doc: &mut Document, texture: ArtboardId, n: usize) {
        let t = &mut doc.artboard_mut(texture).unwrap().sprite;
        t.frames = (0..n)
            .map(|i| Frame {
                duration_ms: 40 + 10 * i as u16,
            })
            .collect();
    }

    fn options(angles: u32, animate_texture: bool, output: BakeOutput) -> BakeOptions {
        BakeOptions {
            angles,
            camera: None,
            animate_texture,
            output,
        }
    }

    fn model_pixels(doc: &Document, model: ArtboardId) -> Vec<Rgba> {
        let sprite = &doc.artboard(model).unwrap().sprite;
        render_frame(&materialize(doc, sprite, 0..1), 0)
    }

    #[test]
    fn eight_angles_without_animation_give_eight_frames_and_no_tags() {
        let (mut doc, model, texture) = with_model();
        texture_frames(&mut doc, texture, 3);
        let (sprite, tags) =
            bake_model(&doc, model, &options(8, false, BakeOutput::Frames)).unwrap();
        assert_eq!(sprite.frames.len(), 8);
        assert_eq!((sprite.width, sprite.height), (SIZE, SIZE));
        assert!(sprite.color_mode.stores_rgba());
        assert!(tags.is_empty());
        // Los ángulos se ven distintos: el cubo girado 45° no es el de frente.
        assert_ne!(render_frame(&sprite, 0), render_frame(&sprite, 1));
    }

    #[test]
    fn an_animated_texture_gives_a_tag_per_angle() {
        let (mut doc, model, texture) = with_model();
        texture_frames(&mut doc, texture, 3);
        let (sprite, tags) =
            bake_model(&doc, model, &options(8, true, BakeOutput::Frames)).unwrap();
        assert_eq!(sprite.frames.len(), 24);
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(
            names,
            ["0°", "45°", "90°", "135°", "180°", "225°", "270°", "315°"]
        );
        assert_eq!((tags[1].from, tags[1].to), (3, 5));
        // Cada frame dura lo que su frame de textura.
        let durations: Vec<u16> = sprite.frames[3..6].iter().map(|f| f.duration_ms).collect();
        assert_eq!(durations, [40, 50, 60]);
    }

    #[test]
    fn a_spritesheet_has_a_row_per_angle_and_a_column_per_texture_frame() {
        let (mut doc, model, texture) = with_model();
        texture_frames(&mut doc, texture, 3);
        let (sheet, tags) =
            bake_model(&doc, model, &options(4, true, BakeOutput::Spritesheet)).unwrap();
        assert_eq!((sheet.width, sheet.height), (SIZE * 3, SIZE * 4));
        assert_eq!(sheet.frames.len(), 1);
        assert!(tags.is_empty());
        // La celda (0, 0) es el frame 0 del horneado en frames.
        let (frames, _) = bake_model(&doc, model, &options(4, true, BakeOutput::Frames)).unwrap();
        let first = render_frame(&frames, 0);
        let sheet = render_frame(&sheet, 0);
        for y in 0..SIZE {
            for x in 0..SIZE {
                assert_eq!(
                    sheet[(y * SIZE * 3 + x) as usize],
                    first[(y * SIZE + x) as usize]
                );
            }
        }
    }

    #[test]
    fn the_first_baked_frame_is_the_model_render() {
        let (doc, model, _) = with_model();
        let (sprite, _) = bake_model(&doc, model, &options(8, false, BakeOutput::Frames)).unwrap();
        assert_eq!(render_frame(&sprite, 0), model_pixels(&doc, model));
    }

    #[test]
    fn a_camera_preset_changes_the_image() {
        let (doc, model, _) = with_model();
        let plain = options(1, false, BakeOutput::Frames);
        let top = BakeOptions {
            camera: Some(CameraPreset::Top),
            ..plain
        };
        let (a, _) = bake_model(&doc, model, &plain).unwrap();
        let (b, _) = bake_model(&doc, model, &top).unwrap();
        assert_eq!(a.frames.len(), 1);
        assert_ne!(render_frame(&a, 0), render_frame(&b, 0));
    }

    #[test]
    fn only_models_are_baked() {
        let (doc, _, texture) = with_model();
        let o = options(8, false, BakeOutput::Frames);
        assert!(bake_model(&doc, texture, &o).is_none());
        assert!(bake_model(&doc, ArtboardId(999), &o).is_none());
    }
}
