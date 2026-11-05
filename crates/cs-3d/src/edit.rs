//! Editar modelos con el mouse en las vistas ortográficas (Front, Top y Side, como picoCAD):
//! qué se ve en cada una, qué hay bajo el puntero, la selección (objetos, vértices, aristas o
//! caras) y moverla o escalarla. Todo en el mundo, en 1/16 de unidad (el punto fijo de los
//! vértices) como `f32`.

use std::collections::{BTreeMap, BTreeSet};

use cs_core::{Mesh, Model3D, Object3D, Transform3D, Vec3i};

/// Un punto del mundo en 1/16 de unidad.
pub type P3 = [f32; 3];

/// Las vistas ortográficas del modo Edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OrthoView {
    /// Desde +Z: X a la derecha, Y arriba.
    #[default]
    Front,
    /// Desde +Y: X a la derecha, Z hacia abajo.
    Top,
    /// Desde +X: Z a la izquierda, Y arriba.
    Side,
}

impl OrthoView {
    pub const ALL: [OrthoView; 3] = [OrthoView::Front, OrthoView::Top, OrthoView::Side];

    pub fn label(self) -> &'static str {
        match self {
            OrthoView::Front => "Front",
            OrthoView::Top => "Top",
            OrthoView::Side => "Side",
        }
    }

    /// El punto en el plano de la vista: `u` a la derecha y `v` arriba.
    pub fn plane(self, p: P3) -> (f32, f32) {
        match self {
            OrthoView::Front => (p[0], p[1]),
            OrthoView::Top => (p[0], -p[2]),
            OrthoView::Side => (-p[2], p[1]),
        }
    }

    /// Qué tan cerca está `p` del que mira (mayor = más cerca).
    pub fn depth(self, p: P3) -> f32 {
        match self {
            OrthoView::Front => p[2],
            OrthoView::Top => p[1],
            OrthoView::Side => p[0],
        }
    }

    /// El punto del mundo con `(u, v)` en el plano y la profundidad de `base`.
    pub fn from_plane(self, (u, v): (f32, f32), base: P3) -> P3 {
        match self {
            OrthoView::Front => [u, v, base[2]],
            OrthoView::Top => [u, base[1], -v],
            OrthoView::Side => [base[0], v, -u],
        }
    }

    /// El desplazamiento del mundo que corre `d` en el plano (sin cambiar la profundidad).
    pub fn world_delta(self, d: (f32, f32)) -> P3 {
        self.from_plane(d, [0.0; 3])
    }

    /// Los ejes del mundo (0 x, 1 y, 2 z) de `u` y de `v`.
    pub fn axes(self) -> (usize, usize) {
        match self {
            OrthoView::Front => (0, 1),
            OrthoView::Top => (0, 2),
            OrthoView::Side => (2, 1),
        }
    }
}

fn sin_cos(tenths: i32) -> (f32, f32) {
    (tenths as f32 / 10.0).to_radians().sin_cos()
}

pub(crate) fn p3(v: Vec3i) -> P3 {
    [v.x as f32, v.y as f32, v.z as f32]
}

pub(crate) fn round3(p: P3) -> Vec3i {
    Vec3i {
        x: p[0].round() as i32,
        y: p[1].round() as i32,
        z: p[2].round() as i32,
    }
}

/// Un punto local del objeto en el mundo, como el rasterizador: escala → rotación X, Y, Z →
/// posición.
pub fn to_world(t: &Transform3D, p: P3) -> P3 {
    let (x, y, z) = (
        p[0] * (t.scale.x as f32 / 1000.0),
        p[1] * (t.scale.y as f32 / 1000.0),
        p[2] * (t.scale.z as f32 / 1000.0),
    );
    let (sx, cx) = sin_cos(t.rotation.x);
    let (sy, cy) = sin_cos(t.rotation.y);
    let (sz, cz) = sin_cos(t.rotation.z);
    let (y, z) = (y * cx - z * sx, y * sx + z * cx);
    let (x, z) = (x * cy + z * sy, -x * sy + z * cy);
    let (x, y) = (x * cz - y * sz, x * sz + y * cz);
    [
        x + t.position.x as f32,
        y + t.position.y as f32,
        z + t.position.z as f32,
    ]
}

/// Lo contrario de [`to_world`]. En un eje con escala 0 no hay vuelta: queda el de `keep`.
pub fn to_local(t: &Transform3D, w: P3, keep: P3) -> P3 {
    let (x, y, z) = (
        w[0] - t.position.x as f32,
        w[1] - t.position.y as f32,
        w[2] - t.position.z as f32,
    );
    let (sx, cx) = sin_cos(t.rotation.x);
    let (sy, cy) = sin_cos(t.rotation.y);
    let (sz, cz) = sin_cos(t.rotation.z);
    let (x, y) = (x * cz + y * sz, -x * sz + y * cz);
    let (x, z) = (x * cy - z * sy, x * sy + z * cy);
    let (y, z) = (y * cx + z * sx, -y * sx + z * cx);
    let back = |v: f32, s: i32, k: f32| if s == 0 { k } else { v * 1000.0 / s as f32 };
    [
        back(x, t.scale.x, keep[0]),
        back(y, t.scale.y, keep[1]),
        back(z, t.scale.z, keep[2]),
    ]
}

/// Un punto local del objeto `o` en el mundo: por su transform y luego por los de sus grupos
/// (del más cercano al de más afuera).
pub fn object_to_world(model: &Model3D, o: usize, p: P3) -> P3 {
    model.object_chain(o).iter().fold(p, |p, t| to_world(t, p))
}

/// Lo contrario de [`object_to_world`]. `keep` (ver [`to_local`]) solo sirve en el último paso,
/// el del objeto.
pub fn object_to_local(model: &Model3D, o: usize, w: P3, keep: P3) -> P3 {
    let chain = model.object_chain(o);
    // De afuera hacia adentro.
    let mut p = w;
    for (k, t) in chain.iter().enumerate().rev() {
        p = to_local(t, p, if k == 0 { keep } else { [0.0; 3] });
    }
    p
}

