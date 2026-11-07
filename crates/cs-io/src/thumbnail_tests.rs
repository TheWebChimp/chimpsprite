use std::io::{Cursor, Write};

use cs_core::{
    Artboard, ArtboardId, ColorMode, Document, FontInfo, FontMode, LayerId, PixelValue, Rgba,
    Sprite,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::*;

const RED: Rgba = Rgba::rgb(255, 0, 0);
const BLUE: Rgba = Rgba::rgb(0, 0, 255);
const T: Rgba = Rgba::TRANSPARENT;

fn bitmap(width: u32, height: u32, pixels: &[Rgba]) -> Bitmap {
    assert_eq!(pixels.len(), (width * height) as usize);
    Bitmap {
        width,
        height,
        pixels: pixels.to_vec(),
    }
}

fn draw(sprite: &mut Sprite, layer: LayerId, x: i32, y: i32, color: Rgba) {
    cs_commands::Command::DrawPixels {
        layer,
        frame: 0,
        pixels: vec![(x, y, PixelValue::Rgba(color))],
    }
    .apply(sprite)
    .unwrap();
}

/// Un sprite RGBA de `w × h` con un píxel rojo en (0, 0).
fn red_dot(w: u32, h: u32) -> Sprite {
    let mut s = Sprite::new(w, h, ColorMode::Rgba);
    draw(&mut s, LayerId(1), 0, 0, RED);
    s
}

fn artboard(id: u32, sprite: Sprite) -> Artboard {
    Artboard {
        model: None,
        in_library: false,
        font: None,
        guides: Vec::new(),
        symmetry: Default::default(),
        id: ArtboardId(id),
        name: format!("a{id}"),
        position: (0, 0),
        sprite,
    }
}

fn zip_with(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn fit_enlarges_by_the_largest_whole_factor() {
    let small = bitmap(2, 1, &[RED, BLUE]);
    let big = fit(&small, 256);
    assert_eq!((big.width, big.height), (256, 128));
    assert_eq!(big.pixels[0], RED);
    assert_eq!(big.pixels[127], RED);
    assert_eq!(big.pixels[128], BLUE, "128 px per source pixel");
    let odd = fit(&Bitmap::new(24, 10), 256);
    assert_eq!((odd.width, odd.height), (240, 100), "10x, not 10.67x");
    let same = fit(&small, 2);
    assert_eq!(same, small);
}

#[test]
fn fit_shrinks_with_nearest_neighbour() {
    // 4×2: columnas rojo, azul, rojo, azul → a 2×1 quedan las columnas 0 y 2.
    let wide = bitmap(4, 2, &[RED, BLUE, RED, BLUE, BLUE, BLUE, BLUE, BLUE]);
    let half = fit(&wide, 2);
    assert_eq!(half, bitmap(2, 1, &[RED, RED]));
    let line = fit(&Bitmap::new(600, 1), 256);
    assert_eq!((line.width, line.height), (256, 1), "never 0 px tall");
    let tall = fit(&Bitmap::new(300, 900), 256);
    assert_eq!((tall.width, tall.height), (85, 256));
    assert_eq!(fit(&wide, 0).width, 1, "size 0 counts as 1");
}

#[test]
fn pick_prefers_the_active_artboard_but_not_a_font() {
    let mut doc = Document::with_sprite("one", red_dot(4, 4));
    doc.artboards.push(artboard(2, red_dot(4, 4)));
    let mut font = artboard(3, red_dot(4, 4));
    font.font = Some(FontInfo::new("Mine", 8, FontMode::OneBit));
    doc.artboards.push(font);
    assert_eq!(pick(&doc, Some(ArtboardId(2))), Some(ArtboardId(2)));
    assert_eq!(pick(&doc, Some(ArtboardId(3))), Some(ArtboardId(1)));
    assert_eq!(pick(&doc, Some(ArtboardId(99))), Some(ArtboardId(1)));
    assert_eq!(pick(&doc, None), Some(ArtboardId(1)));
    // Sin artboards en el lienzo: el primero del documento.
    doc.artboards.retain(|a| a.id == ArtboardId(3));
    assert_eq!(pick(&doc, None), Some(ArtboardId(3)));
    doc.artboards.clear();
    assert_eq!(pick(&doc, None), None);
}

#[test]
fn render_draws_frame_one_enlarged() {
    let mut sprite = red_dot(16, 8);
    draw(&mut sprite, LayerId(1), 15, 7, BLUE);
    let doc = Document::with_sprite("one", sprite);
    let thumb = render(&doc, ArtboardId(1)).unwrap();
    assert_eq!((thumb.width, thumb.height), (256, 128));
    assert_eq!(thumb.pixels[0], RED);
    assert_eq!(thumb.pixels[15], RED);
    assert_eq!(thumb.pixels[16], T);
    assert_eq!(*thumb.pixels.last().unwrap(), BLUE);
    assert!(render(&doc, ArtboardId(9)).is_none());
}

#[test]
fn an_empty_frame_has_no_thumbnail() {
    let doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
    assert!(render(&doc, ArtboardId(1)).is_none());
    // Lo que solo está en una capa de referencia no cuenta.
    let mut sprite = Sprite::new(4, 4, ColorMode::Rgba);
    sprite.layers[0].reference = true;
    draw(&mut sprite, LayerId(1), 1, 1, RED);
    let doc = Document::with_sprite("one", sprite);
    assert!(render(&doc, ArtboardId(1)).is_none());
}

#[test]
fn render_shows_instances() {
    let mut doc = Document::with_sprite("host", Sprite::new(4, 4, ColorMode::Rgba));
    let mut symbol = artboard(2, red_dot(2, 2));
    symbol.in_library = true;
    doc.artboards.push(symbol);
    let host = &mut doc.artboards[0].sprite;
    cs_commands::ops::new_object_layer(host, None, 0, ArtboardId(2), "inst", (3, 3))
        .apply(host)
        .unwrap();
    let thumb = render(&doc, ArtboardId(1)).unwrap();
    assert_eq!(thumb.width, 256);
    // El punto rojo del símbolo en (3, 3) del host, 64 px por píxel.
    assert_eq!(thumb.pixels[(3 * 64 * 256 + 3 * 64) as usize], RED);
    assert_eq!(thumb.pixels[0], T);
}

#[test]
fn read_finds_the_embedded_png() {
    let png = bitmap(2, 1, &[RED, BLUE]).to_png().unwrap();
    let file = zip_with(&[("manifest.json", b"{}"), (ENTRY, &png)]);
    assert_eq!(read_png(Cursor::new(&file)).unwrap(), Some(png));
    let image = read(Cursor::new(&file)).unwrap().unwrap();
    assert_eq!(image, bitmap(2, 1, &[RED, BLUE]));
}

#[test]
fn a_file_without_thumbnail_reads_none_and_garbage_is_an_error() {
    let old = zip_with(&[("manifest.json", b"{}")]);
    assert_eq!(read_png(Cursor::new(&old)).unwrap(), None);
    assert!(read(Cursor::new(&old)).unwrap().is_none());
    assert!(read_png(Cursor::new(b"not a zip")).is_err());
    let bad = zip_with(&[(ENTRY, b"not a png")]);
    assert!(read(Cursor::new(&bad)).is_err());
}

#[test]
fn a_huge_embedded_png_is_refused() {
    let png = Bitmap::new(MAX_SIDE + 1, 1).to_png().unwrap();
    let file = zip_with(&[(ENTRY, &png)]);
    assert!(read(Cursor::new(&file)).is_err());
    let png = Bitmap::new(MAX_SIDE, 1).to_png().unwrap();
    let file = zip_with(&[(ENTRY, &png)]);
    assert!(read(Cursor::new(&file)).unwrap().is_some());
}

#[test]
fn for_file_uses_the_embedded_one_and_fits_it() {
    let png = bitmap(2, 1, &[RED, BLUE]).to_png().unwrap();
    let file = zip_with(&[("manifest.json", b"{}"), (ENTRY, &png)]);
    let image = for_file(&file, 8).unwrap().unwrap();
    assert_eq!((image.width, image.height), (8, 4));
    assert_eq!(image.pixels[4], BLUE);
}

#[test]
fn for_file_renders_an_old_file() {
    let doc = Document::with_sprite("one", red_dot(2, 2));
    let file = chimp::write_with(&doc, chimp::ThumbnailFrom::None).unwrap();
    assert!(read(Cursor::new(&file)).unwrap().is_none(), "no thumbnail");
    let image = for_file(&file, 128).unwrap().unwrap();
    assert_eq!((image.width, image.height), (128, 128));
    assert_eq!(image.pixels[0], RED);
    assert_eq!(image.pixels[64], T);
    let empty = Document::with_sprite("one", Sprite::new(2, 2, ColorMode::Rgba));
    let file = chimp::write_with(&empty, chimp::ThumbnailFrom::None).unwrap();
    assert!(for_file(&file, 128).unwrap().is_none());
}
