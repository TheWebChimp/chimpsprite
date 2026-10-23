//! Herramientas como máquinas de estado: eventos de puntero → comandos.

mod artboard;
mod blur;
mod bucket;
mod contour;
mod eyedropper;
mod gradient;
mod jumble;
mod move_tool;
mod pencil;
mod rng;
mod select;
mod shape;
mod slice;
mod spray;
pub mod text;

pub use artboard::{ArtboardHit, ArtboardTool, Edges, hit_test};
pub use blur::Blur;
pub use bucket::Bucket;
pub use contour::Contour;
pub use cs_raster::Point;
pub use cs_raster::{BrushShape, Dither, GradientShape};
pub use eyedropper::Eyedropper;
pub use gradient::{GradientTool, gradient_pixels, gradient_value};
pub use jumble::Jumble;
pub use move_tool::MoveTool;
pub use pencil::{Eraser, Pencil};
pub use select::{Lasso, MagicWand, Marquee, MarqueeShape, PolygonLasso};
pub use shape::{ShapeKind, ShapeTool};
pub use slice::{SLICE_COLOR, SliceTool};
pub use spray::Spray;

use cs_commands::Command;
use cs_core::{LayerId, Mask, PixelValue, Sprite};

pub struct ToolContext<'a> {
    pub sprite: &'a Sprite,
    pub layer: LayerId,
    pub frame: usize,
    pub color: PixelValue,
    /// El otro color (destino del degradado).
    pub secondary: PixelValue,
    pub brush_size: u32,
    /// Forma del pincel: Pencil, Eraser, Spray, Jumble, Blur y el contorno de Line, Rectangle y
    /// Ellipse.
    pub brush_shape: BrushShape,
    pub pixel_perfect: bool,
    /// Formas rellenas (rectángulo/elipse).
    pub filled: bool,
    /// Radio de las esquinas del rectángulo (0 = rectas).
    pub corner_radius: u32,
    /// Cubeta: solo la región conectada (si no, todos los píxeles del mismo color).
    pub contiguous: bool,
    /// Shift: líneas a 45°, cuadrados y círculos.
    pub constrain: bool,
    pub selection: Option<&'a Mask>,
    /// Varita: diferencia máxima por canal (0–255).
    pub tolerance: u8,
    /// Varita: comparar con la composición visible en vez de la capa actual.
    pub sample_all: bool,
    /// Spray: diámetro del área y puntos por segundo / 6.
    pub spray_width: u32,
    pub spray_speed: u32,
    pub gradient_shape: GradientShape,
    pub dither: Dither,
    /// Slice seleccionado (índice en `sprite.slices`) para moverlo o redimensionarlo.
    pub selected_slice: Option<usize>,
    /// Modo Tiles: `sprite` es el proxy de celdas del editor (cada píxel codifica un tile). El
    /// cuentagotas y la varita leen el valor exacto de la capa en vez del color visible.
    pub tiles: bool,
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
    /// Hay un gesto en curso con un botón presionado.
    fn is_active(&self) -> bool;
    /// El puntero se mueve sin botones (p. ej. la línea elástica del lazo poligonal).
    fn hover(&mut self, _ctx: &ToolContext, _p: Point) {}
    /// Pasa el tiempo con el botón presionado (el spray sigue rociando).
    fn tick(&mut self, _ctx: &ToolContext, _dt_ms: f32) {}
    /// Gesto de varios clics en curso entre un clic y el siguiente.
    fn is_pending(&self) -> bool {
        false
    }
    /// Termina un gesto de varios clics (Enter o doble clic).
    fn finish(&mut self, _ctx: &ToolContext) -> ToolOutput {
        ToolOutput::default()
    }
    /// Descarta el gesto en curso (Esc).
    fn cancel(&mut self) {}
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

    use crate::{BrushShape, ToolContext};

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
            secondary: PixelValue::Rgba(Rgba::rgb(0, 0, 255)),
            brush_size: 1,
            // Los tests de antes se escribieron con el pincel cuadrado.
            brush_shape: BrushShape::Square,
            pixel_perfect: false,
            filled: false,
            corner_radius: 0,
            contiguous: true,
            constrain: false,
            selection: None,
            tolerance: 0,
            sample_all: false,
            spray_width: 8,
            spray_speed: 32,
            gradient_shape: Default::default(),
            dither: Default::default(),
            selected_slice: None,
            tiles: false,
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
