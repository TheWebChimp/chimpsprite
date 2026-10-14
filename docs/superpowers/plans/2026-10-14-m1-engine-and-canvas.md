# M1 — Motor + lienzo pintable — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Workspace de ChimpSprite con modelo de documento, rasterizado, comandos con undo/redo, compositor CPU, herramientas lápiz/borrador y una app egui donde se puede pintar con una paleta.

**Architecture:** Workspace Cargo por capas (`cs-core` → `cs-raster` → `cs-commands` → `cs-render` → `cs-tools` → `cs-ui` → `apps/chimpsprite`). Toda mutación pasa por `Command` (enum serializable) cuya aplicación devuelve su comando inverso; `History` apila inversos. La UI compone en CPU y sube una textura `Nearest`.

**Tech Stack:** Rust 1.95 (edition 2024), serde 1, thiserror 2, log 0.4, proptest 1.12 (dev), eframe/egui 0.36, png 0.18.

Spec: `docs/superpowers/specs/2026-10-14-chimpsprite-core-design.md`

Hitos siguientes (planes separados): M2 herramientas restantes + selección; M3 capas/timeline/animación/onion skin; M4 `.aseprite` + export.

## Global Constraints

- Licencia: MIT OR Apache-2.0 (dual), declarada en `workspace.package`.
- `cs-core`, `cs-raster`, `cs-commands`, `cs-render`, `cs-tools` no dependen de egui/eframe/wgpu.
- Toda mutación del documento pasa por `Command`. UI y herramientas nunca mutan `Sprite` directo.
- Comandos serializables (serde).
- Logging con `log`; nada de `println!` fuera de tests.
- Un comando que devuelve `CmdError` no modifica el documento.
- Desviaciones de diseño respecto al spec (decididas en este plan): `Command` es un `enum` (no trait object, para que sea serializable) y `apply` devuelve el comando inverso; los cels vinculados se representan como cels que comparten el mismo `ImageId` (en vez de `CelContent::Linked`).

---

### Task 1: Workspace + `cs-core`

**Files:**
- Create: `Cargo.toml`, `LICENSE-MIT`, `LICENSE-APACHE`, `README.md`
- Create: `crates/cs-core/Cargo.toml`, `crates/cs-core/src/lib.rs`, `crates/cs-core/src/color.rs`, `crates/cs-core/src/image.rs`, `crates/cs-core/src/sprite.rs`

**Interfaces:**
- Produces: `Rgba { r, g, b, a }` (+ `Rgba::new`, `Rgba::rgb`, `Rgba::TRANSPARENT`), `ColorMode::{Rgba, Indexed { transparent_index }}`, `ColorMode::transparent() -> PixelValue`, `PixelValue::{Rgba(Rgba), Index(u8)}`, `Palette::{new, colors, get, len, is_empty, db32}`, `PixelBuffer::{Rgba(Vec<Rgba>), Indexed(Vec<u8>)}`, `Image::{new(w, h, ColorMode), width, height, pixels, get(x, y) -> Option<PixelValue>, set(x, y, PixelValue) -> Result<(), ImageError>}`, `LayerId(u32)`, `ImageId(u32)`, `BlendMode`, `LayerKind::{Image, Group}`, `Layer { id, name, kind, parent, visible, locked, opacity, blend }` + `Layer::new_image(id, name)`, `Frame { duration_ms }`, `Cel { position, opacity, image }`, `Sprite { width, height, color_mode, palette, layers, frames, cels: BTreeMap<(LayerId, usize), Cel>, images: BTreeMap<ImageId, Arc<Image>> }` + `Sprite::{new, layer, layer_mut, layer_index, next_layer_id, next_image_id, cel, image}`.

- [ ] **Step 1: Workspace files**

`Cargo.toml`:
```toml
[workspace]
resolver = "3"
members = ["crates/*", "apps/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT OR Apache-2.0"
repository = "https://github.com/TheWebChimp/chimpsprite"

[workspace.dependencies]
serde = { version = "1", features = ["derive", "rc"] }
serde_json = "1"
thiserror = "2"
log = "0.4"
proptest = "1.12"
png = "0.18"
eframe = "0.36"
env_logger = "0.11"
cs-core = { path = "crates/cs-core" }
cs-raster = { path = "crates/cs-raster" }
cs-commands = { path = "crates/cs-commands" }
cs-render = { path = "crates/cs-render" }
cs-tools = { path = "crates/cs-tools" }
cs-ui = { path = "crates/cs-ui" }
```

Licencias: texto estándar MIT (copyright "2026 TheWebChimp and ChimpSprite contributors") y Apache-2.0 completo. README breve en español con descripción, estado (pre-alpha), cómo correr (`cargo run -p chimpsprite`), licencia.

`crates/cs-core/Cargo.toml`:
```toml
[package]
name = "cs-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
serde.workspace = true
thiserror.workspace = true
```

- [ ] **Step 2: Write failing tests** (en `crates/cs-core/src/sprite.rs` y `image.rs`, módulos `#[cfg(test)]`)

```rust
// image.rs tests
#[test]
fn new_rgba_image_is_transparent() {
    let img = Image::new(2, 2, ColorMode::Rgba);
    assert_eq!(img.get(1, 1), Some(PixelValue::Rgba(Rgba::TRANSPARENT)));
}
#[test]
fn new_indexed_image_uses_transparent_index() {
    let img = Image::new(2, 2, ColorMode::Indexed { transparent_index: 3 });
    assert_eq!(img.get(0, 0), Some(PixelValue::Index(3)));
}
#[test]
fn set_get_roundtrip_and_errors() {
    let mut img = Image::new(2, 2, ColorMode::Rgba);
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    img.set(1, 0, red).unwrap();
    assert_eq!(img.get(1, 0), Some(red));
    assert_eq!(img.set(2, 0, red), Err(ImageError::OutOfBounds { x: 2, y: 0 }));
    assert_eq!(img.set(0, 0, PixelValue::Index(1)), Err(ImageError::ModeMismatch));
    assert_eq!(img.get(5, 5), None);
}

// sprite.rs tests
#[test]
fn new_sprite_has_one_layer_one_frame_no_cels() {
    let s = Sprite::new(16, 8, ColorMode::Rgba);
    assert_eq!((s.width, s.height), (16, 8));
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.layers[0].name, "Layer 1");
    assert_eq!(s.frames, vec![Frame { duration_ms: 100 }]);
    assert!(s.cels.is_empty());
    assert_eq!(s.palette.len(), 32);
}
#[test]
fn id_allocation_is_max_plus_one() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    assert_eq!(s.next_layer_id(), LayerId(2));
    assert_eq!(s.next_image_id(), ImageId(1));
    s.images.insert(ImageId(7), std::sync::Arc::new(Image::new(1, 1, ColorMode::Rgba)));
    assert_eq!(s.next_image_id(), ImageId(8));
}
```

