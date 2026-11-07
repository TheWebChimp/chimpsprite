//! Paletas pegadas como texto: links de Coolors (`coolors.co/palette/264653-2a9d8f-…`) o listas de
//! colores hex (`#264653, 2a9d8f …`, también los exports de código de Coolors). Puro, sin red.

use cs_core::{Palette, Rgba};

/// Colores como máximo (los de más se ignoran).
pub const MAX_COLORS: usize = 256;

/// Colores del nombre antes de recortarlo.
const NAME_COLORS: usize = 5;

/// Una paleta leída de un texto pegado.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PastedPalette {
    /// `Coolors <hex-hex-…>` o `Pasted <hex-hex-…>` (recortado si es larga).
    pub name: String,
    pub palette: Palette,
    /// La página de Coolors de la paleta (solo si vino de un link).
    pub source: Option<String>,
    /// Vino de un link de Coolors (no de una lista de colores).
    pub from_link: bool,
}

impl PastedPalette {
    /// Seguro que es una paleta (para Ctrl+V fuera de un campo): un link, o al menos dos colores
    /// (un solo número hex puede ser cualquier otra cosa).
    pub fn is_confident(&self) -> bool {
        self.from_link || self.palette.len() >= 2
    }
}

/// La página de Coolors de unos colores `rrggbb` separados por guiones.
pub fn coolors_url(slug: &str) -> String {
    format!("https://coolors.co/palette/{slug}")
}

/// Lee una paleta de `text`: un link de Coolors, una lista de colores hex (separados por
/// guiones, comas, espacios, `;` o saltos; con o sin `#`) o un texto con colores `#rrggbb` o
/// `"rrggbb"` mezclados (los exports de código de Coolors). `None` si no es nada de eso.
pub fn parse_palette_text(text: &str) -> Option<PastedPalette> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Some(slug) = coolors_slug(text) {
        let colors = slug
            .split('-')
            .map(|t| six_digits(t).then(|| Rgba::parse_hex(t)).flatten())
            .collect::<Option<Vec<_>>>()?;
        return Some(pasted(colors, true));
    }
    let colors = hex_list(text).or_else(|| embedded_colors(text))?;
    Some(pasted(colors, false))
}

/// La última parte del camino de un link de `coolors.co` (lo que debería ser la paleta).
fn coolors_slug(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    let path = rest.strip_prefix("coolors.co/")?;
    let path = path.split(['?', '#']).next().unwrap_or("");
    let last = path.trim_end_matches('/').rsplit('/').next()?;
    Some(last.to_string())
}

fn six_digits(t: &str) -> bool {
    t.len() == 6 && t.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Un color de una lista: `rrggbb` o `rrggbbaa` (con o sin `#`) o `#rgb`.
fn list_color(token: &str) -> Option<Rgba> {
    let (hash, digits) = match token.strip_prefix('#') {
        Some(d) => (true, d),
        None => (false, token),
    };
    let digits = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
        .unwrap_or(digits);
    let ok_len = matches!(digits.len(), 6 | 8) || (hash && digits.len() == 3);
    (ok_len && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| Rgba::parse_hex(digits))
        .flatten()
}

/// Todo el texto es una lista de colores (las comillas y corchetes de un arreglo JSON valen como
/// separadores).
fn hex_list(text: &str) -> Option<Vec<Rgba>> {
    let separators = |c: char| {
        c.is_whitespace()
            || matches!(
                c,
                ',' | ';' | '-' | '|' | '"' | '\'' | '[' | ']' | '(' | ')'
            )
    };
    let colors = text
        .split(separators)
        .filter(|t| !t.is_empty())
        .map(list_color)
        .collect::<Option<Vec<_>>>()?;
    (!colors.is_empty()).then_some(colors)
}

/// Los colores `#rrggbb(aa)` o `"rrggbb(aa)"` metidos en un texto (CSS, SCSS, JSON…).
fn embedded_colors(text: &str) -> Option<Vec<Rgba>> {
    let bytes = text.as_bytes();
    let mut colors = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let run = &text[start..i];
        let before = start.checked_sub(1).map(|j| bytes[j]);
        let after = bytes.get(i).copied();
        let quoted = matches!(
            (before, after),
            (Some(b'"'), Some(b'"')) | (Some(b'\''), Some(b'\''))
        );
        let marked = before == Some(b'#') || quoted;
        if marked
            && matches!(run.len(), 6 | 8)
            && run.bytes().all(|b| b.is_ascii_hexdigit())
            && let Some(c) = Rgba::parse_hex(run)
        {
            colors.push(c);
        }
    }
    (!colors.is_empty()).then_some(colors)
}

