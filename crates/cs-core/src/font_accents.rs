//! Los acentuados latinos como glifos compuestos: la tabla de descomposición canónica (NFD) de
//! Latin-1 Supplement, Latin Extended-A y Ș ș Ț ț (generada con `unicodedata` de Python), de
//! dónde sale cada marca y dónde se pone sobre la tinta de la base.

use crate::{FontInfo, GlyphComponent, InkBounds, Sprite};

/// Carácter → (base, marca combinante), en orden.
pub const DECOMPOSITIONS: &[(char, char, char)] = &[
    ('\u{00C0}', 'A', '\u{0300}'),
    ('\u{00C1}', 'A', '\u{0301}'),
    ('\u{00C2}', 'A', '\u{0302}'),
    ('\u{00C3}', 'A', '\u{0303}'),
    ('\u{00C4}', 'A', '\u{0308}'),
    ('\u{00C5}', 'A', '\u{030A}'),
    ('\u{00C7}', 'C', '\u{0327}'),
    ('\u{00C8}', 'E', '\u{0300}'),
    ('\u{00C9}', 'E', '\u{0301}'),
    ('\u{00CA}', 'E', '\u{0302}'),
    ('\u{00CB}', 'E', '\u{0308}'),
    ('\u{00CC}', 'I', '\u{0300}'),
    ('\u{00CD}', 'I', '\u{0301}'),
    ('\u{00CE}', 'I', '\u{0302}'),
    ('\u{00CF}', 'I', '\u{0308}'),
    ('\u{00D1}', 'N', '\u{0303}'),
    ('\u{00D2}', 'O', '\u{0300}'),
    ('\u{00D3}', 'O', '\u{0301}'),
    ('\u{00D4}', 'O', '\u{0302}'),
    ('\u{00D5}', 'O', '\u{0303}'),
    ('\u{00D6}', 'O', '\u{0308}'),
    ('\u{00D9}', 'U', '\u{0300}'),
    ('\u{00DA}', 'U', '\u{0301}'),
    ('\u{00DB}', 'U', '\u{0302}'),
    ('\u{00DC}', 'U', '\u{0308}'),
    ('\u{00DD}', 'Y', '\u{0301}'),
    ('\u{00E0}', 'a', '\u{0300}'),
    ('\u{00E1}', 'a', '\u{0301}'),
    ('\u{00E2}', 'a', '\u{0302}'),
    ('\u{00E3}', 'a', '\u{0303}'),
    ('\u{00E4}', 'a', '\u{0308}'),
    ('\u{00E5}', 'a', '\u{030A}'),
    ('\u{00E7}', 'c', '\u{0327}'),
    ('\u{00E8}', 'e', '\u{0300}'),
    ('\u{00E9}', 'e', '\u{0301}'),
    ('\u{00EA}', 'e', '\u{0302}'),
    ('\u{00EB}', 'e', '\u{0308}'),
    ('\u{00EC}', 'i', '\u{0300}'),
    ('\u{00ED}', 'i', '\u{0301}'),
    ('\u{00EE}', 'i', '\u{0302}'),
    ('\u{00EF}', 'i', '\u{0308}'),
    ('\u{00F1}', 'n', '\u{0303}'),
    ('\u{00F2}', 'o', '\u{0300}'),
    ('\u{00F3}', 'o', '\u{0301}'),
    ('\u{00F4}', 'o', '\u{0302}'),
    ('\u{00F5}', 'o', '\u{0303}'),
    ('\u{00F6}', 'o', '\u{0308}'),
    ('\u{00F9}', 'u', '\u{0300}'),
    ('\u{00FA}', 'u', '\u{0301}'),
    ('\u{00FB}', 'u', '\u{0302}'),
    ('\u{00FC}', 'u', '\u{0308}'),
    ('\u{00FD}', 'y', '\u{0301}'),
    ('\u{00FF}', 'y', '\u{0308}'),
    ('\u{0100}', 'A', '\u{0304}'),
    ('\u{0101}', 'a', '\u{0304}'),
    ('\u{0102}', 'A', '\u{0306}'),
    ('\u{0103}', 'a', '\u{0306}'),
    ('\u{0104}', 'A', '\u{0328}'),
    ('\u{0105}', 'a', '\u{0328}'),
    ('\u{0106}', 'C', '\u{0301}'),
    ('\u{0107}', 'c', '\u{0301}'),
    ('\u{0108}', 'C', '\u{0302}'),
    ('\u{0109}', 'c', '\u{0302}'),
    ('\u{010A}', 'C', '\u{0307}'),
    ('\u{010B}', 'c', '\u{0307}'),
    ('\u{010C}', 'C', '\u{030C}'),
    ('\u{010D}', 'c', '\u{030C}'),
    ('\u{010E}', 'D', '\u{030C}'),
    ('\u{010F}', 'd', '\u{030C}'),
    ('\u{0112}', 'E', '\u{0304}'),
    ('\u{0113}', 'e', '\u{0304}'),
    ('\u{0114}', 'E', '\u{0306}'),
    ('\u{0115}', 'e', '\u{0306}'),
    ('\u{0116}', 'E', '\u{0307}'),
    ('\u{0117}', 'e', '\u{0307}'),
    ('\u{0118}', 'E', '\u{0328}'),
    ('\u{0119}', 'e', '\u{0328}'),
    ('\u{011A}', 'E', '\u{030C}'),
    ('\u{011B}', 'e', '\u{030C}'),
    ('\u{011C}', 'G', '\u{0302}'),
    ('\u{011D}', 'g', '\u{0302}'),
    ('\u{011E}', 'G', '\u{0306}'),
    ('\u{011F}', 'g', '\u{0306}'),
    ('\u{0120}', 'G', '\u{0307}'),
    ('\u{0121}', 'g', '\u{0307}'),
    ('\u{0122}', 'G', '\u{0327}'),
    ('\u{0123}', 'g', '\u{0327}'),
    ('\u{0124}', 'H', '\u{0302}'),
    ('\u{0125}', 'h', '\u{0302}'),
    ('\u{0128}', 'I', '\u{0303}'),
    ('\u{0129}', 'i', '\u{0303}'),
    ('\u{012A}', 'I', '\u{0304}'),
    ('\u{012B}', 'i', '\u{0304}'),
    ('\u{012C}', 'I', '\u{0306}'),
    ('\u{012D}', 'i', '\u{0306}'),
    ('\u{012E}', 'I', '\u{0328}'),
    ('\u{012F}', 'i', '\u{0328}'),
    ('\u{0130}', 'I', '\u{0307}'),
    ('\u{0134}', 'J', '\u{0302}'),
    ('\u{0135}', 'j', '\u{0302}'),
    ('\u{0136}', 'K', '\u{0327}'),
    ('\u{0137}', 'k', '\u{0327}'),
    ('\u{0139}', 'L', '\u{0301}'),
    ('\u{013A}', 'l', '\u{0301}'),
    ('\u{013B}', 'L', '\u{0327}'),
    ('\u{013C}', 'l', '\u{0327}'),
    ('\u{013D}', 'L', '\u{030C}'),
    ('\u{013E}', 'l', '\u{030C}'),
    ('\u{0143}', 'N', '\u{0301}'),
    ('\u{0144}', 'n', '\u{0301}'),
    ('\u{0145}', 'N', '\u{0327}'),
    ('\u{0146}', 'n', '\u{0327}'),
    ('\u{0147}', 'N', '\u{030C}'),
    ('\u{0148}', 'n', '\u{030C}'),
    ('\u{014C}', 'O', '\u{0304}'),
    ('\u{014D}', 'o', '\u{0304}'),
    ('\u{014E}', 'O', '\u{0306}'),
    ('\u{014F}', 'o', '\u{0306}'),
    ('\u{0150}', 'O', '\u{030B}'),
    ('\u{0151}', 'o', '\u{030B}'),
    ('\u{0154}', 'R', '\u{0301}'),
    ('\u{0155}', 'r', '\u{0301}'),
    ('\u{0156}', 'R', '\u{0327}'),
    ('\u{0157}', 'r', '\u{0327}'),
    ('\u{0158}', 'R', '\u{030C}'),
    ('\u{0159}', 'r', '\u{030C}'),
    ('\u{015A}', 'S', '\u{0301}'),
    ('\u{015B}', 's', '\u{0301}'),
    ('\u{015C}', 'S', '\u{0302}'),
    ('\u{015D}', 's', '\u{0302}'),
    ('\u{015E}', 'S', '\u{0327}'),
    ('\u{015F}', 's', '\u{0327}'),
    ('\u{0160}', 'S', '\u{030C}'),
    ('\u{0161}', 's', '\u{030C}'),
    ('\u{0162}', 'T', '\u{0327}'),
    ('\u{0163}', 't', '\u{0327}'),
    ('\u{0164}', 'T', '\u{030C}'),
    ('\u{0165}', 't', '\u{030C}'),
    ('\u{0168}', 'U', '\u{0303}'),
    ('\u{0169}', 'u', '\u{0303}'),
    ('\u{016A}', 'U', '\u{0304}'),
    ('\u{016B}', 'u', '\u{0304}'),
    ('\u{016C}', 'U', '\u{0306}'),
    ('\u{016D}', 'u', '\u{0306}'),
    ('\u{016E}', 'U', '\u{030A}'),
    ('\u{016F}', 'u', '\u{030A}'),
    ('\u{0170}', 'U', '\u{030B}'),
    ('\u{0171}', 'u', '\u{030B}'),
    ('\u{0172}', 'U', '\u{0328}'),
    ('\u{0173}', 'u', '\u{0328}'),
    ('\u{0174}', 'W', '\u{0302}'),
    ('\u{0175}', 'w', '\u{0302}'),
    ('\u{0176}', 'Y', '\u{0302}'),
    ('\u{0177}', 'y', '\u{0302}'),
    ('\u{0178}', 'Y', '\u{0308}'),
    ('\u{0179}', 'Z', '\u{0301}'),
    ('\u{017A}', 'z', '\u{0301}'),
    ('\u{017B}', 'Z', '\u{0307}'),
    ('\u{017C}', 'z', '\u{0307}'),
    ('\u{017D}', 'Z', '\u{030C}'),
    ('\u{017E}', 'z', '\u{030C}'),
    ('\u{0218}', 'S', '\u{0326}'),
    ('\u{0219}', 's', '\u{0326}'),
    ('\u{021A}', 'T', '\u{0326}'),
    ('\u{021B}', 't', '\u{0326}'),
];

