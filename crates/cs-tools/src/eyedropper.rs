use cs_raster::Point;

use crate::{Tool, ToolContext, ToolOutput};

/// Cuentagotas: toma el color que se ve (todas las capas visibles compuestas). En sprites
/// indexados devuelve el índice de paleta de ese color (o el más cercano).
#[derive(Debug, Default)]
pub struct Eyedropper {
    point: Option<Point>,
}

impl Eyedropper {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for Eyedropper {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.point = Some(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        if self.point.is_some() {
            self.point = Some(p);
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.point = None;
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        match self.point {
            Some(p) if ctx.in_canvas(p) => ToolOutput {
                pick: Some(visible_value(ctx.sprite, ctx.frame, p)),
                ..Default::default()
            },
            _ => ToolOutput::default(),
        }
    }

    fn is_active(&self) -> bool {
        self.point.is_some()
    }
}

/// Valor de píxel para el color visible en `p`, en el modo de color del sprite.
pub fn visible_value(sprite: &cs_core::Sprite, frame: usize, p: Point) -> cs_core::PixelValue {
    use cs_core::{ColorMode, PixelValue};
    // `composite_pixel` ya ve los tilemaps (un tile por capa, sin materializar).
    let c = cs_render::composite_pixel(sprite, frame, p.0, p.1);
    match sprite.color_mode {
        ColorMode::Rgba => PixelValue::Rgba(c),
        ColorMode::Indexed { transparent_index } => {
            if c.a < 128 {
                return PixelValue::Index(transparent_index);
            }
            let dist = |q: &cs_core::Rgba| {
                let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
                d(q.r, c.r) + d(q.g, c.g) + d(q.b, c.b)
            };
            let index = sprite
                .palette
                .colors()
                .iter()
                .enumerate()
                .filter(|(i, _)| *i as u8 != transparent_index)
                .min_by_key(|(_, q)| dist(q))
                .map_or(transparent_index, |(i, _)| i as u8);
            PixelValue::Index(index)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use cs_commands::Command;
    use cs_core::LayerId;

    #[test]
    fn picks_the_visible_color_across_layers() {
        use cs_core::{Layer, PixelValue, Rgba};
        let mut s = sprite();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 3, red())],
        }
        .apply(&mut s)
        .unwrap();
        // Capa de arriba con azul: se ve azul aunque la capa activa sea la de abajo.
        Command::AddLayer {
            index: 1,
            layer: Layer::new_image(LayerId(2), "top"),
        }
        .apply(&mut s)
        .unwrap();
        let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
        Command::DrawPixels {
            layer: LayerId(2),
            frame: 0,
            pixels: vec![(2, 3, blue)],
        }
        .apply(&mut s)
        .unwrap();
        let c = ctx(&s); // capa activa: 1
        let mut e = Eyedropper::new();
        e.press(&c, (2, 3));
        assert_eq!(e.release(&c).pick, Some(blue));
        // Oculta la de arriba: ahora se ve (y se toma) el rojo.
        s.layer_mut(LayerId(2)).unwrap().visible = false;
        let c = ctx(&s);
        e.press(&c, (2, 3));
        assert_eq!(e.release(&c).pick, Some(red()));
    }

    #[test]
    fn indexed_sprites_pick_the_palette_index() {
        use cs_core::{ColorMode, PixelValue, Sprite};
        let mut s = Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        );
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Index(27))],
        }
        .apply(&mut s)
        .unwrap();
        assert_eq!(visible_value(&s, 0, (1, 1)), PixelValue::Index(27));
        assert_eq!(visible_value(&s, 0, (0, 0)), PixelValue::Index(0));
    }

    #[test]
    fn picks_layer_pixel() {
        let mut s = sprite();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(2, 3, red())],
        }
        .apply(&mut s)
        .unwrap();
        let c = ctx(&s);
        let mut e = Eyedropper::new();
        e.press(&c, (2, 3));
        assert_eq!(e.release(&c).pick, Some(red()));
        e.press(&c, (20, 3));
        assert_eq!(e.release(&c).pick, None);
    }

    #[test]
    fn picks_colors_from_tilemap_layers() {
        use std::sync::Arc;

        use cs_core::{Cel, Image, ImageId, LayerKind, PixelValue, Rgba, Tile, TileGrid, Tileset};
        let mut s = sprite();
        let mut ts = Tileset::new("t", (2, 2), s.color_mode);
        let mut img = Image::new(2, 2, s.color_mode);
        img.set(0, 0, red()).unwrap();
        ts.tiles.push(Arc::new(img));
        s.tilesets.push(ts);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile::new(1, false, false, false));
        s.tilemaps.insert(ImageId(1), Arc::new(grid));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: ImageId(1),
            },
        );
        assert_eq!(visible_value(&s, 0, (2, 0)), red());
        assert_eq!(
            visible_value(&s, 0, (0, 0)),
            PixelValue::Rgba(Rgba::TRANSPARENT)
        );
    }
}