/// Un punto en el espacio del padre del objeto `o` (donde vive su posición) en el mundo: solo
/// por los transforms de sus grupos.
pub fn parent_to_world(model: &Model3D, o: usize, p: P3) -> P3 {
    model
        .object_chain(o)
        .iter()
        .skip(1)
        .fold(p, |p, t| to_world(t, p))
}

/// Lo contrario de [`parent_to_world`].
pub fn parent_to_local(model: &Model3D, o: usize, w: P3) -> P3 {
    model
        .object_chain(o)
        .iter()
        .skip(1)
        .rev()
        .fold(w, |p, t| to_local(t, p, [0.0; 3]))
}

/// Si el objeto `o` se puede elegir en las vistas: ni oculto ni bloqueado (él o sus grupos).
pub fn pickable(model: &Model3D, o: usize) -> bool {
    !model.object_hidden(o) && !model.object_locked(o)
}

/// Los vértices del objeto `o` en el mundo (con los transforms de sus grupos).
pub fn world_points(model: &Model3D, o: usize) -> Vec<P3> {
    let chain = model.object_chain(o);
    model.objects[o]
        .mesh
        .vertices
        .iter()
        .map(|&v| chain.iter().fold(p3(v), |p, t| to_world(t, p)))
        .collect()
}

/// Las aristas de la malla: los pares de vértices seguidos de cada cara, `(a, b)` con `a < b`,
/// ordenadas y sin repetir (ni las que tocan vértices que no existen).
pub fn mesh_edges(mesh: &Mesh) -> Vec<(u32, u32)> {
    let n = mesh.vertices.len() as u32;
    let mut set = BTreeSet::new();
    for f in &mesh.faces {
        for i in 0..f.verts.len() {
            let (a, b) = (f.verts[i], f.verts[(i + 1) % f.verts.len()]);
            if a != b && a < n && b < n {
                set.insert((a.min(b), a.max(b)));
            }
        }
    }
    set.into_iter().collect()
}

/// Qué se elige con el mouse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SelectMode {
    #[default]
    Object,
    Vertex,
    Edge,
    Face,
}

impl SelectMode {
    pub const ALL: [SelectMode; 4] = [
        SelectMode::Object,
        SelectMode::Vertex,
        SelectMode::Edge,
        SelectMode::Face,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SelectMode::Object => "Object",
            SelectMode::Vertex => "Vertex",
            SelectMode::Edge => "Edge",
            SelectMode::Face => "Face",
        }
    }
}

/// Algo elegido: el objeto y lo que es dentro de él (vértice; arista `(a, b)` con `a < b`; cara).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Item {
    Object(usize),
    Vertex(usize, u32),
    Edge(usize, u32, u32),
    Face(usize, usize),
}

impl Item {
    pub fn object(self) -> usize {
        match self {
            Item::Object(o) | Item::Vertex(o, _) | Item::Edge(o, _, _) | Item::Face(o, _) => o,
        }
    }

    /// El mismo elemento pero en el objeto `n` (al quitar un objeto, los de después se corren).
    pub fn with_object(self, n: usize) -> Item {
        match self {
            Item::Object(_) => Item::Object(n),
            Item::Vertex(_, v) => Item::Vertex(n, v),
            Item::Edge(_, a, b) => Item::Edge(n, a, b),
            Item::Face(_, f) => Item::Face(n, f),
        }
    }

    /// Los vértices que mueve (un objeto, todos los suyos).
    fn vertices(self, model: &Model3D) -> Vec<u32> {
        let mesh = &model.objects[self.object()].mesh;
        let n = mesh.vertices.len() as u32;
        match self {
            Item::Object(_) => (0..n).collect(),
            Item::Vertex(_, v) => vec![v],
            Item::Edge(_, a, b) => vec![a, b],
            Item::Face(_, f) => mesh.faces[f]
                .verts
                .iter()
                .copied()
                .filter(|&v| v < n)
                .collect(),
        }
    }
}

/// Qué elegidos existen en un modelo. Las aristas de cada objeto se calculan una sola vez (con
/// muchas aristas elegidas, recalcularlas por cada una costaría en cada frame).
struct Existing<'a> {
    model: &'a Model3D,
    edges: BTreeMap<usize, BTreeSet<(u32, u32)>>,
}

impl<'a> Existing<'a> {
    fn new(model: &'a Model3D) -> Self {
        Self {
            model,
            edges: BTreeMap::new(),
        }
    }

    fn check(&mut self, item: Item) -> bool {
        let o = item.object();
        let Some(obj) = self.model.objects.get(o) else {
            return false;
        };
        let n = obj.mesh.vertices.len() as u32;
        match item {
            Item::Object(_) => true,
            Item::Vertex(_, v) => v < n,
            Item::Edge(_, a, b) => self
                .edges
                .entry(o)
                .or_insert_with(|| mesh_edges(&obj.mesh).into_iter().collect())
                .contains(&(a, b)),
            Item::Face(_, f) => f < obj.mesh.faces.len(),
        }
    }
}

