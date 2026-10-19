use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use std::ops::Range;

use crate::{ColorMode, Image, Palette, PixelValue, Tag};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct LayerId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ImageId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Addition,
    Subtract,
    Difference,
    /// Modo leído de un archivo que no soportamos; se conserva y se renderiza como Normal.
    Other(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Image,
    Group,
    /// Muestra otro artboard (un símbolo) con su propia línea de tiempo; no tiene cels.
    Instance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub kind: LayerKind,
    pub parent: Option<LayerId>,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend: BlendMode,
    /// Máscara de capa: oculta partes de la capa (o del grupo) sin borrarlas.
    #[serde(default)]
    pub mask: Option<LayerMask>,
    /// Máscara de recorte: la capa solo se ve donde hay píxeles en la capa base (la primera de
    /// abajo, del mismo padre, que no recorta).
    #[serde(default)]
    pub clip: bool,
    /// Keys de la instancia; solo en capas `LayerKind::Instance`.
    #[serde(default)]
    pub instance: Option<crate::InstanceTrack>,
}

impl Layer {
    pub fn new_image(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            kind: LayerKind::Image,
            parent: None,
            visible: true,
            locked: false,
            opacity: 255,
            blend: BlendMode::Normal,
            mask: None,
            clip: false,
            instance: None,
        }
    }
}

/// Máscara de capa en escala de grises: 255 muestra, 0 oculta. Cubre un rectángulo del lienzo;
/// fuera de él vale `default`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerMask {
    pub position: (i32, i32),
    pub width: u32,
    pub height: u32,
    pub data: Arc<Vec<u8>>,
    pub default: u8,
    /// Desactivada, la máscara no afecta (pero se conserva).
    pub enabled: bool,
}

impl LayerMask {
    /// Máscara del tamaño del lienzo con un solo valor.
    pub fn filled(width: u32, height: u32, value: u8) -> Self {
        Self {
            position: (0, 0),
            width,
            height,
            data: Arc::new(vec![value; (width * height) as usize]),
            default: value,
            enabled: true,
        }
    }

    /// Valor guardado en `(x, y)` del lienzo (sin considerar si está activa).
    pub fn get(&self, x: i32, y: i32) -> u8 {
        let (lx, ly) = (x - self.position.0, y - self.position.1);
        if lx < 0 || ly < 0 || lx >= self.width as i32 || ly >= self.height as i32 {
            return self.default;
        }
        self.data[(ly as u32 * self.width + lx as u32) as usize]
    }

    /// Lo que la máscara deja ver en `(x, y)`: 255 si está desactivada.
    pub fn value(&self, x: i32, y: i32) -> u8 {
        if self.enabled { self.get(x, y) } else { 255 }
    }

    /// Copia que también cubre el rectángulo `(x0, y0)..(x1, y1)` (máximo exclusivo).
    pub fn grown_to(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        let (ox0, oy0) = self.position;
        let (ox1, oy1) = (ox0 + self.width as i32, oy0 + self.height as i32);
        let (nx0, ny0, nx1, ny1) = (x0.min(ox0), y0.min(oy0), x1.max(ox1), y1.max(oy1));
        if (nx0, ny0, nx1, ny1) == (ox0, oy0, ox1, oy1) {
            return self.clone();
        }
        let (w, h) = ((nx1 - nx0) as u32, (ny1 - ny0) as u32);
        let mut data = vec![self.default; (w * h) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                let (gx, gy) = ((x as i32 + ox0 - nx0) as u32, (y as i32 + oy0 - ny0) as u32);
                data[(gy * w + gx) as usize] = self.data[(y * self.width + x) as usize];
            }
        }
        Self {
            position: (nx0, ny0),
            width: w,
            height: h,
            data: Arc::new(data),
            ..self.clone()
        }
    }
}

/// Cuadrícula de trabajo del sprite (además del grid de píxeles).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridSettings {
    pub size: (u32, u32),
    pub offset: (i32, i32),
}

