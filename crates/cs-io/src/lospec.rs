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

/// Slug seguro para armar URLs y abrirlas: no vacío y solo `[a-z0-9-]`.
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
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
        NamedPalette {
            name: self.title.clone(),
            palette: self.palette.clone(),
        }
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
    let entries = root["palettes"]
        .as_array()
        .ok_or(LospecError::InvalidJson)?;
    let total = root["totalCount"]
        .as_u64()
        .ok_or(LospecError::InvalidJson)? as u32;
    let palettes = entries.iter().filter_map(|e| parse_entry(e).ok()).collect();
    Ok(LospecPage { palettes, total })
}

fn parse_entry(e: &Value) -> Result<LospecPalette, LospecError> {
    let text = |v: &Value| v.as_str().map(str::to_string);
    // El slug llega tal cual del JSON y termina en URLs que se abren: se valida.
    let slug = text(&e["slug"])
        .filter(|s| is_valid_slug(s))
        .ok_or(LospecError::InvalidJson)?;
    Ok(LospecPalette {
        slug,
        title: text(&e["title"]).ok_or(LospecError::InvalidJson)?,
        author: text(&e["user"]["name"]).filter(|a| !a.trim().is_empty()),
        palette: parse_colors(&e["colors"])?,
        likes: e["likes"].as_u64().map(|n| n as u32),
        downloads: count(&e["downloads"]),
    })
}

/// `{ name, author, colors }` de `/palette-list/<slug>.json`.
pub fn parse_palette(json: &[u8], slug: &str) -> Result<LospecPalette, LospecError> {
    // Los slugs vienen de `slug_from_input`, que ya los limpia; esto es por si acaso.
    if !is_valid_slug(slug) {
        return Err(LospecError::InvalidJson);
    }
    let root: Value = serde_json::from_slice(json).map_err(|_| LospecError::InvalidJson)?;
    Ok(LospecPalette {
        slug: slug.to_string(),
        title: root["name"]
            .as_str()
            .ok_or(LospecError::InvalidJson)?
            .to_string(),
        author: root["author"]
            .as_str()
            .map(str::to_string)
            .filter(|a| !a.trim().is_empty()),
        palette: parse_colors(&root["colors"])?,
        likes: None,
        downloads: None,
    })
}

