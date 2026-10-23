# Biblioteca de paletas: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Presets de paleta incluidos y de usuario, cargar y guardar paletas en `.gpl`, `.hex` y
`.pal` (y leer `.png`, `.ase` y `.aseprite`), y elegir la paleta por defecto de los sprites
nuevos, como en Aseprite.

**Architecture:**
- **`cs-io::palette_files`:** lectores y escritores puros de formatos.
- **`cs-core::palettes`:** presets incluidos, compilados en el binario.
- **`cs-ui::palette_library`:** biblioteca con dos almacenes, una carpeta en escritorio y
  `localStorage` en web.
- **`Editor::apply_palette`:** aplica una paleta como un solo paso de undo.
- **`cs-ui::palette_ui`:** ventana y menú, como un `impl ChimpApp` aparte.

**Tech Stack:** Rust 2024, egui/eframe 0.36, crate `png` 0.18, serde_json.

**Spec:** `docs/superpowers/specs/2026-10-22-palette-library-design.md`.

## Global Constraints

- **Crates del motor:** `cs-core` y `cs-io` no dependen de egui y compilan en wasm32.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés. Imita la densidad de
  comentarios del código vecino.
- **Máximo de colores:** 256. Si un archivo tiene más, se rechaza; no se trunca.
- **Archivos grandes:** `editor.rs` ya es muy grande; la lógica nueva va en módulos `impl` aparte.
- **Toolchain aislado:** antes de usar cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación por tarea:** `cargo fmt --all` y `cargo test -p <crate>`.
- **Verificación final:** `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` y `cargo check -p cs-ui --target wasm32-unknown-unknown`.
- **Commits:** terminan con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Rama:** `palette-library`. Hay que rebasearla sobre `main` si `main` avanzó.

## Desviaciones del spec (decididas al planear)

- **`read_palette(file_name, data)`:** recibe el nombre del archivo en vez de un
  `PaletteFormat`. Con el nombre deduce el formato y el nombre por defecto de la paleta.
- **`write_palette`:** devuelve `Option<Vec<u8>>` (`None` para `Png` y `Aseprite`).
- **`write_gpl(palette, comments)`:** acepta líneas de comentario. El buscador de Lospec las usa
  para `# Author:` y `# Source:`.
- **`PresetSource::User`:** no lleva el nombre de archivo; el archivo se deduce del nombre con
  `preset_file_name`.
- **NES:** tiene 55 colores, como en Lospec, no 54.
- **Licencias:** Lospec no publica licencias de paletas. Las listas de colores no tienen
  derechos de autor y Aseprite incluye estas mismas paletas. Se incluyen con el crédito del
  autor.

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Crear `crates/cs-io/src/palette_files.rs` | Formatos de paleta: leer y escribir |
| Modificar `crates/cs-io/src/lib.rs` | `pub mod palette_files;` |
| Crear `crates/cs-core/src/palettes.rs` | Presets incluidos |
| Modificar `crates/cs-core/src/color.rs` | `DB32` pasa a `pub(crate)` |
| Modificar `crates/cs-core/src/lib.rs` | `mod palettes; pub use palettes::*;` |
| Crear `crates/cs-ui/src/palette_library.rs` | Biblioteca, almacenes y codificación web |
| Crear `crates/cs-ui/src/palette_ops.rs` | `impl Editor`: `apply_palette` |
| Crear `crates/cs-ui/src/palette_ui.rs` | `impl ChimpApp`: ventana Presets, diálogos y acciones |
| Modificar `crates/cs-ui/src/panels.rs` | El panel de paleta devuelve `PaletteAction` |
| Modificar `crates/cs-ui/src/editor.rs` | Secundario inicial dentro de la paleta |
| Modificar `crates/cs-ui/src/lib.rs` | Campos nuevos, persistencia del default, `new_editor(palette)` |
| Modificar `crates/cs-ui/src/tabs.rs` | `DocState::untitled_with_palette` |
| Modificar `crates/cs-ui/src/platform.rs` | `OpenPurpose::LoadPalette` |
| Modificar `crates/cs-ui/src/file_actions.rs` | Despachar `LoadPalette` y paleta por defecto en `new_document` |

---

### Task 1: Formatos de texto `.gpl`, `.hex` y `.pal`

**Files:**
- Create: `crates/cs-io/src/palette_files.rs`
- Modify: `crates/cs-io/src/lib.rs`

**Interfaces:**
- Produces:
  - `cs_io::palette_files::{PaletteFormat, NamedPalette, PaletteError, MAX_COLORS, format_for, read_palette, write_palette, write_gpl}`.
  - `read_palette(file_name: &str, data: &[u8]) -> Result<NamedPalette, PaletteError>`.
  - `write_palette(format: PaletteFormat, palette: &NamedPalette) -> Option<Vec<u8>>`.
  - `write_gpl(palette: &NamedPalette, comments: &[String]) -> Vec<u8>`.

- [ ] **Step 1: Escribir los tests que fallan**

Al final de `crates/cs-io/src/palette_files.rs` (archivo nuevo, de momento solo con el módulo
de tests y `use cs_core::{Palette, Rgba};` arriba):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, colors: &[u32]) -> NamedPalette {
        NamedPalette {
            name: name.into(),
            palette: Palette::new(colors.iter().map(|&c| Rgba::from_hex(c)).collect()),
        }
    }

    #[test]
    fn formats_by_extension() {
        assert_eq!(format_for("a.GPL"), Some(PaletteFormat::Gpl));
        assert_eq!(format_for("dir/b.hex"), Some(PaletteFormat::Hex));
        assert_eq!(format_for("c.pal"), Some(PaletteFormat::Pal));
        assert_eq!(format_for("d.png"), Some(PaletteFormat::Png));
        assert_eq!(format_for("e.ase"), Some(PaletteFormat::Aseprite));
        assert_eq!(format_for("f.aseprite"), Some(PaletteFormat::Aseprite));
        assert_eq!(format_for("g.act"), None);
        assert_eq!(format_for("noext"), None);
    }

    #[test]
    fn text_formats_roundtrip() {
        let p = named("Sunset", &[0x1a1c2c, 0xff00ff, 0xffffff]);
        for format in [PaletteFormat::Gpl, PaletteFormat::Hex, PaletteFormat::Pal] {
            let bytes = write_palette(format, &p).unwrap();
            let ext = match format {
                PaletteFormat::Gpl => "gpl",
                PaletteFormat::Hex => "hex",
                _ => "pal",
            };
            let back = read_palette(&format!("Sunset.{ext}"), &bytes).unwrap();
            assert_eq!(back, p, "{ext}");
        }
        assert!(write_palette(PaletteFormat::Png, &p).is_none());
    }

    #[test]
    fn gpl_with_comments_column_names_and_no_name() {
        let text = "GIMP Palette\r\n# made by hand\r\nColumns: 4\r\n\r\n  0   0   0\tBlack\r\n255 255 255 White\r\n";
        let p = read_palette("mono.gpl", text.as_bytes()).unwrap();
        assert_eq!(p, named("mono", &[0x000000, 0xffffff]));
        let with_name = "GIMP Palette\nName: Night\n10 20 30\n";
        assert_eq!(read_palette("x.gpl", with_name.as_bytes()).unwrap().name, "Night");
    }

    #[test]
    fn gpl_comments_are_written() {
        let p = named("Ice", &[0x7c3f58]);
        let text = String::from_utf8(write_gpl(&p, &["Author: Kerrie".into()])).unwrap();
        assert!(text.starts_with("GIMP Palette\nName: Ice\n"));
        assert!(text.contains("# Author: Kerrie\n"));
        assert_eq!(read_palette("Ice.gpl", text.as_bytes()).unwrap(), p);
    }

    #[test]
    fn hex_with_and_without_hash() {
        let p = read_palette("two.hex", b"#ff0000\n\n00ff00\n").unwrap();
        assert_eq!(p, named("two", &[0xff0000, 0x00ff00]));
    }

    #[test]
    fn errors_report_the_line() {
        let gpl = read_palette("a.gpl", b"GIMP Palette\n1 2 300\n").unwrap_err();
        assert_eq!(gpl.to_string(), "line 2: \"300\" is not a number from 0 to 255");
        let header = read_palette("a.gpl", b"hello\n").unwrap_err();
        assert_eq!(header.to_string(), "line 1: expected \"GIMP Palette\"");
        let hex = read_palette("a.hex", b"ff0000\nzz0000\n").unwrap_err();
        assert_eq!(hex.to_string(), "line 2: \"zz0000\" is not a color (rrggbb)");
        let pal = read_palette("a.pal", b"JASC-PAL\r\n0100\r\n2\r\n0 0 0\r\n").unwrap_err();
        assert_eq!(pal.to_string(), "line 3: the file says 2 colors but has 1");
        assert_eq!(read_palette("a.hex", b"").unwrap_err(), PaletteError::Empty);
        assert_eq!(read_palette("a.hex", b"\n\n").unwrap_err(), PaletteError::NoColors);
        assert_eq!(
            read_palette("a.act", b"x").unwrap_err(),
            PaletteError::UnknownFormat
        );
    }

    #[test]
    fn more_than_256_colors_is_rejected() {
        let text: String = (0..257).map(|i| format!("{i:06x}\n")).collect();
        assert_eq!(
            read_palette("big.hex", text.as_bytes()).unwrap_err(),
            PaletteError::TooManyColors(257)
        );
    }
}
```

En `crates/cs-io/src/lib.rs`, después de `pub mod import;`:

```rust
pub mod palette_files;
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-io palette_files`
Expected: errores de compilación (`format_for`, `read_palette`… no existen).

- [ ] **Step 3: Implementar**

Arriba de `crates/cs-io/src/palette_files.rs`, antes de `mod tests`:

```rust
//! Archivos de paleta: `.gpl` (GIMP), `.hex`, `.pal` (JASC) y lectura de `.png` y `.aseprite`.
//!
//! Funciones puras: reciben y devuelven bytes, sin tocar el disco.

