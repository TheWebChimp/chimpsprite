//! Hornear un modelo 3D: renderizarlo en varios ángulos (y con los frames de su textura) a un
//! sprite normal, como frames con tags o como una hoja de sprites. Las opciones son las "Sheet
//! settings" de picoCAD: frames, tamaño, zoom, inclinación, altura y proyección.

use std::ops::RangeInclusive;
use std::sync::Arc;

use cs_3d::{RenderInput, Texture, render_model};
use cs_core::{
    AnimDirection, Artboard, ArtboardId, Camera3D, Cel, ColorMode, Document, FIXED, Frame, Image,
    ImageId, Model3D, PixelBuffer, Projection, Rgba, Sprite, Tag,
};

use crate::model3d::{texture_frame_of, texture_of};

/// Cómo sale el horneado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BakeOutput {
    /// Un frame por ángulo y frame de textura (ángulo mayor), con un tag por ángulo si la
    /// textura está animada.
    #[default]
    Frames,
    /// Un solo frame: fila = ángulo, columna = frame de textura.
    Spritesheet,
}

/// Los límites de las opciones (el diálogo usa los mismos).
pub const BAKE_FRAMES: RangeInclusive<u32> = 1..=64;
pub const BAKE_SIZE: RangeInclusive<u32> = 8..=512;
pub const BAKE_TILT: RangeInclusive<i32> = -900..=900;

/// El fov de la perspectiva si el modelo es ortográfico y se hornea en perspectiva (60°).
const DEFAULT_FOV: i32 = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BakeOptions {
    /// Cuántos ángulos alrededor del eje Y ([`BAKE_FRAMES`]).
    pub frames: u32,
    /// El lado más largo de cada frame en pixeles ([`BAKE_SIZE`]); el otro sigue la proporción
    /// del modelo.
    pub size: u32,
    /// En %: 100 es la distancia de la cámara del modelo, 200 la mitad.
    pub zoom: u32,
    /// La inclinación (pitch) de la cámara, en décimas de grado ([`BAKE_TILT`]).
    pub tilt: i32,
    /// En 1/16 de unidad, sumado a la altura del punto al que mira la cámara.
    pub height: i32,
    /// Ortográfica; si no, perspectiva (con el fov del modelo o 60°).
    pub ortho: bool,
    /// Usar todos los frames de la textura (si no, solo el 0).
    pub animate_texture: bool,
    pub output: BakeOutput,
}

impl BakeOptions {
    /// Las de un modelo `m` de `size` con una textura de `texture_frames` frames: 8 frames, el
    /// lado más largo del modelo, su cámara, en frames y con la textura animada si tiene varios frames.
    pub fn from_model(m: &Model3D, size: (u32, u32), texture_frames: usize) -> Self {
        BakeOptions {
            frames: 8,
            size: size
                .0
                .max(size.1)
                .clamp(*BAKE_SIZE.start(), *BAKE_SIZE.end()),
            zoom: 100,
            tilt: m.camera.pitch,
            height: 0,
            ortho: matches!(m.camera.projection, Projection::Orthographic),
            animate_texture: texture_frames > 1,
            output: BakeOutput::Frames,
        }
    }

    /// Los ángulos, dentro de [`BAKE_FRAMES`].
    fn angles(&self) -> u32 {
        self.frames.clamp(*BAKE_FRAMES.start(), *BAKE_FRAMES.end())
    }

    /// El tamaño de un frame para un modelo de `w`×`h`: `size` en el lado más largo y el otro en
    /// proporción (redondeado, al menos 1).
    pub fn frame_size(&self, (w, h): (u32, u32)) -> (u32, u32) {
        let long = u64::from(self.size.clamp(*BAKE_SIZE.start(), *BAKE_SIZE.end()));
        let (w, h) = (u64::from(w.max(1)), u64::from(h.max(1)));
        let short = |a: u64, b: u64| ((long * a + b / 2) / b).max(1) as u32;
        if w >= h {
            (long as u32, short(h, w))
        } else {
            (short(w, h), long as u32)
        }
    }

