use super::*;

/// El dibujo en texto: `.` vacío, `T` arriba, `L` izquierda, `R` derecha, `O` contorno y `H`
/// highlight; una fila por línea.
fn text(parts: &IsoParts) -> String {
    let mut out = String::new();
    for y in 0..parts.height as i32 {
        for x in 0..parts.width as i32 {
            out.push(match parts.get(x, y) {
                None => '.',
                Some(IsoPart::Top) => 'T',
                Some(IsoPart::Left) => 'L',
                Some(IsoPart::Right) => 'R',
                Some(IsoPart::Outline) => 'O',
                Some(IsoPart::Highlight) => 'H',
            });
        }
        out.push('\n');
    }
    out
}

fn rows(rows: &[&str]) -> String {
    rows.iter().map(|r| format!("{r}\n")).collect()
}

fn cube(left: u32, right: u32, height: u32, corner: IsoCorner) -> IsoBox {
    IsoBox {
        left,
        right,
        height,
        corner,
    }
}

#[test]
fn a_small_cube_with_a_two_pixel_corner() {
    let b = cube(2, 2, 2, IsoCorner::Two);
    assert_eq!(b.size(), (6, 6));
    assert_eq!(
        text(&iso_box(&b, true, true)),
        rows(&[
            "..OO..", //
            "OOTTOO", "OHTTHO", "OLHHRO", "OOLROO", "..OO..",
        ])
    );
}

#[test]
fn a_three_pixel_corner_has_a_center_column() {
    let b = cube(2, 2, 2, IsoCorner::Three);
    assert_eq!(b.size(), (7, 6));
    assert_eq!(
        text(&iso_box(&b, true, true)),
        rows(&[
            "..OOO..", //
            "OOTTTOO", "OHTTTHO", "OLHHHRO", "OOLHROO", "..OOO..",
        ])
    );
    // Sin highlight, la columna del medio es de la cara izquierda.
    assert_eq!(
        text(&iso_box(&b, true, false)),
        rows(&[
            "..OOO..", //
            "OOTTTOO", "OTTTTTO", "OLTTTRO", "OOLLROO", "..OOO..",
        ])
    );
}

#[test]
fn a_four_pixel_corner_has_a_flat_tip() {
    let b = cube(2, 2, 2, IsoCorner::Four);
    assert_eq!(b.size(), (8, 6));
    assert_eq!(
        text(&iso_box(&b, true, true)),
        rows(&[
            "..OOOO..", //
            "OOTTTTOO", "OHTTTTHO", "OLHHHHRO", "OOLLRROO", "..OOOO..",
        ])
    );
}

#[test]
fn the_classic_32_by_16_tile() {
    let tile = cube(8, 8, 0, IsoCorner::Four);
    assert_eq!(tile.size(), (32, 16));
    let parts = iso_box(&tile, false, false);
    let span = |y: i32| {
        let xs: Vec<i32> = (0..32).filter(|&x| parts.get(x, y).is_some()).collect();
        (xs[0], *xs.last().unwrap(), xs.len())
    };
    assert_eq!(span(0), (14, 17, 4));
    assert_eq!(span(7), (0, 31, 32));
    assert_eq!(span(8), (0, 31, 32));
    assert_eq!(span(15), (14, 17, 4));
    // Plana: todo es la cara de arriba (sin highlight aunque se pida: no hay costados).
    let flat = iso_box(&tile, false, true);
    assert!(flat.points().all(|(_, _, p)| p == IsoPart::Top));
}

#[test]
fn uneven_sides_split_the_faces_at_the_front_corner() {
    let b = cube(3, 1, 1, IsoCorner::Two);
    assert_eq!(b.size(), (6, 5));
    assert_eq!(
        text(&iso_box(&b, false, false)),
        rows(&[
            "....TT", //
            "..TTTT", "TTTTRR", "TTRR..", "LR....",
        ])
    );
}

#[test]
fn equal_sides_are_mirror_images() {
    for corner in IsoCorner::ALL {
        for (n, h) in [(1, 0), (3, 5), (5, 2)] {
            let parts = iso_box(&cube(n, n, h, corner), true, false);
            let w = parts.width as i32;
            for (x, y, p) in parts.points() {
                let mirrored = parts.get(w - 1 - x, y);
                let expected = match p {
                    IsoPart::Left => IsoPart::Right,
                    IsoPart::Right => IsoPart::Left,
                    other => other,
                };
                // La columna del medio (3 px, sin highlight) es de la cara izquierda.
                if corner == IsoCorner::Three && 2 * x + 1 == w {
                    continue;
                }
                assert_eq!(mirrored, Some(expected), "{corner:?} {n} {h} at ({x}, {y})");
            }
        }
    }
}

