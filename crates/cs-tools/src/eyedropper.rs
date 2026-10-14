use cs_raster::Point;

use crate::{Tool, ToolContext, ToolOutput};

/// Cuentagotas: toma el valor del píxel de la capa actual.
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
                pick: Some(ctx.sprite.pixel(ctx.layer, ctx.frame, p.0, p.1)),
                ..Default::default()
            },
            _ => ToolOutput::default(),
        }
    }

    fn is_active(&self) -> bool {
        self.point.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;
    use cs_commands::Command;
    use cs_core::LayerId;

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
}
