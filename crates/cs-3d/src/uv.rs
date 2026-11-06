//! Edición de los UV de las caras (en pixeles de la textura), sin egui: cada función recibe el
//! modelo y devuelve uno nuevo. Las caras o vértices que no existen se ignoran.
//!
//! Redondeo: todo se lleva al pixel entero más cercano (`f32::round`: las mitades se alejan de
//! cero). Los giros y los espejos son exactos en enteros y no redondean nada.

use std::collections::{HashMap, HashSet};

use cs_core::Model3D;

/// Un punto de la textura, en pixeles.
pub type UvPoint = (i32, i32);

/// Las caras de `faces` que existen, sin repetir (en el orden en que llegan), como
/// `(objeto, cara)`.
fn existing(model: &Model3D, faces: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut seen = HashSet::with_capacity(faces.len());
    faces
        .iter()
        .copied()
        .filter(|&(o, f)| {
            model
                .objects
                .get(o)
                .is_some_and(|obj| f < obj.mesh.faces.len())
                && seen.insert((o, f))
        })
        .collect()
}

/// Caja `(min, max)` inclusiva de unos UV.
fn bounds_of(uvs: &[UvPoint]) -> Option<(UvPoint, UvPoint)> {
    let first = *uvs.first()?;
    Some(uvs.iter().fold((first, first), |(lo, hi), &(x, y)| {
        ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)))
    }))
}

/// Aplica `f(uv, caja de su grupo)` a todos los UV de cada cara existente. Un grupo son las
/// caras pedidas de una misma cara completa (mismo objeto y misma `part`); una cara sin parte
/// es su propio grupo. Así una tapa o el lado curvo de un cilindro giran como un todo.
fn map_faces(
    model: &Model3D,
    faces: &[(usize, usize)],
    f: impl Fn(UvPoint, (UvPoint, UvPoint)) -> UvPoint,
) -> Model3D {
    let faces = existing(model, faces);
    // El grupo de cada cara: (objeto, parte) o, sin parte, (objeto, la cara sola). Las cajas
    // se arman de una pasada (con esferas grandes, recalcularlas por cara era cuadrático en
    // cada frame de un arrastre).
    let group = |(o, i): (usize, usize)| {
        let part = model.objects[o].mesh.faces[i].part;
        (o, part, if part.is_some() { 0 } else { i })
    };
    let mut boxes: HashMap<_, (UvPoint, UvPoint)> = HashMap::new();
    for &(o, i) in &faces {
        if let Some((lo, hi)) = bounds_of(&model.objects[o].mesh.faces[i].uvs) {
            boxes
                .entry(group((o, i)))
                .and_modify(|(a, b)| {
                    *a = (a.0.min(lo.0), a.1.min(lo.1));
                    *b = (b.0.max(hi.0), b.1.max(hi.1));
                })
                .or_insert((lo, hi));
        }
    }
    let mut out = model.clone();
    for &(o, i) in &faces {
        if let Some(&b) = boxes.get(&group((o, i))) {
            for uv in &mut out.objects[o].mesh.faces[i].uvs {
                *uv = f(*uv, b);
            }
        }
    }
    out
}

/// La caja `(min, max)` inclusiva de todos los vértices UV de `faces`.
pub fn uv_bounds(model: &Model3D, faces: &[(usize, usize)]) -> Option<(UvPoint, UvPoint)> {
    let all: Vec<UvPoint> = existing(model, faces)
        .into_iter()
        .flat_map(|(o, f)| model.objects[o].mesh.faces[f].uvs.iter().copied())
        .collect();
    bounds_of(&all)
}

/// Desplaza los UV de `faces` en `d`.
pub fn move_uvs(model: &Model3D, faces: &[(usize, usize)], d: UvPoint) -> Model3D {
    map_faces(model, faces, |(x, y), _| (x + d.0, y + d.1))
}