#[test]
fn the_edges_step_two_pixels_per_row() {
    for corner in IsoCorner::ALL {
        let b = cube(5, 3, 4, corner);
        let parts = iso_box(&b, true, false);
        let (l, r) = (5, 3);
        // El contorno de arriba: en cada fila, los píxeles con el de arriba vacío.
        for y in 1..(l + r) {
            let tops: Vec<i32> = (0..parts.width as i32)
                .filter(|&x| parts.get(x, y).is_some() && parts.get(x, y - 1).is_none())
                .collect();
            // Un tramo de 2 por cada arista de atrás que sigue en la fila.
            let edges = usize::from(y < l) + usize::from(y < r);
            assert_eq!(tops.len(), 2 * edges, "{corner:?} row {y}: {tops:?}");
            for pair in tops.chunks(2) {
                assert_eq!(pair[1], pair[0] + 1, "{corner:?} row {y}: {tops:?}");
            }
        }
    }
}

#[test]
fn the_outline_is_one_pixel_thick_on_the_vertical_sides() {
    let b = cube(3, 3, 6, IsoCorner::Three);
    let parts = iso_box(&b, true, false);
    let (_, h) = b.size();
    for y in 4..(h as i32 - 4) {
        assert_eq!(parts.get(0, y), Some(IsoPart::Outline), "row {y}");
        assert_eq!(parts.get(1, y), Some(IsoPart::Left), "row {y}");
        let w = parts.width as i32;
        assert_eq!(parts.get(w - 1, y), Some(IsoPart::Outline), "row {y}");
        assert_eq!(parts.get(w - 2, y), Some(IsoPart::Right), "row {y}");
    }
}

#[test]
fn zero_sides_count_as_one_unit() {
    assert_eq!(
        cube(0, 0, 0, IsoCorner::Two).size(),
        cube(1, 1, 0, IsoCorner::Two).size()
    );
    assert_eq!(cube(1, 1, 0, IsoCorner::Two).size(), (2, 2));
    assert_eq!(IsoCorner::from_pixels(3), Some(IsoCorner::Three));
    assert_eq!(IsoCorner::from_pixels(5), None);
}

/// Luminancia entera, para comparar tonos.
fn luma(c: Rgba) -> u32 {
    299 * c.r as u32 + 587 * c.g as u32 + 114 * c.b as u32
}

#[test]
fn shades_from_a_base_color_go_light_on_top_and_dark_on_the_right() {
    for base in [
        Rgba::rgb(200, 80, 60),
        Rgba::rgb(60, 140, 200),
        Rgba::rgb(128, 128, 128),
        Rgba::rgb(90, 200, 90),
        Rgba::rgb(110, 60, 150),
    ] {
        let c = IsoColors::from_base(base);
        assert_eq!(c.left, base);
        assert!(luma(c.highlight) > luma(c.top), "{base:?}: {c:?}");
        assert!(luma(c.top) > luma(c.left), "{base:?}: {c:?}");
        assert!(luma(c.left) > luma(c.right), "{base:?}: {c:?}");
        assert!(luma(c.right) > luma(c.outline), "{base:?}: {c:?}");
    }
    // Un gris queda gris (sin tinte) y el alfa del base se conserva.
    let gray = IsoColors::from_base(Rgba::new(128, 128, 128, 200));
    for c in [gray.top, gray.right, gray.outline, gray.highlight] {
        assert!(c.r == c.g && c.g == c.b, "{c:?}");
        assert_eq!(c.a, 200);
    }
}

#[test]
fn each_part_takes_its_color() {
    let c = IsoColors {
        top: Rgba::rgb(1, 0, 0),
        left: Rgba::rgb(2, 0, 0),
        right: Rgba::rgb(3, 0, 0),
        outline: Rgba::rgb(4, 0, 0),
        highlight: Rgba::rgb(5, 0, 0),
    };
    assert_eq!(c.color(IsoPart::Top), c.top);
    assert_eq!(c.color(IsoPart::Left), c.left);
    assert_eq!(c.color(IsoPart::Right), c.right);
    assert_eq!(c.color(IsoPart::Outline), c.outline);
    assert_eq!(c.color(IsoPart::Highlight), c.highlight);
    let parts = iso_box(&cube(2, 2, 2, IsoCorner::Two), true, true);
    let pixels = paint_parts(&parts, &c);
    assert_eq!(pixels.len(), 36);
    assert_eq!(pixels[0], None);
    assert_eq!(pixels[2], Some(c.outline));
    // (2, 1) es la cara de arriba; (1, 3), la izquierda.
    assert_eq!(pixels[6 + 2], Some(c.top));
    assert_eq!(pixels[3 * 6 + 1], Some(c.left));
}

