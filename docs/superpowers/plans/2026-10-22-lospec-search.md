# Buscador de Lospec: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pestaña "Lospec" en la ventana Palette Presets. En escritorio, buscador completo
(tag, cantidad de colores, orden, "Load more"). En web y escritorio, traer una paleta pegando su
link o su nombre. Cada resultado se puede aplicar o guardar en la biblioteca.

**Architecture:**
- **`cs-io::lospec`:** lógica pura (URLs, parseo de JSON, slugs), probada con respuestas reales
  guardadas.
- **`cs-ui::lospec_client`:** envuelve `ehttp` y deja cada respuesta en un `Pending<T>`.
- **`cs-ui::lospec_tab`:** estado de la pestaña, sin egui ni red. Se prueba llamando a sus
  métodos `start_*` y `finish_*`.
- **`palette_ui`:** dibuja la pestaña y une todo con la biblioteca.

**Tech Stack:** Rust 2024, egui/eframe 0.36, `ehttp` 0.7, serde_json.

**Spec:** `docs/superpowers/specs/2026-10-22-lospec-search-design.md`.
**Depende de:** `docs/superpowers/plans/2026-10-22-palette-library.md`, que debe estar terminado
(usa `PaletteLibrary`, `NamedPalette`, `write_gpl` con comentarios, `Editor::apply_palette`,
`palette_ui::{PresetsWindow, PresetsTab}`, `ChimpApp::{save_preset_asking, store_preset}` y
`panels::palette_strip`).

## Global Constraints

- **Crates del motor:** `cs-io` no depende de egui ni de la red y compila en wasm32.
- **Idiomas:** código y comentarios en español; textos de la UI en inglés.
- **Peticiones:** solo al presionar Enter o un botón, nunca con cada tecla. Header
  `User-Agent: ChimpSprite/<CARGO_PKG_VERSION>` (los navegadores lo ignoran).
- **Listado:** solo en escritorio (`cfg(not(target_arch = "wasm32"))`), porque no tiene CORS. El
  JSON de cada paleta funciona en las dos plataformas.
- **Máximo de colores:** 256, como en la biblioteca.
- **Errores:** nunca hay panic; todos se muestran en la pestaña, con los textos exactos de la
  tabla de la sección 5 del spec.
- **Toolchain aislado:** antes de usar cargo, en Git Bash:
  `export RUSTUP_HOME=/d/Work/chimpdeas/.rust-toolchain/rustup CARGO_HOME=/d/Work/chimpdeas/.rust-toolchain/cargo; export PATH=$CARGO_HOME/bin:$PATH`
- **Verificación final:** `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` y
  `cargo check -p cs-ui --target wasm32-unknown-unknown`.
- **Commits:** terminan con `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **Rama:** `lospec`, rebaseada sobre `palette-library` (o sobre `main` si la biblioteca ya se
  mergeó).

## Archivos

| Archivo | Qué hace |
| --- | --- |
| Crear `crates/cs-io/src/lospec.rs` | Consultas, URLs, parseo y slugs |
| Crear `crates/cs-io/tests/fixtures/lospec/listing_gameboy.json` | Respuesta real del listado |
| Crear `crates/cs-io/tests/fixtures/lospec/sweetie-16.json` | Respuesta real de una paleta |
| Modificar `crates/cs-io/src/lib.rs` | `pub mod lospec;` |
| Crear `crates/cs-ui/src/lospec_client.rs` | `Pending<T>`, `FetchError` y peticiones con `ehttp` |
| Crear `crates/cs-ui/src/lospec_tab.rs` | Estado de la pestaña |
| Modificar `crates/cs-ui/Cargo.toml` | `ehttp = "0.7"` |
| Modificar `crates/cs-ui/src/lib.rs` | `mod lospec_client; mod lospec_tab;` |
| Modificar `crates/cs-ui/src/palette_ui.rs` | `PresetsTab::Lospec`, dibujo y acciones |

---

### Task 1: Lógica pura de Lospec (`cs-io::lospec`)

**Files:**
- Create: `crates/cs-io/src/lospec.rs`
- Create: `crates/cs-io/tests/fixtures/lospec/listing_gameboy.json` y `sweetie-16.json`
- Modify: `crates/cs-io/src/lib.rs`

**Interfaces:**
- Produces (`cs_io::lospec`):
  - Tipos: `ColorCount { Any, Exact(u16), Min(u16), Max(u16) }`,
    `LospecSort { Default, Alphabetical, Downloads, Newest }` y
    `LospecQuery { pub tag: String, pub colors: ColorCount, pub sort: LospecSort, pub page: u32 }`,
    con `LospecQuery::listing_url(&self) -> String`.
  - `LospecPalette { pub slug, pub title, pub author: Option<String>, pub palette: Palette, pub likes: Option<u32>, pub downloads: Option<u32> }`,
    con `named(&self) -> NamedPalette` y `comments(&self) -> Vec<String>`.
  - `LospecPage { pub palettes: Vec<LospecPalette>, pub total: u32 }`.
  - `LospecError { InvalidJson, TooManyColors, BadColor(usize) }`.
  - Funciones: `palette_url(slug) -> String`, `page_url(slug) -> String`,
    `parse_listing(&[u8]) -> Result<LospecPage, LospecError>`,
    `parse_palette(&[u8], slug: &str) -> Result<LospecPalette, LospecError>` y
    `slug_from_input(&str) -> Option<String>`.

- [ ] **Step 1: Guardar las fixtures reales**

```bash
mkdir -p crates/cs-io/tests/fixtures/lospec
curl -s "https://lospec.com/palette-list/load?colorNumberFilterType=any&colorNumber=8&page=0&tag=gameboy&sortingType=default" \
  -o crates/cs-io/tests/fixtures/lospec/listing_gameboy.json
curl -s "https://lospec.com/palette-list/sweetie-16.json" \
  -o crates/cs-io/tests/fixtures/lospec/sweetie-16.json
