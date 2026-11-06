//! El modelo 3D girando (turntable) para exportarlo a GIF o a una tira PNG: las "GIF settings" de
//! picoCAD 2. Usa la cámara de las opciones de horneado ([`BakeOptions`]) y le suma el contorno,
//! un preset de render (resolución y paleta de una consola), el fondo, la escala entera y las
//! scanlines.

use std::ops::RangeInclusive;

use cs_3d::{RenderInput, Texture, render_model};
use cs_core::{ArtboardId, Document, Rgba};

use crate::bake3d::{BakeOptions, bakeable};
use crate::model3d::texture_frame_of;

/// Los límites de las opciones (el diálogo usa los mismos).
pub const TURNTABLE_SCALE: RangeInclusive<u32> = 1..=8;
pub const TURNTABLE_FRAME_MS: RangeInclusive<u32> = 10..=1000;
/// La intensidad de las scanlines, en % de oscuridad.
pub const SCANLINES: RangeInclusive<u8> = 10..=100;

/// Los 4 verdes de la Game Boy (DMG), de oscuro a claro.
const GAMEBOY: [Rgba; 4] = [
    Rgba::rgb(0x0f, 0x38, 0x0f),
    Rgba::rgb(0x30, 0x62, 0x30),
    Rgba::rgb(0x8b, 0xac, 0x0f),
    Rgba::rgb(0x9b, 0xbc, 0x0f),
];

/// Los 16 colores de PICO-8.
const PICO8: [Rgba; 16] = [
    Rgba::rgb(0x00, 0x00, 0x00),
    Rgba::rgb(0x1d, 0x2b, 0x53),
    Rgba::rgb(0x7e, 0x25, 0x53),
    Rgba::rgb(0x00, 0x87, 0x51),
    Rgba::rgb(0xab, 0x52, 0x36),
    Rgba::rgb(0x5f, 0x57, 0x4f),
    Rgba::rgb(0xc2, 0xc3, 0xc7),
    Rgba::rgb(0xff, 0xf1, 0xe8),
    Rgba::rgb(0xff, 0x00, 0x4d),
    Rgba::rgb(0xff, 0xa3, 0x00),
    Rgba::rgb(0xff, 0xec, 0x27),
    Rgba::rgb(0x00, 0xe4, 0x36),
    Rgba::rgb(0x29, 0xad, 0xff),
    Rgba::rgb(0x83, 0x76, 0x9c),
    Rgba::rgb(0xff, 0x77, 0xa8),
    Rgba::rgb(0xff, 0xcc, 0xaa),
];

/// La resolución (y la paleta) del render.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum RenderPreset {
    /// El tamaño de `size` de las opciones, con la proporción del modelo, y sus colores.
    #[default]
    Native,
    /// La pantalla de la Nintendo DS: 256×192.
    NintendoDs,
    /// La pantalla de la Game Boy: 160×144 con sus 4 verdes.
    GameBoy,
    /// La de PICO-8: 128×128 con sus 16 colores.
    Pico8,
}

impl RenderPreset {
    pub const ALL: [RenderPreset; 4] = [
        RenderPreset::Native,
        RenderPreset::NintendoDs,
        RenderPreset::GameBoy,
        RenderPreset::Pico8,
    ];

