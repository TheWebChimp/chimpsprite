//! Windows: los íconos de la ventana salen del `.ico` que `build.rs` mete en el `.exe`.
//!
//! eframe pone en la ventana el ícono grande (barra de tareas, Alt+Tab) y el chico (barra de
//! título) escalando la imagen que se le pasa, y el pixel art reescalado se ve borroso. Acá se
//! cargan los dos del recurso, cada uno en el tamaño en que se dibuja: Windows elige del `.ico`
//! el dibujado a mano que toca (16, 24, 32 o 48). Como eframe los pone en los primeros frames (cuando
//! la ventana ya está activa), [`WindowIcons::keep`] revisa en cada frame que sigan siendo estos.

use eframe::CreationContext;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::Foundation::{HWND, LPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, HICON, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW,
    SM_CXSMICON, SendMessageW, WM_GETICON, WM_SETICON,
};

/// El id del ícono en los recursos del `.exe` (el que usa `winresource::set_icon`).
const ICON_ID: u16 = 1;

/// Los íconos del `.exe` para la ventana de la app.
pub struct WindowIcons {
    hwnd: HWND,
    /// Chico y grande, con el tipo de `WM_SETICON` de cada uno.
    icons: Vec<(u32, HICON)>,
}

impl WindowIcons {
    /// Carga los íconos del `.exe` para la ventana de `cc`. `None` sin la ventana o sin el
    /// recurso (p. ej. compilado sin el SDK de Windows): queda el ícono de eframe.
    pub fn load(cc: &CreationContext<'_>) -> Option<Self> {
        let handle = cc.window_handle().ok()?;
        let RawWindowHandle::Win32(win32) = handle.as_raw() else {
            return None;
        };
        let hwnd = win32.hwnd.get() as HWND;
        // El chico, el de la barra de título (16 al 100 %). El grande es el de la barra de tareas
        // de Windows 11, que dibuja los íconos a 1.5× el chico (24 al 100 %): con el tamaño de
        // sistema (32) lo reduciría y se vería borroso.
        // SAFETY: lee una métrica del sistema.
        let small = unsafe { GetSystemMetrics(SM_CXSMICON) };
        let mut icons = Vec::new();
        for (kind, side) in [(ICON_SMALL, small), (ICON_BIG, small * 3 / 2)] {
            // SAFETY: el módulo del proceso y un id de recurso; LoadImageW devuelve nulo si no
            // está. Los íconos viven lo que la app (no se liberan).
            let icon = unsafe {
                LoadImageW(
                    GetModuleHandleW(std::ptr::null()),
                    ICON_ID as usize as *const u16,
                    IMAGE_ICON,
                    side,
                    side,
                    LR_DEFAULTCOLOR,
                )
            };
            if !icon.is_null() {
                icons.push((kind, icon as HICON));
            }
        }
        (!icons.is_empty()).then_some(Self { hwnd, icons })
    }

    /// Pone los íconos si la ventana tiene otros (eframe los cambia en los primeros frames).
    pub fn keep(&self) {
        for &(kind, icon) in &self.icons {
            // SAFETY: la ventana de eframe sigue viva mientras corre la app.
            unsafe {
                let current = SendMessageW(self.hwnd, WM_GETICON, kind as usize, 0) as HICON;
                if current != icon {
                    SendMessageW(self.hwnd, WM_SETICON, kind as usize, icon as LPARAM);
                }
            }
        }
    }
}

/// La app con los íconos del `.exe` en su ventana.
pub struct WithIcons<A> {
    pub app: A,
    pub icons: Option<WindowIcons>,
}

impl<A: eframe::App> eframe::App for WithIcons<A> {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
        if let Some(icons) = &self.icons {
            icons.keep();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.app.save(storage);
    }
}
