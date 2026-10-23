//! Geometría de la caja de transformación: dónde caen los lados del original, con signo (volteos)
//! y ángulo. Pura: sin egui, en píxeles locales del artboard.

/// Caja de la transformación. `x0`/`y0` son el borde donde cae el lado izquierdo/superior del
/// original y `x1`/`y1` el del derecho/inferior (exclusivos). Con `x1 < x0` el contenido está
/// volteado. El ángulo (radianes, positivo = horario) gira alrededor del centro.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransformBox {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub angle: f32,
}

impl TransformBox {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x0: x,
            y0: y,
            x1: x + width as i32,
            y1: y + height as i32,
            angle: 0.0,
        }
    }

    /// Ancho con signo (negativo = volteado).
    pub fn width(&self) -> i32 {
        self.x1 - self.x0
    }

    pub fn height(&self) -> i32 {
        self.y1 - self.y0
    }

    /// Lado izquierdo en pantalla (sin rotar).
    pub fn left(&self) -> i32 {
        self.x0.min(self.x1)
    }

    pub fn top(&self) -> i32 {
        self.y0.min(self.y1)
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
            angle: self.angle,
        }
    }

    pub fn center(&self) -> (f32, f32) {
        (
            (self.x0 + self.x1) as f32 / 2.0,
            (self.y0 + self.y1) as f32 / 2.0,
        )
    }

    /// Punto del lienzo → marco de la caja sin rotar.
    pub fn to_frame(self, p: (f32, f32)) -> (f32, f32) {
        rotate_point(p, self.center(), -self.angle)
    }

    /// Punto de un lado o esquina en pantalla (`dirs` como `Handle::dirs`), sin rotar.
    fn unrotated(&self, dirs: (i32, i32)) -> (f32, f32) {
        let side = |d: i32, a: i32, b: i32| match d {
            -1 => a.min(b) as f32,
            0 => (a + b) as f32 / 2.0,
            _ => a.max(b) as f32,
        };
        (
            side(dirs.0, self.x0, self.x1),
            side(dirs.1, self.y0, self.y1),
        )
    }

    /// Posición del handle en el lienzo (rotada).
    pub fn handle_point(&self, handle: Handle) -> (f32, f32) {
        rotate_point(self.unrotated(handle.dirs()), self.center(), self.angle)
    }

    /// Esquinas rotadas en orden NW, NE, SE, SW.
    pub fn corners(&self) -> [(f32, f32); 4] {
        [Handle::NW, Handle::NE, Handle::SE, Handle::SW].map(|h| self.handle_point(h))
    }
}

/// Qué se arrastra sobre lo seleccionado.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransformHit {
    /// Dentro de la selección (o cualquier punto con Move): mover.
    Move,
    /// Un handle: escalar moviendo ese lado o esa esquina.
    Handle(Handle),
    /// Justo por fuera de una esquina: rotar sobre el centro.
    Rotate,
}

/// Uno de los 8 handles de la caja.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl Handle {
    pub const ALL: [Handle; 8] = [
        Handle::NW,
        Handle::N,
        Handle::NE,
        Handle::E,
        Handle::SE,
        Handle::S,
        Handle::SW,
        Handle::W,
    ];

    /// Lado horizontal y vertical: -1 izquierda/arriba, 0 centro, 1 derecha/abajo.
    pub fn dirs(self) -> (i32, i32) {
        match self {
            Handle::NW => (-1, -1),
            Handle::N => (0, -1),
            Handle::NE => (1, -1),
            Handle::E => (1, 0),
            Handle::SE => (1, 1),
            Handle::S => (0, 1),
            Handle::SW => (-1, 1),
            Handle::W => (-1, 0),
        }
    }
}

/// Gira `p` alrededor de `c` (positivo = horario, con y hacia abajo).
pub fn rotate_point(p: (f32, f32), c: (f32, f32), angle: f32) -> (f32, f32) {
    let (s, co) = angle.sin_cos();
    let (dx, dy) = (p.0 - c.0, p.1 - c.1);
    (c.0 + co * dx - s * dy, c.1 + s * dx + co * dy)
}

/// Tope de lo que se arrastra en un gesto (píxeles): la caja nunca se acerca al límite de `i32`.
/// El límite real del tamaño (`MAX_BLOCK_SIDE`) lo pone el editor.
const MAX_DRAG: i32 = 1 << 20;

/// Handle o franja de rotación bajo `p`. `handle` es medio lado del cuadrado de cada handle y
/// `band` el ancho de la franja de rotación fuera de las esquinas, los dos en píxeles locales.
/// Dentro de la caja no devuelve nada: mover lo decide el editor con la máscara.
// El lienzo la usa al conectar los handles (sabe el zoom); hasta entonces solo los tests.
#[allow(dead_code)]
pub fn hit(b: &TransformBox, p: (f32, f32), handle: f32, band: f32) -> Option<TransformHit> {
    let q = b.to_frame(p);
    let near = |h: (f32, f32), r: f32| (q.0 - h.0).abs() <= r && (q.1 - h.1).abs() <= r;
    if let Some(h) = Handle::ALL
        .into_iter()
        .find(|h| near(b.unrotated(h.dirs()), handle))
    {
        return Some(TransformHit::Handle(h));
    }
    let (l, t) = b.unrotated((-1, -1));
    let (r, bottom) = b.unrotated((1, 1));
    let outside = q.0 < l || q.0 > r || q.1 < t || q.1 > bottom;
    let corner = [Handle::NW, Handle::NE, Handle::SE, Handle::SW]
        .into_iter()
        .any(|h| near(b.unrotated(h.dirs()), band));
    (outside && corner).then_some(TransformHit::Rotate)
}

