#![allow(dead_code)] // provisional: Task 6 lo quita cuando la UI use la biblioteca

//! Biblioteca de paletas: presets incluidos más los del usuario.
//!
//! En escritorio, los presets del usuario son archivos en `<storage_dir>/palettes/`. En web,
//! una lista JSON en `localStorage`.

use std::path::PathBuf;

use cs_core::{BUILTIN_PALETTES, Palette, Rgba};
use cs_io::palette_files::{self, NamedPalette};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetSource {
    Builtin,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub author: String,
    pub palette: Palette,
    pub source: PresetSource,
}

/// Dónde viven los presets del usuario.
pub trait PresetStore {
    fn load(&self) -> Vec<NamedPalette>;
    /// Escribe o reemplaza el preset `palette.name`.
    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String>;
    fn delete(&mut self, name: &str) -> Result<(), String>;
    /// Carpeta para "Open Folder" (solo escritorio).
    fn folder(&self) -> Option<PathBuf>;
}

pub struct PaletteLibrary {
    builtin: Vec<Preset>,
    user: Vec<Preset>,
    default: Option<String>,
    store: Box<dyn PresetStore>,
}

impl PaletteLibrary {
    pub fn new(store: Box<dyn PresetStore>) -> Self {
        let builtin = BUILTIN_PALETTES
            .iter()
            .map(|p| Preset {
                name: p.name.into(),
                author: p.author.into(),
                palette: p.palette(),
                source: PresetSource::Builtin,
            })
            .collect();
        let mut lib = Self {
            builtin,
            user: Vec::new(),
            default: None,
            store,
        };
        lib.reload();
        lib
    }