/// Lo elegido en el modo Edit (todo del mismo tipo que `mode`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    pub mode: SelectMode,
    pub items: BTreeSet<Item>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// El objeto del primer elegido (el de índice menor).
    pub fn first_object(&self) -> Option<usize> {
        self.items.iter().map(|i| i.object()).min()
    }

    /// Lo agrega si no estaba; si estaba, lo quita.
    pub fn toggle(&mut self, item: Item) {
        if !self.items.remove(&item) {
            self.items.insert(item);
        }
    }

    /// Los vértices involucrados por objeto (sin repetir; solo de lo que existe).
    pub fn vertices(&self, model: &Model3D) -> BTreeMap<usize, BTreeSet<u32>> {
        let mut out: BTreeMap<usize, BTreeSet<u32>> = BTreeMap::new();
        let mut existing = Existing::new(model);
        for &item in self.items.iter().filter(|&&i| existing.check(i)) {
            out.entry(item.object())
                .or_default()
                .extend(item.vertices(model));
        }
        out
    }

    /// Quita lo que ya no existe en `model` (después de deshacer, por ejemplo).
    pub fn retain_valid(&mut self, model: &Model3D) {
        let mut existing = Existing::new(model);
        self.items.retain(|&i| existing.check(i));
    }

    /// La misma selección en otro modo: los objetos de lo elegido, o los vértices involucrados y
    /// las aristas y caras que tienen todos sus vértices entre ellos.
    pub fn converted(&self, model: &Model3D, mode: SelectMode) -> Selection {
        let mut items = BTreeSet::new();
        if mode == SelectMode::Object {
            let mut existing = Existing::new(model);
            items.extend(
                self.items
                    .iter()
                    .filter(|&&i| existing.check(i))
                    .map(|i| Item::Object(i.object())),
            );
            return Selection { mode, items };
        }
        for (o, vs) in self.vertices(model) {
            let mesh = &model.objects[o].mesh;
            match mode {
                SelectMode::Vertex => items.extend(vs.iter().map(|&v| Item::Vertex(o, v))),
                SelectMode::Edge => items.extend(
                    mesh_edges(mesh)
                        .into_iter()
                        .filter(|(a, b)| vs.contains(a) && vs.contains(b))
                        .map(|(a, b)| Item::Edge(o, a, b)),
                ),
                _ => items.extend(
                    mesh.faces
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| {
                            !f.verts.is_empty() && f.verts.iter().all(|v| vs.contains(v))
                        })
                        .map(|(i, _)| Item::Face(o, i)),
                ),
            }
        }
        Selection { mode, items }
    }
}

/// Qué tan cerca (en puntos de pantalla) hay que hacer clic de un vértice, una arista o el punto
/// de una cara.
pub const PICK_RADIUS: f32 = 4.0;

/// Cómo se ve el mundo en el cuadrante de una vista: `center` cae en `origin` (pantalla, y hacia
/// abajo) y cada 1/16 de unidad mide `scale` puntos.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewMap {
    pub view: OrthoView,
    pub center: P3,
    pub scale: f32,
    pub origin: (f32, f32),
}

impl ViewMap {
    pub fn to_screen(&self, p: P3) -> (f32, f32) {
        let (u, v) = self.view.plane(p);
        let (cu, cv) = self.view.plane(self.center);
        (
            self.origin.0 + (u - cu) * self.scale,
            self.origin.1 - (v - cv) * self.scale,
        )
    }

    /// El punto del plano que se ve en `s`.
    pub fn to_plane(&self, s: (f32, f32)) -> (f32, f32) {
        let (cu, cv) = self.view.plane(self.center);
        (
            cu + (s.0 - self.origin.0) / self.scale,
            cv - (s.1 - self.origin.1) / self.scale,
        )
    }

    /// Un arrastre de `d` puntos en pantalla, como desplazamiento del mundo.
    pub fn drag_delta(&self, d: (f32, f32)) -> P3 {
        self.view.world_delta((d.0 / self.scale, -d.1 / self.scale))
    }
}

pub(crate) fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

pub(crate) fn dist_to_segment(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    dist(p, (a.0 + t * dx, a.1 + t * dy))
}

/// Si `p` cae dentro del polígono (par-impar; sirve en los dos sentidos de giro).
fn inside(poly: &[(f32, f32)], p: (f32, f32)) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            odd = !odd;
        }
    }
    odd
}

/// El candidato `(item, distancia, profundidad)` más cerca del puntero; a la misma distancia
/// (redondeada a 1 punto), el más cercano a la vista.
fn best(cands: Vec<(Item, f32, f32)>) -> Option<Item> {
    let mut out: Option<(Item, f32, f32)> = None;
    for (item, d, depth) in cands {
        let d = d.round();
        let better = match out {
            None => true,
            Some((_, bd, bdepth)) => d < bd || (d == bd && depth > bdepth),
        };
        if better {
            out = Some((item, d, depth));
        }
    }
    out.map(|(item, _, _)| item)
}

/// Las caras de `obj` con al menos 3 vértices válidos: índice y vértices.
fn faces_of(obj: &Object3D) -> impl Iterator<Item = (usize, Vec<u32>)> + '_ {
    let n = obj.mesh.vertices.len() as u32;
    obj.mesh.faces.iter().enumerate().filter_map(move |(i, f)| {
        let vs: Vec<u32> = f.verts.iter().copied().filter(|&v| v < n).collect();
        (vs.len() >= 3).then_some((i, vs))
    })
}

/// Lo que hay bajo `at` (pantalla) en la vista de `map`, según el modo: un vértice o una
/// arista a [`PICK_RADIUS`] o menos; una cara si `at` cae dentro (o cerca de su punto central);
/// un objeto si `at` cae dentro de alguna de sus caras.
pub fn pick(model: &Model3D, map: &ViewMap, mode: SelectMode, at: (f32, f32)) -> Option<Item> {
    let mut cands = Vec::new();
    for (o, obj) in model
        .objects
        .iter()
        .enumerate()
        .filter(|(o, _)| pickable(model, *o))
    {
        let pts = world_points(model, o);
        let scr: Vec<(f32, f32)> = pts.iter().map(|&p| map.to_screen(p)).collect();
        let depth = |i: u32| map.view.depth(pts[i as usize]);
        match mode {
            SelectMode::Vertex => {
                for (i, &s) in scr.iter().enumerate() {
                    let d = dist(s, at);
                    if d <= PICK_RADIUS {
                        cands.push((Item::Vertex(o, i as u32), d, depth(i as u32)));
                    }
                }
            }
            SelectMode::Edge => {
                for (a, b) in mesh_edges(&obj.mesh) {
                    let d = dist_to_segment(at, scr[a as usize], scr[b as usize]);
                    if d <= PICK_RADIUS {
                        cands.push((Item::Edge(o, a, b), d, (depth(a) + depth(b)) / 2.0));
                    }
                }
            }
            SelectMode::Face | SelectMode::Object => {
                for (fi, vs) in faces_of(obj) {
                    let poly: Vec<(f32, f32)> = vs.iter().map(|&v| scr[v as usize]).collect();
                    let k = poly.len() as f32;
                    let c = (
                        poly.iter().map(|p| p.0).sum::<f32>() / k,
                        poly.iter().map(|p| p.1).sum::<f32>() / k,
                    );
                    let cd = vs.iter().map(|&v| depth(v)).sum::<f32>() / k;
                    let d = if inside(&poly, at) {
                        0.0
                    } else if mode == SelectMode::Face && dist(c, at) <= PICK_RADIUS {
                        dist(c, at)
                    } else {
                        continue;
                    };
                    let item = match mode {
                        SelectMode::Face => Item::Face(o, fi),
                        _ => Item::Object(o),
                    };
                    cands.push((item, d, cd));
                }
            }
        }
    }
    best(cands)
}