impl Default for GridSettings {
    fn default() -> Self {
        Self {
            size: (16, 16),
            offset: (0, 0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub duration_ms: u16,
}

/// Contenido de una capa en un frame. Dos cels con el mismo `image` están vinculados.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cel {
    pub position: (i32, i32),
    pub opacity: u8,
    pub image: ImageId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub color_mode: ColorMode,
    pub palette: Palette,
    /// Orden de abajo hacia arriba; los grupos se expresan con `Layer::parent`.
    pub layers: Vec<Layer>,
    pub frames: Vec<Frame>,
    #[serde(with = "as_entries")]
    pub cels: BTreeMap<(LayerId, usize), Cel>,
    #[serde(with = "as_entries")]
    pub images: BTreeMap<ImageId, Arc<Image>>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// Datos de archivo que no interpretamos (p. ej. slices de `.aseprite`) y se reescriben tal cual.
    #[serde(default)]
    pub extra_chunks: Vec<RawChunk>,
    #[serde(default)]
    pub grid: GridSettings,
    /// Rectángulos con nombre por frame (hitboxes, pivotes, 9-slice), como los slices de Aseprite.
    #[serde(default)]
    pub slices: Vec<Slice>,
}

/// Rectángulo con nombre que puede cambiar por frame: hitboxes, puntos de anclaje, 9-slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slice {
    pub name: String,
    pub color: crate::Rgba,
    /// Texto libre (p. ej. "hurtbox", datos para el motor).
    #[serde(default)]
    pub text: String,
    /// Ordenadas por frame; cada una vale desde su frame hasta la siguiente.
    pub keys: Vec<SliceKey>,
}

/// Estado de un slice desde un frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceKey {
    pub frame: usize,
    /// `(x, y, ancho, alto)`; ancho 0 = el slice no existe desde este frame.
    pub bounds: (i32, i32, u32, u32),
    /// Centro de 9-slice, relativo a `bounds`.
    #[serde(default)]
    pub center: Option<(i32, i32, u32, u32)>,
    /// Pivote, relativo a la esquina de `bounds`.
    #[serde(default)]
    pub pivot: Option<(i32, i32)>,
}

impl Slice {
    /// Clave vigente en `frame` (la última que empieza en o antes de él), si el slice existe ahí.
    pub fn key_at(&self, frame: usize) -> Option<&SliceKey> {
        self.keys
            .iter()
            .rev()
            .find(|k| k.frame <= frame)
            .filter(|k| k.bounds.2 > 0 && k.bounds.3 > 0)
    }

    /// Pone (o reemplaza) la clave de su frame, manteniendo el orden.
    pub fn set_key(&mut self, key: SliceKey) {
        match self.keys.binary_search_by_key(&key.frame, |k| k.frame) {
            Ok(i) => self.keys[i] = key,
            Err(i) => self.keys.insert(i, key),
        }
    }
}

/// Chunk de archivo preservado sin interpretar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawChunk {
    pub kind: u16,
    pub data: Vec<u8>,
}

/// Serializa un mapa como lista de pares, para que funcione en formatos (como JSON)
/// que solo aceptan claves de texto.
mod as_entries {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<K: Serialize, V: Serialize, S: Serializer>(
        map: &BTreeMap<K, V>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_seq(map.iter())
    }

    pub fn deserialize<'de, K, V, D>(d: D) -> Result<BTreeMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        Ok(Vec::<(K, V)>::deserialize(d)?.into_iter().collect())
    }
}

impl Sprite {
    pub fn new(width: u32, height: u32, color_mode: ColorMode) -> Self {
        Self {
            width,
            height,
            color_mode,
            palette: Palette::db32(),
            layers: vec![Layer::new_image(LayerId(1), "Layer 1")],
            frames: vec![Frame { duration_ms: 100 }],
            cels: BTreeMap::new(),
            images: BTreeMap::new(),
            tags: Vec::new(),
            extra_chunks: Vec::new(),
            grid: GridSettings::default(),
            slices: Vec::new(),
        }
    }

