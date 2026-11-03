//! Un lienzo RGBA chiquito para dibujar las vistas orto del modo Edit en pixel art: líneas de
//! Bresenham de 1 pixel sin suavizado (recortadas a la imagen), líneas punteadas, rectángulos y
//! pixeles sueltos. Cada pixel de la imagen es un punto de la UI.

/// Un color RGBA (sin premultiplicar; los de las vistas son opacos).
pub type Rgba = [u8; 4];

/// La imagen: `width × height` pixeles, fila por fila.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WireImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Rgba>,
}

impl WireImage {
    /// Una imagen de `width × height` toda de `fill`.
    pub fn new(width: usize, height: usize, fill: Rgba) -> Self {
        Self {
            width,
            height,
            pixels: vec![fill; width * height],
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }

    /// El pixel `(x, y)`, si cae en la imagen.
    pub fn get(&self, x: i32, y: i32) -> Option<Rgba> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Pinta el pixel `(x, y)`; fuera de la imagen no hace nada.
    pub fn put(&mut self, x: i32, y: i32, c: Rgba) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = c;
        }
    }

    /// Un rectángulo lleno de `w × h` con la esquina en `(x, y)` (recortado).
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba) {
        let x0 = x.max(0) as i64;
        let y0 = y.max(0) as i64;
        let x1 = (x as i64 + w as i64).min(self.width as i64);
        let y1 = (y as i64 + h as i64).min(self.height as i64);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.pixels[yy as usize * self.width + xx as usize] = c;
            }
        }
    }

    /// El borde de 1 pixel del rectángulo con esquinas `(x0, y0)` y `(x1, y1)` (incluidas, en
    /// cualquier orden).
    pub fn rect_outline(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        self.line(x0, y0, x1, y0, c);
        self.line(x0, y1, x1, y1, c);
        self.line(x0, y0, x0, y1, c);
        self.line(x1, y0, x1, y1, c);
    }

    /// Una línea de Bresenham de `(x0, y0)` a `(x1, y1)`, los dos extremos incluidos.
    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        self.raster(x0, y0, x1, y1, c, false);
    }

    /// [`WireImage::line`] punteada: 2 pixeles sí y 2 no, contando desde `(x0, y0)`.
    pub fn dashed_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba) {
        self.raster(x0, y0, x1, y1, c, true);
    }

    /// La línea, paso a paso por el eje largo: en el paso `i` el eje corto avanza
    /// `round(i · corto / largo)` (lo mismo que Bresenham). Solo recorre los pasos que caen en la
    /// imagen (el eje corto es monótono, así que se buscan por bisección): una línea enorme por
    /// el zoom no cuesta más que una que cruza la imagen.
    fn raster(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgba, dashed: bool) {
        let (dx, dy) = (x1 as i128 - x0 as i128, y1 as i128 - y0 as i128);
        let (w, h) = (self.width as i128, self.height as i128);
        if w == 0 || h == 0 {
            return;
        }
        let x_major = dx.abs() >= dy.abs();
        let (a0, b0, da, db, size_a, size_b) = if x_major {
            (x0 as i128, y0 as i128, dx, dy, w, h)
        } else {
            (y0 as i128, x0 as i128, dy, dx, h, w)
        };
        let n = da.abs();
        if n == 0 {
            self.put(x0, y0, c);
            return;
        }
        let (sa, sb, adb) = (da.signum(), db.signum(), db.abs());
        let minor = |i: i128| b0 + sb * ((2 * i * adb + n) / (2 * n));
        // Los pasos con el eje largo dentro de la imagen.
        let (lo, hi) = if sa > 0 {
            (-a0, size_a - 1 - a0)
        } else {
            (a0 - (size_a - 1), a0)
        };
        let (lo, hi) = (lo.max(0), hi.min(n));
        if lo > hi {
            return;
        }
        // El primer paso de `lo..=hi` en el que `pred` se cumple (`hi + 1` si ninguno).
        let first = |pred: &dyn Fn(i128) -> bool| {
            let (mut l, mut r) = (lo, hi + 1);
            while l < r {
                let m = l + (r - l) / 2;
                if pred(m) { r = m } else { l = m + 1 }
            }
            l
        };
        let (from, to) = if sb >= 0 {
            (
                first(&|i| minor(i) >= 0),
                first(&|i| minor(i) >= size_b) - 1,
            )
        } else {
            (first(&|i| minor(i) < size_b), first(&|i| minor(i) < 0) - 1)
        };
        for i in from..=to {
            if dashed && i % 4 >= 2 {
                continue;
            }
            let (a, b) = (a0 + sa * i, minor(i));
            let (x, y) = if x_major { (a, b) } else { (b, a) };
            self.put(x as i32, y as i32, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BG: Rgba = [0, 0, 0, 255];
    const C: Rgba = [255, 255, 255, 255];

    fn lit(img: &WireImage) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        for y in 0..img.height as i32 {
            for x in 0..img.width as i32 {
                if img.get(x, y) == Some(C) {
                    out.push((x, y));
                }
            }
        }
        out
    }

    #[test]
    fn a_horizontal_line_lights_every_pixel_between_its_ends() {
        let mut img = WireImage::new(10, 5, BG);
        img.line(2, 1, 6, 1, C);
        assert_eq!(lit(&img), (2..=6).map(|x| (x, 1)).collect::<Vec<_>>());
        // Al revés, lo mismo.
        let mut back = WireImage::new(10, 5, BG);
        back.line(6, 1, 2, 1, C);
        assert_eq!(back, img);
    }

    #[test]
    fn a_vertical_line_lights_every_pixel_between_its_ends() {
        let mut img = WireImage::new(5, 10, BG);
        img.line(3, 8, 3, 2, C);
        assert_eq!(lit(&img), (2..=8).map(|y| (3, y)).collect::<Vec<_>>());
    }

    #[test]
    fn a_diagonal_is_one_pixel_per_step() {
        let mut img = WireImage::new(8, 8, BG);
        img.line(1, 1, 5, 5, C);
        assert_eq!(lit(&img), (1..=5).map(|i| (i, i)).collect::<Vec<_>>());
        let mut anti = WireImage::new(8, 8, BG);
        anti.line(5, 1, 1, 5, C);
        assert_eq!(lit(&anti), (1..=5).map(|i| (6 - i, i)).collect::<Vec<_>>());
    }

    #[test]
    fn a_shallow_line_has_one_pixel_per_column_and_both_ends() {
        let mut img = WireImage::new(12, 6, BG);
        img.line(0, 0, 9, 3, C);
        let px = lit(&img);
        assert_eq!(px.len(), 10);
        assert!(px.contains(&(0, 0)) && px.contains(&(9, 3)));
        for x in 0..=9 {
            assert_eq!(px.iter().filter(|p| p.0 == x).count(), 1, "column {x}");
        }
        // Un pixel solo: los dos extremos iguales.
        let mut dot = WireImage::new(4, 4, BG);
        dot.line(2, 2, 2, 2, C);
        assert_eq!(lit(&dot), vec![(2, 2)]);
    }

    #[test]
    fn lines_are_clipped_to_the_image() {
        let mut img = WireImage::new(10, 5, BG);
        img.line(-100, 2, 100, 2, C);
        assert_eq!(lit(&img), (0..10).map(|x| (x, 2)).collect::<Vec<_>>());
        // Del todo afuera: nada.
        let mut out = WireImage::new(10, 5, BG);
        out.line(-20, -3, 40, -3, C);
        out.line(12, -50, 12, 50, C);
        assert!(lit(&out).is_empty());
        // Muy lejos (zoom enorme): termina enseguida y cruza la imagen en diagonal.
        let mut far = WireImage::new(10, 10, BG);
        far.line(
            -1_000_000_000,
            -1_000_000_000,
            1_000_000_000,
            1_000_000_000,
            C,
        );
        assert_eq!(lit(&far), (0..10).map(|i| (i, i)).collect::<Vec<_>>());
        // Una diagonal recortada sigue sobre la misma recta que sin recortar.
        let mut cut = WireImage::new(6, 6, BG);
        cut.line(-4, -2, 12, 6, C);
        let mut whole = WireImage::new(20, 10, BG);
        whole.line(0, 0, 16, 8, C);
        let shifted: Vec<(i32, i32)> = lit(&whole)
            .into_iter()
            .map(|(x, y)| (x - 4, y - 2))
            .filter(|&(x, y)| (0..6).contains(&x) && (0..6).contains(&y))
            .collect();
        assert_eq!(lit(&cut), shifted);
    }

    #[test]
    fn dashed_lines_are_two_on_two_off_from_the_start() {
        let mut img = WireImage::new(12, 3, BG);
        img.dashed_line(0, 1, 11, 1, C);
        let xs: Vec<i32> = lit(&img).into_iter().map(|p| p.0).collect();
        assert_eq!(xs, vec![0, 1, 4, 5, 8, 9]);
        // Recortada, el patrón sigue contando desde el extremo de verdad.
        let mut cut = WireImage::new(12, 3, BG);
        cut.dashed_line(-3, 1, 11, 1, C);
        let xs: Vec<i32> = lit(&cut).into_iter().map(|p| p.0).collect();
        assert_eq!(xs, vec![1, 2, 5, 6, 9, 10]);
        let mut v = WireImage::new(3, 8, BG);
        v.dashed_line(1, 7, 1, 0, C);
        let ys: Vec<i32> = lit(&v).into_iter().map(|p| p.1).collect();
        assert_eq!(ys, vec![2, 3, 6, 7]);
    }

    #[test]
    fn rects_fill_and_outline_inside_the_image() {
        let mut img = WireImage::new(6, 6, BG);
        img.fill_rect(-1, -1, 3, 3, C);
        assert_eq!(lit(&img), vec![(0, 0), (1, 0), (0, 1), (1, 1)]);
        let mut o = WireImage::new(6, 6, BG);
        o.rect_outline(4, 4, 1, 1, C);
        let px = lit(&o);
        assert_eq!(px.len(), 12);
        assert!(!px.contains(&(2, 2)) && px.contains(&(1, 1)) && px.contains(&(4, 4)));
        // Un rectángulo de 1 × 1 es un pixel.
        let mut one = WireImage::new(3, 3, BG);
        one.rect_outline(1, 1, 1, 1, C);
        assert_eq!(lit(&one), vec![(1, 1)]);
        let mut p = WireImage::new(3, 3, BG);
        p.put(-1, 0, C);
        p.put(3, 0, C);
        p.put(2, 2, C);
        assert_eq!(lit(&p), vec![(2, 2)]);
    }
}
