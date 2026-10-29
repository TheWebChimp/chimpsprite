//! Prueba Groom con una imagen: `cargo run -p cs-io --example groom -- in.png out.png [x y w h]`.
//! Detecta la cuadrícula (dentro del recorte, si se da), toma un color por celda y guarda el
//! resultado agrandado ×8 para verlo.

use cs_io::export::Bitmap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let bytes = std::fs::read(&args[1]).expect("read");
    let mut bitmap = cs_io::import::decode_png(&bytes).expect("png");
    if args.len() >= 7 {
        let n = |i: usize| args[i].parse::<u32>().expect("number");
        let (x0, y0, w, h) = (n(3), n(4), n(5), n(6));
        let pixels = (y0..y0 + h)
            .flat_map(|y| (x0..x0 + w).map(move |x| (x, y)))
            .map(|(x, y)| bitmap.pixels[(y * bitmap.width + x) as usize])
            .collect();
        bitmap = Bitmap {
            width: w,
            height: h,
            pixels,
        };
    }
    let grid = cs_raster::detect_grid(&bitmap.pixels, bitmap.width, bitmap.height);
    let (small, w, h) = cs_raster::sample_grid(&bitmap.pixels, bitmap.width, bitmap.height, grid);
    println!("grid {grid:?} → {w}×{h}");
    // El proceso completo, a tamaño real y recortado a lo pintado: `<salida>_1x.png`.
    let clean = cs_raster::remove_background(&small, w, h, 32);
    let clean = cs_raster::clean_outline(&clean, w, h, None);
    let painted = |i: usize| clean[i].a > 0;
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for i in (0..clean.len()).filter(|&i| painted(i)) {
        let (x, y) = (i as u32 % w, i as u32 / w);
        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
    }
    if x0 <= x1 {
        let (tw, th) = (x1 - x0 + 1, y1 - y0 + 1);
        let pixels = (y0..=y1)
            .flat_map(|y| (x0..=x1).map(move |x| (x, y)))
            .map(|(x, y)| clean[(y * w + x) as usize])
            .collect();
        let trimmed = Bitmap {
            width: tw,
            height: th,
            pixels,
        };
        let one = args[2].replace(".png", "_1x.png");
        std::fs::write(&one, trimmed.to_png().expect("png")).expect("write");
        println!("clean {tw}×{th} → {one}");
    }
    let scale = 8;
    let mut big = Bitmap::new(w * scale, h * scale);
    for y in 0..h * scale {
        for x in 0..w * scale {
            big.pixels[(y * w * scale + x) as usize] =
                small[((y / scale) * w + x / scale) as usize];
        }
    }
    std::fs::write(&args[2], big.to_png().expect("png")).expect("write");
}
