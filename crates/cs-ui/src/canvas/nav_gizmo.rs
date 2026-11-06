//! El gizmo de navegación de la perspectiva (como el de Blender, boceto `mockup-panel-split`):
//! arriba a la derecha del cuadrante Persp, tres bolitas de color (X rojo, Y verde, Z azul) unidas
//! al centro y orientadas con la cámara; las de los ejes negativos, más chicas y apagadas. Un clic
//! en una bolita pone la cámara mirando desde ese lado; arrastrarlo orbita como el botón derecho.

use cs_core::Camera3D;
use eframe::egui::{Pos2, Rect, pos2};

/// Una bolita: el eje (0 x, 1 y, 2 z) y si es la del lado negativo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct NavBall {
    pub axis: usize,
    pub negative: bool,
}

/// Como en el boceto: el centro a 39 puntos del borde derecho del cuadrante y a 35 del de
/// arriba, los ejes de 16 y las bolitas de radio 5 (las negativas, de 3).
const NAV_RIGHT: f32 = 39.0;
const NAV_TOP: f32 = 35.0;
const NAV_AXIS: f32 = 16.0;
const BALL_R: i32 = 5;
const SMALL_R: i32 = 3;
/// Hasta dónde (desde el centro) se agarra el gizmo para orbitar.
const NAV_REACH: f32 = NAV_AXIS + BALL_R as f32 + 2.0;

/// El centro del gizmo en el cuadrante `q` (en un punto entero).
pub(crate) fn nav_center(q: Rect) -> Pos2 {
    pos2((q.right() - NAV_RIGHT).round(), (q.top() + NAV_TOP).round())
}

impl NavBall {
    fn radius(self) -> i32 {
        if self.negative { SMALL_R } else { BALL_R }
    }

    /// La cámara de `start` mirando desde el lado de la bolita (X → Side, Y → Top, Z → Front;
    /// las negativas, del lado opuesto). Lo demás (proyección, distancia y objetivo) no cambia.
    pub(crate) fn camera(self, start: Camera3D) -> Camera3D {
        let (yaw, pitch) = match (self.axis, self.negative) {
            (0, false) => (900, 0),
            (0, true) => (-900, 0),
            (1, false) => (0, 900),
            (1, true) => (0, -900),
            (_, false) => (0, 0),
            (_, true) => (-1800, 0),
        };
        Camera3D {
            yaw,
            pitch,
            ..start
        }
    }
}

/// Las seis bolitas del gizmo de `camera` en el cuadrante `q`, de la más lejana a la más
/// cercana: cada una con su centro en pantalla (en puntos enteros).
pub(crate) fn nav_balls(camera: &Camera3D, q: Rect) -> Vec<(NavBall, Pos2)> {
    let [right, up, fwd] = cs_3d::Projector::new(camera, (1, 1)).axes();
    let c = nav_center(q);
    let mut out: Vec<(NavBall, Pos2, f32)> = Vec::with_capacity(6);
    for axis in 0..3 {
        for negative in [false, true] {
            let s = if negative { -1.0 } else { 1.0 };
            let at = pos2(
                c.x + (right[axis] * s * NAV_AXIS).round(),
                c.y - (up[axis] * s * NAV_AXIS).round(),
            );
            out.push((NavBall { axis, negative }, at, fwd[axis] * s));
        }
    }
    // Lo más adentro de la pantalla primero (lo de adelante se dibuja encima).
    out.sort_by(|a, b| b.2.total_cmp(&a.2));
    out.into_iter().map(|(b, p, _)| (b, p)).collect()
}

/// La bolita bajo `p` (la de más adelante, si se tapan).
pub(crate) fn nav_hit(camera: &Camera3D, q: Rect, p: Pos2) -> Option<NavBall> {
    nav_balls(camera, q)
        .into_iter()
        .rev()
        .find(|(b, at)| (*at - p).length() <= b.radius() as f32 + 1.0)
        .map(|(b, _)| b)
}

