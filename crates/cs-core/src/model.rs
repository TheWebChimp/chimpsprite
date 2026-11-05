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

/// Una carpeta del outliner: mueve, oculta y bloquea a lo que tiene adentro.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Group3D {
    pub name: String,
    pub transform: Transform3D,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    /// Desplegado en el outliner.
    #[serde(default = "yes")]
    pub open: bool,
}

/// Un nodo del árbol del outliner: un objeto o un grupo con sus hijos (índices a `objects` y
/// `groups`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Node {
    Object(usize),
    Group { group: usize, children: Vec<Node> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Object3D {
    pub name: String,
    pub mesh: Mesh,
    pub transform: Transform3D,
    #[serde(default)]
    pub double_sided: bool,
    /// La primitiva de la que salió (para su ícono); `None` en archivos viejos.
    #[serde(default)]
    pub kind: Option<PrimitiveKind>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
}

/// El tipo de primitiva de un objeto (sin sus parámetros).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveKind {
    Cube,
    Plane,
    Cylinder,
    Cone,
    Sphere,
}

impl Object3D {
    /// El tipo que se muestra: el guardado o, en archivos viejos, el que dice la primera palabra del
    /// nombre ("Cylinder 2" es un cilindro); si no, un cubo.
    pub fn shown_kind(&self) -> PrimitiveKind {
        if let Some(k) = self.kind {
            return k;
        }
        let name = self.name.to_ascii_lowercase();
        // La primera palabra entera: "Conveyor" no es un cono.
        let word = name.split_whitespace().next().unwrap_or("");
        [
            ("plane", PrimitiveKind::Plane),
            ("cylinder", PrimitiveKind::Cylinder),
            ("cone", PrimitiveKind::Cone),
            ("sphere", PrimitiveKind::Sphere),
        ]
        .into_iter()
        .find(|(first, _)| word == *first)
        .map_or(PrimitiveKind::Cube, |(_, k)| k)
    }
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
    /// Sombras con tramado: cada cara mezcla los dos pasos de luz más cercanos con un patrón
    /// (en vez de quedarse con uno solo).
    #[serde(default)]
    pub dither: bool,
}

impl Default for Light3D {
    fn default() -> Self {
        Self {
            enabled: true,
            yaw: 300,
            pitch: 450,
            dither: false,
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

    /// Deja la tabla como la esperan [`get`](Self::get) y [`set`](Self::set) (un archivo editado
    /// a mano podría no estarlo): ordenada por (r, g, b, a), sin repetidos (gana el último) ni
    /// entradas vacías.
    pub fn normalize(&mut self) {
        self.entries.sort_by_key(|e| Self::key(e.color));
        // `dedup_by` pasa (la de después, la que queda): la que queda toma los pasos de la última.
        self.entries.dedup_by(|later, kept| {
            let same = Self::key(later.color) == Self::key(kept.color);
            if same {
                kept.steps = later.steps;
            }
            same
        });
        self.entries.retain(|e| e.steps.iter().any(Option::is_some));
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
    /// Las carpetas del outliner.
    #[serde(default)]
    pub groups: Vec<Group3D>,
    /// El árbol del outliner (vacío en archivos viejos: `normalize_tree` lo arma).
    #[serde(default)]
    pub tree: Vec<Node>,
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
            groups: Vec::new(),
            tree: Vec::new(),
        }
    }
}

fn yes() -> bool {
    true
}

/// Busca `target` en `nodes` y devuelve los grupos que lo contienen, del más cercano al más
/// lejano. `None` si no está.
fn path_to(nodes: &[Node], target: &Node) -> Option<Vec<usize>> {
    for n in nodes {
        if n == target {
            return Some(Vec::new());
        }
        if let Node::Group { group, children } = n {
            if let Node::Group { group: g, .. } = target
                && g == group
            {
                return Some(Vec::new());
            }
            if let Some(mut p) = path_to(children, target) {
                p.push(*group);
                return Some(p);
            }
        }
    }
    None
}

impl Model3D {
    /// Deja el árbol del outliner consistente: cada objeto y cada grupo una sola vez (se quitan
    /// los índices fuera de rango y los repetidos) y lo que falte, al final de la raíz en orden.
    pub fn normalize_tree(&mut self) {
        let (no, ng) = (self.objects.len(), self.groups.len());
        let mut seen_o = vec![false; no];
        let mut seen_g = vec![false; ng];
        fn clean(nodes: Vec<Node>, so: &mut [bool], sg: &mut [bool]) -> Vec<Node> {
            nodes
                .into_iter()
                .filter_map(|n| match n {
                    Node::Object(o) if o < so.len() && !so[o] => {
                        so[o] = true;
                        Some(Node::Object(o))
                    }
                    Node::Group { group, children } if group < sg.len() && !sg[group] => {
                        sg[group] = true;
                        Some(Node::Group {
                            group,
                            children: clean(children, so, sg),
                        })
                    }
                    _ => None,
                })
                .collect()
        }
        let mut tree = clean(std::mem::take(&mut self.tree), &mut seen_o, &mut seen_g);
        tree.extend((0..ng).filter(|&g| !seen_g[g]).map(|g| Node::Group {
            group: g,
            children: Vec::new(),
        }));
        tree.extend((0..no).filter(|&o| !seen_o[o]).map(Node::Object));
        self.tree = tree;
    }

