//! Función C para los proveedores de miniaturas que no son Rust (la extensión de Quick Look de
//! macOS, ver `macos/chimp_thumbnail.h`).

use std::io::Cursor;

/// Lo encontró: `*out` y `*out_len` tienen el PNG.
pub const CHIMP_THUMBNAIL_OK: i32 = 0;
/// El archivo no trae miniatura (uno guardado antes de las miniaturas, o vacío).
pub const CHIMP_THUMBNAIL_NONE: i32 = 1;
/// Puntero nulo, o no es un `.chimp` / la miniatura está rota.
pub const CHIMP_THUMBNAIL_ERROR: i32 = -1;

/// La miniatura (PNG RGBA, lado mayor ≤ 256) de los bytes de un `.chimp`. Solo lee el
/// `thumbnail.png` del zip. Si devuelve `CHIMP_THUMBNAIL_OK`, el PNG se libera con
/// `chimp_thumbnail_free(*out, *out_len)`.
///
/// # Safety
/// `data` apunta a `len` bytes legibles (o `len` es 0); `out` y `out_len` son punteros válidos.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chimp_thumbnail_png(
    data: *const u8,
    len: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if out.is_null() || out_len.is_null() || (data.is_null() && len > 0) {
        return CHIMP_THUMBNAIL_ERROR;
    }
    unsafe {
        *out = std::ptr::null_mut();
        *out_len = 0;
    }
    let bytes: &[u8] = match len {
        0 => &[],
        _ => unsafe { std::slice::from_raw_parts(data, len) },
    };
    let png = match cs_io::thumbnail::read(Cursor::new(bytes)) {
        Ok(Some(image)) => match image.to_png() {
            Ok(png) => png,
            Err(_) => return CHIMP_THUMBNAIL_ERROR,
        },
        Ok(None) => return CHIMP_THUMBNAIL_NONE,
        Err(_) => return CHIMP_THUMBNAIL_ERROR,
    };
    let png = png.into_boxed_slice();
    unsafe {
        *out_len = png.len();
        *out = Box::into_raw(png).cast::<u8>();
    }
    CHIMP_THUMBNAIL_OK
}

/// Libera un PNG de `chimp_thumbnail_png`. Con un puntero nulo no hace nada.
///
/// # Safety
/// `ptr` y `len` son los que dio `chimp_thumbnail_png`, y se liberan una sola vez.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn chimp_thumbnail_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)) });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use cs_core::{ColorMode, Document, LayerId, PixelValue, Rgba, Sprite};
    use cs_io::chimp::ThumbnailFrom;

    use super::*;

    pub(crate) fn red_dot_chimp(thumbnail: ThumbnailFrom) -> Vec<u8> {
        let mut sprite = Sprite::new(2, 2, ColorMode::Rgba);
        cs_commands::Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        }
        .apply(&mut sprite)
        .unwrap();
        cs_io::chimp::write_with(&Document::with_sprite("dot", sprite), thumbnail).unwrap()
    }

    fn call(bytes: &[u8]) -> (i32, Option<Vec<u8>>) {
        let mut out = std::ptr::null_mut();
        let mut out_len = 0;
        let code =
            unsafe { chimp_thumbnail_png(bytes.as_ptr(), bytes.len(), &mut out, &mut out_len) };
        let png = (!out.is_null()).then(|| {
            let copy = unsafe { std::slice::from_raw_parts(out, out_len) }.to_vec();
            unsafe { chimp_thumbnail_free(out, out_len) };
            copy
        });
        (code, png)
    }

    #[test]
    fn returns_the_embedded_png() {
        let (code, png) = call(&red_dot_chimp(ThumbnailFrom::Auto));
        assert_eq!(code, CHIMP_THUMBNAIL_OK);
        let image = cs_io::import::decode_png(&png.unwrap()).unwrap();
        assert_eq!((image.width, image.height), (256, 256));
        assert_eq!(image.pixels[0], Rgba::rgb(255, 0, 0));
        assert_eq!(image.pixels[128], Rgba::TRANSPARENT);
    }

    #[test]
    fn old_files_garbage_and_null_pointers() {
        assert_eq!(
            call(&red_dot_chimp(ThumbnailFrom::None)),
            (CHIMP_THUMBNAIL_NONE, None)
        );
        assert_eq!(call(b"not a zip"), (CHIMP_THUMBNAIL_ERROR, None));
        assert_eq!(call(&[]), (CHIMP_THUMBNAIL_ERROR, None));
        let mut len = 0;
        let code = unsafe { chimp_thumbnail_png(b"x".as_ptr(), 1, std::ptr::null_mut(), &mut len) };
        assert_eq!(code, CHIMP_THUMBNAIL_ERROR);
        let mut out = std::ptr::null_mut();
        let code = unsafe { chimp_thumbnail_png(std::ptr::null(), 4, &mut out, &mut len) };
        assert_eq!(code, CHIMP_THUMBNAIL_ERROR);
        unsafe { chimp_thumbnail_free(std::ptr::null_mut(), 0) };
    }

    #[test]
    fn the_c_header_matches() {
        let header = include_str!("../macos/chimp_thumbnail.h");
        for (name, value) in [
            ("CHIMP_THUMBNAIL_OK", CHIMP_THUMBNAIL_OK),
            ("CHIMP_THUMBNAIL_NONE", CHIMP_THUMBNAIL_NONE),
            ("CHIMP_THUMBNAIL_ERROR", CHIMP_THUMBNAIL_ERROR),
        ] {
            assert!(
                header.contains(&format!("#define {name} {value} ")),
                "{name}"
            );
        }
        assert!(header.contains(
            "int32_t chimp_thumbnail_png(const uint8_t *data, size_t len, uint8_t **out, size_t *out_len);"
        ));
        assert!(header.contains("void chimp_thumbnail_free(uint8_t *ptr, size_t len);"));
    }
}
