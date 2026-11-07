//! Pixel art isométrico: la caja de Isobox y las pilas de bloques (`cs_raster::iso`), pintadas
//! en un solo `DrawPixels`.

use cs_core::{ColorMode, PixelValue, Rgba, Sprite};
use cs_raster::{IsoBlocks, IsoBox, IsoColors, IsoCorner, IsoParts};

use crate::error::{Result, invalid};
use crate::params::{ColorArg, DrawIsoBlocks, DrawIsoBox, IsoStyle, Rect};
use crate::target;
use crate::{Reply, Session};

/// Lo más largo de una arista de la caja, en unidades.
const MAX_UNITS: u32 = 512;
/// Lo más alto de los costados de la caja, en píxeles.
const MAX_HEIGHT: u32 = 2048;
/// Celdas por lado de la rejilla de bloques.
const MAX_CELLS: usize = 32;
/// Bloques por pila.
const MAX_STACK: u32 = 64;

/// El color de un argumento como RGBA (en indexado, el de la paleta).
fn rgba(sprite: &Sprite, c: &ColorArg) -> Result<Rgba> {
    Ok(cs_render::resolve(sprite, target::color(sprite, c)?))
}

/// ¿Es "none" (sin esa parte)?
fn is_none(c: &ColorArg) -> bool {
    matches!(c, ColorArg::Text(t) if t.eq_ignore_ascii_case("none"))
}

/// Los colores de cada parte, si va el contorno y si va el highlight.
fn style(sprite: &Sprite, s: &IsoStyle) -> Result<(IsoColors, bool, bool)> {
    let mut colors = IsoColors::from_base(rgba(sprite, &s.color)?);
    let set = |slot: &mut Rgba, c: &Option<ColorArg>| -> Result<bool> {
        match c {
            Some(c) if is_none(c) => Ok(false),
            Some(c) => {
                *slot = rgba(sprite, c)?;
                Ok(true)
            }
            None => Ok(true),
        }
    };
    for (slot, c, what) in [
        (&mut colors.top, &s.top_color, "top_color"),
        (&mut colors.left, &s.left_color, "left_color"),
        (&mut colors.right, &s.right_color, "right_color"),
    ] {
        if !set(slot, c)? {
            return Err(invalid(format!("{what} can't be \"none\"; give a color")));
        }
    }
    let outline = set(&mut colors.outline, &s.outline)?;
    let highlight = set(&mut colors.highlight, &s.highlight)?;
    let indexed = matches!(sprite.color_mode, ColorMode::Indexed { .. });
    if s.snap_to_palette || indexed {
        colors = colors.snapped(sprite.palette.colors());
    }
    Ok((colors, outline, highlight))
}

fn corner(c: Option<u32>) -> Result<IsoCorner> {
    let px = c.unwrap_or(3);
    IsoCorner::from_pixels(px).ok_or_else(|| invalid(format!("corner must be 2, 3 or 4, not {px}")))
}

/// El valor de píxel de `c` para el modo del sprite (en indexado, el índice más cercano).
fn value(sprite: &Sprite, c: Rgba) -> PixelValue {
    match sprite.color_mode {
        ColorMode::Indexed { transparent_index } => {
            PixelValue::Index(sprite.palette.nearest(c, transparent_index))
        }
        ColorMode::Grayscale => PixelValue::Rgba(c.to_gray()),
        ColorMode::Rgba => PixelValue::Rgba(c),
    }
}

impl Session {
    /// Pinta `parts` con su esquina en `(x, y)`.
    #[allow(clippy::too_many_arguments)]
    fn paint_iso(
        &mut self,
        doc: &str,
        artboard: Option<&crate::params::Ref>,
        layer: Option<&crate::params::Ref>,
        frame: usize,
        at: (i32, i32),
        parts: &IsoParts,
        style_arg: &IsoStyle,
        clip: Option<Rect>,
    ) -> Result<Reply> {
        let (artboard, sprite) = self.sprite(doc, artboard)?;
        let layer = target::image_layer(sprite, layer)?;
        let frame = target::frame(sprite, frame)?;
        let (colors, _, _) = style(sprite, style_arg)?;
        let pixels: Vec<(i32, i32, PixelValue)> = parts
            .points()
            .map(|(x, y, part)| (at.0 + x, at.1 + y, value(sprite, colors.color(part))))
            .filter(|&(x, y, _)| target::in_clip(clip, x, y))
            .collect();
        let size = (parts.width, parts.height);
        let reply = self.draw(doc, artboard, layer, frame, pixels, false)?;
        Ok(Reply::text(format!(
            "{} The drawing is {}×{} px with its top-left at ({}, {}).",
            reply.text, size.0, size.1, at.0, at.1
        )))
    }