    /// Como lo escribe picoCAD 2 (en minúsculas).
    pub fn label(self) -> &'static str {
        match self {
            RenderPreset::Native => "native",
            RenderPreset::NintendoDs => "nintendo_ds",
            RenderPreset::GameBoy => "gameboy",
            RenderPreset::Pico8 => "pico-8",
        }
    }

    /// El tamaño de la pantalla; `None` en native.
    pub fn screen(self) -> Option<(u32, u32)> {
        match self {
            RenderPreset::Native => None,
            RenderPreset::NintendoDs => Some((256, 192)),
            RenderPreset::GameBoy => Some((160, 144)),
            RenderPreset::Pico8 => Some((128, 128)),
        }
    }

    /// La paleta a la que se reduce; `None` deja los colores.
    pub fn palette(self) -> Option<&'static [Rgba]> {
        match self {
            RenderPreset::GameBoy => Some(&GAMEBOY),
            RenderPreset::Pico8 => Some(&PICO8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TurntableOptions {
    /// La cámara y los ángulos (`output` y `animate_texture` no cuentan: un frame por ángulo,
    /// con el frame 0 de la textura).
    pub bake: BakeOptions,
    /// El contorno; `None`, sin contorno (aunque el modelo tenga uno).
    pub outline: Option<Rgba>,
    /// Las scanlines, con su intensidad ([`SCANLINES`]); `None`, sin.
    pub scanlines: Option<u8>,
    pub preset: RenderPreset,
    /// Escala entera ([`TURNTABLE_SCALE`]), vecino más cercano.
    pub scale: u32,
    /// Lo que dura cada frame ([`TURNTABLE_FRAME_MS`]).
    pub frame_ms: u32,
    /// El fondo; `None`, transparente.
    pub background: Option<Rgba>,
    /// En bucle infinito; si no, se reproduce una vez.
    pub looping: bool,
}

impl TurntableOptions {
    /// Las de un modelo `m` de `size`: las de [`BakeOptions::from_model`], su contorno, native,
    /// escala 1, 100 ms, fondo transparente y en bucle.
    pub fn from_model(m: &cs_core::Model3D, size: (u32, u32)) -> Self {
        TurntableOptions {
            bake: BakeOptions {
                animate_texture: false,
                ..BakeOptions::from_model(m, size, 1)
            },
            outline: m.outline,
            scanlines: None,
            preset: RenderPreset::Native,
            scale: 1,
            frame_ms: 100,
            background: None,
            looping: true,
        }
    }

    /// La escala, dentro de [`TURNTABLE_SCALE`].
    pub fn scale(&self) -> u32 {
        self.scale
            .clamp(*TURNTABLE_SCALE.start(), *TURNTABLE_SCALE.end())
    }

    /// Lo que dura un frame, dentro de [`TURNTABLE_FRAME_MS`].
    pub fn frame_ms(&self) -> u32 {
        self.frame_ms
            .clamp(*TURNTABLE_FRAME_MS.start(), *TURNTABLE_FRAME_MS.end())
    }

    /// El render (a escala 1) de un modelo de `model_size` y el cuadro de la proyección: en
    /// native, el de [`BakeOptions::frame_size`]; con una pantalla, la pantalla y el cuadro de la
    /// proporción del modelo más grande que entra en ella.
    fn render_size(&self, model_size: (u32, u32)) -> ((u32, u32), Option<(u32, u32)>) {
        match self.preset.screen() {
            None => (self.bake.frame_size(model_size), None),
            Some((sw, sh)) => {
                let (mw, mh) = (
                    u64::from(model_size.0.max(1)),
                    u64::from(model_size.1.max(1)),
                );
                let (sw64, sh64) = (u64::from(sw), u64::from(sh));
                // El lado que limita: el ancho si mw/mh > sw/sh.
                let frame = if mw * sh64 >= mh * sw64 {
                    (sw, ((sw64 * mh + mw / 2) / mw).clamp(1, sh64) as u32)
                } else {
                    (((sh64 * mw + mh / 2) / mh).clamp(1, sw64) as u32, sh)
                };
                ((sw, sh), Some(frame))
            }
        }
    }

    /// El tamaño de cada frame exportado (con la escala) de un modelo de `model_size`.
    pub fn output_size(&self, model_size: (u32, u32)) -> (u32, u32) {
        let ((w, h), _) = self.render_size(model_size);
        (w * self.scale(), h * self.scale())
    }
}

/// Los frames del turntable: uno por ángulo, todos de `width`×`height`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turntable {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Vec<Rgba>>,
}

/// El frame del ángulo `angle` (módulo los ángulos) a la escala `scale` (para la vista previa,
/// que puede usar una menor que la de las opciones): ancho, alto y pixeles. `None` si no es un
/// modelo (o está roto).
pub fn turntable_frame(
    doc: &Document,
    model: ArtboardId,
    options: &TurntableOptions,
    angle: u32,
    scale: u32,
) -> Option<(u32, u32, Vec<Rgba>)> {
    let (artboard, m) = bakeable(doc, model)?;
    let source = &artboard.sprite;
    let texture = texture_frame_of(doc, m, 0, &mut vec![artboard.id]);
    let model = cs_core::Model3D {
        outline: options.outline,
        ..m.clone()
    };
    let (size, frame) = options.render_size((source.width, source.height));
    let angles = options.bake.angles();
    let mut pixels = render_model(&RenderInput {
        model: &model,
        size,
        texture: texture.as_ref().map(|(width, height, pixels)| Texture {
            width: *width,
            height: *height,
            pixels,
        }),
        palette: source.palette.colors(),
        camera: Some(options.bake.camera(&m.camera)),
        extra_yaw: (angle % angles * 3600 / angles) as i32,
        frame,
    });
    if let Some(bg) = options.background {
        for p in &mut pixels {
            if p.a < 128 {
                *p = bg;
            }
        }
    }
    if let Some(palette) = options.preset.palette() {
        let gameboy = options.preset == RenderPreset::GameBoy;
        for p in pixels.iter_mut().filter(|p| p.a >= 128) {
            *p = if gameboy {
                by_luminance(*p, palette)
            } else {
                nearest(*p, palette)
            };
        }
    }
    let scale = scale.clamp(*TURNTABLE_SCALE.start(), *TURNTABLE_SCALE.end());
    let (w, h) = (size.0 * scale, size.1 * scale);
    let mut out: Vec<Rgba> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| pixels[((y / scale) * size.0 + x / scale) as usize])
        .collect();
    if let Some(strength) = options.scanlines {
        let keep = 100 - u32::from(strength.clamp(*SCANLINES.start(), *SCANLINES.end()));
        let dark = |v: u8| (u32::from(v) * keep / 100) as u8;
        for y in (1..h).step_by(2) {
            let row = &mut out[(y * w) as usize..((y + 1) * w) as usize];
            for p in row.iter_mut().filter(|p| p.a >= 128) {
                *p = Rgba::new(dark(p.r), dark(p.g), dark(p.b), p.a);
            }
        }
    }
    Some((w, h, out))
}