/// Escala respecto a `anchor` por `(fx, fy)` y redondea cada vértice al pixel más cercano.
pub fn scale_uvs(
    model: &Model3D,
    faces: &[(usize, usize)],
    anchor: (f32, f32),
    f: (f32, f32),
) -> Model3D {
    map_faces(model, faces, |(x, y), _| {
        (
            (anchor.0 + (x as f32 - anchor.0) * f.0).round() as i32,
            (anchor.1 + (y as f32 - anchor.1) * f.1).round() as i32,
        )
    })
}

/// Lleva un solo vértice UV de una cara a `to`.
pub fn move_uv_vertex(
    model: &Model3D,
    face: (usize, usize),
    vertex: usize,
    to: UvPoint,
) -> Model3D {
    let mut out = model.clone();
    if let Some(uv) = out
        .objects
        .get_mut(face.0)
        .and_then(|o| o.mesh.faces.get_mut(face.1))
        .and_then(|f| f.uvs.get_mut(vertex))
    {
        *uv = to;
    }
    out
}

/// Un cuarto de vuelta en el sentido del reloj en pantalla (la `y` de la textura crece hacia
/// abajo) alrededor del centro de la caja de cada cara (de toda la cara completa si tiene
/// `part`): `(x, y)` pasa a `(a - y, b + x)`.
/// Con `sx = min.x + max.x` y `sy = min.y + max.y`, el giro exacto tiene `a = (sx + sy) / 2` y
/// `b = (sy - sx) / 2`; si `sx + sy` es par son enteros y el giro es exacto. Si es impar (caja
/// de ancho y alto de distinta paridad) el centro cae en medio pixel y se redondea con `+-1/2`
/// según la paridad de `sx`: la caja resultante tiene la paridad contraria, que redondea al
/// revés, así que cuatro giros devuelven exactamente el original (un giro corre la caja medio
/// pixel).
pub fn rotate_uvs_90(model: &Model3D, faces: &[(usize, usize)]) -> Model3D {
    map_faces(model, faces, |(x, y), (lo, hi)| {
        let (sx, sy) = (lo.0 + hi.0, lo.1 + hi.1);
        let s = if (sx + sy).rem_euclid(2) == 0 {
            0
        } else if sx.rem_euclid(2) == 1 {
            1
        } else {
            -1
        };
        let a = (sx + sy + s) / 2;
        let b = (sy - sx + s) / 2;
        (a - y, b + x)
    })
}

/// Espeja respecto al centro de la caja de cada cara (de toda la cara completa si tiene `part`;
/// exacto: `min + max - v`).
pub fn flip_uvs(model: &Model3D, faces: &[(usize, usize)], horizontal: bool) -> Model3D {
    map_faces(model, faces, |(x, y), (lo, hi)| {
        if horizontal {
            (lo.0 + hi.0 - x, y)
        } else {
            (x, lo.1 + hi.1 - y)
        }
    })
}

/// Dónde cruza la recta horizontal `y` la arista `a`–`b` del polígono, si la cruza (las
/// puntas de abajo cuentan y las de arriba no, así un vértice no se cuenta dos veces).
fn crossing(a: UvPoint, b: UvPoint, y: f32) -> Option<f32> {
    let (ay, by) = (a.1 as f32, b.1 as f32);
    ((ay > y) != (by > y)).then(|| a.0 as f32 + (y - ay) * (b.0 - a.0) as f32 / (by - ay))
}

/// Si el punto `p` (en pixeles de la textura) cae dentro del polígono `uvs` (regla par-impar).
/// Con menos de tres vértices no hay adentro.
pub fn uv_polygon_contains(uvs: &[UvPoint], p: (f32, f32)) -> bool {
    let n = uvs.len();
    if n < 3 {
        return false;
    }
    (0..n)
        .filter_map(|i| crossing(uvs[i], uvs[(i + 1) % n], p.1))
        .filter(|&x| p.0 < x)
        .count()
        % 2
        == 1
}

