//! Cambios de topología del modo Edit (picoCAD 2 / Blender): extruir y dividir caras, unir
//! vértices y borrar lo elegido. Todo en el espacio local de cada objeto, en 1/16 de unidad; las
//! caras siguen con 3 o 4 vértices, antihorarias vistas desde afuera (como las primitivas y el
//! rasterizador), y las UVs en pixeles enteros de la textura.

use std::collections::{BTreeMap, BTreeSet};

use cs_core::{Face, Mesh, Model3D, Vec3i};

use crate::{Item, SelectMode, Selection, objects_of, p3, round3};

type Uv = (i32, i32);

/// Un vértice de una cara con su UV.
type Corner = (u32, Uv);

fn uv_at(f: &Face, i: usize) -> Uv {
    f.uvs.get(i).copied().unwrap_or((0, 0))
}

fn key(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

/// Si la cara tiene al menos 3 vértices y todos existen.
fn valid(mesh: &Mesh, f: &Face) -> bool {
    f.verts.len() >= 3 && f.verts.iter().all(|&v| (v as usize) < mesh.vertices.len())
}

fn corners(f: &Face) -> Vec<Corner> {
    f.verts
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, uv_at(f, i)))
        .collect()
}

fn face_of(cs: &[Corner]) -> Face {
    Face {
        verts: cs.iter().map(|c| c.0).collect(),
        uvs: cs.iter().map(|c| c.1).collect(),
    }
}

/// Las caras elegidas por objeto: las de la selección en modo Cara; en los otros modos, las que
/// tienen todos sus vértices entre los involucrados (como al cambiar de modo). Solo las que
/// existen.
fn chosen_faces(model: &Model3D, sel: &Selection) -> BTreeMap<usize, BTreeSet<usize>> {
    let faces = if sel.mode == SelectMode::Face {
        sel.clone()
    } else {
        sel.converted(model, SelectMode::Face)
    };
    let mut out: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for item in faces.items {
        if let Item::Face(o, f) = item
            && model
                .objects
                .get(o)
                .is_some_and(|obj| f < obj.mesh.faces.len())
        {
            out.entry(o).or_default().insert(f);
        }
    }
    out
}

/// Quita los vértices que ninguna cara usa y corre los índices. Devuelve el índice nuevo de cada
/// vértice viejo (`None` si se fue). Los índices fuera de rango siguen fuera de rango.
fn compact(mesh: &mut Mesh) -> Vec<Option<u32>> {
    let n = mesh.vertices.len();
    let mut used = vec![false; n];
    for f in &mesh.faces {
        for &v in &f.verts {
            if let Some(u) = used.get_mut(v as usize) {
                *u = true;
            }
        }
    }
    let mut map = vec![None; n];
    let mut vertices = Vec::new();
    for (i, &u) in used.iter().enumerate() {
        if u {
            map[i] = Some(vertices.len() as u32);
            vertices.push(mesh.vertices[i]);
        }
    }
    for f in &mut mesh.faces {
        for v in &mut f.verts {
            *v = map.get(*v as usize).copied().flatten().unwrap_or(u32::MAX);
        }
    }
    mesh.vertices = vertices;
    map
}

/// El paso de un pixel, perpendicular a la arista `a`→`b` de la textura, hacia `inner` (el
/// centro de las UVs de la cara). Redondeado por eje: en una arista derecha es un pixel justo
/// hacia adentro; en una diagonal, uno en cada eje.
fn inward_step(a: Uv, b: Uv, inner: (f32, f32)) -> Uv {
    let mid = ((a.0 + b.0) as f32 / 2.0, (a.1 + b.1) as f32 / 2.0);
    let to = (inner.0 - mid.0, inner.1 - mid.1);
    let mut n = (-(b.1 - a.1) as f32, (b.0 - a.0) as f32);
    if n == (0.0, 0.0) {
        n = to;
    } else if n.0 * to.0 + n.1 * to.1 < 0.0 {
        n = (-n.0, -n.1);
    }
    let len = (n.0 * n.0 + n.1 * n.1).sqrt();
    if len == 0.0 {
        return (0, 0);
    }
    ((n.0 / len).round() as i32, (n.1 / len).round() as i32)
}

