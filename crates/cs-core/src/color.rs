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

    /// `#rgb`, `#rrggbb` o `#rrggbbaa` (el `#` es opcional).
    pub fn parse_hex(text: &str) -> Option<Self> {
        let t = text.trim().trim_start_matches('#');
        let byte = |i: usize| u8::from_str_radix(t.get(i..i + 2)?, 16).ok();
        match t.len() {
            3 => {
                let d = |i: usize| {
                    u8::from_str_radix(t.get(i..i + 1)?, 16)
                        .ok()
                        .map(|v| v * 17)
                };
                Some(Self::rgb(d(0)?, d(1)?, d(2)?))
            }
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    /// `#rrggbb`, o `#rrggbbaa` si no es opaco.
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
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
            (ColorMode::Rgba, PixelValue::Rgba(_))
                | (ColorMode::Indexed { .. }, PixelValue::Index(_))
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
pub(crate) const DB32: [u32; 32] = [
    0x000000, 0x222034, 0x45283c, 0x663931, 0x8f563b, 0xdf7126, 0xd9a066, 0xeec39a, 0xfbf236,
    0x99e550, 0x6abe30, 0x37946e, 0x4b692f, 0x524b24, 0x323c39, 0x3f3f74, 0x306082, 0x5b6ee1,
    0x639bff, 0x5fcde4, 0xcbdbfc, 0xffffff, 0x9badb7, 0x847e87, 0x696a6a, 0x595652, 0x76428a,
    0xac3232, 0xd95763, 0xd77bba, 0x8f974a, 0x8a6f30,
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

    /// Índice del color más cercano (distancia euclidiana RGB), sin contar `transparent_index`.
    /// Con alfa < 128, o si no hay otro color, devuelve `transparent_index`.
    pub fn nearest(&self, c: Rgba, transparent_index: u8) -> u8 {
        if c.a < 128 {
            return transparent_index;
        }
        let dist = |p: &Rgba| {
            let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
            d(p.r, c.r) + d(p.g, c.g) + d(p.b, c.b)
        };
        self.colors
            .iter()
            .enumerate()
            .filter(|(i, _)| *i as u8 != transparent_index)
            .min_by_key(|(_, p)| dist(p))
            .map_or(transparent_index, |(i, _)| i as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_short_forms() {
        let c = Rgba::new(0x90, 0x52, 0xbc, 255);
        assert_eq!(c.to_hex(), "#9052bc");
        assert_eq!(Rgba::parse_hex("#9052bc"), Some(c));
        assert_eq!(Rgba::parse_hex("9052BC"), Some(c));
        assert_eq!(Rgba::parse_hex("#f0a"), Some(Rgba::rgb(255, 0, 170)));
        assert_eq!(
            Rgba::parse_hex("#11223380"),
            Some(Rgba::new(0x11, 0x22, 0x33, 0x80))
        );
        assert_eq!(Rgba::new(1, 2, 3, 4).to_hex(), "#01020304");
        assert_eq!(Rgba::parse_hex("#12"), None);
        assert_eq!(Rgba::parse_hex("#zzzzzz"), None);
    }

    #[test]
    fn nearest_skips_the_transparent_index_and_low_alpha() {
        let p = Palette::new(vec![
            Rgba::rgb(0, 0, 0),
            Rgba::rgb(10, 10, 10),
            Rgba::rgb(250, 250, 250),
        ]);
        assert_eq!(
            p.nearest(Rgba::rgb(5, 5, 5), 0),
            1,
            "index 0 is transparent"
        );
        assert_eq!(p.nearest(Rgba::rgb(200, 200, 200), 0), 2);
        assert_eq!(p.nearest(Rgba::rgb(5, 5, 5), 1), 0);
        assert_eq!(
            p.nearest(Rgba::new(250, 250, 250, 127), 0),
            0,
            "alpha < 128"
        );
        let lone = Palette::new(vec![Rgba::rgb(1, 2, 3)]);
        assert_eq!(
            lone.nearest(Rgba::rgb(1, 2, 3), 0),
            0,
            "nothing else to pick"
        );
    }
}