    pub fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: LayerId) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn next_layer_id(&self) -> LayerId {
        LayerId(self.layers.iter().map(|l| l.id.0).max().unwrap_or(0) + 1)
    }

    pub fn next_image_id(&self) -> ImageId {
        ImageId(self.images.keys().map(|k| k.0).max().map_or(1, |m| m + 1))
    }

    pub fn cel(&self, layer: LayerId, frame: usize) -> Option<&Cel> {
        self.cels.get(&(layer, frame))
    }

    pub fn image(&self, id: ImageId) -> Option<&Arc<Image>> {
        self.images.get(&id)
    }

    /// Valor del píxel de una capa en coordenadas del lienzo; transparente si no hay cel o cae fuera.
    pub fn pixel(&self, layer: LayerId, frame: usize, x: i32, y: i32) -> PixelValue {
        self.cel(layer, frame)
            .and_then(|cel| {
                let (lx, ly) = (x - cel.position.0, y - cel.position.1);
                if lx < 0 || ly < 0 {
                    return None;
                }
                self.image(cel.image)?.get(lx as u32, ly as u32)
            })
            .unwrap_or_else(|| self.color_mode.transparent())
    }

    /// `true` si `layer` está dentro (a cualquier profundidad) del grupo `ancestor`.
    pub fn is_descendant(&self, layer: LayerId, ancestor: LayerId) -> bool {
        let mut current = self.layer(layer).and_then(|l| l.parent);
        for _ in 0..self.layers.len() {
            match current {
                Some(p) if p == ancestor => return true,
                Some(p) => current = self.layer(p).and_then(|l| l.parent),
                None => return false,
            }
        }
        false
    }

    /// Índices del bloque formado por la capa y sus descendientes (contiguos).
    pub fn subtree_range(&self, id: LayerId) -> Option<Range<usize>> {
        let start = self.layer_index(id)?;
        let len = self.layers[start + 1..]
            .iter()
            .take_while(|l| self.is_descendant(l.id, id))
            .count();
        Some(start..start + 1 + len)
    }

    /// Cantidad de grupos ancestros.
    pub fn depth(&self, id: LayerId) -> usize {
        let mut depth = 0;
        let mut current = self.layer(id).and_then(|l| l.parent);
        while let Some(p) = current {
            depth += 1;
            if depth > self.layers.len() {
                break;
            }
            current = self.layer(p).and_then(|l| l.parent);
        }
        depth
    }

    /// Visible si la capa y todos sus grupos ancestros lo son.
    pub fn is_visible(&self, id: LayerId) -> bool {
        let Some(layer) = self.layer(id) else {
            return false;
        };
        layer.visible && layer.parent.is_none_or(|p| self.is_visible(p))
    }

    /// Invariante: cada padre es un grupo que aparece antes que sus hijos, y entre ambos
    /// solo hay descendientes del mismo grupo.
    pub fn is_valid_tree(&self) -> bool {
        self.layers.iter().enumerate().all(|(i, layer)| {
            let Some(parent) = layer.parent else {
                return true;
            };
            let Some(pi) = self.layer_index(parent) else {
                return false;
            };
            self.layers[pi].kind == LayerKind::Group
                && pi < i
                && self.layers[pi + 1..i]
                    .iter()
                    .all(|l| self.is_descendant(l.id, parent))
        })
    }

    /// Primer tag que contiene `frame`.
    /// Capa base de una capa que recorta: la primera de abajo con el mismo padre que no recorta.
    pub fn clip_base(&self, id: LayerId) -> Option<LayerId> {
        let layer = self.layer(id)?;
        if !layer.clip {
            return None;
        }
        let index = self.layer_index(id)?;
        self.layers[..index]
            .iter()
            .rev()
            .filter(|l| l.parent == layer.parent)
            .find(|l| !l.clip)
            .map(|l| l.id)
    }

    /// Producto de las máscaras de la capa y sus grupos ancestros en `(x, y)` (0–255).
    pub fn mask_value(&self, id: LayerId, x: i32, y: i32) -> u8 {
        let mut value = 255u32;
        let mut current = self.layer(id);
        while let Some(layer) = current {
            if let Some(mask) = &layer.mask {
                value = (value * mask.value(x, y) as u32 + 127) / 255;
            }
            current = layer.parent.and_then(|p| self.layer(p));
        }
        value as u8
    }

    pub fn tag_at(&self, frame: usize) -> Option<&Tag> {
        self.tags.iter().find(|t| t.contains(frame))
    }
}

#[cfg(test)]
mod mask_tests {
    use super::*;

    #[test]
    fn slice_keys_apply_until_the_next_one() {
        let key = |frame, w| SliceKey {
            frame,
            bounds: (1, 2, w, 3),
            center: None,
            pivot: None,
        };
        let mut s = Slice {
            name: "hit".into(),
            color: crate::Rgba::rgb(0, 0, 255),
            text: String::new(),
            keys: vec![key(0, 4)],
        };
        s.set_key(key(3, 0));
        s.set_key(key(1, 5));
        s.set_key(key(1, 6));
        assert_eq!(
            s.keys.iter().map(|k| k.frame).collect::<Vec<_>>(),
            [0, 1, 3]
        );
        assert_eq!(s.key_at(0).unwrap().bounds.2, 4);
        assert_eq!(s.key_at(2).unwrap().bounds.2, 6);
        assert!(s.key_at(5).is_none(), "zero width hides it");
    }

    #[test]
    fn layer_mask_values_and_growth() {
        let mut m = LayerMask::filled(2, 2, 255);
        m.data = Arc::new(vec![0, 255, 128, 255]);
        assert_eq!((m.get(0, 0), m.get(0, 1), m.get(5, 5)), (0, 128, 255));
        m.enabled = false;
        assert_eq!(m.value(0, 0), 255);
        m.enabled = true;
        let g = m.grown_to(-1, 0, 2, 3);
        assert_eq!((g.position, g.width, g.height), ((-1, 0), 3, 3));
        assert_eq!((g.get(0, 0), g.get(0, 1), g.get(-1, 2)), (0, 128, 255));
        assert_eq!(m.grown_to(0, 0, 1, 1), m);
    }

