//! La rampa de la luz: oscurecer un color con los de la paleta del documento.

use cs_core::Rgba;

/// Un paso más oscuro de `c`: el color de la paleta con luminancia menor que la de `c` más
/// cercano a `c` en distancia RGB (sin contar los transparentes). Si no hay ninguno, `c`.
pub fn darker(palette: &[Rgba], c: Rgba) -> Rgba {
    let lc = luminance(c);
    palette
        .iter()
        .filter(|p| p.a > 0 && luminance(**p) < lc)
        .min_by_key(|p| {
            let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
            d(p.r, c.r) + d(p.g, c.g) + d(p.b, c.b)
        })
        .copied()
        .unwrap_or(c)
}

/// La luminancia entera `299·r + 587·g + 114·b`.
fn luminance(c: Rgba) -> u32 {
    299 * c.r as u32 + 587 * c.g as u32 + 114 * c.b as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const NEGRO: Rgba = Rgba::new(0, 0, 0, 255);
    const GRIS: Rgba = Rgba::new(128, 128, 128, 255);
    const BLANCO: Rgba = Rgba::new(255, 255, 255, 255);

    #[test]
    fn un_paso_mas_oscuro() {
        let pal = [NEGRO, GRIS, BLANCO];
        assert_eq!(darker(&pal, BLANCO), GRIS);
        assert_eq!(darker(&pal, GRIS), NEGRO);
        assert_eq!(darker(&pal, NEGRO), NEGRO);
    }

    #[test]
    fn ignora_transparentes_y_paleta_vacia() {
        let pal = [Rgba::new(10, 10, 10, 0), BLANCO];
        assert_eq!(darker(&pal, GRIS), GRIS);
        assert_eq!(darker(&[], BLANCO), BLANCO);
    }

    #[test]
    fn elige_el_mas_cercano_en_rgb() {
        // Ambos son más oscuros que el rojo; el rojo oscuro está más cerca que el gris.
        let rojo = Rgba::new(255, 0, 0, 255);
        let rojo_osc = Rgba::new(150, 0, 0, 255);
        let gris_osc = Rgba::new(60, 60, 60, 255);
        assert_eq!(darker(&[gris_osc, rojo_osc], rojo), rojo_osc);
    }
}
