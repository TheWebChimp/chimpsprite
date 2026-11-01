# Editor de fuentes pixel, parte 1 (núcleo): plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Crear fuentes pixel como artboards (un frame por glifo), editarlas en el lienzo con guías
y letra fantasma, manejarlas desde un panel Font del dock y escribir con ellas en la herramienta
Texto.

**Architecture:** El modelo (`FontInfo`) vive en `cs-core` como campo opcional del `Artboard`. Los
cambios de la fuente son `DocCommand`s con undo en `cs-commands` (agregar o quitar glifos es
`AddFrame`/`RemoveFrame` más `SetFont`). El ancho automático se calcula al usarlo, desde los
pixeles, así que pintar no necesita ganchos. La UI (`cs-ui`) agrega el artboard de fuente al
lienzo, el panel Font, el diálogo New Font y la fuente del documento en la herramienta Texto.

**Tech Stack:** Rust, egui/eframe 0.36 (wgpu), serde, el UI kit propio (`crates/cs-ui/src/kit`).

## Global Constraints

- Spec: `docs/superpowers/specs/2026-11-01-pixel-font-core-design.md`.
- **Cambio sobre el spec, aprobado al planear:** `GlyphInfo` guarda `advance: Option<u32>`
  (`None` = automático) en vez de `advance` + `auto_advance`. El automático se calcula cuando se
  pide (la última columna con pixeles + 1 + `letter_spacing`; vacío, la mitad de la celda;
  Monospace, el ancho de la celda). Así no hay que recalcular después de cada trazo.
- **Cambio sobre el spec:** la letra fantasma elige entre las dos fuentes de egui
  (`GhostSettings { enabled, monospace }`: Ubuntu o Hack), no entre las fuentes de Texto.
- Textos de la UI en inglés, como el resto de la app (`"New Font…"`, `"Font"`, `"Map"`,
  `"Test Drive"`). Comentarios y docs en español, como el código vecino.
- `cs-core`, `cs-commands` y `cs-tools` sin egui (tienen que compilar a wasm).
- `FontInfo` y lo que contiene: `Clone, Debug, PartialEq, Eq, Hash` donde se pueda, más
  `Serialize, Deserialize` (el `Artboard` deriva `Eq` y los `DocCommand` se serializan). Sin `f32`.
- Toolchain y CPU: antes de cada comando de cargo,
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH CARGO_BUILD_JOBS=8`
  y correr cargo a prioridad baja: `cmd //c "start /low /b /wait cargo test -p <crate>"`.
- Ediciones con scripts de Python en `D:/Work/chimpdeas/.refs/tmp/` (nunca `python -` ni
  heredocs con varias comillas). `cargo fmt -p <crate>` y `cargo clippy -p <crate> --all-targets`
  sin avisos antes de cada commit.
- Commits en español, terminando con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
  No tocar `.design/` (el usuario lo edita mientras trabajamos).

---

## Mapa de archivos

| Archivo | Qué |
| --- | --- |
| `crates/cs-core/src/font.rs` (nuevo) | `FontInfo`, `FontMode`, `FontMetrics`, `GhostSettings`, `GlyphInfo`, `font_sprite`, `ink_width`, `PixelFontView`, `Artboard::font_view` |
| `crates/cs-core/src/document.rs` | `Artboard.font: Option<FontInfo>` |
| todos los `Artboard { … }` literales (lista en la Tarea 1) | `font: None` |
| `crates/cs-commands/src/doc_command.rs` | `DocCommand::SetFont` |
| `crates/cs-commands/src/font.rs` (nuevo) | `new_font_artboard`, `add_glyphs`, `remove_glyph`, `set_advance` |
| `crates/cs-io/src/chimp.rs` | `font` en el manifest |
| `crates/cs-tools/src/text.rs` | `FontChoice::Document`, `font_text` |
| `crates/cs-ui/src/editor/font_ops.rs` (nuevo) | lo de la fuente en el `Editor` |
| `crates/cs-ui/src/editor/gestures.rs` | 1 bit pinta con la tinta; Texto con fuente del documento |
| `crates/cs-ui/src/font_overlay.rs` (nuevo) | guías, letra fantasma y línea del ancho en el lienzo |
| `crates/cs-ui/src/canvas.rs`, `canvas/input.rs` | dibujar el overlay, arrastrar el ancho, título y menú ≡ |
| `crates/cs-ui/src/font_dialog.rs` (nuevo) | diálogo New Font |
| `crates/cs-ui/src/font_panel.rs` (nuevo) | panel Font: Map, Test Drive, ajustes |
| `crates/cs-ui/src/dock.rs`, `lib.rs`, `app_menus.rs`, `file_actions.rs` | cablear panel, diálogo, menú y timeline |
| `crates/cs-ui/src/panels/options_bar.rs` | las fuentes del documento en el selector de Texto |
| `crates/cs-ui/src/demo.rs` | `CHIMPSPRITE_DEMO_FONT` |

---

### Task 1: El modelo de la fuente en `cs-core`

**Files:**
- Create: `crates/cs-core/src/font.rs`
- Modify: `crates/cs-core/src/lib.rs` (agregar `mod font;` y `pub use font::*;` junto a los demás)
- Modify: `crates/cs-core/src/document.rs:127-143` (campo nuevo)
- Modify (agregar `font: None,` a cada literal `Artboard { … }`): `cs-core/src/document.rs`
  (líneas ~222, 366, 396, 479), `cs-agent/src/target.rs:193`, `cs-agent/src/ops/structure.rs:69`,
  `cs-render/src/materialize.rs:454`, `cs-commands/tests/doc_undo_roundtrip.rs:63`,
  `cs-commands/src/symbol.rs` (~33, 46, 303), `cs-commands/src/symbol/tests.rs:41`,
  `cs-commands/src/doc_command.rs` (~238, 270), `cs-tools/src/artboard.rs:171`,
  `cs-io/src/chimp.rs` (~262, 559), `cs-io/src/files.rs` (~179, 211), `cs-io/src/export.rs:686`,
  `cs-ui/src/editor/document_ops.rs` (~15, 191). Usar `cargo build --workspace --all-targets`
  para encontrar cualquier otro.

**Interfaces:**
- Produces:
  - `pub enum FontMode { OneBit, Color }`
  - `pub struct FontMetrics { pub baseline: u32, pub cap_height: u32, pub x_height: u32, pub descent: u32 }` con `FontMetrics::for_cell(height: u32) -> Self`
  - `pub struct GhostSettings { pub enabled: bool, pub monospace: bool }` (`Default`: `enabled: true, monospace: false`)
  - `pub struct GlyphInfo { pub ch: char, pub advance: Option<u32> }`
  - `pub struct FontInfo { pub name: String, pub mode: FontMode, pub metrics: FontMetrics, pub letter_spacing: u32, pub space_width: u32, pub monospace: bool, pub ghost: GhostSettings, pub glyphs: Vec<GlyphInfo> }`
  - `FontInfo::new(name: impl Into<String>, cell_height: u32, mode: FontMode) -> FontInfo`
  - `FontInfo::index_of(&self, ch: char) -> Option<usize>`, `FontInfo::insert_index(&self, ch: char) -> Result<usize, usize>`
  - `FontInfo::advance(&self, sprite: &Sprite, index: usize) -> u32`, `FontInfo::missing_width(&self) -> u32`, `FontInfo::text_width(&self, sprite: &Sprite, text: &str) -> u32`
  - `pub const ONE_BIT_INK: PixelValue = PixelValue::Index(1);` y `pub const ONE_BIT_PAPER: PixelValue = PixelValue::Index(0);`
  - `pub fn font_sprite(cell: (u32, u32), mode: FontMode, frames: usize) -> Sprite`
  - `pub fn ink_width(sprite: &Sprite, frame: usize) -> u32`
  - `pub struct GlyphPixels { pub pixels: Vec<(u32, u32, Option<Rgba>)>, pub advance: u32 }` (`None` = tinta de 1 bit)
  - `pub struct PixelFontView { pub cell: (u32, u32), pub metrics: FontMetrics, pub space_width: u32, pub missing_width: u32, pub glyphs: std::collections::BTreeMap<char, GlyphPixels> }`
  - `Artboard::font_view(&self) -> Option<PixelFontView>`
  - `Artboard.font: Option<FontInfo>`

- [ ] **Step 1: Escribir las pruebas que fallan**

