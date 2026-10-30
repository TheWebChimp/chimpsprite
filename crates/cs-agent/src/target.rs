//! Resolver referencias del agente (artboard, capa, frame, color, rectángulo) a valores del modelo.

use cs_core::{ArtboardId, ColorMode, Document, LayerId, LayerKind, PixelValue, Rgba, Sprite};

use crate::error::{Result, invalid};
use crate::params::{ColorArg, Rect, Ref};

/// Artboard por id o nombre; sin referencia, el primero del lienzo (no uno de la biblioteca).
pub fn artboard(doc: &Document, r: Option<&Ref>) -> Result<ArtboardId> {
    let found = match r {
        None => doc.canvas_artboards().next(),
        Some(Ref::Id(id)) => doc.artboards.iter().find(|a| a.id.0 == *id),
        Some(Ref::Name(name)) => doc.artboards.iter().find(|a| a.name == *name).or_else(|| {
            name.parse()
                .ok()
                .and_then(|id: u32| doc.artboards.iter().find(|a| a.id.0 == id))
        }),
    };
    found.map(|a| a.id).ok_or_else(|| {
        let names: Vec<_> = doc
            .artboards
            .iter()
            .map(|a| format!("{} '{}'", a.id.0, a.name))
            .collect();
        invalid(format!(
            "artboard {r:?} not found; artboards: {}",
            names.join(", ")
        ))
    })
}

/// Capa por id o nombre.
pub fn layer(sprite: &Sprite, r: &Ref) -> Result<LayerId> {
    let found = match r {
        Ref::Id(id) => sprite.layers.iter().find(|l| l.id.0 == *id),
        Ref::Name(name) => sprite.layers.iter().find(|l| l.name == *name).or_else(|| {
            name.parse()
                .ok()
                .and_then(|id: u32| sprite.layers.iter().find(|l| l.id.0 == id))
        }),
    };
    found.map(|l| l.id).ok_or_else(|| {
        let names: Vec<_> = sprite
            .layers
            .iter()
            .map(|l| format!("{} '{}'", l.id.0, l.name))
            .collect();
        invalid(format!(
            "layer {r:?} not found; layers: {}",
            names.join(", ")
        ))
    })
}

/// Capa de imagen indicada; sin referencia, la capa de imagen de más arriba.
pub fn image_layer(sprite: &Sprite, r: Option<&Ref>) -> Result<LayerId> {
    let id = match r {
        Some(r) => layer(sprite, r)?,
        None => sprite
            .layers
            .iter()
            .rev()
            .find(|l| l.kind == LayerKind::Image)
            .map(|l| l.id)
            .ok_or_else(|| invalid("the sprite has no image layer; use add_layer"))?,
    };
    let l = sprite.layer(id).expect("resolved");
    if l.kind != LayerKind::Image {
        let what = match l.kind {
            LayerKind::Group => "a group",
            LayerKind::Objects => "an object layer",
            LayerKind::Tilemap => "a tilemap layer (use set_tiles or set_tile_pixels)",
            LayerKind::Image => unreachable!("checked above"),
        };
        return Err(invalid(format!(
            "layer '{}' is {what}, not an image layer",
            l.name
        )));
    }
    Ok(id)
}

pub fn frame(sprite: &Sprite, frame: usize) -> Result<usize> {
    if frame < sprite.frames.len() {
        Ok(frame)
    } else {
        Err(invalid(format!(
            "frame {frame} out of range; the sprite has {} frame(s), numbered from 0",
            sprite.frames.len()
        )))
    }
}

/// Rango inclusivo de frames, validado.
pub fn frames(sprite: &Sprite, from: usize, to: usize) -> Result<std::ops::RangeInclusive<usize>> {
    if from > to {
        return Err(invalid(format!("frame range {from}..{to} is reversed")));
    }
    frame(sprite, to)?;
    Ok(from..=to)
}