- [ ] **Step 3: Run tests, expect compile failure** — `cargo test -p cs-core`

- [ ] **Step 4: Implement**

`crates/cs-core/src/lib.rs`:
```rust
//! Modelo de documento de ChimpSprite: datos puros, sin UI.

mod color;
mod image;
mod sprite;

pub use color::*;
pub use image::*;
pub use sprite::*;
```

`crates/cs-core/src/color.rs`:
```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Rgba = Rgba::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    pub const fn from_hex(hex: u32) -> Self {
        Self::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    Rgba,
    Indexed { transparent_index: u8 },
}

impl ColorMode {
    /// Valor de píxel transparente en este modo.
    pub fn transparent(self) -> PixelValue {
        match self {
            ColorMode::Rgba => PixelValue::Rgba(Rgba::TRANSPARENT),
            ColorMode::Indexed { transparent_index } => PixelValue::Index(transparent_index),
        }
    }

    pub fn accepts(self, value: PixelValue) -> bool {
        matches!(
            (self, value),
            (ColorMode::Rgba, PixelValue::Rgba(_)) | (ColorMode::Indexed { .. }, PixelValue::Index(_))
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelValue {
    Rgba(Rgba),
    Index(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Palette {
    colors: Vec<Rgba>,
}

/// DawnBringer 32.
const DB32: [u32; 32] = [
    0x000000, 0x222034, 0x45283c, 0x663931, 0x8f563b, 0xdf7126, 0xd9a066, 0xeec39a,
    0xfbf236, 0x99e550, 0x6abe30, 0x37946e, 0x4b692f, 0x524b24, 0x323c39, 0x3f3f74,
    0x306082, 0x5b6ee1, 0x639bff, 0x5fcde4, 0xcbdbfc, 0xffffff, 0x9badb7, 0x847e87,
    0x696a6a, 0x595652, 0x76428a, 0xac3232, 0xd95763, 0xd77bba, 0x8f974a, 0x8a6f30,
];

impl Palette {
    pub fn new(colors: Vec<Rgba>) -> Self {
        Self { colors }
    }

    pub fn db32() -> Self {
        Self::new(DB32.iter().map(|&h| Rgba::from_hex(h)).collect())
    }

    pub fn colors(&self) -> &[Rgba] {
        &self.colors
    }

    pub fn get(&self, index: u8) -> Option<Rgba> {
        self.colors.get(index as usize).copied()
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }
}
```

`crates/cs-core/src/image.rs`:
```rust
use serde::{Deserialize, Serialize};

use crate::{ColorMode, PixelValue, Rgba};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelBuffer {
    Rgba(Vec<Rgba>),
    Indexed(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: PixelBuffer,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ImageError {
    #[error("pixel ({x}, {y}) out of bounds")]
    OutOfBounds { x: u32, y: u32 },
    #[error("pixel value does not match the image color mode")]
    ModeMismatch,
}

impl Image {
    /// Imagen llena de píxeles transparentes.
    pub fn new(width: u32, height: u32, mode: ColorMode) -> Self {
        let n = (width * height) as usize;
        let pixels = match mode.transparent() {
            PixelValue::Rgba(c) => PixelBuffer::Rgba(vec![c; n]),
            PixelValue::Index(i) => PixelBuffer::Indexed(vec![i; n]),
        };
        Self { width, height, pixels }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &PixelBuffer {
        &self.pixels
    }

    fn offset(&self, x: u32, y: u32) -> Option<usize> {
        (x < self.width && y < self.height).then(|| (y * self.width + x) as usize)
    }

    pub fn get(&self, x: u32, y: u32) -> Option<PixelValue> {
        let i = self.offset(x, y)?;
        Some(match &self.pixels {
            PixelBuffer::Rgba(p) => PixelValue::Rgba(p[i]),
            PixelBuffer::Indexed(p) => PixelValue::Index(p[i]),
        })
    }

    pub fn set(&mut self, x: u32, y: u32, value: PixelValue) -> Result<(), ImageError> {
        let i = self.offset(x, y).ok_or(ImageError::OutOfBounds { x, y })?;
        match (&mut self.pixels, value) {
            (PixelBuffer::Rgba(p), PixelValue::Rgba(c)) => p[i] = c,
            (PixelBuffer::Indexed(p), PixelValue::Index(c)) => p[i] = c,
            _ => return Err(ImageError::ModeMismatch),
        }
        Ok(())
    }
}
```

`crates/cs-core/src/sprite.rs`:
```rust
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, Palette};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImageId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Addition,
    Subtract,
    Difference,
    /// Modo leído de un archivo que no soportamos; se conserva y se renderiza como Normal.
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Image,
    Group,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub parent: Option<LayerId>,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend: BlendMode,
}

impl Layer {
    pub fn new_image(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Image,
            parent: None,
            visible: true,
            locked: false,
            opacity: 255,
            blend: BlendMode::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub duration_ms: u16,
}

/// Contenido de una capa en un frame. Dos cels con el mismo `image` están vinculados.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cel {
    pub position: (i32, i32),
    pub opacity: u8,
    pub image: ImageId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    pub palette: Palette,
    /// Orden de abajo hacia arriba; los grupos se expresan con `Layer::parent`.
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    pub cels: BTreeMap<(LayerId, usize), Cel>,
    pub images: BTreeMap<ImageId, Arc<Image>>,
}

impl Sprite {
    pub fn new(width: u32, height: u32, color_mode: ColorMode) -> Self {
        Self {
            width,
            height,
            color_mode,
            palette: Palette::db32(),
            layers: vec![Layer::new_image(LayerId(1), "Layer 1")],
            frames: vec![Frame { duration_ms: 100 }],
            cels: BTreeMap::new(),
            images: BTreeMap::new(),
        }
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn next_layer_id(&self) -> LayerId {
        LayerId(self.layers.iter().map(|l| l.id.0).max().unwrap_or(0) + 1)
    }

    pub fn next_image_id(&self) -> ImageId {
        ImageId(self.images.keys().map(|k| k.0).max().map_or(1, |m| m + 1))
    }

    pub fn cel(&self, layer: LayerId, frame: usize) -> Option<&Cel> {
        self.cels.get(&(layer, frame))
    }

    pub fn image(&self, id: ImageId) -> Option<&Arc<Image>> {
        self.images.get(&id)
    }
}
```

- [ ] **Step 5: Run tests, expect PASS** — `cargo test -p cs-core`
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat(core): workspace and document model"`

---

### Task 2: `cs-raster` — línea, pixel-perfect, pincel

**Files:**
- Create: `crates/cs-raster/Cargo.toml` (sin dependencias), `crates/cs-raster/src/lib.rs`

**Interfaces:**
- Produces: `pub type Point = (i32, i32)`, `line(a: Point, b: Point) -> Vec<Point>` (Bresenham, inclusivo), `pixel_perfect(path: &[Point]) -> Vec<Point>`, `brush_square(center: Point, size: u32) -> Vec<Point>`.