use cs_core::{Palette, Rgba};

use crate::files;

/// Igual que Aseprite: una paleta tiene como mucho 256 colores.
pub const MAX_COLORS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteFormat {
    Gpl,
    Hex,
    Pal,
    Png,
    Aseprite,
}

/// Paleta con su nombre (el de `Name:` en `.gpl` o el del archivo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedPalette {
    pub name: String,
    pub palette: Palette,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PaletteError {
    #[error("unsupported palette format")]
    UnknownFormat,
    #[error("the file is empty")]
    Empty,
    #[error("the file has no colors")]
    NoColors,
    #[error("the palette has {0} colors (the maximum is 256)")]
    TooManyColors(usize),
    #[error("line {line}: {message}")]
    Line { line: usize, message: String },
    #[error("{0}")]
    Image(String),
}

fn line_error(line: usize, message: impl Into<String>) -> PaletteError {
    PaletteError::Line {
        line,
        message: message.into(),
    }
}

/// Formato según la extensión (sin importar mayúsculas).
pub fn format_for(file_name: &str) -> Option<PaletteFormat> {
    let ext = file_name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "gpl" => Some(PaletteFormat::Gpl),
        "hex" => Some(PaletteFormat::Hex),
        "pal" => Some(PaletteFormat::Pal),
        "png" => Some(PaletteFormat::Png),
        "ase" | "aseprite" => Some(PaletteFormat::Aseprite),
        _ => None,
    }
}

pub fn read_palette(file_name: &str, data: &[u8]) -> Result<NamedPalette, PaletteError> {
    let format = format_for(file_name).ok_or(PaletteError::UnknownFormat)?;
    if data.is_empty() {
        return Err(PaletteError::Empty);
    }
    let (name, colors) = match format {
        PaletteFormat::Gpl => read_gpl(&text(data))?,
        PaletteFormat::Hex => (None, read_hex(&text(data))?),
        PaletteFormat::Pal => (None, read_pal(&text(data))?),
        PaletteFormat::Png => (None, read_png(data)?),
        PaletteFormat::Aseprite => (None, read_aseprite(data)?),
    };
    if colors.is_empty() {
        return Err(PaletteError::NoColors);
    }
    if colors.len() > MAX_COLORS {
        return Err(PaletteError::TooManyColors(colors.len()));
    }
    let name = name.unwrap_or_else(|| files::stem(base_name(file_name)).to_string());
    Ok(NamedPalette {
        name,
        palette: Palette::new(colors),
    })
}

/// `None` para los formatos que solo se leen (`.png`, `.aseprite`).
pub fn write_palette(format: PaletteFormat, palette: &NamedPalette) -> Option<Vec<u8>> {
    let colors = palette.palette.colors();
    match format {
        PaletteFormat::Gpl => Some(write_gpl(palette, &[])),
        PaletteFormat::Hex => Some(
            colors
                .iter()
                .map(|c| format!("{:02x}{:02x}{:02x}\n", c.r, c.g, c.b))
                .collect::<String>()
                .into_bytes(),
        ),
        PaletteFormat::Pal => {
            let mut out = format!("JASC-PAL\r\n0100\r\n{}\r\n", colors.len());
            for c in colors {
                out.push_str(&format!("{} {} {}\r\n", c.r, c.g, c.b));
            }
            Some(out.into_bytes())
        }
        PaletteFormat::Png | PaletteFormat::Aseprite => None,
    }
}

/// `.gpl` con `Name:` y líneas `# …` (sin el `#`) antes de los colores.
pub fn write_gpl(palette: &NamedPalette, comments: &[String]) -> Vec<u8> {
    let mut out = format!("GIMP Palette\nName: {}\nColumns: 0\n", palette.name);
    for c in comments {
        out.push_str(&format!("# {c}\n"));
    }
    out.push_str("#\n");
    for c in palette.palette.colors() {
        out.push_str(&format!("{:3} {:3} {:3}\n", c.r, c.g, c.b));
    }
    out.into_bytes()
}

fn text(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    s.strip_prefix('\u{feff}').unwrap_or(&s).to_string()
}

/// Nombre del archivo sin carpeta (acepta `/` y `\`).
fn base_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Lee `R G B` al principio de la línea (lo que sigue se ignora).
fn rgb_triplet(line: &str, number: usize) -> Result<Rgba, PaletteError> {
    let mut parts = line.split_whitespace();
    let mut channel = || -> Result<u8, PaletteError> {
        let part = parts
            .next()
            .ok_or_else(|| line_error(number, "expected three numbers (R G B)"))?;
        part.parse::<u8>()
            .map_err(|_| line_error(number, format!("\"{part}\" is not a number from 0 to 255")))
    };
    Ok(Rgba::rgb(channel()?, channel()?, channel()?))
}

fn read_gpl(text: &str) -> Result<(Option<String>, Vec<Rgba>), PaletteError> {
    let mut lines = text.lines().enumerate();
    if lines.next().map(|(_, l)| l.trim()) != Some("GIMP Palette") {
        return Err(line_error(1, "expected \"GIMP Palette\""));
    }
    let mut name = None;
    let mut colors = Vec::new();
    for (i, raw) in lines {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("Columns:") {
            continue;
        }
        if let Some(value) = line.strip_prefix("Name:") {
            name = Some(value.trim().to_string()).filter(|n| !n.is_empty());
            continue;
        }
        colors.push(rgb_triplet(line, i + 1)?);
    }
    Ok((name, colors))
}

fn read_hex(text: &str) -> Result<Vec<Rgba>, PaletteError> {
    let mut colors = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let hex = line.trim_start_matches('#');
        let value = (hex.len() == 6)
            .then(|| u32::from_str_radix(hex, 16).ok())
            .flatten()
            .ok_or_else(|| line_error(i + 1, format!("\"{line}\" is not a color (rrggbb)")))?;
        colors.push(Rgba::from_hex(value));
    }
    Ok(colors)
}

fn read_pal(text: &str) -> Result<Vec<Rgba>, PaletteError> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    if lines.first() != Some(&"JASC-PAL") {
        return Err(line_error(1, "expected \"JASC-PAL\""));
    }
    if lines.get(1) != Some(&"0100") {
        return Err(line_error(2, "expected \"0100\""));
    }
    let count: usize = lines
        .get(2)
        .and_then(|l| l.parse().ok())
        .ok_or_else(|| line_error(3, "expected the number of colors"))?;
    let mut colors = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(3) {
        if !line.is_empty() {
            colors.push(rgb_triplet(line, i + 1)?);
        }
    }
    if colors.len() != count {
        return Err(line_error(
            3,
            format!("the file says {count} colors but has {}", colors.len()),
        ));
    }
    Ok(colors)
}

// Task 2 las reemplaza.
fn read_png(_data: &[u8]) -> Result<Vec<Rgba>, PaletteError> {
    Err(PaletteError::UnknownFormat)
}

fn read_aseprite(_data: &[u8]) -> Result<Vec<Rgba>, PaletteError> {
    Err(PaletteError::UnknownFormat)
}
```

`files::stem` ya existe en `crates/cs-io/src/files.rs:98` y quita la extensión.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-io palette_files`
Expected: los 7 tests en PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-io/src/palette_files.rs crates/cs-io/src/lib.rs
git commit -m "Formatos de paleta .gpl, .hex y .pal (leer y escribir)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Leer la paleta de un `.png` y de un `.aseprite`

