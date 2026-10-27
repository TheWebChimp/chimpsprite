//! Lo que depende del sistema: diálogos de archivo, lectura, escritura y descargas.
//!
//! Abrir es asíncrono en web, así que el resultado siempre llega por una cola (`poll`) que la app
//! revisa cada frame; en escritorio la cola se llena en el momento.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use eframe::egui;

/// Dónde se guarda un archivo: carpeta (solo escritorio) y nombre con extensión.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveTarget {
    pub dir: Option<PathBuf>,
    pub file_name: String,
}

impl SaveTarget {
    /// Destino sin carpeta (en web, una descarga).
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn named(file_name: impl Into<String>) -> Self {
        Self {
            dir: None,
            file_name: file_name.into(),
        }
    }

    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub fn from_path(path: &Path) -> Option<Self> {
        Some(Self {
            dir: path.parent().map(Path::to_path_buf),
            file_name: path.file_name()?.to_str()?.to_string(),
        })
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(&self.file_name))
    }

    pub fn stem(&self) -> &str {
        crate::files::stem(&self.file_name)
    }

    /// Otro archivo en la misma carpeta.
    pub fn sibling(&self, file_name: impl Into<String>) -> Self {
        Self {
            dir: self.dir.clone(),
            file_name: file_name.into(),
        }
    }
}

/// Para qué se pidió abrir el archivo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPurpose {
    /// Reemplazar el documento (ya se confirmó si había cambios).
    Open,
    /// Soltado sobre la ventana: hay que confirmar si hay cambios.
    Dropped,
    /// Agregar como capa.
    ImportLayer,
    /// Agregar como capa de referencia (no sale al exportar).
    ImportReference,
    /// Partir en frames de un sprite nuevo (File → Import Sprite Sheet).
    ImportSheet,
    /// Correr un script (`.rhai` o `.lua`).
    RunScript,
    /// Cargar una paleta (`.gpl`, `.hex`, `.pal`, `.png`, `.aseprite`).
    LoadPalette,
}

pub enum PlatformEvent {
    Opened {
        name: String,
        /// Dónde volver a guardar (solo escritorio).
        target: Option<SaveTarget>,
        bytes: Vec<u8>,
        purpose: OpenPurpose,
    },
    Error(String),
}

#[derive(Clone, Copy)]
pub struct FileFilter {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
}

#[derive(Clone, Default)]
pub struct Platform {
    queue: Rc<RefCell<VecDeque<PlatformEvent>>>,
}

impl Platform {
    pub fn poll(&self) -> Vec<PlatformEvent> {
        self.queue.borrow_mut().drain(..).collect()
    }

    fn push(&self, event: PlatformEvent) {
        self.queue.borrow_mut().push_back(event);
    }

    /// Lee un archivo del disco (escritorio: línea de comandos).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_path(&self, path: &Path, purpose: OpenPurpose) {
        match std::fs::read(path) {
            Ok(bytes) => self.push(PlatformEvent::Opened {
                purpose,
                name: path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file")
                    .to_string(),
                target: SaveTarget::from_path(path),
                bytes,
            }),
            Err(e) => self.push(PlatformEvent::Error(format!("{}: {e}", path.display()))),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_file(&self, filters: &[FileFilter], purpose: OpenPurpose, _ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new();
        for f in filters {
            dialog = dialog.add_filter(f.name, f.extensions);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path, purpose);
        }
    }