- [ ] **Step 1: Failing tests** (`lib.rs` `#[cfg(test)] mod tests`)

```rust
use super::*;

#[test]
fn line_single_point() {
    assert_eq!(line((2, 3), (2, 3)), vec![(2, 3)]);
}
#[test]
fn line_shallow() {
    assert_eq!(line((0, 0), (3, 1)), vec![(0, 0), (1, 0), (2, 1), (3, 1)]);
}
#[test]
fn line_reverse_and_vertical() {
    assert_eq!(line((0, 2), (0, 0)), vec![(0, 2), (0, 1), (0, 0)]);
    assert_eq!(line((2, 2), (0, 0)), vec![(2, 2), (1, 1), (0, 0)]);
}
#[test]
fn pixel_perfect_removes_l_corners() {
    assert_eq!(pixel_perfect(&[(0, 0), (1, 0), (1, 1)]), vec![(0, 0), (1, 1)]);
}
#[test]
fn pixel_perfect_keeps_straight_lines_and_dedupes() {
    assert_eq!(pixel_perfect(&[(0, 0), (1, 0), (1, 0), (2, 0)]), vec![(0, 0), (1, 0), (2, 0)]);
}
#[test]
fn brush_sizes() {
    assert_eq!(brush_square((5, 5), 1), vec![(5, 5)]);
    assert_eq!(brush_square((0, 0), 2), vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
    assert_eq!(brush_square((0, 0), 3).len(), 9);
    assert!(brush_square((0, 0), 3).contains(&(-1, -1)));
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p cs-raster`
- [ ] **Step 3: Implement**

```rust
//! Algoritmos de rasterizado puros sobre coordenadas enteras.

pub type Point = (i32, i32);

/// Línea de Bresenham de `a` a `b`, ambos incluidos.
pub fn line(a: Point, b: Point) -> Vec<Point> {
    let (mut x, mut y) = a;
    let (x1, y1) = b;
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::with_capacity((dx - dy + 1) as usize);
    loop {
        out.push((x, y));
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

/// Quita duplicados consecutivos y esquinas en "L" de un trazo a mano alzada.
pub fn pixel_perfect(path: &[Point]) -> Vec<Point> {
    let mut deduped: Vec<Point> = Vec::with_capacity(path.len());
    for &p in path {
        if deduped.last() != Some(&p) {
            deduped.push(p);
        }
    }
    let mut out: Vec<Point> = Vec::with_capacity(deduped.len());
    for (i, &c) in deduped.iter().enumerate() {
        if let (Some(&p), Some(&n)) = (out.last(), deduped.get(i + 1)) {
            let touches_prev = p.0 == c.0 || p.1 == c.1;
            let touches_next = n.0 == c.0 || n.1 == c.1;
            if touches_prev && touches_next && p.0 != n.0 && p.1 != n.1 {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Pincel cuadrado de lado `size` centrado en `center` (sesgo hacia abajo/derecha en tamaños pares).
pub fn brush_square(center: Point, size: u32) -> Vec<Point> {
    let size = size.max(1) as i32;
    let start = -(size - 1) / 2;
    let mut out = Vec::with_capacity((size * size) as usize);
    for dy in start..start + size {
        for dx in start..start + size {
            out.push((center.0 + dx, center.1 + dy));
        }
    }
    out
}
```

- [ ] **Step 4: Run, expect PASS** — `cargo test -p cs-raster`
- [ ] **Step 5: Commit** — `git commit -am "feat(raster): line, pixel-perfect and square brush"` (add new files first)

---

### Task 3: `cs-commands` — comandos, transacciones, historial

**Files:**
- Create: `crates/cs-commands/Cargo.toml` (deps: `cs-core`, `serde`, `thiserror`, `log`; dev: `proptest`, `serde_json`)
- Create: `crates/cs-commands/src/lib.rs`, `src/command.rs`, `src/draw.rs`, `src/history.rs`
- Test: `crates/cs-commands/tests/undo_roundtrip.rs`

**Interfaces:**
- Consumes: todo `cs-core`.
- Produces:
  - `enum Command { DrawPixels { layer: LayerId, frame: usize, pixels: Vec<(i32, i32, PixelValue)> }, SetCel { layer: LayerId, frame: usize, cel: Option<Cel> }, SetImage { id: ImageId, image: Option<Arc<Image>> }, AddLayer { index: usize, layer: Layer }, RemoveLayer { id: LayerId }, SetLayerProps { id: LayerId, props: LayerProps }, AddFrame { index: usize, frame: Frame }, RemoveFrame { index: usize }, Batch(Vec<Command>) }`
  - `struct LayerProps { name: Option<String>, visible: Option<bool>, locked: Option<bool>, opacity: Option<u8> }` (Default)
  - `Command::apply(&self, &mut Sprite) -> Result<Command, CmdError>` (devuelve el inverso)
  - `enum CmdError`
  - `History::{new, execute(&mut self, &mut Sprite, Command) -> Result<(), CmdError>, undo(&mut self, &mut Sprite) -> Result<bool, CmdError>, redo(...) -> Result<bool, CmdError>, can_undo, can_redo}`

- [ ] **Step 1: Failing unit tests** (en `command.rs` / `draw.rs`)

