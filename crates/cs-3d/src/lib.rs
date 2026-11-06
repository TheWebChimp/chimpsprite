//! Núcleo de los modelos 3D de pocos polígonos: primitivas y sus UVs automáticos, y el
//! rasterizador pixel que los pinta.

mod edit;
mod gizmo;
mod primitives;
mod ramp;
mod raster;
mod topology;
mod uv;
mod wire;

pub use edit::*;
pub use gizmo::*;
pub use primitives::*;
pub use ramp::*;
pub use raster::*;
pub use topology::*;
pub use uv::*;
pub use wire::*;