**Files:**
- Modify: `crates/cs-io/src/palette_files.rs` (reemplazar `read_png` y `read_aseprite` y agregar tests)

**Interfaces:**
- Consumes: `crate::import::decode_png(&[u8]) -> Result<Bitmap>` (`Bitmap { width, height, pixels: Vec<Rgba> }`) y `crate::aseprite::read(&[u8]) -> Result<(Sprite, LoadReport)>`.
- Produces: `read_palette("x.png" | "x.aseprite", …)` funcionando.

- [ ] **Step 1: Escribir los tests que fallan** (dentro de `mod tests`)

```rust
    fn encode_png(color: png::ColorType, palette: Option<&[u8]>, data: &[u8], w: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, 1);
            enc.set_color(color);
            enc.set_depth(png::BitDepth::Eight);
            if let Some(p) = palette {
                enc.set_palette(p.to_vec());
            }
            let mut writer = enc.write_header().unwrap();
            writer.write_image_data(data).unwrap();
        }
        out
    }

    #[test]
    fn indexed_png_uses_its_plte() {
        let plte = [255, 0, 0, 0, 255, 0, 0, 0, 255];
        let bytes = encode_png(png::ColorType::Indexed, Some(&plte), &[2, 2], 2);
        let p = read_palette("rgb.png", &bytes).unwrap();
        assert_eq!(p, named("rgb", &[0xff0000, 0x00ff00, 0x0000ff]));
    }

    #[test]
    fn rgba_png_uses_unique_opaque_colors_in_order() {
        let px = [
            0, 0, 255, 255, // azul
            9, 9, 9, 0, // transparente: se ignora
            255, 0, 0, 255, // rojo
            0, 0, 255, 255, // azul repetido
        ];
        let bytes = encode_png(png::ColorType::Rgba, None, &px, 4);
        let p = read_palette("strip.png", &bytes).unwrap();
        assert_eq!(p, named("strip", &[0x0000ff, 0xff0000]));
    }

    #[test]
    fn rgba_png_with_too_many_colors_is_rejected() {
        let px: Vec<u8> = (0..300u32)
            .flat_map(|i| [(i >> 8) as u8, i as u8, 0, 255])
            .collect();
        let bytes = encode_png(png::ColorType::Rgba, None, &px, 300);
        assert_eq!(
            read_palette("many.png", &bytes).unwrap_err(),
            PaletteError::TooManyColors(300)
        );
    }

    #[test]
    fn aseprite_palette_is_read() {
        let mut sprite = cs_core::Sprite::new(2, 2, cs_core::ColorMode::Rgba);
        sprite.palette = Palette::new(vec![Rgba::from_hex(0x123456), Rgba::from_hex(0xabcdef)]);
        let bytes = crate::aseprite::write(&sprite);
        let p = read_palette("hero.aseprite", &bytes).unwrap();
        assert_eq!(p, named("hero", &[0x123456, 0xabcdef]));
        assert!(matches!(
            read_palette("bad.ase", b"nope").unwrap_err(),
            PaletteError::Image(_)
        ));
    }
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-io palette_files`
Expected: los 4 tests nuevos en FAIL con `UnknownFormat`.

- [ ] **Step 3: Implementar.** Reemplazar las dos funciones provisionales:

```rust
/// PNG indexado: su PLTE. Si no: los colores opacos únicos en orden de aparición.
fn read_png(data: &[u8]) -> Result<Vec<Rgba>, PaletteError> {
    let image_error = |e: String| PaletteError::Image(e);
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let reader = decoder.read_info().map_err(|e| image_error(e.to_string()))?;
    let info = reader.info();
    if info.color_type == png::ColorType::Indexed
        && let Some(plte) = &info.palette
    {
        let (chunks, _) = plte.as_chunks::<3>();
        return Ok(chunks.iter().map(|c| Rgba::rgb(c[0], c[1], c[2])).collect());
    }
    let bitmap = crate::import::decode_png(data).map_err(|e| image_error(e.to_string()))?;
    let mut colors: Vec<Rgba> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for c in bitmap.pixels.into_iter().filter(|c| c.a == 255) {
        if seen.insert(c) {
            colors.push(c);
        }
    }
    Ok(colors)
}

fn read_aseprite(data: &[u8]) -> Result<Vec<Rgba>, PaletteError> {
    let (sprite, _) =
        crate::aseprite::read(data).map_err(|e| PaletteError::Image(e.to_string()))?;
    Ok(sprite.palette.colors().to_vec())
}
```

Si `png::Decoder::new` en 0.18 pide `BufRead + Seek`, `Cursor<&[u8]>` cumple con los dos (así
lo usa `import.rs`). Para cuidar la memoria, `decode_png` ya aplica límites.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-io palette_files`
Expected: los 11 tests en PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-io/src/palette_files.rs
git commit -m "Leer la paleta de .png y .aseprite

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Presets incluidos (`cs-core::palettes`)

**Files:**
- Create: `crates/cs-core/src/palettes.rs`
- Modify: `crates/cs-core/src/color.rs` (`const DB32` → `pub(crate) const DB32`)
- Modify: `crates/cs-core/src/lib.rs` (`mod palettes;` y `pub use palettes::*;`)

**Interfaces:**
- Produces:
  - `pub struct BuiltinPalette { pub name: &'static str, pub author: &'static str, colors: &'static [u32] }`.
  - `BuiltinPalette::palette(&self) -> Palette`.
  - `pub const BUILTIN_PALETTES: &[BuiltinPalette]`.

- [ ] **Step 1: Escribir los tests que fallan.** Archivo nuevo `crates/cs-core/src/palettes.rs`
  con solo los tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_presets_are_valid_and_unique() {
        let mut names: Vec<&str> = BUILTIN_PALETTES.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), 10);
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 10, "nombres repetidos");
        for p in BUILTIN_PALETTES {
            let palette = p.palette();
            assert!((2..=256).contains(&palette.len()), "{}", p.name);
            assert!(palette.colors().iter().all(|c| c.a == 255));
        }
    }

    #[test]
    fn db32_preset_matches_the_factory_default() {
        let db32 = BUILTIN_PALETTES.iter().find(|p| p.name == "DB32").unwrap();
        assert_eq!(db32.palette(), crate::Palette::db32());
    }

    #[test]
    fn color_counts() {
        let count = |name: &str| {
            BUILTIN_PALETTES.iter().find(|p| p.name == name).unwrap().palette().len()
        };
        assert_eq!(count("PICO-8"), 16);
        assert_eq!(count("Game Boy"), 4);
        assert_eq!(count("NES"), 55);
        assert_eq!(count("AAP-64"), 64);
        assert_eq!(count("1-bit"), 2);
    }
}
```

Registrar el módulo en `crates/cs-core/src/lib.rs` (`mod palettes;` en orden alfabético y
`pub use palettes::*;`).

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-core palettes`
Expected: error de compilación (`BUILTIN_PALETTES` no existe).

- [ ] **Step 3: Implementar.** Arriba del módulo de tests (los colores salen de
`https://lospec.com/palette-list/<slug>.json`, consultado el 2026-10-22):