/// Mueve el borde `b` (o `a`) `d` píxeles sin dejar el tamaño en 0; `start_size` da el signo.
fn move_edge(a: &mut i32, b: &mut i32, moves_b: bool, d: i32, start_size: i32) {
    let sign = start_size.signum();
    if moves_b {
        *b += d;
        if *b == *a {
            *b = *a + sign;
        }
    } else {
        *a += d;
        if *a == *b {
            *a = *b - sign;
        }
    }
}

/// Deja el tamaño en `size` (positivo) moviendo `b` (o `a`), con el signo que ya tiene.
fn set_size(a: &mut i32, b: &mut i32, moves_b: bool, size: i32) {
    let sign = if *b >= *a { 1 } else { -1 };
    if moves_b {
        *b = *a + sign * size;
    } else {
        *a = *b - sign * size;
    }
}

/// Arrastrar `handle` de `from` a `to`: se mueve ese lado o esa esquina y el lado opuesto queda
/// quieto (también con la caja girada). Más allá del lado opuesto se voltea; el mínimo es 1 px.
/// `keep_ratio` (Shift) mantiene la proporción en las esquinas.
pub fn resize(
    start: &TransformBox,
    handle: Handle,
    from: (f32, f32),
    to: (f32, f32),
    keep_ratio: bool,
) -> TransformBox {
    let (hx, hy) = handle.dirs();
    let (a, b) = (start.to_frame(from), start.to_frame(to));
    let delta = |v: f32| (v.round() as i32).clamp(-MAX_DRAG, MAX_DRAG);
    let (dx, dy) = (delta(b.0 - a.0), delta(b.1 - a.1));
    let mut out = *start;
    // Qué campo es el lado que se arrastra: con volteo, x0 es el derecho en pantalla.
    let moves_x1 = (hx == 1) == (start.x1 >= start.x0);
    let moves_y1 = (hy == 1) == (start.y1 >= start.y0);
    if hx != 0 {
        move_edge(&mut out.x0, &mut out.x1, moves_x1, dx, start.width());
    }
    if hy != 0 {
        move_edge(&mut out.y0, &mut out.y1, moves_y1, dy, start.height());
    }
    if keep_ratio && hx != 0 && hy != 0 {
        let w0 = start.width().unsigned_abs() as f32;
        let h0 = start.height().unsigned_abs() as f32;
        let s =
            (out.width().unsigned_abs() as f32 / w0).max(out.height().unsigned_abs() as f32 / h0);
        let size = |v: f32| (v * s).round().clamp(1.0, MAX_DRAG as f32) as i32;
        set_size(&mut out.x0, &mut out.x1, moves_x1, size(w0));
        set_size(&mut out.y0, &mut out.y1, moves_y1, size(h0));
    }
    // Girada, el centro se corre al cambiar el tamaño: se compensa para que el lado fijo no se
    // mueva en el lienzo.
    let anchor = |bx: &TransformBox| {
        let (cx, cy) = bx.center();
        let ax = match hx {
            0 => cx,
            _ if moves_x1 => bx.x0 as f32,
            _ => bx.x1 as f32,
        };
        let ay = match hy {
            0 => cy,
            _ if moves_y1 => bx.y0 as f32,
            _ => bx.y1 as f32,
        };
        rotate_point((ax, ay), (cx, cy), bx.angle)
    };
    let (before, after) = (anchor(start), anchor(&out));
    out.translated(
        (before.0 - after.0).round() as i32,
        (before.1 - after.1).round() as i32,
    )
}

