//! Núcleo de los modelos 3D de pocos polígonos: primitivas y sus UVs automáticos, y el
//! rasterizador pixel que los pinta.

mod primitives;
mod ramp;
mod raster;

pub use primitives::*;
pub use ramp::*;
pub use raster::*;