```rust
//! Paletas incluidas en el binario (las de la ventana Palette Presets).

use crate::color::DB32;
use crate::{Palette, Rgba};

pub struct BuiltinPalette {
    pub name: &'static str,
    pub author: &'static str,
    colors: &'static [u32],
}

impl BuiltinPalette {
    pub fn palette(&self) -> Palette {
        Palette::new(self.colors.iter().map(|&h| Rgba::from_hex(h)).collect())
    }
}

/// Mismo orden que en la ventana. Los colores salen de Lospec (`palette-list/<slug>.json`).
pub const BUILTIN_PALETTES: &[BuiltinPalette] = &[
    BuiltinPalette { name: "DB16", author: "DawnBringer", colors: &DB16 },
    BuiltinPalette { name: "DB32", author: "DawnBringer", colors: &DB32 },
    BuiltinPalette { name: "PICO-8", author: "Lexaloffle", colors: &PICO8 },
    BuiltinPalette { name: "Game Boy", author: "Nintendo", colors: &GAME_BOY },
    BuiltinPalette { name: "NES", author: "Nintendo", colors: &NES },
    BuiltinPalette { name: "Sweetie 16", author: "GrafxKid", colors: &SWEETIE16 },
    BuiltinPalette { name: "Endesga 32", author: "ENDESGA", colors: &ENDESGA32 },
    BuiltinPalette { name: "AAP-64", author: "Adigun A. Polack", colors: &AAP64 },
    BuiltinPalette { name: "CGA", author: "IBM", colors: &CGA },
    BuiltinPalette { name: "1-bit", author: "", colors: &[0x000000, 0xffffff] },
];

const DB16: [u32; 16] = [
    0x140c1c, 0x442434, 0x30346d, 0x4e4a4e, 0x854c30, 0x346524, 0xd04648, 0x757161, 0x597dce,
    0xd27d2c, 0x8595a1, 0x6daa2c, 0xd2aa99, 0x6dc2ca, 0xdad45e, 0xdeeed6,
];

const PICO8: [u32; 16] = [
    0x000000, 0x1d2b53, 0x7e2553, 0x008751, 0xab5236, 0x5f574f, 0xc2c3c7, 0xfff1e8, 0xff004d,
    0xffa300, 0xffec27, 0x00e436, 0x29adff, 0x83769c, 0xff77a8, 0xffccaa,
];

const GAME_BOY: [u32; 4] = [0x081820, 0x346856, 0x88c070, 0xe0f8d0];

const NES: [u32; 55] = [
    0x000000, 0xfcfcfc, 0xf8f8f8, 0xbcbcbc, 0x7c7c7c, 0xa4e4fc, 0x3cbcfc, 0x0078f8, 0x0000fc,
    0xb8b8f8, 0x6888fc, 0x0058f8, 0x0000bc, 0xd8b8f8, 0x9878f8, 0x6844fc, 0x4428bc, 0xf8b8f8,
    0xf878f8, 0xd800cc, 0x940084, 0xf8a4c0, 0xf85898, 0xe40058, 0xa80020, 0xf0d0b0, 0xf87858,
    0xf83800, 0xa81000, 0xfce0a8, 0xfca044, 0xe45c10, 0x881400, 0xf8d878, 0xf8b800, 0xac7c00,
    0x503000, 0xd8f878, 0xb8f818, 0x00b800, 0x007800, 0xb8f8b8, 0x58d854, 0x00a800, 0x006800,
    0xb8f8d8, 0x58f898, 0x00a844, 0x005800, 0x00fcfc, 0x00e8d8, 0x008888, 0x004058, 0xf8d8f8,
    0x787878,
];

const SWEETIE16: [u32; 16] = [
    0x1a1c2c, 0x5d275d, 0xb13e53, 0xef7d57, 0xffcd75, 0xa7f070, 0x38b764, 0x257179, 0x29366f,
    0x3b5dc9, 0x41a6f6, 0x73eff7, 0xf4f4f4, 0x94b0c2, 0x566c86, 0x333c57,
];

const ENDESGA32: [u32; 32] = [
    0xbe4a2f, 0xd77643, 0xead4aa, 0xe4a672, 0xb86f50, 0x733e39, 0x3e2731, 0xa22633, 0xe43b44,
    0xf77622, 0xfeae34, 0xfee761, 0x63c74d, 0x3e8948, 0x265c42, 0x193c3e, 0x124e89, 0x0099db,
    0x2ce8f5, 0xffffff, 0xc0cbdc, 0x8b9bb4, 0x5a6988, 0x3a4466, 0x262b44, 0x181425, 0xff0044,
    0x68386c, 0xb55088, 0xf6757a, 0xe8b796, 0xc28569,
];

const AAP64: [u32; 64] = [
    0x060608, 0x141013, 0x3b1725, 0x73172d, 0xb4202a, 0xdf3e23, 0xfa6a0a, 0xf9a31b, 0xffd541,
    0xfffc40, 0xd6f264, 0x9cdb43, 0x59c135, 0x14a02e, 0x1a7a3e, 0x24523b, 0x122020, 0x143464,
    0x285cc4, 0x249fde, 0x20d6c7, 0xa6fcdb, 0xffffff, 0xfef3c0, 0xfad6b8, 0xf5a097, 0xe86a73,
    0xbc4a9b, 0x793a80, 0x403353, 0x242234, 0x221c1a, 0x322b28, 0x71413b, 0xbb7547, 0xdba463,
    0xf4d29c, 0xdae0ea, 0xb3b9d1, 0x8b93af, 0x6d758d, 0x4a5462, 0x333941, 0x422433, 0x5b3138,
    0x8e5252, 0xba756a, 0xe9b5a3, 0xe3e6ff, 0xb9bffb, 0x849be4, 0x588dbe, 0x477d85, 0x23674e,
    0x328464, 0x5daf8d, 0x92dcba, 0xcdf7e2, 0xe4d2aa, 0xc7b08b, 0xa08662, 0x796755, 0x5a4e44,
    0x423934,
];

const CGA: [u32; 16] = [
    0x000000, 0x555555, 0xaaaaaa, 0xffffff, 0x0000aa, 0x5555ff, 0x00aa00, 0x55ff55, 0x00aaaa,
    0x55ffff, 0xaa0000, 0xff5555, 0xaa00aa, 0xff55ff, 0xaa5500, 0xffff55,
];
```

En `crates/cs-core/src/color.rs`, la línea `const DB32: [u32; 32] = [` pasa a
`pub(crate) const DB32: [u32; 32] = [`.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-core palettes`
Expected: los 3 tests en PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-core/src/palettes.rs crates/cs-core/src/color.rs crates/cs-core/src/lib.rs
git commit -m "Presets de paleta incluidos (DB16, DB32, PICO-8, Game Boy, NES…)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Biblioteca de paletas y sus almacenes

**Files:**
- Create: `crates/cs-ui/src/palette_library.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`mod palette_library;` en orden alfabético)

**Interfaces:**
- Consumes: `cs_io::palette_files::{NamedPalette, read_palette, write_gpl}` y `cs_core::BUILTIN_PALETTES`.
- Produces:
  - `pub struct Preset { pub name: String, pub author: String, pub palette: Palette, pub source: PresetSource }` y `pub enum PresetSource { Builtin, User }`.
  - `pub trait PresetStore { fn load(&self) -> Vec<NamedPalette>; fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String>; fn delete(&mut self, name: &str) -> Result<(), String>; fn folder(&self) -> Option<std::path::PathBuf>; }`.
  - `pub struct PaletteLibrary`, con:
    - `new(store: Box<dyn PresetStore>) -> Self` y `for_platform() -> Self`;
    - `reload(&mut self)`, `builtin(&self) -> &[Preset]` y `user(&self) -> &[Preset]`;
    - `has_user(&self, name: &str) -> bool`;
    - `save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String>`;
    - `delete(&mut self, name: &str) -> Result<(), String>`;
    - `find(&self, name: &str) -> Option<&Preset>`;
    - `default_name(&self) -> Option<&str>` y `set_default(&mut self, name: Option<String>)`;
    - `default_palette(&self) -> Palette` y `folder(&self) -> Option<PathBuf>`.
  - `pub fn preset_file_name(name: &str) -> String`, `pub fn encode_web(&[NamedPalette]) -> String` y `pub fn decode_web(&str) -> Vec<NamedPalette>`.
  - `pub struct DirStore { dir: PathBuf }` con `DirStore::new(dir)`.

- [ ] **Step 1: Escribir los tests que fallan** (al final del archivo nuevo)

```rust
#[cfg(test)]
mod tests {
    use cs_core::Rgba;

    use super::*;

    fn named(name: &str, colors: &[u32]) -> NamedPalette {
        NamedPalette {
            name: name.into(),
            palette: Palette::new(colors.iter().map(|&c| Rgba::from_hex(c)).collect()),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cs-palettes-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(preset_file_name("Night: blue/ish?"), "Night_ blue_ish_.gpl");
        assert_eq!(preset_file_name("  "), "Palette.gpl");
    }

    #[test]
    fn folder_reads_valid_files_and_skips_broken_ones() {
        let dir = temp_dir("read");
        std::fs::write(dir.join("two.hex"), "ff0000\n00ff00\n").unwrap();
        std::fs::write(dir.join("broken.gpl"), "not a palette").unwrap();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();
        let lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        let names: Vec<&str> = lib.user().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["two"]);
        assert_eq!(lib.builtin().len(), cs_core::BUILTIN_PALETTES.len());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_replace_and_delete() {
        let dir = temp_dir("save");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("Mine", &[0x111111]), &[]).unwrap();
        assert!(lib.has_user("mine"), "sin importar mayúsculas");
        lib.save(&named("Mine", &[0x222222, 0x333333]), &[]).unwrap();
        assert_eq!(lib.user().len(), 1);
        assert_eq!(lib.find("Mine").unwrap().palette.len(), 2);
        assert!(dir.join("Mine.gpl").exists());
        lib.delete("Mine").unwrap();
        assert!(lib.user().is_empty());
        assert!(!dir.join("Mine.gpl").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn default_falls_back_to_db32() {
        let dir = temp_dir("default");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        assert_eq!(lib.default_palette(), Palette::db32());
        lib.set_default(Some("PICO-8".into()));
        assert_eq!(lib.default_palette().len(), 16);
        lib.set_default(Some("Gone".into()));
        assert_eq!(lib.default_palette(), Palette::db32());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn web_encoding_roundtrip() {
        let list = vec![named("A", &[0x010203]), named("B", &[0xffffff, 0x000000])];
        let text = encode_web(&list);
        assert!(text.contains("\"010203\""));
        assert_eq!(decode_web(&text), list);
        assert!(decode_web("{nope").is_empty());
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui palette_library`
Expected: errores de compilación.

