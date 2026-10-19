use cs_commands::Command;
use cs_core::Cel;
use cs_raster::Point;

use crate::{Drag, Tool, ToolContext, ToolOutput};

/// Mover: desplaza el contenido seleccionado o, sin selección, el cel completo.
#[derive(Debug, Default)]
pub struct MoveTool {
    drag: Drag,
}

impl MoveTool {
    pub fn new() -> Self {
        Self::default()
    }
}

fn move_output(ctx: &ToolContext, dx: i32, dy: i32) -> ToolOutput {
    if (dx, dy) == (0, 0) {
        return ToolOutput::default();
    }
    let sprite = ctx.sprite;
    if sprite.layer(ctx.layer).is_none_or(|l| l.locked) {
        return ToolOutput::default();
    }
    if sprite
        .layer(ctx.layer)
        .is_some_and(|l| l.kind == cs_core::LayerKind::Instance)
    {
        let moved = cs_commands::ops::update_instance_key(sprite, ctx.layer, ctx.frame, |k| {
            k.position = (k.position.0 + dx, k.position.1 + dy);
        });
        return ToolOutput {
            command: moved,
            ..Default::default()
        };
    }
    match ctx.selection {
        Some(mask) => {
            let transparent = sprite.color_mode.transparent();
            let mut pixels: Vec<_> = mask.points().map(|(x, y)| (x, y, transparent)).collect();
            pixels.extend(mask.points().filter_map(|(x, y)| {
                let v = sprite.pixel(ctx.layer, ctx.frame, x, y);
                (v != transparent).then_some((x + dx, y + dy, v))
            }));
            ToolOutput {
                command: Some(Command::DrawPixels {
                    layer: ctx.layer,
                    frame: ctx.frame,
                    pixels,
                }),
                selection: Some(Some(mask.translated(dx, dy))),
                ..Default::default()
            }
        }
        None => match sprite.cel(ctx.layer, ctx.frame) {
            Some(cel) => ToolOutput::command(Command::SetCel {
                layer: ctx.layer,
                frame: ctx.frame,
                cel: Some(Cel {
                    position: (cel.position.0 + dx, cel.position.1 + dy),
                    ..*cel
                }),
            }),
            None => ToolOutput::default(),
        },
    }
}

impl Tool for MoveTool {
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
        match self.drag.get() {
            Some((a, b)) => move_output(ctx, b.0 - a.0, b.1 - a.1),
            None => ToolOutput::default(),
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
    use cs_core::{
        ArtboardId, Frame, InstanceKey, InstanceTrack, LayerId, LayerKind, Mask, Sprite,
    };

    fn with_dot() -> Sprite {
        let mut s = sprite();
        Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red())],
        }
        .apply(&mut s)
        .unwrap();
        s
    }

    fn gesture(c: &ToolContext, a: Point, b: Point) -> ToolOutput {
        let mut t = MoveTool::new();
        t.press(c, a);
        t.drag(c, b);
        t.release(c)
    }

    #[test]
    fn moves_instance_key_creating_one_on_the_current_frame() {
        let mut s = sprite();
        Command::AddFrame {
            index: 1,
            frame: Frame { duration_ms: 100 },
        }
        .apply(&mut s)
        .unwrap();
        let mut layer = cs_core::Layer::new_image(LayerId(2), "i");
        layer.kind = LayerKind::Instance;
        layer.instance = Some(InstanceTrack {
            keys: vec![InstanceKey::new(0, ArtboardId(2), (1, 1))],
        });
        s.layers.push(layer);
        let out = {
            let mut c = ctx(&s);
            c.layer = LayerId(2);
            c.frame = 1;
            gesture(&c, (0, 0), (2, 3))
        };
        out.command.unwrap().apply(&mut s).unwrap();
        let track = s.layer(LayerId(2)).unwrap().instance.clone().unwrap();
        assert_eq!(track.key_at(0).unwrap().position, (1, 1));
        assert_eq!(track.key_at(1).unwrap().position, (3, 4));
    }

    #[test]
    fn moves_selected_pixels_and_selection() {
        let mut s = with_dot();
        let mask = Mask::from_rect(8, 8, (0, 0), (2, 2));
        let out = {
            let mut c = ctx(&s);
            c.selection = Some(&mask);
            gesture(&c, (1, 1), (4, 3))
        };
        let inverse = out.command.unwrap().apply(&mut s).unwrap();
        let t = s.color_mode.transparent();
        assert_eq!(s.pixel(LayerId(1), 0, 1, 1), t);
        assert_eq!(s.pixel(LayerId(1), 0, 4, 3), red());
        assert_eq!(
            out.selection.unwrap().unwrap().bounds(),
            Some(((3, 2), (5, 4)))
        );
        inverse.apply(&mut s).unwrap();
        assert_eq!(s, with_dot());
    }

    #[test]
    fn without_selection_moves_cel() {
        let mut s = with_dot();
        let out = gesture(&ctx(&s), (0, 0), (2, 0));
        out.command.unwrap().apply(&mut s).unwrap();
        assert_eq!(s.cel(LayerId(1), 0).unwrap().position, (2, 0));
        assert_eq!(s.pixel(LayerId(1), 0, 3, 1), red());
    }

    #[test]
    fn zero_offset_is_noop() {
        let s = with_dot();
        assert_eq!(gesture(&ctx(&s), (1, 1), (1, 1)), ToolOutput::default());
    }
}