/// El hex de un color para el nombre: `rrggbb`, o `rrggbbaa` si no es opaco.
fn hex(c: Rgba) -> String {
    if c.a == 255 {
        format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

fn pasted(mut colors: Vec<Rgba>, from_link: bool) -> PastedPalette {
    colors.truncate(MAX_COLORS);
    let slug = |n: usize| {
        colors
            .iter()
            .take(n)
            .map(|&c| hex(c))
            .collect::<Vec<_>>()
            .join("-")
    };
    let mut short = slug(NAME_COLORS);
    if colors.len() > NAME_COLORS {
        short.push_str(&format!(" +{}", colors.len() - NAME_COLORS));
    }
    let prefix = if from_link { "Coolors" } else { "Pasted" };
    PastedPalette {
        name: format!("{prefix} {short}"),
        source: from_link.then(|| coolors_url(&slug(colors.len()))),
        palette: Palette::new(colors),
        from_link,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexes(p: &PastedPalette) -> Vec<String> {
        p.palette.colors().iter().map(|&c| hex(c)).collect()
    }

    const FIVE: [&str; 5] = ["264653", "2a9d8f", "e9c46a", "f4a261", "e76f51"];

    #[test]
    fn coolors_links() {
        let p = parse_palette_text("https://coolors.co/palette/264653-2a9d8f-e9c46a-f4a261-e76f51")
            .unwrap();
        assert_eq!(hexes(&p), FIVE);
        assert!(p.from_link && p.is_confident());
        assert_eq!(p.name, "Coolors 264653-2a9d8f-e9c46a-f4a261-e76f51");
        assert_eq!(
            p.source.as_deref(),
            Some("https://coolors.co/palette/264653-2a9d8f-e9c46a-f4a261-e76f51")
        );
        // Sin `/palette/`, sin esquema, con www, mayúsculas, query y barra final.
        for link in [
            "https://coolors.co/264653-2a9d8f-e9c46a",
            "coolors.co/264653-2a9d8f-e9c46a",
            "  http://www.Coolors.co/palette/264653-2A9D8F-e9c46a/?ref=x#top ",
        ] {
            let p = parse_palette_text(link).unwrap();
            assert_eq!(hexes(&p), FIVE[..3], "{link}");
        }
        // Un link de Coolors que no es una paleta.
        assert_eq!(parse_palette_text("https://coolors.co/generate"), None);
        assert_eq!(
            parse_palette_text("https://coolors.co/palette/264653-zzzzzz"),
            None
        );
        assert_eq!(parse_palette_text("https://coolors.co/"), None);
    }

    #[test]
    fn hex_lists_with_any_separator() {
        for text in [
            "264653-2a9d8f-e9c46a-f4a261-e76f51",
            "#264653, #2a9d8f, #e9c46a, #f4a261, #e76f51",
            "264653 2a9d8f\ne9c46a\r\nf4a261;e76f51",
            "[\"264653\",\"2a9d8f\",\"e9c46a\",\"f4a261\",\"e76f51\"]",
            "0x264653 0x2a9d8f 0xe9c46a 0xf4a261 0xe76f51",
        ] {
            let p = parse_palette_text(text).unwrap();
            assert_eq!(hexes(&p), FIVE, "{text:?}");
            assert!(!p.from_link);
            assert_eq!(p.source, None);
            assert_eq!(p.name, "Pasted 264653-2a9d8f-e9c46a-f4a261-e76f51");
        }
    }

    #[test]
    fn short_and_alpha_colors() {
        let p = parse_palette_text("#fff #000").unwrap();
        assert_eq!(hexes(&p), ["ffffff", "000000"]);
        let p = parse_palette_text("26465380 2a9d8f").unwrap();
        assert_eq!(p.palette.colors()[0], Rgba::new(0x26, 0x46, 0x53, 0x80));
        assert_eq!(p.name, "Pasted 26465380-2a9d8f");
        // Sin `#`, tres dígitos son un número cualquiera.
        assert_eq!(parse_palette_text("fff 000"), None);
    }

    #[test]
    fn coolors_code_exports() {
        let css = "/* CSS HEX */\n--charcoal: #264653ff;\n--persian-green: #2a9d8fff;\n";
        assert_eq!(
            hexes(&parse_palette_text(css).unwrap()),
            ["264653", "2a9d8f"]
        );
        let object = r#"{"Charcoal":"264653","Persian green":"2a9d8f","Saffron":"e9c46a"}"#;
        assert_eq!(hexes(&parse_palette_text(object).unwrap()), FIVE[..3]);
        let scss = "$charcoal: #264653ff; // rgba(38, 70, 83, 1)";
        assert_eq!(hexes(&parse_palette_text(scss).unwrap()), ["264653"]);
    }

    #[test]
    fn text_that_is_not_a_palette() {
        for text in [
            "",
            "   ",
            "hello world",
            "the color is cafe",
            "https://lospec.com/palette-list/sweetie-16",
            "12345",
            "abcdefg",
            "#12345",
        ] {
            assert_eq!(parse_palette_text(text), None, "{text:?}");
        }
        // Un número de 6 dígitos sí es un color, pero no "seguro" para Ctrl+V.
        let one = parse_palette_text("100000").unwrap();
        assert!(!one.is_confident());
    }

    #[test]
    fn long_lists_are_capped_and_the_name_is_short() {
        let text: Vec<String> = (0..300).map(|i| format!("{:06x}", i * 1000)).collect();
        let p = parse_palette_text(&text.join(" ")).unwrap();
        assert_eq!(p.palette.len(), MAX_COLORS);
        assert_eq!(p.name, "Pasted 000000-0003e8-0007d0-000bb8-000fa0 +251");
    }
}
