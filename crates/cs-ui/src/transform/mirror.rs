//! Lo flotante en espejo (selección espejo): lo primario se transforma con la caja, cada reflejo
//! con la transformación conjugada (`op ∘ T ∘ op⁻¹`) y su propio contenido, y las piezas sobre los
//! ejes quedan simétricas. Ver `cs_raster::mirror_plan`.

use cs_core::PixelValue;
use cs_raster::{Block, MirrorOp, MirrorPlan, RotationAlgorithm};

use crate::transform_box::TransformBox;

/// Píxeles sueltos con su valor, en coordenadas del lienzo.
type Pixels = Vec<(i32, i32, PixelValue)>;

/// Lo levantado en espejo, además de lo primario (que es el `source` de `Floating`).
#[derive(Clone, Debug)]
pub(crate) struct MirrorFloat {
    /// Centro de simetría en medios píxeles.
    pub(crate) center2: (i32, i32),
    /// Todos los píxeles levantados: el hueco al confirmar o borrar.
    pub(crate) hole: Vec<(i32, i32)>,
    /// Los reflejos de lo primario: su simetría y su contenido traído al marco de lo primario
    /// (mismo tamaño y esquina que `Floating::source`).
    copies: Vec<(MirrorOp, Block<PixelValue>)>,
    /// Las piezas sobre los ejes.
    fixed: Vec<FixedFloat>,
    /// El valor transparente del sprite: no tapa lo de otra pieza.
    clear: PixelValue,
    /// Todas las simetrías del grupo (para saber qué reflejo se agarra).
    pub(crate) ops: Vec<MirrorOp>,
}

/// Una pieza sobre los ejes, levantada.
#[derive(Clone, Debug)]
struct FixedFloat {
    /// Esquina de la representante.
    origin: (i32, i32),
    /// El contenido de cada copia (la identidad primero), en el marco de la representante.
    sources: Vec<(MirrorOp, Block<PixelValue>)>,
    stabilizer: Vec<MirrorOp>,
    centered_x: bool,
    centered_y: bool,
    diagonal: bool,
}

/// Bloque con los valores de `points` (vistos por `op`) en el marco que empieza en `origin`.
fn pulled_back(
    points: &[(i32, i32)],
    origin: (i32, i32),
    size: (u32, u32),
    op: MirrorOp,
    center2: (i32, i32),
    sample: &impl Fn((i32, i32)) -> PixelValue,
) -> Block<PixelValue> {
    let mut block = Block::new(size.0, size.1);
    for &p in points {
        block.set(
            p.0 - origin.0,
            p.1 - origin.1,
            Some(sample(op.apply(p, center2))),
        );
    }
    block
}

