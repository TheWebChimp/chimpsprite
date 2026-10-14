//! Transformaciones de píxeles en una región: voltear, rotar, desplazar y copiar.

use std::collections::BTreeSet;

use cs_commands::Command;
use cs_core::{PixelValue, Sprite};

use crate::error::{Result, invalid};
use crate::params::{CopyRegion, Flip, FlipAxis, FrameRange, Rect, Ref, Rotate, Shift};
use crate::target;
use crate::{Reply, Session};

/// Región `(x0, y0, x1, y1)` (máximo exclusivo) y, para cada destino `(u, v)` local, el origen
/// local del que toma su píxel (`None` = queda transparente).
type Mapping<'a> = &'a dyn Fn(i32, i32) -> Option<(i32, i32)>;

struct Targets<'a> {
    doc: &'a str,
    artboard: Option<&'a Ref>,
    layer: Option<&'a Ref>,
    frame: usize,
    frames: Option<FrameRange>,
    rect: Option<Rect>,
}

impl Session {
    /// Aplica `mapping` a la región de cada cel afectado (los vinculados, una sola vez).
    fn transform(
        &mut self,
        t: Targets,
        what: &str,
        mapping: impl Fn(i32, i32, i32, i32) -> Option<(i32, i32)>,
    ) -> Result<Reply> {
        let (ab, sprite) = self.sprite(t.doc, t.artboard)?;
        let layer = target::image_layer(sprite, t.layer)?;
        let frames = match t.frames {
            Some(r) => target::frames(sprite, r.from, r.to)?,
            None => {
                let f = target::frame(sprite, t.frame)?;
                f..=f
            }
        };
        let (x0, y0, x1, y1) = target::region(sprite, t.rect)?;
        let (w, h) = (x1 - x0, y1 - y0);
        let map = |u: i32, v: i32| mapping(u, v, w, h);
        let mut done = BTreeSet::new();
        let mut cmds = Vec::new();
        for frame in frames {
            let Some(cel) = sprite.cel(layer, frame) else {
                continue;
            };
            if !done.insert(cel.image) {
                continue;
            }
            let pixels = remap(sprite, layer, frame, (x0, y0, w, h), &map);
            cmds.push(Command::DrawPixels {
                layer,
                frame,
                pixels,
            });
        }
        if cmds.is_empty() {
            return Ok(Reply::text("No cels in range; nothing to transform."));
        }
        let n = cmds.len();
        self.run_on(t.doc, ab, cmds)?;
        Ok(Reply::text(format!(
            "{what} {n} cel(s) of layer {}.",
            layer.0
        )))
    }

    pub fn flip(&mut self, p: Flip) -> Result<Reply> {
        let t = Targets {
            doc: &p.doc,
            artboard: p.artboard.as_ref(),
            layer: p.layer.as_ref(),
            frame: p.frame,
            frames: p.frames,
            rect: p.rect,
        };
        let axis = p.axis;
        self.transform(t, "Flipped", move |u, v, w, h| {
            Some(match axis {
                FlipAxis::Horizontal => (w - 1 - u, v),
                FlipAxis::Vertical => (u, h - 1 - v),
            })
        })
    }

    pub fn rotate(&mut self, p: Rotate) -> Result<Reply> {
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (x0, y0, x1, y1) = target::region(sprite, p.rect)?;
        if matches!(p.degrees, 90 | 270) && x1 - x0 != y1 - y0 {
            return Err(invalid(format!(
                "rotating by {}° needs a square region; this one is {}×{}. Pass a square rect",
                p.degrees,
                x1 - x0,
                y1 - y0
            )));
        }
        let degrees = p.degrees;
        if !matches!(degrees, 90 | 180 | 270) {
            return Err(invalid("degrees must be 90, 180 or 270"));
        }
        let t = Targets {
            doc: &p.doc,
            artboard: p.artboard.as_ref(),
            layer: p.layer.as_ref(),
            frame: p.frame,
            frames: p.frames,
            rect: p.rect,
        };
        // Horario: el destino (u, v) toma el píxel que la rotación inversa lleva hasta ahí.
        self.transform(t, "Rotated", move |u, v, w, h| {
            Some(match degrees {
                90 => (v, w - 1 - u),
                180 => (w - 1 - u, h - 1 - v),
                _ => (h - 1 - v, u),
            })
        })
    }

    pub fn shift(&mut self, p: Shift) -> Result<Reply> {
        let t = Targets {
            doc: &p.doc,
            artboard: p.artboard.as_ref(),
            layer: p.layer.as_ref(),
            frame: p.frame,
            frames: p.frames,
            rect: p.rect,
        };
        let (dx, dy, wrap) = (p.dx, p.dy, p.wrap);
        self.transform(t, "Shifted", move |u, v, w, h| {
            let (su, sv) = (u - dx, v - dy);
            if wrap {
                Some((su.rem_euclid(w), sv.rem_euclid(h)))
            } else {
                (su >= 0 && sv >= 0 && su < w && sv < h).then_some((su, sv))
            }
        })
    }

    pub fn copy_region(&mut self, p: CopyRegion) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let src_layer = target::image_layer(sprite, p.from.layer.as_ref())?;
        let src_frame = target::frame(sprite, p.from.frame)?;
        let dst_layer = target::image_layer(sprite, p.to.layer.as_ref())?;
        let dst_frame = target::frame(sprite, p.to.frame)?;
        let (x0, y0, x1, y1) = target::region(sprite, Some(p.rect))?;
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let transparent = sprite.color_mode.transparent();
        let mut pixels = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                let v = sprite.pixel(src_layer, src_frame, x, y);
                let (tx, ty) = (p.to.x + x - x0, p.to.y + y - y0);
                if v != transparent && tx >= 0 && ty >= 0 && tx < w && ty < h {
                    pixels.push((tx, ty, v));
                }
            }
        }
        if pixels.is_empty() {
            return Ok(Reply::text("The region has no visible pixels to paste."));
        }
        let n = pixels.len();
        self.run_on(
            &p.doc,
            ab,
            vec![Command::DrawPixels {
                layer: dst_layer,
                frame: dst_frame,
                pixels,
            }],
        )?;
        Ok(Reply::text(format!(
            "Pasted {n} pixel(s) at ({}, {}).",
            p.to.x, p.to.y
        )))
    }
}