    /// La cámara `model` con la inclinación, el zoom, la altura y la proyección elegidos.
    fn camera(&self, model: &Camera3D) -> Camera3D {
        let distance = i64::from(model.distance) * 100 / i64::from(self.zoom.max(1));
        let projection = match (self.ortho, model.projection) {
            (true, _) => Projection::Orthographic,
            (false, p @ Projection::Perspective { .. }) => p,
            (false, Projection::Orthographic) => Projection::Perspective { fov: DEFAULT_FOV },
        };
        let mut target = model.target;
        target.y += self.height;
        Camera3D {
            projection,
            pitch: self.tilt.clamp(*BAKE_TILT.start(), *BAKE_TILT.end()),
            distance: distance.clamp(i64::from(FIXED), i64::from(i32::MAX)) as i32,
            target,
            ..*model
        }
    }
}

/// Color de los tags de los ángulos.
const TAG_COLOR: Rgba = Rgba::rgb(0x5b, 0x6e, 0xe1);

/// El artboard `model` y su modelo, si es un modelo que se puede hornear.
fn bakeable(doc: &Document, model: ArtboardId) -> Option<(&Artboard, &Model3D)> {
    let artboard = doc.artboard(model)?;
    let m = artboard.model.as_ref()?;
    let source = &artboard.sprite;
    if !source.color_mode.stores_rgba() || m.check(source).is_err() {
        return None;
    }
    Some((artboard, m))
}

/// Un render del horneado: el ángulo `angle` (de `options.frames`) con `texture`.
fn render_angle(
    m: &Model3D,
    source: &Sprite,
    options: &BakeOptions,
    texture: Option<&(u32, u32, Vec<Rgba>)>,
    angle: u32,
) -> Vec<Rgba> {
    let angles = options.angles();
    render_model(&RenderInput {
        model: m,
        size: options.frame_size((source.width, source.height)),
        texture: texture.map(|(width, height, pixels)| Texture {
            width: *width,
            height: *height,
            pixels,
        }),
        palette: source.palette.colors(),
        camera: Some(options.camera(&m.camera)),
        extra_yaw: (angle % angles * 3600 / angles) as i32,
        frame: None,
    })
}

/// Un solo frame del horneado, para la vista previa: el ángulo `angle` (módulo los frames) con el
/// frame 0 de la textura. Ancho, alto y pixeles; `None` si no es un modelo (o está roto).
pub fn bake_preview(
    doc: &Document,
    model: ArtboardId,
    options: &BakeOptions,
    angle: u32,
) -> Option<(u32, u32, Vec<Rgba>)> {
    let (artboard, m) = bakeable(doc, model)?;
    let source = &artboard.sprite;
    let texture = texture_frame_of(doc, m, 0, &mut vec![artboard.id]);
    let (w, h) = options.frame_size((source.width, source.height));
    Some((
        w,
        h,
        render_angle(m, source, options, texture.as_ref(), angle),
    ))
}