Al final de `crates/cs-core/src/font.rs` (archivo nuevo, por ahora solo con las pruebas y un
`use`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cel, Image};
    use std::sync::Arc;

    /// Pinta `points` con la tinta en el glifo `frame` de un sprite de 1 bit.
    fn ink(sprite: &mut Sprite, frame: usize, points: &[(u32, u32)]) {
        let layer = sprite.layers[0].id;
        let mut image = Image::new(sprite.width, sprite.height, sprite.color_mode);
        for &(x, y) in points {
            image.set(x, y, ONE_BIT_INK).unwrap();
        }
        let id = sprite.next_image_id();
        sprite.images.insert(id, Arc::new(image));
        sprite.cels.insert(
            (layer, frame),
            Cel { position: (0, 0), opacity: 255, image: id },
        );
    }

    #[test]
    fn a_new_font_has_the_printable_ascii_and_metrics_from_the_cell() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        assert_eq!(font.glyphs.len(), 94);
        assert_eq!(font.glyphs[0].ch, '!');
        assert_eq!(font.glyphs[93].ch, '~');
        assert!(font.glyphs.iter().all(|g| g.advance.is_none()));
        assert_eq!(
            font.metrics,
            FontMetrics { baseline: 12, cap_height: 10, x_height: 6, descent: 4 }
        );
        assert_eq!((font.letter_spacing, font.space_width), (1, 3));
        assert!(!font.monospace && font.ghost.enabled);
    }

    #[test]
    fn glyphs_are_found_and_inserted_in_unicode_order() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        assert_eq!(font.index_of('A'), Some(('A' as usize) - ('!' as usize)));
        assert_eq!(font.index_of('á'), None);
        assert_eq!(font.insert_index('á'), Err(94));
        assert_eq!(font.insert_index('!'), Ok(0));
    }

    #[test]
    fn a_one_bit_font_sprite_is_indexed_with_paper_and_ink() {
        let s = font_sprite((16, 16), FontMode::OneBit, 3);
        assert_eq!((s.width, s.height, s.frames.len()), (16, 16, 3));
        assert_eq!(s.color_mode, ColorMode::Indexed { transparent_index: 0 });
        assert_eq!(s.palette.len(), 2);
        let c = font_sprite((8, 10), FontMode::Color, 1);
        assert_eq!(c.color_mode, ColorMode::Rgba);
    }

    #[test]
    fn the_automatic_advance_is_the_ink_plus_the_spacing() {
        let mut font = FontInfo::new("Mine", 16, FontMode::OneBit);
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        ink(&mut s, 0, &[(0, 3), (4, 3)]);
        assert_eq!(ink_width(&s, 0), 5);
        assert_eq!(font.advance(&s, 0), 6, "5 columns + 1 of spacing");
        assert_eq!(font.advance(&s, 1), 8, "empty: half the cell");
        font.glyphs[0].advance = Some(9);
        assert_eq!(font.advance(&s, 0), 9, "manual");
        font.monospace = true;
        assert_eq!(font.advance(&s, 0), 16, "monospace: the cell");
    }

    #[test]
    fn the_width_of_a_text_adds_advances_spaces_and_missing_chars() {
        let font = FontInfo::new("Mine", 16, FontMode::OneBit);
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        let a = font.index_of('A').unwrap();
        ink(&mut s, a, &[(0, 5), (3, 5)]);
        // A (4 + 1) + espacio (3) + A (5) + "á" que falta (missing_width).
        assert_eq!(font.text_width(&s, "A Aá"), 5 + 3 + 5 + font.missing_width());
        assert_eq!(font.missing_width(), 5 + 1);
    }

    #[test]
    fn the_view_of_a_font_artboard_has_its_glyph_pixels() {
        let mut s = font_sprite((16, 16), FontMode::OneBit, 94);
        let a = 'A' as usize - '!' as usize;
        ink(&mut s, a, &[(1, 2)]);
        let artboard = crate::Artboard {
            id: crate::ArtboardId(1),
            name: "Mine".into(),
            position: (0, 0),
            sprite: s,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(FontInfo::new("Mine", 16, FontMode::OneBit)),
        };
        let view = artboard.font_view().unwrap();
        assert_eq!(view.cell, (16, 16));
        assert_eq!(view.glyphs[&'A'].pixels, vec![(1, 2, None)]);
        assert_eq!(view.glyphs[&'A'].advance, 2 + 1);
        assert!(view.glyphs[&'B'].pixels.is_empty());
        let plain = crate::Artboard { font: None, ..artboard };
        assert!(plain.font_view().is_none());
    }

    #[test]
    fn font_info_survives_json() {
        let font = FontInfo::new("Mine", 12, FontMode::Color);
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
    }
}
```

Si `serde_json` no está en `[dev-dependencies]` de `crates/cs-core/Cargo.toml`, agregarlo con la
misma versión que usa `cs-commands` (`grep serde_json crates/cs-commands/Cargo.toml`).

- [ ] **Step 2: Correr las pruebas para ver que fallan**

Run: `cmd //c "start /low /b /wait cargo test -p cs-core font"`
Expected: no compila (`FontInfo`, `font_sprite`… no existen).

- [ ] **Step 3: Implementar**

Arriba de las pruebas en `crates/cs-core/src/font.rs`:

```rust
//! Fuentes pixel: un artboard con `font` es una fuente. Su sprite tiene un frame por glifo, del
//! tamaño de la celda, en el orden de los códigos Unicode de `FontInfo::glyphs`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Artboard, ColorMode, Frame, Palette, PixelValue, Rgba, Sprite};

/// 1 bit (prendido o apagado, como BitFontMaker2) o color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontMode {
    OneBit,
    Color,
}

/// Las medidas, en filas desde arriba de la celda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontMetrics {
    /// La línea de escritura: las filas `0..baseline` quedan arriba de ella.
    pub baseline: u32,
    /// Filas de las mayúsculas sobre la baseline.
    pub cap_height: u32,
    /// Filas de las minúsculas sin ascendentes sobre la baseline.
    pub x_height: u32,
    /// Filas debajo de la baseline.
    pub descent: u32,
}

impl FontMetrics {
    /// Las medidas iniciales de una celda de `height`: baseline en 3/4, mayúsculas en 5/8,
    /// x-height en 3/8 y los descendentes hasta abajo.
    pub fn for_cell(height: u32) -> Self {
        let baseline = (height * 3 / 4).max(1);
        FontMetrics {
            baseline,
            cap_height: (height * 5 / 8).max(1),
            x_height: (height * 3 / 8).max(1),
            descent: height - baseline,
        }
    }
}

/// La letra fantasma del editor: prendida y con qué fuente de egui (Ubuntu o Hack).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GhostSettings {
    pub enabled: bool,
    pub monospace: bool,
}

impl Default for GhostSettings {
    fn default() -> Self {
        GhostSettings { enabled: true, monospace: false }
    }
}

/// Un glifo: su carácter y su ancho (`None`: automático).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GlyphInfo {
    pub ch: char,
    #[serde(default)]
    pub advance: Option<u32>,
}

/// La tinta y el papel de una fuente de 1 bit (índices de su paleta).
pub const ONE_BIT_INK: PixelValue = PixelValue::Index(1);
pub const ONE_BIT_PAPER: PixelValue = PixelValue::Index(0);
/// El color con que se ve la tinta de 1 bit (no se exporta).
const ONE_BIT_INK_COLOR: Rgba = Rgba { r: 5, g: 4, b: 3, a: 255 };

/// Lo de fuente de un artboard. `glyphs` va alineada con los frames del sprite.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontInfo {
    pub name: String,
    pub mode: FontMode,
    pub metrics: FontMetrics,
    pub letter_spacing: u32,
    pub space_width: u32,
    pub monospace: bool,
    #[serde(default)]
    pub ghost: GhostSettings,
    pub glyphs: Vec<GlyphInfo>,
}

impl FontInfo {
    /// Una fuente nueva con los caracteres ASCII imprimibles (`!` a `~`), vacíos.
    pub fn new(name: impl Into<String>, cell_height: u32, mode: FontMode) -> FontInfo {
        FontInfo {
            name: name.into(),
            mode,
            metrics: FontMetrics::for_cell(cell_height),
            letter_spacing: 1,
            space_width: 3,
            monospace: false,
            ghost: GhostSettings::default(),
            glyphs: ('!'..='~').map(|ch| GlyphInfo { ch, advance: None }).collect(),
        }
    }

    /// El índice (el frame) del glifo de `ch`.
    pub fn index_of(&self, ch: char) -> Option<usize> {
        self.insert_index(ch).ok()
    }

    /// `Ok(i)` si `ch` está en `i`; `Err(i)` donde iría.
    pub fn insert_index(&self, ch: char) -> Result<usize, usize> {
        self.glyphs.binary_search_by_key(&ch, |g| g.ch)
    }

    /// El ancho del glifo `index`: el de la celda con Monospace; el manual; o el automático
    /// (lo dibujado + `letter_spacing`; vacío, la mitad de la celda).
    pub fn advance(&self, sprite: &Sprite, index: usize) -> u32 {
        if self.monospace {
            return sprite.width;
        }
        if let Some(advance) = self.glyphs.get(index).and_then(|g| g.advance) {
            return advance;
        }
        match ink_width(sprite, index) {
            0 => sprite.width / 2,
            ink => ink + self.letter_spacing,
        }
    }

    /// Lo que avanza un carácter que la fuente no tiene (el cuadrito tachado).
    pub fn missing_width(&self) -> u32 {
        (self.metrics.cap_height / 2).max(3) + self.letter_spacing
    }

    /// El ancho de una línea de `text` (el espacio avanza `space_width`).
    pub fn text_width(&self, sprite: &Sprite, text: &str) -> u32 {
        text.chars()
            .map(|ch| match ch {
                ' ' => self.space_width,
                _ => self
                    .index_of(ch)
                    .map_or(self.missing_width(), |i| self.advance(sprite, i)),
            })
            .sum()
    }
}

/// El sprite de una fuente: `frames` glifos vacíos de `cell`. 1 bit: indexado con papel
/// (transparente) y tinta.
pub fn font_sprite(cell: (u32, u32), mode: FontMode, frames: usize) -> Sprite {
    let mut sprite = match mode {
        FontMode::OneBit => {
            let mut s = Sprite::new(cell.0, cell.1, ColorMode::Indexed { transparent_index: 0 });
            s.palette = Palette::new(vec![Rgba::TRANSPARENT, ONE_BIT_INK_COLOR]);
            s
        }
        FontMode::Color => Sprite::new(cell.0, cell.1, ColorMode::Rgba),
    };
    sprite.frames = vec![Frame { duration_ms: 100 }; frames.max(1)];
    sprite
}

/// Si `value` es un pixel pintado en `sprite`.
fn painted(sprite: &Sprite, value: PixelValue) -> bool {
    match value {
        PixelValue::Index(i) => sprite.color_mode.transparent() != PixelValue::Index(i),
        PixelValue::Rgba(c) => c.a > 0,
    }
}

/// Las columnas de lo dibujado en el glifo `frame`: la última con pixeles + 1 (0 si está vacío).
pub fn ink_width(sprite: &Sprite, frame: usize) -> u32 {
    let Some(layer) = sprite.layers.first().map(|l| l.id) else {
        return 0;
    };
    (0..sprite.width)
        .rev()
        .find(|&x| {
            (0..sprite.height)
                .any(|y| painted(sprite, sprite.pixel(layer, frame, x as i32, y as i32)))
        })
        .map_or(0, |x| x + 1)
}

/// Los pixeles de un glifo, en la celda: `None` es la tinta de 1 bit; en color, su color.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlyphPixels {
    pub pixels: Vec<(u32, u32, Option<Rgba>)>,
    pub advance: u32,
}

/// Una fuente lista para escribir (la herramienta Texto, el Test Drive).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PixelFontView {
    pub cell: (u32, u32),
    pub metrics: FontMetrics,
    pub space_width: u32,
    pub missing_width: u32,
    pub glyphs: BTreeMap<char, GlyphPixels>,
}

impl Artboard {
    /// La fuente de este artboard lista para escribir; `None` si no es una fuente.
    pub fn font_view(&self) -> Option<PixelFontView> {
        let font = self.font.as_ref()?;
        let sprite = &self.sprite;
        let layer = sprite.layers.first()?.id;
        let glyphs = font
            .glyphs
            .iter()
            .enumerate()
            .map(|(i, g)| {
                let mut pixels = Vec::new();
                for y in 0..sprite.height {
                    for x in 0..sprite.width {
                        let v = sprite.pixel(layer, i, x as i32, y as i32);
                        if !painted(sprite, v) {
                            continue;
                        }
                        let color = match (font.mode, v) {
                            (FontMode::Color, PixelValue::Rgba(c)) => Some(c),
                            (FontMode::Color, PixelValue::Index(k)) => sprite.palette.get(k),
                            (FontMode::OneBit, _) => None,
                        };
                        pixels.push((x, y, color));
                    }
                }
                let advance = font.advance(sprite, i);
                (g.ch, GlyphPixels { pixels, advance })
            })
            .collect();
        Some(PixelFontView {
            cell: (sprite.width, sprite.height),
            metrics: font.metrics,
            space_width: font.space_width,
            missing_width: font.missing_width(),
            glyphs,
        })
    }
}
```