    pub fn draw_iso_box(&mut self, p: DrawIsoBox) -> Result<Reply> {
        for (name, units) in [("left", p.left), ("right", p.right)] {
            if !(1..=MAX_UNITS).contains(&units) {
                return Err(invalid(format!("{name} must be 1-{MAX_UNITS} units")));
            }
        }
        if p.height > MAX_HEIGHT {
            return Err(invalid(format!("height must be 0-{MAX_HEIGHT} px")));
        }
        let shape = IsoBox {
            left: p.left,
            right: p.right,
            height: p.height,
            corner: corner(p.corner)?,
        };
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (_, outline, highlight) = style(sprite, &p.style)?;
        let parts = cs_raster::iso_box(&shape, outline, highlight);
        self.paint_iso(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            (p.x, p.y),
            &parts,
            &p.style,
            p.clip,
        )
    }

    pub fn draw_iso_blocks(&mut self, p: DrawIsoBlocks) -> Result<Reply> {
        let rows = p.heights.len();
        let columns = p.heights.iter().map(Vec::len).max().unwrap_or(0);
        if rows == 0 || columns == 0 || rows > MAX_CELLS || columns > MAX_CELLS {
            return Err(invalid(format!(
                "heights needs 1-{MAX_CELLS} rows of 1-{MAX_CELLS} cells"
            )));
        }
        if p.heights.iter().flatten().any(|&h| h > MAX_STACK) {
            return Err(invalid(format!(
                "a stack can have up to {MAX_STACK} blocks"
            )));
        }
        let size = p.size.unwrap_or(2);
        let block_height = p.block_height.unwrap_or(4);
        if !(1..=64).contains(&size) || block_height > 256 {
            return Err(invalid("size must be 1-64 units and block_height 0-256 px"));
        }
        let mut blocks = IsoBlocks::new(columns as u32, rows as u32);
        blocks.size = size;
        blocks.block_height = block_height;
        blocks.corner = corner(p.corner)?;
        for (r, row) in p.heights.iter().enumerate() {
            for c in 0..columns {
                let h = row.get(c).copied().unwrap_or(0);
                blocks.set_height(c as u32, r as u32, h);
            }
        }
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (_, outline, highlight) = style(sprite, &p.style)?;
        let parts = cs_raster::iso_blocks(&blocks, outline, highlight);
        if parts.width == 0 {
            return Err(invalid("every stack is empty: nothing to draw"));
        }
        self.paint_iso(
            &p.doc,
            p.artboard.as_ref(),
            p.layer.as_ref(),
            p.frame,
            (p.x, p.y),
            &parts,
            &p.style,
            p.clip,
        )
    }
}

#[cfg(test)]
mod tests {
    use cs_core::LayerId;
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::*;

    fn px(s: &Session, x: i32, y: i32) -> PixelValue {
        s.sprite("d1", None).unwrap().1.pixel(LayerId(1), 0, x, y)
    }

    fn rgb(s: &Session, x: i32, y: i32) -> Rgba {
        let sprite = s.sprite("d1", None).unwrap().1;
        cs_render::resolve(sprite, px(s, x, y))
    }

    #[test]
    fn a_box_is_drawn_in_place_with_shades_of_the_color() {
        let mut s = session(16, 16);
        let r = s
            .draw_iso_box(p(
                json!({"doc": "d1", "x": 2, "y": 3, "left": 2, "right": 2,
                "height": 2, "corner": 2, "color": "#c05030"}),
            ))
            .unwrap();
        assert!(r.text.contains("6×6 px"), "{}", r.text);
        let c = IsoColors::from_base(Rgba::rgb(0xc0, 0x50, 0x30));
        // El de `a_small_cube_with_a_two_pixel_corner`, corrido (2, 3).
        assert_eq!(rgb(&s, 2 + 2, 3), c.outline);
        assert_eq!(rgb(&s, 2 + 2, 3 + 1), c.top);
        assert_eq!(rgb(&s, 2 + 1, 3 + 2), c.highlight);
        assert_eq!(rgb(&s, 2 + 1, 3 + 3), c.left);
        assert_eq!(rgb(&s, 2 + 4, 3 + 3), c.right);
        assert_eq!(rgb(&s, 2, 3), Rgba::TRANSPARENT);
        // Un paso de undo.
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        assert_eq!(rgb(&s, 2 + 2, 3 + 1), Rgba::TRANSPARENT);
    }