```

Comprobar que el listado tiene `"totalCount"` y 10 paletas, y que la primera es
`"Ice Cream GB"` con `"downloads"` como texto con comas. Si el orden cambió, ajustar los valores
que esperan los tests del Step 2 a lo que trae la fixture (título, total, descargas). No hay que
cambiar el código.

- [ ] **Step 2: Escribir los tests que fallan.** `crates/cs-io/src/lospec.rs` solo con:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &[u8] = include_bytes!("../tests/fixtures/lospec/listing_gameboy.json");
    const SWEETIE: &[u8] = include_bytes!("../tests/fixtures/lospec/sweetie-16.json");

    fn query(colors: ColorCount, sort: LospecSort) -> LospecQuery {
        LospecQuery { tag: "gameboy".into(), colors, sort, page: 2 }
    }

    #[test]
    fn listing_urls() {
        let base = "https://lospec.com/palette-list/load?";
        assert_eq!(
            query(ColorCount::Any, LospecSort::Default).listing_url(),
            format!("{base}colorNumberFilterType=any&colorNumber=8&page=2&tag=gameboy&sortingType=default")
        );
        assert_eq!(
            query(ColorCount::Exact(4), LospecSort::Alphabetical).listing_url(),
            format!("{base}colorNumberFilterType=exact&colorNumber=4&page=2&tag=gameboy&sortingType=alphabetical")
        );
        assert!(query(ColorCount::Min(16), LospecSort::Downloads).listing_url()
            .contains("colorNumberFilterType=min&colorNumber=16&"));
        assert!(query(ColorCount::Max(8), LospecSort::Newest).listing_url()
            .ends_with("colorNumberFilterType=max&colorNumber=8&page=2&tag=gameboy&sortingType=newest"));
        let odd = LospecQuery { tag: "  Pixel Art&Co ".into(), ..query(ColorCount::Any, LospecSort::Default) };
        assert!(odd.listing_url().contains("&tag=pixel%20art%26co&"));
    }

    #[test]
    fn palette_and_page_urls() {
        assert_eq!(palette_url("sweetie-16"), "https://lospec.com/palette-list/sweetie-16.json");
        assert_eq!(page_url("sweetie-16"), "https://lospec.com/palette-list/sweetie-16");
    }

    #[test]
    fn parses_a_real_listing() {
        let page = parse_listing(LISTING).unwrap();
        assert!(page.total >= 10);
        assert_eq!(page.palettes.len(), 10);
        let first = &page.palettes[0];
        assert_eq!(first.title, "Ice Cream GB");
        assert_eq!(first.slug, "ice-cream-gb");
        assert_eq!(first.author.as_deref(), Some("Kerrie Lake"));
        assert_eq!(first.palette.len(), 4);
        assert!(first.downloads.unwrap() > 1000, "\"25,876\" → 25876");
        assert!(first.likes.is_some());
    }

    #[test]
    fn broken_entries_are_skipped() {
        let json = br#"{"totalCount":3,"palettes":[
            {"title":"Ok","slug":"ok","colors":["000000","ffffff"],"user":{"name":"A"},"downloads":"1,234","likes":5},
            {"title":"Bad","slug":"bad","colors":["zzzzzz"]},
            {"title":"NoColors","slug":"none"}
        ]}"#;
        let page = parse_listing(json).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.palettes.len(), 1);
        assert_eq!(page.palettes[0].downloads, Some(1234));
        assert_eq!(parse_listing(b"<html>").unwrap_err(), LospecError::InvalidJson);
    }

    #[test]
    fn parses_a_real_palette() {
        let p = parse_palette(SWEETIE, "sweetie-16").unwrap();
        assert_eq!(p.title, "Sweetie 16");
        assert_eq!(p.slug, "sweetie-16");
        assert_eq!(p.palette.len(), 16);
        assert_eq!(p.palette.colors()[0], cs_core::Rgba::from_hex(0x1a1c2c));
        assert_eq!(p.author, None, "Lospec manda author vacío");
    }

    #[test]
    fn palette_errors() {
        let too_many = format!(
            r#"{{"name":"Big","author":"","colors":[{}]}}"#,
            (0..257).map(|i| format!("\"{i:06x}\"")).collect::<Vec<_>>().join(",")
        );
        assert_eq!(parse_palette(too_many.as_bytes(), "big").unwrap_err(), LospecError::TooManyColors);
        assert_eq!(
            parse_palette(br#"{"name":"X","colors":["000000","nope"]}"#, "x").unwrap_err(),
            LospecError::BadColor(1)
        );
        assert_eq!(parse_palette(b"{}", "x").unwrap_err(), LospecError::InvalidJson);
    }

    #[test]
    fn named_and_comments() {
        let p = parse_palette(br#"{"name":"Ice","author":"Kerrie","colors":["7c3f58"]}"#, "ice").unwrap();
        assert_eq!(p.named().name, "Ice");
        assert_eq!(
            p.comments(),
            vec!["Author: Kerrie".to_string(), "Source: https://lospec.com/palette-list/ice".to_string()]
        );
    }

    #[test]
    fn slugs_from_links_and_names() {
        let s = |t: &str| slug_from_input(t);
        assert_eq!(s("https://lospec.com/palette-list/sweetie-16").as_deref(), Some("sweetie-16"));
        assert_eq!(s("http://www.lospec.com/palette-list/sweetie-16.json").as_deref(), Some("sweetie-16"));
        assert_eq!(s("lospec.com/palette-list/sweetie-16/?x=1#top").as_deref(), Some("sweetie-16"));
        assert_eq!(s("sweetie-16").as_deref(), Some("sweetie-16"));
        assert_eq!(s("  Sweetie 16 ").as_deref(), Some("sweetie-16"));
        assert_eq!(s("Pokémon  Red!").as_deref(), Some("pok-mon-red"));
        assert_eq!(s("https://example.com/palette-list/x"), None);
        assert_eq!(s("https://lospec.com/pixel-art-tutorials"), None);
        assert_eq!(s("   "), None);
        assert_eq!(s("!!!"), None);
    }
}
```

En `crates/cs-io/src/lib.rs`, después de `pub mod import;`: `pub mod lospec;`.

- [ ] **Step 3: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-io lospec`
Expected: error de compilación.

- [ ] **Step 4: Implementar.** Arriba de `mod tests`:

```rust
//! Lospec (lospec.com): URLs de búsqueda y de paletas, parseo de sus respuestas y slugs.
//!
//! Sin red: el cliente HTTP vive en la UI. El listado (`/palette-list/load`) no está
//! documentado; el JSON de cada paleta (`/palette-list/<slug>.json`) sí.

use cs_core::{Palette, Rgba};
use serde_json::Value;

use crate::palette_files::{MAX_COLORS, NamedPalette};