/// `model` con las caras elegidas extruidas, y la selección nueva (esas mismas caras, en modo
/// Cara). Por objeto: los vértices de la región se duplican y las caras elegidas pasan a los
/// nuevos (en el mismo lugar: luego se mueven); por cada arista del borde de la región (la que
/// usa una sola cara elegida) se agrega un lado `[a, b, b', a']`, con `a`→`b` en el sentido de
/// la cara, así que mira hacia afuera como ella. Los lados van al final de las caras.
///
/// UVs de un lado: la tira de un pixel de la cara de la que sale, justo adentro de su arista,
/// estirada a todo el lado. La fila de `b'`/`a'` (la que sigue a la cara extruida) usa las UVs
/// de la arista; la de `a`/`b`, las mismas corridas un pixel hacia adentro de la cara (ver
/// [`inward_step`]). Con el muestreo *nearest* se ve el borde de la textura de la cara.
pub fn extrude_faces(model: &Model3D, sel: &Selection) -> (Model3D, Selection) {
    let mut out = model.clone();
    let mut items = BTreeSet::new();
    for (o, faces) in chosen_faces(model, sel) {
        let mesh = &mut out.objects[o].mesh;
        let faces: Vec<usize> = faces
            .into_iter()
            .filter(|&f| valid(mesh, &mesh.faces[f]))
            .collect();
        if faces.is_empty() {
            continue;
        }
        // Cuántas caras elegidas usan cada arista; las de 1 son el borde de la región.
        let mut uses: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        let mut dup: BTreeMap<u32, u32> = BTreeMap::new();
        for &f in &faces {
            let vs = &mesh.faces[f].verts;
            for i in 0..vs.len() {
                let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
                if a != b {
                    *uses.entry(key(a, b)).or_insert(0) += 1;
                }
                dup.entry(a).or_insert_with(|| {
                    mesh.vertices.push(mesh.vertices[a as usize]);
                    mesh.vertices.len() as u32 - 1
                });
            }
        }
        let mut sides = Vec::new();
        for &f in &faces {
            let cs = corners(&mesh.faces[f]);
            let k = cs.len() as f32;
            let inner = (
                cs.iter().map(|c| c.1.0 as f32).sum::<f32>() / k,
                cs.iter().map(|c| c.1.1 as f32).sum::<f32>() / k,
            );
            for i in 0..cs.len() {
                let ((a, ua), (b, ub)) = (cs[i], cs[(i + 1) % cs.len()]);
                if a == b || uses[&key(a, b)] != 1 {
                    continue;
                }
                let s = inward_step(ua, ub, inner);
                sides.push(face_of(&[
                    (a, (ua.0 + s.0, ua.1 + s.1)),
                    (b, (ub.0 + s.0, ub.1 + s.1)),
                    (dup[&b], ub),
                    (dup[&a], ua),
                ]));
            }
        }
        for &f in &faces {
            for v in &mut mesh.faces[f].verts {
                *v = dup[v];
            }
        }
        mesh.faces.extend(sides);
        // Si la región era toda la malla, los vértices viejos quedan sueltos.
        compact(mesh);
        items.extend(faces.iter().map(|&f| Item::Face(o, f)));
    }
    let sel = Selection {
        mode: SelectMode::Face,
        items,
    };
    (out, sel)
}

fn mid_uv(a: Uv, b: Uv) -> Uv {
    (
        ((a.0 + b.0) as f32 / 2.0).round() as i32,
        ((a.1 + b.1) as f32 / 2.0).round() as i32,
    )
}

/// Agrega el vértice del centro de `cs` (posición y UV promedio, redondeados) y lo devuelve.
fn center_corner(cs: &[Corner], vertices: &mut Vec<Vec3i>) -> Corner {
    let k = cs.len() as f32;
    let mut p = [0.0; 3];
    let mut uv = (0.0, 0.0);
    for &(v, (u, w)) in cs {
        let q = p3(vertices[v as usize]);
        for i in 0..3 {
            p[i] += q[i] / k;
        }
        uv = (uv.0 + u as f32 / k, uv.1 + w as f32 / k);
    }
    vertices.push(round3(p));
    (
        vertices.len() as u32 - 1,
        (uv.0.round() as i32, uv.1.round() as i32),
    )
}

