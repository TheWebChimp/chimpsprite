//! Biblioteca de paletas: presets incluidos más los del usuario.
//!
//! En escritorio, los presets del usuario son archivos en `<storage_dir>/palettes/`. En web,
//! una lista JSON en `localStorage`.

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;

use cs_core::{BUILTIN_PALETTES, Palette, Rgba};
use cs_io::palette_files::{self, NamedPalette};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetSource {
    Builtin,
    User,
}

impl PresetSource {
    /// Prefijo con el que se guarda la paleta por defecto (`builtin:PICO-8`, `user:Mine`).
    fn tag(self) -> &'static str {
        match self {
            PresetSource::Builtin => "builtin:",
            PresetSource::User => "user:",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: String,
    pub author: String,
    pub palette: Palette,
    pub source: PresetSource,
    /// Se puede borrar desde la ventana: solo los del usuario guardados como texto
    /// (`.gpl`, `.hex`, `.pal`). Un `.png` o `.aseprite` soltado en la carpeta no se borra.
    pub deletable: bool,
}

/// Un preset tal como lo devuelve el almacén.
pub struct StoredPreset {
    pub palette: NamedPalette,
    /// Viene de un formato de texto (o de `localStorage`): borrarlo no pierde otra cosa.
    pub deletable: bool,
}

/// Mismo nombre de preset, sin importar mayúsculas (también fuera de ASCII: "Ñandú" = "ñandú").
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Dónde viven los presets del usuario.
pub trait PresetStore {
    fn load(&self) -> Vec<StoredPreset>;
    /// Escribe o reemplaza el preset `palette.name`.
    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String>;
    fn delete(&mut self, name: &str) -> Result<(), String>;
    /// Carpeta para "Open Folder" (solo escritorio).
    fn folder(&self) -> Option<PathBuf>;
}

pub struct PaletteLibrary {
    builtin: Vec<Preset>,
    user: Vec<Preset>,
    /// `builtin:<nombre>` o `user:<nombre>`; sin prefijo si viene de una versión anterior.
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
                deletable: false,
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
                name: p.palette.name,
                author: String::new(),
                palette: p.palette.palette,
                source: PresetSource::User,
                deletable: p.deletable,
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
        self.user.iter().any(|p| same_name(&p.name, name))
    }

    /// Recarga aunque falle: lo escrito antes del error ya está en el almacén.
    pub fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        let result = self.store.save(palette, comments);
        self.reload();
        result
    }

    pub fn delete(&mut self, name: &str) -> Result<(), String> {
        let result = self.store.delete(name);
        self.reload();
        result
    }

    /// Primero los del usuario (pueden tener el mismo nombre que uno incluido).
    pub fn find(&self, name: &str) -> Option<&Preset> {
        self.user
            .iter()
            .chain(&self.builtin)
            .find(|p| same_name(&p.name, name))
    }

    /// El valor guardado tal cual (para persistirlo).
    pub fn stored_default(&self) -> Option<&str> {
        self.default.as_deref()
    }

    /// Restaura el valor guardado (con prefijo o, de versiones anteriores, solo el nombre).
    pub fn set_stored_default(&mut self, value: Option<String>) {
        self.default = value;
    }

    pub fn set_default(&mut self, source: PresetSource, name: &str) {
        self.default = Some(format!("{}{name}", source.tag()));
    }

    /// El preset por defecto, buscado en su origen. Sin prefijo, como antes: primero el usuario.
    pub fn default_preset(&self) -> Option<&Preset> {
        let value = self.default.as_deref()?;
        let (list, name) = if let Some(name) = value.strip_prefix(PresetSource::Builtin.tag()) {
            (&self.builtin, name)
        } else if let Some(name) = value.strip_prefix(PresetSource::User.tag()) {
            (&self.user, name)
        } else {
            return self.find(value);
        };
        list.iter().find(|p| same_name(&p.name, name))
    }

    /// ¿Es esta fila la del preset por defecto? (una sola, aunque haya nombres repetidos).
    pub fn is_default(&self, preset: &Preset) -> bool {
        self.default_preset()
            .is_some_and(|d| std::ptr::eq(d, preset))
    }

    /// La paleta de los sprites nuevos: el preset por defecto, o DB32 si ya no existe.
    pub fn default_palette(&self) -> Palette {
        self.default_preset()
            .map_or_else(Palette::db32, |p| p.palette.clone())
    }

    pub fn folder(&self) -> Option<PathBuf> {
        self.store.folder()
    }
}