/// Si `p` cae en el gizmo (en una bolita o cerca del centro): ahí se orbita.
pub(crate) fn nav_area(camera: &Camera3D, q: Rect, p: Pos2) -> bool {
    (nav_center(q) - p).length() <= NAV_REACH || nav_hit(camera, q, p).is_some()
}

/// Las letras de 3 × 5 de las bolitas positivas.
const LETTERS: [[&str; 5]; 3] = [
    ["#.#", "#.#", ".#.", "#.#", "#.#"],
    ["#.#", "#.#", ".#.", ".#.", ".#."],
    ["###", "..#", ".#.", "#..", "###"],
];

/// `c` mezclado con `other` (`k` de `other`, de 0 a 1).
fn mix(c: cs_3d::Rgba, other: cs_3d::Rgba, k: f32) -> cs_3d::Rgba {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
    [m(c[0], other[0]), m(c[1], other[1]), m(c[2], other[2]), 255]
}

/// Los colores del gizmo: los de los ejes, el fondo de la perspectiva (para apagar las
/// negativas), el borde claro y el oscuro y el de la bolita bajo el puntero.
pub(crate) struct NavColors {
    pub axes: [cs_3d::Rgba; 3],
    pub background: cs_3d::Rgba,
    pub light: cs_3d::Rgba,
    pub dark: cs_3d::Rgba,
    pub hot: cs_3d::Rgba,
}

/// Un círculo lleno de radio `r` (pixel art: redondo sin escalones raros) centrado en `(x, y)`.
fn disc(img: &mut cs_3d::WireImage, (x, y): (i32, i32), r: i32, c: cs_3d::Rgba) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r + r {
                img.put(x + dx, y + dy, c);
            }
        }
    }
}

/// Una línea de `a` a `b` de `2·w + 1` pixeles de grueso (la de 1 corrida en cada dirección).
fn thick_line(img: &mut cs_3d::WireImage, a: (i32, i32), b: (i32, i32), w: i32, c: cs_3d::Rgba) {
    for dy in -w..=w {
        for dx in -w..=w {
            img.line(a.0 + dx, a.1 + dy, b.0 + dx, b.1 + dy, c);
        }
    }
}

