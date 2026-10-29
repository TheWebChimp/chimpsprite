use std::sync::Arc;

use cs_core::{Cel, Image, LayerId, LayerKind, PixelValue, Sprite};

use crate::{CmdError, Command};

pub(crate) fn draw_pixels(
    doc: &mut Sprite,
    layer_id: LayerId,
    frame: usize,
    pixels: &[(i32, i32, PixelValue)],
) -> Result<Command, CmdError> {
    let layer = doc
        .layer(layer_id)
        .ok_or(CmdError::LayerNotFound(layer_id))?;
    if layer.kind != LayerKind::Image {
        return Err(CmdError::NotImageLayer(layer_id));
    }
    if layer.locked {
        return Err(CmdError::LayerLocked(layer_id));
    }
    if frame >= doc.frames.len() {
        return Err(CmdError::FrameOutOfRange(frame));
    }
    if pixels.iter().any(|&(_, _, v)| !doc.color_mode.accepts(v)) {
        return Err(CmdError::ColorModeMismatch);
    }
    let (w, h) = (doc.width as i32, doc.height as i32);
    // En escala de grises, cada color pasa a su gris.
    let pixels: Vec<_> = pixels
        .iter()
        .copied()
        .filter(|&(x, y, _)| x >= 0 && y >= 0 && x < w && y < h)
        .map(|(x, y, v)| (x, y, doc.color_mode.adapt(v)))
        .collect();
    if pixels.is_empty() {
        return Ok(Command::Batch(Vec::new()));
    }

    let mode = doc.color_mode;
    let existing = doc.cel(layer_id, frame).copied();
    let (cel, old_image) = match existing {
        Some(cel) => (cel, doc.image(cel.image).cloned()),
        None => {
            let cel = Cel {
                position: (0, 0),
                opacity: 255,
                image: doc.next_image_id(),
            };
            (cel, None)
        }
    };
    let mut image = match &old_image {
        Some(img) => img.clone(),
        None => Arc::new(Image::new(doc.width, doc.height, mode)),
    };

    // Expande el cel si algún píxel cae fuera de él.
    let (cx0, cy0) = cel.position;
    let (cx1, cy1) = (cx0 + image.width() as i32, cy0 + image.height() as i32);
    let (mut x0, mut y0, mut x1, mut y1) = (cx0, cy0, cx1, cy1);
    for &(x, y, _) in &pixels {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + 1);
        y1 = y1.max(y + 1);
    }
    let mut new_position = cel.position;
    if (x0, y0, x1, y1) != (cx0, cy0, cx1, cy1) {
        let mut grown = Image::new((x1 - x0) as u32, (y1 - y0) as u32, mode);
        let (ox, oy) = ((cx0 - x0) as u32, (cy0 - y0) as u32);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let v = image.get(x, y).expect("in bounds");
                grown
                    .set(ox + x, oy + y, v)
                    .expect("grown image covers old cel");
            }
        }
        image = Arc::new(grown);
        new_position = (x0, y0);
    }

    let img = Arc::make_mut(&mut image);
    for &(x, y, v) in &pixels {
        img.set((x - new_position.0) as u32, (y - new_position.1) as u32, v)
            .expect("cel covers all pixels");
    }
    doc.images.insert(cel.image, image);

    let mut inverse = vec![Command::SetImage {
        id: cel.image,
        image: old_image,
    }];
    if existing.is_none() {
        doc.cels.insert(
            (layer_id, frame),
            Cel {
                position: new_position,
                ..cel
            },
        );
        inverse.push(Command::SetCel {
            layer: layer_id,
            frame,
            cel: None,
        });
    } else if new_position != cel.position {
        // Los cels vinculados comparten imagen, así que se mueven juntos.
        let linked: Vec<_> = doc
            .cels
            .iter()
            .filter(|(_, c)| c.image == cel.image)
            .map(|(k, _)| *k)
            .collect();
        for key in linked {
            let c = doc.cels.get_mut(&key).expect("key exists");
            inverse.push(Command::SetCel {
                layer: key.0,
                frame: key.1,
                cel: Some(*c),
            });
            c.position = new_position;
        }
    }
    inverse.reverse();
    Ok(Command::Batch(inverse))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cs_core::*;

    use crate::{CmdError, Command};

    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    #[test]
    fn draw_creates_canvas_sized_cel_and_undo_removes_it() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let before = s.clone();
        let cmd = Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 2, red()), (9, 9, red())],
        };
        let inv = cmd.apply(&mut s).unwrap();
        let cel = *s.cel(LayerId(1), 0).unwrap();
        let img = s.image(cel.image).unwrap();
        assert_eq!((img.width(), img.height()), (4, 4));
        assert_eq!(img.get(1, 2), Some(red()));
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn draw_expands_small_cel() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        s.images
            .insert(ImageId(1), Arc::new(Image::new(2, 2, ColorMode::Rgba)));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (2, 2),
                opacity: 255,
                image: ImageId(1),
            },
        );
        let before = s.clone();
        let inv = Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red())],
        }
        .apply(&mut s)
        .unwrap();
        let cel = *s.cel(LayerId(1), 0).unwrap();
        assert_eq!(cel.position, (0, 0));
        let img = s.image(cel.image).unwrap();
        assert_eq!((img.width(), img.height()), (4, 4));
        assert_eq!(img.get(0, 0), Some(red()));
        inv.apply(&mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn draw_on_locked_layer_fails_without_changes() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        s.layers[0].locked = true;
        let before = s.clone();
        let r = Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red())],
        }
        .apply(&mut s);
        assert_eq!(r, Err(CmdError::LayerLocked(LayerId(1))));
        assert_eq!(s, before);
    }

    #[test]
    fn draw_wrong_mode_fails() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let r = Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Index(1))],
        }
        .apply(&mut s);
        assert_eq!(r, Err(CmdError::ColorModeMismatch));
    }
}
