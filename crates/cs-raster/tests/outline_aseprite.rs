//! `outline_mask` contra Edit → FX → Outline de Aseprite 1.3.18, píxel a píxel. El fixture lo
//! genera `fixtures/outline_aseprite.lua` (ver su encabezado para el formato y cómo correrlo).

use cs_raster::{OutlineMatrix, OutlinePlace, OutlineShape, outline_mask};

struct Case {
    name: String,
    shape: OutlineShape,
    bg: bool,
    input: Vec<Vec<u8>>,
    expected: Vec<Vec<u8>>,
}

fn cases() -> Vec<Case> {
    let text = include_str!("fixtures/outline_aseprite.txt");
    let mut cases = Vec::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let name = line.strip_prefix("== ").expect("case header").to_string();
        let mut shape = OutlineShape::default();
        let mut bg = false;
        for field in lines.next().expect("params").split(' ') {
            let (k, v) = field.split_once('=').expect("k=v");
            match k {
                "place" => {
                    shape.place = match v {
                        "outside" => OutlinePlace::Outside,
                        _ => OutlinePlace::Inside,
                    }
                }
                "matrix" => shape.matrix = OutlineMatrix::from_bits(v.parse().unwrap()),
                "tiled" => {
                    shape.wrap_x = matches!(v, "x" | "both");
                    shape.wrap_y = matches!(v, "y" | "both");
                }
                "bg" => bg = v == "b",
                _ => panic!("unknown field {k}"),
            }
        }
        let mut input = Vec::new();
        for line in lines.by_ref() {
            if line == "--" {
                break;
            }
            input.push(line.as_bytes().to_vec());
        }
        let expected: Vec<Vec<u8>> = (0..input.len())
            .map(|_| lines.next().expect("output row").as_bytes().to_vec())
            .collect();
        cases.push(Case {
            name,
            shape,
            bg,
            input,
            expected,
        });
    }
    cases
}

#[test]
fn outline_matches_aseprite_pixel_by_pixel() {
    let cases = cases();
    assert!(cases.len() >= 70, "the fixture has all its cases");
    for case in cases {
        let (w, h) = (case.input[0].len(), case.input.len());
        let empty = |x: i32, y: i32| {
            let c = case.input[y as usize][x as usize];
            c == b'.' || (case.bg && c == b'b')
        };
        let mask = outline_mask(w as u32, h as u32, empty, &case.shape);
        let got: Vec<String> = (0..h)
            .map(|y| {
                (0..w)
                    .map(|x| {
                        if mask[y * w + x] {
                            'k'
                        } else {
                            case.input[y][x] as char
                        }
                    })
                    .collect()
            })
            .collect();
        let expected: Vec<String> = case
            .expected
            .iter()
            .map(|r| String::from_utf8(r.clone()).unwrap())
            .collect();
        assert_eq!(got, expected, "case {}", case.name);
    }
}
