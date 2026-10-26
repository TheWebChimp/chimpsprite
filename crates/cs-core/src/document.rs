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

/// Ejes de simetría para dibujar; se prenden y apagan por separado. Se aplica el grupo de
/// simetrías que generan los ejes prendidos. Los ejes pasan por un centro en **medios píxeles**.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default)]
pub struct SymmetryAxes {
    /// Eje vertical │: espejo izquierda ↔ derecha.
    pub vertical: bool,
    /// Eje horizontal ─: espejo arriba ↔ abajo.
    pub horizontal: bool,
    /// Diagonal ╲.
    pub diagonal_down: bool,
    /// Diagonal ╱.
    pub diagonal_up: bool,
}

impl SymmetryAxes {
    /// Todos apagados: solo el trazo original.
    pub const NONE: Self = Self {
        vertical: false,
        horizontal: false,
        diagonal_down: false,
        diagonal_up: false,
    };

    /// `true` si hay al menos un eje prendido.
    pub fn any(self) -> bool {
        self.vertical || self.horizontal || self.diagonal_down || self.diagonal_up
    }
}

/// El modo único de antes (`mode`). Solo se lee, para abrir documentos y autosaves viejos.
#[derive(Deserialize)]
enum LegacyMode {
    None,
    /// Espejo izquierda ↔ derecha: el eje vertical.
    Horizontal,
    /// Espejo arriba ↔ abajo: el eje horizontal.
    Vertical,
    Both,
    DiagonalDown,
    DiagonalUp,
    All,
}

impl From<LegacyMode> for SymmetryAxes {
    fn from(mode: LegacyMode) -> Self {
        let mut axes = SymmetryAxes::NONE;
        match mode {
            LegacyMode::None => {}
            LegacyMode::Horizontal => axes.vertical = true,
            LegacyMode::Vertical => axes.horizontal = true,
            LegacyMode::Both => {
                axes.vertical = true;
                axes.horizontal = true;
            }
            LegacyMode::DiagonalDown => axes.diagonal_down = true,
            LegacyMode::DiagonalUp => axes.diagonal_up = true,
            LegacyMode::All => {
                axes = SymmetryAxes {
                    vertical: true,
                    horizontal: true,
                    diagonal_down: true,
                    diagonal_up: true,
                }
            }
        }
        axes
    }
}

/// Simetría de un artboard. Se escribe con `axes`; al leer también acepta el `mode` de antes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "SymmetryRepr")]
pub struct Symmetry {
    pub axes: SymmetryAxes,
    /// Centro × 2 (medios píxeles); `None` = centro del artboard.
    pub center2: Option<(i32, i32)>,
}

/// Lo que se lee de una simetría: el formato nuevo (`axes`) o el viejo (`mode`). Si vienen los
/// dos, manda `axes`.
#[derive(Deserialize)]
struct SymmetryRepr {
    #[serde(default)]
    axes: Option<SymmetryAxes>,
    #[serde(default)]
    mode: Option<LegacyMode>,
    #[serde(default)]
    center2: Option<(i32, i32)>,
}

impl From<SymmetryRepr> for Symmetry {
    fn from(repr: SymmetryRepr) -> Self {
        Symmetry {
            axes: repr
                .axes
                .or(repr.mode.map(SymmetryAxes::from))
                .unwrap_or_default(),
            center2: repr.center2,
        }
    }
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

/// Una nota (post-it) sobre el lienzo, entre los artboards. Su esquina está en coordenadas de
/// mundo (se mueve con la vista); su tamaño es de interfaz (no cambia con el zoom, para que el
/// texto siempre se lea).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: u32,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub text: String,
}

/// Documento: varios artboards; el orden es el z-order (el último queda arriba). Las notas van
/// encima de todo.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub artboards: Vec<Artboard>,
    #[serde(default)]
    pub notes: Vec<Note>,
}

