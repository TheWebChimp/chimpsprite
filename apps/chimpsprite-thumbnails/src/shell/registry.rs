//! El registro por usuario (HKCU, sin admin) que hace `regsvr32` con `DllRegisterServer` y
//! deshace `DllUnregisterServer`. Los tests solo prueban `registry_entries` (no tocan el registro).

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HMODULE, WIN32_ERROR};
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
    GetModuleFileNameW, GetModuleHandleExW,
};
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, REG_SZ, RegDeleteTreeW, RegSetKeyValueW,
};
use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
use windows_core::{HSTRING, PCWSTR};

use super::CLSID;

/// El "handler" de miniaturas del shell: `IThumbnailProvider`.
const THUMBNAIL_HANDLER: &str = "{E357FCCD-A995-4576-B01F-234630154E96}";
const NAME: &str = "ChimpSprite Thumbnail Provider";

/// Un valor de texto del registro, bajo `HKEY_CURRENT_USER`. `name: None` es el valor por omisión.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegValue {
    pub key: String,
    pub name: Option<&'static str>,
    pub value: String,
}

fn clsid_text() -> String {
    format!("{{{:?}}}", CLSID)
}

fn clsid_key() -> String {
    format!(r"Software\Classes\CLSID\{}", clsid_text())
}

fn handler_key() -> String {
    format!(r"Software\Classes\.chimp\ShellEx\{THUMBNAIL_HANDLER}")
}

/// Lo que escribe `DllRegisterServer` para la DLL en `dll_path`.
pub fn registry_entries(dll_path: &str) -> Vec<RegValue> {
    let value = |key: String, name, value: &str| RegValue {
        key,
        name,
        value: value.to_string(),
    };
    let server = format!(r"{}\InprocServer32", clsid_key());
    vec![
        value(clsid_key(), None, NAME),
        value(server.clone(), None, dll_path),
        value(server, Some("ThreadingModel"), "Apartment"),
        value(handler_key(), None, &clsid_text()),
    ]
}

/// La ruta de esta DLL (la del ejecutable de tests, en los tests).
pub fn dll_path() -> windows_core::Result<String> {
    let mut module = HMODULE::default();
    let address = PCWSTR(dll_path as *const () as *const u16);
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            address,
            &mut module,
        )?;
    }
    let mut buf = vec![0u16; 32 * 1024];
    let len = unsafe { GetModuleFileNameW(Some(module), &mut buf) } as usize;
    if len == 0 || len >= buf.len() {
        return Err(windows_core::Error::from_thread());
    }
    Ok(String::from_utf16_lossy(&buf[..len]))
}

fn check(status: WIN32_ERROR) -> windows_core::Result<()> {
    match status {
        ERROR_SUCCESS => Ok(()),
        e => Err(e.into()),
    }
}

/// Avisa al Explorador que cambiaron los handlers de un tipo de archivo.
fn notify_shell() {
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

pub fn register() -> windows_core::Result<()> {
    for entry in registry_entries(&dll_path()?) {
        let key = HSTRING::from(entry.key.as_str());
        let name = entry.name.map(HSTRING::from);
        let data: Vec<u16> = entry.value.encode_utf16().chain([0]).collect();
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                &key,
                name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())),
                REG_SZ.0,
                Some(data.as_ptr().cast()),
                (data.len() * 2) as u32,
            )
        };
        check(status)?;
    }
    notify_shell();
    Ok(())
}

pub fn unregister() -> windows_core::Result<()> {
    for key in [clsid_key(), handler_key()] {
        match unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, &HSTRING::from(key.as_str())) } {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => {}
            e => return Err(e.into()),
        }
    }
    notify_shell();
    Ok(())
}