Si `Palette::get` devuelve `Option<Rgba>` está bien así; si devuelve `Rgba`, envolver en `Some`.
Si `Rgba::TRANSPARENT` no es `const`, usar `Rgba::new(0, 0, 0, 0)`.

En `crates/cs-core/src/document.rs`, al final de `struct Artboard`:

```rust
    /// Si es una fuente pixel: sus medidas y sus glifos (un frame por glifo).
    #[serde(default)]
    pub font: Option<FontInfo>,
```

y agregar `font: None,` en cada literal de la lista de **Files** (y los que marque el compilador).

- [ ] **Step 4: Correr las pruebas**

Run: `cmd //c "start /low /b /wait cargo test -p cs-core font"` → PASS.
Run: `cmd //c "start /low /b /wait cargo build --workspace --all-targets"` → compila.

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "Fuentes pixel: el modelo (FontInfo, un frame por glifo) en el artboard

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Los comandos de la fuente (`cs-commands`)

**Files:**
- Modify: `crates/cs-commands/src/doc_command.rs` (variante `SetFont`, su `apply` y su etiqueta)
- Create: `crates/cs-commands/src/font.rs` (y `pub mod font;` en `crates/cs-commands/src/lib.rs`)

**Interfaces:**
- Consumes: Task 1 (`FontInfo`, `FontMode`, `GlyphInfo`, `font_sprite`).
- Produces:
  - `DocCommand::SetFont { id: ArtboardId, font: Option<Box<FontInfo>> }` (etiqueta `"Font"`)
  - `cs_commands::font::new_font_artboard(doc: &Document, name: &str, cell: (u32, u32), mode: FontMode, position: (i32, i32)) -> (ArtboardId, DocCommand)`
  - `cs_commands::font::add_glyphs(artboard: &Artboard, chars: &[char]) -> Option<DocCommand>`
  - `cs_commands::font::remove_glyph(artboard: &Artboard, ch: char) -> Option<DocCommand>`
  - `cs_commands::font::set_advance(artboard: &Artboard, ch: char, advance: Option<u32>) -> Option<DocCommand>`

- [ ] **Step 1: Pruebas que fallan** — en `crates/cs-commands/src/font.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{ColorMode, Document, Sprite};

    fn doc_with_font() -> (Document, ArtboardId) {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (id, cmd) = new_font_artboard(&doc, "Mine", (16, 16), FontMode::OneBit, (40, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, id)
    }

    #[test]
    fn a_new_font_artboard_has_one_empty_frame_per_ascii_glyph() {
        let (doc, id) = doc_with_font();
        let a = doc.artboard(id).unwrap();
        let font = a.font.as_ref().unwrap();
        assert_eq!(a.name, "Mine");
        assert_eq!(a.position, (40, 0));
        assert_eq!(a.sprite.frames.len(), font.glyphs.len());
        assert!(a.sprite.cels.is_empty());
    }

    #[test]
    fn adding_glyphs_inserts_frames_in_order_and_undoes() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let cmd = add_glyphs(doc.artboard(id).unwrap(), &['é', 'á', 'A']).unwrap();
        let undo = cmd.apply(&mut doc).unwrap();
        let a = doc.artboard(id).unwrap();
        let font = a.font.as_ref().unwrap();
        assert_eq!(font.glyphs.len(), 96, "'A' was already there");
        assert_eq!(font.glyphs[94].ch, 'á');
        assert_eq!(font.glyphs[95].ch, 'é');
        assert_eq!(a.sprite.frames.len(), 96);
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        assert!(add_glyphs(doc.artboard(id).unwrap(), &['A']).is_none());
    }

    #[test]
    fn removing_a_glyph_removes_its_frame_and_undoes() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let undo = remove_glyph(doc.artboard(id).unwrap(), 'A')
            .unwrap()
            .apply(&mut doc)
            .unwrap();
        let a = doc.artboard(id).unwrap();
        assert_eq!(a.font.as_ref().unwrap().index_of('A'), None);
        assert_eq!(a.sprite.frames.len(), 93);
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        assert!(remove_glyph(doc.artboard(id).unwrap(), 'á').is_none());
    }

    #[test]
    fn set_advance_and_set_font_roundtrip() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let undo = set_advance(doc.artboard(id).unwrap(), 'A', Some(7))
            .unwrap()
            .apply(&mut doc)
            .unwrap();
        let font = doc.artboard(id).unwrap().font.clone().unwrap();
        assert_eq!(font.glyphs[font.index_of('A').unwrap()].advance, Some(7));
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        let json = serde_json::to_string(&DocCommand::SetFont { id, font: None }).unwrap();
        assert!(serde_json::from_str::<DocCommand>(&json).is_ok());
    }
}
```

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-commands font"` → no compila.

- [ ] **Step 3: Implementar**

En `crates/cs-commands/src/doc_command.rs`, en el enum `DocCommand` (junto a `SetSymmetry`):

```rust
    /// Lo de fuente de un artboard (`None`: deja de ser fuente).
    SetFont {
        id: ArtboardId,
        font: Option<Box<FontInfo>>,
    },
```

en el `match` de `apply` (junto a `SetSymmetry`, ~142):

```rust
            DocCommand::SetFont { id, font } => {
                let artboard = doc
                    .artboard_mut(*id)
                    .ok_or(CmdError::ArtboardNotFound(*id))?;
                let old = std::mem::replace(&mut artboard.font, font.as_deref().cloned());
                Ok(DocCommand::SetFont { id: *id, font: old.map(Box::new) })
            }
```

y en `label` (~209-223): `DocCommand::SetFont { .. } => "Font",`. Importar `FontInfo` de
`cs_core`.

`crates/cs-commands/src/font.rs` (arriba de las pruebas):

```rust
//! Los cambios de una fuente pixel: crearla, agregar o quitar glifos (un frame cada uno) y sus
//! anchos. Pintar un glifo es editar un cel, como siempre.

use cs_core::{Artboard, ArtboardId, Document, FontInfo, FontMode, Frame, GlyphInfo, font_sprite};

use crate::{Command, DocCommand};

/// El artboard de una fuente nueva en `position` (con los ASCII imprimibles vacíos) y el comando
/// que lo agrega al final.
pub fn new_font_artboard(
    doc: &Document,
    name: &str,
    cell: (u32, u32),
    mode: FontMode,
    position: (i32, i32),
) -> (ArtboardId, DocCommand) {
    let id = doc.next_artboard_id();
    let font = FontInfo::new(name, cell.1, mode);
    let artboard = Artboard {
        id,
        name: name.to_owned(),
        position,
        sprite: font_sprite(cell, mode, font.glyphs.len()),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: false,
        font: Some(font),
    };
    let cmd = DocCommand::AddArtboard { index: doc.artboards.len(), artboard: Box::new(artboard) };
    (id, cmd)
}

fn set_font(artboard: &Artboard, font: FontInfo) -> DocCommand {
    DocCommand::SetFont { id: artboard.id, font: Some(Box::new(font)) }
}

/// Agrega los glifos de `chars` que no estén (vacíos, en su lugar); `None` si no falta ninguno.
pub fn add_glyphs(artboard: &Artboard, chars: &[char]) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let mut chars: Vec<char> = chars.to_vec();
    chars.sort_unstable();
    chars.dedup();
    let mut cmds = Vec::new();
    for ch in chars {
        if let Err(index) = font.insert_index(ch) {
            font.glyphs.insert(index, GlyphInfo { ch, advance: None });
            cmds.push(DocCommand::OnArtboard {
                id: artboard.id,
                cmd: Command::AddFrame { index, frame: Frame { duration_ms: 100 } },
            });
        }
    }
    if cmds.is_empty() {
        return None;
    }
    cmds.push(set_font(artboard, font));
    Some(DocCommand::Batch(cmds))
}

/// Quita el glifo de `ch` y su frame; `None` si no está o es el último.
pub fn remove_glyph(artboard: &Artboard, ch: char) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs.len() < 2 {
        return None;
    }
    font.glyphs.remove(index);
    Some(DocCommand::Batch(vec![
        DocCommand::OnArtboard { id: artboard.id, cmd: Command::RemoveFrame { index } },
        set_font(artboard, font),
    ]))
}

/// El ancho del glifo de `ch` (`None`: automático); `None` si no cambia nada.
pub fn set_advance(artboard: &Artboard, ch: char, advance: Option<u32>) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs[index].advance == advance {
        return None;
    }
    font.glyphs[index].advance = advance;
    Some(set_font(artboard, font))
}
```

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-commands"` → PASS (todas).

- [ ] **Step 5: Commit** — `git add crates/cs-commands && git commit -m "Fuentes pixel: los comandos (crear, agregar y quitar glifos, anchos)…"` (con el `Co-Authored-By`).

---

### Task 3: La fuente en el `.chimp` (`cs-io`)

**Files:**
- Modify: `crates/cs-io/src/chimp.rs` (`struct Entry` 34-50, `write` ~197-206, `read` ~261-269, prueba ~558)

**Interfaces:**
- Consumes: Task 1 (`Artboard.font`).
- Produces: el `.chimp` (y el autosave, que usa `chimp::write`/`read`) guarda y lee la fuente.

- [ ] **Step 1: Prueba que falla** — en el `mod tests` de `chimp.rs`, junto a `library_roundtrip`:

```rust
    #[test]
    fn a_font_artboard_roundtrips_with_its_glyphs() {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        font.glyphs[0].advance = Some(3);
        let mut sprite = font_sprite((8, 8), FontMode::OneBit, font.glyphs.len());
        let layer = sprite.layers[0].id;
        let mut image = Image::new(8, 8, sprite.color_mode);
        image.set(2, 3, ONE_BIT_INK).unwrap();
        let id = sprite.next_image_id();
        sprite.images.insert(id, std::sync::Arc::new(image));
        sprite.cels.insert((layer, 5), Cel { position: (0, 0), opacity: 255, image: id });
        doc.artboards.push(Artboard {
            id: ArtboardId(2),
            name: "Mine".into(),
            position: (20, 0),
            sprite,
            guides: Vec::new(),
            symmetry: Default::default(),
            in_library: false,
            font: Some(font.clone()),
        });
        let bytes = write(&doc).unwrap();
        let back = read(&bytes).unwrap();
        let a = back.artboard(ArtboardId(2)).unwrap();
        assert_eq!(a.font.as_ref(), Some(&font));
        assert_eq!(a.sprite.frames.len(), font.glyphs.len());
        assert_eq!(a.sprite.pixel(a.sprite.layers[0].id, 5, 2, 3), ONE_BIT_INK);
        assert!(back.artboard(ArtboardId(1)).unwrap().font.is_none());
    }
```