- [ ] **Step 3: Implementar.** Arriba de `mod tests`:

```rust
//! Biblioteca de paletas: presets incluidos más los del usuario.
//!
//! En escritorio, los presets del usuario son archivos en `<storage_dir>/palettes/`. En web,
//! una lista JSON en `localStorage`.

use std::path::PathBuf;

use cs_core::{BUILTIN_PALETTES, Palette, Rgba};
use cs_io::palette_files::{self, NamedPalette};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetSource {
    Builtin,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub author: String,
    pub palette: Palette,
    pub source: PresetSource,
}

/// Dónde viven los presets del usuario.
pub trait PresetStore {
    fn load(&self) -> Vec<NamedPalette>;
    /// Escribe o reemplaza el preset `palette.name`.
    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String>;
    fn delete(&mut self, name: &str) -> Result<(), String>;
    /// Carpeta para "Open Folder" (solo escritorio).
    fn folder(&self) -> Option<PathBuf>;
}

pub struct PaletteLibrary {
    builtin: Vec<Preset>,
    user: Vec<Preset>,
    default: Option<String>,
    store: Box<dyn PresetStore>,
}

impl PaletteLibrary {
    pub fn new(store: Box<dyn PresetStore>) -> Self {
        let builtin = BUILTIN_PALETTES
            .iter()
            .map(|p| Preset {
                name: p.name.into(),
                author: p.author.into(),
                palette: p.palette(),
                source: PresetSource::Builtin,
            })
            .collect();
        let mut lib = Self {
            builtin,
            user: Vec::new(),
            default: None,
            store,
        };
        lib.reload();
        lib
    }

    /// Carpeta de usuario en escritorio y `localStorage` en web.
    pub fn for_platform() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let store: Box<dyn PresetStore> = Box::new(DirStore::new(
            eframe::storage_dir("ChimpSprite")
                .unwrap_or_else(|| PathBuf::from("."))
                .join("palettes"),
        ));
        #[cfg(target_arch = "wasm32")]
        let store: Box<dyn PresetStore> = Box::new(web::LocalStore);
        Self::new(store)
    }

    /// Vuelve a leer los presets del usuario (los que se soltaron a mano en la carpeta).
    pub fn reload(&mut self) {
        let mut user: Vec<Preset> = self
            .store
            .load()
            .into_iter()
            .map(|p| Preset {
                name: p.name,
                author: String::new(),
                palette: p.palette,
                source: PresetSource::User,
            })
            .collect();
        user.sort_by_key(|p| p.name.to_lowercase());
        self.user = user;
    }

    pub fn builtin(&self) -> &[Preset] {
        &self.builtin
    }

    pub fn user(&self) -> &[Preset] {
        &self.user
    }

    /// ¿Hay un preset de usuario con ese nombre? (sin importar mayúsculas, como en Windows).
    pub fn has_user(&self, name: &str) -> bool {
        self.user.iter().any(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        self.store.save(palette, comments)?;
        self.reload();
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        self.store.delete(name)?;
        self.reload();
        Ok(())
    }

    /// Primero los del usuario (pueden tener el mismo nombre que uno incluido).
    pub fn find(&self, name: &str) -> Option<&Preset> {
        self.user
            .iter()
            .chain(&self.builtin)
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn default_name(&self) -> Option<&str> {
        self.default.as_deref()
    }

    pub fn set_default(&mut self, name: Option<String>) {
        self.default = name;
    }

    /// La paleta de los sprites nuevos: el preset por defecto, o DB32 si ya no existe.
    pub fn default_palette(&self) -> Palette {
        self.default
            .as_deref()
            .and_then(|n| self.find(n))
            .map_or_else(Palette::db32, |p| p.palette.clone())
    }

    pub fn folder(&self) -> Option<PathBuf> {
        self.store.folder()
    }
}

/// `<nombre>.gpl` sin caracteres que Windows no acepta.
pub fn preset_file_name(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = if clean.is_empty() { "Palette".to_string() } else { clean };
    format!("{clean}.gpl")
}

/// Escritorio: un archivo por preset.
pub struct DirStore {
    dir: PathBuf,
}

impl DirStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl PresetStore for DirStore {
    fn load(&self) -> Vec<NamedPalette> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for path in entries.flatten().map(|e| e.path()) {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if palette_files::format_for(name).is_none() {
                continue;
            }
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| palette_files::read_palette(name, &b).map_err(|e| e.to_string()))
            {
                Ok(p) => out.push(p),
                Err(e) => log::warn!("skipping palette {}: {e}", path.display()),
            }
        }
        out
    }

    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Con otra extensión o mayúsculas distintas sería un duplicado: se borra primero.
        let _ = self.delete(&palette.name);
        let path = self.dir.join(preset_file_name(&palette.name));
        std::fs::write(path, palette_files::write_gpl(palette, comments)).map_err(|e| e.to_string())
    }

    fn delete(&mut self, name: &str) -> Result<(), String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(());
        };
        for path in entries.flatten().map(|e| e.path()) {
            let Some(file) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let matches = palette_files::format_for(file).is_some()
                && std::fs::read(&path)
                    .ok()
                    .and_then(|b| palette_files::read_palette(file, &b).ok())
                    .is_some_and(|p| p.name.eq_ignore_ascii_case(name));
            if matches {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn folder(&self) -> Option<PathBuf> {
        Some(self.dir.clone())
    }
}

#[derive(Serialize, Deserialize)]
struct WebPalette {
    name: String,
    colors: Vec<String>,
}

/// Lista JSON de `{ name, colors: ["rrggbb", …] }` (lo que se guarda en web).
pub fn encode_web(list: &[NamedPalette]) -> String {
    let items: Vec<WebPalette> = list
        .iter()
        .map(|p| WebPalette {
            name: p.name.clone(),
            colors: p
                .palette
                .colors()
                .iter()
                .map(|c| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                .collect(),
        })
        .collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
}

/// Lo ilegible se descarta (lista vacía o entradas sueltas).
pub fn decode_web(text: &str) -> Vec<NamedPalette> {
    let items: Vec<WebPalette> = serde_json::from_str(text).unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            let colors: Option<Vec<Rgba>> = item
                .colors
                .iter()
                .map(|h| u32::from_str_radix(h, 16).ok().map(Rgba::from_hex))
                .collect();
            let colors = colors.filter(|c| !c.is_empty() && c.len() <= palette_files::MAX_COLORS)?;
            Some(NamedPalette {
                name: item.name,
                palette: Palette::new(colors),
            })
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;

    const KEY: &str = "chimpsprite.palettes";

    fn local() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub struct LocalStore;

    impl LocalStore {
        fn write(list: &[NamedPalette]) -> Result<(), String> {
            local()
                .ok_or("browser storage is not available")?
                .set_item(KEY, &encode_web(list))
                .map_err(|_| "the browser refused to store the palette (quota?)".to_string())
        }
    }

    impl PresetStore for LocalStore {
        fn load(&self) -> Vec<NamedPalette> {
            local()
                .and_then(|s| s.get_item(KEY).ok().flatten())
                .map(|t| decode_web(&t))
                .unwrap_or_default()
        }

        fn save(&mut self, palette: &NamedPalette, _comments: &[String]) -> Result<(), String> {
            let mut list = self.load();
            list.retain(|p| !p.name.eq_ignore_ascii_case(&palette.name));
            list.push(palette.clone());
            Self::write(&list)
        }

        fn delete(&mut self, name: &str) -> Result<(), String> {
            let mut list = self.load();
            list.retain(|p| !p.name.eq_ignore_ascii_case(name));
            Self::write(&list)
        }

        fn folder(&self) -> Option<PathBuf> {
            None
        }
    }
}
```

