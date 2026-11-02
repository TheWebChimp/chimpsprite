//! Modelos 3D de pocos polígonos (tipo picoCAD): un artboard con `model` es un modelo. Todo lo
//! guardado es entero (sin floats): vértices en 1/16 de unidad, ángulos en décimas de grado y
//! escalas en milésimas.

use serde::{Deserialize, Serialize};

use crate::{ArtboardId, LayerKind, Rgba, Sprite};

/// Una unidad del modelo equivale a `FIXED` pasos de punto fijo.
pub const FIXED: i32 = 16;

/// Vector de enteros; en vértices y posiciones son 1/16 de unidad.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Vec3i {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Un triángulo o un cuadrilátero; los UV están en pixeles de la textura.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Face {
    pub verts: Vec<u32>,
    pub uvs: Vec<(i32, i32)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vec3i>,
    pub faces: Vec<Face>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Transform3D {
    pub position: Vec3i,
    /// En décimas de grado.
    pub rotation: Vec3i,
    /// En milésimas (1000 = 1).
    pub scale: Vec3i,
}

impl Default for Transform3D {
    fn default() -> Self {
        Self {
            position: Vec3i::default(),
            rotation: Vec3i::default(),
            scale: Vec3i {
                x: 1000,
                y: 1000,
                z: 1000,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Object3D {
    pub name: String,
    pub mesh: Mesh,
    pub transform: Transform3D,
    #[serde(default)]
    pub double_sided: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Projection {
    /// `fov` en décimas de grado.
    Perspective {
        fov: i32,
    },
    Orthographic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Camera3D {
    pub projection: Projection,
    /// Décimas de grado.
    pub yaw: i32,
    /// Décimas de grado.
    pub pitch: i32,
    /// En 1/16 de unidad.
    pub distance: i32,
    pub target: Vec3i,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CameraPreset {
    Iso,
    Top,
    Side,
    Front,
}

impl CameraPreset {
    /// La cámara ortográfica del preset, mirando a `target` desde `distance`.
    pub fn camera(self, distance: i32, target: Vec3i) -> Camera3D {
        // La iso es la 2:1 del pixel art: 45° de giro y 30° de altura.
        let (yaw, pitch) = match self {
            CameraPreset::Iso => (450, 300),
            CameraPreset::Top => (0, 900),
            CameraPreset::Side => (900, 0),
            CameraPreset::Front => (0, 0),
        };
        Camera3D {
            projection: Projection::Orthographic,
            yaw,
            pitch,
            distance,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Light3D {
    pub enabled: bool,
    pub yaw: i32,
    pub pitch: i32,
}

impl Default for Light3D {
    fn default() -> Self {
        Self {
            enabled: true,
            yaw: 300,
            pitch: 450,
        }
    }
}

/// Los colores a los que se oscurece `color` en cada paso de luz; `None` = automático.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadeEntry {
    pub color: Rgba,
    pub steps: [Option<Rgba>; 2],
}

/// Rampas de luz explícitas por color, ordenadas por (r, g, b, a), sin repetidos ni entradas
/// vacías.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ShadingTable {
    pub entries: Vec<ShadeEntry>,
}

impl ShadingTable {
    fn key(c: Rgba) -> (u8, u8, u8, u8) {
        (c.r, c.g, c.b, c.a)
    }

    /// El color fijado para `color` en el paso `step` (1 o 2), si lo hay.
    pub fn get(&self, color: Rgba, step: usize) -> Option<Rgba> {
        if !(1..=2).contains(&step) {
            return None;
        }
        let i = self
            .entries
            .binary_search_by_key(&Self::key(color), |e| Self::key(e.color))
            .ok()?;
        self.entries[i].steps[step - 1]
    }

    /// Fija (o con `None` quita) el color del paso `step` (1 o 2); borra la entrada si queda
    /// vacía.
    pub fn set(&mut self, color: Rgba, step: usize, value: Option<Rgba>) {
        if !(1..=2).contains(&step) {
            return;
        }
        let found = self
            .entries
            .binary_search_by_key(&Self::key(color), |e| Self::key(e.color));
        match (found, value) {
            (Ok(i), _) => {
                self.entries[i].steps[step - 1] = value;
                if self.entries[i].steps.iter().all(Option::is_none) {
                    self.entries.remove(i);
                }
            }
            (Err(i), Some(_)) => {
                let mut entry = ShadeEntry {
                    color,
                    steps: [None; 2],
                };
                entry.steps[step - 1] = value;
                self.entries.insert(i, entry);
            }
            (Err(_), None) => {}
        }
    }
}

/// Un modelo 3D: objetos de malla, la textura (otro artboard), la cámara y la luz.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Model3D {
    pub objects: Vec<Object3D>,
    pub texture: Option<ArtboardId>,
    pub camera: Camera3D,
    pub light: Light3D,
    #[serde(default)]
    pub outline: Option<Rgba>,
    #[serde(default)]
    pub shading: ShadingTable,
}

impl Default for Model3D {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            texture: None,
            camera: Camera3D {
                projection: Projection::Perspective { fov: 600 },
                yaw: 300,
                pitch: 250,
                distance: 5 * FIXED,
                target: Vec3i::default(),
            },
            light: Light3D::default(),
            outline: None,
            shading: ShadingTable::default(),
        }
    }
}

impl Model3D {
    /// Revisa que `sprite` (el del artboard) sirva de modelo: una capa de imagen, un frame, y
    /// caras de 3 o 4 vértices válidos con un UV por vértice.
    pub fn check(&self, sprite: &Sprite) -> Result<(), String> {
        if sprite.layers.len() != 1 || sprite.layers[0].kind != LayerKind::Image {
            return Err("un modelo 3D tiene una sola capa de imagen".into());
        }
        if sprite.frames.len() != 1 {
            return Err("un modelo 3D tiene un solo frame".into());
        }
        for obj in &self.objects {
            let n = obj.mesh.vertices.len();
            for (i, face) in obj.mesh.faces.iter().enumerate() {
                if !(3..=4).contains(&face.verts.len()) {
                    return Err(format!(
                        "{}: la cara {i} tiene {} vértices (deben ser 3 o 4)",
                        obj.name,
                        face.verts.len()
                    ));
                }
                if face.verts.iter().any(|&v| v as usize >= n) {
                    return Err(format!(
                        "{}: la cara {i} usa un vértice que no existe",
                        obj.name
                    ));
                }
                if face.uvs.len() != face.verts.len() {
                    return Err(format!(
                        "{}: la cara {i} no tiene un UV por vértice",
                        obj.name
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ColorMode, Sprite};

    fn cube() -> Mesh {
        let v = |x, y, z| Vec3i { x, y, z };
        let vertices = vec![
            v(-8, -8, -8),
            v(8, -8, -8),
            v(8, 8, -8),
            v(-8, 8, -8),
            v(-8, -8, 8),
            v(8, -8, 8),
            v(8, 8, 8),
            v(-8, 8, 8),
        ];
        let quad = |a, b, c, d| Face {
            verts: vec![a, b, c, d],
            uvs: vec![(0, 0), (8, 0), (8, 8), (0, 8)],
        };
        let faces = vec![
            quad(0, 1, 2, 3),
            quad(5, 4, 7, 6),
            quad(4, 0, 3, 7),
            quad(1, 5, 6, 2),
            quad(4, 5, 1, 0),
            quad(3, 2, 6, 7),
        ];
        Mesh { vertices, faces }
    }

    fn model_with(mesh: Mesh) -> Model3D {
        Model3D {
            objects: vec![Object3D {
                name: "o".into(),
                mesh,
                transform: Transform3D::default(),
                double_sided: false,
            }],
            ..Model3D::default()
        }
    }

    fn sprite() -> Sprite {
        Sprite::new(16, 16, ColorMode::Rgba)
    }

    #[test]
    fn transform_por_defecto_escala_1000() {
        let t = Transform3D::default();
        assert_eq!(
            t.scale,
            Vec3i {
                x: 1000,
                y: 1000,
                z: 1000
            }
        );
        assert_eq!(t.position, Vec3i::default());
        assert_eq!(t.rotation, Vec3i::default());
    }

    #[test]
    fn modelo_por_defecto_va_y_vuelve_por_json() {
        let m = Model3D::default();
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Model3D>(&s).unwrap(), m);
        assert_eq!(m.camera.projection, Projection::Perspective { fov: 600 });
        assert_eq!((m.camera.yaw, m.camera.pitch), (300, 250));
        assert_eq!(m.camera.distance, 5 * FIXED);
        assert_eq!(
            m.light,
            Light3D {
                enabled: true,
                yaw: 300,
                pitch: 450
            }
        );
    }

    #[test]
    fn check_pasa_con_un_cubo() {
        assert_eq!(model_with(cube()).check(&sprite()), Ok(()));
    }

    #[test]
    fn check_falla_con_dos_frames() {
        let mut s = sprite();
        s.frames.push(crate::Frame { duration_ms: 100 });
        assert!(model_with(cube()).check(&s).is_err());
    }

    #[test]
    fn check_falla_con_cara_de_cinco() {
        let mut m = cube();
        m.faces[0].verts.push(0);
        m.faces[0].uvs.push((0, 0));
        assert!(model_with(m).check(&sprite()).is_err());
    }

    #[test]
    fn check_falla_con_indice_fuera_de_rango() {
        let mut m = cube();
        m.faces[0].verts[0] = 8;
        assert!(model_with(m).check(&sprite()).is_err());
    }

    #[test]
    fn check_falla_con_uvs_de_otro_largo() {
        let mut m = cube();
        m.faces[0].uvs.pop();
        assert!(model_with(m).check(&sprite()).is_err());
    }

    #[test]
    fn shading_set_get_ordena_y_borra_vacias() {
        let c = |v| Rgba::new(v, v, v, 255);
        let mut t = ShadingTable::default();
        t.set(c(200), 1, Some(c(10)));
        t.set(c(50), 2, Some(c(20)));
        t.set(c(200), 2, Some(c(30)));
        t.set(c(50), 2, Some(c(21)));
        assert_eq!(t.entries.len(), 2);
        assert_eq!(t.entries[0].color, c(50));
        assert_eq!(t.get(c(200), 1), Some(c(10)));
        assert_eq!(t.get(c(200), 2), Some(c(30)));
        assert_eq!(t.get(c(50), 1), None);
        assert_eq!(t.get(c(50), 2), Some(c(21)));
        assert_eq!(t.get(c(9), 1), None);
        t.set(c(50), 2, None);
        assert_eq!(t.entries.len(), 1);
        t.set(c(200), 1, None);
        t.set(c(200), 2, None);
        assert!(t.entries.is_empty());
        // Borrar algo que no existe no crea entradas.
        t.set(c(7), 1, None);
        assert!(t.entries.is_empty());
    }

    #[test]
    fn modelo_de_la_parte_1_abre_con_tabla_vacia() {
        let mut v = serde_json::to_value(Model3D::default()).unwrap();
        v.as_object_mut().unwrap().remove("shading");
        let m: Model3D = serde_json::from_value(v).unwrap();
        assert_eq!(m.shading, ShadingTable::default());
    }

    #[test]
    fn preset_iso() {
        let c = CameraPreset::Iso.camera(80, Vec3i::default());
        assert_eq!(c.projection, Projection::Orthographic);
        assert_eq!((c.yaw, c.pitch, c.distance), (450, 300, 80));
    }
}
