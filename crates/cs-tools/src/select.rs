use cs_core::Mask;
use cs_raster::{Point, constrain_square};

use crate::{Drag, Tool, ToolContext, ToolOutput};

/// Selección rectangular. Un clic sin arrastrar deselecciona.
#[derive(Debug, Default)]
pub struct RectSelect {
    drag: Drag,
}

impl RectSelect {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tool for RectSelect {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.drag.press(p);
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        self.drag.drag(p);
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.drag.take();
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        let Some((a, b)) = self.drag.get() else {
            return ToolOutput::default();
        };
        let selection = if a == b {
            None
        } else {
            let b = if ctx.constrain {
                constrain_square(a, b)
            } else {
                b
            };
            let mask = Mask::from_rect(ctx.sprite.width, ctx.sprite.height, a, b);
            (!mask.is_empty()).then_some(mask)
        };
        ToolOutput {
            selection: Some(selection),
            ..Default::default()
        }
    }

    fn is_active(&self) -> bool {
        self.drag.get().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    #[test]
    fn drag_selects_and_click_deselects() {
        let s = sprite();
        let c = ctx(&s);
        let mut t = RectSelect::new();
        t.press(&c, (1, 1));
        t.drag(&c, (3, 2));
        let sel = t.release(&c).selection.unwrap().unwrap();
        assert_eq!(sel.bounds(), Some(((1, 1), (3, 2))));
        t.press(&c, (5, 5));
        assert_eq!(t.release(&c).selection, Some(None));
    }
}
