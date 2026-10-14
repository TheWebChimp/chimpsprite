use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{ColorMode, Image, Palette, PixelValue};

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
