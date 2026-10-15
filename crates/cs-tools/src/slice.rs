//! Herramienta Slice: crear rectángulos con nombre (hitboxes, pivotes) y moverlos o
//! redimensionarlos por frame.

use cs_commands::Command;
use cs_core::{Rgba, Slice, SliceKey};
use cs_raster::Point;

use crate::{Tool, ToolContext, ToolOutput};

/// Color de los slices nuevos (el azul de Aseprite).
pub const SLICE_COLOR: Rgba = Rgba::rgb(0, 0, 255);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Create,
    Move,
    /// Bordes a mover: izquierda, arriba, derecha, abajo.
    Resize(bool, bool, bool, bool),
}

#[derive(Debug, Default)]
pub struct SliceTool {
    gesture: Option<(Mode, Point, Point)>,
}

impl SliceTool {
    pub fn new() -> Self {
        Self::default()
    }

    fn selected_bounds(ctx: &ToolContext) -> Option<(i32, i32, u32, u32)> {
        let slice = ctx.sprite.slices.get(ctx.selected_slice?)?;
        slice.key_at(ctx.frame).map(|k| k.bounds)
    }

    /// Rectángulo resultante `(x0, y0, x1, y1)` inclusivo.
    fn result(&self, ctx: &ToolContext) -> Option<(Mode, (i32, i32, i32, i32))> {
        let (mode, a, b) = self.gesture?;
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let rect = match mode {
            Mode::Create => (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)),
            Mode::Move | Mode::Resize(..) => {
                let (x, y, w, h) = Self::selected_bounds(ctx)?;
                let (mut x0, mut y0, mut x1, mut y1) = (x, y, x + w as i32 - 1, y + h as i32 - 1);
                match mode {
                    Mode::Move => {
                        x0 += dx;
                        x1 += dx;
                        y0 += dy;
                        y1 += dy;
                    }
                    Mode::Resize(l, t, r, btm) => {
                        if l {
                            x0 = (x0 + dx).min(x1);
                        }
                        if t {
                            y0 = (y0 + dy).min(y1);
                        }
                        if r {
                            x1 = (x1 + dx).max(x0);
                        }
                        if btm {
                            y1 = (y1 + dy).max(y0);
                        }
                    }
                    Mode::Create => unreachable!(),
                }
                (x0, y0, x1, y1)
            }
        };
        Some((mode, rect))
    }
}

impl Tool for SliceTool {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        let mode = match Self::selected_bounds(ctx) {
            Some((x, y, w, h)) => {
                let (x1, y1) = (x + w as i32 - 1, y + h as i32 - 1);
                let inside = p.0 >= x && p.1 >= y && p.0 <= x1 && p.1 <= y1;
                let (l, t, r, b) = (p.0 == x, p.1 == y, p.0 == x1, p.1 == y1);
                if !inside {
                    Mode::Create
                } else if w > 2 && h > 2 && (l || t || r || b) {
                    Mode::Resize(l, t, r, b)
                } else {
                    Mode::Move
                }
            }
            None => Mode::Create,
        };
        self.gesture = Some((mode, p, p));
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        if let Some((_, _, end)) = &mut self.gesture {
            *end = p;
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> ToolOutput {
        let out = self.preview(ctx);
        self.gesture = None;
        out
    }

    fn preview(&self, ctx: &ToolContext) -> ToolOutput {
        let Some((mode, (x0, y0, x1, y1))) = self.result(ctx) else {
            return ToolOutput::default();
        };
        let (a, b) = self.gesture.map(|g| (g.1, g.2)).expect("active");
        let bounds = (x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        let mut slices = ctx.sprite.slices.clone();
        match mode {
            // Un clic sin arrastrar no crea nada.
            Mode::Create if a == b => return ToolOutput::default(),
            Mode::Create => {
                let n = (1..)
                    .find(|n| !slices.iter().any(|s| s.name == format!("Slice {n}")))
                    .expect("free name");
                slices.push(Slice {
                    name: format!("Slice {n}"),
                    color: SLICE_COLOR,
                    text: String::new(),
                    keys: vec![SliceKey {
                        frame: 0,
                        bounds,
                        center: None,
                        pivot: None,
                    }],
                });
            }
            Mode::Move | Mode::Resize(..) => {
                if a == b {
                    return ToolOutput::default();
                }
                let index = ctx.selected_slice.expect("editing the selected slice");
                let slice = &mut slices[index];
                let current = *slice.key_at(ctx.frame).expect("visible key");
                slice.set_key(SliceKey {
                    frame: ctx.frame,
                    bounds,
                    ..current
                });
            }
        }
        ToolOutput::command(Command::SetSlices(slices))
    }

    fn is_active(&self) -> bool {
        self.gesture.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::*;

    fn slices(out: ToolOutput) -> Vec<Slice> {
        match out.command {
            Some(Command::SetSlices(s)) => s,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn create_then_move_and_resize_per_frame() {
        let mut s = sprite();
        s.frames.push(cs_core::Frame { duration_ms: 100 });
        let mut t = SliceTool::new();
        {
            let c = ctx(&s);
            t.press(&c, (1, 1));
            t.drag(&c, (4, 3));
            let created = slices(t.release(&c));
            assert_eq!(created[0].name, "Slice 1");
            assert_eq!(created[0].keys[0].bounds, (1, 1, 4, 3));
            s.slices = created;
        }
        // Mover en el frame 1 agrega una clave ahí; el frame 0 queda igual.
        let mut c = ctx(&s);
        c.selected_slice = Some(0);
        c.frame = 1;
        t.press(&c, (2, 2));
        t.drag(&c, (4, 4));
        let moved = slices(t.release(&c));
        assert_eq!(moved[0].key_at(0).unwrap().bounds, (1, 1, 4, 3));
        assert_eq!(moved[0].key_at(1).unwrap().bounds, (3, 3, 4, 3));
        // Arrastrar el borde derecho agranda.
        t.press(&c, (4, 2));
        t.drag(&c, (6, 2));
        assert_eq!(
            slices(t.release(&c))[0].key_at(1).unwrap().bounds,
            (1, 1, 6, 3)
        );
        // Un clic fuera sin arrastrar no hace nada.
        t.press(&c, (7, 7));
        assert_eq!(t.release(&c), ToolOutput::default());
    }
}
