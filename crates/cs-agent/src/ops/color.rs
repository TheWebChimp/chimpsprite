//! Paleta, recolor y cuantización.

use std::collections::BTreeSet;
use std::sync::Arc;

use cs_commands::{Command, DocCommand};
use cs_core::{ColorMode, Image, ImageId, LayerId, Palette, PixelBuffer, PixelValue, Rgba, Sprite};

use crate::error::{Result, invalid};
use crate::params::{Quantize, Recolor, Scope, SetPalette};
use crate::quantize::{median_cut, nearest};
use crate::target;
use crate::{Reply, Session};

/// Imágenes (sin repetir) de los cels del alcance, saltando capas bloqueadas.
fn images_in_scope(
    sprite: &Sprite,
    scope: Scope,
    layer: LayerId,
    frame: usize,
) -> (Vec<ImageId>, usize) {
    let mut seen = BTreeSet::new();
    let mut locked = 0;
    for (&(l, f), cel) in &sprite.cels {
        let hit = match scope {
            Scope::Cel => l == layer && f == frame,
            Scope::Layer => l == layer,
            Scope::Frame => f == frame,
            Scope::Sprite => true,
        };
        if !hit {
            continue;
        }
        if sprite.layer(l).is_some_and(|l| l.locked) {
            locked += 1;
            continue;
        }
        seen.insert(cel.image);
    }
    (seen.into_iter().collect(), locked)
}

/// Copia de `image` con cada píxel transformado por `f`.
fn map_image(image: &Image, mut f: impl FnMut(PixelValue) -> PixelValue) -> Image {
    let (w, h) = (image.width(), image.height());
    let mut out = image.clone();
    for y in 0..h {
        for x in 0..w {
            let v = image.get(x, y).expect("in bounds");
            out.set(x, y, f(v)).expect("same mode");
        }
    }
    out
}