```rust
// draw.rs tests
use cs_core::*;
use crate::Command;

fn red() -> PixelValue { PixelValue::Rgba(Rgba::rgb(255, 0, 0)) }

#[test]
fn draw_creates_canvas_sized_cel_and_undo_removes_it() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let cmd = Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(1, 2, red()), (9, 9, red())] };
    let inv = cmd.apply(&mut s).unwrap();
    let cel = *s.cel(LayerId(1), 0).unwrap();
    let img = s.image(cel.image).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    assert_eq!(img.get(1, 2), Some(red()));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn draw_expands_small_cel() {
    let mut s = Sprite::new(8, 8, ColorMode::Rgba);
    s.images.insert(ImageId(1), std::sync::Arc::new(Image::new(2, 2, ColorMode::Rgba)));
    s.cels.insert((LayerId(1), 0), Cel { position: (2, 2), opacity: 255, image: ImageId(1) });
    let before = s.clone();
    let inv = Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, red())] }
        .apply(&mut s).unwrap();
    let cel = *s.cel(LayerId(1), 0).unwrap();
    assert_eq!(cel.position, (0, 0));
    let img = s.image(cel.image).unwrap();
    assert_eq!((img.width(), img.height()), (4, 4));
    assert_eq!(img.get(0, 0), Some(red()));
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn draw_on_locked_layer_fails_without_changes() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    s.layers[0].locked = true;
    let before = s.clone();
    let r = Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, red())] }.apply(&mut s);
    assert_eq!(r, Err(crate::CmdError::LayerLocked(LayerId(1))));
    assert_eq!(s, before);
}

#[test]
fn draw_wrong_mode_fails() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let r = Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Index(1))] }.apply(&mut s);
    assert_eq!(r, Err(crate::CmdError::ColorModeMismatch));
}

// command.rs tests
use cs_core::*;
use crate::{Command, CmdError, LayerProps};

#[test]
fn add_and_remove_layer_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let id = s.next_layer_id();
    let inv = Command::AddLayer { index: 1, layer: Layer::new_image(id, "Layer 2") }.apply(&mut s).unwrap();
    assert_eq!(s.layers[1].id, id);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn remove_layer_restores_cels_on_undo() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))] }
        .apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::RemoveLayer { id: LayerId(1) }.apply(&mut s).unwrap();
    assert!(s.layers.is_empty() && s.cels.is_empty());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn frames_shift_cels() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 2, 3)))] }
        .apply(&mut s).unwrap();
    let before = s.clone();
    let inv = Command::AddFrame { index: 0, frame: Frame { duration_ms: 50 } }.apply(&mut s).unwrap();
    assert_eq!(s.frames.len(), 2);
    assert!(s.cel(LayerId(1), 0).is_none() && s.cel(LayerId(1), 1).is_some());
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
    assert_eq!(Command::RemoveFrame { index: 0 }.apply(&mut s), Err(CmdError::LastFrame));
}

#[test]
fn set_layer_props_roundtrip() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let inv = Command::SetLayerProps { id: LayerId(1), props: LayerProps { visible: Some(false), opacity: Some(10), ..Default::default() } }
        .apply(&mut s).unwrap();
    assert!(!s.layers[0].visible);
    assert_eq!(s.layers[0].opacity, 10);
    inv.apply(&mut s).unwrap();
    assert_eq!(s, before);
}

#[test]
fn failing_batch_rolls_back() {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    let before = s.clone();
    let r = Command::Batch(vec![
        Command::AddFrame { index: 1, frame: Frame { duration_ms: 100 } },
        Command::RemoveLayer { id: LayerId(99) },
    ]).apply(&mut s);
    assert_eq!(r, Err(CmdError::LayerNotFound(LayerId(99))));
    assert_eq!(s, before);
}

#[test]
fn commands_serialize() {
    let c = Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Index(2))] };
    let json = serde_json::to_string(&c).unwrap();
    assert_eq!(serde_json::from_str::<Command>(&json).unwrap(), c);
}
```

- [ ] **Step 2: Failing property test** — `crates/cs-commands/tests/undo_roundtrip.rs`

```rust
use cs_commands::{Command, History, LayerProps};
use cs_core::*;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Draw { x: i32, y: i32, c: u8 },
    AddLayer,
    AddFrame,
    RemoveFrame,
    Hide,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (-2..10i32, -2..10i32, any::<u8>()).prop_map(|(x, y, c)| Op::Draw { x, y, c }),
        1 => Just(Op::AddLayer),
        1 => Just(Op::AddFrame),
        1 => Just(Op::RemoveFrame),
        1 => Just(Op::Hide),
    ]
}

fn to_command(s: &Sprite, op: &Op) -> Command {
    let top = s.layers.last().unwrap().id;
    let last_frame = s.frames.len() - 1;
    match *op {
        Op::Draw { x, y, c } => Command::DrawPixels { layer: top, frame: last_frame, pixels: vec![(x, y, PixelValue::Rgba(Rgba::rgb(c, c, c)))] },
        Op::AddLayer => Command::AddLayer { index: s.layers.len(), layer: Layer::new_image(s.next_layer_id(), "L") },
        Op::AddFrame => Command::AddFrame { index: 0, frame: Frame { duration_ms: 100 } },
        Op::RemoveFrame => Command::RemoveFrame { index: 0 },
        Op::Hide => Command::SetLayerProps { id: top, props: LayerProps { visible: Some(false), ..Default::default() } },
    }
}

proptest! {
    #[test]
    fn undo_all_restores_initial_and_redo_all_restores_final(ops in proptest::collection::vec(op(), 0..30)) {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let initial = s.clone();
        let mut h = History::new();
        for o in &ops {
            let cmd = to_command(&s, o);
            let _ = h.execute(&mut s, cmd); // los errores (p. ej. LastFrame) no cambian nada
        }
        let fin = s.clone();
        while h.undo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &initial);
        while h.redo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &fin);
    }
}
```

- [ ] **Step 3: Run, expect FAIL** — `cargo test -p cs-commands`

- [ ] **Step 4: Implement**

`Cargo.toml`:
```toml
[package]
name = "cs-commands"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
cs-core.workspace = true
serde.workspace = true
thiserror.workspace = true
log.workspace = true

[dev-dependencies]
proptest.workspace = true
serde_json.workspace = true
```

`src/lib.rs`:
```rust
//! Motor de comandos: toda mutación del documento pasa por aquí.

mod command;
mod draw;
mod history;

pub use command::{CmdError, Command, LayerProps};
pub use history::History;
```