(Ajustar `write`/`read` a las firmas reales del módulo: mirar cómo los llama `library_roundtrip`.)

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-io font"` → falla (la fuente se pierde).

- [ ] **Step 3: Implementar** — en `struct Entry`:

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<FontInfo>,
```

en `write`, en el `Entry { … }`: `font: artboard.font.clone(),`; en `read`, en el
`Artboard { … }`: `font: entry.font,` (o el nombre de la variable que use el bucle).

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-io"` → PASS.

- [ ] **Step 5: Commit** — "Fuentes pixel: el .chimp guarda la fuente del artboard".

---

### Task 4: Escribir con una fuente del documento (`cs-tools::text`)

**Files:**
- Modify: `crates/cs-tools/src/text.rs`
- Modify (arms nuevos donde el compilador los pida): `crates/cs-agent/src/ops/draw.rs`,
  `crates/cs-ui/src/panels/options_bar.rs`, `crates/cs-ui/src/dafont.rs`

**Interfaces:**
- Consumes: Task 1 (`PixelFontView`, `GlyphPixels`).
- Produces:
  - `FontChoice::Document { artboard: u32, name: String }` (`name()` = `name`; `chars()` vacío;
    `vector()` = `None`)
  - `pub fn font_text(view: &PixelFontView, text: &str, scale: u32) -> Vec<(i32, i32, Option<Rgba>)>`
    (coordenadas desde la esquina de arriba a la izquierda del texto; `None` = tinta)

- [ ] **Step 1: Pruebas que fallan** — en el `mod tests` de `text.rs`:

```rust
    fn view_with(glyphs: &[(char, &[(u32, u32)], u32)]) -> cs_core::PixelFontView {
        cs_core::PixelFontView {
            cell: (8, 8),
            metrics: cs_core::FontMetrics::for_cell(8),
            space_width: 3,
            missing_width: 4,
            glyphs: glyphs
                .iter()
                .map(|(ch, px, advance)| {
                    let pixels = px.iter().map(|&(x, y)| (x, y, None)).collect();
                    (*ch, cs_core::GlyphPixels { pixels, advance: *advance })
                })
                .collect(),
        }
    }

    #[test]
    fn font_text_places_glyphs_by_their_advance() {
        let view = view_with(&[('a', &[(0, 5)], 3), ('b', &[(1, 4)], 4)]);
        let out = font_text(&view, "ab a", 1);
        // a en 0, b en 3, espacio de 3, a en 3 + 4 + 3 = 10.
        assert_eq!(out, vec![(0, 5, None), (4, 4, None), (10, 5, None)]);
    }

    #[test]
    fn font_text_scales_and_breaks_lines_by_the_cell_height() {
        let view = view_with(&[('a', &[(0, 0)], 2)]);
        let out = font_text(&view, "a\na", 2);
        assert_eq!(out.len(), 8, "two 2×2 blocks");
        assert!(out.contains(&(0, 0, None)) && out.contains(&(1, 1, None)));
        assert!(out.contains(&(0, 16, None)), "the second line, one cell (8) × 2 down");
    }

    #[test]
    fn a_missing_char_is_a_crossed_box() {
        let view = view_with(&[]);
        let out = font_text(&view, "z", 1);
        assert!(!out.is_empty());
        let right = out.iter().map(|p| p.0).max().unwrap();
        assert!(right < view.missing_width as i32);
    }
```

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-tools text"` → no compila.

- [ ] **Step 3: Implementar** — en `FontChoice`:

```rust
    /// Una fuente pixel del documento (un artboard de fuente). La UI le pasa su
    /// [`PixelFontView`](cs_core::PixelFontView) al escribir.
    Document { artboard: u32, name: String },
```

`name()`: `FontChoice::Document { name, .. } => name.clone()`. `chars()`: con `Document`, lista
vacía (agregar el arm antes del `match self.vector()`). `vector()`: `Document` → `None`.
`rasterize` con `Document` devuelve `Vec::new()` (quien escribe con una fuente del documento usa
`font_text`).

Al final del módulo (antes de las pruebas):

```rust
/// Los pixeles de `text` con la fuente pixel `view` a escala `scale`, desde la esquina de arriba
/// a la izquierda: `None` es la tinta (1 bit), `Some` el color del glifo. Cada línea mide la celda.
/// Un carácter que falta es un cuadrito tachado del alto de las mayúsculas, apoyado en la baseline.
pub fn font_text(
    view: &cs_core::PixelFontView,
    text: &str,
    scale: u32,
) -> Vec<(i32, i32, Option<cs_core::Rgba>)> {
    let s = scale.max(1) as i32;
    let mut out = Vec::new();
    let mut put = |x: i32, y: i32, c: Option<cs_core::Rgba>| {
        for dy in 0..s {
            for dx in 0..s {
                out.push((x * s + dx, y * s + dy, c));
            }
        }
    };
    let (cell_h, m) = (view.cell.1 as i32, view.metrics);
    for (row, line) in text.lines().enumerate() {
        let top = row as i32 * cell_h;
        let mut pen = 0i32;
        for ch in line.chars() {
            if ch == ' ' {
                pen += view.space_width as i32;
                continue;
            }
            match view.glyphs.get(&ch) {
                Some(g) => {
                    for &(x, y, c) in &g.pixels {
                        put(pen + x as i32, top + y as i32, c);
                    }
                    pen += g.advance as i32;
                }
                None => {
                    // El cuadrito tachado: borde y una diagonal.
                    let w = view.missing_width.saturating_sub(1).max(2) as i32;
                    let h = m.cap_height.max(2) as i32;
                    let y0 = top + m.baseline as i32 - h;
                    for y in 0..h {
                        for x in 0..w {
                            let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
                            if edge || x * (h - 1) == (h - 1 - y) * (w - 1) {
                                put(pen + x, y0 + y, None);
                            }
                        }
                    }
                    pen += view.missing_width as i32;
                }
            }
        }
    }
    out
}
```

Ajustar el orden de la primera prueba a lo que salga (ordena `out` antes de comparar si
hace falta: `out.sort()` en la prueba, no en la función). En `cs-agent/src/ops/draw.rs` no hay un
`FontArg` para fuentes del documento: si algún `match` sobre `FontChoice` deja de compilar,
agregar `FontChoice::Document { .. } => …` con el comportamiento de `Pixel`. Igual en
`options_bar.rs` y `dafont.rs`.

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-tools"` y
`cargo build --workspace --all-targets` → PASS / compila.

- [ ] **Step 5: Commit** — "Fuentes pixel: escribir con una fuente del documento (font_text)".

---

### Task 5: La fuente en el `Editor` (`cs-ui`)

**Files:**
- Create: `crates/cs-ui/src/editor/font_ops.rs` (y `mod font_ops;` en `editor.rs` junto a los
  demás `mod …_ops`)
- Modify: `crates/cs-ui/src/editor.rs` (campo `glyph_clipboard`)
- Modify: `crates/cs-ui/src/editor/gestures.rs:127-160` (`text_command` y `color`)
- Create: `crates/cs-ui/src/editor/font_tests.rs` (`#[cfg(test)] mod font_tests;` en `editor.rs`)

**Interfaces:**
- Consumes: Tasks 1, 2, 4.
- Produces (todos `impl Editor`):
  - `pub fn new_font(&mut self, name: &str, cell: (u32, u32), mode: FontMode) -> bool`
  - `pub fn font(&self) -> Option<&FontInfo>` (la del artboard activo)
  - `pub fn current_glyph(&self) -> Option<char>`
  - `pub fn go_to_glyph(&mut self, ch: char) -> bool`
  - `pub fn add_glyphs(&mut self, chars: &[char]) -> bool`
  - `pub fn remove_glyph(&mut self, ch: char) -> bool`
  - `pub fn set_glyph_advance(&mut self, ch: char, advance: Option<u32>) -> bool`
  - `pub fn set_font_info(&mut self, info: FontInfo) -> bool`
  - `pub fn glyph_advance(&self) -> Option<u32>` (el efectivo del glifo actual)
  - `pub fn shift_glyph(&mut self, dx: i32, dy: i32) -> bool`
  - `pub fn copy_glyph(&mut self)`, `pub fn paste_glyph(&mut self) -> bool`, `pub fn clear_glyph(&mut self) -> bool`
  - `pub fn document_fonts(&self) -> Vec<(ArtboardId, String)>` (para el selector de Texto)

- [ ] **Step 1: Pruebas que fallan** — `crates/cs-ui/src/editor/font_tests.rs`:

```rust
use cs_core::{ColorMode, FontMode, ONE_BIT_INK, PixelValue, Sprite};

use super::Editor;

fn editor_with_font() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
    e
}

#[test]
fn a_new_font_is_active_with_its_first_glyph() {
    let e = editor_with_font();
    assert_eq!(e.font().unwrap().name, "Mine");
    assert_eq!(e.current_glyph(), Some('!'));
    assert_eq!(e.document.artboards.len(), 2);
}

#[test]
fn going_to_a_glyph_is_changing_the_frame() {
    let mut e = editor_with_font();
    assert!(e.go_to_glyph('A'));
    assert_eq!(e.current_glyph(), Some('A'));
    assert!(!e.go_to_glyph('á'));
    e.next_frame();
    assert_eq!(e.current_glyph(), Some('B'));
}

#[test]
fn one_bit_fonts_paint_with_the_ink_whatever_the_color() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    e.primary = PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
    assert_eq!(e.color(), ONE_BIT_INK);
}

#[test]
fn glyphs_are_added_removed_and_their_advance_set_with_undo() {
    let mut e = editor_with_font();
    assert!(e.add_glyphs(&['á']));
    assert!(e.go_to_glyph('á'));
    assert!(e.set_glyph_advance('á', Some(5)));
    assert_eq!(e.glyph_advance(), Some(5));
    e.undo();
    assert_eq!(e.glyph_advance(), Some(4), "empty: half the 8 px cell");
    assert!(e.remove_glyph('á'));
    assert_eq!(e.font().unwrap().index_of('á'), None);
    e.undo();
    assert!(e.font().unwrap().index_of('á').is_some());
}

#[test]
fn shift_copy_paste_and_clear_a_glyph() {
    let mut e = editor_with_font();
    e.go_to_glyph('A');
    let layer = e.layer;
    let frame = e.frame;
    e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(1, 1, ONE_BIT_INK)],
    });
    assert!(e.shift_glyph(1, 0));
    assert_eq!(e.sprite().pixel(layer, frame, 2, 1), ONE_BIT_INK);
    e.copy_glyph();
    e.go_to_glyph('B');
    assert!(e.paste_glyph());
    assert_eq!(e.sprite().pixel(layer, e.frame, 2, 1), ONE_BIT_INK);
    assert!(e.clear_glyph());
    assert_ne!(e.sprite().pixel(layer, e.frame, 2, 1), ONE_BIT_INK);
}

#[test]
fn the_text_tool_writes_with_a_document_font() {
    let mut e = editor_with_font();
    let font_id = e.active;
    e.go_to_glyph('A');
    let (layer, frame) = (e.layer, e.frame);
    e.execute(cs_commands::Command::DrawPixels {
        layer,
        frame,
        pixels: vec![(0, 0, ONE_BIT_INK)],
    });
    assert_eq!(e.document_fonts(), vec![(font_id, "Mine".to_owned())]);
    e.activate(cs_core::ArtboardId(1));
    e.text_style.font = cs_tools::text::FontChoice::Document {
        artboard: font_id.0,
        name: "Mine".into(),
    };
    e.primary = PixelValue::Rgba(cs_core::Rgba::rgb(0, 255, 0));
    e.text = Some(crate::editor::TextDraft { origin: (2, 3), text: "A".into(), ..Default::default() });
    let Some(cs_commands::Command::DrawPixels { pixels, .. }) = e.text_command() else {
        panic!("a draw command");
    };
    assert_eq!(pixels, vec![(2, 3, e.primary)]);
}
```

(Si `TextDraft` no deriva `Default`, construirlo con sus campos reales — ver su definición en
`editor.rs` — o derivarle `Default`. Si `color()`/`text_command()` son `pub(super)`, la prueba vive
dentro de `editor`, así que las ve.)

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui font_tests"` → no compila.

- [ ] **Step 3: Implementar** — `crates/cs-ui/src/editor/font_ops.rs`:

```rust
//! Lo de las fuentes pixel en el editor: crear una, moverse entre glifos, agregar y quitar,
//! anchos, y desplazar, copiar, pegar y borrar un glifo.

use std::sync::Arc;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Cel, FontInfo, FontMode};

use super::Editor;

impl Editor {
    /// Una fuente nueva a la derecha de todo, activa y en su primer glifo.
    pub fn new_font(&mut self, name: &str, cell: (u32, u32), mode: FontMode) -> bool {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let (id, cmd) =
            cs_commands::font::new_font_artboard(&self.document, name, cell, mode, (x1 + 16, y0));
        if !self.execute_doc(cmd) {
            return false;
        }
        self.activate(id);
        self.set_frame(0);
        true
    }

    /// La fuente del artboard activo.
    pub fn font(&self) -> Option<&FontInfo> {
        self.artboard().font.as_ref()
    }

    /// El carácter del glifo actual (el frame activo).
    pub fn current_glyph(&self) -> Option<char> {
        self.font()?.glyphs.get(self.frame).map(|g| g.ch)
    }

    pub fn go_to_glyph(&mut self, ch: char) -> bool {
        let Some(index) = self.font().and_then(|f| f.index_of(ch)) else {
            return false;
        };
        self.set_frame(index);
        true
    }

    pub fn add_glyphs(&mut self, chars: &[char]) -> bool {
        cs_commands::font::add_glyphs(self.artboard(), chars).is_some_and(|c| self.execute_doc(c))
    }

    pub fn remove_glyph(&mut self, ch: char) -> bool {
        cs_commands::font::remove_glyph(self.artboard(), ch).is_some_and(|c| self.execute_doc(c))
    }

    pub fn set_glyph_advance(&mut self, ch: char, advance: Option<u32>) -> bool {
        cs_commands::font::set_advance(self.artboard(), ch, advance)
            .is_some_and(|c| self.execute_doc(c))
    }

    pub fn set_font_info(&mut self, info: FontInfo) -> bool {
        if self.font() == Some(&info) {
            return false;
        }
        self.execute_doc(DocCommand::SetFont { id: self.active, font: Some(Box::new(info)) })
    }

    /// El ancho efectivo del glifo actual.
    pub fn glyph_advance(&self) -> Option<u32> {
        let font = self.font()?;
        (self.frame < font.glyphs.len()).then(|| font.advance(self.sprite(), self.frame))
    }

    /// Mueve lo dibujado del glifo actual (su cel) `dx`, `dy` pixeles.
    pub fn shift_glyph(&mut self, dx: i32, dy: i32) -> bool {
        let (layer, frame) = (self.layer, self.frame);
        let Some(cel) = self.sprite().cel(layer, frame).copied() else {
            return false;
        };
        let moved = Cel { position: (cel.position.0 + dx, cel.position.1 + dy), ..cel };
        self.execute(Command::SetCel { layer, frame, cel: Some(moved) })
    }

    pub fn copy_glyph(&mut self) {
        let (layer, frame) = (self.layer, self.frame);
        self.glyph_clipboard = self.sprite().cel(layer, frame).and_then(|cel| {
            let image = self.sprite().image(cel.image)?.clone();
            Some((image, cel.position))
        });
    }

    pub fn paste_glyph(&mut self) -> bool {
        let Some((image, position)) = self.glyph_clipboard.clone() else {
            return false;
        };
        let (layer, frame) = (self.layer, self.frame);
        let id = self.sprite().next_image_id();
        self.execute(Command::Batch(vec![
            Command::SetImage { id, image: Some(image) },
            Command::SetCel { layer, frame, cel: Some(Cel { position, opacity: 255, image: id }) },
        ]))
    }

    pub fn clear_glyph(&mut self) -> bool {
        let (layer, frame) = (self.layer, self.frame);
        self.sprite().cel(layer, frame).is_some()
            && self.execute(Command::SetCel { layer, frame, cel: None })
    }

    /// Las fuentes del documento (para el selector de la herramienta Texto).
    pub fn document_fonts(&self) -> Vec<(ArtboardId, String)> {
        self.document
            .artboards
            .iter()
            .filter_map(|a| Some((a.id, a.font.as_ref()?.name.clone())))
            .collect()
    }
}

/// Lo que guarda Copy glyph: la imagen del cel y dónde iba.
pub type GlyphClipboard = Option<(Arc<cs_core::Image>, (i32, i32))>;
```

Ajustes según las firmas reales: `self.sprite().image(id)` devuelve `Option<&Arc<Image>>`
(usar `.cloned()` si hace falta); `Command::SetImage`/`SetCel` con los campos de
`crates/cs-commands/src/command.rs:65-160`.

En `editor.rs`, en `struct Editor`: `pub(crate) glyph_clipboard: font_ops::GlyphClipboard,` e
inicializarlo en `None` en `Editor::new` (~375).

En `gestures.rs`, `color()` (~151):

```rust
    pub(super) fn color(&self) -> PixelValue {
        // Una fuente de 1 bit solo prende o apaga pixeles.
        if self.font().is_some_and(|f| f.mode == cs_core::FontMode::OneBit) {
            return if self.painting_secondary {
                cs_core::ONE_BIT_PAPER
            } else {
                cs_core::ONE_BIT_INK
            };
        }
        if self.painting_secondary { self.secondary } else { self.primary }
    }
```

y en `context()` (~189), el `secondary` del `ToolContext` con la misma regla (papel o tinta, el
contrario de `color()`). En `text_command` (~127), antes del `text_pixels`:

```rust
        if let cs_tools::text::FontChoice::Document { artboard, .. } = &self.text_style.font {
            let view = self.document.artboard(cs_core::ArtboardId(*artboard))?.font_view()?;
            let scale = self.text_style.size.clamp(1, 8);
            let (ox, oy) = draft.origin;
            let pixels = cs_tools::text::font_text(&view, &draft.text, scale)
                .into_iter()
                .map(|(x, y, c)| {
                    (ox + x, oy + y, c.map_or(self.primary, |c| self.value_of(c)))
                })
                .collect();
            return Some(Command::DrawPixels { layer: self.layer, frame: self.frame, pixels });
        }
```

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` → PASS (todas).

- [ ] **Step 5: Commit** — "Fuentes pixel: crear, recorrer y editar glifos desde el editor; 1 bit pinta con la tinta".

---

### Task 6: El artboard de fuente en el lienzo

**Files:**
- Create: `crates/cs-ui/src/font_overlay.rs` (y `mod font_overlay;` en `lib.rs`)
- Modify: `crates/cs-ui/src/canvas.rs` (dibujar el overlay en el bucle de artboards ~753-851, antes
  del `continue` de Pixel UI en ~824; `label_text` ~1327; `artboard_menu` ~1547)
- Modify: `crates/cs-ui/src/canvas/input.rs` (arrastrar la línea del ancho, como `guide_input` ~131)
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: Tasks 1, 5.
- Produces:
  - `font_overlay::shapes(painter: &egui::Painter, pixels: Rect, font: &FontInfo, glyph: char, advance: u32, scale: f32) -> Vec<Shape>` — guías, fantasma y línea del ancho del glifo, sobre el área de pixeles `pixels` (en pantalla) con `scale` puntos por pixel
  - `font_overlay::advance_x(pixels: Rect, advance: u32, scale: f32) -> f32`
  - `font_overlay::GUIDE_RED: Color32 = Color32::from_rgb(230, 40, 60)` y `GUIDE_DIM: Color32 = Color32::from_rgba_unmultiplied(230, 40, 60, 90)`

- [ ] **Step 1: Pruebas que fallan** — en `layout_tests.rs` (usar `LayoutHarness::new(pixel_ui(), true)`):

```rust
#[test]
fn a_font_artboard_shows_its_glyph_in_the_title_and_hides_the_timeline() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.doc.editor.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.app.doc.editor.go_to_glyph('A');
    h.frame();
    assert!(h.texts.iter().any(|t| t.contains("A · Mine")), "{:?}", h.texts);
    assert!(h.panel("timeline").width() == 0.0 || !h.app.view_timeline_shown());
}