    pub fn object_parents(&self, o: usize) -> Vec<usize> {
        path_to(&self.tree, &Node::Object(o)).unwrap_or_default()
    }

    pub fn group_parents(&self, g: usize) -> Vec<usize> {
        path_to(
            &self.tree,
            &Node::Group {
                group: g,
                children: Vec::new(),
            },
        )
        .unwrap_or_default()
    }

    pub fn object_hidden(&self, o: usize) -> bool {
        self.objects.get(o).is_some_and(|ob| ob.hidden)
            || self
                .object_parents(o)
                .iter()
                .any(|&g| self.groups.get(g).is_some_and(|g| g.hidden))
    }

    pub fn object_locked(&self, o: usize) -> bool {
        self.objects.get(o).is_some_and(|ob| ob.locked)
            || self
                .object_parents(o)
                .iter()
                .any(|&g| self.groups.get(g).is_some_and(|g| g.locked))
    }

    pub fn object_chain(&self, o: usize) -> Vec<Transform3D> {
        let mut out = vec![
            self.objects
                .get(o)
                .map(|ob| ob.transform)
                .unwrap_or_default(),
        ];
        out.extend(
            self.object_parents(o)
                .iter()
                .filter_map(|&g| self.groups.get(g).map(|g| g.transform)),
        );
        out
    }

    pub fn group_objects(&self, g: usize) -> Vec<usize> {
        fn find(nodes: &[Node], g: usize) -> Option<&[Node]> {
            nodes.iter().find_map(|n| match n {
                Node::Group { group, children } if *group == g => Some(children.as_slice()),
                Node::Group { children, .. } => find(children, g),
                Node::Object(_) => None,
            })
        }
        fn collect(nodes: &[Node], out: &mut Vec<usize>) {
            for n in nodes {
                match n {
                    Node::Object(o) => out.push(*o),
                    Node::Group { children, .. } => collect(children, out),
                }
            }
        }
        let mut out = Vec::new();
        if let Some(children) = find(&self.tree, g) {
            collect(children, &mut out);
        }
        out.sort_unstable();
        out
    }

    pub fn push_object(&mut self, obj: Object3D, parent: Option<usize>) -> usize {
        self.normalize_tree();
        self.objects.push(obj);
        let i = self.objects.len() - 1;
        fn put(nodes: &mut [Node], g: usize, i: usize) -> bool {
            nodes.iter_mut().any(|n| match n {
                Node::Group { group, children } if *group == g => {
                    children.push(Node::Object(i));
                    true
                }
                Node::Group { children, .. } => put(children, g, i),
                Node::Object(_) => false,
            })
        }
        if !parent.is_some_and(|g| put(&mut self.tree, g, i)) {
            self.tree.push(Node::Object(i));
        }
        i
    }

