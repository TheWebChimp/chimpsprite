//! El UI kit: los componentes estándar de Pixel UI, cada uno con las medidas del mockup del autor
//! (`.design/Design.aseprite`, capa Mockup, frame 9). Se agregan de a uno; la pestaña UI Catalog
//! ([`catalog`]) muestra cada componente en todos sus estados.

pub mod button;
pub mod catalog;
pub mod check;
pub mod color_button;
pub mod group;
pub mod icon;
pub mod input;
pub mod select;
pub mod slider;
pub mod slot;
pub mod stepper;
pub mod tabs;
pub mod title_bar;

pub use group::{GroupItem, button_group};