/// Los pedazos de la cara de esquinas `cs` cuando las aristas con `m[i]` (la arista `i` va de
/// `cs[i]` a `cs[i + 1]`) ganan su vértice de la mitad. Todos con 3 o 4 vértices y en el mismo
/// sentido que la cara:
///
/// - triángulo: con las 3 partidas, 4 triángulos (las esquinas y el del medio); con 1 o 2, un
///   abanico desde la primera mitad (2 o 3 triángulos);
/// - cuadrilátero: con las 4, 4 cuadriláteros alrededor de un centro nuevo; con dos opuestas,
///   se corta en 2 cuadriláteros por sus mitades (y cada uno sigue con lo que le quede: así se
///   resuelven las de 3); con 1, 1 triángulo (la mitad, la esquina siguiente y la de después) +
///   1 cuadrilátero; con 2 vecinas, el triángulo de la esquina entre ellas, otro triángulo y 1
///   cuadrilátero;
/// - más de 4 vértices (no debería haber): un abanico desde la primera mitad.
fn pieces(cs: &[Corner], m: &[Option<Corner>], vertices: &mut Vec<Vec3i>) -> Vec<Vec<Corner>> {
    let n = cs.len();
    let k = m.iter().filter(|x| x.is_some()).count();
    if k == 0 {
        return vec![cs.to_vec()];
    }
    let c = |i: usize| cs[i % n];
    let mid = |i: usize| m[i % n].expect("arista partida");
    let split = |i: usize| m[i % n].is_some();
    if n == 3 && k == 3 {
        return vec![
            vec![c(0), mid(0), mid(2)],
            vec![mid(0), c(1), mid(1)],
            vec![mid(2), mid(1), c(2)],
            vec![mid(0), mid(1), mid(2)],
        ];
    }
    if n == 4 {
        if k == 4 {
            let ce = center_corner(cs, vertices);
            return vec![
                vec![c(0), mid(0), ce, mid(3)],
                vec![mid(0), c(1), mid(1), ce],
                vec![ce, mid(1), c(2), mid(2)],
                vec![mid(3), ce, mid(2), c(3)],
            ];
        }
        if let Some(e) = (0..2).find(|&e| split(e) && split(e + 2)) {
            let mut out = pieces(
                &[c(e), mid(e), mid(e + 2), c(e + 3)],
                &[None, None, None, m[(e + 3) % 4]],
                vertices,
            );
            out.extend(pieces(
                &[mid(e), c(e + 1), c(e + 2), mid(e + 2)],
                &[None, m[(e + 1) % 4], None, None],
                vertices,
            ));
            return out;
        }
        if k == 1 {
            let e = (0..4).find(|&e| split(e)).expect("una partida");
            return vec![
                vec![mid(e), c(e + 1), c(e + 2)],
                vec![mid(e), c(e + 2), c(e + 3), c(e)],
            ];
        }
        // Dos vecinas (con 3 siempre hay dos opuestas).
        let e = (0..4).find(|&e| split(e) && split(e + 1)).expect("vecinas");
        return vec![
            vec![mid(e), c(e + 1), mid(e + 1)],
            vec![mid(e + 1), c(e + 2), c(e + 3)],
            vec![c(e), mid(e), mid(e + 1), c(e + 3)],
        ];
    }
    // Abanico desde la primera mitad.
    let mut poly = Vec::new();
    let mut start = None;
    for (&corner, &half) in cs.iter().zip(m) {
        poly.push(corner);
        if let Some(x) = half {
            start.get_or_insert(poly.len());
            poly.push(x);
        }
    }
    poly.rotate_left(start.unwrap_or(0));
    (1..poly.len() - 1)
        .map(|i| vec![poly[0], poly[i], poly[i + 1]])
        .collect()
}

/// `model` con las caras elegidas partidas en 4, y la selección nueva (los pedazos, en modo
/// Cara). Un cuadrilátero se parte por las mitades de sus aristas y su centro; un triángulo, por
/// las mitades. Las mitades se comparten entre las caras de la arista: las vecinas no elegidas
/// que la usan se re-arman con ellas (ver [`pieces`]) para que no queden huecos. Las caras de
/// más de 4 vértices no se parten.
///
/// Las posiciones nuevas (mitades y centros) se redondean al 1/16 más cercano; las UVs, al pixel
/// más cercano (`.5` lejos del cero). En una arista de 1/16 la mitad caería sobre una punta: esa
/// arista no se parte (las caras que la usan se re-arman con las demás mitades). Cada cara deja
/// sus pedazos en su lugar de la lista: los índices de las caras de después se corren.
pub fn subdivide_faces(model: &Model3D, sel: &Selection) -> (Model3D, Selection) {
    let mut out = model.clone();
    let mut items = BTreeSet::new();
    for (o, chosen) in chosen_faces(model, sel) {
        let mesh = &mut out.objects[o].mesh;
        let chosen: BTreeSet<usize> = chosen
            .into_iter()
            .filter(|&f| {
                let face = &mesh.faces[f];
                valid(mesh, face) && face.verts.len() <= 4
            })
            .collect();
        if chosen.is_empty() {
            continue;
        }
        let mut mids: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut seen: BTreeSet<(u32, u32)> = BTreeSet::new();
        for &f in &chosen {
            let vs = &mesh.faces[f].verts;
            for i in 0..vs.len() {
                let (a, b) = (vs[i], vs[(i + 1) % vs.len()]);
                if a != b && !seen.contains(&key(a, b)) {
                    seen.insert(key(a, b));
                    let (va, vb) = (mesh.vertices[a as usize], mesh.vertices[b as usize]);
                    let (pa, pb) = (p3(va), p3(vb));
                    let p = round3([
                        (pa[0] + pb[0]) / 2.0,
                        (pa[1] + pb[1]) / 2.0,
                        (pa[2] + pb[2]) / 2.0,
                    ]);
                    // Una arista tan corta que su mitad cae sobre una punta no se parte (sería
                    // una cara sin área); las caras que la usan siguen sin partirla.
                    if p != va && p != vb {
                        mids.insert(key(a, b), mesh.vertices.len() as u32);
                        mesh.vertices.push(p);
                    }
                }
            }
        }
        let old = std::mem::take(&mut mesh.faces);
        let mut faces = Vec::with_capacity(old.len() + chosen.len() * 4);
        for (i, face) in old.into_iter().enumerate() {
            if !valid(mesh, &face) {
                faces.push(face);
                continue;
            }
            let cs = corners(&face);
            let m: Vec<Option<Corner>> = (0..cs.len())
                .map(|j| {
                    let ((a, ua), (b, ub)) = (cs[j], cs[(j + 1) % cs.len()]);
                    mids.get(&key(a, b)).map(|&v| (v, mid_uv(ua, ub)))
                })
                .collect();
            let start = faces.len();
            faces.extend(
                pieces(&cs, &m, &mut mesh.vertices)
                    .iter()
                    .map(|p| face_of(p)),
            );
            if chosen.contains(&i) {
                items.extend((start..faces.len()).map(|f| Item::Face(o, f)));
            }
        }
        mesh.faces = faces;
    }
    let sel = Selection {
        mode: SelectMode::Face,
        items,
    };
    (out, sel)
}

