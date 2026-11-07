//! El proveedor llamado directo (sin registrarlo): un `.chimp` en un stream de memoria, y los
//! píxeles del HBITMAP que devuelve.

use windows::Win32::Foundation::{ERROR_ALREADY_INITIALIZED, HANDLE};
use windows::Win32::Graphics::Gdi::{DIBSECTION, DeleteObject, GetObjectW, HGDIOBJ};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows_core::Interface;

use super::*;
use crate::ffi::tests::red_dot_chimp;
use cs_io::chimp::ThumbnailFrom;

fn provider() -> IInitializeWithStream {
    ThumbnailProvider::new().into()
}

fn stream(bytes: &[u8]) -> IStream {
    unsafe { SHCreateMemStream(Some(bytes)) }.expect("memory stream")
}

/// Ancho, alto y píxeles BGRA (en el orden de la memoria) de la sección DIB, que se borra.
fn read_dib(bitmap: HBITMAP) -> (u32, u32, Vec<[u8; 4]>) {
    let mut section = DIBSECTION::default();
    let size = size_of::<DIBSECTION>() as i32;
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(bitmap.0),
            size,
            Some((&mut section as *mut DIBSECTION).cast()),
        )
    };
    assert_eq!(got, size, "a DIB section");
    let header = section.dsBmih;
    assert_eq!(header.biBitCount, 32);
    // `GetObjectW` da el alto siempre positivo: que sea de arriba abajo lo prueban los píxeles
    // (el punto rojo de arriba a la izquierda está al principio de la memoria).
    let (w, h) = (header.biWidth as u32, header.biHeight.unsigned_abs());
    assert_eq!(section.dshSection, HANDLE::default());
    let bytes = unsafe {
        std::slice::from_raw_parts(section.dsBm.bmBits.cast::<u8>(), (w * h * 4) as usize)
    };
    let pixels = bytes.as_chunks::<4>().0.to_vec();
    assert!(unsafe { DeleteObject(HGDIOBJ(bitmap.0)) }.as_bool());
    (w, h, pixels)
}

fn thumbnail(
    init: &IInitializeWithStream,
    cx: u32,
) -> windows_core::Result<(HBITMAP, WTS_ALPHATYPE)> {
    let provider: IThumbnailProvider = init.cast()?;
    let mut bitmap = HBITMAP::default();
    let mut alpha = WTS_ALPHATYPE::default();
    unsafe { provider.GetThumbnail(cx, &mut bitmap, &mut alpha) }?;
    Ok((bitmap, alpha))
}

const RED: [u8; 4] = [0, 0, 255, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

#[test]
fn gives_the_embedded_thumbnail_as_an_argb_bitmap() {
    let init = provider();
    let bytes = red_dot_chimp(ThumbnailFrom::Auto);
    unsafe { init.Initialize(&stream(&bytes), 0) }.unwrap();
    let (bitmap, alpha) = thumbnail(&init, 64).unwrap();
    assert_eq!(alpha, WTSAT_ARGB);
    let (w, h, pixels) = read_dib(bitmap);
    assert_eq!((w, h), (64, 64));
    // El punto rojo es el cuadro de arriba a la izquierda (32×32); el resto, transparente.
    assert_eq!(pixels[0], RED);
    assert_eq!(pixels[31], RED);
    assert_eq!(pixels[32], CLEAR);
    assert_eq!(pixels[31 * 64], RED);
    assert_eq!(pixels[32 * 64], CLEAR);
    // Pedirla otra vez (otro tamaño) vuelve a leer el stream desde donde haga falta.
    let (bitmap, _) = thumbnail(&init, 300).unwrap();
    let (w, h, pixels) = read_dib(bitmap);
    assert_eq!((w, h), (256, 256), "no bigger than the embedded one");
    assert_eq!(pixels[127], RED);
    assert_eq!(pixels[128], CLEAR);
}

#[test]
fn the_class_factory_creates_providers() {
    let mut raw = std::ptr::null_mut();
    let hr = unsafe { DllGetClassObject(&CLSID, &IClassFactory::IID, &mut raw) };
    assert_eq!(hr, S_OK);
    let factory = unsafe { IClassFactory::from_raw(raw) };
    assert_eq!(DllCanUnloadNow(), S_FALSE, "the factory is alive");
    let init: IInitializeWithStream = unsafe { factory.CreateInstance(None) }.unwrap();
    let bytes = red_dot_chimp(ThumbnailFrom::Auto);
    unsafe { init.Initialize(&stream(&bytes), 0) }.unwrap();
    let (bitmap, _) = thumbnail(&init, 16).unwrap();
    let (w, h, pixels) = read_dib(bitmap);
    assert_eq!((w, h, pixels[0], pixels[8]), (16, 16, RED, CLEAR));
    // Otro CLSID no.
    let other = GUID::from_u128(1);
    let hr = unsafe { DllGetClassObject(&other, &IClassFactory::IID, &mut raw) };
    assert_eq!((hr, raw.is_null()), (CLASS_E_CLASSNOTAVAILABLE, true));
}

#[test]
fn a_file_without_thumbnail_fails_so_explorer_shows_the_icon() {
    for bytes in [red_dot_chimp(ThumbnailFrom::None), b"not a zip".to_vec()] {
        let init = provider();
        unsafe { init.Initialize(&stream(&bytes), 0) }.unwrap();
        assert_eq!(thumbnail(&init, 64).unwrap_err().code(), E_FAIL);
    }
}

#[test]
fn initialize_once_and_before_get_thumbnail() {
    let init = provider();
    assert_eq!(thumbnail(&init, 64).unwrap_err().code(), E_UNEXPECTED);
    let bytes = red_dot_chimp(ThumbnailFrom::Auto);
    unsafe { init.Initialize(&stream(&bytes), 0) }.unwrap();
    let again = unsafe { init.Initialize(&stream(&bytes), 0) }.unwrap_err();
    assert_eq!(
        again.code(),
        HRESULT::from_win32(ERROR_ALREADY_INITIALIZED.0)
    );
    let provider: IThumbnailProvider = init.cast().unwrap();
    let hr = unsafe { provider.GetThumbnail(64, std::ptr::null_mut(), std::ptr::null_mut()) };
    assert_eq!(hr.unwrap_err().code(), E_POINTER);
}

#[test]
fn registry_entries_are_per_user_for_dot_chimp() {
    let entries = registry_entries(r"C:\Apps\chimpsprite_thumbnails.dll");
    let clsid = r"Software\Classes\CLSID\{4E744B03-3050-45ED-9272-5B94F8A131B4}";
    let get = |key: &str, name: Option<&str>| {
        entries
            .iter()
            .find(|e| e.key == key && e.name == name)
            .map(|e| e.value.as_str())
    };
    assert_eq!(get(clsid, None), Some("ChimpSprite Thumbnail Provider"));
    let server = format!(r"{clsid}\InprocServer32");
    assert_eq!(
        get(&server, None),
        Some(r"C:\Apps\chimpsprite_thumbnails.dll")
    );
    assert_eq!(get(&server, Some("ThreadingModel")), Some("Apartment"));
    assert_eq!(
        get(
            r"Software\Classes\.chimp\ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}",
            None
        ),
        Some("{4E744B03-3050-45ED-9272-5B94F8A131B4}")
    );
    assert_eq!(entries.len(), 4);
}

#[test]
fn the_module_path_is_found() {
    // En los tests es el ejecutable de tests; en el shell, la DLL.
    let path = registry::dll_path().unwrap();
    assert!(std::path::Path::new(&path).is_file(), "{path}");
}