/// Los pixeles de una textura de `w × h` que cubren los polígonos UV de `faces`: los que tienen
/// el centro adentro (regla par-impar), repetidos con `rem_euclid` como el muestreo. Fila por
/// fila.
pub fn uv_coverage(model: &Model3D, faces: &[(usize, usize)], (w, h): (u32, u32)) -> Vec<bool> {
    let mut out = vec![false; w as usize * h as usize];
    if w == 0 || h == 0 {
        return out;
    }
    let (wi, hi) = (w as i64, h as i64);
    let mut xs: Vec<f32> = Vec::new();
    for (o, f) in existing(model, faces) {
        let uvs = &model.objects[o].mesh.faces[f].uvs;
        let n = uvs.len();
        let Some(((_, y0), (_, y1))) = bounds_of(uvs).filter(|_| n >= 3) else {
            continue;
        };
        for y in y0..y1 {
            xs.clear();
            xs.extend((0..n).filter_map(|i| crossing(uvs[i], uvs[(i + 1) % n], y as f32 + 0.5)));
            xs.sort_by(f32::total_cmp);
            let row = (y as i64).rem_euclid(hi) as usize * w as usize;
            for span in xs.as_chunks::<2>().0 {
                // Los pixeles con el centro en [a, b).
                let a = (span[0] - 0.5).ceil() as i64;
                let b = (span[1] - 0.5).ceil() as i64;
                if b - a >= wi {
                    out[row..row + w as usize].fill(true);
                    continue;
                }
                for x in a..b {
                    out[row + x.rem_euclid(wi) as usize] = true;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{Face, Mesh, Object3D, Transform3D};

    fn quad(uvs: [UvPoint; 4]) -> Face {
        Face {
            verts: vec![0, 1, 2, 3],
            uvs: uvs.to_vec(),
            part: None,
        }
    }

    /// Un objeto con dos caras: un rectángulo 4×2 en (10,20) y un cuadrado en el origen.
    fn model() -> Model3D {
        let mesh = Mesh {
            faces: vec![
                quad([(10, 20), (14, 20), (14, 22), (10, 22)]),
                quad([(0, 0), (3, 0), (3, 3), (0, 3)]),
            ],
            ..Mesh::default()
        };
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

    fn uvs(m: &Model3D, f: usize) -> Vec<UvPoint> {
        m.objects[0].mesh.faces[f].uvs.clone()
    }

    #[test]
    fn bounds_de_una_cara_y_de_varias() {
        let m = model();
        assert_eq!(uv_bounds(&m, &[(0, 0)]), Some(((10, 20), (14, 22))));
        assert_eq!(uv_bounds(&m, &[(0, 0), (0, 1)]), Some(((0, 0), (14, 22))));
        assert_eq!(uv_bounds(&m, &[(0, 9), (3, 0)]), None);
        assert_eq!(uv_bounds(&m, &[]), None);
    }

    #[test]
    fn move_uvs_desplaza_solo_las_caras_pedidas() {
        let m = move_uvs(&model(), &[(0, 0)], (3, -2));
        assert_eq!(uvs(&m, 0), vec![(13, 18), (17, 18), (17, 20), (13, 20)]);
        assert_eq!(uvs(&m, 1), uvs(&model(), 1));
    }

    #[test]
    fn scale_uvs_x2_desde_la_esquina_menor() {
        let m = scale_uvs(&model(), &[(0, 0)], (10.0, 20.0), (2.0, 2.0));
        assert_eq!(uvs(&m, 0), vec![(10, 20), (18, 20), (18, 24), (10, 24)]);
        // Redondeo al pixel más cercano.
        let m = scale_uvs(&model(), &[(0, 1)], (0.0, 0.0), (0.5, 0.5));
        assert_eq!(uvs(&m, 1), vec![(0, 0), (2, 0), (2, 2), (0, 2)]);
    }

    #[test]
    fn move_uv_vertex_cambia_solo_ese_vertice() {
        let m = move_uv_vertex(&model(), (0, 0), 2, (99, 98));
        assert_eq!(uvs(&m, 0), vec![(10, 20), (14, 20), (99, 98), (10, 22)]);
        assert_eq!(uvs(&m, 1), uvs(&model(), 1));
    }

    #[test]
    fn rotar_cuatro_veces_es_el_original() {
        for faces in [vec![(0, 0)], vec![(0, 1)], vec![(0, 0), (0, 1)]] {
            let mut m = model();
            for _ in 0..4 {
                m = rotate_uvs_90(&m, &faces);
            }
            assert_eq!(m, model());
        }
        // Cajas de todos los tamaños (pares e impares) y con coordenadas negativas.
        for (w, h) in [
            (0, 0),
            (1, 0),
            (0, 1),
            (3, 2),
            (2, 3),
            (5, 4),
            (4, 5),
            (6, 3),
        ] {
            for (ox, oy) in [(0, 0), (7, 3), (-5, -8), (-4, 9)] {
                let mut m = model();
                m.objects[0].mesh.faces[1] =
                    quad([(ox, oy), (ox + w, oy), (ox + w, oy + h), (ox, oy + h)]);
                let orig = m.clone();
                for _ in 0..4 {
                    m = rotate_uvs_90(&m, &[(0, 1)]);
                }
                assert_eq!(m, orig, "caja {w}x{h} en ({ox},{oy})");
            }
        }
    }

    #[test]
    fn rotar_un_rectangulo_4x2_da_uno_2x4_con_el_mismo_centro() {
        let m = rotate_uvs_90(&model(), &[(0, 0)]);
        // Centro (12, 21). La esquina sup. izq. (10,20) pasa a sup. der.: sentido del reloj.
        assert_eq!(uvs(&m, 0), vec![(13, 19), (13, 23), (11, 23), (11, 19)]);
        assert_eq!(uv_bounds(&m, &[(0, 0)]), Some(((11, 19), (13, 23))));
        assert_eq!(uvs(&m, 1), uvs(&model(), 1));
    }

    #[test]
    fn flip_dos_veces_es_el_original_y_una_invierte() {
        let m = flip_uvs(&model(), &[(0, 0)], true);
        assert_eq!(uvs(&m, 0), vec![(14, 20), (10, 20), (10, 22), (14, 22)]);
        assert_eq!(flip_uvs(&m, &[(0, 0)], true), model());
        let m = flip_uvs(&model(), &[(0, 0)], false);
        assert_eq!(uvs(&m, 0), vec![(10, 22), (14, 22), (14, 20), (10, 20)]);
        assert_eq!(flip_uvs(&m, &[(0, 0)], false), model());
    }

    /// Un cilindro de 8 lados: el lado curvo (caras 0..8, parte 0), la tapa de arriba (8..16,
    /// parte 1) y la de abajo (16..24, parte 2).
    fn cylinder() -> Model3D {
        let mut m = model();
        m.objects[0].mesh = crate::primitive_mesh(&crate::Primitive::Cylinder { sides: 8 });
        m
    }

    fn range(r: std::ops::Range<usize>) -> Vec<(usize, usize)> {
        r.map(|f| (0, f)).collect()
    }

    /// Si las cajas de las caras `faces` se enciman (con área en común, no solo un borde).
    fn overlapping(m: &Model3D, faces: &[(usize, usize)]) -> bool {
        let boxes: Vec<_> = faces.iter().map(|&f| uv_bounds(m, &[f]).unwrap()).collect();
        boxes.iter().enumerate().any(|(i, a)| {
            boxes[i + 1..]
                .iter()
                .any(|b| a.0.0 < b.1.0 && b.0.0 < a.1.0 && a.0.1 < b.1.1 && b.0.1 < a.1.1)
        })
    }

    #[test]
    fn rotar_el_lado_del_cilindro_gira_la_tira_completa() {
        let side = range(0..8);
        let m = rotate_uvs_90(&cylinder(), &side);
        // La caja de las 8 tiras (32×16) gira como un todo: 16×32 con el mismo centro.
        assert_eq!(uv_bounds(&cylinder(), &side), Some(((0, 32), (32, 48))));
        assert_eq!(uv_bounds(&m, &side), Some(((8, 24), (24, 56))));
        assert!(!overlapping(&m, &side));
        // Las tapas no se tocan.
        assert_eq!(
            m.objects[0].mesh.faces[8..],
            cylinder().objects[0].mesh.faces[8..]
        );
    }

    #[test]
    fn rotar_la_tapa_del_cilindro_gira_sus_gajos_juntos() {
        let cap = range(8..16);
        let m = rotate_uvs_90(&cylinder(), &cap);
        // El centro de la tapa es el mismo punto en los 8 gajos.
        let centers: Vec<UvPoint> = (8..16).map(|f| uvs(&m, f)[0]).collect();
        assert!(centers.iter().all(|&c| c == centers[0]), "{centers:?}");
        let (lo, hi) = uv_bounds(&cylinder(), &cap).unwrap();
        let (rlo, rhi) = uv_bounds(&m, &cap).unwrap();
        assert_eq!((rhi.0 - rlo.0, rhi.1 - rlo.1), (hi.1 - lo.1, hi.0 - lo.0));
    }

    #[test]
    fn cuatro_giros_de_una_parte_son_el_original() {
        for faces in [range(0..8), range(8..16), range(0..24)] {
            let mut m = cylinder();
            for _ in 0..4 {
                m = rotate_uvs_90(&m, &faces);
            }
            assert_eq!(m, cylinder());
        }
    }

    #[test]
    fn flip_h_espeja_la_tira_completa() {
        let side = range(0..8);
        let m = flip_uvs(&cylinder(), &side, true);
        // La primera tira (x 0..4) pasa al otro extremo (x 28..32), espejada.
        assert_eq!(uvs(&m, 0), vec![(32, 48), (28, 48), (28, 32), (32, 32)]);
        assert_eq!(uv_bounds(&m, &side), uv_bounds(&cylinder(), &side));
        assert!(!overlapping(&m, &side));
        assert_eq!(flip_uvs(&m, &side, true), cylinder());
    }

    #[test]
    fn caras_repetidas_o_desordenadas_dan_lo_mismo() {
        // El cilindro y un objeto más (las caras de `model()`, sin parte).
        let mut m = cylinder();
        m.objects.push(model().objects[0].clone());
        let once: Vec<(usize, usize)> = range(0..6)
            .into_iter()
            .chain(range(9..12))
            .chain([(1, 1)])
            .collect();
        let mut messy: Vec<(usize, usize)> = once.iter().rev().copied().collect();
        messy.extend(once.iter().copied());
        messy.push((0, 99));
        for op in [rotate_uvs_90, |m: &Model3D, f: &[(usize, usize)]| {
            flip_uvs(m, f, true)
        }] {
            assert_eq!(op(&m, &messy), op(&m, &once));
        }
        // La caja del pedazo pedido de la tira (6 de 8), no la de toda la parte.
        let r = rotate_uvs_90(&m, &once);
        let (lo, hi) = uv_bounds(&m, &range(0..6)).unwrap();
        let (rlo, rhi) = uv_bounds(&r, &range(0..6)).unwrap();
        assert_eq!((rhi.0 - rlo.0, rhi.1 - rlo.1), (hi.1 - lo.1, hi.0 - lo.0));
        assert_eq!(r.objects[0].mesh.faces[6..9], m.objects[0].mesh.faces[6..9]);
    }

    #[test]
    fn mover_las_uvs_de_una_esfera_grande_es_rapido() {
        // 64×32: 2048 caras de una sola parte, como al arrastrar la caja UV de la esfera.
        let mut m = model();
        m.objects[0].mesh = crate::primitive_mesh(&crate::Primitive::Sphere {
            sides: 64,
            rings: 32,
        });
        let all = range(0..m.objects[0].mesh.faces.len());
        let start = std::time::Instant::now();
        let moved = move_uvs(&m, &all, (1, 0));
        let spent = start.elapsed();
        assert_eq!(
            moved.objects[0].mesh.faces[0].uvs[0].0,
            m.objects[0].mesh.faces[0].uvs[0].0 + 1
        );
        // Antes (cuadrático), en debug, unos 40 s; ahora, milisegundos.
        assert!(spent.as_millis() < 2000, "{spent:?}");
    }

    #[test]
    fn caras_sin_parte_giran_solas() {
        // Las dos caras de `model()` no tienen parte: cada una gira en su propia caja.
        let m = rotate_uvs_90(&model(), &[(0, 0), (0, 1)]);
        assert_eq!(
            m,
            rotate_uvs_90(&rotate_uvs_90(&model(), &[(0, 0)]), &[(0, 1)])
        );
    }

    /// Los pixeles `(x, y)` cubiertos de una textura de `w` de ancho.
    fn covered(c: &[bool], w: usize) -> Vec<(usize, usize)> {
        c.iter()
            .enumerate()
            .filter(|(_, on)| **on)
            .map(|(i, _)| (i % w, i / w))
            .collect()
    }

    #[test]
    fn la_cobertura_son_los_pixeles_con_el_centro_adentro() {
        let c = uv_coverage(&model(), &[(0, 1)], (8, 8));
        let square: Vec<_> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).collect();
        assert_eq!(covered(&c, 8), square);
        // Un triángulo rectángulo de 4: los centros con x + y < 3.
        let mut m = model();
        m.objects[0].mesh.faces[1].uvs = vec![(0, 0), (4, 0), (0, 4)];
        let c = uv_coverage(&m, &[(0, 1)], (8, 8));
        let tri = vec![(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (0, 2)];
        assert_eq!(covered(&c, 8), tri);
        // Las dos caras juntas; sin caras, nada.
        let both = uv_coverage(&model(), &[(0, 0), (0, 1)], (32, 32));
        assert_eq!(covered(&both, 32).len(), 9 + 8);
        assert!(!uv_coverage(&model(), &[], (8, 8)).contains(&true));
    }

    #[test]
    fn la_cobertura_se_repite_fuera_de_la_textura() {
        // El rectángulo 4×2 en (10, 20) cae en (10, 4) de una textura de 16.
        let c = uv_coverage(&model(), &[(0, 0)], (16, 16));
        let want: Vec<_> = (4..6).flat_map(|y| (10..14).map(move |x| (x, y))).collect();
        assert_eq!(covered(&c, 16), want);
        // Uno más ancho que la textura cubre filas enteras.
        let mut m = model();
        m.objects[0].mesh.faces[1].uvs = vec![(-5, 1), (30, 1), (30, 2), (-5, 2)];
        let c = uv_coverage(&m, &[(0, 1)], (8, 8));
        assert_eq!(covered(&c, 8), (0..8).map(|x| (x, 1)).collect::<Vec<_>>());
    }

    #[test]
    fn un_punto_dentro_de_un_poligono_uv() {
        let tri = [(0, 0), (4, 0), (0, 4)];
        assert!(uv_polygon_contains(&tri, (0.5, 0.5)));
        assert!(uv_polygon_contains(&tri, (1.0, 2.5)));
        assert!(!uv_polygon_contains(&tri, (3.0, 3.0)));
        assert!(!uv_polygon_contains(&tri, (-0.5, 1.0)));
        assert!(!uv_polygon_contains(&[(0, 0), (4, 0)], (1.0, 0.0)));
    }

    #[test]
    fn caras_inexistentes_no_truenan() {
        let bad = [(7, 0), (0, 9)];
        let base = model();
        assert_eq!(move_uvs(&base, &bad, (1, 1)), base);
        assert_eq!(scale_uvs(&base, &bad, (0.0, 0.0), (2.0, 2.0)), base);
        assert_eq!(rotate_uvs_90(&base, &bad), base);
        assert_eq!(flip_uvs(&base, &bad, true), base);
        assert_eq!(move_uv_vertex(&base, (0, 0), 9, (1, 1)), base);
        assert_eq!(move_uv_vertex(&base, (5, 5), 0, (1, 1)), base);
    }
}
