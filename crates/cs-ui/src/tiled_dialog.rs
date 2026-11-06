//! File → Export Tiled Map…: el mapa (`.tmx`/`.tmj`), sus tilesets y las imágenes de las capas.

use std::path::PathBuf;

use cs_io::tiled::{self, Encoding, MapFormat, TiledExport, TiledFrames, TiledOptions};
use eframe::egui;

use crate::editor::Editor;
use crate::platform::{FileFilter, Platform, SaveTarget};

#[derive(Clone, Debug)]
pub struct TiledDialog {
    pub format: MapFormat,
    /// Tilesets en `.tsx`/`.tsj` aparte (si no, dentro del mapa).
    pub external_tilesets: bool,
    pub encoding: Encoding,
    /// Un mapa por frame (si no, solo el actual).
    pub all_frames: bool,
    /// Nombre del mapa, sin extensión.
    pub name: String,
    /// Carpeta de salida (escritorio); vacía = preguntar con el diálogo de guardar.
    pub folder: String,
}

impl Default for TiledDialog {
    fn default() -> Self {
        Self {
            format: MapFormat::Tmx,
            external_tilesets: true,
            encoding: Encoding::Csv,
            all_frames: false,
            name: String::new(),
            folder: String::new(),
        }
    }
}

/// Lo que pasó al exportar: archivos escritos (rutas, o el `.zip` descargado) y avisos.
pub struct TiledDone {
    pub files: Vec<String>,
    pub warnings: Vec<String>,
}

pub enum TiledResult {
    Open,
    Cancel,
    Done(TiledDone),
    Failed(String),
}

fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Csv => "CSV",
        Encoding::Base64 => "Base64",
        Encoding::Base64Zlib => "Base64 + zlib",
        Encoding::Base64Gzip => "Base64 + gzip",
    }
}

impl TiledDialog {
    /// Reabre con las opciones de la última vez; el nombre es el del artboard y la carpeta, la
    /// última usada o la del documento.
    pub fn reopen(last: &TiledDialog, editor: &Editor, doc_dir: Option<PathBuf>) -> TiledDialog {
        let folder = if last.folder.is_empty() {
            doc_dir.map(|d| d.display().to_string()).unwrap_or_default()
        } else {
            last.folder.clone()
        };
        TiledDialog {
            name: editor.artboard().name.clone(),
            folder,
            ..last.clone()
        }
    }

    pub fn options(&self, editor: &Editor) -> TiledOptions {
        TiledOptions {
            format: self.format,
            external_tilesets: self.external_tilesets,
            encoding: self.encoding,
            frames: if self.all_frames {
                TiledFrames::All
            } else {
                TiledFrames::Current(editor.frame)
            },
        }
    }

    fn stem(&self, editor: &Editor) -> String {
        match self.name.trim() {
            "" => editor.artboard().name.clone(),
            name => name.to_string(),
        }
    }

    /// Los archivos a escribir (sin E/S).
    pub fn build(&self, editor: &Editor, stem: &str) -> Result<TiledExport, String> {
        tiled::export(
            &editor.document,
            editor.sprite(),
            stem,
            &self.options(editor),
        )
        .map_err(|e| e.to_string())
    }

    /// Escribe todo en `dir` (escritorio) o descarga un `.zip` (web, `dir = None`).
    pub fn export_to(
        &self,
        editor: &mut Editor,
        dir: Option<PathBuf>,
        stem: &str,
        platform: &Platform,
    ) -> Result<TiledDone, String> {
        // El diálogo no es modal: lo flotante se confirma antes.
        editor.commit_floating();
        let out = self.build(editor, stem)?;
        let mut files = Vec::new();
        match dir {
            Some(dir) => {
                #[cfg(not(target_arch = "wasm32"))]
                std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                for (name, bytes) in &out.files {
                    let target = SaveTarget {
                        dir: Some(dir.clone()),
                        file_name: name.clone(),
                    };
                    platform.write(&target, bytes)?;
                    files.push(
                        target
                            .path()
                            .map_or(name.clone(), |p| p.display().to_string()),
                    );
                }
            }
            None => {
                let name = format!("{stem}-tiled.zip");
                let zip = out.zip().map_err(|e| e.to_string())?;
                platform.write(&SaveTarget::named(&name), &zip)?;
                files.push(name);
            }
        }
        Ok(TiledDone {
            files,
            warnings: out.warnings,
        })
    }