`src/command.rs`:
```rust
use std::sync::Arc;

use cs_core::{Cel, Frame, Image, ImageId, Layer, LayerId, PixelValue, Sprite};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CmdError {
    #[error("layer {0:?} not found")]
    LayerNotFound(LayerId),
    #[error("layer {0:?} is locked")]
    LayerLocked(LayerId),
    #[error("layer {0:?} is not an image layer")]
    NotImageLayer(LayerId),
    #[error("layer {0:?} has child layers")]
    LayerHasChildren(LayerId),
    #[error("layer id {0:?} already exists")]
    DuplicateLayerId(LayerId),
    #[error("frame {0} out of range")]
    FrameOutOfRange(usize),
    #[error("index {0} out of range")]
    IndexOutOfRange(usize),
    #[error("cannot remove the last frame")]
    LastFrame,
    #[error("pixel value does not match the sprite color mode")]
    ColorModeMismatch,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerProps {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    pub opacity: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Pinta píxeles en coordenadas del lienzo; crea o expande el cel si hace falta.
    DrawPixels { layer: LayerId, frame: usize, pixels: Vec<(i32, i32, PixelValue)> },
    /// Primitiva: pone (o quita, con `None`) el cel de una capa en un frame.
    SetCel { layer: LayerId, frame: usize, cel: Option<Cel> },
    /// Primitiva: pone (o quita, con `None`) una imagen del pool.
    SetImage { id: ImageId, image: Option<Arc<Image>> },
    AddLayer { index: usize, layer: Layer },
    RemoveLayer { id: LayerId },
    SetLayerProps { id: LayerId, props: LayerProps },
    AddFrame { index: usize, frame: Frame },
    RemoveFrame { index: usize },
    /// Transacción: se aplica completa o nada; cuenta como un paso de undo.
    Batch(Vec<Command>),
}

impl Command {
    /// Aplica el comando y devuelve su inverso. Si falla, el documento no cambia.
    pub fn apply(&self, doc: &mut Sprite) -> Result<Command, CmdError> {
        match self {
            Command::DrawPixels { layer, frame, pixels } => crate::draw::draw_pixels(doc, *layer, *frame, pixels),
            Command::SetCel { layer, frame, cel } => set_cel(doc, *layer, *frame, *cel),
            Command::SetImage { id, image } => {
                let old = match image {
                    Some(img) => doc.images.insert(*id, img.clone()),
                    None => doc.images.remove(id),
                };
                Ok(Command::SetImage { id: *id, image: old })
            }
            Command::AddLayer { index, layer } => add_layer(doc, *index, layer),
            Command::RemoveLayer { id } => remove_layer(doc, *id),
            Command::SetLayerProps { id, props } => set_layer_props(doc, *id, props),
            Command::AddFrame { index, frame } => add_frame(doc, *index, *frame),
            Command::RemoveFrame { index } => remove_frame(doc, *index),
            Command::Batch(cmds) => apply_batch(doc, cmds),
        }
    }
}

fn set_cel(doc: &mut Sprite, layer: LayerId, frame: usize, cel: Option<Cel>) -> Result<Command, CmdError> {
    let old = match cel {
        Some(c) => {
            doc.layer(layer).ok_or(CmdError::LayerNotFound(layer))?;
            if frame >= doc.frames.len() {
                return Err(CmdError::FrameOutOfRange(frame));
            }
            doc.cels.insert((layer, frame), c)
        }
        None => doc.cels.remove(&(layer, frame)),
    };
    Ok(Command::SetCel { layer, frame, cel: old })
}

fn add_layer(doc: &mut Sprite, index: usize, layer: &Layer) -> Result<Command, CmdError> {
    if index > doc.layers.len() {
        return Err(CmdError::IndexOutOfRange(index));
    }
    if doc.layer(layer.id).is_some() {
        return Err(CmdError::DuplicateLayerId(layer.id));
    }
    doc.layers.insert(index, layer.clone());
    Ok(Command::RemoveLayer { id: layer.id })
}

fn remove_layer(doc: &mut Sprite, id: LayerId) -> Result<Command, CmdError> {
    let index = doc.layer_index(id).ok_or(CmdError::LayerNotFound(id))?;
    if doc.layers.iter().any(|l| l.parent == Some(id)) {
        return Err(CmdError::LayerHasChildren(id));
    }
    let layer = doc.layers.remove(index);
    let keys: Vec<_> = doc.cels.keys().filter(|(l, _)| *l == id).copied().collect();
    let mut inverse = vec![Command::AddLayer { index, layer }];
    for key in keys {
        let cel = doc.cels.remove(&key);
        inverse.push(Command::SetCel { layer: key.0, frame: key.1, cel });
    }
    Ok(Command::Batch(inverse))
}

fn set_layer_props(doc: &mut Sprite, id: LayerId, props: &LayerProps) -> Result<Command, CmdError> {
    let layer = doc.layer_mut(id).ok_or(CmdError::LayerNotFound(id))?;
    let mut old = LayerProps::default();
    if let Some(v) = &props.name {
        old.name = Some(std::mem::replace(&mut layer.name, v.clone()));
    }
    if let Some(v) = props.visible {
        old.visible = Some(std::mem::replace(&mut layer.visible, v));
    }
    if let Some(v) = props.locked {
        old.locked = Some(std::mem::replace(&mut layer.locked, v));
    }
    if let Some(v) = props.opacity {
        old.opacity = Some(std::mem::replace(&mut layer.opacity, v));
    }
    Ok(Command::SetLayerProps { id, props: old })
}

/// Reindexa los cels: `f(frame)` devuelve el nuevo índice.
fn remap_frames(doc: &mut Sprite, f: impl Fn(usize) -> usize) {
    let cels = std::mem::take(&mut doc.cels);
    doc.cels = cels.into_iter().map(|((l, fr), c)| ((l, f(fr)), c)).collect();
}

fn add_frame(doc: &mut Sprite, index: usize, frame: Frame) -> Result<Command, CmdError> {
    if index > doc.frames.len() {
        return Err(CmdError::IndexOutOfRange(index));
    }
    remap_frames(doc, |f| if f >= index { f + 1 } else { f });
    doc.frames.insert(index, frame);
    Ok(Command::RemoveFrame { index })
}

fn remove_frame(doc: &mut Sprite, index: usize) -> Result<Command, CmdError> {
    if index >= doc.frames.len() {
        return Err(CmdError::FrameOutOfRange(index));
    }
    if doc.frames.len() == 1 {
        return Err(CmdError::LastFrame);
    }
    let keys: Vec<_> = doc.cels.keys().filter(|(_, f)| *f == index).copied().collect();
    let mut inverse = vec![Command::AddFrame { index, frame: doc.frames.remove(index) }];
    for key in keys {
        let cel = doc.cels.remove(&key);
        inverse.push(Command::SetCel { layer: key.0, frame: key.1, cel });
    }
    remap_frames(doc, |f| if f > index { f - 1 } else { f });
    Ok(Command::Batch(inverse))
}

fn apply_batch(doc: &mut Sprite, cmds: &[Command]) -> Result<Command, CmdError> {
    let mut inverses = Vec::with_capacity(cmds.len());
    for cmd in cmds {
        match cmd.apply(doc) {
            Ok(inv) => inverses.push(inv),
            Err(e) => {
                for inv in inverses.iter().rev() {
                    if let Err(rollback) = inv.apply(doc) {
                        log::error!("batch rollback failed: {rollback}");
                    }
                }
                return Err(e);
            }
        }
    }
    inverses.reverse();
    Ok(Command::Batch(inverses))
}
```