pub fn hex(text: &str) -> Result<Rgba> {
    Rgba::parse_hex(text).ok_or_else(|| {
        invalid(format!(
            "invalid color '{text}'; use #rgb, #rrggbb or #rrggbbaa"
        ))
    })
}

/// Color del agente como valor de píxel válido para el modo del sprite.
pub fn color(sprite: &Sprite, c: &ColorArg) -> Result<PixelValue> {
    match (c, sprite.color_mode) {
        (ColorArg::Text(t), mode) if t.eq_ignore_ascii_case("transparent") => {
            Ok(mode.transparent())
        }
        (ColorArg::Text(t), ColorMode::Rgba) => Ok(PixelValue::Rgba(hex(t)?)),
        (ColorArg::Text(t), ColorMode::Grayscale) => Ok(PixelValue::Rgba(hex(t)?.to_gray())),
        (ColorArg::Text(t), ColorMode::Indexed { .. }) => {
            let rgba = hex(t)?;
            let index = sprite
                .palette
                .colors()
                .iter()
                .position(|&p| p == rgba)
                .ok_or_else(|| {
                    invalid(format!(
                        "color {t} is not in the palette of this indexed sprite; use a palette \
                         index or add it with set_palette"
                    ))
                })?;
            Ok(PixelValue::Index(index as u8))
        }
        (ColorArg::Index(i), ColorMode::Indexed { .. }) => {
            if (*i as usize) < sprite.palette.len() {
                Ok(PixelValue::Index(*i))
            } else {
                Err(invalid(format!(
                    "palette index {i} out of range (palette has {} colors)",
                    sprite.palette.len()
                )))
            }
        }
        (ColorArg::Index(_), ColorMode::Rgba | ColorMode::Grayscale) => Err(invalid(
            "palette indices only work on indexed sprites; use a hex color like \"#ff0000\"",
        )),
    }
}

/// Color para pintar en la capa o, con `on_mask`, en su máscara (donde vale cualquier hex aunque
/// el sprite sea indexado: solo importa su luminosidad).
pub fn paint_value(sprite: &Sprite, c: &ColorArg, on_mask: bool) -> Result<PixelValue> {
    match c {
        ColorArg::Text(t) if on_mask && !t.eq_ignore_ascii_case("transparent") => {
            Ok(PixelValue::Rgba(hex(t)?))
        }
        ColorArg::Text(_) if on_mask => Ok(PixelValue::Rgba(Rgba::TRANSPARENT)),
        _ => color(sprite, c),
    }
}

/// Rectángulo indicado (o el lienzo completo) recortado al lienzo, como `(x0, y0, x1, y1)`
/// con máximo exclusivo. Error si queda vacío.
pub fn region(sprite: &Sprite, rect: Option<Rect>) -> Result<(i32, i32, i32, i32)> {
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    let Some(r) = rect else {
        return Ok((0, 0, w, h));
    };
    let (x0, y0) = (r.x.max(0), r.y.max(0));
    let (x1, y1) = ((r.x + r.w as i32).min(w), (r.y + r.h as i32).min(h));
    if x0 >= x1 || y0 >= y1 {
        return Err(invalid(format!(
            "rect {r:?} does not overlap the {w}×{h} canvas"
        )));
    }
    Ok((x0, y0, x1, y1))
}

/// ¿El punto cae dentro del recorte (si hay)?
pub fn in_clip(clip: Option<Rect>, x: i32, y: i32) -> bool {
    clip.is_none_or(|r| x >= r.x && y >= r.y && x < r.x + r.w as i32 && y < r.y + r.h as i32)
}

#[cfg(test)]
mod tests {
    use cs_core::{Artboard, Palette};

    use super::*;

