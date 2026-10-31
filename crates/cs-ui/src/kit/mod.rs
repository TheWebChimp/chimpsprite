//! El UI kit: los componentes estándar de Pixel UI, cada uno con las medidas del mockup del autor
//! (`.design/Design.aseprite`, capa Mockup, frame 9). Se agregan de a uno; la pestaña UI Catalog
//! ([`catalog`]) muestra cada componente en todos sus estados.

pub mod button;
pub mod catalog;
pub mod check;
pub mod color_button;
pub mod group;
pub mod input;
pub mod slider;
pub mod tabs;

pub use group::{GroupItem, button_group};
