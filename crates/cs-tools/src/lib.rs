//! Herramientas como máquinas de estado: eventos de puntero → comandos.

mod pencil;

pub use cs_raster::Point;
pub use pencil::{Eraser, Pencil};

use cs_commands::Command;
use cs_core::{LayerId, PixelValue};

#[derive(Clone, Copy, Debug)]
pub struct ToolContext {
    pub layer: LayerId,
    pub frame: usize,
    pub color: PixelValue,
    pub brush_size: u32,
    pub pixel_perfect: bool,
}

pub trait Tool {
    fn press(&mut self, ctx: &ToolContext, p: Point);
    fn drag(&mut self, ctx: &ToolContext, p: Point);
    /// Termina el gesto y devuelve el comando a ejecutar, si hay uno.
    fn release(&mut self, ctx: &ToolContext) -> Option<Command>;
    /// Píxeles a mostrar como vista previa mientras el gesto está activo.
    fn preview(&self, ctx: &ToolContext) -> Vec<(i32, i32, PixelValue)>;
    fn is_active(&self) -> bool;
}
