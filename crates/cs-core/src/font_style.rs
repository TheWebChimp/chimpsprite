//! Los estilos de una fuente pixel: las fuentes del documento con el mismo nombre son una
//! familia (Regular, Bold, Italic…), cada una su propio artboard. Una variante generada guarda de
//! qué fuente y con qué receta salió, para volver a generarla (Rebuild).

use serde::{Deserialize, Serialize};

use crate::{ArtboardId, FontInfo, Rgba};

/// Los pesos con nombre (usWeightClass de OpenType).
pub const WEIGHTS: [(u16, &str); 9] = [
    (100, "Thin"),
    (200, "Extra Light"),
    (300, "Light"),
    (400, "Regular"),
    (500, "Medium"),
    (600, "Semi Bold"),
    (700, "Bold"),
    (800, "Extra Bold"),
    (900, "Black"),
];

/// El nombre de un peso (el más cercano de [`WEIGHTS`]).
pub fn weight_name(weight: u16) -> &'static str {
    WEIGHTS
        .iter()
        .min_by_key(|(w, _)| w.abs_diff(weight))
        .map_or("Regular", |(_, n)| n)
}

/// El estilo de una fuente de la familia.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontStyle {
    /// "Regular", "Bold", "Italic", "Bold Italic", "Outline"…
    pub name: String,
    /// De 100 a 900: Regular 400, Bold 700.
    pub weight: u16,
    pub italic: bool,
    /// La inclinación de la itálica: 1 columna cada `slant` filas (0: derecha).
    #[serde(default)]
    pub slant: u32,
    /// De qué fuente se generó y cómo (para Rebuild); `None` si se dibujó o se importó.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<StyleSource>,
}

impl Default for FontStyle {
    fn default() -> Self {
        FontStyle {
            name: "Regular".into(),
            weight: 400,
            italic: false,
            slant: 0,
            source: None,
        }
    }
}

/// De dónde salió una variante generada.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StyleSource {
    pub artboard: ArtboardId,
    pub recipe: VariantRecipe,
}

/// Cómo se genera una variante desde otra fuente.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VariantKind {
    /// Cada pixel también 1 a la derecha.
    Bold,
    /// Las filas de arriba de la baseline, corridas a la derecha.
    Italic,
    BoldItalic,
    /// El contorno de afuera (en 1 bit, la letra hueca).
    Outline,
    /// La sombra 1 abajo a la derecha (en 1 bit, hueca con el borde de abajo y la derecha doble).
    Shadow,
    /// Igual, para dibujarla a mano.
    Copy,
}

impl VariantKind {
    pub const ALL: [VariantKind; 6] = [
        VariantKind::Bold,
        VariantKind::Italic,
        VariantKind::BoldItalic,
        VariantKind::Outline,
        VariantKind::Shadow,
        VariantKind::Copy,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VariantKind::Bold => "Bold",
            VariantKind::Italic => "Italic",
            VariantKind::BoldItalic => "Bold Italic",
            VariantKind::Outline => "Outline",
            VariantKind::Shadow => "Shadow",
            VariantKind::Copy => "Copy (draw it yourself)",
        }
    }

    /// El nombre del estilo que sale (Copy: el de la fuente de origen con " Copy").
    pub fn style_name(self) -> &'static str {
        match self {
            VariantKind::Copy => "Copy",
            kind => kind.label(),
        }
    }

    pub fn weight(self) -> u16 {
        match self {
            VariantKind::Bold | VariantKind::BoldItalic => 700,
            _ => 400,
        }
    }

    pub fn italic(self) -> bool {
        matches!(self, VariantKind::Italic | VariantKind::BoldItalic)
    }

    pub fn bold(self) -> bool {
        matches!(self, VariantKind::Bold | VariantKind::BoldItalic)
    }

    /// Lo que crece el ancho de una letra (los anchos manuales crecen eso).
    pub fn extra_width(self) -> u32 {
        match self {
            VariantKind::Bold | VariantKind::BoldItalic | VariantKind::Shadow => 1,
            VariantKind::Outline => 2,
            VariantKind::Italic | VariantKind::Copy => 0,
        }
    }
}

