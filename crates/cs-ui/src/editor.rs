use cs_commands::{Command, History};
use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};
use cs_tools::{Eraser, Pencil, Point, Tool, ToolContext};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pencil,
    Eraser,
}

/// Estado del documento abierto y de la edición. No sabe nada de egui.
pub struct Editor {
    pub sprite: Sprite,
    pub history: History,
    pub layer: LayerId,
    pub frame: usize,
    pub primary: PixelValue,
    pub secondary: PixelValue,
    pub tool: ToolKind,
    pub brush_size: u32,
    pub pixel_perfect: bool,
    /// Aumenta cada vez que el documento cambia; la UI recompone cuando difiere.
    pub revision: u64,
    pencil: Pencil,
    eraser: Eraser,
    painting_secondary: bool,
}

impl Editor {
    pub fn new(sprite: Sprite) -> Self {
        let layer = sprite.layers.last().map(|l| l.id).unwrap_or(LayerId(1));
        let eraser = Eraser::new(sprite.color_mode.transparent());
        let mut editor = Self {
            primary: PixelValue::Index(0),
            secondary: PixelValue::Index(0),
            sprite,
            history: History::new(),
            layer,
            frame: 0,
            tool: ToolKind::Pencil,
            brush_size: 1,
            pixel_perfect: true,
            revision: 0,
            pencil: Pencil::new(),
            eraser,
            painting_secondary: false,
        };
        editor.primary = editor.palette_value(0);
        editor.secondary = editor.palette_value(21);
        editor
    }

    /// Valor de píxel para la entrada `index` de la paleta según el modo de color.
    pub fn palette_value(&self, index: u8) -> PixelValue {
        match self.sprite.color_mode {
            ColorMode::Indexed { .. } => PixelValue::Index(index),
            ColorMode::Rgba => PixelValue::Rgba(self.sprite.palette.get(index).unwrap_or_default()),
        }
    }

    pub fn resolve(&self, value: PixelValue) -> Rgba {
        cs_render::resolve(&self.sprite, value)
    }

    pub fn execute(&mut self, cmd: Command) {
        match self.history.execute(&mut self.sprite, cmd) {
            Ok(()) => self.revision += 1,
            Err(e) => log::warn!("command rejected: {e}"),
        }
    }

    pub fn undo(&mut self) {
        match self.history.undo(&mut self.sprite) {
            Ok(true) => self.revision += 1,
            Ok(false) => {}
            Err(e) => log::error!("undo failed: {e}"),
        }
    }

    pub fn redo(&mut self) {
        match self.history.redo(&mut self.sprite) {
            Ok(true) => self.revision += 1,
            Ok(false) => {}
            Err(e) => log::error!("redo failed: {e}"),
        }
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
    }

    pub fn tool_context(&self) -> ToolContext {
        ToolContext {
            layer: self.layer,
            frame: self.frame,
            color: if self.painting_secondary {
                self.secondary
            } else {
                self.primary
            },
            brush_size: self.brush_size,
            pixel_perfect: self.pixel_perfect,
        }
    }

    fn tool_mut(&mut self) -> &mut dyn Tool {
        match self.tool {
            ToolKind::Pencil => &mut self.pencil,
            ToolKind::Eraser => &mut self.eraser,
        }
    }

    fn tool_ref(&self) -> &dyn Tool {
        match self.tool {
            ToolKind::Pencil => &self.pencil,
            ToolKind::Eraser => &self.eraser,
        }
    }

    pub fn is_painting(&self) -> bool {
        self.tool_ref().is_active()
    }

    pub fn pointer_press(&mut self, p: Point, secondary: bool) {
        self.painting_secondary = secondary;
        let ctx = self.tool_context();
        self.tool_mut().press(&ctx, p);
    }

    pub fn pointer_drag(&mut self, p: Point) {
        let ctx = self.tool_context();
        self.tool_mut().drag(&ctx, p);
    }

    pub fn pointer_release(&mut self) {
        let ctx = self.tool_context();
        if let Some(cmd) = self.tool_mut().release(&ctx) {
            self.execute(cmd);
        }
    }

    pub fn preview(&self) -> Vec<(i32, i32, PixelValue)> {
        self.tool_ref().preview(&self.tool_context())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execute_undo_redo_bump_revision() {
        let mut e = Editor::new(Sprite::new(4, 4, ColorMode::Rgba));
        let r0 = e.revision;
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, e.primary)],
        });
        assert!(e.revision > r0 && e.history.can_undo());
        e.undo();
        assert!(e.sprite.cels.is_empty());
        e.redo();
        assert!(!e.sprite.cels.is_empty());
    }

    #[test]
    fn stroke_with_secondary_button_uses_secondary_color() {
        let mut e = Editor::new(Sprite::new(4, 4, ColorMode::Rgba));
        e.pointer_press((1, 1), true);
        e.pointer_release();
        let cel = *e.sprite.cel(LayerId(1), 0).unwrap();
        let img = e.sprite.image(cel.image).unwrap();
        assert_eq!(img.get(1, 1), Some(e.secondary));
    }

    #[test]
    fn indexed_sprite_uses_palette_indices() {
        let e = Editor::new(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        assert_eq!(e.primary, PixelValue::Index(0));
        assert_eq!(e.secondary, PixelValue::Index(21));
    }
}