Agregar `mod palette_library;` en `crates/cs-ui/src/lib.rs`. Si clippy marca `dead_code` porque
la UI todavía no la usa, poner `#![allow(dead_code)]` provisional arriba del archivo; Task 6 lo
quita.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-ui palette_library` y luego
`cargo check -p cs-ui --target wasm32-unknown-unknown`.
Expected: los 5 tests en PASS; wasm compila.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/src/palette_library.rs crates/cs-ui/src/lib.rs
git commit -m "Biblioteca de paletas: carpeta de usuario, localStorage y paleta por defecto

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `Editor::apply_palette` y paleta por defecto de los sprites nuevos

**Files:**
- Create: `crates/cs-ui/src/palette_ops.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`mod palette_ops;` y `new_editor(palette)`)
- Modify: `crates/cs-ui/src/editor.rs:564-565` (secundario inicial)
- Modify: `crates/cs-ui/src/tabs.rs` (`untitled_with_palette`)

**Interfaces:**
- Consumes: `Command::SetPalette(Palette)` (`crates/cs-commands/src/command.rs:140`) y
  `Editor::execute(Command) -> bool`.
- Produces:
  - `Editor::apply_palette(&mut self, palette: Palette) -> bool`;
  - `crate::new_editor(palette: Palette) -> Editor`;
  - `DocState::untitled_with_palette(name, palette)`.

- [ ] **Step 1: Escribir los tests que fallan.** `crates/cs-ui/src/palette_ops.rs`:

```rust
#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Palette, PixelValue, Rgba, Sprite};

    use crate::Editor;

    fn colors(n: u32) -> Palette {
        Palette::new((0..n).map(|i| Rgba::from_hex(i * 0x010101)).collect())
    }

    #[test]
    fn rgba_apply_is_one_undo_step_and_keeps_pixels() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let before = e.sprite().palette.clone();
        assert!(e.apply_palette(colors(4)));
        assert_eq!(e.sprite().palette.len(), 4);
        e.undo();
        assert_eq!(e.sprite().palette, before);
        assert!(!e.apply_palette(before.clone()), "la misma paleta no hace nada");
        assert!(!e.apply_palette(Palette::new(Vec::new())), "vacía no se aplica");
    }

    #[test]
    fn indexed_colors_outside_the_new_palette_go_to_zero() {
        let mut e = Editor::with_sprite(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        e.primary = PixelValue::Index(30);
        e.secondary = PixelValue::Index(2);
        assert!(e.apply_palette(colors(8)));
        assert_eq!(e.primary, PixelValue::Index(0));
        assert_eq!(e.secondary, PixelValue::Index(2));
    }

    #[test]
    fn new_editor_uses_the_given_palette() {
        let e = crate::new_editor(colors(4));
        assert_eq!(e.sprite().palette, colors(4));
        // Con menos de 22 colores, el secundario no cae fuera de la paleta.
        assert_eq!(e.secondary, e.palette_value(3));
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui palette_ops`
Expected: error de compilación (`apply_palette` no existe; `new_editor` no recibe argumentos).

- [ ] **Step 3: Implementar.** Arriba de `crates/cs-ui/src/palette_ops.rs`:

```rust
//! Aplicar una paleta al sprite activo (`Editor::apply_palette`).

use cs_commands::Command;
use cs_core::{Palette, PixelValue};

use crate::Editor;

impl Editor {
    /// Cambia la paleta del sprite activo en un paso de undo. En RGBA los píxeles no cambian; en
    /// indexado cada índice toma el color nuevo de su posición, como en Aseprite.
    pub fn apply_palette(&mut self, palette: Palette) -> bool {
        if palette.is_empty() || palette == self.sprite().palette {
            return false;
        }
        if !self.execute(Command::SetPalette(palette)) {
            return false;
        }
        self.primary = self.fit_to_palette(self.primary);
        self.secondary = self.fit_to_palette(self.secondary);
        true
    }

    /// Un índice fuera de la paleta pasa al 0 (el Mask se queda como está).
    fn fit_to_palette(&self, value: PixelValue) -> PixelValue {
        match value {
            PixelValue::Index(i)
                if !self.is_mask(value) && i as usize >= self.sprite().palette.len() =>
            {
                PixelValue::Index(0)
            }
            other => other,
        }
    }
}
```

En `crates/cs-ui/src/editor.rs`, dentro de `Editor::new` (líneas 564–565):

```rust
        editor.primary = editor.palette_value(0);
        // El 21 es el blanco de DB32; con paletas más cortas, el último color.
        let last = editor.sprite().palette.len().saturating_sub(1).min(21) as u8;
        editor.secondary = editor.palette_value(last);
```

En `crates/cs-ui/src/lib.rs`, agregar `mod palette_ops;` y cambiar `new_editor`:

```rust
fn new_editor(palette: cs_core::Palette) -> Editor {
    let mut sprite = Sprite::new(NEW_SPRITE_SIZE, NEW_SPRITE_SIZE, ColorMode::Rgba);
    sprite.palette = palette;
    Editor::with_sprite(sprite)
}
```

En `crates/cs-ui/src/tabs.rs`:

```rust
    pub fn untitled(name: impl Into<String>) -> Self {
        Self::untitled_with_palette(name, cs_core::Palette::db32())
    }

    /// Documento nuevo con la paleta por defecto del usuario.
    pub fn untitled_with_palette(name: impl Into<String>, palette: cs_core::Palette) -> Self {
        Self::new(
            crate::new_editor(palette),
            DocLocation {
                name: name.into(),
                target: None,
            },
        )
    }
```

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-ui`
Expected: todo en PASS (también los tests viejos del editor y las pestañas).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/src/palette_ops.rs crates/cs-ui/src/editor.rs crates/cs-ui/src/lib.rs crates/cs-ui/src/tabs.rs
git commit -m "Editor::apply_palette y sprites nuevos con una paleta dada

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Interfaz (ventana Palette Presets, menú ⋯, cargar y guardar, paleta por defecto)

**Files:**
- Create: `crates/cs-ui/src/palette_ui.rs`
- Modify: `crates/cs-ui/src/panels.rs` (`palette` devuelve `Option<PaletteAction>`; agregar `palette_strip`)
- Modify: `crates/cs-ui/src/lib.rs` (campos, `new`, llamada al panel, `save`, `mod palette_ui;`)
- Modify: `crates/cs-ui/src/platform.rs` (`OpenPurpose::LoadPalette`)
- Modify: `crates/cs-ui/src/file_actions.rs` (despachar `LoadPalette` y `new_document`)

**Interfaces:**
- Consumes: `PaletteLibrary` (Task 4), `Editor::apply_palette` (Task 5),
  `palette_files::{read_palette, write_palette, format_for, NamedPalette}` (Task 1),
  `Platform::open_file(filters, purpose, ctx)`, `Platform::choose_save(suggested, filters) -> Option<SaveTarget>`,
  `Platform::write(&SaveTarget, &[u8]) -> Result<(), String>` y
  `ChimpApp::show_message(title, body)`.
- Produces:
  - `pub enum PaletteAction { OpenPresets, Load, Save, SaveAsPreset, SetAsDefault, LoadDefault, OpenFolder }`.
  - `pub fn palette_strip(ui, palette: &Palette, max: usize, size: f32)` en `panels`.
  - `ChimpApp` con los campos nuevos:
    - `palettes: palette_library::PaletteLibrary`;
    - `presets_window: Option<palette_ui::PresetsWindow>`;
    - `preset_name: Option<palette_ui::NameDialog>`;
    - `palette_confirm: Option<palette_ui::Confirm>`.
  - `palette_ui::PresetsWindow` tiene un campo `pub tab: PresetsTab` con la variante `Library`.
    El plan de Lospec agrega la variante `Lospec`.

Esta tarea es de UI. Los tests cubren las funciones puras nuevas (`PaletteAction` no tiene
lógica) y la verificación final es manual, con la app.

- [ ] **Step 1: Escribir los tests que fallan.** En `crates/cs-ui/src/palette_ui.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_format_follows_the_extension() {
        assert_eq!(save_format("a.hex"), PaletteFormat::Hex);
        assert_eq!(save_format("a.PAL"), PaletteFormat::Pal);
        assert_eq!(save_format("a.gpl"), PaletteFormat::Gpl);
        assert_eq!(save_format("a"), PaletteFormat::Gpl);
        assert_eq!(save_format("a.png"), PaletteFormat::Gpl);
    }

    #[test]
    fn search_matches_case_insensitive_substrings() {
        assert!(matches_search("Sweetie 16", "sweet"));
        assert!(matches_search("Sweetie 16", ""));
        assert!(!matches_search("PICO-8", "nes"));
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui palette_ui`
Expected: error de compilación.

- [ ] **Step 3: Implementar**

**3a. `crates/cs-ui/src/platform.rs`:** agregar una variante a `OpenPurpose`:

```rust
    /// Cargar una paleta (`.gpl`, `.hex`, `.pal`, `.png`, `.aseprite`).
    LoadPalette,
```

**3b. `crates/cs-ui/src/file_actions.rs`:**
- En el `match purpose` de la línea ~167, agregar:

```rust
                        OpenPurpose::LoadPalette if self.has_doc() => {
                            self.load_palette_bytes(&loaded.name, &loaded.bytes)
                        }
                        OpenPurpose::LoadPalette => {}
```

- En `new_document`, cambiar `DocState::untitled(name)` por
  `DocState::untitled_with_palette(name, self.palettes.default_palette())`.

**3c. `crates/cs-ui/src/panels.rs`:**
- Agregar `use cs_core::Palette;` donde convenga.
- Cambiar la firma a `pub fn palette(ui: &mut egui::Ui, editor: &mut Editor) -> Option<PaletteAction>`.
- Reemplazar `ui.label("Palette");` por la fila con el botón y el menú, y devolver `action` al
  final de la función:

```rust
/// Lo que se pidió desde el encabezado del panel de paleta (lo resuelve `ChimpApp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    OpenPresets,
    Load,
    Save,
    SaveAsPreset,
    SetAsDefault,
    LoadDefault,
    OpenFolder,
}
```

```rust
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label("Palette");
        if ui.small_button("Presets ▾").clicked() {
            action = Some(PaletteAction::OpenPresets);
        }
        ui.menu_button("⋯", |ui| {
            let mut item = |ui: &mut egui::Ui, label: &str, a: PaletteAction| {
                if ui.button(label).clicked() {
                    action = Some(a);
                    ui.close();
                }
            };
            item(ui, "Load Palette…", PaletteAction::Load);
            item(ui, "Save Palette…", PaletteAction::Save);
            item(ui, "Save as Preset…", PaletteAction::SaveAsPreset);
            ui.separator();
            item(ui, "Set as Default", PaletteAction::SetAsDefault);
            item(ui, "Load Default", PaletteAction::LoadDefault);
            #[cfg(not(target_arch = "wasm32"))]
            {
                ui.separator();
                item(ui, "Open Folder", PaletteAction::OpenFolder);
            }
        });
    });