#[test]
fn dragging_the_advance_line_sets_a_manual_advance() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    e.go_to_glyph('A');
    h.frame();
    let line = h.app.doc.canvas.advance_line_rect().expect("the advance line");
    let scale = h.app.doc.canvas.scale().x;
    h.drag(line.center(), line.center() + egui::vec2(3.0 * scale, 0.0));
    assert_eq!(h.app.doc.editor.glyph_advance(), Some(8 + 3));
    assert_eq!(h.app.doc.editor.font().unwrap().glyphs[h.app.doc.editor.frame].advance, Some(11));
}
```

Ajustar a lo que exista en `LayoutHarness`: si no hay `drag`, agregarlo (press en `from`,
`PointerMoved` a `to`, release) junto a `click` (~layout_tests.rs:20-306). Para la timeline,
comparar con la prueba `the_dock_timeline_button_hides_and_shows_the_timeline` (~846): usar el
mismo criterio con que esa prueba ve si la timeline está (`h.panel("timeline")`).
`advance_line_rect()` es un `pub(crate) fn` nuevo de `CanvasView` que devuelve el rect de golpe
de la línea del ancho del artboard activo (lo usa la prueba y `input.rs`).

- [ ] **Step 2: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui font_artboard dragging_the_advance"` → falla.

- [ ] **Step 3: Implementar**

`font_overlay.rs`:

```rust
//! Lo que el lienzo dibuja sobre un artboard de fuente (no son pixeles, no se exportan): la letra
//! fantasma detrás, las guías de las medidas y las líneas del borde izquierdo y del ancho.

use eframe::egui::{self, Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Shape, Stroke};

use cs_core::FontInfo;

pub const GUIDE_RED: Color32 = Color32::from_rgb(230, 40, 60);
pub const GUIDE_DIM: Color32 = Color32::from_rgba_premultiplied(90, 16, 24, 90);
const GHOST: Color32 = Color32::from_rgba_premultiplied(60, 60, 60, 60);

/// La x en pantalla de la línea del ancho.
pub fn advance_x(pixels: Rect, advance: u32, scale: f32) -> f32 {
    pixels.left() + advance as f32 * scale
}

/// Una línea horizontal en la fila `row` (su borde de arriba) de lado a lado.
fn row_line(pixels: Rect, row: u32, scale: f32, stroke: Stroke, dashed: bool) -> Vec<Shape> {
    let y = pixels.top() + row as f32 * scale;
    let (a, b) = (Pos2::new(pixels.left(), y), Pos2::new(pixels.right(), y));
    if dashed {
        Shape::dashed_line(&[a, b], stroke, 4.0, 3.0)
    } else {
        vec![Shape::line_segment([a, b], stroke)]
    }
}

/// La letra fantasma (detrás de los pixeles): `glyph` en gris, de la altura de las mayúsculas,
/// apoyada en la baseline y desde el borde izquierdo.
pub fn ghost(painter: &Painter, pixels: Rect, font: &FontInfo, glyph: char, scale: f32) -> Option<Shape> {
    if !font.ghost.enabled {
        return None;
    }
    let family = if font.ghost.monospace { FontFamily::Monospace } else { FontFamily::Proportional };
    // La altura de las mayúsculas de egui es ~0.7 del tamaño de la fuente.
    let size = font.metrics.cap_height as f32 * scale / 0.7;
    let galley = painter.layout_no_wrap(glyph.to_string(), FontId::new(size, family), GHOST);
    let baseline = pixels.top() + font.metrics.baseline as f32 * scale;
    let ascent = galley.rows.first().map_or(size, |r| r.row.height() * 0.8);
    let at = Pos2::new(pixels.left(), baseline - ascent);
    Some(Shape::galley(at, galley, GHOST))
}

/// Las guías y las líneas del borde izquierdo y del ancho (encima de los pixeles).
pub fn guides(pixels: Rect, font: &FontInfo, advance: u32, scale: f32) -> Vec<Shape> {
    let m = font.metrics;
    let strong = Stroke::new(1.0, GUIDE_RED);
    let dim = Stroke::new(1.0, GUIDE_DIM);
    let mut shapes = Vec::new();
    shapes.extend(row_line(pixels, m.baseline, scale, strong, false));
    shapes.extend(row_line(pixels, m.baseline.saturating_sub(m.cap_height), scale, dim, true));
    shapes.extend(row_line(pixels, m.baseline.saturating_sub(m.x_height), scale, dim, true));
    shapes.extend(row_line(pixels, m.baseline + m.descent, scale, dim, true));
    let left = pixels.left();
    shapes.push(Shape::line_segment([Pos2::new(left, pixels.top()), Pos2::new(left, pixels.bottom())], strong));
    let x = advance_x(pixels, advance, scale);
    shapes.push(Shape::line_segment([Pos2::new(x, pixels.top()), Pos2::new(x, pixels.bottom())], Stroke::new(2.0, GUIDE_RED)));
    shapes
}
```

En `canvas.rs`, en el bucle de artboards (~753): con `artboard.font` y el artboard activo (`artboard.id == editor.active`), dibujar `font_overlay::ghost(...)` **antes** del checker/texture (~777; ponerlo en una capa `Shape::Noop` reservada con `painter.add(Shape::Noop)` antes del checker y `painter.set(...)` después, o dibujar el fantasma encima del checker y debajo de la textura del sprite) y `font_overlay::guides(...)` después de la cuadrícula de tiles (~823), antes del `continue` de Pixel UI. `glyph` = `editor.current_glyph()`, `advance` = `editor.glyph_advance()`, `scale` = `self.scale().x`, `pixels` = `screen` (el `pixels_on_screen` del bucle). Los artboards de fuente que no están activos se dibujan como siempre (sin overlay).

`label_text` (~1327):

```rust
fn label_text(artboard: &Artboard, frame: Option<usize>) -> String {
    if let Some(font) = &artboard.font {
        let ch = frame.and_then(|f| font.glyphs.get(f)).map_or('?', |g| g.ch);
        return format!("{ch} · {}", font.name);
    }
    format!("{}  {}×{}", artboard.name, artboard.sprite.width, artboard.sprite.height)
}
```

pasando `Some(editor.frame)` si el artboard es el activo y `Some(0)` si no (actualizar las dos
llamadas: `bar_shapes` ~833 y `title_at` ~1649).

`artboard_menu` (~1547): si `editor.artboard().font.is_some()` (después del `activate`), agregar
antes de "Rename":

```rust
        if editor.artboard().font.is_some() {
            for (label, dx, dy) in [
                ("Shift Left", -1, 0),
                ("Shift Right", 1, 0),
                ("Shift Up", 0, -1),
                ("Shift Down", 0, 1),
            ] {
                if ui.add(menus::plain(label)).clicked() {
                    editor.shift_glyph(dx, dy);
                    ui.close();
                }
            }
            if ui.add(menus::plain("Copy Glyph")).clicked() {
                editor.copy_glyph();
                ui.close();
            }
            if ui.add(menus::plain("Paste Glyph")).clicked() {
                editor.paste_glyph();
                ui.close();
            }
            if ui.add(menus::plain("Clear Glyph")).clicked() {
                editor.clear_glyph();
                ui.close();
            }
            ui.add_enabled(false, menus::plain("Glyph From…"));
            menus::separator(ui);
        }
```

La línea del ancho: en `CanvasView` agregar el campo `advance_drag: Option<i32>` (el ancho en
columnas mientras se arrastra) y

```rust
    /// El rect de golpe de la línea del ancho del glifo actual (artboard de fuente activo).
    pub(crate) fn advance_line_rect(&self) -> Option<Rect> { self.advance_line }
```

guardando `self.advance_line = Some(Rect::from_center_size(Pos2::new(x, pixels.center().y), vec2(6.0, pixels.height())))` al dibujarla (y `None` si no hay fuente activa). En `canvas/input.rs`, antes de `guide_input` en `handle_input` (~415), una función `advance_input(&mut self, response, editor) -> Option<CursorIcon>` con la forma de `guide_input` (~131-239): al presionar dentro de `advance_line` empieza `advance_drag = Some(actual)`; al mover, `advance_drag = Some(((pointer.x - pixels.left()) / scale).round().max(0.0) as i32)` y la guía se dibuja en esa columna (usar `advance_drag` en vez de `glyph_advance()` en el overlay mientras dure); al soltar, `editor.set_glyph_advance(ch, Some(n as u32))` y `advance_drag = None`. Devolver `Some(CursorIcon::ResizeHorizontal)` mientras se arrastra o se pasa encima, para que las herramientas no reciban el clic (como `ruler_busy` ~425).

En `lib.rs:1284`, la condición de la timeline: `if self.view.timeline && self.doc.editor.font().is_none() { … }`.

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` → PASS.

- [ ] **Step 5: Revisar en pantalla** — build (`CARGO_TARGET_DIR=target/gui-checkN cargo build --release -p chimpsprite`), captura con `CHIMPSPRITE_SCREENSHOT` y una fuente creada (Task 10 agrega la variable de demo; mientras, crear la fuente con `File → New Font…` no existe aún: dejar la captura para la Task 10).

- [ ] **Step 6: Commit** — "Fuentes pixel: el artboard de fuente (título, guías, letra fantasma, ancho arrastrable, menú ≡) y sin timeline".

---

### Task 7: El diálogo New Font

**Files:**
- Create: `crates/cs-ui/src/font_dialog.rs` (y `mod font_dialog;` en `lib.rs`)
- Modify: `crates/cs-ui/src/lib.rs` (campo `new_font: Option<font_dialog::NewFontDialog>`, en `None` en `with_parts` ~298; `fn update_font_dialog` llamado junto a `update_tilemap_dialogs` ~1447)
- Modify: `crates/cs-ui/src/file_actions.rs:97-116` (`is_modal`: `|| self.new_font.is_some()`)
- Modify: `crates/cs-ui/src/app_menus.rs` (File, después de "New" ~702: `"New Font…"`)
- Modify: `crates/cs-ui/src/dialog_tests.rs` (llamar `update_font_dialog` en el frame del `DialogHarness` ~67-86)
- Test: `crates/cs-ui/src/dialog_tests.rs`

**Interfaces:**
- Consumes: Task 5 (`Editor::new_font`), `tilemap_dialog::{DialogResult, dialog_window, finish}`.
- Produces: `pub struct NewFontDialog { pub name: String, pub cell: (u32, u32), pub mode: FontMode }` con `Default` (`"New Font"`, `(16, 16)`, `OneBit`) y `pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult`.

- [ ] **Step 1: Prueba que falla** — en `dialog_tests.rs`, siguiendo `new_tilemap()` (~374):

```rust
#[test]
fn new_font_creates_a_font_artboard() {
    let mut h = DialogHarness::new(true);
    h.app.new_font = Some(Default::default());
    h.settle();
    assert!(h.drew_text("New Font"));
    h.click("OK");
    h.settle();
    assert!(h.app.new_font.is_none());
    let font = h.app.doc.editor.font().expect("the active artboard is a font");
    assert_eq!(font.name, "New Font");
    assert_eq!(h.app.doc.editor.sprite().width, 16);
}
```

- [ ] **Step 2: Correr** → no compila.

- [ ] **Step 3: Implementar** — `font_dialog.rs`:

```rust
//! New Font…: el nombre, la celda y el modo de una fuente pixel nueva.