fn parse_colors(v: &Value) -> Result<Palette, LospecError> {
    let list = v
        .as_array()
        .filter(|l| !l.is_empty())
        .ok_or(LospecError::InvalidJson)?;
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
        let c = if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            c
        } else {
            '-'
        };
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
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &[u8] = include_bytes!("../tests/fixtures/lospec/listing_gameboy.json");
    const SWEETIE: &[u8] = include_bytes!("../tests/fixtures/lospec/sweetie-16.json");

    fn query(colors: ColorCount, sort: LospecSort) -> LospecQuery {
        LospecQuery {
            tag: "gameboy".into(),
            colors,
            sort,
            page: 2,
        }
    }

    #[test]
    fn listing_urls() {
        let base = "https://lospec.com/palette-list/load?";
        assert_eq!(
            query(ColorCount::Any, LospecSort::Default).listing_url(),
            format!(
                "{base}colorNumberFilterType=any&colorNumber=8&page=2&tag=gameboy&sortingType=default"
            )
        );
        assert_eq!(
            query(ColorCount::Exact(4), LospecSort::Alphabetical).listing_url(),
            format!(
                "{base}colorNumberFilterType=exact&colorNumber=4&page=2&tag=gameboy&sortingType=alphabetical"
            )
        );
        assert!(
            query(ColorCount::Min(16), LospecSort::Downloads)
                .listing_url()
                .contains("colorNumberFilterType=min&colorNumber=16&")
        );
        assert!(
            query(ColorCount::Max(8), LospecSort::Newest)
                .listing_url()
                .ends_with(
                    "colorNumberFilterType=max&colorNumber=8&page=2&tag=gameboy&sortingType=newest"
                )
        );
        let odd = LospecQuery {
            tag: "  Pixel Art&Co ".into(),
            ..query(ColorCount::Any, LospecSort::Default)
        };
        assert!(odd.listing_url().contains("&tag=pixel%20art%26co&"));
    }

    #[test]
    fn palette_and_page_urls() {
        assert_eq!(
            palette_url("sweetie-16"),
            "https://lospec.com/palette-list/sweetie-16.json"
        );
        assert_eq!(
            page_url("sweetie-16"),
            "https://lospec.com/palette-list/sweetie-16"
        );
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
            {"title":"NoColors","slug":"none"},
            {"title":"Comma","slug":"x,/root,C:\\","colors":["000000"]},
            {"title":"Quote","slug":"a\"b","colors":["000000"]},
            {"title":"Empty","slug":"","colors":["000000"]},
            {"title":"Upper","slug":"Ice-Cream","colors":["000000"]}
        ]}"#;
        let page = parse_listing(json).unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.palettes.len(), 1);
        assert_eq!(page.palettes[0].downloads, Some(1234));
        assert_eq!(
            parse_listing(b"<html>").unwrap_err(),
            LospecError::InvalidJson
        );
    }

    #[test]
    fn parses_a_real_palette() {
        let p = parse_palette(SWEETIE, "sweetie-16").unwrap();
        assert_eq!(p.title, "Sweetie 16");
        assert_eq!(p.slug, "sweetie-16");
        assert_eq!(p.palette.len(), 16);
        assert_eq!(p.palette.colors()[0], cs_core::Rgba::from_hex(0x1a1c2c));
        assert_eq!(p.author, None, "Lospec manda author vacío");
        assert_eq!(
            parse_palette(SWEETIE, "x,/root").unwrap_err(),
            LospecError::InvalidJson
        );
    }

    #[test]
    fn valid_slugs_are_lowercase_digits_and_dashes() {
        assert!(is_valid_slug("sweetie-16"));
        assert!(is_valid_slug("pico-8"));
        assert!(!is_valid_slug(""));
        assert!(!is_valid_slug("Sweetie"));
        assert!(!is_valid_slug("a b"));
        assert!(!is_valid_slug("x,/root,C:\\"));
        assert!(!is_valid_slug("a\"b"));
        assert!(!is_valid_slug("../x"));
    }

    #[test]
    fn palette_errors() {
        let too_many = format!(
            r#"{{"name":"Big","author":"","colors":[{}]}}"#,
            (0..257)
                .map(|i| format!("\"{i:06x}\""))
                .collect::<Vec<_>>()
                .join(",")
        );
        assert_eq!(
            parse_palette(too_many.as_bytes(), "big").unwrap_err(),
            LospecError::TooManyColors
        );
        assert_eq!(
            parse_palette(br#"{"name":"X","colors":["000000","nope"]}"#, "x").unwrap_err(),
            LospecError::BadColor(1)
        );
        assert_eq!(
            parse_palette(b"{}", "x").unwrap_err(),
            LospecError::InvalidJson
        );
    }

    #[test]
    fn named_and_comments() {
        let p = parse_palette(
            br#"{"name":"Ice","author":"Kerrie","colors":["7c3f58"]}"#,
            "ice",
        )
        .unwrap();
        assert_eq!(p.named().name, "Ice");
        assert_eq!(
            p.comments(),
            vec![
                "Author: Kerrie".to_string(),
                "Source: https://lospec.com/palette-list/ice".to_string()
            ]
        );
    }

    #[test]
    fn slugs_from_links_and_names() {
        let s = |t: &str| slug_from_input(t);
        assert_eq!(
            s("https://lospec.com/palette-list/sweetie-16").as_deref(),
            Some("sweetie-16")
        );
        assert_eq!(
            s("http://www.lospec.com/palette-list/sweetie-16.json").as_deref(),
            Some("sweetie-16")
        );
        assert_eq!(
            s("lospec.com/palette-list/sweetie-16/?x=1#top").as_deref(),
            Some("sweetie-16")
        );
        assert_eq!(s("sweetie-16").as_deref(), Some("sweetie-16"));
        assert_eq!(s("  Sweetie 16 ").as_deref(), Some("sweetie-16"));
        assert_eq!(s("Pokémon  Red!").as_deref(), Some("pok-mon-red"));
        assert_eq!(s("https://example.com/palette-list/x"), None);
        assert_eq!(s("https://lospec.com/pixel-art-tutorials"), None);
        assert_eq!(s("   "), None);
        assert_eq!(s("!!!"), None);
    }
}
