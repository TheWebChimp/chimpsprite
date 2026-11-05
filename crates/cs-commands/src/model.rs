//! Los modelos 3D nuevos: el artboard del modelo (con un cubo) y el de su textura, que nace con
//! la plantilla de zonas de las primitivas dibujada.

use std::sync::Arc;

use cs_3d::{Primitive, TEMPLATE_SIZE, darker, primitive_mesh};
use cs_core::{
    Artboard, ArtboardId, Cel, ColorMode, Document, Image, ImageId, Model3D, Object3D, Palette,
    PixelValue, Rgba, Sprite, Transform3D,
};

use crate::DocCommand;

/// Las zonas de la plantilla con el índice (en db32) de su color; las celdas del cubo comparten
/// color y se separan con la cuadrícula de 16 px.
const ZONES: [((i32, i32, i32, i32), u8); 7] = [
    ((0, 0, 48, 32), 6),    // cubo
    ((48, 0, 16, 16), 9),   // plano
    ((0, 32, 32, 16), 18),  // cilindro
    ((0, 48, 16, 16), 19),  // tapa de arriba
    ((16, 48, 16, 16), 17), // base
    ((32, 32, 32, 16), 28), // cono
    ((32, 48, 32, 16), 29), // esfera
];

/// El sprite de una textura nueva (RGBA, `TEMPLATE_SIZE`, 1 capa y 1 frame) con la plantilla de
/// las primitivas: cada zona rellena con un color de `palette`, un borde de 1 px un paso más
/// oscuro y la cuadrícula de 16 px del cubo marcada.
pub fn texture_template(palette: &Palette) -> Sprite {
    let (w, h) = TEMPLATE_SIZE;
    let mut sprite = Sprite::new(w, h, ColorMode::Rgba);
    sprite.palette = palette.clone();
    let colors = palette.colors();
    let color = |i: u8| match colors.len() {
        0 => Rgba::rgb(255, 255, 255),
        n => colors[i as usize % n],
    };
    let mut image = Image::new(w, h, ColorMode::Rgba);
    let mut put = |x: i32, y: i32, c: Rgba| {
        if image.set(x as u32, y as u32, PixelValue::Rgba(c)).is_err() {
            log::error!("template pixel ({x}, {y}) out of bounds");
        }
    };
    for ((zx, zy, zw, zh), i) in ZONES {
        let fill = color(i);
        let edge = darker(colors, fill);
        let cube = (zx, zy, zw, zh) == Primitive::Cube.zone();
        for y in zy..zy + zh {
            for x in zx..zx + zw {
                let border = x == zx || y == zy || x == zx + zw - 1 || y == zy + zh - 1;
                let grid = cube && (x % 16 == 0 || y % 16 == 0);
                put(x, y, if border || grid { edge } else { fill });
            }
        }
    }
    let id = ImageId(1);
    sprite.images.insert(id, Arc::new(image));
    sprite.cels.insert(
        (sprite.layers[0].id, 0),
        Cel {
            position: (0, 0),
            opacity: 255,
            image: id,
        },
    );
    sprite
}

