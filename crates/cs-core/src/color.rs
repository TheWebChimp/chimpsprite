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
const DB32: [u32; 32] = [
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
}