/// `<nombre>.gpl` sin caracteres que Windows no acepta.
#[cfg(not(target_arch = "wasm32"))]
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

/// Formatos que escribimos nosotros: los únicos que se pueden borrar o reemplazar.
#[cfg(not(target_arch = "wasm32"))]
fn is_text_format(file_name: &str) -> bool {
    use palette_files::PaletteFormat;
    matches!(
        palette_files::format_for(file_name),
        Some(PaletteFormat::Gpl | PaletteFormat::Hex | PaletteFormat::Pal)
    )
}

/// Nombre interno de un archivo de paleta (`None` si no se puede leer).
#[cfg(not(target_arch = "wasm32"))]
fn palette_name_in(path: &Path) -> Option<String> {
    let file = path.file_name()?.to_str()?;
    let bytes = std::fs::read(path).ok()?;
    palette_files::read_palette(file, &bytes)
        .ok()
        .map(|p| p.name)
}

/// Escritorio: un archivo por preset.
#[cfg(not(target_arch = "wasm32"))]
pub struct DirStore {
    dir: PathBuf,
    /// Archivos ya avisados: un archivo ilegible se registra una sola vez.
    warned: std::cell::RefCell<std::collections::HashSet<PathBuf>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DirStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            warned: Default::default(),
        }
    }

    /// Archivo para el preset `name`: `<nombre>.gpl`, o `<nombre> (2).gpl`, `(3)`… si ese
    /// archivo ya guarda otro preset ("Night?" y "Night*" dan los dos `Night_.gpl`).
    fn target_for(&self, name: &str) -> PathBuf {
        let file = preset_file_name(name);
        let stem = file.strip_suffix(".gpl").unwrap_or(&file);
        let mut path = self.dir.join(&file);
        let mut n = 2;
        // Un archivo ilegible cuenta como ajeno: no se pisa.
        while path.exists() && !palette_name_in(&path).is_some_and(|p| same_name(&p, name)) {
            path = self.dir.join(format!("{stem} ({n}).gpl"));
            n += 1;
        }
        path
    }

    /// Borra los archivos de texto (`.gpl`, `.hex`, `.pal`) cuyo nombre interno coincide,
    /// salvo `keep`. Los `.png` y `.aseprite` nunca se tocan: son archivos del usuario.
    fn remove_matching(&self, name: &str, keep: Option<&Path>) -> Result<(), String> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(());
        };
        // Se compara la ruta real: en Windows `Mine.gpl` y `mine.gpl` son el mismo archivo.
        let keep = keep.and_then(|k| std::fs::canonicalize(k).ok());
        for path in entries.flatten().map(|e| e.path()) {
            let Some(file) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !is_text_format(file) {
                continue;
            }
            if keep.is_some() && std::fs::canonicalize(&path).ok() == keep {
                continue;
            }
            if palette_name_in(&path).is_some_and(|p| same_name(&p, name)) {
                std::fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PresetStore for DirStore {
    fn load(&self) -> Vec<StoredPreset> {
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
                Ok(p) => out.push(StoredPreset {
                    palette: p,
                    deletable: is_text_format(name),
                }),
                Err(e) => {
                    if self.warned.borrow_mut().insert(path.clone()) {
                        log::warn!("skipping palette {}: {e}", path.display());
                    }
                }
            }
        }
        out
    }

    fn save(&mut self, palette: &NamedPalette, comments: &[String]) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Primero se escribe el nuevo (temporal + renombrar): si falla, el anterior sigue intacto.
        let target = self.target_for(&palette.name);
        crate::platform::write_atomic(&target, &palette_files::write_gpl(palette, comments))
            .map_err(|e| e.to_string())?;
        // Con otra extensión o mayúsculas distintas sería un duplicado: se borra después.
        self.remove_matching(&palette.name, Some(&target))
    }

    fn delete(&mut self, name: &str) -> Result<(), String> {
        self.remove_matching(name, None)
    }

    fn folder(&self) -> Option<PathBuf> {
        Some(self.dir.clone())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // solo web (y los tests en escritorio)
#[derive(Serialize, Deserialize)]
struct WebPalette {
    name: String,
    colors: Vec<String>,
}

/// Lista JSON de `{ name, colors: ["rrggbb", …] }` (lo que se guarda en web).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // solo web (y los tests en escritorio)
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

/// Lo ilegible se descarta entrada por entrada (una mala no tira las demás).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // solo web (y los tests en escritorio)
pub fn decode_web(text: &str) -> Vec<NamedPalette> {
    let items: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();
    items
        .into_iter()
        .filter_map(|value| {
            let item = serde_json::from_value::<WebPalette>(value).ok()?;
            let colors: Option<Vec<Rgba>> = item
                .colors
                .iter()
                .map(|h| {
                    let valid = h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit());
                    if !valid {
                        return None;
                    }
                    u32::from_str_radix(h, 16).ok().map(Rgba::from_hex)
                })
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
        fn read() -> Vec<NamedPalette> {
            local()
                .and_then(|s| s.get_item(KEY).ok().flatten())
                .map(|t| decode_web(&t))
                .unwrap_or_default()
        }

        fn write(list: &[NamedPalette]) -> Result<(), String> {
            local()
                .ok_or("browser storage is not available")?
                .set_item(KEY, &encode_web(list))
                .map_err(|_| "the browser refused to store the palette (quota?)".to_string())
        }
    }

    impl PresetStore for LocalStore {
        /// En web todo lo guardado lo escribió la app: siempre se puede borrar.
        fn load(&self) -> Vec<StoredPreset> {
            Self::read()
                .into_iter()
                .map(|palette| StoredPreset {
                    palette,
                    deletable: true,
                })
                .collect()
        }

        fn save(&mut self, palette: &NamedPalette, _comments: &[String]) -> Result<(), String> {
            let mut list = Self::read();
            list.retain(|p| !same_name(&p.name, &palette.name));
            list.push(palette.clone());
            Self::write(&list)
        }

        fn delete(&mut self, name: &str) -> Result<(), String> {
            let mut list = Self::read();
            list.retain(|p| !same_name(&p.name, name));
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
    fn save_replaces_a_preset_with_another_extension_or_case() {
        let dir = temp_dir("replace-ext");
        std::fs::write(
            dir.join("Mine.hex"),
            "ff0000
",
        )
        .unwrap();
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("mine", &[0x00ff00]), &[]).unwrap();
        assert_eq!(lib.user().len(), 1);
        let found = lib.find("mine").unwrap();
        assert_eq!(found.palette.colors()[0], Rgba::from_hex(0x00ff00));
        assert!(!dir.join("Mine.hex").exists());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn web_decoding_skips_only_broken_entries() {
        let json = r#"[
            {"name":"ok","colors":["ff0000","00ff00"]},
            {"name":"num","colors":[1]},
            {"name":"nohex","colors":["zzzzzz"]},
            {"name":"long","colors":["1ffffff"]}
        ]"#;
        let got = decode_web(json);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "ok");
        assert_eq!(got[0].palette.len(), 2);
    }

    #[test]
    fn default_falls_back_to_db32() {
        let dir = temp_dir("default");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        assert_eq!(lib.default_palette(), Palette::db32());
        lib.set_default(PresetSource::Builtin, "PICO-8");
        assert_eq!(lib.default_palette().len(), 16);
        lib.set_default(PresetSource::User, "Gone");
        assert_eq!(lib.default_palette(), Palette::db32());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn colliding_file_names_keep_both_presets() {
        let dir = temp_dir("collide");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("Night?", &[0x111111]), &[]).unwrap();
        lib.save(&named("Night*", &[0x222222, 0x333333]), &[])
            .unwrap();
        assert_eq!(lib.user().len(), 2);
        assert_eq!(lib.find("Night?").unwrap().palette.len(), 1);
        assert_eq!(lib.find("Night*").unwrap().palette.len(), 2);
        assert!(dir.join("Night_.gpl").exists());
        assert!(dir.join("Night_ (2).gpl").exists());
        // Reemplazar el segundo usa su mismo archivo.
        lib.save(&named("Night*", &[0x444444]), &[]).unwrap();
        assert_eq!(lib.user().len(), 2);
        assert_eq!(lib.find("Night*").unwrap().palette.len(), 1);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn names_match_case_insensitively_beyond_ascii() {
        let dir = temp_dir("unicode");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("Ñandú", &[0x111111]), &[]).unwrap();
        assert!(lib.has_user("ñandú"));
        lib.save(&named("ñandú", &[0x222222, 0x333333]), &[])
            .unwrap();
        assert_eq!(lib.user().len(), 1);
        assert_eq!(lib.find("ÑANDÚ").unwrap().palette.len(), 2);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sprite_files_are_never_deleted_or_replaced() {
        let dir = temp_dir("sprite");
        let mut sprite = cs_core::Sprite::new(4, 4, cs_core::ColorMode::Rgba);
        sprite.palette = Palette::new(vec![Rgba::from_hex(0xff0000)]);
        std::fs::write(dir.join("Mine.aseprite"), cs_io::aseprite::write(&sprite)).unwrap();
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        let found = lib.find("Mine").unwrap();
        assert!(!found.deletable);
        // Replace: se agrega el .gpl y el .aseprite queda (dos filas con el mismo nombre).
        lib.save(&named("Mine", &[0x00ff00]), &[]).unwrap();
        assert!(dir.join("Mine.aseprite").exists());
        assert!(dir.join("Mine.gpl").exists());
        assert_eq!(lib.user().len(), 2);
        let deletable: Vec<bool> = lib.user().iter().map(|p| p.deletable).collect();
        assert!(deletable.contains(&true) && deletable.contains(&false));
        // Delete solo borra el .gpl.
        lib.delete("Mine").unwrap();
        assert!(dir.join("Mine.aseprite").exists());
        assert!(!dir.join("Mine.gpl").exists());
        assert_eq!(lib.user().len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn default_is_looked_up_in_its_source() {
        let dir = temp_dir("default-source");
        // Lo que creará el importador de Lospec: un preset de usuario con nombre de uno incluido.
        std::fs::write(dir.join("PICO-8.hex"), "ff0000\n00ff00\n").unwrap();
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        assert_eq!(lib.find("PICO-8").unwrap().source, PresetSource::User);
        lib.set_default(PresetSource::Builtin, "PICO-8");
        assert_eq!(lib.default_palette().len(), 16);
        assert_eq!(lib.stored_default(), Some("builtin:PICO-8"));
        let builtin = lib.builtin().iter().find(|p| p.name == "PICO-8").unwrap();
        assert!(lib.is_default(builtin));
        assert!(!lib.is_default(&lib.user()[0]));
        lib.set_default(PresetSource::User, "PICO-8");
        assert_eq!(lib.default_palette().len(), 2);
        assert!(lib.is_default(&lib.user()[0]));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_default_without_source_still_resolves() {
        let dir = temp_dir("default-legacy");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.set_stored_default(Some("pico-8".into()));
        assert_eq!(lib.default_palette().len(), 16);
        let builtin = lib.builtin().iter().find(|p| p.name == "PICO-8").unwrap();
        assert!(lib.is_default(builtin));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_save_keeps_the_previous_file() {
        let dir = temp_dir("atomic");
        let mut lib = PaletteLibrary::new(Box::new(DirStore::new(dir.clone())));
        lib.save(&named("Mine", &[0x111111]), &[]).unwrap();
        let before = std::fs::read(dir.join("Mine.gpl")).unwrap();
        // Un directorio donde va el temporal hace fallar la escritura.
        std::fs::create_dir(dir.join("Mine.gpl.tmp")).unwrap();
        assert!(
            lib.save(&named("Mine", &[0x222222, 0x333333]), &[])
                .is_err()
        );
        assert_eq!(std::fs::read(dir.join("Mine.gpl")).unwrap(), before);
        assert_eq!(lib.find("Mine").unwrap().palette.len(), 1);
        // Sin el obstáculo, se reemplaza el contenido.
        std::fs::remove_dir(dir.join("Mine.gpl.tmp")).unwrap();
        lib.save(&named("Mine", &[0x222222, 0x333333]), &[])
            .unwrap();
        assert_eq!(lib.find("Mine").unwrap().palette.len(), 2);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
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