    #[test]
    fn clip_base_and_nested_masks() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        let mut group = Layer::new_image(LayerId(2), "g");
        group.kind = LayerKind::Group;
        group.mask = Some(LayerMask::filled(2, 2, 128));
        let mut base = Layer::new_image(LayerId(3), "base");
        base.parent = Some(LayerId(2));
        let mut shade = Layer::new_image(LayerId(4), "shade");
        shade.parent = Some(LayerId(2));
        shade.clip = true;
        shade.mask = Some(LayerMask::filled(2, 2, 128));
        let mut light = Layer::new_image(LayerId(5), "light");
        light.parent = Some(LayerId(2));
        light.clip = true;
        s.layers.extend([group, base, shade, light]);
        assert_eq!(s.clip_base(LayerId(4)), Some(LayerId(3)));
        assert_eq!(
            s.clip_base(LayerId(5)),
            Some(LayerId(3)),
            "stacked clips share the base"
        );
        assert_eq!(s.clip_base(LayerId(3)), None);
        assert_eq!(s.mask_value(LayerId(3), 0, 0), 128);
        assert_eq!(s.mask_value(LayerId(4), 0, 0), 64);
        assert_eq!(s.mask_value(LayerId(1), 0, 0), 255);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sprite_has_one_layer_one_frame_no_cels() {
        let s = Sprite::new(16, 8, ColorMode::Rgba);
        assert_eq!((s.width, s.height), (16, 8));
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.layers[0].name, "Layer 1");
        assert_eq!(s.frames, vec![Frame { duration_ms: 100 }]);
        assert!(s.cels.is_empty());
        assert_eq!(s.palette.len(), 32);
    }

    #[test]
    fn id_allocation_is_max_plus_one() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        assert_eq!(s.next_layer_id(), LayerId(2));
        assert_eq!(s.next_image_id(), ImageId(1));
        s.images
            .insert(ImageId(7), Arc::new(Image::new(1, 1, ColorMode::Rgba)));
        assert_eq!(s.next_image_id(), ImageId(8));
    }

    /// Abajo → arriba: bg, grupo g { a, b }, top.
    fn tree() -> Sprite {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        let mut group = Layer::new_image(LayerId(2), "g");
        group.kind = LayerKind::Group;
        let mut a = Layer::new_image(LayerId(3), "a");
        a.parent = Some(LayerId(2));
        let mut b = Layer::new_image(LayerId(4), "b");
        b.parent = Some(LayerId(2));
        s.layers
            .extend([group, a, b, Layer::new_image(LayerId(5), "top")]);
        s
    }

    #[test]
    fn subtree_depth_and_visibility() {
        let mut s = tree();
        assert_eq!(s.subtree_range(LayerId(2)), Some(1..4));
        assert_eq!(s.subtree_range(LayerId(5)), Some(4..5));
        assert_eq!(s.depth(LayerId(4)), 1);
        assert!(s.is_valid_tree());
        s.layer_mut(LayerId(2)).unwrap().visible = false;
        assert!(!s.is_visible(LayerId(3)));
        assert!(s.is_visible(LayerId(5)));
    }

    #[test]
    fn invalid_tree_is_detected() {
        let mut s = tree();
        // Mueve "top" entre el grupo y sus hijos.
        let top = s.layers.remove(4);
        s.layers.insert(2, top);
        assert!(!s.is_valid_tree());
        // Intercambiar dos hermanos dentro del grupo es válido.
        let mut s = tree();
        s.layers.swap(2, 3);
        assert!(s.is_valid_tree());
        // Un hijo antes que su grupo no lo es.
        s.layers.swap(1, 2);
        assert!(!s.is_valid_tree());
    }

    #[test]
    fn json_roundtrip_with_cels() {
        let mut s = Sprite::new(4, 4, ColorMode::Rgba);
        let mut img = Image::new(2, 1, ColorMode::Rgba);
        img.set(1, 0, PixelValue::Rgba(crate::Rgba::rgb(1, 2, 3)))
            .unwrap();
        s.images.insert(ImageId(3), Arc::new(img));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (-1, 2),
                opacity: 200,
                image: ImageId(3),
            },
        );
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Sprite>(&json).unwrap(), s);
    }

    #[test]
    fn pixel_respects_cel_position() {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let red = PixelValue::Rgba(crate::Rgba::rgb(255, 0, 0));
        let mut img = Image::new(2, 2, ColorMode::Rgba);
        img.set(1, 1, red).unwrap();
        s.images.insert(ImageId(1), Arc::new(img));
        s.cels.insert(
            (LayerId(1), 0),
            Cel {
                position: (3, 4),
                opacity: 255,
                image: ImageId(1),
            },
        );
        assert_eq!(s.pixel(LayerId(1), 0, 4, 5), red);
        assert_eq!(s.pixel(LayerId(1), 0, 3, 4), s.color_mode.transparent());
        assert_eq!(s.pixel(LayerId(1), 0, 0, 0), s.color_mode.transparent());
        assert_eq!(s.pixel(LayerId(1), 1, 4, 5), s.color_mode.transparent());
    }
}