/// `model` con los vértices involucrados de cada objeto unidos en uno (no se une entre objetos),
/// y la selección nueva (esos vértices, en modo Vértice). El que queda es el de índice menor, en
/// el centro de la caja de todos (local, redondeado a 1/16). Las caras pierden los vértices
/// repetidos (queda la primera vez, con su UV); las que quedan con menos de 3 se borran, y luego
/// los vértices sin caras (los índices se compactan).
pub fn merge_selection(model: &Model3D, sel: &Selection) -> (Model3D, Selection) {
    let mut out = model.clone();
    let mut items = BTreeSet::new();
    for (o, vs) in sel.vertices(model) {
        let Some(&target) = vs.first() else {
            continue;
        };
        let mesh = &mut out.objects[o].mesh;
        let mut lo = p3(mesh.vertices[target as usize]);
        let mut hi = lo;
        for &v in &vs {
            let p = p3(mesh.vertices[v as usize]);
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        mesh.vertices[target as usize] = round3(crate::center((lo, hi)));
        for f in &mut mesh.faces {
            let mut seen = BTreeSet::new();
            let kept: Vec<Corner> = corners(f)
                .into_iter()
                .map(|(v, uv)| (if vs.contains(&v) { target } else { v }, uv))
                .filter(|&(v, _)| seen.insert(v))
                .collect();
            *f = face_of(&kept);
        }
        mesh.faces.retain(|f| f.verts.len() >= 3);
        if let Some(v) = compact(mesh)[target as usize] {
            items.insert(Item::Vertex(o, v));
        }
    }
    let sel = Selection {
        mode: SelectMode::Vertex,
        items,
    };
    (out, sel)
}

/// Lo que se borra de un objeto.
#[derive(Default)]
struct Gone {
    verts: BTreeSet<u32>,
    edges: BTreeSet<(u32, u32)>,
    faces: BTreeSet<usize>,
}

impl Gone {
    fn takes(&self, i: usize, f: &Face) -> bool {
        let vs = &f.verts;
        self.faces.contains(&i)
            || vs.iter().any(|v| self.verts.contains(v))
            || (0..vs.len()).any(|j| self.edges.contains(&key(vs[j], vs[(j + 1) % vs.len()])))
    }
}

/// `model` sin lo elegido, y la selección vacía (en el mismo modo). Vértices: se van con las
/// caras que los usan; aristas: se van las caras que las usan; caras: se van esas. Luego los
/// objetos tocados pierden los vértices sin caras (los índices se compactan). En modo Objeto se
/// quitan los objetos.
pub fn delete_selection(model: &Model3D, sel: &Selection) -> (Model3D, Selection) {
    let mut out = model.clone();
    let empty = Selection {
        mode: sel.mode,
        items: BTreeSet::new(),
    };
    if sel.mode == SelectMode::Object {
        // De mayor a menor y sin repetir (los índices de después se corren al quitar).
        let mut objects = objects_of(sel, model);
        objects.sort_unstable();
        objects.dedup();
        for o in objects.into_iter().rev() {
            out.objects.remove(o);
        }
        return (out, empty);
    }
    let mut gone: BTreeMap<usize, Gone> = BTreeMap::new();
    for &item in &sel.items {
        if item.object() >= model.objects.len() {
            continue;
        }
        let g = gone.entry(item.object()).or_default();
        match item {
            Item::Vertex(_, v) => {
                g.verts.insert(v);
            }
            Item::Edge(_, a, b) => {
                g.edges.insert(key(a, b));
            }
            Item::Face(_, f) => {
                g.faces.insert(f);
            }
            Item::Object(_) => {}
        }
    }
    for (o, g) in gone {
        let mesh = &mut out.objects[o].mesh;
        let old = std::mem::take(&mut mesh.faces);
        mesh.faces = old
            .into_iter()
            .enumerate()
            .filter(|(i, f)| !g.takes(*i, f))
            .map(|(_, f)| f)
            .collect();
        compact(mesh);
    }
    (out, empty)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        Item, P3, Primitive, RenderInput, SelectMode, primitive_mesh, render_model, translate,
    };
    use cs_core::{CameraPreset, FIXED, Light3D, Mesh, Object3D, Transform3D, Vec3i};

    // Caras del cubo: 0 frente (+Z), 1 derecha (+X), 2 atrás, 3 izquierda, 4 arriba, 5 abajo.
    const FRONT: usize = 0;
    const TOP: usize = 4;

    fn cube_obj(position: Vec3i) -> Object3D {
        Object3D {
            name: "Cube".into(),
            mesh: primitive_mesh(&Primitive::Cube),
            transform: Transform3D {
                position,
                ..Transform3D::default()
            },
            double_sided: false,
            kind: Some(Primitive::Cube.kind()),
        }
    }

    fn model_of(objects: Vec<Object3D>) -> Model3D {
        Model3D {
            objects,
            camera: CameraPreset::Iso.camera(4 * FIXED, Vec3i::default()),
            light: Light3D {
                enabled: false,
                ..Light3D::default()
            },
            ..Model3D::default()
        }
    }

    fn cube() -> Model3D {
        model_of(vec![cube_obj(Vec3i::default())])
    }

    fn sel(mode: SelectMode, items: impl IntoIterator<Item = Item>) -> Selection {
        Selection {
            mode,
            items: items.into_iter().collect(),
        }
    }

    fn faces_sel(o: usize, faces: &[usize]) -> Selection {
        sel(SelectMode::Face, faces.iter().map(|&f| Item::Face(o, f)))
    }

    /// Cuántas veces usan las caras cada arista dirigida.
    fn directed(mesh: &Mesh) -> BTreeMap<(u32, u32), usize> {
        let mut out = BTreeMap::new();
        for f in &mesh.faces {
            for i in 0..f.verts.len() {
                let (a, b) = (f.verts[i], f.verts[(i + 1) % f.verts.len()]);
                *out.entry((a, b)).or_insert(0) += 1;
            }
        }
        out
    }

    /// "Sin huecos": cada arista la usan 1 (borde) o 2 caras, nunca más; con `closed`, todas en
    /// exactamente 2, una en cada sentido (el giro de las caras es consistente). Además: índices
    /// válidos, 3 o 4 vértices por cara, UV por vértice y sin vértices sueltos.
    fn closed_edges_ok(mesh: &Mesh, closed: bool) -> Result<(), String> {
        let n = mesh.vertices.len() as u32;
        let mut used = vec![false; n as usize];
        for (i, f) in mesh.faces.iter().enumerate() {
            if !(3..=4).contains(&f.verts.len()) {
                return Err(format!("la cara {i} tiene {} vértices", f.verts.len()));
            }
            if f.uvs.len() != f.verts.len() {
                return Err(format!("la cara {i} no tiene una UV por vértice"));
            }
            for &v in &f.verts {
                if v >= n {
                    return Err(format!("la cara {i} usa el vértice {v} (hay {n})"));
                }
                used[v as usize] = true;
            }
        }
        if let Some(v) = used.iter().position(|u| !u) {
            return Err(format!("el vértice {v} no lo usa ninguna cara"));
        }
        let dir = directed(mesh);
        let mut undirected: BTreeMap<(u32, u32), usize> = BTreeMap::new();
        for (&(a, b), &k) in &dir {
            if a == b {
                return Err(format!("arista degenerada {a}-{a}"));
            }
            *undirected.entry((a.min(b), a.max(b))).or_insert(0) += k;
            if closed && (k != 1 || dir.get(&(b, a)) != Some(&1)) {
                return Err(format!("la arista {a}->{b} no cierra (x{k})"));
            }
        }
        for (e, k) in undirected {
            if k > 2 || (closed && k != 2) {
                return Err(format!("la arista {e:?} la usan {k} caras"));
            }
        }
        Ok(())
    }

    fn check_closed(mesh: &Mesh) {
        if let Err(e) = closed_edges_ok(mesh, true) {
            panic!("{e}\n{mesh:#?}");
        }
    }

    fn pos(mesh: &Mesh, v: u32) -> Vec3i {
        mesh.vertices[v as usize]
    }

    /// La normal (Newell) de la cara, sin normalizar.
    fn normal(mesh: &Mesh, f: usize) -> P3 {
        let vs = &mesh.faces[f].verts;
        let mut n = [0.0; 3];
        for i in 0..vs.len() {
            let a = pos(mesh, vs[i]);
            let b = pos(mesh, vs[(i + 1) % vs.len()]);
            let (a, b) = (
                [a.x as f32, a.y as f32, a.z as f32],
                [b.x as f32, b.y as f32, b.z as f32],
            );
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        n
    }

    fn face_center(mesh: &Mesh, f: usize) -> P3 {
        let vs = &mesh.faces[f].verts;
        let k = vs.len() as f32;
        let mut c = [0.0; 3];
        for &v in vs {
            let p = pos(mesh, v);
            c[0] += p.x as f32 / k;
            c[1] += p.y as f32 / k;
            c[2] += p.z as f32 / k;
        }
        c
    }

    fn uv_box(mesh: &Mesh, f: usize) -> ((i32, i32), (i32, i32)) {
        let uvs = &mesh.faces[f].uvs;
        let lo = (
            uvs.iter().map(|u| u.0).min().unwrap(),
            uvs.iter().map(|u| u.1).min().unwrap(),
        );
        let hi = (
            uvs.iter().map(|u| u.0).max().unwrap(),
            uvs.iter().map(|u| u.1).max().unwrap(),
        );
        (lo, hi)
    }

    fn painted(model: &Model3D) -> Vec<bool> {
        render_model(&RenderInput {
            model,
            size: (48, 48),
            texture: None,
            palette: &[],
            camera: None,
            extra_yaw: 0,
            frame: None,
        })
        .iter()
        .map(|p| p.a > 0)
        .collect()
    }

    fn all_subsets() -> impl Iterator<Item = Vec<usize>> {
        (1u32..64).map(|bits| (0..6).filter(|f| bits & (1 << f) != 0).collect())
    }

    // --- Extrude ---

    #[test]
    fn extruir_una_cara_del_cubo() {
        let m = cube();
        let (out, s) = extrude_faces(&m, &faces_sel(0, &[TOP]));
        let mesh = &out.objects[0].mesh;
        assert_eq!(mesh.vertices.len(), 12);
        assert_eq!(mesh.faces.len(), 10);
        assert_eq!(s, faces_sel(0, &[TOP]));
        // La cara extruida quedó en los vértices nuevos, en el mismo lugar que antes.
        let old = &m.objects[0].mesh;
        for (i, &v) in mesh.faces[TOP].verts.iter().enumerate() {
            assert!(v >= 8, "{v}");
            assert_eq!(pos(mesh, v), pos(old, old.faces[TOP].verts[i]));
        }
        assert_eq!(mesh.faces[TOP].uvs, old.faces[TOP].uvs);
        // Las otras cinco no cambian.
        for f in (0..6).filter(|&f| f != TOP) {
            assert_eq!(mesh.faces[f], old.faces[f]);
        }
        check_closed(mesh);
        // Los lados toman la textura de la celda de arriba (16..32, 16..32).
        for f in 6..10 {
            let ((u0, v0), (u1, v1)) = uv_box(mesh, f);
            assert!(u0 >= 16 && v0 >= 16 && u1 <= 32 && v1 <= 32, "{f}");
            assert!(
                u1 - u0 >= 1 && v1 - v0 >= 1,
                "el lado {f} tiene área en la textura"
            );
        }
    }

    #[test]
    fn los_lados_de_la_extrusion_miran_hacia_afuera() {
        let (out, s) = extrude_faces(&cube(), &faces_sel(0, &[TOP]));
        let moved = translate(&out, &s, [0.0, 16.0, 0.0]);
        let mesh = &moved.objects[0].mesh;
        check_closed(mesh);
        for f in 0..mesh.faces.len() {
            let (n, c) = (normal(mesh, f), face_center(mesh, f));
            // El centro del bloque extruido queda en y = 8.
            let out_dir = [c[0], c[1] - 8.0, c[2]];
            let d = n[0] * out_dir[0] + n[1] * out_dir[1] + n[2] * out_dir[2];
            assert!(d > 0.0, "la cara {f} mira hacia adentro");
        }
        assert!(painted(&moved).iter().any(|&p| p));
    }

    #[test]
    fn extruir_dos_caras_vecinas_no_crea_lado_entre_ellas() {
        let (out, s) = extrude_faces(&cube(), &faces_sel(0, &[FRONT, TOP]));
        let mesh = &out.objects[0].mesh;
        // 6 vértices nuevos (la arista compartida se duplica una vez); 6 lados (4 + 4 − 2).
        assert_eq!(mesh.vertices.len(), 14);
        assert_eq!(mesh.faces.len(), 12);
        assert_eq!(s, faces_sel(0, &[FRONT, TOP]));
        check_closed(mesh);
        let shared: BTreeSet<u32> = mesh.faces[FRONT].verts.iter().copied().collect();
        let shared: Vec<u32> = mesh.faces[TOP]
            .verts
            .iter()
            .copied()
            .filter(|v| shared.contains(v))
            .collect();
        assert_eq!(shared.len(), 2, "las dos caras siguen unidas por su arista");
    }

    #[test]
    fn extruir_cualquier_juego_de_caras_cierra() {
        for faces in all_subsets() {
            let (out, s) = extrude_faces(&cube(), &faces_sel(0, &faces));
            check_closed(&out.objects[0].mesh);
            assert_eq!(s, faces_sel(0, &faces));
        }
    }

    #[test]
    fn extruir_desde_vertices_usa_las_caras_que_cubren() {
        // En modo Vértice, las caras con todos sus vértices elegidos.
        let top: Vec<Item> = cube().objects[0].mesh.faces[TOP]
            .verts
            .iter()
            .map(|&v| Item::Vertex(0, v))
            .collect();
        let (out, s) = extrude_faces(&cube(), &sel(SelectMode::Vertex, top));
        assert_eq!(out.objects[0].mesh.faces.len(), 10);
        assert_eq!(s, faces_sel(0, &[TOP]));
    }

    // --- Subdivide ---

    #[test]
    fn dividir_una_cara_del_cubo() {
        let m = cube();
        let (out, s) = subdivide_faces(&m, &faces_sel(0, &[TOP]));
        let mesh = &out.objects[0].mesh;
        // 8 + 4 mitades + 1 centro; la de arriba en 4 y cada vecina en 1 cuadrilátero + 1 triángulo.
        assert_eq!(mesh.vertices.len(), 13);
        assert_eq!(mesh.faces.len(), 4 + 4 * 2 + 1);
        assert_eq!(s.mode, SelectMode::Face);
        assert_eq!(s.items.len(), 4);
        check_closed(mesh);
        assert_eq!(mesh.faces.iter().filter(|f| f.verts.len() == 3).count(), 4);
        // Las 4 nuevas son los cuartos de la celda de arriba (16..32, 16..32), sin repetirse.
        let mut quarters = BTreeSet::new();
        for item in &s.items {
            let Item::Face(0, f) = *item else {
                panic!("{item:?}")
            };
            assert_eq!(mesh.faces[f].verts.len(), 4);
            let n = normal(mesh, f);
            assert!(
                n[1] > 0.0 && n[0] == 0.0 && n[2] == 0.0,
                "mira hacia arriba"
            );
            let ((u0, v0), (u1, v1)) = uv_box(mesh, f);
            assert_eq!((u1 - u0, v1 - v0), (8, 8));
            quarters.insert((u0, v0));
            for &v in &mesh.faces[f].verts {
                assert_eq!(pos(mesh, v).y, FIXED);
            }
        }
        let want: BTreeSet<_> = [(16, 16), (24, 16), (16, 24), (24, 24)].into();
        assert_eq!(quarters, want);
        // Se ve igual que el cubo de antes: sin huecos en el render.
        let before = painted(&m);
        assert!(before.iter().filter(|&&p| p).count() > 100);
        assert_eq!(painted(&out), before);
    }

    #[test]
    fn dividir_cualquier_juego_de_caras_cierra() {
        for faces in all_subsets() {
            let m = cube();
            let (out, s) = subdivide_faces(&m, &faces_sel(0, &faces));
            let mesh = &out.objects[0].mesh;
            check_closed(mesh);
            assert_eq!(s.items.len(), faces.len() * 4, "{faces:?}");
            assert_eq!(painted(&out), painted(&m), "{faces:?}");
        }
    }

    #[test]
    fn dividir_no_parte_aristas_de_un_dieciseisavo() {
        // Una caja de 1/16 de ancho en x (de -16 a -15): sus aristas en x no se parten.
        let mut obj = cube_obj(Vec3i::default());
        for v in &mut obj.mesh.vertices {
            if v.x > 0 {
                v.x = -15;
            }
        }
        let m = model_of(vec![obj]);
        let (out, s) = subdivide_faces(&m, &faces_sel(0, &[FRONT, TOP]));
        let mesh = &out.objects[0].mesh;
        check_closed(mesh);
        for f in 0..mesh.faces.len() {
            let n = normal(mesh, f);
            assert!(n.iter().any(|&c| c != 0.0), "la cara {f} no tiene área");
        }
        // Cada una se parte solo a lo alto o a lo hondo: 2 pedazos.
        assert_eq!(s.items.len(), 4);
    }

    #[test]
    fn dividir_triangulos_cierra() {
        let m = model_of(vec![Object3D {
            mesh: primitive_mesh(&Primitive::Cylinder { sides: 6 }),
            ..cube_obj(Vec3i::default())
        }]);
        let n = m.objects[0].mesh.faces.len();
        // Un lado, un triángulo de la tapa, y los dos juntos (vecinos).
        for faces in [
            vec![0],
            vec![6],
            vec![0, 6],
            vec![6, 7, 8],
            (0..n).collect(),
        ] {
            let (out, s) = subdivide_faces(&m, &faces_sel(0, &faces));
            check_closed(&out.objects[0].mesh);
            assert_eq!(s.items.len(), faces.len() * 4, "{faces:?}");
        }
    }

    // --- Merge ---

    #[test]
    fn unir_los_vertices_de_una_cara() {
        let m = cube();
        let top: Vec<Item> = m.objects[0].mesh.faces[TOP]
            .verts
            .iter()
            .map(|&v| Item::Vertex(0, v))
            .collect();
        for s in [sel(SelectMode::Vertex, top), faces_sel(0, &[TOP])] {
            let (out, got) = merge_selection(&m, &s);
            let mesh = &out.objects[0].mesh;
            assert_eq!(mesh.vertices.len(), 5);
            // La de arriba se va; los 4 lados quedan en triángulos; la de abajo igual.
            assert_eq!(mesh.faces.len(), 5);
            assert_eq!(mesh.faces.iter().filter(|f| f.verts.len() == 3).count(), 4);
            closed_edges_ok(mesh, true).unwrap();
            let [Item::Vertex(0, v)] = got.items.iter().copied().collect::<Vec<_>>()[..] else {
                panic!("{got:?}")
            };
            assert_eq!(got.mode, SelectMode::Vertex);
            assert_eq!(
                pos(mesh, v),
                Vec3i {
                    x: 0,
                    y: FIXED,
                    z: 0
                }
            );
        }
    }

    #[test]
    fn unir_no_mezcla_objetos() {
        let m = model_of(vec![
            cube_obj(Vec3i::default()),
            cube_obj(Vec3i { x: 64, y: 0, z: 0 }),
        ]);
        let s = sel(SelectMode::Edge, [Item::Edge(0, 0, 1), Item::Edge(1, 0, 1)]);
        let (out, got) = merge_selection(&m, &s);
        for o in 0..2 {
            assert_eq!(out.objects[o].mesh.vertices.len(), 7);
            closed_edges_ok(&out.objects[o].mesh, false).unwrap();
        }
        assert_eq!(got.items.len(), 2);
    }

    // --- Delete ---

    /// Las caras como posiciones (para comparar después de compactar los índices).
    fn face_positions(mesh: &Mesh) -> Vec<Vec<Vec3i>> {
        mesh.faces
            .iter()
            .map(|f| f.verts.iter().map(|&v| pos(mesh, v)).collect())
            .collect()
    }

    #[test]
    fn borrar_un_vertice() {
        let m = cube();
        let (out, s) = delete_selection(&m, &sel(SelectMode::Vertex, [Item::Vertex(0, 0)]));
        let mesh = &out.objects[0].mesh;
        // El vértice 0 (−,−,−) está en atrás, izquierda y abajo.
        assert_eq!(mesh.faces.len(), 3);
        assert_eq!(mesh.vertices.len(), 7);
        closed_edges_ok(mesh, false).unwrap();
        let old = face_positions(&m.objects[0].mesh);
        assert_eq!(
            face_positions(mesh),
            vec![old[0].clone(), old[1].clone(), old[4].clone()]
        );
        assert!(s.is_empty());
        assert_eq!(s.mode, SelectMode::Vertex);
    }

    #[test]
    fn borrar_una_arista() {
        let m = cube();
        let (out, s) = delete_selection(&m, &sel(SelectMode::Edge, [Item::Edge(0, 0, 1)]));
        let mesh = &out.objects[0].mesh;
        // La arista 0-1 es de atrás y de abajo.
        assert_eq!(mesh.faces.len(), 4);
        assert_eq!(mesh.vertices.len(), 8);
        closed_edges_ok(mesh, false).unwrap();
        let old = face_positions(&m.objects[0].mesh);
        assert_eq!(face_positions(mesh), [0, 1, 3, 4].map(|f| old[f].clone()));
        assert!(s.is_empty());
        assert_eq!(s.mode, SelectMode::Edge);
    }

    #[test]
    fn borrar_caras_compacta_los_indices() {
        let m = cube();
        let (out, s) = delete_selection(&m, &faces_sel(0, &[TOP]));
        assert_eq!(out.objects[0].mesh.faces.len(), 5);
        assert_eq!(out.objects[0].mesh.vertices.len(), 8);
        assert!(s.is_empty());
        let (out, _) = delete_selection(&m, &faces_sel(0, &[1, 2, 3, 4, 5]));
        let mesh = &out.objects[0].mesh;
        assert_eq!(mesh.vertices.len(), 4);
        closed_edges_ok(mesh, false).unwrap();
        assert_eq!(
            face_positions(mesh),
            vec![face_positions(&m.objects[0].mesh)[FRONT].clone()]
        );
        assert_eq!(mesh.faces[0].uvs, m.objects[0].mesh.faces[FRONT].uvs);
    }

    #[test]
    fn borrar_objetos_en_modo_objeto() {
        let at = |x| Vec3i { x, y: 0, z: 0 };
        let m = model_of(vec![cube_obj(at(0)), cube_obj(at(64)), cube_obj(at(128))]);
        let (out, s) = delete_selection(
            &m,
            &sel(SelectMode::Object, [Item::Object(0), Item::Object(2)]),
        );
        assert_eq!(out.objects.len(), 1);
        assert_eq!(out.objects[0].transform.position, at(64));
        assert!(s.is_empty());
        assert_eq!(s.mode, SelectMode::Object);
    }

    #[test]
    fn borrar_objetos_con_una_seleccion_revuelta_quita_cada_uno_una_vez() {
        let at = |x| Vec3i { x, y: 0, z: 0 };
        let m = model_of(vec![cube_obj(at(0)), cube_obj(at(64)), cube_obj(at(128))]);
        // Los objetos salen 2, 0, 2: sin ordenar, se quitaría uno de más.
        let mixed = sel(
            SelectMode::Object,
            [Item::Object(2), Item::Vertex(0, 0), Item::Vertex(2, 0)],
        );
        let (out, _) = delete_selection(&m, &mixed);
        assert_eq!(out.objects.len(), 1);
        assert_eq!(out.objects[0].transform.position, at(64));
    }

    #[test]
    fn borrar_solo_toca_los_objetos_elegidos() {
        let m = model_of(vec![cube_obj(Vec3i::default()), cube_obj(Vec3i::default())]);
        let (out, _) = delete_selection(&m, &faces_sel(1, &[TOP]));
        assert_eq!(out.objects[0], m.objects[0]);
        assert_eq!(out.objects[1].mesh.faces.len(), 5);
    }
}