/// La receta de una variante: el tipo, la inclinación (Italic), el color del contorno o la sombra
/// (en fuentes de color) y la matriz del contorno (Square: 8 vecinos; si no, 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VariantRecipe {
    pub kind: VariantKind,
    pub slant: u32,
    pub color: Rgba,
    pub square: bool,
}

impl VariantRecipe {
    /// La inclinación por defecto: 1 columna cada 3 filas (≈ 18°).
    pub const DEFAULT_SLANT: u32 = 3;

    pub fn new(kind: VariantKind) -> Self {
        VariantRecipe {
            kind,
            slant: Self::DEFAULT_SLANT,
            color: Rgba::new(0, 0, 0, 255),
            square: false,
        }
    }
}

impl FontStyle {
    /// Si es el Regular de siempre (no se escribe en el archivo).
    pub fn is_default(&self) -> bool {
        *self == FontStyle::default()
    }

    /// El estilo de una variante de `kind` (con su receta y su fuente).
    pub fn for_variant(source: ArtboardId, recipe: VariantRecipe, name: &str) -> Self {
        let kind = recipe.kind;
        FontStyle {
            name: name.to_owned(),
            weight: kind.weight(),
            italic: kind.italic(),
            slant: if kind.italic() { recipe.slant } else { 0 },
            source: Some(StyleSource {
                artboard: source,
                recipe,
            }),
        }
    }

    /// Los nombres "legacy" de los formatos (IDs 1 y 2 del `name` de TrueType, la regla RIBBI):
    /// Regular, Bold, Italic y Bold Italic van con la familia tal cual; otro estilo se lleva a la
    /// familia (sin "Italic") y la subfamilia queda Regular o Italic.
    pub fn legacy_names(&self, family: &str) -> (String, String) {
        let name = self.name.trim();
        let ribbi = ["Regular", "Bold", "Italic", "Bold Italic"]
            .into_iter()
            .find(|r| r.eq_ignore_ascii_case(name));
        if let Some(r) = ribbi {
            return (family.to_owned(), r.to_owned());
        }
        if name.is_empty() {
            return (family.to_owned(), "Regular".to_owned());
        }
        let base = name
            .strip_suffix("Italic")
            .map(str::trim_end)
            .filter(|b| self.italic && !b.is_empty())
            .unwrap_or(name);
        let sub = if self.italic { "Italic" } else { "Regular" };
        (format!("{family} {base}"), sub.to_owned())
    }

    /// Si es un estilo RIBBI (no hace falta la familia tipográfica en el `name`).
    pub fn is_ribbi(&self) -> bool {
        let name = self.name.trim();
        ["Regular", "Bold", "Italic", "Bold Italic"]
            .iter()
            .any(|r| r.eq_ignore_ascii_case(name))
    }

    /// Si es negrita para `fsSelection` y `macStyle`: su subfamilia legacy es Bold o Bold Italic.
    pub fn is_bold(&self) -> bool {
        self.legacy_names("").1.starts_with("Bold")
    }

    /// El estilo para nombres de archivo y el nombre PostScript: sin espacios ("BoldItalic").
    pub fn compact_name(&self) -> String {
        let compact: String = self.name.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.is_empty() {
            "Regular".into()
        } else {
            compact
        }
    }

    /// El ángulo de la itálica en grados (negativo: hacia la derecha), de `slant`.
    pub fn italic_angle(&self) -> f64 {
        if self.slant == 0 {
            return 0.0;
        }
        -(1.0 / self.slant as f64).atan().to_degrees()
    }
}

impl FontInfo {
    /// El nombre completo: la familia y el estilo ("Mine Bold"); el Regular, solo la familia.
    /// Es el nombre del artboard.
    pub fn full_name(&self) -> String {
        let style = self.style.name.trim();
        if style.is_empty() || style.eq_ignore_ascii_case("Regular") {
            self.name.clone()
        } else if self.name.trim().is_empty() {
            style.to_owned()
        } else {
            format!("{} {style}", self.name)
        }
    }