/// La base y la marca combinante de `ch` (si está en la tabla).
pub fn decomposition(ch: char) -> Option<(char, char)> {
    DECOMPOSITIONS
        .binary_search_by_key(&ch, |d| d.0)
        .ok()
        .map(|i| (DECOMPOSITIONS[i].1, DECOMPOSITIONS[i].2))
}

/// Los caracteres que sabe armar Auto-build accents.
pub fn accent_chars() -> impl Iterator<Item = char> {
    DECOMPOSITIONS.iter().map(|d| d.0)
}

/// Las marcas espaciadoras (las que se dibujan solas) de cada marca combinante.
fn spacing_marks(mark: char) -> &'static [char] {
    match mark {
        '\u{0300}' => &['`'],
        '\u{0301}' => &['´'],
        '\u{0302}' => &['ˆ', '^'],
        '\u{0303}' => &['˜', '~'],
        '\u{0304}' => &['¯'],
        '\u{0306}' => &['˘'],
        '\u{0307}' => &['˙'],
        '\u{0308}' => &['¨'],
        '\u{030A}' => &['˚'],
        '\u{030B}' => &['˝'],
        '\u{030C}' => &['ˇ'],
        '\u{0326}' => &[','],
        '\u{0327}' => &['¸'],
        '\u{0328}' => &['˛'],
        _ => &[],
    }
}

