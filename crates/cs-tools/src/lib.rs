//! Herramientas como máquinas de estado: eventos de puntero → comandos.

mod bucket;
mod eyedropper;
mod move_tool;
mod pencil;
mod select;
mod shape;

pub use bucket::Bucket;
pub use cs_raster::Point;
pub use eyedropper::Eyedropper;
pub use move_tool::MoveTool;
pub use pencil::{Eraser, Pencil};
pub use select::RectSelect;
pub use shape::{ShapeKind, ShapeTool};

use cs_commands::Command;
use cs_core::{LayerId, Mask, PixelValue, Sprite};

pub struct ToolContext<'a> {
    pub sprite: &'a Sprite,
    pub layer: LayerId,
    pub frame: usize,
    pub color: PixelValue,
    pub brush_size: u32,
    pub pixel_perfect: bool,
    /// Formas rellenas (rectángulo/elipse).
    pub filled: bool,
    /// Cubeta: solo la región conectada (si no, todos los píxeles del mismo color).
    pub contiguous: bool,
    /// Shift: líneas a 45°, cuadrados y círculos.
    pub constrain: bool,
    pub selection: Option<&'a Mask>,
}

impl ToolContext<'_> {
    pub fn in_canvas(&self, p: Point) -> bool {
        p.0 >= 0 && p.1 >= 0 && p.0 < self.sprite.width as i32 && p.1 < self.sprite.height as i32
    }

    fn draw(&self, points: impl IntoIterator<Item = Point>, color: PixelValue) -> Command {
        Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels: points.into_iter().map(|(x, y)| (x, y, color)).collect(),
        }
    }
}

/// Resultado de un gesto. Cada campo es opcional e independiente.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolOutput {
    pub command: Option<Command>,
    /// Color elegido con el cuentagotas.
    pub pick: Option<PixelValue>,
    /// Cambio de selección: `Some(None)` deselecciona.
    pub selection: Option<Option<Mask>>,
}

impl ToolOutput {
    pub fn command(cmd: Command) -> Self {
        Self {
            command: Some(cmd),
            ..Default::default()
        }
    }
}

pub trait Tool {
    fn press(&mut self, ctx: &ToolContext, p: Point);
    fn drag(&mut self, ctx: &ToolContext, p: Point);
    /// Termina el gesto.
    fn release(&mut self, ctx: &ToolContext) -> ToolOutput;
    /// Lo que pasaría si el gesto terminara ahora (para la vista previa).
    fn preview(&self, ctx: &ToolContext) -> ToolOutput;
    fn is_active(&self) -> bool;
}

/// Estado común de las herramientas de "arrastrar de A a B".
#[derive(Debug, Default)]
pub(crate) struct Drag {
    start: Option<Point>,
    end: Point,
}

impl Drag {
    fn press(&mut self, p: Point) {
        self.start = Some(p);
        self.end = p;
    }

    fn drag(&mut self, p: Point) {
        if self.start.is_some() {
            self.end = p;
        }
    }

    fn get(&self) -> Option<(Point, Point)> {
        self.start.map(|s| (s, self.end))
    }

    fn take(&mut self) -> Option<(Point, Point)> {
        let r = self.get();
        self.start = None;
        r
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};

    use crate::ToolContext;

    pub fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    pub fn sprite() -> Sprite {
        Sprite::new(8, 8, ColorMode::Rgba)
    }

    pub fn ctx(sprite: &Sprite) -> ToolContext<'_> {
        ToolContext {
            sprite,
            layer: LayerId(1),
            frame: 0,
            color: red(),
            brush_size: 1,
            pixel_perfect: false,
            filled: false,
            contiguous: true,
            constrain: false,
            selection: None,
        }
    }

    pub fn coords(out: &crate::ToolOutput) -> Vec<(i32, i32)> {
        match &out.command {
            Some(cs_commands::Command::DrawPixels { pixels, .. }) => {
                pixels.iter().map(|&(x, y, _)| (x, y)).collect()
            }
            other => panic!("expected DrawPixels, got {other:?}"),
        }
    }
}