/// Todos los frames del turntable de `model` con `options` (a su escala). `None` si no es un
/// modelo (o está roto).
pub fn turntable(
    doc: &Document,
    model: ArtboardId,
    options: &TurntableOptions,
) -> Option<Turntable> {
    let scale = options.scale();
    let mut frames = Vec::new();
    let (mut width, mut height) = (0, 0);
    for angle in 0..options.bake.angles() {
        let (w, h, pixels) = turntable_frame(doc, model, options, angle, scale)?;
        (width, height) = (w, h);
        frames.push(pixels);
    }
    Some(Turntable {
        width,
        height,
        frames,
    })
}

/// El color de `palette` (ordenada de oscuro a claro) del nivel de luminancia de `c`.
fn by_luminance(c: Rgba, palette: &[Rgba]) -> Rgba {
    let luminance = (299 * u32::from(c.r) + 587 * u32::from(c.g) + 114 * u32::from(c.b)) / 1000;
    let level = (luminance as usize * palette.len() / 256).min(palette.len() - 1);
    palette[level]
}

/// El color de `palette` más cercano a `c` (distancia RGB).
fn nearest(c: Rgba, palette: &[Rgba]) -> Rgba {
    let d = |p: &Rgba| {
        let (r, g, b) = (
            i32::from(c.r) - i32::from(p.r),
            i32::from(c.g) - i32::from(p.g),
            i32::from(c.b) - i32::from(p.b),
        );
        r * r + g * g + b * b
    };
    palette.iter().copied().min_by_key(d).unwrap_or(c)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use cs_commands::model::new_model_artboard;
    use cs_core::{ColorMode, Sprite};

    use super::*;
    use crate::{BakeOutput, bake_model, render_frame};

    const SIZE: u32 = 32;

    fn with_model() -> (Document, ArtboardId) {
        let mut doc = Document::with_sprite("host", Sprite::new(40, 40, ColorMode::Rgba));
        let (model, _, cmd) = new_model_artboard(&doc, "Cube", (SIZE, SIZE), (0, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, model)
    }

    fn options(doc: &Document, model: ArtboardId) -> TurntableOptions {
        let ab = doc.artboard(model).unwrap();
        let m = ab.model.as_ref().unwrap();
        TurntableOptions::from_model(m, (ab.sprite.width, ab.sprite.height))
    }

    fn colors(pixels: &[Rgba]) -> HashSet<Rgba> {
        pixels.iter().copied().filter(|p| p.a > 0).collect()
    }

    #[test]
    fn native_without_effects_is_the_bake() {
        let (doc, model) = with_model();
        // De entrada, el contorno del modelo: igual que el horneado.
        let o = options(&doc, model);
        let t = turntable(&doc, model, &o).unwrap();
        assert_eq!((t.width, t.height), (SIZE, SIZE));
        assert_eq!(t.frames.len(), 8);
        let bake = BakeOptions {
            output: BakeOutput::Frames,
            ..o.bake
        };
        let (sprite, _) = bake_model(&doc, model, &bake).unwrap();
        for f in [0, 5] {
            assert_eq!(t.frames[f], render_frame(&sprite, f), "{f}");
        }
    }

    #[test]
    fn the_scale_multiplies_the_size_with_nearest_pixels() {
        let (doc, model) = with_model();
        let one = options(&doc, model);
        let three = TurntableOptions { scale: 3, ..one };
        assert_eq!(three.output_size((SIZE, SIZE)), (SIZE * 3, SIZE * 3));
        let (_, _, a) = turntable_frame(&doc, model, &one, 0, 1).unwrap();
        let (w, h, b) = turntable_frame(&doc, model, &three, 0, 3).unwrap();
        assert_eq!((w, h), (SIZE * 3, SIZE * 3));
        for (x, y) in [(0, 0), (16, 16), (20, 9)] {
            let src = a[(y * SIZE + x) as usize];
            for (dx, dy) in [(0, 0), (2, 2), (1, 0)] {
                assert_eq!(b[((y * 3 + dy) * w + x * 3 + dx) as usize], src);
            }
        }
    }

    #[test]
    fn a_screen_preset_has_the_screen_size() {
        let (doc, model) = with_model();
        for (preset, size) in [
            (RenderPreset::NintendoDs, (256, 192)),
            (RenderPreset::GameBoy, (160, 144)),
            (RenderPreset::Pico8, (128, 128)),
        ] {
            let o = TurntableOptions {
                preset,
                scale: 2,
                ..options(&doc, model)
            };
            assert_eq!(o.output_size((SIZE, SIZE)), (size.0 * 2, size.1 * 2));
            let t = turntable(&doc, model, &TurntableOptions { scale: 1, ..o }).unwrap();
            assert_eq!((t.width, t.height), size, "{preset:?}");
            assert!(
                colors(&t.frames[0]).len() > 1,
                "{preset:?}: the cube is drawn"
            );
        }
    }

    #[test]
    fn the_screen_frame_keeps_the_model_aspect() {
        let (doc, model) = with_model();
        let o = TurntableOptions {
            preset: RenderPreset::NintendoDs,
            ..options(&doc, model)
        };
        assert_eq!(o.render_size((32, 32)), ((256, 192), Some((192, 192))));
        assert_eq!(o.render_size((64, 16)), ((256, 192), Some((256, 64))));
        assert_eq!(o.render_size((16, 64)), ((256, 192), Some((48, 192))));
    }

    #[test]
    fn the_gameboy_preset_uses_only_its_four_greens() {
        let (doc, model) = with_model();
        let o = TurntableOptions {
            preset: RenderPreset::GameBoy,
            background: Some(Rgba::rgb(255, 255, 255)),
            ..options(&doc, model)
        };
        let (_, _, pixels) = turntable_frame(&doc, model, &o, 1, 1).unwrap();
        let used = colors(&pixels);
        assert!(used.iter().all(|c| GAMEBOY.contains(c)), "{used:?}");
        assert!(used.len() >= 2);
        // El fondo blanco es el verde más claro.
        assert_eq!(pixels[0], GAMEBOY[3]);
    }

    #[test]
    fn the_pico8_preset_uses_only_its_palette() {
        let (doc, model) = with_model();
        let o = TurntableOptions {
            preset: RenderPreset::Pico8,
            ..options(&doc, model)
        };
        let (_, _, pixels) = turntable_frame(&doc, model, &o, 0, 1).unwrap();
        assert!(colors(&pixels).iter().all(|c| PICO8.contains(c)));
    }

    #[test]
    fn the_background_fills_the_transparent_pixels() {
        let (doc, model) = with_model();
        let clear = options(&doc, model);
        let (_, _, a) = turntable_frame(&doc, model, &clear, 0, 1).unwrap();
        assert_eq!(a[0].a, 0);
        let bg = Rgba::rgb(10, 20, 30);
        let o = TurntableOptions {
            background: Some(bg),
            ..clear
        };
        let (_, _, b) = turntable_frame(&doc, model, &o, 0, 1).unwrap();
        assert!(b.iter().all(|p| p.a == 255));
        assert_eq!(b[0], bg);
        let center = (SIZE / 2 * SIZE + SIZE / 2) as usize;
        assert_eq!(b[center], a[center], "the model is the same");
    }

    #[test]
    fn the_outline_is_chosen_in_the_options() {
        let (doc, model) = with_model();
        let black = Rgba::rgb(1, 2, 3);
        let off = TurntableOptions {
            outline: None,
            ..options(&doc, model)
        };
        let on = TurntableOptions {
            outline: Some(black),
            ..off
        };
        let (_, _, a) = turntable_frame(&doc, model, &off, 0, 1).unwrap();
        let (_, _, b) = turntable_frame(&doc, model, &on, 0, 1).unwrap();
        assert!(!a.contains(&black));
        assert!(b.contains(&black));
        let opaque = |p: &[Rgba]| p.iter().filter(|c| c.a > 0).count();
        assert!(opaque(&b) > opaque(&a), "the outline adds pixels around");
    }

    #[test]
    fn scanlines_darken_the_odd_rows_of_the_final_image() {
        let (doc, model) = with_model();
        let bg = Rgba::rgb(200, 100, 50);
        let plain = TurntableOptions {
            background: Some(bg),
            scale: 2,
            ..options(&doc, model)
        };
        let lined = TurntableOptions {
            scanlines: Some(50),
            ..plain
        };
        let (w, _, a) = turntable_frame(&doc, model, &plain, 0, 2).unwrap();
        let (_, _, b) = turntable_frame(&doc, model, &lined, 0, 2).unwrap();
        // La fila 0 queda igual y la 1 a la mitad (en el fondo, de color conocido).
        assert_eq!(b[0], a[0]);
        assert_eq!(b[w as usize], Rgba::rgb(100, 50, 25));
        // Sin fondo, lo transparente sigue transparente.
        let clear = TurntableOptions {
            background: None,
            ..lined
        };
        let (_, _, c) = turntable_frame(&doc, model, &clear, 0, 2).unwrap();
        assert_eq!(c[w as usize].a, 0);
    }

    #[test]
    fn the_frames_are_the_angles_and_differ() {
        let (doc, model) = with_model();
        let o = TurntableOptions {
            bake: BakeOptions {
                frames: 4,
                ..options(&doc, model).bake
            },
            ..options(&doc, model)
        };
        let t = turntable(&doc, model, &o).unwrap();
        assert_eq!(t.frames.len(), 4);
        assert_ne!(t.frames[0], t.frames[1]);
        assert!(turntable(&doc, ArtboardId(999), &o).is_none());
    }

    #[test]
    fn from_model_defaults() {
        let (doc, model) = with_model();
        let o = options(&doc, model);
        assert_eq!(o.preset, RenderPreset::Native);
        assert_eq!((o.scale, o.frame_ms), (1, 100));
        assert!(o.looping && o.background.is_none() && o.scanlines.is_none());
        assert!(!o.bake.animate_texture);
        assert_eq!(o.bake.frames, 8);
    }
}
