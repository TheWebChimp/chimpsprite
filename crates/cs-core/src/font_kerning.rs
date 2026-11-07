//! El kerning de una fuente pixel: pares de caracteres y clases (como las *kerning classes* de
//! FontForge y los grupos `kern1`/`kern2` de UFO). El valor va en pixeles y se suma al avance del
//! primer carácter del par: negativo acerca, positivo separa.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

/// Un lado de un par: un carácter o una clase.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KernSide {
    Char(char),
    Class(String),
}

impl KernSide {
    /// Lee un lado como se escribe en el panel: `@nombre` es una clase; si no, el primer carácter
    /// que no es espacio. `None` si no hay nada.
    pub fn parse(text: &str) -> Option<KernSide> {
        let text = text.trim();
        if let Some(name) = text.strip_prefix('@') {
            let name = name.trim();
            return (!name.is_empty()).then(|| KernSide::Class(name.to_owned()));
        }
        text.chars()
            .find(|c| !c.is_whitespace())
            .map(KernSide::Char)
    }

    /// Como se escribe: el carácter o `@nombre`.
    pub fn label(&self) -> String {
        match self {
            KernSide::Char(c) => c.to_string(),
            KernSide::Class(name) => format!("@{name}"),
        }
    }
}

/// De qué lado del par cuenta una clase: *First* (el carácter de la izquierda) o *Second* (el de
/// la derecha).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ClassSide {
    First,
    Second,
}

impl ClassSide {
    pub const ALL: [ClassSide; 2] = [ClassSide::First, ClassSide::Second];

    pub fn label(self) -> &'static str {
        match self {
            ClassSide::First => "First",
            ClassSide::Second => "Second",
        }
    }
}

/// Una clase de kerning: caracteres que se kernean igual de un lado.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KernClass {
    pub name: String,
    pub side: ClassSide,
    pub chars: Vec<char>,
}

/// Un par de kerning y su valor en pixeles.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KernPair {
    pub first: KernSide,
    pub second: KernSide,
    pub value: i32,
}

/// Qué tan específico es un par (de más a menos): así se decide cuál vale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KernSource {
    /// `a b`.
    Exact,
    /// `a @clase`.
    CharClass,
    /// `@clase b`.
    ClassChar,
    /// `@clase @clase`.
    Classes,
}

/// El kerning de una fuente: los pares (en orden por sus lados, sin repetir) y las clases.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontKerning {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pairs: Vec<KernPair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<KernClass>,
}