/// Lo que cae en la caja de pantalla entre `a` y `b`: vértices dentro; aristas y caras con todos
/// sus vértices dentro; objetos con algún vértice dentro.
pub fn box_select(
    model: &Model3D,
    map: &ViewMap,
    mode: SelectMode,
    a: (f32, f32),
    b: (f32, f32),
) -> BTreeSet<Item> {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    let mut out = BTreeSet::new();
    for (o, obj) in model
        .objects
        .iter()
        .enumerate()
        .filter(|(o, _)| pickable(model, *o))
    {
        let inn: Vec<bool> = world_points(model, o)
            .iter()
            .map(|&p| {
                let s = map.to_screen(p);
                s.0 >= x0 && s.0 <= x1 && s.1 >= y0 && s.1 <= y1
            })
            .collect();
        let is_in = |v: u32| inn[v as usize];
        match mode {
            SelectMode::Object => {
                if inn.iter().any(|&i| i) {
                    out.insert(Item::Object(o));
                }
            }
            SelectMode::Vertex => out.extend(
                (0..inn.len() as u32)
                    .filter(|&v| is_in(v))
                    .map(|v| Item::Vertex(o, v)),
            ),
            SelectMode::Edge => out.extend(
                mesh_edges(&obj.mesh)
                    .into_iter()
                    .filter(|&(a, b)| is_in(a) && is_in(b))
                    .map(|(a, b)| Item::Edge(o, a, b)),
            ),
            SelectMode::Face => out.extend(
                faces_of(obj)
                    .filter(|(_, vs)| vs.iter().all(|&v| is_in(v)))
                    .map(|(i, _)| Item::Face(o, i)),
            ),
        }
    }
    out
}

fn bounds_of(points: impl Iterator<Item = P3>) -> Option<(P3, P3)> {
    let mut out: Option<(P3, P3)> = None;
    for p in points {
        let (lo, hi) = out.get_or_insert((p, p));
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    out
}

/// La caja (en el mundo) de los vértices involucrados en la selección.
pub fn bounds(model: &Model3D, sel: &Selection) -> Option<(P3, P3)> {
    let verts = sel.vertices(model);
    bounds_of(verts.iter().flat_map(|(&o, vs)| {
        let obj = &model.objects[o];
        vs.iter()
            .map(move |&v| object_to_world(model, o, p3(obj.mesh.vertices[v as usize])))
    }))
}

/// La caja (en el mundo) de todo lo que se ve del modelo (sin los objetos ocultos).
pub fn model_bounds(model: &Model3D) -> Option<(P3, P3)> {
    bounds_of(
        (0..model.objects.len())
            .filter(|&o| !model.object_hidden(o))
            .flat_map(|o| world_points(model, o)),
    )
}

pub fn center((lo, hi): (P3, P3)) -> P3 {
    [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ]
}

/// Cambia en `out` los vértices involucrados en `sel` (de `model`): `f` lleva su posición en el
/// mundo a la nueva. Siempre desde los de `model`, así el redondeo no se acumula en un arrastre.
pub(crate) fn edit_vertices(
    out: &mut Model3D,
    model: &Model3D,
    sel: &Selection,
    f: impl Fn(P3) -> P3,
) {
    for (o, vs) in sel.vertices(model) {
        // Lo bloqueado (él o un grupo suyo) no se edita.
        if model.object_locked(o) {
            continue;
        }
        for v in vs {
            let local = p3(model.objects[o].mesh.vertices[v as usize]);
            let w = f(object_to_world(model, o, local));
            out.objects[o].mesh.vertices[v as usize] = round3(object_to_local(model, o, w, local));
        }
    }
}

pub(crate) fn objects_of(sel: &Selection, model: &Model3D) -> Vec<usize> {
    let mut out: Vec<usize> = sel
        .items
        .iter()
        .map(|i| i.object())
        .filter(|&o| o < model.objects.len())
        .collect();
    out.dedup();
    out
}

/// Los objetos de la selección que se pueden mover, escalar o girar: los no bloqueados.
pub(crate) fn movable_objects(sel: &Selection, model: &Model3D) -> Vec<usize> {
    objects_of(sel, model)
        .into_iter()
        .filter(|&o| !model.object_locked(o))
        .collect()
}

fn comp(v: &mut Vec3i, axis: usize) -> &mut i32 {
    match axis {
        0 => &mut v.x,
        1 => &mut v.y,
        _ => &mut v.z,
    }
}

/// `model` con lo elegido corrido `d` (en el mundo): en modo Objeto, la posición de los objetos;
/// si no, los vértices involucrados. Lo bloqueado no se mueve.
pub fn translate(model: &Model3D, sel: &Selection, d: P3) -> Model3D {
    let mut out = model.clone();
    if sel.mode == SelectMode::Object {
        for o in movable_objects(sel, model) {
            // La posición vive en el espacio de sus grupos: se corre en el mundo.
            let origin = parent_to_world(model, o, p3(model.objects[o].transform.position));
            let moved = [origin[0] + d[0], origin[1] + d[1], origin[2] + d[2]];
            out.objects[o].transform.position = round3(parent_to_local(model, o, moved));
        }
        return out;
    }
    edit_vertices(&mut out, model, sel, |w| {
        [w[0] + d[0], w[1] + d[1], w[2] + d[2]]
    });
    out
}

/// El eje local (0 x, 1 y, 2 z) que, ya girado, apunta más hacia el eje `axis` del mundo.
fn local_axis(t: &Transform3D, axis: usize) -> usize {
    let r = Transform3D {
        rotation: t.rotation,
        ..Transform3D::default()
    };
    let along = |k: usize| {
        let mut u = [0.0; 3];
        u[k] = 1.0;
        to_world(&r, u)[axis].abs()
    };
    (0..3)
        .max_by(|&a, &b| along(a).total_cmp(&along(b)))
        .unwrap_or(axis)
}

/// Una escala (en milésimas) por `f`: nunca 0 (queda en ±1 con el signo que toca).
fn scaled(s: i32, f: f32) -> i32 {
    match (s as f32 * f).round() as i32 {
        0 if (s < 0) != (f < 0.0) => -1,
        0 => 1,
        v => v,
    }
}

/// `model` con lo elegido escalado `f` (por eje del mundo) desde `anchor`. En modo Objeto cambia
/// la escala del eje local más alineado con cada eje del mundo (exacto sin rotación; con la del
/// objeto, no la de sus grupos: aproximado si están girados) y corrige la posición (en el mundo)
/// para que el ancla no se mueva; si no, escala las posiciones de los vértices. Lo bloqueado no
/// cambia.
pub fn scale(model: &Model3D, sel: &Selection, anchor: P3, f: P3) -> Model3D {
    let mut out = model.clone();
    if sel.mode == SelectMode::Object {
        for o in movable_objects(sel, model) {
            let origin = parent_to_world(model, o, p3(model.objects[o].transform.position));
            let t = &mut out.objects[o].transform;
            for (axis, &fa) in f.iter().enumerate() {
                if fa == 1.0 {
                    continue;
                }
                let k = local_axis(t, axis);
                let s = comp(&mut t.scale, k);
                *s = scaled(*s, fa);
            }
            let moved: P3 = std::array::from_fn(|i| anchor[i] + (origin[i] - anchor[i]) * f[i]);
            t.position = round3(parent_to_local(model, o, moved));
        }
        return out;
    }
    edit_vertices(&mut out, model, sel, |w| {
        std::array::from_fn(|i| anchor[i] + (w[i] - anchor[i]) * f[i])
    });
    out
}

/// `v` en el múltiplo de `grid` más cercano.
pub fn snap(v: f32, grid: i32) -> f32 {
    let g = grid.max(1) as f32;
    (v / g).round() * g
}

/// El desplazamiento `d` corregido para que `grab + d` caiga en la cuadrícula `grid` (en 1/16)
/// en los dos ejes de `view`; sin cuadrícula (`None`, con Ctrl), en pasos de 1/16. El eje de
/// profundidad no se mueve.
pub fn snapped_delta(view: OrthoView, grab: P3, d: P3, grid: Option<i32>) -> P3 {
    let g = grid.unwrap_or(1);
    let (a, b) = view.axes();
    let mut out = [0.0; 3];
    for i in [a, b] {
        out[i] = snap(grab[i] + d[i], g) - grab[i];
    }
    out
}

/// Un asa de la caja de la selección en el plano de la vista: en `u` (a la derecha) y en `v`
/// (arriba), −1 el lado menor, 1 el mayor y 0 ninguno.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle {
    pub u: i8,
    pub v: i8,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle { u: -1, v: -1 },
        Handle { u: 0, v: -1 },
        Handle { u: 1, v: -1 },
        Handle { u: -1, v: 0 },
        Handle { u: 1, v: 0 },
        Handle { u: -1, v: 1 },
        Handle { u: 0, v: 1 },
        Handle { u: 1, v: 1 },
    ];
}