use cs_core::FontMode;
use eframe::egui;

use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window, finish};
use crate::widgets;

pub struct NewFontDialog {
    pub name: String,
    pub cell: (u32, u32),
    pub mode: FontMode,
}

impl Default for NewFontDialog {
    fn default() -> Self {
        NewFontDialog { name: "New Font".into(), cell: (16, 16), mode: FontMode::OneBit }
    }
}

impl NewFontDialog {
    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let (result, open) = dialog_window(ctx, skin, "new_font", "New Font", |ui| {
            egui::Grid::new("new_font_grid").num_columns(2).show(ui, |ui| {
                ui.label("Name");
                widgets::text_line(ui, &mut self.name);
                ui.end_row();
                ui.label("Width");
                widgets::number(ui, &mut self.cell.0, 1..=64);
                ui.end_row();
                ui.label("Height");
                widgets::number(ui, &mut self.cell.1, 1..=64);
                ui.end_row();
                ui.label("Mode");
                widgets::combo(ui, "new_font_mode", mode_name(self.mode), |ui| {
                    for mode in [FontMode::OneBit, FontMode::Color] {
                        widgets::choice(ui, &mut self.mode, mode, mode_name(mode));
                    }
                });
                ui.end_row();
            });
        });
        finish(ctx, open, result)
    }
}

pub fn mode_name(mode: FontMode) -> &'static str {
    match mode {
        FontMode::OneBit => "1 bit",
        FontMode::Color => "Color",
    }
}
```

(Usar las firmas reales de `widgets::text_line`, `widgets::number`, `widgets::combo` y
`widgets::choice` tal como las usa `NewTilemapDialog::show`, `tilemap_dialog.rs:77-159`.)

`lib.rs`:

```rust
    fn update_font_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.new_font else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let (name, cell, mode) = (dialog.name.trim().to_owned(), dialog.cell, dialog.mode);
                let name = if name.is_empty() { "New Font".to_owned() } else { name };
                // La Task 8 agrega aquí `dock::set_open(ctx, Some(dock::DockPanel::Font))`
                // cuando la fuente se crea.
                self.doc.editor.new_font(&name, cell, mode);
                self.new_font = None;
            }
            tilemap_dialog::DialogResult::Cancel => self.new_font = None,
        }
    }
```

Menú File:

```rust
            if ui.add(menus::plain("New Font…")).clicked() {
                self.new_font = Some(Default::default());
                ui.close();
            }
```

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` → PASS.
- [ ] **Step 5: Commit** — "Fuentes pixel: el diálogo New Font (File → New Font…)".

---

### Task 8: El panel Font del dock — esqueleto y Map

**Files:**
- Create: `crates/cs-ui/src/font_panel.rs` (y `mod font_panel;` en `lib.rs`)
- Modify: `crates/cs-ui/src/dock.rs` (variante `Font`, `ALL` +1 en las dos versiones, `label` "Font", `icon` "icon.dock.font", `plain` "Aa", y en el `match` de app panels)
- Modify: `crates/cs-ui/src/kit/catalog.rs:1276` (`DOCK_ICONS` +1)
- Modify: `crates/cs-ui/src/lib.rs` (campo `font_panel: font_panel::FontPanelState`, `fn font_panel(&mut self, ctx, anchor, modal)` como `library_panel` ~582-598, y el arm `Some((dock::DockPanel::Font, anchor)) => self.font_panel(ui.ctx(), anchor, modal)` ~1327)
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: Task 5.
- Produces:
  - `pub const WIDTH: f32 = 300.0;`
  - `#[derive(Default)] pub struct FontPanelState { pub page: Page, pub add_char: String, pub test_text: String }` (`test_text` por defecto `"The quick brown fox"`)
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)] pub enum Page { #[default] Ascii, Latin1, LatinExtA, All }` con `fn chars(self, font: &FontInfo) -> Vec<char>` (ASCII `!`..=`~`; Latin-1 `¡`..=`ÿ`; Latin Ext-A `Ā`..=`ſ`; All = los de la fuente)
  - `pub enum FontAction { NewFont }`
  - `pub fn show(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &mut FontPanelState) -> Option<FontAction>`

- [ ] **Step 1: Pruebas que fallan** — en `layout_tests.rs`:

```rust
#[test]
fn the_font_panel_offers_new_font_and_its_map_picks_glyphs() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.click(h.button("Font").center(), PointerButton::Primary);
    h.frame();
    assert!(h.button("New Font…").width() > 0.0);
    h.app.doc.editor.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.frame();
    h.click(h.button("Glyph B").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.current_glyph(), Some('B'));
    // Doble clic en un carácter que falta (página Latin-1) lo agrega.
    h.click(h.button("Latin-1").center(), PointerButton::Primary);
    let cell = h.button("Glyph á").center();
    h.double_click(cell);
    assert!(h.app.doc.editor.font().unwrap().index_of('á').is_some());
}
```

(El panel se abre con el botón "Font" del dock, como `Library` en la prueba de
`layout_tests.rs:2765`. Si el harness no tiene `double_click`, agregarlo junto a `click`: dos
pares de `Event::PointerButton` press/release en el mismo punto, en frames seguidos.)

- [ ] **Step 2: Correr** → no compila.

- [ ] **Step 3: Implementar** — `font_panel.rs` con, en este orden y con los componentes del kit:

1. Sin fuente activa (`editor.font()` es `None`): un `kit::button::button(ui, skin, "New Font…")` que devuelve `Some(FontAction::NewFont)`, y un texto "The active artboard is not a font." Con Pixel UI apagado, `pixel_ui::text_button`.
2. Con fuente: el título de sección "Map" (como las secciones de Lumen/Groom), una tira de pestañas del kit (`kit::tabs::tab_strip`) con `["ASCII", "Latin-1", "Latin Ext-A", "All"]` que cambia `state.page`, y la cuadrícula:

```rust
/// Una celda del mapa: el carácter arriba (11 px) y la miniatura del glifo abajo.
const CELL_W: f32 = 22.0;
const CELL_H: f32 = 24.0;
const COLUMNS: usize = 13;

fn map(ui: &mut Ui, editor: &mut Editor, skin: Option<&Skin>, state: &FontPanelState) {
    let Some(font) = editor.font().cloned() else { return };
    let chars = state.page.chars(&font);
    let current = editor.current_glyph();
    let view = editor.artboard().font_view();
    let (rect, _) = ui.allocate_exact_size(
        vec2(CELL_W * COLUMNS as f32, CELL_H * chars.len().div_ceil(COLUMNS) as f32),
        Sense::hover(),
    );
    let painter = ui.painter().clone();
    let colors = skin.map(|s| *s.colors());
    for (i, ch) in chars.iter().enumerate() {
        let cell = Rect::from_min_size(
            rect.min + vec2((i % COLUMNS) as f32 * CELL_W, (i / COLUMNS) as f32 * CELL_H),
            vec2(CELL_W, CELL_H),
        )
        .shrink(1.0);
        let id = ui.id().with(("font_map", *ch));
        let response = ui.interact(cell, id, Sense::click());
        let label = format!("Glyph {ch}");
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, Some(*ch) == current, &label));
        let has = font.index_of(*ch).is_some();
        // Fondo: blanco si la fuente tiene el carácter, apagado si no; marco rosa el actual.
        let (paper, ink, accent) = colors.map_or(
            (Color32::WHITE, Color32::BLACK, Color32::from_rgb(226, 114, 133)),
            |c| (if has { Color32::WHITE } else { c.panel }, c.text, c.accent),
        );
        painter.rect_filled(cell, 0.0, paper);
        painter.text(cell.center_top() + vec2(0.0, 1.0), Align2::CENTER_TOP, ch.to_string(), TextStyle::Body.resolve(ui.style()), ink);
        if let Some(g) = view.as_ref().and_then(|v| v.glyphs.get(ch)) {
            // La miniatura: 1 pixel de pantalla por pixel del glifo, abajo y centrada.
            let (w, h) = (font_cell_w(editor), font_cell_h(editor));
            let thumb = Rect::from_center_size(cell.center_bottom() - vec2(0.0, 1.0 + h / 2.0), vec2(w, h));
            for &(x, y, c) in &g.pixels {
                let color = c.map_or(ink, |c| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
                painter.rect_filled(Rect::from_min_size(thumb.min + vec2(x as f32, y as f32), Vec2::splat(1.0)), 0.0, color);
            }
        }
        if Some(*ch) == current {
            painter.rect_stroke(cell, 0.0, Stroke::new(1.0, accent), StrokeKind::Inside);
        }
        if response.double_clicked() && !has {
            editor.add_glyphs(&[*ch]);
            editor.go_to_glyph(*ch);
        } else if response.clicked() && has {
            editor.go_to_glyph(*ch);
        }
        menus::context_menu(&response, |ui| glyph_menu(ui, editor, *ch));
    }
}
```

donde `font_cell_w/h` son el ancho y el alto del sprite (`editor.sprite().width as f32`, en
puntos: con la escala 2 de Pixel UI la miniatura sale a 2 pixeles de pantalla por pixel, que cabe
en 22×24 para celdas de hasta 10×12; para celdas más grandes, escalar con
`(CELL_W - 4.0) / w` y redondear hacia abajo, mínimo `0.5`). `glyph_menu` tiene `Copy`, `Paste`,
`Clear` (yendo primero al glifo con `go_to_glyph`) y `Remove` (`editor.remove_glyph(ch)`). Debajo
de la cuadrícula, un campo del kit `kit::input::text_input(ui, skin, &mut state.add_char, 120.0, "Add char", "Add char")` y un botón "Add" que hace `editor.add_glyphs(&state.add_char.chars().filter(|c| !c.is_whitespace()).collect::<Vec<_>>())` y limpia el campo.

`dock.rs`: variante `Font` y sus cuatro arms; `ALL` con `DockPanel::Font` al final (las dos
versiones, longitudes +1); en el `match` de app panels agregar `DockPanel::Font` a la lista que
devuelve `out.app_panel = Some((panel, anchor))`. `lib.rs`: `fn font_panel` igual a
`library_panel` (~582-598) con `font_panel::WIDTH` y `font_panel::show(ui, editor, skin, state)`,
y `Some(FontAction::NewFont) => self.new_font = Some(Default::default())`. En
`update_font_dialog` (Task 7), al aceptar: `if self.doc.editor.new_font(&name, cell, mode) {
dock::set_open(ctx, Some(dock::DockPanel::Font)); }`.

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` → PASS.
- [ ] **Step 5: Commit** — "Fuentes pixel: el panel Font del dock con el mapa de caracteres".

---

### Task 9: El panel Font — Test Drive y ajustes

**Files:**
- Modify: `crates/cs-ui/src/font_panel.rs`
- Test: `crates/cs-ui/src/layout_tests.rs`

**Interfaces:**
- Consumes: Tasks 4 (`font_text`), 5, 8.
- Produces: las secciones "Test Drive" y "Font" en `font_panel::show`.

- [ ] **Step 1: Pruebas que fallan**:

```rust
#[test]
fn the_test_drive_draws_the_text_and_clicking_a_letter_goes_to_its_glyph() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    let e = &mut h.app.doc.editor;
    e.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.app.font_panel.test_text = "AB".into();
    h.click(h.button("Font").center(), PointerButton::Primary);
    h.frame();
    let drive = h.button("Test Drive B");
    h.click(drive.center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.current_glyph(), Some('B'));
}

