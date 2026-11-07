//! La extensión del shell de Windows: `IInitializeWithStream` + `IThumbnailProvider`, su fábrica
//! y las funciones que exporta la DLL (`DllGetClassObject`, `DllCanUnloadNow`,
//! `DllRegisterServer`, `DllUnregisterServer`).

mod registry;
mod stream;

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicUsize, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_POINTER, E_UNEXPECTED,
    ERROR_ALREADY_INITIALIZED, S_FALSE, S_OK,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl, IStream};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTS_ALPHATYPE, WTSAT_ARGB,
};
use windows_core::{BOOL, GUID, HRESULT, IUnknown, Interface, Ref, implement};

pub use registry::{RegValue, registry_entries};
use stream::StreamReader;

/// El CLSID del proveedor de miniaturas de ChimpSprite.
pub const CLSID: GUID = GUID::from_u128(0x4e744b03_3050_45ed_9272_5b94f8a131b4);

/// Objetos COM vivos más los `LockServer(TRUE)`: con 0, la DLL se puede descargar.
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// Cuenta un objeto vivo mientras exista.
struct Live;

impl Live {
    fn new() -> Self {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Live
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// El proveedor: el shell lo crea, le pasa el archivo como stream y le pide la miniatura.
#[implement(IInitializeWithStream, IThumbnailProvider)]
pub struct ThumbnailProvider {
    stream: RefCell<Option<IStream>>,
    _live: Live,
}

impl ThumbnailProvider {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            stream: RefCell::new(None),
            _live: Live::new(),
        }
    }
}

impl IInitializeWithStream_Impl for ThumbnailProvider_Impl {
    fn Initialize(&self, stream: Ref<IStream>, _mode: u32) -> windows_core::Result<()> {
        let stream = stream.ok()?;
        let mut slot = self.stream.borrow_mut();
        if slot.is_some() {
            return Err(ERROR_ALREADY_INITIALIZED.into());
        }
        *slot = Some(stream.clone());
        Ok(())
    }
}

impl IThumbnailProvider_Impl for ThumbnailProvider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> windows_core::Result<()> {
        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(E_POINTER.into());
        }
        let stream = self.stream.borrow().clone().ok_or(E_UNEXPECTED)?;
        // Sin miniatura incrustada (un archivo viejo) el Explorador pone el ícono normal. Un
        // pánico (un archivo roto que no se previó) tampoco cruza a COM.
        let image = std::panic::catch_unwind(AssertUnwindSafe(|| {
            cs_io::thumbnail::read(StreamReader(stream))
                .ok()
                .flatten()
                .map(|image| cs_io::thumbnail::fit(&image, cx))
        }))
        .ok()
        .flatten()
        .ok_or(E_FAIL)?;
        let bitmap = dib(&image)?;
        unsafe {
            *phbmp = bitmap;
            *pdwalpha = WTSAT_ARGB;
        }
        Ok(())
    }
}

/// Sección DIB de 32 bits de arriba abajo, BGRA sin premultiplicar (lo que espera `WTSAT_ARGB`).
fn dib(image: &cs_io::export::Bitmap) -> windows_core::Result<HBITMAP> {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: image.width as i32,
            biHeight: -(image.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
    if bits.is_null() {
        return Err(E_FAIL.into());
    }
    let len = image.pixels.len() * 4;
    let dst = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), len) };
    for (d, c) in dst.as_chunks_mut::<4>().0.iter_mut().zip(&image.pixels) {
        d.copy_from_slice(&[c.b, c.g, c.r, c.a]);
    }
    Ok(bitmap)
}

#[implement(IClassFactory)]
struct Factory {
    _live: Live,
}

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        riid: *const GUID,
        ppv: *mut *mut c_void,
    ) -> windows_core::Result<()> {
        if ppv.is_null() || riid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *ppv = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let provider: IUnknown = ThumbnailProvider::new().into();
        unsafe { provider.query(riid, ppv) }.ok()
    }

    fn LockServer(&self, lock: BOOL) -> windows_core::Result<()> {
        if lock.as_bool() {
            LIVE.fetch_add(1, Ordering::SeqCst);
        } else {
            // Un `LockServer(FALSE)` de más no da la vuelta al contador.
            let mut n = LIVE.load(Ordering::SeqCst);
            while n > 0 {
                match LIVE.compare_exchange(n, n - 1, Ordering::SeqCst, Ordering::SeqCst) {
                    Ok(_) => break,
                    Err(now) => n = now,
                }
            }
        }
        Ok(())
    }
}

/// La fábrica del proveedor (solo para `CLSID`).
///
/// # Safety
/// Punteros válidos, como los pasa COM.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || rclsid.is_null() || riid.is_null() {
        return E_POINTER;
    }
    unsafe { *ppv = std::ptr::null_mut() };
    if unsafe { *rclsid } != CLSID {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = Factory { _live: Live::new() }.into();
    unsafe { factory.query(riid, ppv) }
}

/// `S_OK` si no queda ningún objeto vivo ni bloqueo.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LIVE.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

/// `regsvr32 chimpsprite_thumbnails.dll`: registra el proveedor para `.chimp` (usuario actual).
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match registry::register() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

/// `regsvr32 /u chimpsprite_thumbnails.dll`: lo quita.
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match registry::unregister() {
        Ok(()) => S_OK,
        Err(e) => e.code(),
    }
}

#[cfg(test)]
mod tests;
