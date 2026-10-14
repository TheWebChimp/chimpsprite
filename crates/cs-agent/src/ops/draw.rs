//! Dibujo: píxeles sueltos, líneas, rectángulos, elipses y relleno.

use cs_commands::Command;
use cs_core::{ArtboardId, LayerId, PixelValue};
use cs_raster::Point;

use crate::error::{Result, invalid};
use crate::params::{ColorArg, DrawLine, DrawPixels, DrawShape, Fill, Rect, Ref};
use crate::target;
use crate::{Reply, Session};

/// Tamaño máximo del pincel.
const MAX_BRUSH: u32 = 64;

/// Destino de un trazo ya resuelto.
struct Stroke {
    artboard: ArtboardId,
    layer: LayerId,
    frame: usize,
    color: PixelValue,
}

impl Session {
    fn stroke(
        &self,
        doc: &str,
        artboard: Option<&Ref>,
        layer: Option<&Ref>,
        frame: usize,
        color: &ColorArg,
    ) -> Result<Stroke> {
        let (artboard, sprite) = self.sprite(doc, artboard)?;
        Ok(Stroke {
            artboard,
            layer: target::image_layer(sprite, layer)?,
            frame: target::frame(sprite, frame)?,
            color: target::color(sprite, color)?,
        })
    }

    /// Pinta `points` (recortados a `clip`) con un solo color.
    fn paint(
        &mut self,
        doc: &str,
        s: &Stroke,
        points: Vec<Point>,
        clip: Option<Rect>,
    ) -> Result<Reply> {
        let pixels: Vec<_> = points
            .into_iter()
            .filter(|&(x, y)| target::in_clip(clip, x, y))
            .map(|(x, y)| (x, y, s.color))
            .collect();
        self.draw(doc, s.artboard, s.layer, s.frame, pixels)
    }

    fn draw(
        &mut self,
        doc: &str,
        artboard: ArtboardId,
        layer: LayerId,
        frame: usize,
        mut pixels: Vec<(i32, i32, PixelValue)>,
    ) -> Result<Reply> {
        let (_, sprite) = self.sprite(doc, Some(&Ref::Id(artboard.0)))?;
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        pixels.retain(|&(x, y, _)| x >= 0 && y >= 0 && x < w && y < h);
        pixels.sort_by_key(|&(x, y, _)| (y, x));
        pixels.dedup_by_key(|&mut (x, y, _)| (x, y));
        if pixels.is_empty() {
            return Ok(Reply::text(
                "Nothing drawn: every pixel was outside the canvas or the clip.",
            ));
        }
        let n = pixels.len();
        self.run_on(
            doc,
            artboard,
            vec![Command::DrawPixels {
                layer,
                frame,
                pixels,
            }],
        )?;
        Ok(Reply::text(format!(
            "Drew {n} pixel(s) on layer {} frame {frame}.",
            layer.0
        )))
    }

