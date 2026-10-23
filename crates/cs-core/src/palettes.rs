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
    BuiltinPalette {
        name: "DB16",
        author: "DawnBringer",
        colors: &DB16,
    },
    BuiltinPalette {
        name: "DB32",
        author: "DawnBringer",
        colors: &DB32,
    },
    BuiltinPalette {
        name: "PICO-8",
        author: "Lexaloffle",
        colors: &PICO8,
    },
    BuiltinPalette {
        name: "Game Boy",
        author: "Nintendo",
        colors: &GAME_BOY,
    },
    BuiltinPalette {
        name: "NES",
        author: "Nintendo",
        colors: &NES,
    },
    BuiltinPalette {
        name: "Sweetie 16",
        author: "GrafxKid",
        colors: &SWEETIE16,
    },
    BuiltinPalette {
        name: "Endesga 32",
        author: "ENDESGA",
        colors: &ENDESGA32,
    },
    BuiltinPalette {
        name: "AAP-64",
        author: "Adigun A. Polack",
        colors: &AAP64,
    },
    BuiltinPalette {
        name: "CGA",
        author: "IBM",
        colors: &CGA,
    },
    BuiltinPalette {
        name: "1-bit",
        author: "",
        colors: &[0x000000, 0xffffff],
    },
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
            BUILTIN_PALETTES
                .iter()
                .find(|p| p.name == name)
                .unwrap()
                .palette()
                .len()
        };
        assert_eq!(count("PICO-8"), 16);
        assert_eq!(count("Game Boy"), 4);
        assert_eq!(count("NES"), 55);
        assert_eq!(count("AAP-64"), 64);
        assert_eq!(count("1-bit"), 2);
    }
}