    /// Abre el diálogo directamente en `dir` (carpetas recientes).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_file_in(
        &self,
        filters: &[FileFilter],
        dir: &Path,
        purpose: OpenPurpose,
        _ctx: &egui::Context,
    ) {
        let mut dialog = rfd::FileDialog::new().set_directory(dir);
        for f in filters {
            dialog = dialog.add_filter(f.name, f.extensions);
        }
        if let Some(path) = dialog.pick_file() {
            self.open_path(&path, purpose);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open_file(&self, filters: &[FileFilter], purpose: OpenPurpose, ctx: &egui::Context) {
        let mut dialog = rfd::AsyncFileDialog::new();
        for f in filters {
            dialog = dialog.add_filter(f.name, f.extensions);
        }
        let this = self.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(handle) = dialog.pick_file().await {
                let bytes = handle.read().await;
                this.push(PlatformEvent::Opened {
                    purpose,
                    name: handle.file_name(),
                    target: None,
                    bytes,
                });
                ctx.request_repaint();
            }
        });
    }

    /// Archivo soltado sobre la ventana.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_dropped(&self, file: &egui::DroppedFileHandle, _ctx: &egui::Context) {
        self.open_path(file.path(), OpenPurpose::Dropped);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn open_dropped(&self, file: &egui::DroppedFileHandle, ctx: &egui::Context) {
        let file = file.clone();
        let this = self.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let name = file.path().to_string_lossy().into_owned();
            match file.bytes_async().await {
                Ok(bytes) => this.push(PlatformEvent::Opened {
                    purpose: OpenPurpose::Dropped,
                    name,
                    target: None,
                    bytes,
                }),
                Err(e) => this.push(PlatformEvent::Error(e)),
            }
            ctx.request_repaint();
        });
    }

    /// Pregunta dónde guardar. En web no hay diálogo: se descarga con el nombre sugerido.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn choose_save(&self, suggested: &str, filters: &[FileFilter]) -> Option<SaveTarget> {
        let mut dialog = rfd::FileDialog::new().set_file_name(suggested);
        for f in filters {
            dialog = dialog.add_filter(f.name, f.extensions);
        }
        dialog.save_file().and_then(|p| SaveTarget::from_path(&p))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn choose_save(&self, suggested: &str, _filters: &[FileFilter]) -> Option<SaveTarget> {
        Some(SaveTarget::named(suggested))
    }

    /// Escribe (escritorio, de forma atómica) o descarga (web).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn write(&self, target: &SaveTarget, bytes: &[u8]) -> Result<(), String> {
        let path = target
            .path()
            .ok_or_else(|| format!("no folder chosen for {}", target.file_name))?;
        write_atomic(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn write(&self, target: &SaveTarget, bytes: &[u8]) -> Result<(), String> {
        web::download(&target.file_name, bytes)
    }

    /// En web "guardar" siempre descarga; la UI lo dice así.
    pub fn saves_by_download(&self) -> bool {
        cfg!(target_arch = "wasm32")
    }
}

/// Cómo arrancó la app: con el documento de ejemplo y/o un archivo a abrir.
pub struct LaunchOptions {
    pub demo: bool,
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    pub open: Option<PathBuf>,
}

/// Escritorio: `--demo`, `CHIMPSPRITE_DEMO` y la ruta de un archivo.
#[cfg(not(target_arch = "wasm32"))]
pub fn launch_options() -> LaunchOptions {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    LaunchOptions {
        demo: std::env::var_os("CHIMPSPRITE_DEMO").is_some() || args.iter().any(|a| a == "--demo"),
        open: args
            .iter()
            .find(|a| !a.to_string_lossy().starts_with("--"))
            .map(PathBuf::from),
    }
}

/// Web: `?demo` en la URL.
#[cfg(target_arch = "wasm32")]
pub fn launch_options() -> LaunchOptions {
    let search = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default();
    LaunchOptions {
        demo: search
            .trim_start_matches('?')
            .split('&')
            .any(|p| p == "demo" || p.starts_with("demo=")),
        open: None,
    }
}

/// Web: el navegador pregunta antes de cerrar la pestaña si hay cambios sin guardar.
#[cfg(target_arch = "wasm32")]
pub fn install_unload_guard() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    let Some(window) = web_sys::window() else {
        return;
    };
    let handler = Closure::<dyn FnMut(web_sys::BeforeUnloadEvent)>::new(
        |event: web_sys::BeforeUnloadEvent| {
            if web::UNSAVED.with(|u| u.get()) {
                event.prevent_default();
                event.set_return_value("unsaved");
            }
        },
    );
    window.set_onbeforeunload(Some(handler.as_ref().unchecked_ref()));
    handler.forget();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn install_unload_guard() {}