    fn doc() -> Document {
        let mut d = Document::with_sprite("hero", Sprite::new(4, 4, ColorMode::Rgba));
        d.artboards.push(Artboard {
            in_library: false,
            id: ArtboardId(2),
            name: "coin".into(),
            position: (10, 0),
            sprite: Sprite::new(
                2,
                2,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            ),
            guides: Vec::new(),
            symmetry: Default::default(),
        });
        d
    }

    #[test]
    fn artboards_by_id_name_or_default() {
        let d = doc();
        assert_eq!(artboard(&d, None).unwrap(), ArtboardId(1));
        assert_eq!(artboard(&d, Some(&Ref::Id(2))).unwrap(), ArtboardId(2));
        assert_eq!(
            artboard(&d, Some(&Ref::Name("coin".into()))).unwrap(),
            ArtboardId(2)
        );
        assert_eq!(
            artboard(&d, Some(&Ref::Name("2".into()))).unwrap(),
            ArtboardId(2)
        );
        let err = artboard(&d, Some(&Ref::Name("nope".into())))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("1 'hero'") && err.contains("2 'coin'"),
            "{err}"
        );
    }

    #[test]
    fn the_default_artboard_is_the_first_on_the_canvas() {
        let mut d = doc();
        d.artboards[0].in_library = true;
        assert_eq!(artboard(&d, None).unwrap(), ArtboardId(2));
    }

    #[test]
    fn default_layer_is_the_top_image_layer() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        let mut group = cs_core::Layer::new_image(LayerId(2), "g");
        group.kind = LayerKind::Group;
        s.layers.push(group);
        assert_eq!(image_layer(&s, None).unwrap(), LayerId(1));
        assert!(image_layer(&s, Some(&Ref::Name("g".into()))).is_err());
        assert_eq!(layer(&s, &Ref::Name("Layer 1".into())).unwrap(), LayerId(1));
    }

    #[test]
    fn colors_follow_the_color_mode() {
        let rgba = Sprite::new(1, 1, ColorMode::Rgba);
        assert_eq!(
            color(&rgba, &ColorArg::Text("#f00".into())).unwrap(),
            PixelValue::Rgba(Rgba::rgb(255, 0, 0))
        );
        assert_eq!(
            color(&rgba, &ColorArg::Text("transparent".into())).unwrap(),
            PixelValue::Rgba(Rgba::TRANSPARENT)
        );
        assert!(color(&rgba, &ColorArg::Index(3)).is_err());
        assert!(color(&rgba, &ColorArg::Text("red".into())).is_err());

        let mut indexed = Sprite::new(
            1,
            1,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        indexed.palette = Palette::new(vec![Rgba::TRANSPARENT, Rgba::rgb(1, 2, 3)]);
        assert_eq!(
            color(&indexed, &ColorArg::Index(1)).unwrap(),
            PixelValue::Index(1)
        );
        assert!(color(&indexed, &ColorArg::Index(2)).is_err());
        assert_eq!(
            color(&indexed, &ColorArg::Text("#010203".into())).unwrap(),
            PixelValue::Index(1)
        );
        assert!(color(&indexed, &ColorArg::Text("#ffffff".into())).is_err());
        assert_eq!(
            color(&indexed, &ColorArg::Text("transparent".into())).unwrap(),
            PixelValue::Index(0)
        );
    }

    #[test]
    fn regions_are_clipped_to_the_canvas() {
        let s = Sprite::new(4, 4, ColorMode::Rgba);
        assert_eq!(region(&s, None).unwrap(), (0, 0, 4, 4));
        let r = Rect {
            x: -2,
            y: 1,
            w: 4,
            h: 10,
        };
        assert_eq!(region(&s, Some(r)).unwrap(), (0, 1, 2, 4));
        assert!(
            region(
                &s,
                Some(Rect {
                    x: 9,
                    y: 0,
                    w: 1,
                    h: 1
                })
            )
            .is_err()
        );
        assert!(in_clip(None, 99, 99));
        assert!(in_clip(Some(r), -1, 3) && !in_clip(Some(r), 2, 3));
    }
}