const BASE: &str = "https://lospec.com/palette-list";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorCount {
    Any,
    Exact(u16),
    Min(u16),
    Max(u16),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LospecSort {
    #[default]
    Default,
    Alphabetical,
    Downloads,
    Newest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LospecQuery {
    pub tag: String,
    pub colors: ColorCount,
    pub sort: LospecSort,
    pub page: u32,
}

impl LospecQuery {
    pub fn listing_url(&self) -> String {
        let (filter, number) = match self.colors {
            // Lospec pide `colorNumber` aunque el filtro sea `any`.
            ColorCount::Any => ("any", 8),
            ColorCount::Exact(n) => ("exact", n),
            ColorCount::Min(n) => ("min", n),
            ColorCount::Max(n) => ("max", n),
        };
        let sort = match self.sort {
            LospecSort::Default => "default",
            LospecSort::Alphabetical => "alphabetical",
            LospecSort::Downloads => "downloads",
            LospecSort::Newest => "newest",
        };
        format!(
            "{BASE}/load?colorNumberFilterType={filter}&colorNumber={number}&page={}&tag={}&sortingType={sort}",
            self.page,
            encode_component(&self.tag.trim().to_lowercase()),
        )
    }
}

pub fn palette_url(slug: &str) -> String {
    format!("{BASE}/{slug}.json")
}

pub fn page_url(slug: &str) -> String {
    format!("{BASE}/{slug}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LospecPalette {
    pub slug: String,
    pub title: String,
    pub author: Option<String>,
    pub palette: Palette,
    pub likes: Option<u32>,
    pub downloads: Option<u32>,
}

impl LospecPalette {
    pub fn named(&self) -> NamedPalette {
        NamedPalette { name: self.title.clone(), palette: self.palette.clone() }
    }

    /// Líneas `# …` del `.gpl` guardado: autor (si hay) y página de origen.
    pub fn comments(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(author) = &self.author {
            out.push(format!("Author: {author}"));
        }
        out.push(format!("Source: {}", page_url(&self.slug)));
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LospecPage {
    pub palettes: Vec<LospecPalette>,
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LospecError {
    #[error("unexpected response from Lospec")]
    InvalidJson,
    #[error("the palette has more than 256 colors")]
    TooManyColors,
    #[error("color {0} is not valid")]
    BadColor(usize),
}

/// Una página del listado. Las paletas que no se pueden leer se saltan.
pub fn parse_listing(json: &[u8]) -> Result<LospecPage, LospecError> {
    let root: Value = serde_json::from_slice(json).map_err(|_| LospecError::InvalidJson)?;
    let entries = root["palettes"].as_array().ok_or(LospecError::InvalidJson)?;
    let total = root["totalCount"].as_u64().ok_or(LospecError::InvalidJson)? as u32;
    let palettes = entries.iter().filter_map(|e| parse_entry(e).ok()).collect();
    Ok(LospecPage { palettes, total })
}

fn parse_entry(e: &Value) -> Result<LospecPalette, LospecError> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    Ok(LospecPalette {
        slug: text(&e["slug"]).ok_or(LospecError::InvalidJson)?,
        title: text(&e["title"]).ok_or(LospecError::InvalidJson)?,
        author: text(&e["user"]["name"]).filter(|a| !a.trim().is_empty()),
        palette: parse_colors(&e["colors"])?,
        likes: e["likes"].as_u64().map(|n| n as u32),
        downloads: count(&e["downloads"]),
    })
}

/// `{ name, author, colors }` de `/palette-list/<slug>.json`.
pub fn parse_palette(json: &[u8], slug: &str) -> Result<LospecPalette, LospecError> {
    let root: Value = serde_json::from_slice(json).map_err(|_| LospecError::InvalidJson)?;
    Ok(LospecPalette {
        slug: slug.to_string(),
        title: root["name"].as_str().ok_or(LospecError::InvalidJson)?.to_string(),
        author: root["author"].as_str().map(str::to_string).filter(|a| !a.trim().is_empty()),
        palette: parse_colors(&root["colors"])?,
        likes: None,
        downloads: None,
    })
}

fn parse_colors(v: &Value) -> Result<Palette, LospecError> {
    let list = v.as_array().filter(|l| !l.is_empty()).ok_or(LospecError::InvalidJson)?;
    if list.len() > MAX_COLORS {
        return Err(LospecError::TooManyColors);
    }
    let colors = list
        .iter()
        .enumerate()
        .map(|(i, c)| {
            c.as_str()
                .filter(|h| h.len() == 6)
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .map(Rgba::from_hex)
                .ok_or(LospecError::BadColor(i))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Palette::new(colors))
}

/// Lospec manda las descargas como texto con comas (`"25,876"`) o como número.
fn count(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().map(|n| n as u32),
        Value::String(s) => s.replace(',', "").trim().parse().ok(),
        _ => None,
    }
}

/// Slug a partir de un link de Lospec, un slug o un nombre ("Sweetie 16" → `sweetie-16`).
pub fn slug_from_input(input: &str) -> Option<String> {
    let text = input.trim();
    let lower = text.to_lowercase();
    let without_scheme = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let raw = if without_scheme.contains('/') || without_scheme.contains("lospec.com") {
        let path = without_scheme
            .strip_prefix("www.")
            .unwrap_or(without_scheme)
            .strip_prefix("lospec.com/palette-list/")?;
        let path = path.split(['?', '#']).next().unwrap_or("");
        let path = path.trim_end_matches('/');
        path.strip_suffix(".json").unwrap_or(path).to_string()
    } else {
        lower
    };
    let mut slug = String::new();
    for c in raw.chars() {
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' { c } else { '-' };
        if !(c == '-' && slug.ends_with('-')) {
            slug.push(c);
        }
    }
    let slug = slug.trim_matches('-').to_string();
    (!slug.is_empty()).then_some(slug)
}

/// Codificación de URL para un valor de query (RFC 3986: solo se dejan los no reservados).
fn encode_component(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
```

Nota: `"Pokémon  Red!"` da `pok-mon-red`, porque `é` no es ASCII y pasa a `-`, que se junta con
los demás. Es lo que pide el spec.

- [ ] **Step 5: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-io lospec`
Expected: los 8 tests en PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/cs-io/src/lospec.rs crates/cs-io/src/lib.rs crates/cs-io/tests/fixtures/lospec
git commit -m "Lospec: URLs, parseo de listado y paletas, slugs

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Cliente HTTP (`cs-ui::lospec_client`)

**Files:**
- Create: `crates/cs-ui/src/lospec_client.rs`
- Modify: `crates/cs-ui/Cargo.toml` (en `[dependencies]`: `ehttp = "0.7"`)
- Modify: `crates/cs-ui/src/lib.rs` (`mod lospec_client;`)

**Interfaces:**
- Consumes: `cs_io::lospec::{LospecQuery, LospecPage, LospecPalette, LospecError, parse_listing, parse_palette, palette_url}`.
- Produces:
  - `pub enum FetchError { Network, NotFound, Status(u16), Parse(LospecError) }`.
  - `pub struct Pending<T>`, con `Pending::poll(&self) -> Option<Result<T, FetchError>>` y, en
    los tests, `Pending::ready(Result<T, FetchError>)`.
  - `pub fn fetch_palette(ctx: &egui::Context, slug: &str) -> Pending<LospecPalette>`.
  - `#[cfg(not(wasm32))] pub fn fetch_listing(ctx: &egui::Context, query: &LospecQuery) -> Pending<LospecPage>`.
  - `pub fn classify(status: u16, bytes: &[u8], parse) -> Result<T, FetchError>`, la parte
    pura, que es la que se prueba.

- [ ] **Step 1: Escribir los tests que fallan** (`crates/cs-ui/src/lospec_client.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn parse_len(bytes: &[u8]) -> Result<usize, LospecError> {
        if bytes == b"bad" { Err(LospecError::InvalidJson) } else { Ok(bytes.len()) }
    }

    #[test]
    fn status_codes_map_to_errors() {
        assert_eq!(classify(200, b"abc", parse_len), Ok(3));
        assert_eq!(classify(404, b"", parse_len), Err(FetchError::NotFound));
        assert_eq!(classify(503, b"", parse_len), Err(FetchError::Status(503)));
        assert_eq!(
            classify(200, b"bad", parse_len),
            Err(FetchError::Parse(LospecError::InvalidJson))
        );
    }

    #[test]
    fn pending_hands_the_result_once() {
        let p = Pending::ready(Ok(5));
        assert_eq!(p.poll(), Some(Ok(5)));
        assert_eq!(p.poll(), None);
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui lospec_client`
Expected: error de compilación.

- [ ] **Step 3: Implementar.** Agregar la dependencia `ehttp = "0.7"` a `crates/cs-ui/Cargo.toml`
y `mod lospec_client;` a `lib.rs`. Arriba de `mod tests`:

```rust
//! Peticiones a Lospec con `ehttp` (`ureq` en un hilo en escritorio, `fetch` en web).
//!
//! Cada petición devuelve un `Pending` que la UI revisa cada frame; al llegar la respuesta se
//! pide un repintado.

use std::sync::{Arc, Mutex};

use cs_io::lospec::{self, LospecError, LospecPalette};
use eframe::egui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// Sin red, timeout o DNS.
    Network,
    NotFound,
    Status(u16),
    Parse(LospecError),
}

/// Respuesta que todavía puede no haber llegado. `poll` la entrega una sola vez.
pub struct Pending<T> {
    slot: Arc<Mutex<Option<Result<T, FetchError>>>>,
}

impl<T> Pending<T> {
    pub fn poll(&self) -> Option<Result<T, FetchError>> {
        self.slot.lock().ok()?.take()
    }

    #[cfg(test)]
    pub fn ready(result: Result<T, FetchError>) -> Self {
        Self { slot: Arc::new(Mutex::new(Some(result))) }
    }
}

/// Lo que se hace con una respuesta: código de estado y luego el parseo.
pub fn classify<T>(
    status: u16,
    bytes: &[u8],
    parse: impl FnOnce(&[u8]) -> Result<T, LospecError>,
) -> Result<T, FetchError> {
    match status {
        200..=299 => parse(bytes).map_err(FetchError::Parse),
        404 => Err(FetchError::NotFound),
        other => Err(FetchError::Status(other)),
    }
}

fn fetch<T: Send + 'static>(
    ctx: &egui::Context,
    url: String,
    parse: impl FnOnce(&[u8]) -> Result<T, LospecError> + Send + 'static,
) -> Pending<T> {
    let slot = Arc::new(Mutex::new(None));
    let out = Arc::clone(&slot);
    let ctx = ctx.clone();
    let mut request = ehttp::Request::get(url);
    request.headers.insert(
        "User-Agent",
        concat!("ChimpSprite/", env!("CARGO_PKG_VERSION")),
    );
    ehttp::fetch(request, move |response| {
        let result = match response {
            Ok(r) => classify(r.status, &r.bytes, parse),
            Err(e) => {
                log::warn!("lospec request failed: {e}");
                Err(FetchError::Network)
            }
        };
        if let Ok(mut slot) = out.lock() {
            *slot = Some(result);
        }
        ctx.request_repaint();
    });
    Pending { slot }
}

pub fn fetch_palette(ctx: &egui::Context, slug: &str) -> Pending<LospecPalette> {
    let owned = slug.to_string();
    fetch(ctx, lospec::palette_url(slug), move |b| lospec::parse_palette(b, &owned))
}

/// Solo escritorio: el listado de Lospec no acepta peticiones desde otros sitios (sin CORS).
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_listing(
    ctx: &egui::Context,
    query: &lospec::LospecQuery,
) -> Pending<lospec::LospecPage> {
    fetch(ctx, query.listing_url(), lospec::parse_listing)
}
```

Si en ehttp 0.7 `Headers::insert` tiene otra firma, revisar `ehttp::Headers` en docs.rs y usar
el método equivalente (por ejemplo `ehttp::Headers::new(&[("User-Agent", …)])`). Lo que importa
es que se mande el header.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-ui lospec_client` y `cargo check -p cs-ui --target wasm32-unknown-unknown`
Expected: PASS; wasm compila (`ehttp` usa `fetch` allí).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/Cargo.toml crates/cs-ui/src/lospec_client.rs crates/cs-ui/src/lib.rs Cargo.lock
git commit -m "Cliente HTTP de Lospec con ehttp

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Estado de la pestaña (`cs-ui::lospec_tab`)

**Files:**
- Create: `crates/cs-ui/src/lospec_tab.rs`
- Modify: `crates/cs-ui/src/lib.rs` (`mod lospec_tab;`)

**Interfaces:**
- Consumes: `FetchError` (Task 2) y `cs_io::lospec::*` (Task 1).
- Produces (`crate::lospec_tab`):
  - `pub enum CountKind { Any, Exact, Min, Max }`.
  - `pub struct SearchStart { pub generation: u64, pub listing: LospecQuery, pub exact: Option<String> }`.
  - `pub struct LospecTab`, con los campos públicos de formulario
    `tag: String, count_kind: CountKind, count: u16, sort: LospecSort, link: String`.
  - Métodos de `LospecTab` para empezar peticiones:
    - `start_search(&mut self) -> SearchStart`;
    - `start_more(&mut self) -> Option<(u64, LospecQuery)>`;
    - `start_lookup(&mut self) -> Option<(u64, String)>`.
  - Métodos para entregar respuestas:
    - `finish_listing(&mut self, generation: u64, result: Result<LospecPage, FetchError>)`;
    - `finish_exact(&mut self, generation: u64, result: Result<LospecPalette, FetchError>)`;
    - `finish_lookup(&mut self, generation: u64, result: Result<LospecPalette, FetchError>)`.
  - Métodos para leer el estado:
    - `results(&self) -> Vec<&LospecPalette>` (la coincidencia exacta va primero, sin repetirse);
    - `is_exact(&self, slug: &str) -> bool` y `lookup_result(&self) -> Option<&LospecPalette>`;
    - `can_load_more(&self) -> bool`, `listing_loading(&self) -> bool` y
      `lookup_loading(&self) -> bool`;
    - `error(&self) -> Option<&str>` y `shown_of_total(&self) -> (usize, u32)`.

- [ ] **Step 1: Escribir los tests que fallan**

```rust
#[cfg(test)]
mod tests {
    use cs_core::{Palette, Rgba};

    use super::*;

    fn pal(slug: &str) -> LospecPalette {
        LospecPalette {
            slug: slug.into(),
            title: slug.to_uppercase(),
            author: None,
            palette: Palette::new(vec![Rgba::from_hex(0x102030)]),
            likes: None,
            downloads: None,
        }
    }

    fn page(slugs: &[&str], total: u32) -> LospecPage {
        LospecPage { palettes: slugs.iter().map(|s| pal(s)).collect(), total }
    }

    #[test]
    fn search_resets_and_load_more_appends() {
        let mut tab = LospecTab { tag: "gameboy".into(), ..LospecTab::default() };
        let s = tab.start_search();
        assert_eq!(s.listing.page, 0);
        assert_eq!(s.listing.tag, "gameboy");
        assert!(tab.listing_loading());
        tab.finish_listing(s.generation, Ok(page(&["a", "b"], 3)));
        assert!(!tab.listing_loading());
        assert!(tab.can_load_more());
        let (g, q) = tab.start_more().unwrap();
        assert_eq!(q.page, 1);
        tab.finish_listing(g, Ok(page(&["c"], 3)));
        assert_eq!(tab.results().len(), 3);
        assert!(!tab.can_load_more());
        assert_eq!(tab.shown_of_total(), (3, 3));
        let again = tab.start_search();
        assert!(tab.results().is_empty());
        assert_eq!(again.listing.page, 0);
    }

    #[test]
    fn color_filter_builds_the_query() {
        let mut tab = LospecTab { count_kind: CountKind::Max, count: 4, ..LospecTab::default() };
        assert_eq!(tab.start_search().listing.colors, ColorCount::Max(4));
        tab.count_kind = CountKind::Any;
        assert_eq!(tab.start_search().listing.colors, ColorCount::Any);
    }

    #[test]
    fn exact_match_goes_first_without_duplicates() {
        let mut tab = LospecTab { tag: "Sweetie 16".into(), ..LospecTab::default() };
        let s = tab.start_search();
        assert_eq!(s.exact.as_deref(), Some("sweetie-16"));
        tab.finish_listing(s.generation, Ok(page(&["x", "sweetie-16"], 2)));
        tab.finish_exact(s.generation, Ok(pal("sweetie-16")));
        let slugs: Vec<&str> = tab.results().iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, ["sweetie-16", "x"]);
        assert!(tab.is_exact("sweetie-16"));
    }

    #[test]
    fn exact_lookup_failure_is_silent() {
        let mut tab = LospecTab { tag: "nothing".into(), ..LospecTab::default() };
        let s = tab.start_search();
        tab.finish_exact(s.generation, Err(FetchError::NotFound));
        tab.finish_listing(s.generation, Ok(page(&[], 0)));
        assert_eq!(tab.error(), None);
    }

    #[test]
    fn stale_responses_are_ignored() {
        let mut tab = LospecTab::default();
        let old = tab.start_search();
        let new = tab.start_search();
        tab.finish_listing(old.generation, Ok(page(&["old"], 1)));
        assert!(tab.results().is_empty());
        assert!(tab.listing_loading());
        tab.finish_listing(new.generation, Ok(page(&["new"], 1)));
        assert_eq!(tab.results()[0].slug, "new");
    }

    #[test]
    fn error_messages() {
        let cases = [
            (FetchError::Network, "Couldn't reach Lospec."),
            (FetchError::Status(500), "Lospec returned an error (500)."),
            (
                FetchError::Parse(LospecError::InvalidJson),
                "Lospec search is unavailable. Paste a palette link instead.",
            ),
        ];
        for (err, msg) in cases {
            let mut tab = LospecTab::default();
            let s = tab.start_search();
            tab.finish_listing(s.generation, Err(err));
            assert_eq!(tab.error(), Some(msg));
        }
        let mut tab = LospecTab { link: "sweetie-16".into(), ..LospecTab::default() };
        let (g, slug) = tab.start_lookup().unwrap();
        assert_eq!(slug, "sweetie-16");
        tab.finish_lookup(g, Err(FetchError::NotFound));
        assert_eq!(tab.error(), Some("Palette not found on Lospec."));
        let (g, _) = tab.start_lookup().unwrap();
        tab.finish_lookup(g, Err(FetchError::Parse(LospecError::TooManyColors)));
        assert_eq!(tab.error(), Some("That palette can't be loaded."));
        let (g, _) = tab.start_lookup().unwrap();
        tab.finish_lookup(g, Ok(pal("sweetie-16")));
        assert_eq!(tab.error(), None);
        assert_eq!(tab.lookup_result().unwrap().slug, "sweetie-16");
        tab.link = "https://example.com/x".into();
        assert!(tab.start_lookup().is_none());
        assert_eq!(tab.error(), Some("Paste a lospec.com palette link or a palette name."));
    }
}
```

- [ ] **Step 2: Correr los tests y comprobar que fallan**

Run: `cargo test -p cs-ui lospec_tab`
Expected: error de compilación.

- [ ] **Step 3: Implementar.** Arriba de `mod tests`:

```rust
//! Estado de la pestaña Lospec, sin egui ni red: la UI pide con `start_*`, hace la petición y
//! entrega la respuesta con `finish_*`. Las respuestas de búsquedas viejas se descartan.

use cs_io::lospec::{
    ColorCount, LospecError, LospecPage, LospecPalette, LospecQuery, LospecSort, slug_from_input,
};

use crate::lospec_client::FetchError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CountKind {
    #[default]
    Any,
    Exact,
    Min,
    Max,
}

pub struct SearchStart {
    pub generation: u64,
    pub listing: LospecQuery,
    /// Slug a buscar en paralelo si el texto también podría ser el nombre de una paleta.
    pub exact: Option<String>,
}

pub struct LospecTab {
    pub tag: String,
    pub count_kind: CountKind,
    /// 1–256; solo cuenta si `count_kind` no es `Any`.
    pub count: u16,
    pub sort: LospecSort,
    pub link: String,
    query: Option<LospecQuery>,
    results: Vec<LospecPalette>,
    exact: Option<LospecPalette>,
    total: u32,
    generation: u64,
    listing_loading: bool,
    lookup: Option<LospecPalette>,
    lookup_generation: u64,
    lookup_loading: bool,
    error: Option<String>,
}

impl Default for LospecTab {
    fn default() -> Self {
        Self {
            tag: String::new(),
            count_kind: CountKind::Any,
            count: 8,
            sort: LospecSort::Default,
            link: String::new(),
            query: None,
            results: Vec::new(),
            exact: None,
            total: 0,
            generation: 0,
            listing_loading: false,
            lookup: None,
            lookup_generation: 0,
            lookup_loading: false,
            error: None,
        }
    }
}

impl LospecTab {
    pub fn start_search(&mut self) -> SearchStart {
        let n = self.count.clamp(1, 256);
        let colors = match self.count_kind {
            CountKind::Any => ColorCount::Any,
            CountKind::Exact => ColorCount::Exact(n),
            CountKind::Min => ColorCount::Min(n),
            CountKind::Max => ColorCount::Max(n),
        };
        let query = LospecQuery { tag: self.tag.clone(), colors, sort: self.sort, page: 0 };
        self.generation += 1;
        self.query = Some(query.clone());
        self.results.clear();
        self.exact = None;
        self.total = 0;
        self.listing_loading = true;
        self.error = None;
        SearchStart { generation: self.generation, listing: query, exact: slug_from_input(&self.tag) }
    }

    pub fn start_more(&mut self) -> Option<(u64, LospecQuery)> {
        if !self.can_load_more() || self.listing_loading {
            return None;
        }
        let query = self.query.as_mut()?;
        query.page += 1;
        let query = query.clone();
        self.generation += 1;
        self.listing_loading = true;
        self.error = None;
        Some((self.generation, query))
    }

    pub fn start_lookup(&mut self) -> Option<(u64, String)> {
        self.lookup = None;
        let Some(slug) = slug_from_input(&self.link) else {
            self.error = Some("Paste a lospec.com palette link or a palette name.".into());
            return None;
        };
        self.lookup_generation += 1;
        self.lookup_loading = true;
        self.error = None;
        Some((self.lookup_generation, slug))
    }

    pub fn finish_listing(&mut self, generation: u64, result: Result<LospecPage, FetchError>) {
        if generation != self.generation {
            return;
        }
        self.listing_loading = false;
        match result {
            Ok(page) => {
                self.total = page.total;
                self.results.extend(page.palettes);
            }
            Err(FetchError::Parse(_)) => {
                self.error =
                    Some("Lospec search is unavailable. Paste a palette link instead.".into())
            }
            Err(e) => self.error = Some(message(&e)),
        }
    }

    /// La búsqueda exacta por slug es un extra: si falla, no se muestra error.
    pub fn finish_exact(&mut self, generation: u64, result: Result<LospecPalette, FetchError>) {
        if generation == self.generation || self.query.as_ref().is_some_and(|q| q.page > 0) {
            if let Ok(p) = result {
                self.exact = Some(p);
            }
        }
    }

    pub fn finish_lookup(&mut self, generation: u64, result: Result<LospecPalette, FetchError>) {
        if generation != self.lookup_generation {
            return;
        }
        self.lookup_loading = false;
        match result {
            Ok(p) => self.lookup = Some(p),
            Err(e) => self.error = Some(message(&e)),
        }
    }

    pub fn results(&self) -> Vec<&LospecPalette> {
        let exact_slug = self.exact.as_ref().map(|p| p.slug.as_str());
        self.exact
            .iter()
            .chain(self.results.iter().filter(|p| Some(p.slug.as_str()) != exact_slug))
            .collect()
    }

    pub fn is_exact(&self, slug: &str) -> bool {
        self.exact.as_ref().is_some_and(|p| p.slug == slug)
    }

    pub fn lookup_result(&self) -> Option<&LospecPalette> {
        self.lookup.as_ref()
    }

    pub fn can_load_more(&self) -> bool {
        (self.results.len() as u32) < self.total
    }

    pub fn listing_loading(&self) -> bool {
        self.listing_loading
    }

    pub fn lookup_loading(&self) -> bool {
        self.lookup_loading
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Para "Load more (10 of 412)".
    pub fn shown_of_total(&self) -> (usize, u32) {
        (self.results.len(), self.total)
    }
}

fn message(e: &FetchError) -> String {
    match e {
        FetchError::Network => "Couldn't reach Lospec.".into(),
        FetchError::NotFound => "Palette not found on Lospec.".into(),
        FetchError::Status(code) => format!("Lospec returned an error ({code})."),
        FetchError::Parse(LospecError::InvalidJson | LospecError::TooManyColors | LospecError::BadColor(_)) => {
            "That palette can't be loaded.".into()
        }
    }
}
```

Ojo con `finish_exact`: la coincidencia exacta llega con la `generation` de `start_search`. Si
el usuario pulsa "Load more" antes de que llegue, la `generation` ya avanzó, pero la búsqueda
sigue siendo la misma (`page > 0`). Por eso la condición acepta los dos casos. Las búsquedas
nuevas siempre vuelven a `page = 0`, así que una respuesta exacta de una búsqueda vieja que
llega después de una nueva se descarta.

- [ ] **Step 4: Correr los tests y comprobar que pasan**

Run: `cargo test -p cs-ui lospec_tab`
Expected: los 6 tests en PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/src/lospec_tab.rs crates/cs-ui/src/lib.rs
git commit -m "Estado de la pestaña Lospec (búsqueda, Load more, link, errores)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Pestaña Lospec en Palette Presets

**Files:**
- Modify: `crates/cs-ui/src/palette_ui.rs`

**Interfaces:**
- Consumes:
  - `LospecTab` y `SearchStart` (Task 3); `Pending`, `fetch_palette` y `fetch_listing` (Task 2).
  - `LospecPalette::{named, comments}` (Task 1).
  - `ChimpApp::{save_preset_asking, store_preset}`, `PaletteLibrary::has_user`,
    `Editor::apply_palette` y `panels::palette_strip`, de la biblioteca.
- Produces: `PresetsTab::Lospec` y los campos de `PresetsWindow` `lospec: LospecTab` y
  `requests: LospecRequests`.

- [ ] **Step 1: Escribir el test que falla** (en `palette_ui::tests`)

```rust
    #[test]
    fn row_actions_save_with_lospec_comments() {
        let p = cs_io::lospec::parse_palette(
            br#"{"name":"Ice Cream GB","author":"Kerrie Lake","colors":["7c3f58","fff6d3"]}"#,
            "ice-cream-gb",
        )
        .unwrap();
        let (named, comments) = lospec_preset(&p);
        assert_eq!(named.name, "Ice Cream GB");
        assert_eq!(comments[0], "Author: Kerrie Lake");
        assert_eq!(comments[1], "Source: https://lospec.com/palette-list/ice-cream-gb");
    }
```

- [ ] **Step 2: Correr el test y comprobar que falla**

Run: `cargo test -p cs-ui palette_ui`
Expected: error de compilación (`lospec_preset` no existe).

- [ ] **Step 3: Implementar**

Agregar a los `use` de `palette_ui.rs`:

```rust
use cs_io::lospec::{LospecPage, LospecPalette, LospecSort};

use crate::lospec_client::{self, Pending};
use crate::lospec_tab::{CountKind, LospecTab};
```

Cambiar `PresetsTab` y `PresetsWindow`:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresetsTab {
    #[default]
    Library,
    Lospec,
}

#[derive(Default)]
pub struct PresetsWindow {
    pub tab: PresetsTab,
    search: String,
    lospec: LospecTab,
    requests: LospecRequests,
}

/// Peticiones en curso, cada una con la `generation` con la que se pidió.
#[derive(Default)]
struct LospecRequests {
    listing: Option<(u64, Pending<LospecPage>)>,
    exact: Option<(u64, Pending<LospecPalette>)>,
    lookup: Option<(u64, Pending<LospecPalette>)>,
}

impl LospecRequests {
    /// Entrega a la pestaña lo que haya llegado.
    fn poll(&mut self, tab: &mut LospecTab) {
        if let Some((g, p)) = &self.listing
            && let Some(r) = p.poll()
        {
            tab.finish_listing(*g, r);
            self.listing = None;
        }
        if let Some((g, p)) = &self.exact
            && let Some(r) = p.poll()
        {
            tab.finish_exact(*g, r);
            self.exact = None;
        }
        if let Some((g, p)) = &self.lookup
            && let Some(r) = p.poll()
        {
            tab.finish_lookup(*g, r);
            self.lookup = None;
        }
    }
}

/// Lo que se guarda en la biblioteca para un resultado de Lospec.
fn lospec_preset(p: &LospecPalette) -> (NamedPalette, Vec<String>) {
    (p.named(), p.comments())
}

/// Clic en un botón de una fila de Lospec.
enum LospecAction {
    Apply(LospecPalette),
    Save(LospecPalette),
}
```

En `update_presets_window`:
- Agregar la pestaña al selector:
  `ui.selectable_value(&mut window.tab, PresetsTab::Lospec, "Lospec");`.
- Antes de `egui::Window::new`, agregar `window.requests.poll(&mut window.lospec);` y
  `let mut lospec_action = None;`.
- En el `match window.tab`, agregar el brazo:

```rust
                    PresetsTab::Lospec => {
                        let library = &self.palettes;
                        lospec_action = lospec_ui(ui, &mut window, |name| library.has_user(name));
                    }
```

- Después de la ventana, junto a `apply`, `set_default` y `delete`:

```rust
        match lospec_action {
            Some(LospecAction::Apply(p)) => {
                self.doc.editor.apply_palette(p.palette.clone());
                if !self.palettes.has_user(&p.title) {
                    let (named, comments) = lospec_preset(&p);
                    self.store_preset(named, comments, false);
                }
            }
            Some(LospecAction::Save(p)) => {
                let (named, comments) = lospec_preset(&p);
                self.save_preset_asking(named, comments, false);
            }
            None => {}
        }
```

Funciones nuevas en `palette_ui.rs`:

```rust
/// Contenido de la pestaña Lospec. Devuelve el botón que se pulsó en alguna fila.
fn lospec_ui(
    ui: &mut egui::Ui,
    window: &mut PresetsWindow,
    in_library: impl Fn(&str) -> bool,
) -> Option<LospecAction> {
    let ctx = ui.ctx().clone();
    let PresetsWindow { lospec: tab, requests, .. } = window;
    let mut action = None;

    #[cfg(not(target_arch = "wasm32"))]
    search_form(ui, &ctx, tab, requests);

    ui.horizontal(|ui| {
        ui.label("Paste a Lospec link or palette name");
        let response = ui.text_edit_singleline(&mut tab.link);
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if tab.lookup_loading() {
            ui.spinner();
        } else if (ui.button("Fetch").clicked() || enter)
            && let Some((g, slug)) = tab.start_lookup()
        {
            requests.lookup = Some((g, lospec_client::fetch_palette(&ctx, &slug)));
        }
    });
    #[cfg(target_arch = "wasm32")]
    ui.weak("Search is available in the desktop app");

    if let Some(error) = tab.error() {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(p) = tab.lookup_result() {
            lospec_row(ui, p, false, in_library(&p.title), &mut action);
            ui.separator();
        }
        for p in tab.results() {
            lospec_row(ui, p, tab.is_exact(&p.slug), in_library(&p.title), &mut action);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if tab.can_load_more() {
            ui.vertical_centered(|ui| {
                let (shown, total) = tab.shown_of_total();
                if tab.listing_loading() {
                    ui.spinner();
                } else if ui.button(format!("Load more ({shown} of {total})")).clicked()
                    && let Some((g, query)) = tab.start_more()
                {
                    requests.listing = Some((g, lospec_client::fetch_listing(&ctx, &query)));
                }
            });
        }
    });
    action
}

#[cfg(not(target_arch = "wasm32"))]
fn search_form(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    tab: &mut LospecTab,
    requests: &mut LospecRequests,
) {
    let mut search = false;
    ui.horizontal(|ui| {
        ui.label("Tag");
        let response = ui.text_edit_singleline(&mut tab.tag);
        search |= response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.label("Colors");
        egui::ComboBox::from_id_salt("lospec_count")
            .selected_text(format!("{:?}", tab.count_kind))
            .show_ui(ui, |ui| {
                for kind in [CountKind::Any, CountKind::Exact, CountKind::Min, CountKind::Max] {
                    ui.selectable_value(&mut tab.count_kind, kind, format!("{kind:?}"));
                }
            });
        ui.add_enabled(
            tab.count_kind != CountKind::Any,
            egui::DragValue::new(&mut tab.count).range(1..=256),
        );
        ui.label("Sort");
        egui::ComboBox::from_id_salt("lospec_sort")
            .selected_text(format!("{:?}", tab.sort))
            .show_ui(ui, |ui| {
                for sort in [
                    LospecSort::Default,
                    LospecSort::Alphabetical,
                    LospecSort::Downloads,
                    LospecSort::Newest,
                ] {
                    ui.selectable_value(&mut tab.sort, sort, format!("{sort:?}"));
                }
            });
        if tab.listing_loading() && tab.shown_of_total().0 == 0 {
            ui.spinner();
        } else {
            search |= ui.button("Search").clicked();
        }
    });
    if search {
        let start = tab.start_search();
        requests.listing = Some((start.generation, lospec_client::fetch_listing(ctx, &start.listing)));
        requests.exact = start
            .exact
            .map(|slug| (start.generation, lospec_client::fetch_palette(ctx, &slug)));
    }
}

/// Fila de un resultado: título, autor, colores, likes y descargas, tira, Apply/Save y link.
fn lospec_row(
    ui: &mut egui::Ui,
    p: &LospecPalette,
    exact: bool,
    in_library: bool,
    action: &mut Option<LospecAction>,
) {
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong(&p.title);
            let mut meta = Vec::new();
            if let Some(a) = &p.author {
                meta.push(a.clone());
            }
            meta.push(format!("{} colors", p.palette.len()));
            if let Some(l) = p.likes {
                meta.push(format!("♥{l}"));
            }
            if let Some(d) = p.downloads {
                meta.push(format!("⤓{d}"));
            }
            if exact {
                meta.push("Exact match".into());
            }
            if in_library {
                meta.push("In library".into());
            }
            ui.weak(meta.join(" · "));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Save").clicked() {
                    *action = Some(LospecAction::Save(p.clone()));
                }
                if ui.button("Apply").clicked() {
                    *action = Some(LospecAction::Apply(p.clone()));
                }
            });
        });
        panels::palette_strip(ui, &p.palette, STRIP_COLORS, 8.0);
        ui.hyperlink_to("lospec.com", cs_io::lospec::page_url(&p.slug));
    });
}
```

Si el borrow checker no acepta `let PresetsWindow { lospec: tab, requests, .. } = window;` junto
con `in_library` (que toma prestado `self.palettes`), no pasa nada: `window` ya está fuera de
`self` gracias al `take()` del principio de `update_presets_window`. Si se queja el uso de
`ui.ctx()`, el `ctx.clone()` de arriba lo evita.

- [ ] **Step 4: Correr los tests y comprobar que pasan, también en wasm**

Run: `cargo test -p cs-ui` y `cargo check -p cs-ui --target wasm32-unknown-unknown`
Expected: PASS; wasm compila sin warnings de código muerto. Si `search_form`, `fetch_listing` o
`CountKind` quedan sin usar en wasm, agregar `#[cfg_attr(target_arch = "wasm32", allow(dead_code))]`
donde haga falta, como hace `recent.rs`.

- [ ] **Step 5: Verificación manual con la app (escritorio)**

Con `CARGO_TARGET_DIR=target/gui-check cargo run --bin chimpsprite` en background, antes hacer
una copia de `%APPDATA%/ChimpSprite/data/app.ron` y restaurarla al terminar. Comprobar:
- **Búsqueda:** Presets ▾ → Lospec. Con Tag `gameboy` + Enter salen 10 filas con tira y datos.
  "Load more (10 of N)" agrega 10 más.
- **Filtros:** Colors `Max` + `4` + Search devuelve solo paletas de 4 colores o menos. Sort
  `Downloads` pone primero "Resurrect 64".
- **Coincidencia exacta:** Tag `sweetie 16` + Search pone "Sweetie 16" primero, marcada como
  "Exact match".
- **Apply:** cambia la paleta del sprite; Ctrl+Z la devuelve. La paleta queda en Library → User
  como "Ice Cream GB", y su `.gpl` tiene `# Author:` y `# Source:`.
- **Save:** en una paleta que ya existe, pregunta si reemplazarla.
- **Link:** pegar `https://lospec.com/palette-list/pico-8` + Fetch muestra la fila. `no-existe-xyz`
  muestra "Palette not found on Lospec.".
- **Sin red:** con el Wi-Fi apagado, Search muestra "Couldn't reach Lospec.".

- [ ] **Step 6: Verificación en web (opcional, si hay `trunk`)**

`trunk serve` en `apps/chimpsprite` (ver `.github/workflows/pages.yml` para el comando exacto).
En la pestaña Lospec solo sale el campo de link y la nota "Search is available in the desktop
app"; Fetch de `sweetie-16` funciona.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
git add crates/cs-ui/src/palette_ui.rs
git commit -m "Pestaña Lospec en Palette Presets (buscar, Load more, link, Apply y Save)

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Test contra Lospec real, verificación final y PR

**Files:**
- Modify: `crates/cs-ui/src/lospec_client.rs` (test `#[ignore]`)

- [ ] **Step 1: Agregar el test real** (en `lospec_client::tests`; usa `ehttp::fetch_blocking`,
  que solo existe en escritorio):

```rust
    /// Contra lospec.com de verdad: `cargo test -p cs-ui -- --ignored lospec`.
    #[test]
    #[ignore]
    #[cfg(not(target_arch = "wasm32"))]
    fn live_lospec() {
        let get = |url: String| ehttp::fetch_blocking(&ehttp::Request::get(url)).unwrap();
        let q = lospec::LospecQuery {
            tag: "gameboy".into(),
            colors: lospec::ColorCount::Any,
            sort: lospec::LospecSort::Default,
            page: 0,
        };
        let r = get(q.listing_url());
        let page = classify(r.status, &r.bytes, lospec::parse_listing).unwrap();
        assert!(!page.palettes.is_empty());
        let r = get(lospec::palette_url("sweetie-16"));
        let p = classify(r.status, &r.bytes, |b| lospec::parse_palette(b, "sweetie-16")).unwrap();
        assert_eq!(p.palette.len(), 16);
    }
```

Run: `cargo test -p cs-ui -- --ignored live_lospec`
Expected: PASS (requiere internet).

- [ ] **Step 2:** `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` y
  `cargo check -p cs-ui --target wasm32-unknown-unknown`. Todo en verde.
- [ ] **Step 3: Commit**

```bash
git add crates/cs-ui/src/lospec_client.rs
git commit -m "Test manual contra Lospec real

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 4: Push y PR**
  - `git push -u origin lospec`.
  - `gh pr create --title "Buscador de Lospec en Palette Presets"`. Si la biblioteca no se ha
    mergeado, usar `--base palette-library`.
  - El cuerpo lleva un resumen, la nota de que el listado no está documentado y solo funciona
    en escritorio, y los pasos de prueba manual. Termina con
    `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
