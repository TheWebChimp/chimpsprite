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

/// Modos de simetría para dibujar. Los ejes pasan por un centro en **medios píxeles**.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymmetryMode {
    #[default]
    None,
    /// Espejo izquierda ↔ derecha (eje vertical).
    Horizontal,
    /// Espejo arriba ↔ abajo (eje horizontal).
    Vertical,
    Both,
    /// Diagonal ╲.
    DiagonalDown,
    /// Diagonal ╱.
    DiagonalUp,
    /// Las 8 simetrías del cuadrado.
    All,
}

/// Simetría de un artboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symmetry {
    pub mode: SymmetryMode,
    /// Centro × 2 (medios píxeles); `None` = centro del artboard.
    pub center2: Option<(i32, i32)>,
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
    #[serde(default)]
    pub symmetry: Symmetry,
}

impl Artboard {
    /// Centro de simetría efectivo en medios píxeles.
    pub fn symmetry_center2(&self) -> (i32, i32) {
        self.symmetry
            .center2
            .unwrap_or((self.sprite.width as i32, self.sprite.height as i32))
    }
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
                symmetry: Default::default(),
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

    /// Artboards que `id` instancia directamente (en cualquier key de cualquier capa).
    pub fn instance_sources(&self, id: ArtboardId) -> Vec<ArtboardId> {
        let Some(artboard) = self.artboard(id) else {
            return Vec::new();
        };
        let mut out: Vec<ArtboardId> = artboard
            .sprite
            .layers
            .iter()
            .filter_map(|l| l.instance.as_ref())
            .flat_map(|t| t.keys.iter().filter_map(|k| k.source))
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// ¿Instanciar `source` dentro de `host` haría que `host` se contenga a sí mismo?
    pub fn would_cycle(&self, host: ArtboardId, source: ArtboardId) -> bool {
        let mut stack = vec![source];
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = stack.pop() {
            if id == host {
                return true;
            }
            if seen.insert(id) {
                stack.extend(self.instance_sources(id));
            }
        }
        false
    }

    /// Ninguna instancia de `id` lo contiene (directa o indirectamente).
    pub fn cycle_free(&self, id: ArtboardId) -> bool {
        self.instance_sources(id)
            .into_iter()
            .all(|s| !self.would_cycle(id, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorMode, InstanceKey, InstanceTrack, Layer, LayerId, LayerKind};

    fn doc() -> Document {
        let mut d = Document::with_sprite("a", Sprite::new(10, 10, ColorMode::Rgba));
        d.artboards.push(Artboard {
            guides: Vec::new(),
            symmetry: Default::default(),
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

    fn instancing(ids: &[(u32, &[u32])]) -> Document {
        let artboards = ids
            .iter()
            .map(|&(id, sources)| {
                let mut sprite = Sprite::new(4, 4, ColorMode::Rgba);
                for (i, &s) in sources.iter().enumerate() {
                    let mut layer = Layer::new_image(LayerId(10 + i as u32), "i");
                    layer.kind = LayerKind::Instance;
                    layer.instance = Some(InstanceTrack {
                        keys: vec![InstanceKey::new(0, ArtboardId(s), (0, 0))],
                    });
                    sprite.layers.push(layer);
                }
                Artboard {
                    id: ArtboardId(id),
                    name: format!("a{id}"),
                    position: (0, 0),
                    sprite,
                    guides: Vec::new(),
                    symmetry: Default::default(),
                }
            })
            .collect();
        Document { artboards }
    }

    #[test]
    fn instance_cycles_are_detected() {
        // 1 → 2 → 3; 4 suelto.
        let d = instancing(&[(1, &[2, 2]), (2, &[3]), (3, &[]), (4, &[])]);
        assert_eq!(d.instance_sources(ArtboardId(1)), vec![ArtboardId(2)]);
        assert!(d.would_cycle(ArtboardId(1), ArtboardId(1)), "itself");
        assert!(d.would_cycle(ArtboardId(2), ArtboardId(1)), "direct");
        assert!(d.would_cycle(ArtboardId(3), ArtboardId(1)), "indirect");
        assert!(!d.would_cycle(ArtboardId(1), ArtboardId(3)));
        assert!(!d.would_cycle(ArtboardId(4), ArtboardId(1)));
        assert!(d.cycle_free(ArtboardId(1)));
        let looped = instancing(&[(1, &[2]), (2, &[3]), (3, &[1])]);
        assert!(!looped.cycle_free(ArtboardId(3)));
    }
}