`src/draw.rs`:
```rust
use std::sync::Arc;

use cs_core::{Cel, Image, LayerId, LayerKind, PixelValue, Sprite};

use crate::{CmdError, Command};

pub(crate) fn draw_pixels(
    doc: &mut Sprite,
    layer_id: LayerId,
    frame: usize,
    pixels: &[(i32, i32, PixelValue)],
) -> Result<Command, CmdError> {
    let layer = doc.layer(layer_id).ok_or(CmdError::LayerNotFound(layer_id))?;
    if layer.kind != LayerKind::Image {
        return Err(CmdError::NotImageLayer(layer_id));
    }
    if layer.locked {
        return Err(CmdError::LayerLocked(layer_id));
    }
    if frame >= doc.frames.len() {
        return Err(CmdError::FrameOutOfRange(frame));
    }
    if pixels.iter().any(|&(_, _, v)| !doc.color_mode.accepts(v)) {
        return Err(CmdError::ColorModeMismatch);
    }
    let (w, h) = (doc.width as i32, doc.height as i32);
    let pixels: Vec<_> = pixels.iter().copied().filter(|&(x, y, _)| x >= 0 && y >= 0 && x < w && y < h).collect();
    if pixels.is_empty() {
        return Ok(Command::Batch(Vec::new()));
    }

    let mode = doc.color_mode;
    let (cel, old_image) = match doc.cel(layer_id, frame).copied() {
        Some(cel) => (cel, doc.image(cel.image).cloned()),
        None => {
            let id = doc.next_image_id();
            let cel = Cel { position: (0, 0), opacity: 255, image: id };
            (cel, None)
        }
    };
    let mut image = match &old_image {
        Some(img) => img.clone(),
        None => Arc::new(Image::new(doc.width, doc.height, mode)),
    };

    // Expande el cel si algún píxel cae fuera de él.
    let (mut x0, mut y0) = cel.position;
    let (mut x1, mut y1) = (x0 + image.width() as i32, y0 + image.height() as i32);
    let (cx0, cy0, cx1, cy1) = (x0, y0, x1, y1);
    for &(x, y, _) in &pixels {
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + 1);
        y1 = y1.max(y + 1);
    }
    let mut new_position = cel.position;
    if (x0, y0, x1, y1) != (cx0, cy0, cx1, cy1) {
        let mut grown = Image::new((x1 - x0) as u32, (y1 - y0) as u32, mode);
        for y in 0..image.height() {
            for x in 0..image.width() {
                let v = image.get(x, y).expect("in bounds");
                let (gx, gy) = ((cx0 - x0) as u32 + x, (cy0 - y0) as u32 + y);
                grown.set(gx, gy, v).expect("grown image covers old cel");
            }
        }
        image = Arc::new(grown);
        new_position = (x0, y0);
    }

    let img = Arc::make_mut(&mut image);
    for &(x, y, v) in &pixels {
        img.set((x - new_position.0) as u32, (y - new_position.1) as u32, v)
            .expect("cel covers all pixels");
    }
    doc.images.insert(cel.image, image);

    // Todos los cels que comparten la imagen (vinculados) se mueven juntos.
    let mut inverse = vec![Command::SetImage { id: cel.image, image: old_image }];
    if doc.cel(layer_id, frame).is_none() {
        doc.cels.insert((layer_id, frame), Cel { position: new_position, ..cel });
        inverse.push(Command::SetCel { layer: layer_id, frame, cel: None });
    } else if new_position != cel.position {
        let linked: Vec<_> = doc.cels.iter().filter(|(_, c)| c.image == cel.image).map(|(k, _)| *k).collect();
        for key in linked {
            let c = doc.cels.get_mut(&key).expect("key exists");
            inverse.push(Command::SetCel { layer: key.0, frame: key.1, cel: Some(*c) });
            c.position = new_position;
        }
    }
    inverse.reverse();
    Ok(Command::Batch(inverse))
}
```

`src/history.rs`:
```rust
use cs_core::Sprite;

use crate::{CmdError, Command};

/// Pilas de undo/redo. Cada entrada es el comando que deshace (o rehace) un paso.
#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(&mut self, doc: &mut Sprite, cmd: Command) -> Result<(), CmdError> {
        let inverse = cmd.apply(doc)?;
        self.undo.push(inverse);
        self.redo.clear();
        Ok(())
    }

    /// Devuelve `false` si no había nada que deshacer.
    pub fn undo(&mut self, doc: &mut Sprite) -> Result<bool, CmdError> {
        Self::step(&mut self.undo, &mut self.redo, doc)
    }

    pub fn redo(&mut self, doc: &mut Sprite) -> Result<bool, CmdError> {
        Self::step(&mut self.redo, &mut self.undo, doc)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn step(from: &mut Vec<Command>, to: &mut Vec<Command>, doc: &mut Sprite) -> Result<bool, CmdError> {
        let Some(cmd) = from.pop() else { return Ok(false) };
        match cmd.apply(doc) {
            Ok(opposite) => {
                to.push(opposite);
                Ok(true)
            }
            Err(e) => {
                from.push(cmd);
                Err(e)
            }
        }
    }
}
```

- [ ] **Step 5: Run, expect PASS** — `cargo test -p cs-commands`
- [ ] **Step 6: Commit** — `feat(commands): command engine with inverse-based undo/redo`

---

### Task 4: `cs-render` — compositor CPU

**Files:**
- Create: `crates/cs-render/Cargo.toml` (deps: `cs-core`; dev: `cs-commands`), `crates/cs-render/src/lib.rs`

**Interfaces:**
- Produces: `render_frame(sprite: &Sprite, frame: usize) -> Vec<Rgba>` (len `width*height`, fila por fila), `resolve(sprite: &Sprite, v: PixelValue) -> Rgba`, `blend_over(dst: Rgba, src: Rgba, alpha: u8) -> Rgba`.

- [ ] **Step 1: Failing tests**

```rust
use super::*;
use cs_commands::Command;
use cs_core::*;

#[test]
fn blend_over_math() {
    let blue = Rgba::rgb(0, 0, 255);
    assert_eq!(blend_over(blue, Rgba::rgb(255, 0, 0), 255), Rgba::rgb(255, 0, 0));
    assert_eq!(blend_over(blue, Rgba::rgb(255, 0, 0), 128), Rgba::new(128, 0, 127, 255));
    assert_eq!(blend_over(Rgba::TRANSPARENT, Rgba::rgb(9, 9, 9), 0), Rgba::TRANSPARENT);
}

#[test]
fn renders_layers_bottom_to_top_and_skips_hidden() {
    let mut s = Sprite::new(2, 1, ColorMode::Rgba);
    let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
    let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
    Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, red), (1, 0, red)] }.apply(&mut s).unwrap();
    Command::AddLayer { index: 1, layer: Layer::new_image(LayerId(2), "top") }.apply(&mut s).unwrap();
    Command::DrawPixels { layer: LayerId(2), frame: 0, pixels: vec![(1, 0, green)] }.apply(&mut s).unwrap();
    assert_eq!(render_frame(&s, 0), vec![Rgba::rgb(255, 0, 0), Rgba::rgb(0, 255, 0)]);
    s.layer_mut(LayerId(2)).unwrap().visible = false;
    assert_eq!(render_frame(&s, 0), vec![Rgba::rgb(255, 0, 0), Rgba::rgb(255, 0, 0)]);
}

#[test]
fn indexed_resolves_palette_and_transparent_index() {
    let mut s = Sprite::new(2, 1, ColorMode::Indexed { transparent_index: 0 });
    Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Index(21))] }.apply(&mut s).unwrap();
    assert_eq!(render_frame(&s, 0), vec![Rgba::rgb(255, 255, 255), Rgba::TRANSPARENT]);
}

#[test]
fn hidden_group_hides_children() {
    let mut s = Sprite::new(1, 1, ColorMode::Rgba);
    Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 1, 1)))] }.apply(&mut s).unwrap();
    let mut group = Layer::new_image(LayerId(2), "g");
    group.kind = LayerKind::Group;
    group.visible = false;
    s.layers.push(group);
    s.layers[0].parent = Some(LayerId(2));
    assert_eq!(render_frame(&s, 0), vec![Rgba::TRANSPARENT]);
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p cs-render`
- [ ] **Step 3: Implement**