/// De dónde sale la marca de `ch`, en orden: en minúsculas la espaciadora y al final la
/// combinante; en mayúsculas primero la combinante (la "versión cap", si alguien la dibujó).
pub fn mark_sources(ch: char, mark: char) -> Vec<char> {
    let spacing = spacing_marks(mark).iter().copied();
    if ch.is_uppercase() {
        std::iter::once(mark).chain(spacing).collect()
    } else {
        spacing.chain(std::iter::once(mark)).collect()
    }
}

/// Dónde va una marca.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// Arriba, centrada, con 1 px de aire.
    Above,
    /// Debajo, centrada y pegada (cedilla, coma).
    Below,
    /// Debajo, pegada y a la derecha (ogonek).
    Ogonek,
    /// A la derecha, arriba (la caron de ď ľ ť Ľ hecha con un apóstrofo).
    Right,
}

/// Las letras cuya caron se escribe como apóstrofo a la derecha.
const APOSTROPHE_CARON: &[char] = &['\u{010F}', '\u{013D}', '\u{013E}', '\u{0165}'];

/// El compuesto que arma Auto-build accents para un carácter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccentPlan {
    /// La base en (0, 0) y la marca en su lugar.
    pub components: Vec<GlyphComponent>,
    /// La marca no cupo con su aire y quedó más pegada (o recortada).
    pub cramped: bool,
}

