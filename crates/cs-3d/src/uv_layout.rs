//! Acomodo de los objetos en la textura: la caja de las UVs de cada objeto y el primer lugar
//! libre para una caja nueva (sin encimarse con las demás, con `MARGIN` de aire).

use cs_core::Model3D;

/// Una caja de la textura `(x, y, w, h)` en pixeles (`w`, `h` >= 1): cubre de `x` a `x + w`.
pub type UvRect = (i32, i32, i32, i32);

/// Los pixeles de aire entre dos cajas.
pub const MARGIN: i32 = 1;

/// La caja de todas las UVs del objeto `o` (None si no tiene caras).
pub fn object_uv_rect(model: &Model3D, o: usize) -> Option<UvRect> {
    let obj = model.objects.get(o)?;
    let faces: Vec<(usize, usize)> = (0..obj.mesh.faces.len()).map(|f| (o, f)).collect();
    let ((x0, y0), (x1, y1)) = crate::uv_bounds(model, &faces)?;
    Some((x0, y0, (x1 - x0).max(1), (y1 - y0).max(1)))
}

/// Si la caja `a` toca a `b` (con `MARGIN` de aire entre las dos ya no se tocan).
fn touches(a: UvRect, b: UvRect) -> bool {
    a.0 < b.0 + b.2 + MARGIN
        && b.0 < a.0 + a.2 + MARGIN
        && a.1 < b.1 + b.3 + MARGIN
        && b.1 < a.1 + a.3 + MARGIN
}

/// El primer lugar (de arriba a la izquierda, en filas) de `w`×`h` dentro de `side`×`side` que no
/// toca ninguna de `taken` con `MARGIN` (1 px) de aire. None si no cabe.
pub fn find_free(taken: &[UvRect], w: i32, h: i32, side: i32) -> Option<(i32, i32)> {
    let (w, h) = (w.max(1), h.max(1));
    for y in 0..=side - h {
        let mut x = 0;
        while x + w <= side {
            match taken.iter().find(|&&r| touches((x, y, w, h), r)) {
                // Salta al primer lugar a la derecha de lo que estorba.
                Some(r) => x = x.max(r.0 + r.2 + MARGIN),
                None => return Some((x, y)),
            }
        }
    }
    None
}

/// `model` con las UVs del objeto `o` corridas para que su caja empiece en `at`.
pub fn place_object(model: &Model3D, o: usize, at: (i32, i32)) -> Model3D {
    let Some((x, y, _, _)) = object_uv_rect(model, o) else {
        return model.clone();
    };
    let faces: Vec<(usize, usize)> = (0..model.objects[o].mesh.faces.len())
        .map(|f| (o, f))
        .collect();
    crate::move_uvs(model, &faces, (at.0 - x, at.1 - y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Primitive, primitive_mesh};
    use cs_core::{Object3D, Transform3D};

    fn model_with(p: Primitive) -> Model3D {
        let mut m = Model3D::default();
        m.push_object(
            Object3D {
                name: p.name().into(),
                mesh: primitive_mesh(&p),
                transform: Transform3D::default(),
                double_sided: false,
                kind: Some(p.kind()),
                hidden: false,
                locked: false,
            },
            None,
        );
        m
    }

    #[test]
    fn in_an_empty_texture_the_place_is_the_corner() {
        assert_eq!(find_free(&[], 16, 16, 64), Some((0, 0)));
    }

    #[test]
    fn a_box_goes_right_of_a_taken_one_with_one_pixel_of_air() {
        assert_eq!(find_free(&[(0, 0, 48, 32)], 16, 16, 128), Some((49, 0)));
        // En 64 no cabe a la derecha: va abajo.
        assert_eq!(find_free(&[(0, 0, 48, 32)], 16, 16, 64), Some((0, 33)));
    }

    #[test]
    fn without_room_there_is_no_place() {
        assert_eq!(find_free(&[(0, 0, 48, 32)], 48, 32, 64), None);
        assert_eq!(find_free(&[], 65, 1, 64), None);
    }

    #[test]
    fn the_box_of_a_cube_is_its_zone() {
        let m = model_with(Primitive::Cube);
        assert_eq!(object_uv_rect(&m, 0), Some(Primitive::Cube.zone()));
        assert_eq!(object_uv_rect(&m, 1), None);
    }

    #[test]
    fn placing_moves_the_box_and_keeps_the_layout() {
        let m = model_with(Primitive::Cylinder { sides: 8 });
        let (x, y, w, h) = object_uv_rect(&m, 0).unwrap();
        let placed = place_object(&m, 0, (49, 3));
        assert_eq!(object_uv_rect(&placed, 0), Some((49, 3, w, h)));
        let d = (49 - x, 3 - y);
        for (a, b) in m.objects[0]
            .mesh
            .faces
            .iter()
            .zip(&placed.objects[0].mesh.faces)
        {
            let moved: Vec<(i32, i32)> = a.uvs.iter().map(|&(u, v)| (u + d.0, v + d.1)).collect();
            assert_eq!(b.uvs, moved);
            assert_eq!(b.verts, a.verts);
        }
    }
}