    /// La familia que sale de un nombre completo nuevo (renombrar el artboard): sin el estilo si
    /// termina en él.
    pub fn family_from_name(&self, full: &str) -> String {
        let full = full.trim();
        let style = self.style.name.trim();
        if style.is_empty() || style.eq_ignore_ascii_case("Regular") {
            return full.to_owned();
        }
        match full.strip_suffix(style).map(str::trim_end) {
            Some(family) if !family.is_empty() && full.len() > style.len() => family.to_owned(),
            _ => full.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FontMode;

    fn style(name: &str, weight: u16, italic: bool) -> FontStyle {
        FontStyle {
            name: name.into(),
            weight,
            italic,
            ..FontStyle::default()
        }
    }

    #[test]
    fn the_full_name_is_the_family_and_the_style_but_regular_is_the_family() {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        assert_eq!(font.full_name(), "Mine");
        assert!(font.style.is_default());
        font.style = style("Bold", 700, false);
        assert_eq!(font.full_name(), "Mine Bold");
        assert_eq!(font.family_from_name("Other Bold"), "Other");
        assert_eq!(font.family_from_name("Other"), "Other");
        assert_eq!(font.family_from_name("Bold"), "Bold", "nothing left");
        font.style.name = "Regular".into();
        assert_eq!(font.family_from_name("Mine Regular"), "Mine Regular");
    }

    #[test]
    fn legacy_names_follow_the_ribbi_rule() {
        let names = |s: FontStyle| s.legacy_names("Mine");
        assert_eq!(
            names(FontStyle::default()),
            ("Mine".into(), "Regular".into())
        );
        assert_eq!(
            names(style("bold italic", 700, true)),
            ("Mine".into(), "Bold Italic".into())
        );
        assert_eq!(
            names(style("Outline", 400, false)),
            ("Mine Outline".into(), "Regular".into())
        );
        assert_eq!(
            names(style("Light Italic", 300, true)),
            ("Mine Light".into(), "Italic".into())
        );
        assert!(style("Bold", 700, false).is_bold());
        assert!(!style("Black", 900, false).is_bold(), "not RIBBI");
        assert!(style("Italic", 400, true).is_ribbi());
        assert!(!style("Shadow", 400, false).is_ribbi());
        assert_eq!(style("Bold Italic", 700, true).compact_name(), "BoldItalic");
    }

    #[test]
    fn a_variant_style_comes_from_its_kind() {
        let recipe = VariantRecipe::new(VariantKind::BoldItalic);
        let s = FontStyle::for_variant(ArtboardId(3), recipe, "Bold Italic");
        assert_eq!((s.weight, s.italic, s.slant), (700, true, 3));
        assert_eq!(s.source.unwrap().artboard, ArtboardId(3));
        let s = FontStyle::for_variant(ArtboardId(3), VariantRecipe::new(VariantKind::Bold), "B");
        assert_eq!(s.slant, 0, "only italics slant");
        assert!((style("I", 400, true).italic_angle()).abs() < 1e-9);
        let italic = FontStyle {
            slant: 3,
            ..style("Italic", 400, true)
        };
        assert!((italic.italic_angle() + 18.43).abs() < 0.01);
        assert_eq!(weight_name(700), "Bold");
        assert_eq!(weight_name(450), "Regular");
    }

    #[test]
    fn styles_survive_json_and_old_fonts_are_regular() {
        let mut font = FontInfo::new("Mine", 8, FontMode::OneBit);
        let json = serde_json::to_string(&font).unwrap();
        assert!(!json.contains("style"));
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
        font.style = FontStyle::for_variant(
            ArtboardId(1),
            VariantRecipe::new(VariantKind::Outline),
            "Outline",
        );
        let json = serde_json::to_string(&font).unwrap();
        assert_eq!(serde_json::from_str::<FontInfo>(&json).unwrap(), font);
    }
}