/// Por qué no se arma un acentuado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccentSkip {
    /// No está en la tabla.
    Unknown,
    /// La fuente no tiene la base, o está vacía.
    NoBase,
    /// La fuente no tiene ninguna de sus marcas con tinta.
    NoMark,
}

impl FontInfo {
    /// El glifo de `ch` y la caja de lo que se ve, si tiene tinta.
    fn inked(&self, sprite: &Sprite, ch: char) -> Option<InkBounds> {
        self.ink_bounds(sprite, self.index_of(ch)?)
    }

    /// Cómo armar `ch` como compuesto de base + marca, sobre la tinta de los dos.
    pub fn plan_accent(&self, sprite: &Sprite, ch: char) -> Result<AccentPlan, AccentSkip> {
        let (base, mark) = decomposition(ch).ok_or(AccentSkip::Unknown)?;
        let mut place = match mark {
            '\u{0326}' | '\u{0327}' => Place::Below,
            '\u{0328}' => Place::Ogonek,
            _ => Place::Above,
        };
        // La i y la j sin punto, si la fuente las tiene.
        let dotless = match base {
            'i' => Some('ı'),
            'j' => Some('ȷ'),
            _ => None,
        };
        let base = match dotless {
            Some(d) if place == Place::Above && self.inked(sprite, d).is_some() => d,
            _ => base,
        };
        let bb = self.inked(sprite, base).ok_or(AccentSkip::NoBase)?;
        // ď ľ ť Ľ: el apóstrofo a la derecha, si lo hay.
        let apostrophe = APOSTROPHE_CARON
            .contains(&ch)
            .then(|| ['\'', '\u{2019}'].into_iter())
            .into_iter()
            .flatten()
            .find_map(|a| Some((a, self.inked(sprite, a)?)));
        let (mark_ch, mb) = match apostrophe {
            Some(found) => {
                place = Place::Right;
                found
            }
            None => mark_sources(ch, mark)
                .into_iter()
                .find_map(|m| Some((m, self.inked(sprite, m)?)))
                .ok_or(AccentSkip::NoMark)?,
        };
        let baseline = self.metrics.baseline as i32;
        // Con descendente (ģ), la cedilla va arriba.
        if place == Place::Below && bb.y1 > baseline {
            place = Place::Above;
        }
        let height = sprite.height as i32;
        let centered = bb.x0 + (bb.width() - mb.width()).div_euclid(2) - mb.x0;
        let mut cramped = false;
        let offset = match place {
            Place::Above => {
                let mut dy = (bb.y0 - 1) - mb.y1;
                if mb.y0 + dy < 0 {
                    dy = -mb.y0;
                    cramped = true;
                }
                (centered, dy)
            }
            Place::Below | Place::Ogonek => {
                let dy = bb.y1 - mb.y0;
                cramped = mb.y1 + dy > height;
                let dx = match place {
                    Place::Ogonek => bb.x1 - mb.x1,
                    _ => centered,
                };
                (dx, dy)
            }
            Place::Right => (bb.x1 + 1 - mb.x0, bb.y0 - mb.y0),
        };
        Ok(AccentPlan {
            components: vec![
                GlyphComponent {
                    ch: base,
                    offset: (0, 0),
                },
                GlyphComponent {
                    ch: mark_ch,
                    offset,
                },
            ],
            cramped,
        })
    }
}

#[cfg(test)]
#[path = "font_accents_tests.rs"]
mod tests;
