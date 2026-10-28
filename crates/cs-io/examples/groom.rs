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