/// La caja `b` (del mundo) en el plano de la vista: `((min_u, min_v), (max_u, max_v))`.
pub fn plane_box(view: OrthoView, (lo, hi): (P3, P3)) -> ((f32, f32), (f32, f32)) {
    let (a, c) = (view.plane(lo), view.plane(hi));
    ((a.0.min(c.0), a.1.min(c.1)), (a.0.max(c.0), a.1.max(c.1)))
}

/// Arrastrar `handle` de la caja `b` hasta `to` (en el plano): el ancla (el lado o la esquina
/// opuesta, en el mundo) y el factor por eje del mundo. El borde arrastrado cae en la
/// cuadrícula (`None`: pasos de 1/16). Con `proportional` (Shift, en una esquina) los dos ejes
/// usan el factor que más cambió.
pub fn handle_scale(
    view: OrthoView,
    b: (P3, P3),
    handle: Handle,
    to: (f32, f32),
    grid: Option<i32>,
    proportional: bool,
) -> (P3, P3) {
    let ((u0, v0), (u1, v1)) = plane_box(view, b);
    let g = grid.unwrap_or(1);
    let side = |s: i8, lo: f32, hi: f32, at: f32| {
        let (edge, anchor) = if s > 0 { (hi, lo) } else { (lo, hi) };
        let f = if (edge - anchor).abs() < f32::EPSILON {
            1.0
        } else {
            (snap(at, g) - anchor) / (edge - anchor)
        };
        (anchor, f)
    };
    let mid = center(b);
    let (mu, mv) = view.plane(mid);
    let (au, fu) = if handle.u != 0 {
        side(handle.u, u0, u1, to.0)
    } else {
        (mu, 1.0)
    };
    let (av, fv) = if handle.v != 0 {
        side(handle.v, v0, v1, to.1)
    } else {
        (mv, 1.0)
    };
    let (fu, fv) = if proportional && handle.u != 0 && handle.v != 0 {
        let f = if (fu - 1.0).abs() >= (fv - 1.0).abs() {
            fu
        } else {
            fv
        };
        (f, f)
    } else {
        (fu, fv)
    };
    let (a, c) = view.axes();
    let mut f = [1.0; 3];
    f[a] = fu;
    f[c] = fv;
    (view.from_plane((au, av), mid), f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Primitive, primitive_mesh};
    use cs_core::{Model3D, Object3D, Transform3D, Vec3i};

    #[test]
    fn with_object_cambia_solo_el_objeto() {
        assert_eq!(Item::Object(3).with_object(1), Item::Object(1));
        assert_eq!(Item::Vertex(3, 7).with_object(1), Item::Vertex(1, 7));
        assert_eq!(Item::Edge(3, 1, 2).with_object(0), Item::Edge(0, 1, 2));
        assert_eq!(Item::Face(3, 5).with_object(2), Item::Face(2, 5));
    }

    pub(super) fn cube_at(position: Vec3i) -> Object3D {
        Object3D {
            name: "Cube".into(),
            mesh: primitive_mesh(&Primitive::Cube),
            transform: Transform3D {
                position,
                ..Transform3D::default()
            },
            double_sided: false,
            kind: Some(Primitive::Cube.kind()),
            hidden: false,
            locked: false,
        }
    }

    pub(super) fn one_cube() -> Model3D {
        Model3D {
            objects: vec![cube_at(Vec3i::default())],
            ..Model3D::default()
        }
    }

    fn close(a: P3, b: P3) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
    }

    #[test]
    fn las_vistas_proyectan_como_picocad() {
        let p = [1.0, 2.0, 3.0];
        assert_eq!(OrthoView::Front.plane(p), (1.0, 2.0));
        assert_eq!(OrthoView::Front.depth(p), 3.0);
        assert_eq!(OrthoView::Top.plane(p), (1.0, -3.0));
        assert_eq!(OrthoView::Top.depth(p), 2.0);
        assert_eq!(OrthoView::Side.plane(p), (-3.0, 2.0));
        assert_eq!(OrthoView::Side.depth(p), 1.0);
        for v in OrthoView::ALL {
            assert_eq!(v.from_plane(v.plane(p), p), p, "{v:?}");
        }
        assert_eq!(OrthoView::Top.world_delta((4.0, 2.0)), [4.0, 0.0, -2.0]);
        assert_eq!(OrthoView::Side.axes(), (2, 1));
    }

    #[test]
    fn to_local_deshace_to_world() {
        let t = Transform3D {
            position: Vec3i { x: 5, y: -3, z: 7 },
            rotation: Vec3i {
                x: 300,
                y: 450,
                z: -200,
            },
            scale: Vec3i {
                x: 2000,
                y: 500,
                z: -1000,
            },
        };
        let p = [3.0, -7.0, 11.0];
        assert!(close(to_local(&t, to_world(&t, p), [0.0; 3]), p));
        // 90° en Y: +X local apunta a -Z en el mundo.
        let yaw = Transform3D {
            rotation: Vec3i { x: 0, y: 900, z: 0 },
            ..Transform3D::default()
        };
        assert!(close(to_world(&yaw, [16.0, 0.0, 0.0]), [0.0, 0.0, -16.0]));
        // Con escala 0 en un eje, ese eje se queda con `keep`.
        let flat = Transform3D {
            scale: Vec3i {
                x: 1000,
                y: 0,
                z: 1000,
            },
            ..Transform3D::default()
        };
        assert_eq!(
            to_local(&flat, [1.0, 0.0, 2.0], [9.0, 9.0, 9.0]),
            [1.0, 9.0, 2.0]
        );
    }

    #[test]
    fn el_cubo_tiene_12_aristas() {
        let edges = mesh_edges(&primitive_mesh(&Primitive::Cube));
        assert_eq!(edges.len(), 12);
        assert!(edges.iter().all(|&(a, b)| a < b));
    }

    #[test]
    fn convertir_la_seleccion_entre_modos() {
        let m = one_cube();
        let obj = Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0)].into(),
        };
        assert_eq!(obj.converted(&m, SelectMode::Vertex).items.len(), 8);
        assert_eq!(obj.converted(&m, SelectMode::Edge).items.len(), 12);
        assert_eq!(obj.converted(&m, SelectMode::Face).items.len(), 6);
        // Los vértices de una cara dan esa cara (y sus 4 aristas).
        let face = &m.objects[0].mesh.faces[0];
        let verts = Selection {
            mode: SelectMode::Vertex,
            items: face.verts.iter().map(|&v| Item::Vertex(0, v)).collect(),
        };
        assert_eq!(
            verts.converted(&m, SelectMode::Face).items,
            [Item::Face(0, 0)].into()
        );
        assert_eq!(verts.converted(&m, SelectMode::Edge).items.len(), 4);
        assert_eq!(
            verts.converted(&m, SelectMode::Object).items,
            [Item::Object(0)].into()
        );
    }

    #[test]
    fn retain_valid_quita_lo_que_ya_no_existe() {
        let m = one_cube();
        let mut s = Selection {
            mode: SelectMode::Vertex,
            items: [Item::Vertex(0, 1), Item::Vertex(0, 99), Item::Vertex(3, 0)].into(),
        };
        s.retain_valid(&m);
        assert_eq!(s.items, [Item::Vertex(0, 1)].into());
        assert_eq!(s.first_object(), Some(0));
        s.toggle(Item::Vertex(0, 1));
        assert!(s.is_empty());
    }

    #[test]
    fn retain_valid_y_vertices_con_muchas_aristas() {
        let m = one_cube();
        let edges: Vec<Item> = mesh_edges(&m.objects[0].mesh)
            .into_iter()
            .map(|(a, b)| Item::Edge(0, a, b))
            .collect();
        assert_eq!(edges.len(), 12);
        let mut s = Selection {
            mode: SelectMode::Edge,
            items: edges.iter().copied().collect(),
        };
        // La diagonal de una cara no es arista; un objeto que no existe, tampoco.
        s.items.extend([Item::Edge(0, 0, 7), Item::Edge(2, 0, 1)]);
        assert_eq!(s.vertices(&m), [(0, (0..8).collect())].into());
        s.retain_valid(&m);
        assert_eq!(s.items, edges.into_iter().collect());
    }

    fn front_map() -> ViewMap {
        ViewMap {
            view: OrthoView::Front,
            center: [0.0; 3],
            scale: 2.0,
            origin: (100.0, 100.0),
        }
    }

    fn in_group(offset: i32) -> Model3D {
        let mut m = Model3D::default();
        m.objects.push(cube_at(Vec3i::default()));
        m.groups.push(cs_core::Group3D {
            name: "g".into(),
            transform: Transform3D {
                position: Vec3i {
                    x: offset,
                    y: 0,
                    z: 0,
                },
                ..Default::default()
            },
            hidden: false,
            locked: false,
            open: true,
        });
        m.tree = vec![cs_core::Node::Group {
            group: 0,
            children: vec![cs_core::Node::Object(0)],
        }];
        m
    }

    #[test]
    fn a_group_moves_its_objects_in_the_world() {
        let m = in_group(64);
        let pts = world_points(&m, 0);
        assert!(pts.iter().all(|p| p[0] >= 48.0 && p[0] <= 80.0), "{pts:?}");
        let w = object_to_world(&m, 0, [16.0, 0.0, 0.0]);
        assert_eq!(w, [80.0, 0.0, 0.0]);
        assert_eq!(object_to_local(&m, 0, w, [0.0; 3]), [16.0, 0.0, 0.0]);
    }

    #[test]
    fn moving_an_object_inside_a_turned_group_follows_the_world() {
        let mut m = in_group(0);
        m.groups[0].transform.rotation.y = 900; // 90°: el x del grupo es el -z del mundo
        let sel = Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0)].into(),
        };
        let out = translate(&m, &sel, [16.0, 0.0, 0.0]);
        let before = object_to_world(&m, 0, [0.0; 3]);
        let after = object_to_world(&out, 0, [0.0; 3]);
        assert!(
            (after[0] - before[0] - 16.0).abs() < 0.5,
            "{before:?} -> {after:?}"
        );
        assert!((after[2] - before[2]).abs() < 0.5);
    }

    #[test]
    fn hidden_and_locked_objects_are_not_picked() {
        let map = front_map();
        let mut m = in_group(0);
        let at = map.to_screen([0.0, 0.0, 16.0]);
        assert!(pick(&m, &map, SelectMode::Object, at).is_some());
        m.groups[0].locked = true;
        assert!(pick(&m, &map, SelectMode::Object, at).is_none());
        assert!(box_select(&m, &map, SelectMode::Object, (0.0, 0.0), (1e4, 1e4)).is_empty());
        m.groups[0].locked = false;
        m.objects[0].hidden = true;
        assert!(pick(&m, &map, SelectMode::Object, at).is_none());
        assert!(box_select(&m, &map, SelectMode::Object, (0.0, 0.0), (1e4, 1e4)).is_empty());
    }

    #[test]
    fn locked_objects_are_not_moved_scaled_or_rotated() {
        let mut m = one_cube();
        m.objects.push(cube_at(Vec3i { x: 64, y: 0, z: 0 }));
        m.normalize_tree();
        m.objects[1].locked = true;
        let objs = Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0), Item::Object(1)].into(),
        };
        let verts = objs.converted(&m, SelectMode::Vertex);
        for s in [&objs, &verts] {
            let moved = translate(&m, s, [16.0, 0.0, 0.0]);
            assert_ne!(moved.objects[0], m.objects[0], "{:?}", s.mode);
            assert_eq!(moved.objects[1], m.objects[1], "{:?}", s.mode);
            let scaled = scale(&m, s, [0.0; 3], [2.0, 1.0, 1.0]);
            assert_ne!(scaled.objects[0], m.objects[0], "{:?}", s.mode);
            assert_eq!(scaled.objects[1], m.objects[1], "{:?}", s.mode);
            let turned = crate::rotate_selection(&m, s, [0.0; 3], 1, 900);
            assert_ne!(turned.objects[0], m.objects[0], "{:?}", s.mode);
            assert_eq!(turned.objects[1], m.objects[1], "{:?}", s.mode);
        }
    }

    #[test]
    fn view_map_ida_y_vuelta() {
        let m = front_map();
        assert_eq!(m.to_screen([16.0, 16.0, 0.0]), (132.0, 68.0));
        assert_eq!(m.to_plane((132.0, 68.0)), (16.0, 16.0));
        assert_eq!(m.drag_delta((4.0, -2.0)), [2.0, 1.0, 0.0]);
    }

    #[test]
    fn pick_de_vertice_gana_el_mas_cercano_a_la_vista() {
        // Los vértices 3 (x+, y+, z−) y 7 (x+, y+, z+) caen en el mismo punto de Front.
        let m = one_cube();
        let map = front_map();
        assert_eq!(
            pick(&m, &map, SelectMode::Vertex, (131.0, 69.0)),
            Some(Item::Vertex(0, 7))
        );
        assert_eq!(pick(&m, &map, SelectMode::Vertex, (120.0, 69.0)), None);
    }

    #[test]
    fn pick_de_arista_cara_y_objeto() {
        let m = one_cube();
        let map = front_map();
        // El lado derecho (x = 16) a media altura: la arista de enfrente, 5 (x+, z+) – 7.
        assert_eq!(
            pick(&m, &map, SelectMode::Edge, (132.0, 100.0)),
            Some(Item::Edge(0, 5, 7))
        );
        // El centro: la cara de enfrente (la de z+).
        let Some(Item::Face(0, f)) = pick(&m, &map, SelectMode::Face, (100.0, 100.0)) else {
            panic!("no face");
        };
        let mesh = &m.objects[0].mesh;
        assert!(
            mesh.faces[f]
                .verts
                .iter()
                .all(|&v| mesh.vertices[v as usize].z > 0)
        );
        // Dos cubos alineados: gana el de enfrente.
        let mut two = one_cube();
        two.objects.insert(0, cube_at(Vec3i { x: 0, y: 0, z: -64 }));
        assert_eq!(
            pick(&two, &map, SelectMode::Object, (100.0, 100.0)),
            Some(Item::Object(1))
        );
        assert_eq!(pick(&two, &map, SelectMode::Object, (0.0, 0.0)), None);
    }

    #[test]
    fn caja_de_seleccion_por_modo() {
        let m = one_cube();
        let map = front_map();
        let corner = box_select(&m, &map, SelectMode::Vertex, (120.0, 60.0), (140.0, 80.0));
        assert_eq!(corner, [Item::Vertex(0, 3), Item::Vertex(0, 7)].into());
        let all = box_select(&m, &map, SelectMode::Face, (0.0, 0.0), (200.0, 200.0));
        assert_eq!(all.len(), 6);
        let obj = box_select(&m, &map, SelectMode::Object, (140.0, 80.0), (120.0, 60.0));
        assert_eq!(obj, [Item::Object(0)].into());
        let edge = box_select(&m, &map, SelectMode::Edge, (120.0, 60.0), (140.0, 80.0));
        assert_eq!(edge, [Item::Edge(0, 3, 7)].into(), "the edge seen end-on");
    }

    #[test]
    fn limites_de_la_seleccion() {
        let m = one_cube();
        let s = Selection {
            mode: SelectMode::Vertex,
            items: [Item::Vertex(0, 0), Item::Vertex(0, 7)].into(),
        };
        let b = bounds(&m, &s).unwrap();
        assert_eq!(b, ([-16.0; 3], [16.0; 3]));
        assert_eq!(center(b), [0.0; 3]);
        assert_eq!(bounds(&m, &Selection::default()), None);
        assert_eq!(model_bounds(&m), Some(([-16.0; 3], [16.0; 3])));
    }

    fn sel(mode: SelectMode, items: &[Item]) -> Selection {
        Selection {
            mode,
            items: items.iter().copied().collect(),
        }
    }

    #[test]
    fn mover_un_objeto_y_un_vertice_de_un_objeto_girado() {
        let m = one_cube();
        let moved = translate(
            &m,
            &sel(SelectMode::Object, &[Item::Object(0)]),
            [4.0, 0.0, -2.0],
        );
        assert_eq!(
            moved.objects[0].transform.position,
            Vec3i { x: 4, y: 0, z: -2 }
        );
        let mut turned = one_cube();
        turned.objects[0].transform.rotation.y = 900;
        let before = world_points(&turned, 0)[1];
        let after = translate(
            &turned,
            &sel(SelectMode::Vertex, &[Item::Vertex(0, 1)]),
            [4.0, 0.0, 0.0],
        );
        let now = world_points(&after, 0)[1];
        assert!(
            close(now, [before[0] + 4.0, before[1], before[2]]),
            "{now:?}"
        );
        // Los demás no se mueven.
        assert_eq!(
            after.objects[0].mesh.vertices[0],
            turned.objects[0].mesh.vertices[0]
        );
    }

    #[test]
    fn snapping_a_la_cuadricula() {
        assert_eq!(snap(5.0, 4), 4.0);
        assert_eq!(snap(6.0, 4), 8.0);
        assert_eq!(snap(-6.0, 4), -8.0);
        let d = snapped_delta(OrthoView::Front, [3.0, 5.0, 7.0], [10.0, 0.0, 0.0], Some(4));
        assert_eq!(d, [9.0, -1.0, 0.0]);
        // Sin cuadrícula (Ctrl): pasos de 1/16.
        let d = snapped_delta(OrthoView::Front, [3.0, 5.0, 7.0], [2.4, 0.0, 0.0], None);
        assert_eq!(d, [2.0, 0.0, 0.0]);
    }

    #[test]
    fn escalar_vertices_con_ancla() {
        let m = one_cube();
        let all = sel(SelectMode::Object, &[Item::Object(0)]).converted(&m, SelectMode::Vertex);
        let out = scale(&m, &all, [-16.0, -16.0, -16.0], [2.0, 1.0, 1.0]);
        for (v, w) in out.objects[0]
            .mesh
            .vertices
            .iter()
            .zip(&m.objects[0].mesh.vertices)
        {
            assert_eq!(v.x, if w.x > 0 { 48 } else { -16 });
            assert_eq!((v.y, v.z), (w.y, w.z));
        }
        // Factor 0: quedan planos.
        let flat = scale(&m, &all, [0.0; 3], [1.0, 0.0, 1.0]);
        assert!(flat.objects[0].mesh.vertices.iter().all(|v| v.y == 0));
    }

    #[test]
    fn escalar_objetos_cambia_su_escala_y_su_posicion() {
        let m = one_cube();
        let s = sel(SelectMode::Object, &[Item::Object(0)]);
        let out = scale(&m, &s, [-16.0, 0.0, 0.0], [2.0, 1.0, 1.0]);
        let t = out.objects[0].transform;
        assert_eq!(
            t.scale,
            Vec3i {
                x: 2000,
                y: 1000,
                z: 1000
            }
        );
        assert_eq!(t.position.x, 16);
        // Girado 90° en Y, el eje X del mundo es el Z local.
        let mut turned = m.clone();
        turned.objects[0].transform.rotation.y = 900;
        let out = scale(&turned, &s, [0.0; 3], [2.0, 1.0, 1.0]);
        assert_eq!(out.objects[0].transform.scale.z, 2000);
        // Nunca 0; negativo voltea.
        let out = scale(&m, &s, [0.0; 3], [0.0, -1.0, 1.0]);
        assert_eq!(out.objects[0].transform.scale.x, 1);
        assert_eq!(out.objects[0].transform.scale.y, -1000);
    }

    #[test]
    fn asas_de_lado_y_de_esquina() {
        let b = ([-16.0; 3], [16.0; 3]);
        let (anchor, f) = handle_scale(
            OrthoView::Front,
            b,
            Handle { u: 1, v: 0 },
            (47.0, 0.0),
            Some(4),
            false,
        );
        assert_eq!(anchor[0], -16.0);
        assert_eq!(f, [2.0, 1.0, 1.0]);
        let (_, f) = handle_scale(
            OrthoView::Front,
            b,
            Handle { u: 1, v: 1 },
            (48.0, 16.0),
            Some(4),
            true,
        );
        assert_eq!(f, [2.0, 2.0, 1.0]);
        // En Top, v es -Z: el asa de arriba escala Z con el ancla en z = +16.
        let (anchor, f) = handle_scale(
            OrthoView::Top,
            b,
            Handle { u: 0, v: 1 },
            (0.0, 48.0),
            Some(4),
            false,
        );
        assert_eq!(anchor[2], 16.0);
        assert_eq!(f, [1.0, 1.0, 2.0]);
        assert_eq!(
            plane_box(OrthoView::Side, b),
            ((-16.0, -16.0), (16.0, 16.0))
        );
    }
}