```

(`ui.close()` es como egui 0.36 cierra un menú. Si el compilador lo rechaza, usar lo que ya usa
el menú "Show" en `lib.rs:314`.)

Tira de colores, compartida con la ventana Presets y con el buscador de Lospec:

```rust
/// Tira de colores de una paleta: como mucho `max` y termina en "…" si hay más.
pub fn palette_strip(ui: &mut egui::Ui, palette: &Palette, max: usize, size: f32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for &c in palette.colors().iter().take(max) {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, to_color32(c));
        }
        if palette.len() > max {
            ui.weak("…");
        }
    });
}
```

**3d. `crates/cs-ui/src/lib.rs`:**
- Agregar `mod palette_ui;`.
- Agregar las constantes y los campos:

```rust
const DEFAULT_PALETTE_KEY: &str = "chimpsprite.default_palette";
```

```rust
    palettes: palette_library::PaletteLibrary,
    presets_window: Option<palette_ui::PresetsWindow>,
    preset_name: Option<palette_ui::NameDialog>,
    palette_confirm: Option<palette_ui::Confirm>,
```

- En `new`, antes de `let mut app`:

```rust
        let mut palettes = palette_library::PaletteLibrary::for_platform();
        palettes.set_default(
            cc.storage
                .and_then(|s| eframe::get_value::<Option<String>>(s, DEFAULT_PALETTE_KEY))
                .flatten(),
        );
```

  e inicializar `palettes, presets_window: None, preset_name: None, palette_confirm: None`.

- Donde se llama al panel (línea ~764):

```rust
                            if let Some(action) = panels::palette(ui, &mut self.doc.editor) {
                                self.palette_action(ui.ctx(), action);
                            }