    /// Carpeta de usuario en escritorio y `localStorage` en web.
    pub fn for_platform() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let store: Box<dyn PresetStore> = Box::new(DirStore::new(
            eframe::storage_dir("ChimpSprite")
                .unwrap_or_else(|| PathBuf::from("."))
                .join("palettes"),
        ));
        #[cfg(target_arch = "wasm32")]
        let store: Box<dyn PresetStore> = Box::new(web::LocalStore);
        Self::new(store)
    }

    /// Vuelve a leer los presets del usuario (los que se soltaron a mano en la carpeta).
    pub fn reload(&mut self) {
        let mut user: Vec<Preset> = self
            .store
            .load()
            .into_iter()
            .map(|p| Preset {
                name: p.name,
                author: String::new(),
                palette: p.palette,
                source: PresetSource::User,
            })
            .collect();
        user.sort_by_key(|p| p.name.to_lowercase());
        self.user = user;
    }

    pub fn builtin(&self) -> &[Preset] {
        &self.builtin
    }

    pub fn user(&self) -> &[Preset] {
        &self.user
    }

    /// ¿Hay un preset de usuario con ese nombre? (sin importar mayúsculas, como en Windows).
    pub fn has_user(&self, name: &str) -> bool {
        self.user.iter().any(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        self.store.save(palette, comments)?;
        self.reload();
        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        self.store.delete(name)?;
        self.reload();
        Ok(())
    }

    /// Primero los del usuario (pueden tener el mismo nombre que uno incluido).
    pub fn find(&self, name: &str) -> Option<&Preset> {
        self.user
            .iter()
            .chain(&self.builtin)
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn default_name(&self) -> Option<&str> {
        self.default.as_deref()
    }

    pub fn set_default(&mut self, name: Option<String>) {
        self.default = name;
    }

    /// La paleta de los sprites nuevos: el preset por defecto, o DB32 si ya no existe.
    pub fn default_palette(&self) -> Palette {
        self.default
            .as_deref()
            .and_then(|n| self.find(n))
            .map_or_else(Palette::db32, |p| p.palette.clone())
    }

    pub fn folder(&self) -> Option<PathBuf> {
        self.store.folder()
    }
}

/// `<nombre>.gpl` sin caracteres que Windows no acepta.
pub fn preset_file_name(name: &str) -> String {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let clean = if clean.is_empty() {
        "Palette".to_string()
    } else {
        clean
    };
    format!("{clean}.gpl")
}

/// Escritorio: un archivo por preset.
pub struct DirStore {
    dir: PathBuf,
}

impl DirStore {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl PresetStore for DirStore {
    fn load(&self) -> Vec<NamedPalette> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for path in entries.flatten().map(|e| e.path()) {
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if palette_files::format_for(name).is_none() {
                continue;
            }
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| palette_files::read_palette(name, &b).map_err(|e| e.to_string()))
            {
                Ok(p) => out.push(p),
                Err(e) => log::warn!("skipping palette {}: {e}", path.display()),
            }
        }
        out
    }

    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Con otra extensión o mayúsculas distintas sería un duplicado: se borra primero.
        let _ = self.delete(&palette.name);
        let path = self.dir.join(preset_file_name(&palette.name));
        std::fs::write(path, palette_files::write_gpl(palette, comments)).map_err(|e| e.to_string())
    }

    fn delete(&mut self, name: &str) -> Result<(), String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(());
        };
        for path in entries.flatten().map(|e| e.path()) {
            let Some(file) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let matches = palette_files::format_for(file).is_some()
                && std::fs::read(&path)
                    .ok()
                    .and_then(|b| palette_files::read_palette(file, &b).ok())
                    .is_some_and(|p| p.name.eq_ignore_ascii_case(name));
            if matches {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn folder(&self) -> Option<PathBuf> {
        Some(self.dir.clone())
    }
}

#[derive(Serialize, Deserialize)]
struct WebPalette {
    name: String,
    colors: Vec<String>,
}

/// Lista JSON de `{ name, colors: ["rrggbb", …] }` (lo que se guarda en web).
pub fn encode_web(list: &[NamedPalette]) -> String {
    let items: Vec<WebPalette> = list
        .iter()
        .map(|p| WebPalette {
            name: p.name.clone(),
            colors: p
                .palette
                .colors()
                .iter()
                .map(|c| format!("{:02x}{:02x}{:02x}", c.r, c.g, c.b))
                .collect(),
        })
        .collect();
    serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
}

/// Lo ilegible se descarta (lista vacía o entradas sueltas).
pub fn decode_web(text: &str) -> Vec<NamedPalette> {
    let items: Vec<WebPalette> = serde_json::from_str(text).unwrap_or_default();
    items
        .into_iter()
        .filter_map(|item| {
            let colors: Option<Vec<Rgba>> = item
                .colors
                .iter()
                .map(|h| u32::from_str_radix(h, 16).ok().map(Rgba::from_hex))
                .collect();
            let colors =
                colors.filter(|c| !c.is_empty() && c.len() <= palette_files::MAX_COLORS)?;
            Some(NamedPalette {
                name: item.name,
                palette: Palette::new(colors),
            })
        })
        .collect()
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;

    const KEY: &str = "chimpsprite.palettes";

    fn local() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub struct LocalStore;

    impl LocalStore {
        fn write(list: &[NamedPalette]) -> Result<(), String> {
            local()
                .ok_or("browser storage is not available")?
                .set_item(KEY, &encode_web(list))
                .map_err(|_| "the browser refused to store the palette (quota?)".to_string())
        }
    }

    impl PresetStore for LocalStore {
        fn load(&self) -> Vec<NamedPalette> {
            local()
                .and_then(|s| s.get_item(KEY).ok().flatten())
                .map(|t| decode_web(&t))
                .unwrap_or_default()
        }

        fn save(&mut self, palette: &NamedPalette, _comments: &[String]) -> Result<(), String> {
            let mut list = self.load();
            list.retain(|p| !p.name.eq_ignore_ascii_case(&palette.name));
            list.push(palette.clone());
            Self::write(&list)
        }

        fn delete(&mut self, name: &str) -> Result<(), String> {
            let mut list = self.load();
            list.retain(|p| !p.name.eq_ignore_ascii_case(name));
            Self::write(&list)
        }

        fn folder(&self) -> Option<PathBuf> {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::Rgba;

    use super::*;

    fn named(name: &str, colors: &[u32]) -> NamedPalette {
        NamedPalette {
            name: name.into(),
            palette: Palette::new(colors.iter().map(|&c| Rgba::from_hex(c)).collect()),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cs-palettes-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(preset_file_name("Night: blue/ish?"), "Night_ blue_ish_.gpl");
        assert_eq!(preset_file_name("  "), "Palette.gpl");
    }

    #[test]
    fn folder_reads_valid_files_and_skips_broken_ones() {
        let dir = temp_dir("read");
        std::fs::write(dir.join("two.hex"), "ff0000\n00ff00\n").unwrap();
        std::fs::write(dir.join("broken.gpl"), "not a palette").unwrap();
        std::fs::write(dir.join("notes.txt"), "hello").unwrap();
        let lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        let names: Vec<&str> = lib.user().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["two"]);
        assert_eq!(lib.builtin().len(), cs_core::BUILTIN_PALETTES.len());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_replace_and_delete() {
        let dir = temp_dir("save");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("Mine", &[0x111111]), &[]).unwrap();
        assert!(lib.has_user("mine"), "sin importar mayúsculas");
        lib.save(&named("Mine", &[0x222222, 0x333333]), &[])
            .unwrap();
        assert_eq!(lib.user().len(), 1);
        assert_eq!(lib.find("Mine").unwrap().palette.len(), 2);
        assert!(dir.join("Mine.gpl").exists());
        lib.delete("Mine").unwrap();
        assert!(lib.user().is_empty());
        assert!(!dir.join("Mine.gpl").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn default_falls_back_to_db32() {
        let dir = temp_dir("default");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        assert_eq!(lib.default_palette(), Palette::db32());
        lib.set_default(Some("PICO-8".into()));
        assert_eq!(lib.default_palette().len(), 16);
        lib.set_default(Some("Gone".into()));
        assert_eq!(lib.default_palette(), Palette::db32());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn web_encoding_roundtrip() {
        let list = vec![named("A", &[0x010203]), named("B", &[0xffffff, 0x000000])];
        let text = encode_web(&list);
        assert!(text.contains("\"010203\""));
        assert_eq!(decode_web(&text), list);
        assert!(decode_web("{nope").is_empty());
    }
}
