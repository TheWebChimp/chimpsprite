use serde::{Deserialize, Serialize};

use crate::Sprite;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtboardId(pub u32);

/// Orientación de una guía.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Axis {
    /// Línea horizontal en `y = position`.
    Horizontal,
    /// Línea vertical en `x = position`.
    Vertical,
}

/// Guía en coordenadas locales del artboard, siempre en un borde de píxel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Guide {
    pub axis: Axis,
    pub position: i32,
}

/// Un sprite independiente colocado en el lienzo infinito del documento.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artboard {
    pub id: ArtboardId,
    pub name: String,
    /// Esquina superior izquierda en coordenadas de mundo.
    pub position: (i32, i32),
    pub sprite: Sprite,
    #[serde(default)]
    pub guides: Vec<Guide>,
}

impl Artboard {
    /// Rectángulo en el mundo: mínimo inclusivo, máximo exclusivo.
    pub fn rect(&self) -> ((i32, i32), (i32, i32)) {
        let (x, y) = self.position;
        (
            (x, y),
            (x + self.sprite.width as i32, y + self.sprite.height as i32),
        )
    }

    pub fn contains(&self, p: (f32, f32)) -> bool {
        let ((x0, y0), (x1, y1)) = self.rect();
        p.0 >= x0 as f32 && p.1 >= y0 as f32 && p.0 < x1 as f32 && p.1 < y1 as f32
    }
}

/// Documento: varios artboards; el orden es el z-order (el último queda arriba).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub artboards: Vec<Artboard>,
}

impl Document {
    pub fn with_sprite(name: impl Into<String>, sprite: Sprite) -> Self {
        Self {
            artboards: vec![Artboard {
                guides: Vec::new(),
                id: ArtboardId(1),
                name: name.into(),
                position: (0, 0),
                sprite,
            }],
        }
    }

    pub fn index(&self, id: ArtboardId) -> Option<usize> {
        self.artboards.iter().position(|a| a.id == id)
    }

    pub fn artboard(&self, id: ArtboardId) -> Option<&Artboard> {
        self.artboards.iter().find(|a| a.id == id)
    }

    pub fn artboard_mut(&mut self, id: ArtboardId) -> Option<&mut Artboard> {
        self.artboards.iter_mut().find(|a| a.id == id)
    }

    pub fn next_artboard_id(&self) -> ArtboardId {
        ArtboardId(self.artboards.iter().map(|a| a.id.0).max().unwrap_or(0) + 1)
    }

    /// El artboard de más arriba bajo el punto de mundo `p`.
    pub fn artboard_at(&self, p: (f32, f32)) -> Option<ArtboardId> {
        self.artboards
            .iter()
            .rev()
            .find(|a| a.contains(p))
            .map(|a| a.id)
    }

    /// Rectángulo que contiene a todos los artboards (máximo exclusivo).
    pub fn bounds(&self) -> Option<((i32, i32), (i32, i32))> {
        self.artboards
            .iter()
            .map(Artboard::rect)
            .reduce(|(a0, a1), (b0, b1)| {
                (
                    (a0.0.min(b0.0), a0.1.min(b0.1)),
                    (a1.0.max(b1.0), a1.1.max(b1.1)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorMode;

    fn doc() -> Document {
        let mut d = Document::with_sprite("a", Sprite::new(10, 10, ColorMode::Rgba));
        d.artboards.push(Artboard {
            guides: Vec::new(),
            id: d.next_artboard_id(),
            name: "b".into(),
            position: (5, 5),
            sprite: Sprite::new(10, 4, ColorMode::Rgba),
        });
        d
    }

    #[test]
    fn artboard_at_prefers_topmost() {
        let d = doc();
        assert_eq!(d.artboard_at((6.0, 6.0)), Some(ArtboardId(2)));
        assert_eq!(d.artboard_at((1.0, 1.0)), Some(ArtboardId(1)));
        assert_eq!(d.artboard_at((14.5, 8.9)), Some(ArtboardId(2)));
        assert_eq!(d.artboard_at((14.5, 9.0)), None);
        assert_eq!(d.artboard_at((-0.5, 0.0)), None);
    }

    #[test]
    fn bounds_union() {
        assert_eq!(doc().bounds(), Some(((0, 0), (15, 10))));
        assert_eq!(Document { artboards: vec![] }.bounds(), None);
    }
}