```

- En `eframe::App::save`:
  `eframe::set_value(storage, DEFAULT_PALETTE_KEY, &self.palettes.default_name());`.
- Donde se actualizan los demás diálogos cada frame (buscar la llamada a
  `self.update_canvas_size(` en `lib.rs`), llamar justo después a
  `self.update_palette_windows(ctx);`.
- Quitar el `#![allow(dead_code)]` provisional de `palette_library.rs`, si se agregó.

**3e. `crates/cs-ui/src/palette_ui.rs`** (arriba de `mod tests`):

```rust
//! Ventana Palette Presets, menú ⋯ del panel de paleta y sus diálogos.

use cs_io::palette_files::{self, NamedPalette, PaletteFormat};
use eframe::egui;

use crate::ChimpApp;
use crate::palette_library::{Preset, PresetSource};
use crate::panels::{self, PaletteAction};
use crate::platform::{FileFilter, OpenPurpose};

const LOAD_FILTERS: &[FileFilter] = &[FileFilter {
    name: "Palettes",
    extensions: &["gpl", "hex", "pal", "png", "ase", "aseprite"],
}];

const SAVE_FILTERS: &[FileFilter] = &[
    FileFilter { name: "GIMP Palette", extensions: &["gpl"] },
    FileFilter { name: "Hex", extensions: &["hex"] },
    FileFilter { name: "JASC Palette", extensions: &["pal"] },
];

/// Colores visibles por fila en la ventana.
const STRIP_COLORS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresetsTab {
    #[default]
    Library,
}

#[derive(Default)]
pub struct PresetsWindow {
    pub tab: PresetsTab,
    search: String,
}

/// Pedir el nombre para "Save as Preset" (y para "Set as Default" si la paleta no es un preset).
pub struct NameDialog {
    name: String,
    then_default: bool,
}

/// Confirmaciones pendientes.
pub enum Confirm {
    Replace { palette: NamedPalette, comments: Vec<String>, then_default: bool },
    Delete(String),
}

/// Formato al guardar, según la extensión elegida (`.gpl` si no se reconoce).
fn save_format(file_name: &str) -> PaletteFormat {
    match palette_files::format_for(file_name) {
        Some(f @ (PaletteFormat::Hex | PaletteFormat::Pal)) => f,
        _ => PaletteFormat::Gpl,
    }
}

fn matches_search(name: &str, search: &str) -> bool {
    name.to_lowercase().contains(&search.trim().to_lowercase())
}

impl ChimpApp {
    pub(crate) fn palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::OpenPresets => {
                self.palettes.reload();
                self.presets_window = Some(PresetsWindow::default());
            }
            PaletteAction::Load => {
                self.platform.open_file(LOAD_FILTERS, OpenPurpose::LoadPalette, ctx)
            }
            PaletteAction::Save => self.save_palette_file(),
            PaletteAction::SaveAsPreset => {
                self.preset_name = Some(NameDialog {
                    name: self.doc.tab_name_stem(),
                    then_default: false,
                })
            }
            PaletteAction::SetAsDefault => self.set_current_as_default(),
            PaletteAction::LoadDefault => {
                let palette = self.palettes.default_palette();
                self.doc.editor.apply_palette(palette);
            }
            PaletteAction::OpenFolder => {
                if let Some(dir) = self.palettes.folder() {
                    let _ = std::fs::create_dir_all(&dir);
                    ctx.open_url(egui::OpenUrl::same_tab(format!(
                        "file:///{}",
                        dir.display().to_string().replace('\\', "/")
                    )));
                }
            }
        }
    }

    pub(crate) fn load_palette_bytes(&mut self, name: &str, bytes: &[u8]) {
        match palette_files::read_palette(name, bytes) {
            Ok(p) => {
                self.doc.editor.apply_palette(p.palette);
            }
            Err(e) => self.show_message("Could not load the palette", e.to_string()),
        }
    }

    fn current_palette(&self, name: String) -> NamedPalette {
        NamedPalette { name, palette: self.doc.editor.sprite().palette.clone() }
    }

    fn save_palette_file(&mut self) {
        let suggested = format!("{}.gpl", self.doc.tab_name_stem());
        let Some(target) = self.platform.choose_save(&suggested, SAVE_FILTERS) else {
            return;
        };
        let palette = self.current_palette(target.stem().to_string());
        let bytes = palette_files::write_palette(save_format(&target.file_name), &palette)
            .expect("Gpl, Hex y Pal se pueden escribir");
        if let Err(e) = self.platform.write(&target, &bytes) {
            self.show_message("Could not save the palette", e);
        }
    }

    /// Si la paleta actual ya es un preset, lo marca; si no, primero pide un nombre.
    fn set_current_as_default(&mut self) {
        let current = self.doc.editor.sprite().palette.clone();
        let existing = self
            .palettes
            .user()
            .iter()
            .chain(self.palettes.builtin())
            .find(|p| p.palette == current)
            .map(|p| p.name.clone());
        match existing {
            Some(name) => self.palettes.set_default(Some(name)),
            None => {
                self.preset_name = Some(NameDialog {
                    name: self.doc.tab_name_stem(),
                    then_default: true,
                })
            }
        }
    }

    /// Guarda en la biblioteca y, si se pidió, la marca como la paleta por defecto.
    pub(crate) fn store_preset(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        let name = palette.name.clone();
        match self.palettes.save(&palette, &comments) {
            Ok(()) if then_default => self.palettes.set_default(Some(name)),
            Ok(()) => {}
            Err(e) => self.show_message("Could not save the preset", e),
        }
    }

    /// Pregunta si reemplazar cuando ya existe un preset de usuario con ese nombre.
    pub(crate) fn save_preset_asking(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        if self.palettes.has_user(&palette.name) {
            self.palette_confirm = Some(Confirm::Replace { palette, comments, then_default });
        } else {
            self.store_preset(palette, comments, then_default);
        }
    }

    pub(crate) fn update_palette_windows(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.presets_window = None;
            self.preset_name = None;
            self.palette_confirm = None;
            return;
        }
        self.update_presets_window(ctx);
        self.update_name_dialog(ctx);
        self.update_palette_confirm(ctx);
    }

    fn update_presets_window(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.presets_window.take() else {
            return;
        };
        let mut open = true;
        let mut apply = None;
        let mut delete = None;
        let mut set_default = None;
        egui::Window::new("Palette Presets")
            .open(&mut open)
            .default_size([420.0, 480.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut window.tab, PresetsTab::Library, "Library");
                });
                ui.separator();
                match window.tab {
                    PresetsTab::Library => {
                        ui.horizontal(|ui| {
                            ui.label("Search");
                            ui.text_edit_singleline(&mut window.search);
                        });
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let default = self.palettes.default_name().map(str::to_string);
                            for (title, list) in
                                [("Built-in", self.palettes.builtin()), ("User", self.palettes.user())]
                            {
                                ui.strong(title);
                                let shown: Vec<&Preset> = list
                                    .iter()
                                    .filter(|p| matches_search(&p.name, &window.search))
                                    .collect();
                                if shown.is_empty() {
                                    ui.weak("No palettes");
                                }
                                for preset in shown {
                                    let is_default = default
                                        .as_deref()
                                        .is_some_and(|d| d.eq_ignore_ascii_case(&preset.name));
                                    let response = preset_row(ui, preset, is_default);
                                    if response.clicked() {
                                        apply = Some(preset.palette.clone());
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Set as Default").clicked() {
                                            set_default = Some(preset.name.clone());
                                            ui.close();
                                        }
                                        if preset.source == PresetSource::User
                                            && ui.button("Delete").clicked()
                                        {
                                            delete = Some(preset.name.clone());
                                            ui.close();
                                        }
                                    });
                                }
                                ui.add_space(8.0);
                            }
                        });
                    }
                }
            });
        if let Some(palette) = apply {
            self.doc.editor.apply_palette(palette);
        }
        if let Some(name) = set_default {
            self.palettes.set_default(Some(name));
        }
        if let Some(name) = delete {
            self.palette_confirm = Some(Confirm::Delete(name));
        }
        if open {
            self.presets_window = Some(window);
        }
    }

    fn update_name_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.preset_name.take() else {
            return;
        };
        let mut result = None;
        centered_dialog("Save as Preset").show(ctx, |ui| {
            let response = ui.text_edit_singleline(&mut dialog.name);
            response.request_focus();
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let valid = !dialog.name.trim().is_empty();
                if ui.add_enabled(valid, egui::Button::new("Save")).clicked() || (enter && valid) {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            });
        });
        match result {
            None => self.preset_name = Some(dialog),
            Some(false) => {}
            Some(true) => {
                let palette = self.current_palette(dialog.name.trim().to_string());
                self.save_preset_asking(palette, Vec::new(), dialog.then_default);
            }
        }
    }

    fn update_palette_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.palette_confirm.take() else {
            return;
        };
        let (title, body, ok) = match &confirm {
            Confirm::Replace { palette, .. } => (
                "Replace preset?",
                format!("A preset named \"{}\" already exists.", palette.name),
                "Replace",
            ),
            Confirm::Delete(name) => (
                "Delete preset?",
                format!("\"{name}\" will be removed from your presets."),
                "Delete",
            ),
        };
        let mut result = None;
        centered_dialog(title).show(ctx, |ui| {
            ui.label(body);
            ui.horizontal(|ui| {
                if ui.button(ok).clicked() {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            });
        });
        match (result, confirm) {
            (None, confirm) => self.palette_confirm = Some(confirm),
            (Some(false), _) => {}
            (Some(true), Confirm::Replace { palette, comments, then_default }) => {
                self.store_preset(palette, comments, then_default)
            }
            (Some(true), Confirm::Delete(name)) => {
                if let Err(e) = self.palettes.delete(&name) {
                    self.show_message("Could not delete the preset", e);
                }
            }
        }
    }
}

/// Diálogo fijo en el centro, como "Unsaved changes" (`file_actions.rs`).
fn centered_dialog(title: &str) -> egui::Window<'static> {
    egui::Window::new(title.to_string())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
}

/// Fila de la ventana: nombre, cantidad de colores y tira. Toda la fila es clicable.
fn preset_row(ui: &mut egui::Ui, preset: &Preset, is_default: bool) -> egui::Response {
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(&preset.name);
            ui.weak(format!("{} colors", preset.palette.len()));
            if !preset.author.is_empty() {
                ui.weak(format!("· {}", preset.author));
            }
            if is_default {
                ui.weak("· default");
            }
        });
        panels::palette_strip(ui, &preset.palette, STRIP_COLORS, 8.0);
    });
    ui.interact(inner.response.rect, inner.response.id.with("row"), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}
```

`self.doc.tab_name_stem()` no existe todavía. Agregarla a `DocState` en `tabs.rs`:

```rust
    /// Nombre de la pestaña sin extensión (para sugerir nombres de archivo).
    pub fn tab_name_stem(&self) -> String {
        crate::files::stem(&self.location.name).to_string()
    }
```

`DocState::location` es un `DocLocation { name, target }` (ver `file_actions.rs`).

`centered_dialog` imita el diálogo "Unsaved changes" de `file_actions.rs` (un `egui::Window`
centrado), para que todos los diálogos se vean igual.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-ui` y `cargo check -p cs-ui --target wasm32-unknown-unknown`
Expected: todo en PASS; wasm compila.

- [ ] **Step 5: Verificación manual con la app**

Con `CARGO_TARGET_DIR=target/gui-check cargo run --bin chimpsprite` en background, antes hacer
una copia de `%APPDATA%/ChimpSprite/data/app.ron` y restaurarla al terminar. Comprobar:
- Presets ▾ abre la ventana. Al hacer clic en PICO-8, cambia la paleta del panel; Ctrl+Z la
  devuelve.
- ⋯ → Save as Preset… "Mine" → aparece en User. Guardar otra vez con el mismo nombre pregunta
  si reemplazar.
- Clic derecho en "Mine" → Set as Default. File → New: el sprite nuevo usa "Mine". Después de
  reiniciar la app, sigue siendo la paleta por defecto.
- ⋯ → Save Palette… como `.hex`, y Load Palette… del mismo archivo.
- ⋯ → Open Folder abre la carpeta `palettes/`.
- Clic derecho en "Mine" → Delete → confirmar.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/src
git commit -m "Ventana Palette Presets, menú de paleta y paleta por defecto

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Verificación final y PR

- [ ] **Step 1:** `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` y
  `cargo check -p cs-ui --target wasm32-unknown-unknown`. Todo en verde.
- [ ] **Step 2:** `git push -u origin palette-library` y
  `gh pr create --title "Biblioteca de paletas (presets, formatos, paleta por defecto)"`. El
  cuerpo lleva un resumen y los pasos de prueba manual, y termina con
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
