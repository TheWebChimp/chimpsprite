//! Con Pixel UI, la selección del diseño: el borde de 1 punto con tramos de 4 blancos y 4
//! negros (siguen la diagonal `x + y`, así los lados empalman en las esquinas) y los handles de
//! 5×5, con borde de tinta y centro blanco, por fuera de la caja.

use eframe::egui::{Color32, Painter, Pos2, Rect, Vec2};

use crate::pixel_cursors::INK;

/// Largo de cada tramo del borde.
const DASH: i32 = 4;
/// Lado de cada handle.
pub const HANDLE: f32 = 5.0;

/// Color del borde en el punto `(x, y)` de pantalla; `phase` lo hace avanzar (marching ants).
fn ant(x: i32, y: i32, phase: i32) -> Color32 {
    if (x + y - phase).rem_euclid(2 * DASH) < DASH {
        Color32::WHITE
    } else {
        INK
    }
}

/// Tramos de un lado de `len` puntos que empieza en `(x, y)` y avanza por `(dx, dy)`: un rect por
/// tramo del mismo color.
fn line(x: i32, y: i32, (dx, dy): (i32, i32), len: i32, phase: i32) -> Vec<(Rect, Color32)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < len {
        let (px, py) = (x + dx * i, y + dy * i);
        let color = ant(px, py, phase);
        // Hasta donde sigue el mismo color (o termina el lado).
        let mut n = 1;
        while i + n < len && ant(px + dx * n, py + dy * n, phase) == color {
            n += 1;
        }
        let size = Vec2::new((dx * (n - 1) + 1) as f32, (dy * (n - 1) + 1) as f32);
        out.push((
            Rect::from_min_size(Pos2::new(px as f32, py as f32), size),
            color,
        ));
        i += n;
    }
    out
}

/// Fila de `x0` a `x1` (incluidos) en `y`.
pub fn hline(x0: i32, x1: i32, y: i32, phase: i32) -> Vec<(Rect, Color32)> {
    line(x0.min(x1), y, (1, 0), (x1 - x0).abs() + 1, phase)
}

/// Columna de `y0` a `y1` (incluidos) en `x`.
pub fn vline(x: i32, y0: i32, y1: i32, phase: i32) -> Vec<(Rect, Color32)> {
    line(x, y0.min(y1), (0, 1), (y1 - y0).abs() + 1, phase)
}

/// Un punto de pantalla redondeado.
fn snap(v: f32) -> i32 {
    v.round() as i32
}

/// Borde de `r` (en pantalla): arriba y a la izquierda en la primera fila y columna de adentro;
/// abajo y a la derecha, en la de afuera (como cada lado de píxel del borde de una máscara).
pub fn rect_outline(r: Rect, phase: i32) -> Vec<(Rect, Color32)> {
    let (l, t, rr, b) = (
        snap(r.left()),
        snap(r.top()),
        snap(r.right()),
        snap(r.bottom()),
    );
    let mut out = hline(l, rr, t, phase);
    out.extend(hline(l, rr, b, phase));
    out.extend(vline(l, t, b, phase));
    out.extend(vline(rr, t, b, phase));
    out
}

/// Los 8 handles de `r` por fuera de la caja: los de las esquinas tocan la esquina en diagonal y
/// los del medio quedan centrados en su lado. `dirs` son las del handle (-1, 0 o 1 por eje).
pub fn handle_rect(r: Rect, (dx, dy): (i32, i32)) -> Rect {
    let (l, t, rr, b) = (
        snap(r.left()),
        snap(r.top()),
        snap(r.right()),
        snap(r.bottom()),
    );
    let side = HANDLE as i32;
    let at = |d: i32, lo: i32, hi: i32| match d {
        d if d < 0 => lo - side,
        0 => (lo + hi) / 2 - side / 2,
        _ => hi,
    };
    Rect::from_min_size(
        Pos2::new(at(dx, l, rr) as f32, at(dy, t, b) as f32),
        Vec2::splat(HANDLE),
    )
}

/// Un handle: borde de tinta y centro blanco.
pub fn draw_handle(painter: &Painter, r: Rect) {
    painter.rect_filled(r, 0.0, INK);
    painter.rect_filled(r.shrink(1.0), 0.0, Color32::WHITE);
}

/// Pinta los tramos de un borde.
pub fn paint(painter: &Painter, dashes: Vec<(Rect, Color32)>) {
    for (r, c) in dashes {
        painter.rect_filled(r, 0.0, c);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo pintado como en el diseño: `o` blanco, `#` tinta, `.` nada; relativo a `origin`.
    fn ascii(dashes: &[(Rect, Color32)], origin: (i32, i32), size: (i32, i32)) -> Vec<String> {
        (0..size.1)
            .map(|y| {
                (0..size.0)
                    .map(|x| {
                        let p = Pos2::new((origin.0 + x) as f32 + 0.5, (origin.1 + y) as f32 + 0.5);
                        match dashes.iter().rev().find(|(r, _)| r.contains(p)) {
                            Some((_, c)) if *c == INK => '#',
                            Some(_) => 'o',
                            None => '.',
                        }
                    })
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_outline_is_4_white_4_black_along_the_diagonal_like_the_design() {
        // La caja del diseño: de (25, 14) a (231, 84).
        let r = Rect::from_min_max(Pos2::new(25.0, 14.0), Pos2::new(231.0, 84.0));
        let dashes = rect_outline(r, 0);
        assert_eq!(
            ascii(&dashes, (25, 14), (12, 10)),
            [
                "#oooo####ooo",
                "o...........",
                "o...........",
                "o...........",
                "o...........",
                "#...........",
                "#...........",
                "#...........",
                "#...........",
                "o...........",
            ]
        );
        // Abajo a la derecha, como en el diseño (ahí el diseño tapa la esquina con el handle).
        assert_eq!(
            ascii(&dashes, (225, 80), (7, 5)),
            ["......#", "......o", "......o", "......o", "###oooo",]
        );
    }

    #[test]
    fn the_ants_march_with_the_phase() {
        let still = hline(0, 7, 0, 0);
        let moved = hline(0, 7, 0, 1);
        assert_ne!(still, moved);
        assert_eq!(ascii(&moved, (0, 0), (8, 1)), ["#oooo###"]);
    }

    #[test]
    fn each_dash_is_one_rect() {
        assert_eq!(hline(0, 15, 0, 0).len(), 4);
        assert_eq!(vline(3, 0, 15, 0).len(), 5, "starts mid-dash");
    }

    #[test]
    fn the_handles_sit_outside_the_box_like_the_design() {
        let r = Rect::from_min_max(Pos2::new(25.0, 14.0), Pos2::new(231.0, 84.0));
        let at = |d| handle_rect(r, d).min;
        assert_eq!(at((-1, -1)), Pos2::new(20.0, 9.0));
        assert_eq!(at((0, -1)), Pos2::new(126.0, 9.0));
        assert_eq!(at((1, -1)), Pos2::new(231.0, 9.0));
        assert_eq!(at((-1, 0)), Pos2::new(20.0, 47.0));
        assert_eq!(at((1, 0)), Pos2::new(231.0, 47.0));
        assert_eq!(at((-1, 1)), Pos2::new(20.0, 84.0));
        assert_eq!(at((0, 1)), Pos2::new(126.0, 84.0));
        assert_eq!(at((1, 1)), Pos2::new(231.0, 84.0));
        assert_eq!(handle_rect(r, (1, 1)).size(), Vec2::splat(5.0));
    }
}