    fn run_export(&mut self, editor: &mut Editor, platform: &Platform) -> TiledResult {
        let stem = self.stem(editor);
        let result = if platform.saves_by_download() {
            self.export_to(editor, None, &stem, platform)
        } else if self.folder.trim().is_empty() {
            let ext = self.format.map_extension();
            let filter = [FileFilter {
                name: "Tiled map",
                extensions: if ext == "tmx" { &["tmx"] } else { &["tmj"] },
            }];
            let Some(target) = platform.choose_save(&format!("{stem}.{ext}"), &filter) else {
                return TiledResult::Open;
            };
            self.folder = target
                .dir
                .as_ref()
                .map(|d| d.display().to_string())
                .unwrap_or_default();
            let stem = target.stem().to_string();
            self.name = stem.clone();
            self.export_to(editor, target.dir.clone(), &stem, platform)
        } else {
            let dir = PathBuf::from(self.folder.trim());
            self.export_to(editor, Some(dir), &stem, platform)
        };
        match result {
            Ok(done) => TiledResult::Done(done),
            Err(e) => TiledResult::Failed(e),
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        platform: &Platform,
        skin: Option<&crate::skin::Skin>,
    ) -> TiledResult {
        let mut result = TiledResult::Open;
        let web = platform.saves_by_download();
        let (_, closed) = crate::pixel_ui::app_window(
            ctx,
            skin,
            "tiled_export_window",
            "Export Tiled Map",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                egui::Grid::new("tiled_export_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label("Format");
                        ui.horizontal(|ui| {
                            crate::widgets::radio_value(
                                ui,
                                &mut self.format,
                                MapFormat::Tmx,
                                "TMX (XML)",
                            );
                            crate::widgets::radio_value(
                                ui,
                                &mut self.format,
                                MapFormat::Tmj,
                                "TMJ (JSON)",
                            );
                        });
                        ui.end_row();
                        ui.label("Tilesets");
                        ui.horizontal(|ui| {
                            let external = format!("External .{}", self.format.tileset_extension());
                            crate::widgets::radio_value(
                                ui,
                                &mut self.external_tilesets,
                                true,
                                &external,
                            );
                            crate::widgets::radio_value(
                                ui,
                                &mut self.external_tilesets,
                                false,
                                "Embedded",
                            );
                        });
                        ui.end_row();
                        ui.label("Layer data");
                        ui.horizontal(|ui| {
                            for encoding in Encoding::ALL {
                                crate::widgets::radio_value(
                                    ui,
                                    &mut self.encoding,
                                    encoding,
                                    encoding_label(encoding),
                                );
                            }
                        });
                        ui.end_row();
                        ui.label("Frames");
                        ui.horizontal(|ui| {
                            let current = format!("Current ({})", editor.frame + 1);
                            crate::widgets::radio_value(ui, &mut self.all_frames, false, &current);
                            crate::widgets::radio_value(
                                ui,
                                &mut self.all_frames,
                                true,
                                "All (one map each)",
                            );
                        });
                        ui.end_row();
                        ui.label("Name");
                        crate::widgets::text_line_named(ui, &mut self.name, "Name");
                        ui.end_row();
                        if !web {
                            ui.label("Folder");
                            ui.horizontal(|ui| {
                                crate::widgets::text_line_named(ui, &mut self.folder, "Folder");
                                if crate::pixel_ui::text_button(ui, "Browse…").clicked()
                                    && let Some(dir) = platform.choose_folder(&self.folder)
                                {
                                    self.folder = dir.display().to_string();
                                }
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(4.0);
                ui.weak(summary(editor));
                ui.weak("Image layers get their own PNGs; slices become objects.");
                if web {
                    ui.weak("Downloads a .zip with the map, tilesets and images.");
                } else if self.folder.trim().is_empty() {
                    ui.weak("No folder: Export asks where to save the map.");
                }
                ui.add_space(6.0);
                match crate::pixel_ui::dialog_buttons(ui, skin, &["Export", "Cancel"]) {
                    Some(0) => result = self.run_export(editor, platform),
                    Some(_) => result = TiledResult::Cancel,
                    None => {}
                }
            },
        );
        if closed || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return TiledResult::Cancel;
        }
        result
    }
}

/// "16×12 tiles of 16×16 · 2 tilesets · 3 tilemap layers" (o por qué no se puede).
pub fn summary(editor: &Editor) -> String {
    let sprite = editor.sprite();
    let tilemaps: Vec<&cs_core::Layer> = sprite
        .layers
        .iter()
        .filter(|l| l.kind == cs_core::LayerKind::Tilemap && !l.reference)
        .collect();
    let Some(ts) = tilemaps
        .iter()
        .find_map(|l| sprite.tilesets.get(l.tileset? as usize))
    else {
        return NEEDS_TILEMAP.into();
    };
    let (tw, th) = ts.tile_size;
    let mut used: Vec<u32> = tilemaps.iter().filter_map(|l| l.tileset).collect();
    used.sort_unstable();
    used.dedup();
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    format!(
        "{}×{} tiles of {tw}×{th} · {} · {}",
        sprite.width.div_ceil(tw.max(1)),
        sprite.height.div_ceil(th.max(1)),
        plural(used.len(), "tileset"),
        plural(tilemaps.len(), "tilemap layer"),
    )
}

/// Por qué no hay nada que exportar (también el tooltip del menú).
pub const NEEDS_TILEMAP: &str = "Needs a tilemap layer (Layer → Convert to Tilemap)";

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cs_core::{Cel, ColorMode, Image, LayerKind, Sprite, Tile, TileGrid, Tileset};

    use super::*;

    /// 16×8 con una capa tilemap de tiles de 8×8 (un tile).
    fn editor() -> Editor {
        let mut s = Sprite::new(16, 8, ColorMode::Rgba);
        let mut ts = Tileset::new("ground", (8, 8), ColorMode::Rgba);
        ts.tiles.push(Arc::new(Image::new(8, 8, ColorMode::Rgba)));
        s.tilesets.push(ts);
        s.layers[0].kind = LayerKind::Tilemap;
        s.layers[0].tileset = Some(0);
        let mut grid = TileGrid::new(2, 1);
        grid.set(1, 0, Tile(1));
        let id = s.next_image_id();
        s.tilemaps.insert(id, Arc::new(grid));
        s.cels.insert(
            (s.layers[0].id, 0),
            Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            },
        );
        Editor::with_sprite(s)
    }

    #[test]
    fn summary_counts_tiles_and_layers() {
        assert_eq!(
            summary(&editor()),
            "2×1 tiles of 8×8 · 1 tileset · 1 tilemap layer"
        );
        let plain = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        assert_eq!(summary(&plain), NEEDS_TILEMAP);
    }

    #[test]
    fn reopen_keeps_the_options_and_picks_name_and_folder() {
        let editor = editor();
        let last = TiledDialog {
            format: MapFormat::Tmj,
            all_frames: true,
            ..Default::default()
        };
        let d = TiledDialog::reopen(&last, &editor, Some(PathBuf::from("maps")));
        assert_eq!(d.format, MapFormat::Tmj);
        assert!(d.all_frames);
        assert_eq!(d.name, editor.artboard().name);
        assert_eq!(d.folder, "maps");
        let used = TiledDialog {
            folder: "elsewhere".into(),
            ..last
        };
        assert_eq!(
            TiledDialog::reopen(&used, &editor, Some(PathBuf::from("maps"))).folder,
            "elsewhere"
        );
        assert_eq!(d.options(&editor).frames, TiledFrames::All);
        let current = TiledDialog::default();
        assert_eq!(current.options(&editor).frames, TiledFrames::Current(0));
    }

    #[test]
    fn export_writes_the_map_tilesets_and_images_to_the_folder() {
        let dir = std::env::temp_dir().join(format!("chimpsprite-tiled-{}", std::process::id()));
        let mut editor = editor();
        let d = TiledDialog::default();
        let done = d
            .export_to(
                &mut editor,
                Some(dir.join("maps")),
                "level",
                &Platform::default(),
            )
            .unwrap();
        assert_eq!(done.files.len(), 3, "{:?}", done.files);
        for name in ["level.tmx", "ground.tsx", "ground.png"] {
            assert!(dir.join("maps").join(name).exists(), "{name}");
        }
        let tmx = std::fs::read_to_string(dir.join("maps/level.tmx")).unwrap();
        let map = tiled::read_tmx(&tmx).unwrap();
        assert_eq!((map.width, map.height), (2, 1));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn building_without_tilemaps_fails_with_the_reason() {
        let editor = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let err = TiledDialog::default().build(&editor, "x").unwrap_err();
        assert!(err.contains("no tilemap layers"), "{err}");
    }
}
