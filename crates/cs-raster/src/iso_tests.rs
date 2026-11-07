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