    #[test]
    fn faces_can_be_given_and_outline_or_highlight_left_out() {
        let mut s = session(16, 16);
        s.draw_iso_box(p(
            json!({"doc": "d1", "x": 0, "y": 0, "left": 2, "right": 2,
            "height": 2, "corner": 2, "color": "#808080", "top_color": "#ff0000",
            "right_color": "#0000ff", "outline": "none", "highlight": "none"}),
        ))
        .unwrap();
        // Sin contorno, la punta de arriba es la cara de arriba.
        assert_eq!(rgb(&s, 2, 0), Rgba::rgb(255, 0, 0));
        assert_eq!(rgb(&s, 1, 3), Rgba::rgb(128, 128, 128));
        assert_eq!(rgb(&s, 5, 3), Rgba::rgb(0, 0, 255));
        assert!(
            s.draw_iso_box(p(
                json!({"doc": "d1", "x": 0, "y": 0, "left": 2, "right": 2,
                "height": 2, "color": "#808080", "top_color": "none"})
            ))
            .is_err()
        );
        assert!(
            s.draw_iso_box(p(
                json!({"doc": "d1", "x": 0, "y": 0, "left": 0, "right": 2,
                "height": 2, "color": "#808080"})
            ))
            .is_err()
        );
        assert!(
            s.draw_iso_box(p(
                json!({"doc": "d1", "x": 0, "y": 0, "left": 2, "right": 2,
                "height": 2, "corner": 5, "color": "#808080"})
            ))
            .is_err()
        );
    }

    #[test]
    fn indexed_sprites_get_the_nearest_palette_colors() {
        let mut s = Session::new(None);
        s.new_document(p(
            json!({"width": 16, "height": 16, "color_mode": "indexed"}),
        ))
        .unwrap();
        s.draw_iso_box(p(
            json!({"doc": "d1", "x": 0, "y": 0, "left": 3, "right": 3,
            "height": 4, "color": 7}),
        ))
        .unwrap();
        let sprite = s.sprite("d1", None).unwrap().1;
        let mut seen = std::collections::HashSet::new();
        for y in 0..16 {
            for x in 0..16 {
                match px(&s, x, y) {
                    PixelValue::Index(i) => {
                        seen.insert(i);
                    }
                    v => panic!("not an index: {v:?}"),
                }
            }
        }
        // El transparente, el base (7) y los tonos de la paleta.
        assert!(
            seen.contains(&7) && seen.contains(&0) && seen.len() >= 4,
            "{seen:?}"
        );
        assert!(seen.iter().all(|&i| (i as usize) < sprite.palette.len()));
    }

    #[test]
    fn blocks_are_drawn_back_to_front() {
        let mut s = session(32, 32);
        let r = s
            .draw_iso_blocks(p(json!({"doc": "d1", "x": 0, "y": 0,
                "heights": [[1, 1], [0, 2]], "size": 2, "block_height": 3,
                "color": "#30a060"})))
            .unwrap();
        assert!(
            r.text.contains("px with its top-left at (0, 0)"),
            "{}",
            r.text
        );
        let parts = {
            let mut b = IsoBlocks::new(2, 2);
            b.heights = vec![1, 1, 0, 2];
            b.block_height = 3;
            b.corner = IsoCorner::Three;
            cs_raster::iso_blocks(&b, true, true)
        };
        let c = IsoColors::from_base(Rgba::rgb(0x30, 0xa0, 0x60));
        for (x, y, part) in parts.points() {
            assert_eq!(rgb(&s, x, y), c.color(part), "({x}, {y})");
        }
        assert!(
            s.draw_iso_blocks(p(json!({"doc": "d1", "x": 0, "y": 0,
                "heights": [[0, 0]], "color": "#30a060"})))
                .is_err()
        );
        assert!(
            s.draw_iso_blocks(p(json!({"doc": "d1", "x": 0, "y": 0,
                "heights": [], "color": "#30a060"})))
                .is_err()
        );
    }
}
