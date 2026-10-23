//! Recarga en vivo del skin (solo escritorio). Con `CHIMPSPRITE_SKIN=<ruta>` la app lee el skin de
//! ese archivo y lo vuelve a leer cuando cambia su fecha de modificación.

use std::path::PathBuf;
use std::time::SystemTime;

/// Variable de entorno con la ruta del skin a vigilar.
pub const SKIN_ENV: &str = "CHIMPSPRITE_SKIN";

/// Cada cuánto se mira la fecha del archivo, como mucho (segundos).
const INTERVAL: f64 = 1.0;

pub struct SkinWatcher {
    path: PathBuf,
    /// Fecha de la última lectura.
    modified: Option<SystemTime>,
    /// Última revisión (reloj de egui).
    checked: Option<f64>,
    /// Ya se avisó que no se puede leer (se avisa una vez hasta que vuelva a andar).
    warned: bool,
}

impl SkinWatcher {
    pub fn from_env() -> Option<Self> {
        std::env::var_os(SKIN_ENV).map(|p| Self::new(PathBuf::from(p)))
    }

    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            modified: None,
            checked: None,
            warned: false,
        }
    }

    /// Los bytes del archivo si cambió desde la última lectura. `now` es el reloj de egui (s).
    pub fn poll(&mut self, now: f64) -> Option<Vec<u8>> {
        if self.checked.is_some_and(|t| now - t < INTERVAL) {
            return None;
        }
        self.checked = Some(now);
        let modified = match std::fs::metadata(&self.path).and_then(|m| m.modified()) {
            Ok(m) => m,
            Err(e) => {
                self.warn(&e);
                return None;
            }
        };
        if self.modified == Some(modified) {
            return None;
        }
        match std::fs::read(&self.path) {
            Ok(bytes) => {
                self.modified = Some(modified);
                self.warned = false;
                Some(bytes)
            }
            Err(e) => {
                self.warn(&e);
                None
            }
        }
    }

    fn warn(&mut self, e: &std::io::Error) {
        if !self.warned {
            log::warn!("skin {}: {e}", self.path.display());
            self.warned = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    fn temp_file(tag: &str) -> PathBuf {
        let name = format!("cs-skin-{}-{tag}.aseprite", std::process::id());
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn reads_once_and_again_when_the_file_changes() {
        let path = temp_file("changes");
        std::fs::write(&path, b"one").unwrap();
        let mut w = SkinWatcher::new(path.clone());
        assert_eq!(w.poll(0.0).as_deref(), Some(&b"one"[..]));
        assert_eq!(w.poll(0.5), None, "at most once per second");
        assert_eq!(w.poll(1.5), None, "unchanged");
        std::fs::write(&path, b"two").unwrap();
        let later = SystemTime::now() + Duration::from_secs(10);
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(later).unwrap();
        drop(file);
        assert_eq!(w.poll(1.6), None, "still within the second");
        assert_eq!(w.poll(3.0).as_deref(), Some(&b"two"[..]));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_file_gives_nothing() {
        let mut w = SkinWatcher::new(temp_file("missing"));
        assert_eq!(w.poll(0.0), None);
        assert_eq!(w.poll(5.0), None);
    }
}