```rust
//! Compositor CPU de referencia: capas visibles → buffer RGBA.

use cs_core::{Layer, LayerKind, PixelValue, Rgba, Sprite};

/// Convierte un valor de píxel a color, resolviendo la paleta en modo indexado.
pub fn resolve(sprite: &Sprite, value: PixelValue) -> Rgba {
    match value {
        PixelValue::Rgba(c) => c,
        PixelValue::Index(i) => match sprite.color_mode {
            cs_core::ColorMode::Indexed { transparent_index } if i == transparent_index => Rgba::TRANSPARENT,
            _ => sprite.palette.get(i).unwrap_or(Rgba::TRANSPARENT),
        },
    }
}

/// Mezcla "normal" (src over dst) con colores no premultiplicados; `alpha` ya incluye opacidades.
pub fn blend_over(dst: Rgba, src: Rgba, alpha: u8) -> Rgba {
    let sa = alpha as u32;
    if sa == 0 {
        return dst;
    }
    let da = dst.a as u32;
    let out_a = sa * 255 + da * (255 - sa);
    if out_a == 0 {
        return Rgba::TRANSPARENT;
    }
    let ch = |s: u8, d: u8| ((s as u32 * sa * 255 + d as u32 * da * (255 - sa) + out_a / 2) / out_a) as u8;
    Rgba::new(ch(src.r, dst.r), ch(src.g, dst.g), ch(src.b, dst.b), ((out_a + 127) / 255) as u8)
}

fn mul(a: u32, b: u32) -> u32 {
    (a * b + 127) / 255
}

/// Visibilidad y opacidad efectivas considerando los grupos ancestros.
fn effective(sprite: &Sprite, layer: &Layer) -> Option<u32> {
    let mut opacity = layer.opacity as u32;
    let mut current = layer;
    if !current.visible {
        return None;
    }
    while let Some(parent) = current.parent.and_then(|p| sprite.layer(p)) {
        if !parent.visible {
            return None;
        }
        opacity = mul(opacity, parent.opacity as u32);
        current = parent;
    }
    Some(opacity)
}

pub fn render_frame(sprite: &Sprite, frame: usize) -> Vec<Rgba> {
    let (w, h) = (sprite.width as i32, sprite.height as i32);
    let mut out = vec![Rgba::TRANSPARENT; (w * h) as usize];
    for layer in sprite.layers.iter().filter(|l| l.kind == LayerKind::Image) {
        let Some(layer_opacity) = effective(sprite, layer) else { continue };
        let Some(cel) = sprite.cel(layer.id, frame) else { continue };
        let Some(image) = sprite.image(cel.image) else { continue };
        let opacity = mul(layer_opacity, cel.opacity as u32);
        for iy in 0..image.height() {
            let y = cel.position.1 + iy as i32;
            if y < 0 || y >= h {
                continue;
            }
            for ix in 0..image.width() {
                let x = cel.position.0 + ix as i32;
                if x < 0 || x >= w {
                    continue;
                }
                let src = resolve(sprite, image.get(ix, iy).expect("in bounds"));
                let alpha = mul(src.a as u32, opacity) as u8;
                let i = (y * w + x) as usize;
                out[i] = blend_over(out[i], src, alpha);
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run, expect PASS** — `cargo test -p cs-render`
- [ ] **Step 5: Commit** — `feat(render): CPU compositor with normal blend and indexed palettes`

---

### Task 5: `cs-tools` — lápiz y borrador

**Files:**
- Create: `crates/cs-tools/Cargo.toml` (deps: `cs-core`, `cs-raster`, `cs-commands`), `crates/cs-tools/src/lib.rs`, `crates/cs-tools/src/pencil.rs`

**Interfaces:**
- Consumes: `cs_raster::{line, pixel_perfect, brush_square}`, `Command::DrawPixels`.
- Produces: `ToolContext { layer: LayerId, frame: usize, color: PixelValue, brush_size: u32, pixel_perfect: bool }`, `trait Tool { press(&mut self, &ToolContext, Point); drag(&mut self, &ToolContext, Point); release(&mut self, &ToolContext) -> Option<Command>; preview(&self, &ToolContext) -> Vec<(i32, i32, PixelValue)>; is_active(&self) -> bool }`, `Pencil::new()`, `Eraser::new(transparent: PixelValue)` — el borrador pinta con el color transparente que recibe.

- [ ] **Step 1: Failing tests** (`pencil.rs`)

```rust
use super::*;
use cs_core::{LayerId, PixelValue, Rgba};

fn ctx(pp: bool) -> ToolContext {
    ToolContext { layer: LayerId(1), frame: 0, color: PixelValue::Rgba(Rgba::rgb(9, 9, 9)), brush_size: 1, pixel_perfect: pp }
}

fn coords(cmd: Command) -> Vec<(i32, i32)> {
    let Command::DrawPixels { pixels, .. } = cmd else { panic!("expected DrawPixels") };
    pixels.into_iter().map(|(x, y, _)| (x, y)).collect()
}