/// Píxeles nuevos de la región `(x0, y0, w, h)` del cel según `map`.
fn remap(
    sprite: &Sprite,
    layer: cs_core::LayerId,
    frame: usize,
    (x0, y0, w, h): (i32, i32, i32, i32),
    map: Mapping,
) -> Vec<(i32, i32, PixelValue)> {
    let transparent = sprite.color_mode.transparent();
    let mut out = Vec::with_capacity((w * h) as usize);
    for v in 0..h {
        for u in 0..w {
            let value = match map(u, v) {
                Some((su, sv)) => sprite.pixel(layer, frame, x0 + su, y0 + sv),
                None => transparent,
            };
            out.push((x0 + u, y0 + v, value));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use cs_core::{LayerId, Rgba};
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{p, session};

    const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));

    fn red_at(s: &Session, frame: usize) -> Vec<(i32, i32)> {
        let (_, sp) = s.sprite("d1", None).unwrap();
        (0..sp.height as i32)
            .flat_map(|y| (0..sp.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| sp.pixel(LayerId(1), frame, x, y) == RED)
            .collect()
    }

    fn dot(s: &mut Session, x: i32, y: i32) {
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": x, "y": y, "color": "#f00"}]}),
        ))
        .unwrap();
    }

    #[test]
    fn flip_and_rotate_the_canvas() {
        let mut s = session(4, 4);
        dot(&mut s, 0, 1);
        s.flip(p(json!({"doc": "d1", "axis": "horizontal"})))
            .unwrap();
        assert_eq!(red_at(&s, 0), [(3, 1)]);
        s.flip(p(json!({"doc": "d1", "axis": "vertical"}))).unwrap();
        assert_eq!(red_at(&s, 0), [(3, 2)]);
        s.rotate(p(json!({"doc": "d1", "degrees": 90}))).unwrap();
        assert_eq!(red_at(&s, 0), [(1, 3)], "clockwise: (x, y) -> (h-1-y, x)");
        s.rotate(p(json!({"doc": "d1", "degrees": 270}))).unwrap();
        assert_eq!(red_at(&s, 0), [(3, 2)]);
        s.rotate(p(json!({"doc": "d1", "degrees": 180}))).unwrap();
        assert_eq!(red_at(&s, 0), [(0, 1)]);
        assert!(s.rotate(p(json!({"doc": "d1", "degrees": 45}))).is_err());
        assert!(
            s.rotate(p(
                json!({"doc": "d1", "degrees": 90, "rect": {"x": 0, "y": 0, "w": 2, "h": 3}})
            ))
            .is_err()
        );
    }

    #[test]
    fn transforms_inside_a_rect_only() {
        let mut s = session(6, 2);
        dot(&mut s, 0, 0);
        dot(&mut s, 5, 0);
        s.flip(p(
            json!({"doc": "d1", "axis": "horizontal", "rect": {"x": 0, "y": 0, "w": 2, "h": 2}}),
        ))
        .unwrap();
        assert_eq!(red_at(&s, 0), [(1, 0), (5, 0)]);
    }

    #[test]
    fn shift_with_and_without_wrap() {
        let mut s = session(4, 4);
        dot(&mut s, 3, 0);
        s.shift(p(json!({"doc": "d1", "dx": 1, "dy": 1, "wrap": true})))
            .unwrap();
        assert_eq!(red_at(&s, 0), [(0, 1)]);
        s.shift(p(json!({"doc": "d1", "dx": -1, "dy": 0}))).unwrap();
        assert!(red_at(&s, 0).is_empty(), "falls off the left edge");
    }

    #[test]
    fn linked_cels_transform_once_across_frames() {
        let mut s = session(4, 4);
        dot(&mut s, 0, 0);
        s.add_frame(p(json!({"doc": "d1", "duplicate": 0, "linked": true})))
            .unwrap();
        s.add_frame(p(json!({"doc": "d1", "duplicate": 1})))
            .unwrap();
        s.shift(p(
            json!({"doc": "d1", "dx": 1, "dy": 0, "frames": {"from": 0, "to": 2}}),
        ))
        .unwrap();
        assert_eq!(red_at(&s, 0), [(1, 0)]);
        assert_eq!(red_at(&s, 1), [(1, 0)], "linked: shifted once, not twice");
        assert_eq!(red_at(&s, 2), [(1, 0)]);
    }

    #[test]
    fn copy_region_pastes_over() {
        let mut s = session(6, 6);
        dot(&mut s, 0, 0);
        s.add_layer(p(json!({"doc": "d1", "name": "top"}))).unwrap();
        s.copy_region(p(
            json!({"doc": "d1", "rect": {"x": 0, "y": 0, "w": 2, "h": 2},
            "from": {"layer": 1}, "to": {"layer": "top", "x": 4, "y": 4}}),
        ))
        .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.pixel(LayerId(2), 0, 4, 4), RED);
        assert_eq!(
            sp.pixel(LayerId(2), 0, 0, 0),
            PixelValue::Rgba(Rgba::TRANSPARENT)
        );
    }
}