impl Document {
    pub fn with_sprite(name: impl Into<String>, sprite: Sprite) -> Self {
        Self {
            notes: Vec::new(),
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

    /// Id nuevo: mayor que el de todo artboard existente y que toda fuente de instancia, para que
    /// las keys que apuntan a un artboard borrado no pasen a mostrar uno nuevo.
    pub fn next_artboard_id(&self) -> ArtboardId {
        let sources = self
            .artboards
            .iter()
            .flat_map(|a| a.sprite.layers.iter())
            .filter_map(|l| l.instance.as_ref())
            .flat_map(|t| t.keys.iter().filter_map(|k| k.source));
        let max = self
            .artboards
            .iter()
            .map(|a| a.id)
            .chain(sources)
            .map(|id| id.0)
            .max()
            .unwrap_or(0);
        ArtboardId(max + 1)
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
        assert_eq!(Document::default().bounds(), None);
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
        Document {
            artboards,
            notes: Vec::new(),
        }
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

    #[test]
    fn next_artboard_id_skips_ids_still_referenced_by_instances() {
        // El 1 instancia al 5, que ya no existe: el próximo id no puede ser 5.
        let d = instancing(&[(1, &[5]), (2, &[])]);
        assert_eq!(d.next_artboard_id(), ArtboardId(6));
        let d = instancing(&[(1, &[]), (3, &[1])]);
        assert_eq!(d.next_artboard_id(), ArtboardId(4));
    }

    #[test]
    fn the_old_symmetry_mode_reads_as_axes() {
        let axes = |v, h, d, u| SymmetryAxes {
            vertical: v,
            horizontal: h,
            diagonal_down: d,
            diagonal_up: u,
        };
        let cases = [
            ("None", axes(false, false, false, false)),
            ("Horizontal", axes(true, false, false, false)),
            ("Vertical", axes(false, true, false, false)),
            ("Both", axes(true, true, false, false)),
            ("DiagonalDown", axes(false, false, true, false)),
            ("DiagonalUp", axes(false, false, false, true)),
            ("All", axes(true, true, true, true)),
        ];
        for (mode, expected) in cases {
            // Así escribía la simetría la versión anterior.
            let json = format!(r#"{{"mode":"{mode}","center2":[5,3]}}"#);
            let s: Symmetry = serde_json::from_str(&json).unwrap();
            assert_eq!(s.axes, expected, "{mode}");
            assert_eq!(s.center2, Some((5, 3)), "{mode}");
        }
        let old: Symmetry = serde_json::from_str(r#"{"mode":"Both","center2":null}"#).unwrap();
        assert_eq!(old.center2, None);
    }

    #[test]
    fn the_symmetry_is_written_with_axes_and_reads_back() {
        let s = Symmetry {
            axes: SymmetryAxes {
                vertical: true,
                diagonal_up: true,
                ..SymmetryAxes::NONE
            },
            center2: Some((4, 6)),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(
            json,
            r#"{"axes":{"vertical":true,"horizontal":false,"diagonal_down":false,"diagonal_up":true},"center2":[4,6]}"#
        );
        assert_eq!(serde_json::from_str::<Symmetry>(&json).unwrap(), s);
        // Si vienen los dos, manda `axes`; sin nada, todo apagado.
        let both: Symmetry =
            serde_json::from_str(r#"{"axes":{"horizontal":true},"mode":"All"}"#).unwrap();
        assert_eq!(
            both.axes,
            SymmetryAxes {
                horizontal: true,
                ..SymmetryAxes::NONE
            }
        );
        assert_eq!(
            serde_json::from_str::<Symmetry>("{}").unwrap(),
            Symmetry::default()
        );
    }

    #[test]
    fn an_old_artboard_with_mode_still_loads() {
        let mut json = serde_json::to_value(&doc().artboards[1]).unwrap();
        json["symmetry"] = serde_json::json!({"mode": "DiagonalUp", "center2": null});
        let artboard: Artboard = serde_json::from_value(json).unwrap();
        assert!(artboard.symmetry.axes.diagonal_up);
        assert!(!artboard.symmetry.axes.vertical);
    }
}