/// Un modelo 3D nuevo de `size` con un cubo, y su textura en `position` (el modelo a su derecha,
/// 16 px después). Devuelve `(modelo, textura, comando)`; el comando los agrega al final.
pub fn new_model_artboard(
    doc: &Document,
    name: &str,
    size: (u32, u32),
    position: (i32, i32),
) -> (ArtboardId, ArtboardId, DocCommand) {
    let palette = Palette::db32();
    let texture_id = doc.next_artboard_id();
    let model_id = ArtboardId(texture_id.0 + 1);
    let artboard = |id, name: String, position, sprite, model| Artboard {
        id,
        name,
        position,
        sprite,
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: false,
        font: None,
        model,
    };
    let texture = artboard(
        texture_id,
        format!("{name} Texture"),
        position,
        texture_template(&palette),
        None,
    );
    let cube = Primitive::Cube;
    let model = Model3D {
        objects: vec![Object3D {
            name: cube.name().to_owned(),
            mesh: primitive_mesh(&cube),
            transform: Transform3D::default(),
            double_sided: false,
            kind: Some(cube.kind()),
            hidden: false,
            locked: false,
        }],
        texture: Some(texture_id),
        ..Default::default()
    };
    let mut sprite = Sprite::new(size.0, size.1, ColorMode::Rgba);
    sprite.palette = palette;
    let model_artboard = artboard(
        model_id,
        name.to_owned(),
        (position.0 + TEMPLATE_SIZE.0 as i32 + 16, position.1),
        sprite,
        Some(model),
    );
    let index = doc.artboards.len();
    let cmd = DocCommand::Batch(vec![
        DocCommand::AddArtboard {
            index,
            artboard: Box::new(texture),
        },
        DocCommand::AddArtboard {
            index: index + 1,
            artboard: Box::new(model_artboard),
        },
    ]);
    (model_id, texture_id, cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{ColorMode, PixelValue, Rgba, Sprite};

    #[test]
    fn a_new_model_brings_its_texture_and_undoes() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let before = doc.clone();
        let (model_id, texture_id, cmd) = new_model_artboard(&doc, "Box", (48, 40), (10, 20));
        let undo = cmd.apply(&mut doc).unwrap();
        assert_eq!(doc.artboards.len(), 3);
        assert_ne!(model_id, texture_id);

        let tex = doc.artboard(texture_id).unwrap();
        assert_eq!(tex.position, (10, 20));
        assert_eq!((tex.sprite.width, tex.sprite.height), TEMPLATE_SIZE);
        assert_eq!(tex.sprite.color_mode, ColorMode::Rgba);
        assert!(tex.model.is_none());
        let cel = &tex.sprite.cels[&(tex.sprite.layers[0].id, 0)];
        let img = &tex.sprite.images[&cel.image];
        let opaque = |x, y| matches!(img.get(x, y), Some(PixelValue::Rgba(c)) if c.a == 255);
        assert!(opaque(8, 8), "the cube zone is painted");
        assert!(opaque(0, 0) && opaque(47, 31));
        // Borde más oscuro y cuadrícula de 16 px en el cubo.
        assert_ne!(img.get(0, 0), img.get(8, 8));
        assert_eq!(img.get(16, 8), img.get(0, 8));
        assert_ne!(img.get(24, 8), img.get(16, 8));

        let m = doc.artboard(model_id).unwrap();
        assert_eq!(m.name, "Box");
        assert_eq!((m.sprite.width, m.sprite.height), (48, 40));
        assert_eq!(m.position, (10 + TEMPLATE_SIZE.0 as i32 + 16, 20));
        assert_eq!(m.sprite.frames.len(), 1);
        assert_eq!(m.sprite.layers.len(), 1);
        let model = m.model.as_ref().unwrap();
        assert_eq!(model.texture, Some(texture_id));
        assert_eq!(model.objects.len(), 1);
        assert_eq!(model.objects[0].name, "Cube");
        assert_eq!(model.check(&m.sprite), Ok(()));

        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    /// Los pixeles RGBA de la plantilla (fila por fila).
    fn template_pixels() -> Vec<Rgba> {
        let tex = texture_template(&Palette::db32());
        let img = &tex.images[&tex.cels.values().next().unwrap().image];
        let (w, h) = TEMPLATE_SIZE;
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| match img.get(x, y) {
                Some(PixelValue::Rgba(c)) => c,
                _ => Rgba::TRANSPARENT,
            })
            .collect()
    }

    /// Render iso de 64×64 de `objects` con la plantilla de textura.
    fn render_iso(objects: Vec<Object3D>) -> Vec<Rgba> {
        let model = Model3D {
            objects,
            camera: cs_core::CameraPreset::Iso.camera(5 * cs_core::FIXED, Default::default()),
            ..Default::default()
        };
        let pixels = template_pixels();
        let palette = Palette::db32();
        cs_3d::render_model(&cs_3d::RenderInput {
            model: &model,
            size: (64, 64),
            texture: Some(cs_3d::Texture {
                width: TEMPLATE_SIZE.0,
                height: TEMPLATE_SIZE.1,
                pixels: &pixels,
            }),
            palette: palette.colors(),
            camera: None,
            extra_yaw: 0,
            frame: None,
        })
    }

    fn object(p: Primitive, transform: Transform3D) -> Object3D {
        Object3D {
            name: p.name().into(),
            mesh: primitive_mesh(&p),
            transform,
            double_sided: false,
            kind: Some(p.kind()),
            hidden: false,
            locked: false,
        }
    }

    #[test]
    fn every_primitive_paints_with_the_template() {
        for p in [
            Primitive::Cube,
            Primitive::Plane,
            Primitive::Cylinder { sides: 8 },
            Primitive::Cone { sides: 4 },
            Primitive::Cone { sides: 8 },
            Primitive::Sphere { sides: 8, rings: 4 },
        ] {
            let img = render_iso(vec![object(p, Transform3D::default())]);
            let painted = img.iter().filter(|c| c.a > 0).count();
            assert!(painted > 50, "{p:?}: {painted} pixeles pintados");
        }
    }

    #[test]
    fn the_template_uses_the_palette() {
        let palette = Palette::db32();
        let tex = texture_template(&palette);
        assert_eq!(tex.palette, palette);
        let cel = tex.cels.values().next().unwrap();
        let img = &tex.images[&cel.image];
        for (x, y, w, h) in Primitive::Cube.zones() {
            let Some(PixelValue::Rgba(c)) = img.get((x + w / 2) as u32, (y + h / 2) as u32) else {
                panic!("not rgba");
            };
            assert!(palette.colors().contains(&c));
        }
    }
}