#[test]
fn drag_interpolates_with_line() {
    let mut p = Pencil::new();
    let c = ctx(false);
    p.press(&c, (0, 0));
    p.drag(&c, (3, 0));
    assert!(p.is_active());
    assert_eq!(coords(p.release(&c).unwrap()), vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
    assert!(!p.is_active());
}

#[test]
fn pixel_perfect_applies_on_release() {
    let mut p = Pencil::new();
    let c = ctx(true);
    p.press(&c, (0, 0));
    p.drag(&c, (1, 0));
    p.drag(&c, (1, 1));
    assert_eq!(coords(p.release(&c).unwrap()), vec![(0, 0), (1, 1)]);
}

#[test]
fn eraser_uses_transparent_value() {
    let t = PixelValue::Index(0);
    let mut e = Eraser::new(t);
    let c = ctx(false);
    e.press(&c, (2, 2));
    let Command::DrawPixels { pixels, .. } = e.release(&c).unwrap() else { panic!() };
    assert_eq!(pixels, vec![(2, 2, t)]);
}

#[test]
fn release_without_press_is_none() {
    assert!(Pencil::new().release(&ctx(false)).is_none());
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p cs-tools`
- [ ] **Step 3: Implement**

`lib.rs`:
```rust
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
```

`pencil.rs`:
```rust
use std::collections::BTreeSet;

use cs_commands::Command;
use cs_core::PixelValue;
use cs_raster::{brush_square, line, pixel_perfect, Point};

use crate::{Tool, ToolContext};

#[derive(Debug, Default)]
pub struct Pencil {
    path: Vec<Point>,
}

impl Pencil {
    pub fn new() -> Self {
        Self::default()
    }

    fn pixels(&self, ctx: &ToolContext, color: PixelValue) -> Vec<(i32, i32, PixelValue)> {
        let path = if ctx.pixel_perfect && ctx.brush_size <= 1 { pixel_perfect(&self.path) } else { self.path.clone() };
        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for p in path {
            for q in brush_square(p, ctx.brush_size) {
                if seen.insert(q) {
                    out.push((q.0, q.1, color));
                }
            }
        }
        out
    }

    fn finish(&mut self, ctx: &ToolContext, color: PixelValue) -> Option<Command> {
        if self.path.is_empty() {
            return None;
        }
        let pixels = self.pixels(ctx, color);
        self.path.clear();
        Some(Command::DrawPixels { layer: ctx.layer, frame: ctx.frame, pixels })
    }
}

impl Tool for Pencil {
    fn press(&mut self, _ctx: &ToolContext, p: Point) {
        self.path = vec![p];
    }

    fn drag(&mut self, _ctx: &ToolContext, p: Point) {
        let Some(&last) = self.path.last() else { return };
        if last != p {
            self.path.extend(line(last, p).into_iter().skip(1));
        }
    }

    fn release(&mut self, ctx: &ToolContext) -> Option<Command> {
        self.finish(ctx, ctx.color)
    }

    fn preview(&self, ctx: &ToolContext) -> Vec<(i32, i32, PixelValue)> {
        self.pixels(ctx, ctx.color)
    }

    fn is_active(&self) -> bool {
        !self.path.is_empty()
    }
}

/// Lápiz que pinta con el valor transparente del sprite.
#[derive(Debug)]
pub struct Eraser {
    pencil: Pencil,
    transparent: PixelValue,
}

impl Eraser {
    pub fn new(transparent: PixelValue) -> Self {
        Self { pencil: Pencil::new(), transparent }
    }
}

impl Tool for Eraser {
    fn press(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.press(ctx, p);
    }

    fn drag(&mut self, ctx: &ToolContext, p: Point) {
        self.pencil.drag(ctx, p);
    }

    fn release(&mut self, ctx: &ToolContext) -> Option<Command> {
        self.pencil.finish(ctx, self.transparent)
    }

    fn preview(&self, ctx: &ToolContext) -> Vec<(i32, i32, PixelValue)> {
        self.pencil.pixels(ctx, self.transparent)
    }

    fn is_active(&self) -> bool {
        self.pencil.is_active()
    }
}
```

- [ ] **Step 4: Run, expect PASS** — `cargo test -p cs-tools`
- [ ] **Step 5: Commit** — `feat(tools): pencil and eraser`

---

### Task 6: `cs-ui` + `apps/chimpsprite` — app con lienzo pintable

**Files:**
- Create: `crates/cs-ui/Cargo.toml` (deps: todos los cs-*, `eframe` con feature `wgpu`, `png`, `log`), `crates/cs-ui/src/lib.rs`, `src/editor.rs`, `src/canvas.rs`, `src/panels.rs`, `src/theme.rs`, `src/capture.rs`
- Create: `apps/chimpsprite/Cargo.toml` (deps: `cs-ui`, `eframe`, `env_logger`), `apps/chimpsprite/src/main.rs`

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: `cs_ui::ChimpApp::new(&eframe::CreationContext) -> ChimpApp` (impl `eframe::App`); `Editor` (estado del documento: `sprite`, `history`, `layer`, `frame`, `primary: PixelValue`, `secondary: PixelValue`, `tool: ToolKind`, `brush_size`, `pixel_perfect`, `revision: u64`) con `Editor::execute(Command)`, `undo()`, `redo()`, `tool_context()`.
- Env vars de desarrollo: `CHIMPSPRITE_DEMO=1` dibuja un sprite de demo al arrancar (vía comandos); `CHIMPSPRITE_SCREENSHOT=<ruta.png>` guarda una captura de la ventana tras unos frames y cierra.

Comportamiento:
- Sprite inicial 64×64 RGBA.
- Lienzo central: fondo damero (textura 2×2 en repeat), sprite como textura `Nearest` recompuesta cuando cambia `revision`, preview de la herramienta encima, grid de píxeles si zoom ≥ 8, borde del lienzo.
- Zoom con rueda (niveles 1,2,3,4,6,8,12,16,24,32,48,64) anclado al cursor; paneo con botón medio o espacio+arrastre; zoom inicial que ajusta el sprite.
- Clic izquierdo pinta con color primario, derecho con secundario.
- Toolbar izquierda: lápiz (B), borrador (E); tamaño de pincel; checkbox pixel-perfect.
- Panel derecho: swatches primario/secundario (X intercambia), grilla de paleta (clic izq → primario, der → secundario).
- Ctrl+Z / Ctrl+Y (y Ctrl+Shift+Z) undo/redo; barra de estado inferior con coordenadas del cursor, zoom y tamaño del sprite.
- Tema oscuro propio.

- [ ] **Step 1: Unit tests de `Editor`** (en `editor.rs`, sin UI)

```rust
#[test]
fn execute_undo_redo_bump_revision() {
    let mut e = Editor::new(Sprite::new(4, 4, ColorMode::Rgba));
    let r0 = e.revision;
    e.execute(Command::DrawPixels { layer: LayerId(1), frame: 0, pixels: vec![(0, 0, e.primary)] });
    assert!(e.revision > r0 && e.history.can_undo());
    e.undo();
    assert!(e.sprite.cels.is_empty());
    e.redo();
    assert!(!e.sprite.cels.is_empty());
}
```

- [ ] **Step 2: Implementar `editor.rs`, `theme.rs`, `capture.rs`, `panels.rs`, `canvas.rs`, `lib.rs`, `main.rs`** según el comportamiento descrito (código en el commit; la API de egui 0.36 se valida con `cargo build`).
- [ ] **Step 3: `cargo test --workspace` PASS y `cargo clippy --workspace -- -D warnings` limpio.**
- [ ] **Step 4: Verificación visual** — `CHIMPSPRITE_DEMO=1 CHIMPSPRITE_SCREENSHOT=target/m1.png cargo run -p chimpsprite`; revisar la captura; luego lanzar la app normal para el usuario.
- [ ] **Step 5: Commit** — `feat(ui): egui editor with paintable canvas and palette`

---

### Task 7: CI

**Files:**
- Create: `.github/workflows/ci.yml`

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
      - run: cargo check --target wasm32-unknown-unknown -p cs-core -p cs-raster -p cs-commands -p cs-render -p cs-tools
```

- [ ] **Step 1:** Crear el workflow, `cargo fmt --all` local.
- [ ] **Step 2: Commit + push** — `ci: fmt, clippy, tests and wasm check`; verificar el run con `gh run watch`.
