//! Edición de los UV de las caras (en pixeles de la textura), sin egui: cada función recibe el
//! modelo y devuelve uno nuevo. Las caras o vértices que no existen se ignoran.
//!
//! Redondeo: todo se lleva al pixel entero más cercano (`f32::round`: las mitades se alejan de
//! cero). Los giros y los espejos son exactos en enteros y no redondean nada.

use cs_core::Model3D;

/// Un punto de la textura, en pixeles.
pub type UvPoint = (i32, i32);

/// Las caras de `faces` que existen, sin repetir, como `(objeto, cara)`.
fn existing(model: &Model3D, faces: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for &(o, f) in faces {
        let ok = model
            .objects
            .get(o)
            .is_some_and(|obj| f < obj.mesh.faces.len());
        if ok && !out.contains(&(o, f)) {
            out.push((o, f));
        }
    }
    out
}

/// Caja `(min, max)` inclusiva de unos UV.
fn bounds_of(uvs: &[UvPoint]) -> Option<(UvPoint, UvPoint)> {
    let first = *uvs.first()?;
    Some(uvs.iter().fold((first, first), |(lo, hi), &(x, y)| {
        ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)))
    }))
}

/// Aplica `f(uv, caja de su cara)` a todos los UV de cada cara existente.
fn map_faces(
    model: &Model3D,
    faces: &[(usize, usize)],
    f: impl Fn(UvPoint, (UvPoint, UvPoint)) -> UvPoint,
) -> Model3D {
    let mut out = model.clone();
    for (o, i) in existing(model, faces) {
        let face = &mut out.objects[o].mesh.faces[i];
        if let Some(b) = bounds_of(&face.uvs) {
            for uv in &mut face.uvs {
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
/// abajo) alrededor del centro de la caja de cada cara: `(x, y)` pasa a `(a - y, b + x)`.
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

/// Espeja respecto al centro de la caja de cada cara (exacto: `min + max - v`).
pub fn flip_uvs(model: &Model3D, faces: &[(usize, usize)], horizontal: bool) -> Model3D {
    map_faces(model, faces, |(x, y), (lo, hi)| {
        if horizontal {
            (lo.0 + hi.0 - x, y)
        } else {
            (x, lo.1 + hi.1 - y)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{Face, Mesh, Object3D, Transform3D};

    fn quad(uvs: [UvPoint; 4]) -> Face {
        Face {
            verts: vec![0, 1, 2, 3],
            uvs: uvs.to_vec(),
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