/// Arrastrar fuera de una esquina: el ángulo sigue al puntero alrededor del centro. `snap`
/// (Shift) ajusta a pasos de 15°.
pub fn rotate(start: &TransformBox, from: (f32, f32), to: (f32, f32), snap: bool) -> TransformBox {
    use std::f32::consts::{PI, TAU};
    let c = start.center();
    let turn = (to.1 - c.1).atan2(to.0 - c.0) - (from.1 - c.1).atan2(from.0 - c.0);
    let mut angle = (start.angle + turn).rem_euclid(TAU);
    if snap {
        let step = PI / 12.0;
        angle = ((angle / step).round() * step).rem_euclid(TAU);
    }
    TransformBox { angle, ..*start }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

    use super::*;

    #[test]
    fn box_sides_and_flips() {
        let b = TransformBox::new(2, 3, 4, 5);
        assert_eq!((b.width(), b.height(), b.left(), b.top()), (4, 5, 2, 3));
        let flipped = TransformBox { x0: 6, x1: 2, ..b };
        assert_eq!((flipped.width(), flipped.left()), (-4, 2));
        assert_eq!(b.translated(1, -1), TransformBox::new(3, 2, 4, 5));
    }

    #[test]
    fn handles_rotation_band_and_misses() {
        let b = TransformBox::new(2, 2, 4, 4);
        assert_eq!(
            hit(&b, (6.0, 6.0), 0.25, 0.5),
            Some(TransformHit::Handle(Handle::SE))
        );
        assert_eq!(
            hit(&b, (4.1, 1.9), 0.25, 0.5),
            Some(TransformHit::Handle(Handle::N))
        );
        assert_eq!(
            hit(&b, (4.0, 4.0), 0.25, 0.5),
            None,
            "inside: the editor moves it"
        );
        assert_eq!(hit(&b, (6.4, 1.6), 0.25, 0.5), Some(TransformHit::Rotate));
        assert_eq!(
            hit(&b, (7.0, 4.0), 0.25, 0.5),
            None,
            "beside a side, far from the corners"
        );
        // Girada 90°: el handle E queda abajo del centro.
        let r = TransformBox {
            angle: FRAC_PI_2,
            ..TransformBox::new(0, 0, 4, 2)
        };
        assert_eq!(
            hit(&r, (2.0, 3.0), 0.25, 0.5),
            Some(TransformHit::Handle(Handle::E))
        );
    }

    #[test]
    fn dragging_a_side_moves_only_that_side_and_flips_past_the_other() {
        let b = TransformBox::new(0, 0, 4, 2);
        let e = resize(&b, Handle::E, (4.0, 1.0), (7.0, 5.0), false);
        assert_eq!(
            e,
            TransformBox::new(0, 0, 7, 2),
            "E ignores the vertical motion"
        );
        let w = resize(&b, Handle::W, (0.0, 1.0), (6.0, 1.0), false);
        assert_eq!(
            (w.x0, w.x1, w.width()),
            (6, 4, -2),
            "past the other side: flipped"
        );
        let thin = resize(&b, Handle::E, (4.0, 1.0), (0.0, 1.0), false);
        assert_eq!(thin.width(), 1, "never 0 px");
        // Volteada, el lado derecho en pantalla es x0.
        let f = resize(&w, Handle::E, (6.0, 1.0), (8.0, 1.0), false);
        assert_eq!((f.x0, f.x1), (8, 4));
    }

    #[test]
    fn shift_on_a_corner_keeps_the_proportion() {
        let b = TransformBox::new(0, 0, 4, 2);
        let free = resize(&b, Handle::SE, (4.0, 2.0), (8.0, 3.0), false);
        assert_eq!(free, TransformBox::new(0, 0, 8, 3));
        let kept = resize(&b, Handle::SE, (4.0, 2.0), (8.0, 3.0), true);
        assert_eq!(kept, TransformBox::new(0, 0, 8, 4));
        let nw = resize(&b, Handle::NW, (0.0, 0.0), (-4.0, 0.0), true);
        assert_eq!(nw, TransformBox::new(-4, -2, 8, 4));
    }

    #[test]
    fn a_rotated_box_keeps_the_opposite_side_in_place() {
        let b = TransformBox {
            angle: FRAC_PI_2,
            ..TransformBox::new(0, 0, 4, 2)
        };
        let anchor = |bx: &TransformBox| {
            let p = (bx.x0 as f32, (bx.y0 + bx.y1) as f32 / 2.0);
            rotate_point(p, bx.center(), bx.angle)
        };
        // Girada 90°, el lado E está abajo: bajar el puntero 2 la ensancha 2.
        let r = resize(&b, Handle::E, (2.0, 3.0), (2.0, 5.0), false);
        assert_eq!(r.width(), 6);
        let (a, c) = (anchor(&b), anchor(&r));
        assert!(
            (a.0 - c.0).abs() < 1e-4 && (a.1 - c.1).abs() < 1e-4,
            "{a:?} {c:?}"
        );
    }

    #[test]
    fn rotating_follows_the_pointer_around_the_center_and_shift_snaps() {
        let b = TransformBox::new(0, 0, 4, 4);
        let quarter = rotate(&b, (5.0, 2.0), (2.0, 5.0), false);
        assert!((quarter.angle - FRAC_PI_2).abs() < 1e-5);
        assert_eq!(
            (quarter.x0, quarter.y0, quarter.x1, quarter.y1),
            (0, 0, 4, 4),
            "only the angle changes"
        );
        let back = rotate(&b, (5.0, 2.0), (2.0, -1.0), false);
        assert!(
            (back.angle - 3.0 * FRAC_PI_2).abs() < 1e-5,
            "counterclockwise wraps to 270°"
        );
        let a = 50f32.to_radians();
        let to = (2.0 + 3.0 * a.cos(), 2.0 + 3.0 * a.sin());
        let snapped = rotate(&b, (5.0, 2.0), to, true);
        assert!(
            (snapped.angle - FRAC_PI_4).abs() < 1e-5,
            "{}",
            snapped.angle
        );
    }
}
