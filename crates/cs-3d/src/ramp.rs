//! La rampa de la luz: oscurecer un color con los de la paleta del documento.

use cs_core::{Rgba, ShadingTable};

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

/// `c` oscurecido `steps` pasos (0, 1 o 2): la tabla manda y, donde no fija nada, la regla
/// automática de `darker`.
pub fn shade(palette: &[Rgba], table: &ShadingTable, c: Rgba, steps: u32) -> Rgba {
    match steps {
        0 => c,
        1 => table.get(c, 1).unwrap_or_else(|| darker(palette, c)),
        _ => table
            .get(c, 2)
            .unwrap_or_else(|| darker(palette, darker(palette, c))),
    }
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
    fn shade_con_tabla_vacia_es_la_regla_automatica() {
        let pal = [NEGRO, GRIS, BLANCO];
        let t = ShadingTable::default();
        assert_eq!(shade(&pal, &t, BLANCO, 0), BLANCO);
        assert_eq!(shade(&pal, &t, BLANCO, 1), GRIS);
        assert_eq!(shade(&pal, &t, BLANCO, 2), NEGRO);
    }

    #[test]
    fn shade_usa_la_tabla_por_paso() {
        let pal = [NEGRO, GRIS, BLANCO];
        let rojo = Rgba::new(255, 0, 0, 255);
        let mut t = ShadingTable::default();
        t.set(BLANCO, 1, Some(rojo));
        assert_eq!(shade(&pal, &t, BLANCO, 1), rojo);
        assert_eq!(shade(&pal, &t, BLANCO, 2), NEGRO);
        t.set(BLANCO, 2, Some(rojo));
        assert_eq!(shade(&pal, &t, BLANCO, 2), rojo);
        assert_eq!(shade(&pal, &t, BLANCO, 0), BLANCO);
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