#[test]
fn colors_snap_to_the_nearest_opaque_palette_entry() {
    let palette = [
        Rgba::new(0, 0, 0, 0),
        Rgba::rgb(10, 10, 10),
        Rgba::rgb(250, 250, 250),
        Rgba::rgb(200, 0, 0),
    ];
    let c = IsoColors::from_base(Rgba::rgb(190, 20, 20)).snapped(&palette);
    assert_eq!(c.left, Rgba::rgb(200, 0, 0));
    assert_eq!(c.outline, Rgba::rgb(10, 10, 10));
    for color in [c.top, c.left, c.right, c.outline, c.highlight] {
        assert!(palette[1..].contains(&color), "{color:?}");
    }
    // Sin colores opacos, no cambia nada.
    let base = IsoColors::from_base(Rgba::rgb(190, 20, 20));
    assert_eq!(base.snapped(&palette[..1]), base);
}

/// Una rejilla de `columns × rows` con las alturas dadas por filas.
fn grid(
    columns: u32,
    heights: &[u32],
    size: u32,
    block_height: u32,
    corner: IsoCorner,
) -> IsoBlocks {
    IsoBlocks {
        columns,
        rows: heights.len() as u32 / columns,
        heights: heights.to_vec(),
        size,
        block_height,
        corner,
    }
}

#[test]
fn one_block_is_a_box() {
    for corner in IsoCorner::ALL {
        for (outline, highlight) in [(true, true), (true, false), (false, true), (false, false)] {
            let blocks = grid(1, &[1], 3, 5, corner);
            assert_eq!(
                iso_blocks(&blocks, outline, highlight),
                iso_box(&cube(3, 3, 5, corner), outline, highlight),
                "{corner:?} {outline} {highlight}"
            );
        }
    }
}

#[test]
fn stacked_blocks_are_a_taller_box() {
    for corner in IsoCorner::ALL {
        for highlight in [false, true] {
            assert_eq!(
                iso_blocks(&grid(1, &[3], 2, 4, corner), true, highlight),
                iso_box(&cube(2, 2, 12, corner), true, highlight),
                "{corner:?} {highlight}"
            );
        }
    }
}

#[test]
fn blocks_side_by_side_are_a_wider_box() {
    for corner in [IsoCorner::Two, IsoCorner::Four] {
        // Dos columnas: sigue la arista de atrás-derecha.
        assert_eq!(
            iso_blocks(&grid(2, &[1, 1], 2, 3, corner), true, false),
            iso_box(&cube(2, 4, 3, corner), true, false),
            "{corner:?} columns"
        );
        // Dos filas: la de atrás-izquierda.
        assert_eq!(
            iso_blocks(&grid(1, &[1, 1], 2, 3, corner), true, false),
            iso_box(&cube(4, 2, 3, corner), true, false),
            "{corner:?} rows"
        );
        // La junta de arriba no se marca con highlight: solo el borde de adelante.
        assert_eq!(
            iso_blocks(&grid(2, &[1, 1], 2, 3, corner), true, true),
            iso_box(&cube(2, 4, 3, corner), true, true),
            "{corner:?} highlight"
        );
    }
}

#[test]
fn a_block_in_front_hides_the_one_behind() {
    // El segundo bloque de la celda de adelante (1, 1) cae justo sobre el de la (0, 0).
    let both = grid(2, &[1, 0, 0, 2], 2, 4, IsoCorner::Three);
    let front = grid(2, &[0, 0, 0, 2], 2, 4, IsoCorner::Three);
    assert_eq!(
        iso_blocks(&both, true, true),
        iso_blocks(&front, true, true)
    );
}

#[test]
fn a_step_shows_the_highlight_of_the_higher_block() {
    // Atrás dos bloques, adelante (a la derecha) uno: el borde de adelante del alto se ve sobre
    // su costado derecho, que asoma encima del bajo.
    let step = iso_blocks(&grid(2, &[2, 1], 2, 4, IsoCorner::Two), true, true);
    assert!(step.points().any(|(_, _, p)| p == IsoPart::Highlight));
    assert!(step.points().any(|(_, _, p)| p == IsoPart::Right));
}

#[test]
fn an_empty_grid_draws_nothing() {
    let empty = grid(2, &[0, 0, 0, 0], 2, 4, IsoCorner::Two);
    let parts = iso_blocks(&empty, true, true);
    assert_eq!((parts.width, parts.height), (0, 0));
}

#[test]
fn resizing_the_grid_keeps_what_fits() {
    let mut g = grid(2, &[1, 2, 3, 4], 2, 4, IsoCorner::Two);
    g.resize(3, 1);
    assert_eq!((g.columns, g.rows), (3, 1));
    assert_eq!(g.heights, vec![1, 2, 0]);
    assert_eq!(g.height_at(1, 0), 2);
    assert_eq!(g.height_at(5, 5), 0);
    g.set_height(2, 0, 7);
    assert_eq!(g.heights, vec![1, 2, 7]);
}