    pub fn draw_pixels(&mut self, p: DrawPixels) -> Result<Reply> {
        let (artboard, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::image_layer(sprite, p.layer.as_ref())?;
        let frame = target::frame(sprite, p.frame)?;
        let mut pixels = Vec::with_capacity(p.pixels.len());
        // El último gana si un píxel se repite.
        for px in p.pixels.iter().rev() {
            if target::in_clip(p.clip, px.x, px.y) {
                pixels.push((px.x, px.y, target::color(sprite, &px.color)?));
            }
        }
        pixels.sort_by_key(|&(x, y, _)| (y, x));
        self.draw(&p.doc, artboard, layer, frame, pixels)
    }

    pub fn draw_line(&mut self, p: DrawLine) -> Result<Reply> {
        let s = self.stroke(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            &p.color,
        )?;
        let vertices: Vec<Point> = p.points.iter().map(|&[x, y]| (x, y)).collect();
        let mut path = match vertices.as_slice() {
            [] => return Err(invalid("points needs at least one [x, y]")),
            [only] => vec![*only],
            _ => Vec::new(),
        };
        for pair in vertices.windows(2) {
            path.extend(cs_raster::line(pair[0], pair[1]));
        }
        let points = cs_raster::stamp(&path, brush(p.brush_size)?);
        self.paint(&p.doc, &s, points, p.clip)
    }

    pub fn draw_rect(&mut self, p: DrawShape) -> Result<Reply> {
        let s = self.stroke(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            &p.color,
        )?;
        let (a, b) = ((p.x0, p.y0), (p.x1, p.y1));
        let points = if p.filled {
            cs_raster::rect_filled(a, b)
        } else {
            cs_raster::stamp(&cs_raster::rect_outline(a, b), brush(p.brush_size)?)
        };
        self.paint(&p.doc, &s, points, p.clip)
    }

    pub fn draw_ellipse(&mut self, p: DrawShape) -> Result<Reply> {
        let s = self.stroke(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            &p.color,
        )?;
        let (a, b) = ((p.x0, p.y0), (p.x1, p.y1));
        let points = if p.filled {
            cs_raster::ellipse_filled(a, b)
        } else {
            cs_raster::stamp(&cs_raster::ellipse_outline(a, b), brush(p.brush_size)?)
        };
        self.paint(&p.doc, &s, points, p.clip)
    }

    pub fn fill(&mut self, p: Fill) -> Result<Reply> {
        let s = self.stroke(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            &p.color,
        )?;
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (w, h) = (sprite.width, sprite.height);
        if p.x < 0 || p.y < 0 || p.x >= w as i32 || p.y >= h as i32 {
            return Err(invalid(format!(
                "({}, {}) is outside the {w}×{h} canvas",
                p.x, p.y
            )));
        }
        let at = |x: i32, y: i32| sprite.pixel(s.layer, s.frame, x, y);
        let seed = at(p.x, p.y);
        let matches = |x: i32, y: i32| target::in_clip(p.clip, x, y) && at(x, y) == seed;
        let points = if p.contiguous.unwrap_or(true) {
            cs_raster::flood_fill(w, h, (p.x, p.y), matches)
        } else {
            (0..h as i32)
                .flat_map(|y| (0..w as i32).map(move |x| (x, y)))
                .filter(|&(x, y)| matches(x, y))
                .collect()
        };
        self.paint(&p.doc, &s, points, p.clip)
    }
}

fn brush(size: Option<u32>) -> Result<u32> {
    match size.unwrap_or(1) {
        s @ 1..=MAX_BRUSH => Ok(s),
        s => Err(invalid(format!("brush_size {s} must be 1-{MAX_BRUSH}"))),
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{LayerId, Rgba};
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{p, session};

    fn px(s: &Session, x: i32, y: i32) -> PixelValue {
        s.sprite("d1", None).unwrap().1.pixel(LayerId(1), 0, x, y)
    }

    const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    const CLEAR: PixelValue = PixelValue::Rgba(Rgba::TRANSPARENT);

    #[test]
    fn pixels_last_wins_and_clip() {
        let mut s = session(4, 4);
        s.draw_pixels(p(json!({"doc": "d1", "pixels": [
            {"x": 1, "y": 1, "color": "#00f"}, {"x": 1, "y": 1, "color": "#f00"},
            {"x": 3, "y": 3, "color": "#f00"}, {"x": 9, "y": 9, "color": "#f00"}],
            "clip": {"x": 0, "y": 0, "w": 2, "h": 2}})))
            .unwrap();
        assert_eq!(px(&s, 1, 1), RED);
        assert_eq!(px(&s, 3, 3), CLEAR);
        let r = s
            .draw_pixels(p(
                json!({"doc": "d1", "pixels": [{"x": 9, "y": 9, "color": "#f00"}]}),
            ))
            .unwrap();
        assert!(r.text.starts_with("Nothing drawn"));
        assert!(
            s.draw_pixels(p(
                json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "nope"}]})
            ))
            .is_err()
        );
    }

    #[test]
    fn polyline_with_brush_and_erase() {
        let mut s = session(8, 8);
        s.draw_line(p(
            json!({"doc": "d1", "points": [[0, 0], [3, 0], [3, 3]], "color": "#f00"}),
        ))
        .unwrap();
        for (x, y) in [(0, 0), (2, 0), (3, 0), (3, 2), (3, 3)] {
            assert_eq!(px(&s, x, y), RED, "({x}, {y})");
        }
        s.draw_line(p(
            json!({"doc": "d1", "points": [[6, 6]], "color": "#f00", "brush_size": 2}),
        ))
        .unwrap();
        let n = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| x >= 5 && y >= 5 && px(&s, x, y) == RED)
            .count();
        assert_eq!(n, 4);
        s.draw_line(p(
            json!({"doc": "d1", "points": [[0, 0], [3, 0]], "color": "transparent"}),
        ))
        .unwrap();
        assert_eq!(px(&s, 1, 0), CLEAR);
        assert!(
            s.draw_line(p(json!({"doc": "d1", "points": [], "color": "#f00"})))
                .is_err()
        );
        assert!(
            s.draw_line(p(
                json!({"doc": "d1", "points": [[0, 0]], "color": "#f00", "brush_size": 0})
            ))
            .is_err()
        );
    }

    #[test]
    fn rect_ellipse_and_fill() {
        let mut s = session(8, 8);
        s.draw_rect(p(
            json!({"doc": "d1", "x0": 0, "y0": 0, "x1": 5, "y1": 5, "color": "#f00"}),
        ))
        .unwrap();
        assert_eq!(px(&s, 0, 3), RED);
        assert_eq!(px(&s, 2, 2), CLEAR);
        s.fill(p(json!({"doc": "d1", "x": 2, "y": 2, "color": "#0f0"})))
            .unwrap();
        assert_eq!(px(&s, 2, 2), PixelValue::Rgba(Rgba::rgb(0, 255, 0)));
        assert_eq!(px(&s, 7, 7), CLEAR, "outside the box stays empty");
        s.fill(p(
            json!({"doc": "d1", "x": 7, "y": 7, "color": "#00f", "contiguous": false}),
        ))
        .unwrap();
        assert_eq!(px(&s, 7, 0), PixelValue::Rgba(Rgba::rgb(0, 0, 255)));
        assert!(
            s.fill(p(json!({"doc": "d1", "x": 8, "y": 0, "color": "#00f"})))
                .is_err()
        );

        let mut s = session(8, 8);
        s.draw_ellipse(p(json!({"doc": "d1", "x0": 0, "y0": 0, "x1": 7, "y1": 7, "color": "#f00", "filled": true})))
            .unwrap();
        assert_eq!(px(&s, 4, 4), RED);
        assert_eq!(px(&s, 0, 0), CLEAR);
    }

    #[test]
    fn locked_layers_report_a_clear_error() {
        let mut s = session(4, 4);
        s.set_layer(p(json!({"doc": "d1", "layer": 1, "locked": true})))
            .unwrap();
        let err = s
            .draw_pixels(p(
                json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}]}),
            ))
            .unwrap_err()
            .to_string();
        assert!(err.contains("locked"), "{err}");
    }
}
