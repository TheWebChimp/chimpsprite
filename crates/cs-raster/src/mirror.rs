//! Selección espejo: cómo se parte una selección simétrica en lo que se transforma (lo primario,
//! del lado del ancla), sus reflejos y las piezas que caen sobre los ejes. Puro, sin píxeles: la UI
//! trae los valores y transforma.

use std::collections::VecDeque;

use cs_core::Mask;

use crate::Point;
use crate::symmetry::{self, IDENTITY, SymmetryAxes, Transform};

/// Una de las simetrías del grupo que generan los ejes prendidos (reflejo o giro alrededor del
/// centro de simetría).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorOp(Transform);

impl MirrorOp {
    pub const IDENTITY: MirrorOp = MirrorOp(IDENTITY);

    /// El píxel `p` llevado por esta simetría (centro en medios píxeles).
    pub fn apply(self, p: Point, center2: (i32, i32)) -> Point {
        symmetry::apply(self.0, p, center2)
    }

    /// La simetría inversa (las matrices son ortogonales: la traspuesta).
    pub fn inverse(self) -> MirrorOp {
        let [a, b, c, d] = self.0.0;
        MirrorOp(Transform([a, c, b, d]))
    }

    /// Un desplazamiento llevado por esta simetría (solo la parte lineal).
    pub fn linear(self, (dx, dy): (i32, i32)) -> (i32, i32) {
        let [a, b, c, d] = self.0.0;
        (a * dx + b * dy, c * dx + d * dy)
    }

    pub fn is_identity(self) -> bool {
        self == Self::IDENTITY
    }

    /// La matriz `[a, b, c, d]`: `(dx, dy) → (a·dx + b·dy, c·dx + d·dy)`.
    pub fn matrix(self) -> [i32; 4] {
        self.0.0
    }
}

/// Las simetrías del grupo de `axes`, con la identidad primero.
pub fn mirror_ops(axes: SymmetryAxes) -> Vec<MirrorOp> {
    symmetry::group(axes).iter().map(|&t| MirrorOp(t)).collect()
}

/// Una pieza que es su propio reflejo (cruza un eje, está sobre él o son dos mitades que se
/// tocan): no se parte, se mueve dejándola simétrica.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedPiece {
    /// Los píxeles de la representante de su órbita.
    pub points: Vec<Point>,
    /// Las simetrías que la dejan igual (con la identidad).
    pub stabilizer: Vec<MirrorOp>,
    /// Sus reflejos que son otras piezas (con más de un eje): la copia es `op(points)`.
    pub copies: Vec<MirrorOp>,
}

impl FixedPiece {
    /// Simétrica de izquierda a derecha alrededor del centro (│ o el giro de 180°): su caja queda
    /// centrada en x.
    pub fn centered_x(&self) -> bool {
        self.stabilizer
            .iter()
            .any(|op| matches!(op.matrix(), [-1, 0, 0, _]))
    }

    /// Simétrica de arriba a abajo (─ o el giro de 180°): centrada en y.
    pub fn centered_y(&self) -> bool {
        self.stabilizer
            .iter()
            .any(|op| matches!(op.matrix(), [_, 0, 0, -1]))
    }

    /// Simétrica sobre una diagonal o por un giro de 90°: solo se traslada.
    pub fn diagonal(&self) -> bool {
        self.stabilizer.iter().any(|op| op.matrix()[1] != 0)
    }
}

/// Cómo se transforma en espejo una selección simétrica.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MirrorPlan {
    /// Centro de simetría en medios píxeles.
    pub center2: (i32, i32),
    /// Lo primario: las piezas libres del lado del ancla. Se transforma con la caja.
    pub primary: Vec<Point>,
    /// Los reflejos de lo primario (todas las simetrías menos la identidad): cada copia es
    /// `op(primary)` y se transforma conjugada, `op ∘ T ∘ op⁻¹`.
    pub copies: Vec<MirrorOp>,
    /// Las piezas sobre los ejes (una por órbita).
    pub fixed: Vec<FixedPiece>,
}

impl MirrorPlan {
    /// Esquinas (inclusivas) de lo primario y las piezas fijas representantes: la caja.
    pub fn bounds(&self) -> Option<(Point, Point)> {
        self.primary
            .iter()
            .chain(self.fixed.iter().flat_map(|f| f.points.iter()))
            .fold(None, |acc, &(x, y)| match acc {
                None => Some(((x, y), (x, y))),
                Some(((x0, y0), (x1, y1))) => {
                    Some(((x0.min(x), y0.min(y)), (x1.max(x), y1.max(y))))
                }
            })
    }
}