impl FontKerning {
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty() && self.classes.is_empty()
    }

    /// La clase de `side` en la que cuenta `ch`: la primera que lo tiene.
    pub fn class_of(&self, ch: char, side: ClassSide) -> Option<&str> {
        self.classes
            .iter()
            .find(|c| c.side == side && c.chars.contains(&ch))
            .map(|c| c.name.as_str())
    }

    /// El índice del par de `first` y `second`, tal cual.
    pub fn index_of(&self, first: &KernSide, second: &KernSide) -> Option<usize> {
        self.pairs
            .binary_search_by(|p| (&p.first, &p.second).cmp(&(first, second)))
            .ok()
    }

    /// El par que vale para `a` seguida de `b` (como UFO: el exacto; si no, `a @clase`; si no,
    /// `@clase b`; si no, `@clase @clase`) y qué tan específico es.
    pub fn source(&self, a: char, b: char) -> Option<(usize, KernSource)> {
        let (ca, cb) = (KernSide::Char(a), KernSide::Char(b));
        let first = self
            .class_of(a, ClassSide::First)
            .map(|n| KernSide::Class(n.to_owned()));
        let second = self
            .class_of(b, ClassSide::Second)
            .map(|n| KernSide::Class(n.to_owned()));
        let tries = [
            (Some(&ca), Some(&cb), KernSource::Exact),
            (Some(&ca), second.as_ref(), KernSource::CharClass),
            (first.as_ref(), Some(&cb), KernSource::ClassChar),
            (first.as_ref(), second.as_ref(), KernSource::Classes),
        ];
        tries.into_iter().find_map(|(f, s, source)| {
            let i = self.index_of(f?, s?)?;
            Some((i, source))
        })
    }

    /// Lo que se suma entre `a` y `b` (0 si ningún par vale).
    pub fn value(&self, a: char, b: char) -> i32 {
        self.source(a, b).map_or(0, |(i, _)| self.pairs[i].value)
    }

    /// Pone el par (`value` en el que ya estaba o uno nuevo en su lugar).
    pub fn set(&mut self, first: KernSide, second: KernSide, value: i32) {
        let found = self
            .pairs
            .binary_search_by(|p| (&p.first, &p.second).cmp(&(&first, &second)));
        match found {
            Ok(i) => self.pairs[i].value = value,
            Err(i) => self.pairs.insert(
                i,
                KernPair {
                    first,
                    second,
                    value,
                },
            ),
        }
    }

    /// Quita el par; devuelve si estaba.
    pub fn remove(&mut self, first: &KernSide, second: &KernSide) -> bool {
        match self.index_of(first, second) {
            Some(i) => {
                self.pairs.remove(i);
                true
            }
            None => false,
        }
    }

    /// Deja los pares en orden y sin repetir (gana el último), por si vienen de un archivo.
    pub fn normalize(&mut self) {
        let pairs = std::mem::take(&mut self.pairs);
        for p in pairs {
            self.set(p.first, p.second, p.value);
        }
    }

    /// Los caracteres de un lado de un par (el carácter o los de la clase de ese lado).
    fn members(&self, side: &KernSide, class_side: ClassSide) -> Vec<char> {
        match side {
            KernSide::Char(c) => vec![*c],
            KernSide::Class(name) => self
                .classes
                .iter()
                .filter(|c| c.side == class_side && c.name == *name)
                .flat_map(|c| c.chars.iter().copied())
                .collect(),
        }
    }

    /// Todos los pares de caracteres con valor distinto de 0, solo entre los que `has` dice que
    /// tiene la fuente: lo que usan el render y los exports.
    pub fn flatten(&self, has: impl Fn(char) -> bool) -> BTreeMap<(char, char), i32> {
        let mut candidates = BTreeSet::new();
        for p in &self.pairs {
            let firsts = self.members(&p.first, ClassSide::First);
            let seconds = self.members(&p.second, ClassSide::Second);
            for &a in firsts.iter().filter(|&&a| has(a)) {
                for &b in seconds.iter().filter(|&&b| has(b)) {
                    candidates.insert((a, b));
                }
            }
        }
        candidates
            .into_iter()
            .filter_map(|(a, b)| match self.value(a, b) {
                0 => None,
                v => Some(((a, b), v)),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(name: &str, side: ClassSide, chars: &str) -> KernClass {
        KernClass {
            name: name.into(),
            side,
            chars: chars.chars().collect(),
        }
    }

    fn ch(c: char) -> KernSide {
        KernSide::Char(c)
    }

    fn cl(name: &str) -> KernSide {
        KernSide::Class(name.into())
    }

    #[test]
    fn pairs_stay_sorted_and_unique() {
        let mut k = FontKerning::default();
        k.set(ch('V'), ch('A'), -1);
        k.set(ch('A'), ch('V'), -2);
        k.set(cl("o"), ch('A'), 1);
        k.set(ch('A'), ch('V'), -3);
        let firsts: Vec<_> = k.pairs.iter().map(|p| p.first.label()).collect();
        assert_eq!(firsts, vec!["A", "V", "@o"]);
        assert_eq!(k.value('A', 'V'), -3);
        assert!(k.remove(&ch('V'), &ch('A')));
        assert!(!k.remove(&ch('V'), &ch('A')));
        assert_eq!(k.value('V', 'A'), 0);
        let mut messy = FontKerning {
            pairs: vec![
                KernPair {
                    first: ch('b'),
                    second: ch('a'),
                    value: 1,
                },
                KernPair {
                    first: ch('a'),
                    second: ch('b'),
                    value: 2,
                },
                KernPair {
                    first: ch('b'),
                    second: ch('a'),
                    value: 5,
                },
            ],
            classes: Vec::new(),
        };
        messy.normalize();
        assert_eq!(messy.pairs.len(), 2);
        assert_eq!(messy.value('b', 'a'), 5, "the last one wins");
    }

    #[test]
    fn the_most_specific_pair_wins_like_ufo() {
        let mut k = FontKerning {
            pairs: Vec::new(),
            classes: vec![
                class("round", ClassSide::First, "OQ"),
                class("diag", ClassSide::Second, "VW"),
            ],
        };
        k.set(cl("round"), cl("diag"), -1);
        assert_eq!(k.value('O', 'W'), -1);
        assert_eq!(k.source('O', 'W').unwrap().1, KernSource::Classes);
        k.set(cl("round"), ch('V'), -2);
        assert_eq!(k.value('O', 'V'), -2);
        assert_eq!(k.value('Q', 'W'), -1);
        k.set(ch('O'), cl("diag"), -3);
        assert_eq!(k.value('O', 'V'), -3, "char·class beats class·char");
        assert_eq!(k.source('O', 'V').unwrap().1, KernSource::CharClass);
        k.set(ch('O'), ch('V'), 0);
        assert_eq!(k.value('O', 'V'), 0, "an exception of 0");
        assert_eq!(k.source('O', 'V').unwrap().1, KernSource::Exact);
        assert_eq!(k.value('V', 'O'), 0, "the classes have sides");
        assert_eq!(k.class_of('V', ClassSide::Second), Some("diag"));
        assert_eq!(k.class_of('V', ClassSide::First), None);
    }

    #[test]
    fn flattening_expands_classes_only_for_chars_of_the_font() {
        let mut k = FontKerning {
            pairs: Vec::new(),
            classes: vec![
                class("round", ClassSide::First, "OQ"),
                class("diag", ClassSide::Second, "VW"),
            ],
        };
        k.set(cl("round"), cl("diag"), -1);
        k.set(ch('O'), ch('V'), 0);
        k.set(ch('A'), ch('T'), 2);
        let flat = k.flatten(|c| c != 'Q');
        let expected: BTreeMap<(char, char), i32> =
            [(('A', 'T'), 2), (('O', 'W'), -1)].into_iter().collect();
        assert_eq!(flat, expected);
    }

    #[test]
    fn sides_are_read_and_written_with_the_at_sign() {
        assert_eq!(KernSide::parse(" A "), Some(ch('A')));
        assert_eq!(KernSide::parse("@round"), Some(cl("round")));
        assert_eq!(KernSide::parse("@ "), None);
        assert_eq!(KernSide::parse("   "), None);
        assert_eq!(KernSide::parse("@"), None);
        assert_eq!(cl("round").label(), "@round");
        assert_eq!(ch('é').label(), "é");
    }

    #[test]
    fn kerning_survives_json_and_empty_kerning_is_nothing() {
        let mut k = FontKerning {
            pairs: Vec::new(),
            classes: vec![class("round", ClassSide::First, "OQ")],
        };
        k.set(cl("round"), ch('A'), -1);
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(serde_json::from_str::<FontKerning>(&json).unwrap(), k);
        assert_eq!(
            serde_json::to_string(&FontKerning::default()).unwrap(),
            "{}"
        );
        assert!(FontKerning::default().is_empty());
    }
}