/// Dibuja el gizmo de `camera` del cuadrante `q` en `img` (cuyo pixel `(0, 0)` es el punto
/// `min`): primero los bordes de las líneas, después de atrás hacia adelante cada línea y su
/// bolita (con borde oscuro y claro, como el boceto) y las negativas, chicas, sin borde ni
/// letra. La de `hot`, con el borde en ese color.
pub(crate) fn draw_nav(
    img: &mut cs_3d::WireImage,
    min: (f32, f32),
    camera: &Camera3D,
    q: Rect,
    hot: Option<NavBall>,
    colors: &NavColors,
) {
    let px = |p: Pos2| ((p.x - min.0).floor() as i32, (p.y - min.1).floor() as i32);
    let c = px(nav_center(q));
    let balls = nav_balls(camera, q);
    let positive = || balls.iter().filter(|(b, _)| !b.negative);
    for (_, at) in positive() {
        thick_line(img, c, px(*at), 2, colors.light);
    }
    for (_, at) in positive() {
        thick_line(img, c, px(*at), 1, colors.dark);
    }
    for (b, at) in &balls {
        let at = px(*at);
        let color = colors.axes[b.axis];
        let ring = if hot == Some(*b) {
            colors.hot
        } else {
            colors.light
        };
        // Las negativas, sin borde (solo bajo el puntero) y apagadas hacia el fondo.
        if b.negative {
            if hot == Some(*b) {
                disc(img, at, SMALL_R + 1, ring);
            }
            disc(img, at, SMALL_R, mix(color, colors.background, 0.5));
            continue;
        }
        img.line(c.0, c.1, at.0, at.1, color);
        disc(img, at, BALL_R + 2, ring);
        disc(img, at, BALL_R + 1, colors.dark);
        disc(img, at, BALL_R, color);
        let ink = mix(color, colors.dark, 0.45);
        for (row, line) in LETTERS[b.axis].iter().enumerate() {
            for (col, ch) in line.chars().enumerate() {
                if ch == '#' {
                    img.put(at.0 - 1 + col as i32, at.1 - 2 + row as i32, ink);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{FIXED, Projection, Vec3i};

    fn camera(yaw: i32, pitch: i32) -> Camera3D {
        Camera3D {
            projection: Projection::Perspective { fov: 600 },
            yaw,
            pitch,
            distance: 5 * FIXED,
            target: Vec3i::default(),
        }
    }

    const Q: Rect = Rect {
        min: pos2(100.0, 50.0),
        max: pos2(400.0, 250.0),
    };

    fn ball(balls: &[(NavBall, Pos2)], axis: usize, negative: bool) -> (usize, Pos2) {
        balls
            .iter()
            .enumerate()
            .find(|(_, (b, _))| *b == NavBall { axis, negative })
            .map(|(i, (_, p))| (i, *p))
            .expect("the ball")
    }

    #[test]
    fn the_balls_follow_the_camera_and_the_near_ones_go_last() {
        let c = nav_center(Q);
        assert_eq!(c, pos2(361.0, 85.0));
        // De frente: X a la derecha, Y arriba, Z hacia uno (la última, encima de todo) y -Z
        // detrás (la primera).
        let balls = nav_balls(&camera(0, 0), Q);
        assert_eq!(ball(&balls, 0, false).1, c + eframe::egui::vec2(16.0, 0.0));
        assert_eq!(ball(&balls, 1, false).1, c + eframe::egui::vec2(0.0, -16.0));
        assert_eq!(ball(&balls, 0, true).1, c + eframe::egui::vec2(-16.0, 0.0));
        assert_eq!(ball(&balls, 2, false), (5, c));
        assert_eq!(ball(&balls, 2, true), (0, c));
        // Desde el costado (+X): Z a la izquierda.
        let side = nav_balls(&camera(900, 0), Q);
        assert_eq!(ball(&side, 2, false).1, c + eframe::egui::vec2(-16.0, 0.0));
        // Encima, la de adelante gana el clic.
        assert_eq!(
            nav_hit(&camera(0, 0), Q, c),
            Some(NavBall {
                axis: 2,
                negative: false
            })
        );
        assert_eq!(
            nav_hit(&camera(0, 0), Q, c + eframe::egui::vec2(8.0, 8.0)),
            None
        );
        assert!(nav_area(&camera(0, 0), Q, c + eframe::egui::vec2(8.0, 8.0)));
        assert!(!nav_area(
            &camera(0, 0),
            Q,
            c + eframe::egui::vec2(30.0, 0.0)
        ));
    }

    #[test]
    fn a_ball_gives_the_camera_from_its_side() {
        let start = Camera3D {
            distance: 7 * FIXED,
            target: Vec3i { x: 1, y: 2, z: 3 },
            ..camera(300, 250)
        };
        for (axis, negative, want) in [
            (0, false, (900, 0)),
            (0, true, (-900, 0)),
            (1, false, (0, 900)),
            (1, true, (0, -900)),
            (2, false, (0, 0)),
            (2, true, (-1800, 0)),
        ] {
            let c = NavBall { axis, negative }.camera(start);
            assert_eq!((c.yaw, c.pitch), want);
            assert_eq!(
                (c.distance, c.target, c.projection),
                (start.distance, start.target, start.projection)
            );
            // La bolita queda en el centro, de frente.
            let balls = nav_balls(&c, Q);
            assert_eq!(
                ball(&balls, axis, negative),
                (5, nav_center(Q)),
                "{axis} {negative}"
            );
        }
    }
}