/// El sprite horneado del modelo `model` (RGBA, una capa, con la paleta del modelo) y sus tags
/// (el sprite sale sin ellos), con frames de [`BakeOptions::frame_size`]. El ángulo `a` gira
/// todos los objetos `a·360/frames` grados en Y, visto con la cámara de las opciones. Con
/// `animate_texture`, un render por frame de la textura (cada uno dura lo que su frame); si
/// no, solo con el frame 0 (y lo que dura ese frame). `None` si no es un modelo (o está roto).
pub fn bake_model(
    doc: &Document,
    model: ArtboardId,
    options: &BakeOptions,
) -> Option<(Sprite, Vec<Tag>)> {
    let (artboard, m) = bakeable(doc, model)?;
    let source = &artboard.sprite;
    let angles = options.angles();
    let (w, h) = options.frame_size((source.width, source.height));
    // Los frames de la textura que se usan, con su duración.
    let texture = texture_of(doc, m, &[artboard.id]);
    let durations: Vec<u16> = match texture {
        Some(t) if options.animate_texture => {
            t.sprite.frames.iter().map(|f| f.duration_ms).collect()
        }
        // Sin animar: lo que dura el frame 0 de la textura (o, sin textura, el del modelo).
        _ => {
            let frames = texture.map_or(&source.frames, |t| &t.sprite.frames);
            vec![frames.first().map_or(100, |f| f.duration_ms)]
        }
    };
    // Los renders, en orden ángulo-mayor.
    let mut stack = vec![artboard.id];
    let textures: Vec<_> = (0..durations.len())
        .map(|f| texture_frame_of(doc, m, f, &mut stack))
        .collect();
    let renders: Vec<Vec<Rgba>> = (0..angles)
        .flat_map(|a| textures.iter().map(move |t| (a, t)))
        .map(|(a, t)| render_angle(m, source, options, t.as_ref(), a))
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

    /// Las opciones por defecto del modelo, con `frames` ángulos.
    fn options_for(
        doc: &Document,
        model: ArtboardId,
        frames: u32,
        animate_texture: bool,
        output: BakeOutput,
    ) -> BakeOptions {
        let ab = doc.artboard(model).unwrap();
        let m = ab.model.as_ref().unwrap();
        BakeOptions {
            frames,
            animate_texture,
            output,
            ..BakeOptions::from_model(m, (ab.sprite.width, ab.sprite.height), 1)
        }
    }

    fn options(frames: u32, animate_texture: bool, output: BakeOutput) -> BakeOptions {
        let (doc, model, _) = with_model();
        options_for(&doc, model, frames, animate_texture, output)
    }

    fn opaque(sprite: &Sprite) -> usize {
        render_frame(sprite, 0).iter().filter(|c| c.a > 0).count()
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
        // Sin animar, cada frame dura lo del frame 0 de la textura.
        assert!(sprite.frames.iter().all(|f| f.duration_ms == 40));
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
    fn the_top_preset_bakes_like_the_model_render() {
        // Grande, para que 1° de diferencia se note en los pixeles.
        let mut doc = Document::with_sprite("host", Sprite::new(40, 40, ColorMode::Rgba));
        let (model, _, cmd) = new_model_artboard(&doc, "Big", (256, 256), (0, 0));
        cmd.apply(&mut doc).unwrap();
        let m = doc.artboard_mut(model).unwrap().model.as_mut().unwrap();
        m.camera = cs_core::CameraPreset::Top.camera(m.camera.distance, m.camera.target);
        m.camera.yaw = 300;
        let o = options_for(&doc, model, 8, false, BakeOutput::Frames);
        assert_eq!(o.tilt, 900);
        let (sprite, _) = bake_model(&doc, model, &o).unwrap();
        assert_eq!(render_frame(&sprite, 0), model_pixels(&doc, model));
    }

    #[test]
    fn the_size_limits_the_longer_side() {
        let o = BakeOptions {
            size: 64,
            ..options(1, false, BakeOutput::Frames)
        };
        assert_eq!(o.frame_size((16, 256)), (4, 64));
        assert_eq!(o.frame_size((256, 16)), (64, 4));
        assert_eq!(o.frame_size((32, 32)), (64, 64));
        // El lado corto es al menos 1.
        assert_eq!(o.frame_size((1, 512)), (1, 64));
    }

    #[test]
    fn from_model_defaults_to_the_longer_side() {
        let (doc, model, _) = with_model();
        let m = doc.artboard(model).unwrap().model.as_ref().unwrap();
        assert_eq!(BakeOptions::from_model(m, (16, 100), 1).size, 100);
    }

    #[test]
    fn from_model_takes_the_model_camera_and_size() {
        let (doc, model, _) = with_model();
        let ab = doc.artboard(model).unwrap();
        let m = ab.model.as_ref().unwrap();
        let o = BakeOptions::from_model(m, (SIZE, SIZE), 3);
        assert_eq!(o.frames, 8);
        assert_eq!(o.size, SIZE);
        assert_eq!((o.zoom, o.height), (100, 0));
        assert_eq!(o.tilt, m.camera.pitch);
        assert_eq!(
            o.ortho,
            matches!(m.camera.projection, cs_core::Projection::Orthographic)
        );
        assert!(o.animate_texture);
        assert_eq!(o.output, BakeOutput::Frames);
        assert!(!BakeOptions::from_model(m, (SIZE, SIZE), 1).animate_texture);
    }

    #[test]
    fn from_model_clamps_the_size_to_the_stepper_range() {
        let (doc, model, _) = with_model();
        let m = doc.artboard(model).unwrap().model.as_ref().unwrap();
        assert_eq!(BakeOptions::from_model(m, (1024, 64), 1).size, 512);
        assert_eq!(BakeOptions::from_model(m, (2, 2), 1).size, 8);
    }

    #[test]
    fn twelve_frames_give_twelve_frames() {
        let (doc, model, _) = with_model();
        let o = options_for(&doc, model, 12, false, BakeOutput::Frames);
        let (sprite, _) = bake_model(&doc, model, &o).unwrap();
        assert_eq!(sprite.frames.len(), 12);
    }

    #[test]
    fn the_size_is_the_frame_width_keeping_the_model_aspect() {
        let mut doc = Document::with_sprite("host", Sprite::new(40, 40, ColorMode::Rgba));
        let (model, _, cmd) = new_model_artboard(&doc, "Big", (128, 128), (0, 0));
        cmd.apply(&mut doc).unwrap();
        let o = BakeOptions {
            size: 48,
            ..options_for(&doc, model, 4, false, BakeOutput::Frames)
        };
        let (sprite, _) = bake_model(&doc, model, &o).unwrap();
        assert_eq!((sprite.width, sprite.height), (48, 48));
        assert!(opaque(&sprite) > 0);
        // Un modelo de 64×32: 48 de ancho, 24 de alto.
        let (wide, _, cmd) = new_model_artboard(&doc, "Wide", (64, 32), (200, 0));
        cmd.apply(&mut doc).unwrap();
        let o = BakeOptions {
            size: 48,
            ..options_for(&doc, wide, 1, false, BakeOutput::Frames)
        };
        let (sprite, _) = bake_model(&doc, wide, &o).unwrap();
        assert_eq!((sprite.width, sprite.height), (48, 24));
    }

    #[test]
    fn zooming_in_makes_the_model_bigger() {
        let (doc, model, _) = with_model();
        let plain = options_for(&doc, model, 1, false, BakeOutput::Frames);
        let near = BakeOptions { zoom: 200, ..plain };
        let (a, _) = bake_model(&doc, model, &plain).unwrap();
        let (b, _) = bake_model(&doc, model, &near).unwrap();
        assert!(opaque(&b) > opaque(&a), "{} vs {}", opaque(&b), opaque(&a));
    }

    #[test]
    fn tilt_height_and_ortho_change_the_image() {
        let (doc, model, _) = with_model();
        let plain = options_for(&doc, model, 1, false, BakeOutput::Frames);
        let (base, _) = bake_model(&doc, model, &plain).unwrap();
        let base = render_frame(&base, 0);
        let variants = [
            BakeOptions {
                tilt: plain.tilt + 300,
                ..plain
            },
            BakeOptions {
                height: plain.height + 8,
                ..plain
            },
            BakeOptions {
                ortho: !plain.ortho,
                ..plain
            },
        ];
        for v in variants {
            let (s, _) = bake_model(&doc, model, &v).unwrap();
            assert_ne!(render_frame(&s, 0), base, "{v:?}");
        }
    }

    #[test]
    fn the_preview_is_the_baked_frame_of_its_angle() {
        let (mut doc, model, texture) = with_model();
        texture_frames(&mut doc, texture, 3);
        let o = options_for(&doc, model, 8, false, BakeOutput::Frames);
        let (sprite, _) = bake_model(&doc, model, &o).unwrap();
        for angle in [0, 3] {
            let (w, h, pixels) = bake_preview(&doc, model, &o, angle).unwrap();
            assert_eq!((w, h), (SIZE, SIZE));
            assert_eq!(pixels, render_frame(&sprite, angle as usize), "{angle}");
        }
        // Con la textura animada también es el frame 0 de la textura.
        let animated = BakeOptions {
            animate_texture: true,
            ..o
        };
        assert_eq!(
            bake_preview(&doc, model, &animated, 3),
            bake_preview(&doc, model, &o, 3)
        );
        assert!(bake_preview(&doc, texture, &o, 0).is_none());
    }

    #[test]
    fn only_models_are_baked() {
        let (doc, _, texture) = with_model();
        let o = options(8, false, BakeOutput::Frames);
        assert!(bake_model(&doc, texture, &o).is_none());
        assert!(bake_model(&doc, ArtboardId(999), &o).is_none());
    }
}