/// El desplazamiento `d` llevado a lo que deja simétrica una pieza con este estabilizador: el
/// promedio de `h(d)` (truncado hacia cero, así sigue siendo simétrico).
pub fn project(stabilizer: &[MirrorOp], d: (i32, i32)) -> (i32, i32) {
    let n = stabilizer.len().max(1) as i32;
    let (sx, sy) = stabilizer.iter().fold((0, 0), |(sx, sy), op| {
        let (x, y) = op.linear(d);
        (sx + x, sy + y)
    });
    (sx / n, sy / n)
}

/// Parte la selección `mask` según los ejes `axes` (centro `center2`, en medios píxeles) para
/// transformarla en espejo. `anchor2` (medios píxeles) elige el lado: de cada grupo de reflejos se
/// toma la pieza más cercana. `None` si no hay ejes, la selección está vacía o no es simétrica
/// (algún reflejo cae fuera de ella o del lienzo).
pub fn mirror_plan(
    mask: &Mask,
    axes: SymmetryAxes,
    center2: (i32, i32),
    anchor2: (i32, i32),
) -> Option<MirrorPlan> {
    if !axes.any() {
        return None;
    }
    let ops = mirror_ops(axes);
    let (w, h) = (mask.width() as i32, mask.height() as i32);
    let index = |(x, y): Point| (y * w + x) as usize;
    // Invariante y sin perder píxeles: cada reflejo está seleccionado y vuelve a su lugar.
    for p in mask.points() {
        for &op in &ops {
            let q = op.apply(p, center2);
            if !mask.contains(q.0, q.1) || op.inverse().apply(q, center2) != p {
                return None;
            }
        }
    }
    // Componentes de 8 vecinos.
    let mut label = vec![u32::MAX; (w.max(0) * h.max(0)) as usize];
    let mut components: Vec<Vec<Point>> = Vec::new();
    for p in mask.points() {
        if label[index(p)] != u32::MAX {
            continue;
        }
        let id = components.len() as u32;
        let mut points = Vec::new();
        let mut queue = VecDeque::from([p]);
        label[index(p)] = id;
        while let Some((x, y)) = queue.pop_front() {
            points.push((x, y));
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let n = (x + dx, y + dy);
                    if mask.contains(n.0, n.1) && label[index(n)] == u32::MAX {
                        label[index(n)] = id;
                        queue.push_back(n);
                    }
                }
            }
        }
        components.push(points);
    }
    if components.is_empty() {
        return None;
    }
    // La pieza a la que lleva `op` a la componente `c`, si la lleva entera a una sola.
    let image = |c: usize, op: MirrorOp| -> Option<usize> {
        let points = &components[c];
        let first = op.apply(points[0], center2);
        let target = label[index(first)] as usize;
        (components[target].len() == points.len()
            && points
                .iter()
                .all(|&p| label[index(op.apply(p, center2))] as usize == target))
        .then_some(target)
    };
    // Distancia (al cuadrado, escalada por el tamaño) del centro de la pieza al ancla.
    let distance = |c: usize| -> i64 {
        let points = &components[c];
        let n = points.len() as i64;
        let (sx, sy) = points.iter().fold((0i64, 0i64), |(sx, sy), &(x, y)| {
            (sx + 2 * x as i64 + 1, sy + 2 * y as i64 + 1)
        });
        let (dx, dy) = (sx - n * anchor2.0 as i64, sy - n * anchor2.1 as i64);
        dx * dx + dy * dy
    };
    let mut done = vec![false; components.len()];
    let mut plan = MirrorPlan {
        center2,
        primary: Vec::new(),
        copies: ops[1..].to_vec(),
        fixed: Vec::new(),
    };
    for c in 0..components.len() {
        if done[c] {
            continue;
        }
        let mut members = Vec::new();
        for &op in &ops {
            let m = image(c, op)?;
            if !members.contains(&m) {
                members.push(m);
            }
        }
        for &m in &members {
            done[m] = true;
        }
        // Empates: la primera en el orden de lectura (la de menor número).
        let rep = members
            .iter()
            .copied()
            .min_by_key(|&m| (distance(m), m))
            .unwrap_or(c);
        let mut stabilizer = Vec::new();
        let mut copies = Vec::new();
        let mut seen = Vec::new();
        for &op in &ops {
            let m = image(rep, op)?;
            if m == rep {
                stabilizer.push(op);
            } else if !seen.contains(&m) {
                seen.push(m);
                copies.push(op);
            }
        }
        if stabilizer.len() == 1 {
            plan.primary.extend_from_slice(&components[rep]);
        } else {
            plan.fixed.push(FixedPiece {
                points: components[rep].clone(),
                stabilizer,
                copies,
            });
        }
    }
    Some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    const V: SymmetryAxes = SymmetryAxes {
        vertical: true,
        ..SymmetryAxes::NONE
    };
    const VH: SymmetryAxes = SymmetryAxes {
        vertical: true,
        horizontal: true,
        ..SymmetryAxes::NONE
    };
    const DOWN: SymmetryAxes = SymmetryAxes {
        diagonal_down: true,
        ..SymmetryAxes::NONE
    };
    const ALL: SymmetryAxes = SymmetryAxes {
        vertical: true,
        horizontal: true,
        diagonal_down: true,
        diagonal_up: true,
    };

    fn mask(w: u32, h: u32, points: &[Point]) -> Mask {
        Mask::from_points(w, h, points.iter().copied())
    }

    fn sorted(mut v: Vec<Point>) -> Vec<Point> {
        v.sort();
        v
    }

    #[test]
    fn ops_invert_and_carry_offsets() {
        for op in mirror_ops(ALL) {
            assert_eq!(op.inverse().linear(op.linear((2, 1))), (2, 1), "{op:?}");
            assert_eq!(op.inverse().apply(op.apply((1, 2), (8, 8)), (8, 8)), (1, 2));
        }
        let ops = mirror_ops(V);
        assert!(ops[0].is_identity());
        assert_eq!(ops[1].linear((1, 3)), (-1, 3), "│ flips dx");
    }

    #[test]
    fn two_eyes_split_into_the_anchor_side_and_its_reflection() {
        // 8×8: eje vertical entre 3 y 4. Ojos en (1,1) y (6,1).
        let m = mask(8, 8, &[(1, 1), (6, 1)]);
        let plan = mirror_plan(&m, V, (8, 8), (3, 3)).unwrap();
        assert_eq!(
            plan.primary,
            vec![(1, 1)],
            "the left eye, next to the anchor"
        );
        assert_eq!(plan.copies.len(), 1);
        assert_eq!(plan.copies[0].apply((1, 1), (8, 8)), (6, 1));
        assert!(plan.fixed.is_empty());
        // Con el ancla a la derecha, lo primario es el ojo derecho.
        let plan = mirror_plan(&m, V, (8, 8), (13, 3)).unwrap();
        assert_eq!(plan.primary, vec![(6, 1)]);
        assert_eq!(plan.bounds(), Some(((6, 1), (6, 1))));
    }

    #[test]
    fn a_piece_across_the_axis_is_fixed_and_not_split() {
        // Una boca de x = 2 a 5 (cruza el eje entre 3 y 4) y los ojos.
        let mut points = vec![(1, 1), (6, 1)];
        points.extend((2..=5).map(|x| (x, 5)));
        let plan = mirror_plan(&mask(8, 8, &points), V, (8, 8), (3, 3)).unwrap();
        assert_eq!(plan.primary, vec![(1, 1)]);
        assert_eq!(plan.fixed.len(), 1);
        let mouth = &plan.fixed[0];
        assert_eq!(
            sorted(mouth.points.clone()),
            vec![(2, 5), (3, 5), (4, 5), (5, 5)]
        );
        assert_eq!(mouth.stabilizer.len(), 2);
        assert!(mouth.copies.is_empty());
        assert!(mouth.centered_x() && !mouth.centered_y() && !mouth.diagonal());
        // Solo se mueve a lo largo del eje.
        assert_eq!(project(&mouth.stabilizer, (3, 2)), (0, 2));
        assert_eq!(plan.bounds(), Some(((1, 1), (5, 5))));
    }

    #[test]
    fn a_pixel_on_the_axis_and_halves_that_touch_are_fixed() {
        // Ancho 7: la columna 3 es su propio reflejo.
        let plan = mirror_plan(&mask(7, 7, &[(3, 0)]), V, (7, 7), (1, 1)).unwrap();
        assert!(plan.primary.is_empty());
        assert_eq!(plan.fixed.len(), 1);
        // Dos mitades que se tocan en el eje (3 y 4 de un ancho 8): una sola pieza.
        let plan = mirror_plan(&mask(8, 8, &[(3, 2), (4, 2)]), V, (8, 8), (1, 1)).unwrap();
        assert!(plan.primary.is_empty());
        assert_eq!(plan.fixed[0].points.len(), 2);
        // Las mitades se tocan en diagonal (8 vecinos) con lo del medio: también es una sola.
        let points = [(2, 2), (3, 3), (4, 3), (5, 2)];
        let plan = mirror_plan(&mask(8, 8, &points), V, (8, 8), (1, 1)).unwrap();
        assert_eq!(plan.fixed.len(), 1);
        assert_eq!(plan.fixed[0].points.len(), 4);
    }

    #[test]
    fn with_both_straight_axes_a_mouth_moves_mirrored_with_its_twin() {
        // 8×8 con │ ─: boca abajo (cruza │) y su reflejo arriba; un ojo en cada cuadrante.
        let mut points = vec![(1, 1), (6, 1), (1, 6), (6, 6)];
        points.extend([(3, 5), (4, 5), (3, 2), (4, 2)]);
        let plan = mirror_plan(&mask(8, 8, &points), VH, (8, 8), (3, 3)).unwrap();
        assert_eq!(plan.primary, vec![(1, 1)], "top-left quadrant");
        assert_eq!(plan.copies.len(), 3);
        assert_eq!(plan.fixed.len(), 1);
        let mark = &plan.fixed[0];
        assert_eq!(
            sorted(mark.points.clone()),
            vec![(3, 2), (4, 2)],
            "the top one"
        );
        assert_eq!(mark.copies.len(), 1);
        let twin: Vec<_> = mark
            .points
            .iter()
            .map(|&p| mark.copies[0].apply(p, (8, 8)))
            .collect();
        assert_eq!(sorted(twin), vec![(3, 5), (4, 5)]);
        // Abajo, la boca baja cuando la marca de arriba sube.
        assert_eq!(project(&mark.stabilizer, (2, -1)), (0, -1));
        assert_eq!(mark.copies[0].linear((0, -1)), (0, 1));
    }

    #[test]
    fn diagonal_pieces_project_along_the_diagonal() {
        // ╲ en 8×8: (1,5) ↔ (5,1); la pieza (2,2)-(3,3) está sobre la diagonal.
        let points = [(1, 5), (5, 1), (2, 2), (3, 3)];
        let plan = mirror_plan(&mask(8, 8, &points), DOWN, (8, 8), (1, 11)).unwrap();
        assert_eq!(
            plan.primary,
            vec![(1, 5)],
            "below the diagonal, like the anchor"
        );
        let piece = &plan.fixed[0];
        assert!(piece.diagonal());
        assert_eq!(project(&piece.stabilizer, (1, 0)), (0, 0));
        assert_eq!(project(&piece.stabilizer, (1, 1)), (1, 1));
        assert_eq!(project(&piece.stabilizer, (3, 1)), (2, 2));
    }

    #[test]
    fn all_axes_give_eight_copies() {
        // (0, 2) y sus 7 reflejos no se tocan entre sí.
        let p = (0, 2);
        let points: Vec<_> = crate::symmetric_points(p, ALL, (8, 8));
        assert_eq!(points.len(), 8);
        let plan = mirror_plan(&mask(8, 8, &points), ALL, (8, 8), (1, 5)).unwrap();
        assert_eq!(plan.primary, vec![p]);
        assert_eq!(plan.copies.len(), 7);
        let copies: Vec<_> = plan.copies.iter().map(|op| op.apply(p, (8, 8))).collect();
        assert_eq!(copies.len(), 7);
        assert!(!copies.contains(&p));
        // El centro (con las 8 simetrías) no se mueve.
        let plan = mirror_plan(
            &mask(8, 8, &[(3, 3), (4, 3), (3, 4), (4, 4)]),
            ALL,
            (8, 8),
            (0, 0),
        );
        let center = &plan.unwrap().fixed[0];
        assert_eq!(project(&center.stabilizer, (5, -2)), (0, 0));
        assert!(center.centered_x() && center.centered_y() && center.diagonal());
    }

    #[test]
    fn no_plan_without_axes_or_for_an_asymmetric_selection() {
        let m = mask(8, 8, &[(1, 1)]);
        assert!(mirror_plan(&m, SymmetryAxes::NONE, (8, 8), (0, 0)).is_none());
        assert!(
            mirror_plan(&m, V, (8, 8), (0, 0)).is_none(),
            "the reflection is missing"
        );
        assert!(mirror_plan(&Mask::new(8, 8), V, (8, 8), (0, 0)).is_none());
        // ╲ en un lienzo ancho: el reflejo de (7,0) cae fuera.
        let m = mask(10, 4, &[(7, 0)]);
        assert!(mirror_plan(&m, DOWN, (10, 4), (0, 0)).is_none());
    }
}
