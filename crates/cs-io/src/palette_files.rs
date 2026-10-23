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

/// PNG indexado: su PLTE. Si no: los colores opacos únicos en orden de aparición.
fn read_png(data: &[u8]) -> Result<Vec<Rgba>, PaletteError> {
    let image_error = |e: String| PaletteError::Image(e);
    let decoder = png::Decoder::new(std::io::Cursor::new(data));
    let reader = decoder
        .read_info()
        .map_err(|e| image_error(e.to_string()))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, colors: &[u32]) -> NamedPalette {
        NamedPalette {
            name: name.into(),
            palette: Palette::new(colors.iter().map(|&c| Rgba::from_hex(c)).collect()),
        }
    }

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
        let text = "GIMP Palette\r\n# made by hand\r\nColumns: 4\r\n\r\n  0   0   0\tBlack\r\n255 255 255 White\n";
        let p = read_palette("mono.gpl", text.as_bytes()).unwrap();
        assert_eq!(p, named("mono", &[0x000000, 0xffffff]));
        let with_name = "GIMP Palette\nName: Night\n10 20 30\n";
        assert_eq!(
            read_palette("x.gpl", with_name.as_bytes()).unwrap().name,
            "Night"
        );
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
        assert_eq!(
            gpl.to_string(),
            "line 2: \"300\" is not a number from 0 to 255"
        );
        let header = read_palette("a.gpl", b"hello\n").unwrap_err();
        assert_eq!(header.to_string(), "line 1: expected \"GIMP Palette\"");
        let hex = read_palette("a.hex", b"ff0000\nzz0000\n").unwrap_err();
        assert_eq!(
            hex.to_string(),
            "line 2: \"zz0000\" is not a color (rrggbb)"
        );
        let pal = read_palette("a.pal", b"JASC-PAL\r\n0100\r\n2\r\n0 0 0\r\n").unwrap_err();
        assert_eq!(pal.to_string(), "line 3: the file says 2 colors but has 1");
        assert_eq!(read_palette("a.hex", b"").unwrap_err(), PaletteError::Empty);
        assert_eq!(
            read_palette("a.hex", b"\n\n").unwrap_err(),
            PaletteError::NoColors
        );
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