/// Esquina y tamaño de `points` (no vacío).
fn frame(points: &[(i32, i32)]) -> ((i32, i32), (u32, u32)) {
    let x0 = points.iter().map(|p| p.0).min().unwrap_or(0);
    let y0 = points.iter().map(|p| p.1).min().unwrap_or(0);
    let x1 = points.iter().map(|p| p.0).max().unwrap_or(x0);
    let y1 = points.iter().map(|p| p.1).max().unwrap_or(y0);
    ((x0, y0), ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
}

/// Un lado de la caja de una pieza fija, llevado por la caja de lo primario (`from` → `to`, los
/// dos bordes de cada eje). Centrada (`center2`): el mismo ancho, con la paridad del centro, y
/// re-centrada en el eje. Devuelve los bordes con signo (volteada si `b < a`).
fn fixed_side(
    (a, b): (i32, i32),
    from: (i32, i32),
    to: (i32, i32),
    centered: Option<i32>,
) -> (i32, i32) {
    let span = (from.1 - from.0) as f32;
    let map = |v: i32| {
        if span == 0.0 {
            v + to.0 - from.0
        } else {
            (to.0 as f32 + (v - from.0) as f32 * (to.1 - to.0) as f32 / span).round() as i32
        }
    };
    let (na, mut nb) = (map(a), map(b));
    if na == nb {
        // Nunca de ancho cero: un píxel en el sentido de la caja.
        nb = na + if to.1 < to.0 { -1 } else { 1 };
    }
    let Some(c2) = centered else {
        return (na, nb);
    };
    let original = b - a;
    let mut size = (nb - na).abs();
    if (size - c2).rem_euclid(2) != 0 {
        size += if size >= original { 1 } else { -1 };
    }
    if size < 1 {
        size += 2;
    }
    let left = (c2 - size) / 2;
    if nb < na {
        (left + size, left)
    } else {
        (left, left + size)
    }
}

impl MirrorFloat {
    /// Levanta según `plan`: devuelve el contenido de lo primario (en el marco de `bounds`, la
    /// caja), su esquina y lo demás. `sample` lee la capa.
    pub(crate) fn lift(
        plan: &MirrorPlan,
        hole: Vec<(i32, i32)>,
        clear: PixelValue,
        ops: Vec<MirrorOp>,
        sample: impl Fn((i32, i32)) -> PixelValue,
    ) -> Option<(Block<PixelValue>, (i32, i32), MirrorFloat)> {
        let ((x0, y0), (x1, y1)) = plan.bounds()?;
        let origin = (x0, y0);
        let size = ((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        let c2 = plan.center2;
        let source = pulled_back(&plan.primary, origin, size, MirrorOp::IDENTITY, c2, &sample);
        let copies = plan
            .copies
            .iter()
            .map(|&op| {
                (
                    op,
                    pulled_back(&plan.primary, origin, size, op, c2, &sample),
                )
            })
            .collect();
        let fixed = plan
            .fixed
            .iter()
            .map(|piece| {
                let (origin, size) = frame(&piece.points);
                let sources = std::iter::once(MirrorOp::IDENTITY)
                    .chain(piece.copies.iter().copied())
                    .map(|op| {
                        (
                            op,
                            pulled_back(&piece.points, origin, size, op, c2, &sample),
                        )
                    })
                    .collect();
                FixedFloat {
                    origin,
                    sources,
                    stabilizer: piece.stabilizer.clone(),
                    centered_x: piece.centered_x(),
                    centered_y: piece.centered_y(),
                    diagonal: piece.diagonal(),
                }
            })
            .collect();
        let float = MirrorFloat {
            center2: c2,
            hole,
            copies,
            fixed,
            clear,
            ops,
        };
        Some((source, origin, float))
    }

    fn is_clear(&self, v: PixelValue) -> bool {
        v == self.clear || matches!(v, PixelValue::Rgba(c) if c.a == 0)
    }

    /// Todo transformado: lo primario (`source`, que estaba en la caja `start`) con la caja `b`,
    /// sus reflejos conjugados y las piezas fijas. Lo primario se pinta al final (gana en los
    /// solapes) y lo transparente no tapa lo opaco de otra pieza. Devuelve el bloque y su esquina.
    pub(crate) fn render(
        &self,
        source: &Block<PixelValue>,
        start: TransformBox,
        b: TransformBox,
        algorithm: RotationAlgorithm,
    ) -> (Block<PixelValue>, (i32, i32)) {
        let c2 = self.center2;
        let transformed = |block: &Block<PixelValue>| {
            let scaled = cs_raster::scale_nearest(block, b.width(), b.height());
            let (rotated, (dx, dy)) = algorithm.rotate(&scaled, b.angle);
            let at = (b.left() + dx, b.top() + dy);
            rotated
                .points()
                .map(|(x, y, v)| (at.0 + x, at.1 + y, v))
                .collect::<Vec<_>>()
        };
        let pushed = |points: Pixels, op: MirrorOp| {
            points.into_iter().map(move |(x, y, v)| {
                let (qx, qy) = op.apply((x, y), c2);
                (qx, qy, v)
            })
        };
        let mut layers: Vec<Pixels> = Vec::new();
        for (op, block) in &self.copies {
            layers.push(pushed(transformed(block), *op).collect());
        }
        // Las piezas fijas: sus copias primero y la representante después.
        let mut fixed_own = Vec::new();
        for piece in &self.fixed {
            let rendered = self.render_fixed(piece, start, b);
            for (i, (op, points)) in rendered.into_iter().enumerate() {
                let points: Vec<_> = pushed(points, op).collect();
                if i == 0 {
                    fixed_own.push(points);
                } else {
                    layers.push(points);
                }
            }
        }
        layers.extend(fixed_own);
        layers.push(transformed(source));
        self.compose(layers)
    }

    /// Una pieza fija en el marco de su representante, por copia (identidad primero).
    fn render_fixed(
        &self,
        piece: &FixedFloat,
        start: TransformBox,
        b: TransformBox,
    ) -> Vec<(MirrorOp, Pixels)> {
        let Some((_, first)) = piece.sources.first() else {
            return Vec::new();
        };
        let (w, h) = (first.width as i32, first.height as i32);
        let (ox, oy) = piece.origin;
        let (x, y) = if piece.diagonal {
            // Solo se traslada: el corrimiento del centro de la caja, proyectado.
            let d = (
                (b.x0 + b.x1 - start.x0 - start.x1) / 2,
                (b.y0 + b.y1 - start.y0 - start.y1) / 2,
            );
            let (dx, dy) = cs_raster::project(&piece.stabilizer, d);
            ((ox + dx, ox + dx + w), (oy + dy, oy + dy + h))
        } else {
            let center = |on: bool, c: i32| on.then_some(c);
            (
                fixed_side(
                    (ox, ox + w),
                    (start.x0, start.x1),
                    (b.x0, b.x1),
                    center(piece.centered_x, self.center2.0),
                ),
                fixed_side(
                    (oy, oy + h),
                    (start.y0, start.y1),
                    (b.y0, b.y1),
                    center(piece.centered_y, self.center2.1),
                ),
            )
        };
        let (left, top) = (x.0.min(x.1), y.0.min(y.1));
        piece
            .sources
            .iter()
            .map(|(op, block)| {
                let scaled = cs_raster::scale_nearest(block, x.1 - x.0, y.1 - y.0);
                let points = scaled
                    .points()
                    .map(|(px, py, v)| (left + px, top + py, v))
                    .collect();
                (*op, points)
            })
            .collect()
    }

    /// Junta las capas en un bloque: cada una encima de las anteriores, salvo lo transparente.
    fn compose(&self, layers: Vec<Pixels>) -> (Block<PixelValue>, (i32, i32)) {
        let all = || layers.iter().flatten();
        let (Some(x0), Some(y0), Some(x1), Some(y1)) = (
            all().map(|p| p.0).min(),
            all().map(|p| p.1).min(),
            all().map(|p| p.0).max(),
            all().map(|p| p.1).max(),
        ) else {
            return (Block::new(0, 0), (0, 0));
        };
        let mut block = Block::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        for &(x, y, v) in all() {
            let (bx, by) = (x - x0, y - y0);
            if block.get(bx, by).is_none() || !self.is_clear(v) {
                block.set(bx, by, Some(v));
            }
        }
        (block, (x0, y0))
    }

    /// Qué copia está en el píxel `p`: la simetría que lo lleva más cerca del centro de la caja
    /// de lo primario (`box2`, en medios píxeles). Lo de lo primario da la identidad.
    pub(crate) fn op_at(&self, p: (i32, i32), box2: (i32, i32)) -> MirrorOp {
        let distance = |op: &MirrorOp| {
            let (x, y) = op.inverse().apply(p, self.center2);
            let (dx, dy) = ((2 * x + 1 - box2.0) as i64, (2 * y + 1 - box2.1) as i64);
            dx * dx + dy * dy
        };
        self.ops
            .iter()
            .copied()
            .min_by_key(distance)
            .unwrap_or(MirrorOp::IDENTITY)
    }
}

#[cfg(test)]
mod tests {
    use super::fixed_side;

    #[test]
    fn a_fixed_side_follows_the_box_and_stays_centered() {
        // La caja de lo primario se corre 2: la pieza libre en ese eje también.
        assert_eq!(fixed_side((2, 6), (0, 6), (2, 8), None), (4, 8));
        // Centrada en el eje (8 = entre 3 y 4): no se mueve aunque la caja se corra.
        assert_eq!(fixed_side((2, 6), (0, 6), (2, 8), Some(8)), (2, 6));
        // Escalar la caja al doble desde la izquierda: el ancho 4 pasa a 8, centrado.
        assert_eq!(fixed_side((2, 6), (0, 6), (0, 12), Some(8)), (0, 8));
        // Con centro impar (7), el ancho queda impar: 3 → 4 por la escala → 5.
        assert_eq!(fixed_side((2, 5), (0, 7), (0, 8), Some(7)), (1, 6));
        // Volteada: los bordes se invierten.
        assert_eq!(fixed_side((2, 6), (0, 6), (6, 0), Some(8)), (6, 2));
    }
}
