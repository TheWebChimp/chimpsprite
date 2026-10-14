//! Archivos y carpetas recientes (persisten entre sesiones en escritorio).

// En web no hay rutas: la lista existe pero no se usa.
#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_FILES: usize = 15;
const MAX_FOLDERS: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFiles {
    files: Vec<PathBuf>,
}

impl RecentFiles {
    /// Lleva `path` al principio (sin duplicados).
    pub fn add(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
        self.files.insert(0, path.to_path_buf());
        self.files.truncate(MAX_FILES);
    }

    pub fn remove(&mut self, path: &Path) {
        self.files.retain(|p| p != path);
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Carpetas de los archivos recientes, en el mismo orden y sin repetir.
    pub fn folders(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = Vec::new();
        for dir in self.files.iter().filter_map(|f| f.parent()) {
            if !out.iter().any(|d| d == dir) {
                out.push(dir.to_path_buf());
            }
        }
        out.truncate(MAX_FOLDERS);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn most_recent_first_without_duplicates() {
        let mut r = RecentFiles::default();
        r.add(Path::new("/a/one.chimp"));
        r.add(Path::new("/b/two.aseprite"));
        r.add(Path::new("/a/one.chimp"));
        assert_eq!(
            r.files(),
            &[
                PathBuf::from("/a/one.chimp"),
                PathBuf::from("/b/two.aseprite")
            ]
        );
        assert_eq!(r.folders(), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
        for i in 0..20 {
            r.add(Path::new(&format!("/c/{i}.png")));
        }
        assert_eq!(r.files().len(), MAX_FILES);
        r.remove(Path::new("/c/19.png"));
        assert_eq!(r.files()[0], PathBuf::from("/c/18.png"));
    }
}