impl Session {
    pub fn set_palette(&mut self, p: SetPalette) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        if p.colors.is_empty() || p.colors.len() > 256 {
            return Err(invalid("colors must have 1-256 entries"));
        }
        let colors = p
            .colors
            .iter()
            .map(|c| target::hex(c))
            .collect::<Result<Vec<_>>>()?;
        if let ColorMode::Indexed { .. } = sprite.color_mode {
            let used = sprite
                .images
                .values()
                .filter_map(|i| match i.pixels() {
                    PixelBuffer::Indexed(px) => px.iter().copied().max(),
                    PixelBuffer::Rgba(_) => None,
                })
                .max()
                .unwrap_or(0);
            if used as usize >= colors.len() {
                return Err(invalid(format!(
                    "the sprite uses palette index {used}; the new palette needs at least {} colors",
                    used as usize + 1
                )));
            }
        }
        let n = colors.len();
        self.run_on(&p.doc, ab, vec![Command::SetPalette(Palette::new(colors))])?;
        Ok(Reply::text(format!("Palette set ({n} colors).")))
    }

    pub fn recolor(&mut self, p: Recolor) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        if p.map.is_empty() {
            return Err(invalid("map needs at least one {from, to} pair"));
        }
        let pairs = p
            .map
            .iter()
            .map(|m| {
                Ok((
                    target::color(sprite, &m.from)?,
                    target::color(sprite, &m.to)?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let layer = match p.scope {
            Scope::Cel | Scope::Layer => target::image_layer(sprite, p.layer.as_ref())?,
            Scope::Frame | Scope::Sprite => LayerId(0),
        };
        let frame = target::frame(sprite, p.frame)?;
        let (images, locked) = images_in_scope(sprite, p.scope, layer, frame);
        let mut changed = 0;
        let mut cmds = Vec::new();
        for id in images {
            let image = sprite.image(id).expect("cel image exists");
            let mut hits = 0;
            let new = map_image(image, |v| match pairs.iter().find(|(from, _)| *from == v) {
                Some(&(_, to)) => {
                    hits += 1;
                    to
                }
                None => v,
            });
            if hits > 0 {
                changed += hits;
                cmds.push(Command::SetImage {
                    id,
                    image: Some(Arc::new(new)),
                });
            }
        }
        if cmds.is_empty() {
            return Ok(Reply::text("No pixels matched; nothing changed."));
        }
        self.run_on(&p.doc, ab, cmds)?;
        let mut text = format!("Recolored {changed} pixel(s).");
        if locked > 0 {
            text.push_str(&format!(" Skipped {locked} cel(s) on locked layers."));
        }
        Ok(Reply::text(text))
    }

    pub fn quantize(&mut self, p: Quantize) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        if !sprite.color_mode.stores_rgba() {
            return Err(invalid(
                "the sprite is already indexed; edit its palette with set_palette or recolor",
            ));
        }
        // Los tiles también tendrían que convertirse; queda para cuando se editen tilemaps.
        if !sprite.tilesets.is_empty() {
            return Err(invalid(
                "quantize does not support artboards with tilesets yet",
            ));
        }
        let limit = if p.to_indexed { 255 } else { 256 };
        let palette = match (&p.max_colors, &p.palette) {
            (Some(n), None) => {
                if !(1..=limit).contains(n) {
                    return Err(invalid(format!("max_colors must be 1-{limit}")));
                }
                let colors = sprite.images.values().flat_map(|i| match i.pixels() {
                    PixelBuffer::Rgba(px) => px.clone(),
                    PixelBuffer::Indexed(_) => Vec::new(),
                });
                median_cut(colors.filter(|c| c.a > 0), *n as usize)
            }
            (None, Some(list)) => {
                if list.is_empty() || list.len() > limit as usize {
                    return Err(invalid(format!("palette must have 1-{limit} colors")));
                }
                list.iter()
                    .map(|c| target::hex(c))
                    .collect::<Result<Vec<_>>>()?
            }
            _ => return Err(invalid("pass either max_colors or palette")),
        };
        if palette.is_empty() {
            return Err(invalid("the sprite has no visible pixels to quantize"));
        }
        let mut out = sprite.clone();
        for image in out.images.values_mut() {
            let PixelBuffer::Rgba(px) = image.pixels() else {
                continue;
            };
            let mapped = if p.to_indexed {
                PixelBuffer::Indexed(
                    px.iter()
                        .map(|c| {
                            if c.a < 128 {
                                0
                            } else {
                                1 + nearest(&palette, *c) as u8
                            }
                        })
                        .collect(),
                )
            } else {
                PixelBuffer::Rgba(
                    px.iter()
                        .map(|&c| {
                            if c.a == 0 {
                                c
                            } else {
                                Rgba {
                                    a: c.a,
                                    ..palette[nearest(&palette, c)]
                                }
                            }
                        })
                        .collect(),
                )
            };
            *image = Arc::new(
                Image::from_buffer(image.width(), image.height(), mapped).expect("same size"),
            );
        }
        if p.to_indexed {
            let mut colors = vec![Rgba::TRANSPARENT];
            colors.extend_from_slice(&palette);
            out.palette = Palette::new(colors);
            out.color_mode = ColorMode::Indexed {
                transparent_index: 0,
            };
        } else {
            out.palette = Palette::new(palette.clone());
        }
        self.run(
            &p.doc,
            vec![DocCommand::SetSprite {
                id: ab,
                sprite: Box::new(out),
            }],
        )?;
        let hexes: Vec<_> = palette.iter().map(|c| c.to_hex()).collect();
        Ok(Reply::text(format!(
            "Quantized to {} color(s){}: {}",
            palette.len(),
            if p.to_indexed {
                " and converted to indexed (index 0 = transparent)"
            } else {
                ""
            },
            hexes.join(" ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{p, session};

    fn rgba(c: Rgba) -> PixelValue {
        PixelValue::Rgba(c)
    }

    #[test]
    fn recolor_scopes_and_linked_cels() {
        let mut s = session(4, 4);
        s.draw_pixels(p(json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}, {"x": 1, "y": 0, "color": "#0f0"}]})))
            .unwrap();
        s.add_frame(p(json!({"doc": "d1", "duplicate": 0})))
            .unwrap();
        s.recolor(p(
            json!({"doc": "d1", "map": [{"from": "#f00", "to": "#00f"}]}),
        ))
        .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.pixel(LayerId(1), 0, 0, 0), rgba(Rgba::rgb(0, 0, 255)));
        assert_eq!(
            sp.pixel(LayerId(1), 1, 0, 0),
            rgba(Rgba::rgb(255, 0, 0)),
            "cel scope"
        );
        s.recolor(p(
            json!({"doc": "d1", "map": [{"from": "#0f0", "to": "transparent"}], "scope": "sprite"}),
        ))
        .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.pixel(LayerId(1), 1, 1, 0), rgba(Rgba::TRANSPARENT));
        let r = s
            .recolor(p(
                json!({"doc": "d1", "map": [{"from": "#123456", "to": "#000"}]}),
            ))
            .unwrap();
        assert!(r.text.starts_with("No pixels"));
    }

    #[test]
    fn quantize_to_indexed_and_undo() {
        let mut s = session(4, 1);
        s.draw_pixels(p(json!({"doc": "d1", "pixels": [
            {"x": 0, "y": 0, "color": "#ff0000"}, {"x": 1, "y": 0, "color": "#f00a00"},
            {"x": 2, "y": 0, "color": "#0000ff"}]})))
            .unwrap();
        let before = s.sprite("d1", None).unwrap().1.clone();
        s.quantize(p(json!({"doc": "d1", "max_colors": 2, "to_indexed": true})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(
            sp.color_mode,
            ColorMode::Indexed {
                transparent_index: 0
            }
        );
        assert_eq!(sp.palette.len(), 3);
        assert_eq!(sp.pixel(LayerId(1), 0, 0, 0), sp.pixel(LayerId(1), 0, 1, 0));
        assert_ne!(sp.pixel(LayerId(1), 0, 0, 0), sp.pixel(LayerId(1), 0, 2, 0));
        assert_eq!(sp.pixel(LayerId(1), 0, 3, 0), PixelValue::Index(0));
        assert!(
            s.quantize(p(json!({"doc": "d1", "max_colors": 2})))
                .is_err(),
            "already indexed"
        );
        // En indexado se dibuja con índices o con colores de la paleta.
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 3, "y": 0, "color": 1}]}),
        ))
        .unwrap();
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1, &before);
    }

    #[test]
    fn quantize_to_a_given_palette_keeps_rgba() {
        let mut s = session(2, 1);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#ee1111"}]}),
        ))
        .unwrap();
        s.quantize(p(json!({"doc": "d1", "palette": ["#000000", "#ff0000"]})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.pixel(LayerId(1), 0, 0, 0), rgba(Rgba::rgb(255, 0, 0)));
        assert_eq!(sp.palette.len(), 2);
        assert!(s.quantize(p(json!({"doc": "d1"}))).is_err());
    }

    #[test]
    fn set_palette_protects_used_indices() {
        let mut s = Session::new(None);
        s.new_document(p(json!({"width": 2, "height": 2, "color_mode": "indexed"})))
            .unwrap();
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": 5}]}),
        ))
        .unwrap();
        assert!(
            s.set_palette(p(json!({"doc": "d1", "colors": ["#000", "#fff"]})))
                .is_err()
        );
        s.set_palette(p(
            json!({"doc": "d1", "colors": ["#000", "#111", "#222", "#333", "#444", "#555"]}),
        ))
        .unwrap();
        assert!(
            s.set_palette(p(json!({"doc": "d1", "colors": ["#zz0"]})))
                .is_err()
        );
    }
}
