//! Suavizado: promedio del vecindario de 3×3.

use cs_core::Rgba;

use crate::Point;

/// Promedio del vecindario de 3×3 de `p`. El color se pondera por alfa y el alfa es el promedio
/// simple. `get` devuelve `None` fuera del lienzo: esos vecinos no cuentan.
pub fn blur_average(p: Point, get: impl Fn(Point) -> Option<Rgba>) -> Rgba {
    let (mut n, mut a, mut r, mut g, mut b) = (0u32, 0u32, 0u32, 0u32, 0u32);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let Some(c) = get((p.0 + dx, p.1 + dy)) else {
                continue;
            };
            let ca = c.a as u32;
            n += 1;
            a += ca;
            r += c.r as u32 * ca;
            g += c.g as u32 * ca;
            b += c.b as u32 * ca;
        }
    }
    let alpha = (a + n / 2).checked_div(n).unwrap_or(0);
    if alpha == 0 {
        return Rgba::TRANSPARENT;
    }
    // Con alfa > 0 la suma `a` no es 0. Redondeo al entero más cercano.
    let channel = |sum: u32| ((sum + a / 2) / a) as u8;
    Rgba::new(channel(r), channel(g), channel(b), alpha as u8)
}

#[cfg(test)]
mod tests {
    use cs_core::Rgba;

    use super::*;

    /// Lienzo de `w`×`h` transparente con algunos píxeles pintados.
    fn canvas(w: i32, h: i32, painted: &[(Point, Rgba)]) -> impl Fn(Point) -> Option<Rgba> + use<> {
        let painted = painted.to_vec();
        move |p: Point| {
            (p.0 >= 0 && p.1 >= 0 && p.0 < w && p.1 < h).then(|| {
                painted
                    .iter()
                    .find(|(q, _)| *q == p)
                    .map_or(Rgba::TRANSPARENT, |&(_, c)| c)
            })
        }
    }

    #[test]
    fn a_lone_pixel_spreads_over_its_neighbourhood() {
        let get = canvas(5, 5, &[((2, 2), Rgba::rgb(255, 0, 0))]);
        // Un noveno del alfa (255 / 9 ≈ 28), con el color intacto.
        let soft = Rgba::new(255, 0, 0, 28);
        for y in 1..=3 {
            for x in 1..=3 {
                assert_eq!(blur_average((x, y), &get), soft, "({x}, {y})");
            }
        }
        assert_eq!(
            blur_average((0, 0), &get),
            Rgba::TRANSPARENT,
            "out of reach"
        );
    }

    #[test]
    fn a_flat_area_stays_the_same() {
        let c = Rgba::rgb(10, 20, 30);
        let all: Vec<(Point, Rgba)> = (0..3)
            .flat_map(|y| (0..3).map(move |x| ((x, y), c)))
            .collect();
        let get = canvas(3, 3, &all);
        for y in 0..3 {
            for x in 0..3 {
                assert_eq!(blur_average((x, y), &get), c, "({x}, {y})");
            }
        }
    }

    #[test]
    fn pixels_outside_the_canvas_do_not_count() {
        let get = canvas(
            2,
            1,
            &[
                ((0, 0), Rgba::rgb(0, 0, 0)),
                ((1, 0), Rgba::rgb(255, 255, 255)),
            ],
        );
        // Solo hay dos vecinos, los dos opacos. Si los de afuera contaran como transparentes,
        // el alfa bajaría.
        assert_eq!(blur_average((0, 0), &get), Rgba::new(128, 128, 128, 255));
    }

    #[test]
    fn color_is_weighted_by_alpha() {
        let get = canvas(
            2,
            1,
            &[
                ((0, 0), Rgba::rgb(255, 0, 0)),
                ((1, 0), Rgba::new(0, 0, 255, 0)),
            ],
        );
        // El azul transparente no tiñe; solo baja el alfa a la mitad.
        assert_eq!(blur_average((0, 0), &get), Rgba::new(255, 0, 0, 128));
    }
}