#[test]
fn the_font_settings_change_the_metrics_with_undo() {
    let mut h = LayoutHarness::new(pixel_ui(), true);
    h.app.doc.editor.new_font("Mine", (16, 16), cs_core::FontMode::OneBit);
    h.click(h.button("Font").center(), PointerButton::Primary);
    h.frame();
    h.click(h.button("Baseline +").center(), PointerButton::Primary);
    assert_eq!(h.app.doc.editor.font().unwrap().metrics.baseline, 13);
    h.app.doc.editor.undo();
    assert_eq!(h.app.doc.editor.font().unwrap().metrics.baseline, 12);
}
```

(Los nombres accesibles de − y + del stepper del kit se ven en `kit/stepper.rs`; usar los que
den — p. ej. el stepper recibe `id_salt` y sus botones se llaman con el nombre del campo. Ajustar
la prueba a esos nombres.)

- [ ] **Step 2: Correr** → falla.

- [ ] **Step 3: Implementar**:

- **Test Drive:** título de sección; `kit::input::text_input(ui, skin, &mut state.test_text, WIDTH - 16.0, "Test text", "Type to test")`; debajo, el texto con `cs_tools::text::font_text(&view, &state.test_text, 2)` pintado en un rect blanco de `WIDTH - 16` de ancho y `cell_h * 2 * líneas + 8` de alto (cada pixel un `rect_filled` de 1×1 punto; tinta `colors.text`, color del glifo si `Some`). Para ir a un glifo: recorrer los caracteres sumando `advance * 2` (espacio: `space_width * 2`, faltante: `missing_width * 2`) y crear con `ui.interact` un rect por letra con el nombre accesible `"Test Drive {ch}"`; clic → `editor.go_to_glyph(ch)`.
- **Font:** título de sección; filas `etiqueta + control` del kit:
  - "Name": `kit::input::buffered` + `text_input` (confirmar al salir) → `set_font_info`.
  - "Mode": texto `font_dialog::mode_name(font.mode)`; "Cell": `"{w}×{h}"`.
  - "Baseline", "Cap height", "x-height", "Descent": `kit::stepper::stepper(ui, skin, 80.0, "<nombre>", &mut v, 0..=cell_h, 1, None)`; si cambia, `editor.set_font_info(FontInfo { metrics: …, ..font.clone() })`.
  - "Letter spacing" (0..=8) y "Space width" (0..=cell_w): steppers igual.
  - "Monospace": `kit::check::checkbox`.
  - "Ghost": `kit::check::checkbox` (enabled) y un `kit::select` con "Ubuntu"/"Hack" (`ghost.monospace`).
  - "Auto width" para el glifo actual: `kit::check::checkbox` prendido si `advance` es `None`; al prenderlo, `set_glyph_advance(ch, None)`; al apagarlo, `set_glyph_advance(ch, Some(editor.glyph_advance()))`.
  - Botón "Build ▾" deshabilitado (`ui.add_enabled_ui(false, …)`) con tooltip "Coming soon".

Cada cambio es un `set_font_info` (un paso de undo); los steppers que se arrastran se confirman
al soltar si el stepper lo permite (si no, cada clic es un paso, como la opacidad del timeline).

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` → PASS.
- [ ] **Step 5: Commit** — "Fuentes pixel: Test Drive y ajustes de la fuente en el panel Font".

---

### Task 10: La herramienta Texto lista las fuentes del documento + demo y verificación

**Files:**
- Modify: `crates/cs-ui/src/panels/options_bar.rs:738-830` (`text_style_options`)
- Modify: `crates/cs-ui/src/demo.rs` (`CHIMPSPRITE_DEMO_FONT`)
- Test: `crates/cs-ui/src/panels/tests.rs` (`BarHarness`)

**Interfaces:**
- Consumes: Tasks 4, 5, 8.
- Produces: el selector "Font" incluye `FontChoice::Document { artboard, name }` por cada fuente de `editor.document_fonts()`; `CHIMPSPRITE_DEMO_FONT` crea una fuente "Demo Font" de 8×8 con unas letras dibujadas y abre el panel Font.

- [ ] **Step 1: Prueba que falla** — en `panels/tests.rs`, con `BarHarness`:

```rust
#[test]
fn the_text_tool_lists_the_document_fonts() {
    let mut h = BarHarness::with_skin(1200.0, true);
    h.editor.new_font("Mine", (8, 8), cs_core::FontMode::OneBit);
    h.editor.activate(cs_core::ArtboardId(1));
    h.editor.set_tool(ToolKind::Text);
    h.frame(vec![]);
    h.open("Pixel 5×7");
    h.click("Mine");
    assert_eq!(h.editor.text_style.font.name(), "Mine");
}
```

(Usar el nombre real del setter de herramienta y de los campos de `BarHarness`.)

- [ ] **Step 2: Correr** → falla.

- [ ] **Step 3: Implementar** — en `text_style_options` (~740):

```rust
    let mut fonts = vec![FontChoice::Pixel, FontChoice::Hack, FontChoice::Ubuntu];
    fonts.extend(editor.document_fonts().into_iter().map(|(id, name)| FontChoice::Document {
        artboard: id.0,
        name,
    }));
    if let FontChoice::Custom { .. } = style.font {
        fonts.push(style.font.clone());
    }
```

y en `pick`, al elegir una `Document`: `style.size = 1` (escala entera, como Pixel); donde se
pregunta `== FontChoice::Pixel` para mostrar "Scale" en vez de "Size" (~812, ~825), usar
`matches!(style.font, FontChoice::Pixel | FontChoice::Document { .. })`.

`demo.rs`, en `startup` (junto a los demás):

```rust
    // Desarrollo: una fuente de 8×8 con unas letras y el panel Font abierto, para capturas.
    if dev_var("CHIMPSPRITE_DEMO_FONT").is_some() && app.has_doc() {
        let e = &mut app.doc.editor;
        if e.new_font("Demo Font", (8, 8), cs_core::FontMode::OneBit) {
            let letters: [(char, &[(i32, i32)]); 3] = [
                ('A', &[(1, 1), (2, 1), (0, 2), (3, 2), (0, 3), (1, 3), (2, 3), (3, 3), (0, 4), (3, 4), (0, 5), (3, 5)]),
                ('B', &[(0, 1), (1, 1), (2, 1), (0, 2), (3, 2), (0, 3), (1, 3), (2, 3), (0, 4), (3, 4), (0, 5), (1, 5), (2, 5)]),
                ('C', &[(1, 1), (2, 1), (3, 1), (0, 2), (0, 3), (0, 4), (1, 5), (2, 5), (3, 5)]),
            ];
            for (ch, points) in letters {
                e.go_to_glyph(ch);
                let (layer, frame) = (e.layer, e.frame);
                e.execute(cs_commands::Command::DrawPixels {
                    layer,
                    frame,
                    pixels: points.iter().map(|&(x, y)| (x, y, cs_core::ONE_BIT_INK)).collect(),
                });
            }
            e.go_to_glyph('A');
            app.font_panel.test_text = "ABC CAB".into();
            dock::set_open(ctx, Some(dock::DockPanel::Font));
            app.doc.canvas.request_zoom(crate::view::ZoomRequest::Fit);
        }
    }
```

- [ ] **Step 4: Correr** `cmd //c "start /low /b /wait cargo test -p cs-ui"` y `cargo clippy -p cs-ui --all-targets` → PASS, sin avisos.

- [ ] **Step 5: Verificación en pantalla** — build release en un `target/gui-checkN` nuevo; respaldar `$APPDATA/ChimpSprite/data/app.ron`; `CHIMPSPRITE_DEMO_FONT=1 CHIMPSPRITE_SCREENSHOT=D:/Work/chimpdeas/.refs/tmp/font.png CHIMPSPRITE_SCREENSHOT_DELAY=2 timeout 40 <exe> --demo`; restaurar `app.ron`. Revisar en la captura: el título "A · Demo Font", las guías (baseline roja, punteadas), la letra fantasma detrás, la línea del ancho, la timeline oculta, el panel Font con el mapa (A, B, C con miniatura y A marcada), el Test Drive "ABC CAB" y los ajustes. Corregir lo que no se vea así.

- [ ] **Step 6: Commit** — "Fuentes pixel: la herramienta Texto escribe con las fuentes del documento; CHIMPSPRITE_DEMO_FONT".

---

### Task 11: Cierre

**Files:**
- Modify: `docs/superpowers/specs/2026-11-01-pixel-font-core-design.md` (los dos cambios de "Global Constraints": `advance: Option<u32>` y la letra fantasma con Ubuntu/Hack; estado "implementado")

- [ ] **Step 1:** Actualizar el spec con los dos cambios y el estado.
- [ ] **Step 2:** `cmd //c "start /low /b /wait cargo test --workspace"` → PASS; `cargo clippy --workspace --all-targets` sin avisos; `cargo fmt --all --check` limpio.
- [ ] **Step 3: Commit** — "Spec de fuentes pixel, parte 1: implementado (anchos automáticos al usarlos, fantasma Ubuntu/Hack)".