    pub fn remove_object(&mut self, o: usize) {
        if o >= self.objects.len() {
            return;
        }
        self.objects.remove(o);
        fn fix(nodes: Vec<Node>, o: usize) -> Vec<Node> {
            nodes
                .into_iter()
                .filter_map(|n| match n {
                    Node::Object(i) if i == o => None,
                    Node::Object(i) => Some(Node::Object(if i > o { i - 1 } else { i })),
                    Node::Group { group, children } => Some(Node::Group {
                        group,
                        children: fix(children, o),
                    }),
                })
                .collect()
        }
        self.tree = fix(std::mem::take(&mut self.tree), o);
        self.normalize_tree();
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
                kind: None,
                hidden: false,
                locked: false,
            }],
            ..Model3D::default()
        }
    }

    fn sprite() -> Sprite {
        Sprite::new(16, 16, ColorMode::Rgba)
    }

    #[test]
    fn el_tipo_del_objeto_va_y_vuelve_por_json() {
        let mut o = model_with(cube()).objects.remove(0);
        o.kind = Some(PrimitiveKind::Cylinder);
        let s = serde_json::to_string(&o).unwrap();
        let back: Object3D = serde_json::from_str(&s).unwrap();
        assert_eq!(back, o);
        assert_eq!(back.shown_kind(), PrimitiveKind::Cylinder);
    }

    #[test]
    fn un_objeto_viejo_sin_tipo_lo_deduce_del_nombre() {
        let o = model_with(cube()).objects.remove(0);
        let mut v = serde_json::to_value(&o).unwrap();
        v.as_object_mut().unwrap().remove("kind");
        let named = |name: &str| {
            let mut v = v.clone();
            v["name"] = serde_json::Value::from(name);
            serde_json::from_value::<Object3D>(v).unwrap()
        };
        let old = named("Cone");
        assert_eq!(old.kind, None);
        assert_eq!(old.shown_kind(), PrimitiveKind::Cone);
        assert_eq!(named("sphere 2").shown_kind(), PrimitiveKind::Sphere);
        assert_eq!(named("PLANE").shown_kind(), PrimitiveKind::Plane);
        assert_eq!(named("Cylinder").shown_kind(), PrimitiveKind::Cylinder);
        assert_eq!(named("Cube").shown_kind(), PrimitiveKind::Cube);
        assert_eq!(named("Casa").shown_kind(), PrimitiveKind::Cube);
        assert_eq!(named("cone").shown_kind(), PrimitiveKind::Cone);
        assert_eq!(named("cone 2").shown_kind(), PrimitiveKind::Cone);
        assert_eq!(named("Conveyor").shown_kind(), PrimitiveKind::Cube);
        assert_eq!(named("Planet").shown_kind(), PrimitiveKind::Cube);
        // El tipo guardado manda sobre el nombre.
        let mut o = named("Cone");
        o.kind = Some(PrimitiveKind::Plane);
        assert_eq!(o.shown_kind(), PrimitiveKind::Plane);
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
                pitch: 450,
                dither: false,
            }
        );
    }

    #[test]
    fn a_light_saved_before_dither_reads_without_it() {
        let old = r#"{"enabled":true,"yaw":300,"pitch":450}"#;
        let light: Light3D = serde_json::from_str(old).unwrap();
        assert_eq!(light, Light3D::default());
        let on = Light3D {
            dither: true,
            ..Light3D::default()
        };
        let back: Light3D = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert_eq!(back, on);
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
    fn normalize_ordena_quita_repetidos_y_vacias() {
        let c = |v| Rgba::new(v, v, v, 255);
        let e = |color, a, b| ShadeEntry {
            color,
            steps: [a, b],
        };
        let mut t = ShadingTable {
            entries: vec![
                e(c(200), Some(c(1)), None),
                e(c(50), None, None),
                e(c(100), None, Some(c(2))),
                e(c(200), None, Some(c(3))),
            ],
        };
        t.normalize();
        assert_eq!(
            t.entries,
            vec![e(c(100), None, Some(c(2))), e(c(200), None, Some(c(3)))]
        );
        assert_eq!(t.get(c(200), 2), Some(c(3)));
        assert_eq!(t.get(c(100), 2), Some(c(2)));
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

    fn cube_obj() -> Object3D {
        model_with(cube()).objects.remove(0)
    }

    fn grouped() -> Model3D {
        // cube(0) suelto; grupo 0 "legs" con cube(1) y el grupo 1 "foot" con cube(2).
        let mut m = model_with(cube());
        m.objects.push(cube_obj());
        m.objects.push(cube_obj());
        m.groups = vec![
            Group3D {
                name: "legs".into(),
                transform: Transform3D::default(),
                hidden: false,
                locked: false,
                open: true,
            },
            Group3D {
                name: "foot".into(),
                transform: Transform3D::default(),
                hidden: false,
                locked: false,
                open: true,
            },
        ];
        m.tree = vec![
            Node::Object(0),
            Node::Group {
                group: 0,
                children: vec![
                    Node::Object(1),
                    Node::Group {
                        group: 1,
                        children: vec![Node::Object(2)],
                    },
                ],
            },
        ];
        m
    }

    #[test]
    fn a_model_without_a_tree_gets_one_with_its_objects_in_order() {
        let mut m = model_with(cube());
        m.objects.push(cube_obj());
        m.normalize_tree();
        assert_eq!(m.tree, vec![Node::Object(0), Node::Object(1)]);
        // Idempotente.
        let again = m.clone();
        m.normalize_tree();
        assert_eq!(m, again);
    }

    #[test]
    fn normalize_drops_bad_indices_and_adds_what_is_missing() {
        let mut m = grouped();
        m.tree.push(Node::Object(7)); // no existe
        m.tree.push(Node::Object(1)); // repetido
        m.objects.push(cube_obj()); // el 3 no está en el árbol
        m.normalize_tree();
        assert_eq!(m.tree.len(), 3, "{:?}", m.tree);
        assert_eq!(m.tree[2], Node::Object(3));
        assert_eq!(m.object_parents(1), vec![0]);
    }

    #[test]
    fn parents_hidden_locked_and_chain_are_inherited() {
        let mut m = grouped();
        assert_eq!(m.object_parents(2), vec![1, 0]);
        assert_eq!(m.object_parents(0), Vec::<usize>::new());
        assert_eq!(m.group_parents(1), vec![0]);
        m.groups[0].hidden = true;
        m.groups[1].locked = true;
        assert!(m.object_hidden(2) && m.object_hidden(1) && !m.object_hidden(0));
        assert!(m.object_locked(2) && !m.object_locked(1));
        m.groups[0].transform.position.x = 16;
        let chain = m.object_chain(2);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[2].position.x, 16, "the outermost group last");
        assert_eq!(m.group_objects(0), vec![1, 2]);
    }

    #[test]
    fn push_and_remove_keep_the_tree() {
        let mut m = grouped();
        let i = m.push_object(cube_obj(), Some(1));
        assert_eq!(i, 3);
        assert_eq!(m.object_parents(3), vec![1, 0]);
        m.remove_object(1);
        // El 2 pasa a ser el 1 (y el 3 el 2), en sus mismos grupos.
        assert_eq!(m.objects.len(), 3);
        assert_eq!(m.object_parents(1), vec![1, 0]);
        assert_eq!(m.object_parents(2), vec![1, 0]);
        assert!(!format!("{:?}", m.tree).contains("Object(3)"));
    }

    #[test]
    fn a_model_saved_before_groups_reads_without_them() {
        let m = model_with(cube());
        let mut v = serde_json::to_value(&m).unwrap();
        let o = v.as_object_mut().unwrap();
        o.remove("groups");
        o.remove("tree");
        for obj in o["objects"].as_array_mut().unwrap() {
            let obj = obj.as_object_mut().unwrap();
            obj.remove("hidden");
            obj.remove("locked");
        }
        let back: Model3D = serde_json::from_value(v).unwrap();
        assert!(back.groups.is_empty() && back.tree.is_empty());
        assert!(!back.objects[0].hidden && !back.objects[0].locked);
    }
}
