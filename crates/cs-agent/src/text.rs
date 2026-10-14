//! Píxeles como cuadrícula de texto: una leyenda de colores y una fila por línea.

use cs_core::Rgba;

const CODES: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// Una celda leída: color visible y, en sprites indexados, su índice de paleta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub color: Rgba,
    pub index: Option<u8>,
}

/// `rows` de igual ancho → leyenda + filas. `.` (o `..`) es transparente; con más de 62 colores los
/// códigos son de dos caracteres.
pub fn grid(rows: &[Vec<Cell>]) -> String {
    let mut legend: Vec<Cell> = Vec::new();
    for cell in rows.iter().flatten() {
        if cell.color.a > 0 && !legend.contains(cell) {
            legend.push(*cell);
        }
    }
    let wide = legend.len() > CODES.len();
    let code = |i: usize| -> String {
        if wide {
            let (a, b) = (i / CODES.len(), i % CODES.len());
            format!("{}{}", CODES[a] as char, CODES[b] as char)
        } else {
            (CODES[i] as char).to_string()
        }
    };
    let blank = if wide { ".." } else { "." };
    let mut out = String::from("legend:\n");
    out.push_str(&format!("  {blank} transparent\n"));
    for (i, cell) in legend.iter().enumerate() {
        out.push_str(&format!("  {} {}", code(i), cell.color.to_hex()));
        if let Some(index) = cell.index {
            out.push_str(&format!(" (index {index})"));
        }
        out.push('\n');
    }
    out.push_str("rows:\n");
    for row in rows {
        for cell in row {
            match legend.iter().position(|c| c == cell) {
                Some(i) if cell.color.a > 0 => out.push_str(&code(i)),
                _ => out.push_str(blank),
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(c: Rgba) -> Cell {
        Cell {
            color: c,
            index: None,
        }
    }

    #[test]
    fn small_grid_with_legend() {
        let t = cell(Rgba::TRANSPARENT);
        let r = cell(Rgba::rgb(255, 0, 0));
        let b = Cell {
            color: Rgba::rgb(0, 0, 255),
            index: Some(3),
        };
        let text = grid(&[vec![t, r], vec![b, r]]);
        assert_eq!(
            text,
            "legend:\n  . transparent\n  a #ff0000\n  b #0000ff (index 3)\nrows:\n.a\nba\n"
        );
    }

    #[test]
    fn many_colors_use_two_character_codes() {
        let row: Vec<Cell> = (0..70).map(|i| cell(Rgba::rgb(i, 0, 0))).collect();
        let text = grid(&[row]);
        let last = text.lines().last().unwrap();
        assert_eq!(last.len(), 140);
        assert!(last.starts_with("aaabac"));
    }
}