/// Título de la ventana (escritorio) o de la pestaña (web).
#[cfg(not(target_arch = "wasm32"))]
pub fn set_title(ctx: &egui::Context, title: &str) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.to_string()));
}

#[cfg(target_arch = "wasm32")]
pub fn set_title(_ctx: &egui::Context, title: &str) {
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        document.set_title(title);
    }
}

/// Informa si hay cambios sin guardar (lo usa el aviso de cierre en web).
#[cfg(target_arch = "wasm32")]
pub fn set_unsaved(unsaved: bool) {
    web::UNSAVED.with(|u| u.set(unsaved));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set_unsaved(_unsaved: bool) {}

/// Abre una carpeta o una URL con el programa del sistema.
#[cfg(not(target_arch = "wasm32"))]
fn open_with_system(target: &std::ffi::OsStr) -> Result<(), String> {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(target)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Abre una carpeta en el explorador de archivos del sistema.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_folder(dir: &Path) -> Result<(), String> {
    open_with_system(dir.as_os_str())
}

/// Abre una URL en el navegador. En escritorio no sirve `ctx.open_url`: eframe va sin la
/// feature `links` (sin `webbrowser`), así que se usa el mismo programa que `open_folder`.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_url(_ctx: &egui::Context, url: &str) -> Result<(), String> {
    open_with_system(url.as_ref())
}

/// En web eframe abre la URL en una pestaña nueva.
#[cfg(target_arch = "wasm32")]
pub fn open_url(ctx: &egui::Context, url: &str) -> Result<(), String> {
    ctx.open_url(egui::OpenUrl::new_tab(url));
    Ok(())
}

/// En web no hay carpetas: los presets viven en `localStorage`.
#[cfg(target_arch = "wasm32")]
pub fn open_folder(_dir: &Path) -> Result<(), String> {
    Ok(())
}

/// Escribe un temporal y lo renombra, así un fallo no deja el archivo a medias.
#[cfg(not(target_arch = "wasm32"))]
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

#[cfg(target_arch = "wasm32")]
pub mod web {
    use std::cell::Cell;

    use wasm_bindgen::JsCast;

    thread_local! {
        pub static UNSAVED: Cell<bool> = const { Cell::new(false) };
    }

    fn js_error(e: wasm_bindgen::JsValue) -> String {
        format!("{e:?}")
    }

    /// Descarga `bytes` como `file_name` (Blob + enlace temporal).
    pub fn download(file_name: &str, bytes: &[u8]) -> Result<(), String> {
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::of1(&array);
        let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).map_err(js_error)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let anchor: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(js_error)?
            .dyn_into()
            .map_err(|_| "not an anchor".to_string())?;
        anchor.set_href(&url);
        anchor.set_download(file_name);
        anchor.click();
        web_sys::Url::revoke_object_url(&url).map_err(js_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_target_paths() {
        let t = SaveTarget::from_path(Path::new("/tmp/art/hero.aseprite")).unwrap();
        assert_eq!(t.stem(), "hero");
        assert_eq!(t.file_name, "hero.aseprite");
        let json = t.sibling("hero.json");
        assert_eq!(json.path(), Some(PathBuf::from("/tmp/art/hero.json")));
        assert_eq!(SaveTarget::named("x.png").path(), None);
    }

    #[test]
    fn opened_files_arrive_through_the_queue() {
        let dir = std::env::temp_dir().join(format!("chimpsprite-platform-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.chimp");
        let platform = Platform::default();
        platform
            .write(&SaveTarget::from_path(&path).unwrap(), b"abc")
            .unwrap();
        platform.open_path(&path, OpenPurpose::Open);
        platform.open_path(&dir.join("missing.png"), OpenPurpose::Open);
        let events = platform.poll();
        assert!(
            matches!(&events[0], PlatformEvent::Opened { name, bytes, .. } if name == "a.chimp" && bytes == b"abc")
        );
        assert!(matches!(events[1], PlatformEvent::Error(_)));
        assert!(platform.poll().is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
