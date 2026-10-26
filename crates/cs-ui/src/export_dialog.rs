//! Diálogo "Export": formato, alcance, escala y opciones de spritesheet.

use cs_io::engines;
use cs_io::export::{self, JsonFormat, SheetLayout};
use eframe::egui;

use crate::editor::Editor;
use crate::files::{self, FileError};
use crate::pixel_ui::Tip as _;
use crate::platform::{FileFilter, Platform, SaveTarget};
use crate::widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    /// Frame actual del artboard activo.
    Png,
    PngSequence,
    Gif,
    Sheet,
    /// Todos los artboards en una sola imagen.
    Atlas,
    /// Hoja + recurso `SpriteFrames` de Godot 4.
    Godot,
    /// Hoja + `.meta` de Unity con un sprite por frame.
    Unity,
    /// Hoja + atlas RON para Bevy.
    Bevy,
}

impl ExportFormat {
    const ALL: [ExportFormat; 8] = [
        ExportFormat::Png,
        ExportFormat::PngSequence,
        ExportFormat::Gif,
        ExportFormat::Sheet,
        ExportFormat::Atlas,
        ExportFormat::Godot,
        ExportFormat::Unity,
        ExportFormat::Bevy,
    ];

    /// Formatos que arman una hoja con layout configurable.
    fn uses_sheet(self) -> bool {
        matches!(
            self,
            ExportFormat::Sheet | ExportFormat::Godot | ExportFormat::Unity | ExportFormat::Bevy
        )
    }

    /// Formatos que pueden llevar normal map.
    fn supports_normals(self) -> bool {
        self.uses_sheet() || self == ExportFormat::Png
    }

    fn label(self) -> &'static str {
        match self {
            ExportFormat::Png => "PNG (current frame)",
            ExportFormat::PngSequence => "PNG sequence",
            ExportFormat::Gif => "Animated GIF",
            ExportFormat::Sheet => "Spritesheet + JSON",
            ExportFormat::Atlas => "Atlas of all artboards + JSON",
            ExportFormat::Godot => "Godot 4 (SpriteFrames .tres)",
            ExportFormat::Unity => "Unity (sliced sprites .meta)",
            ExportFormat::Bevy => "Bevy (atlas .ron)",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            ExportFormat::Gif => "gif",
            _ => "png",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GifRange {
    All,
    /// El tag que contiene al frame actual.
    CurrentTag,
    /// Los frames seleccionados en el timeline.
    Selection,
}

#[derive(Clone, Debug)]
pub struct ExportDialog {
    pub format: ExportFormat,
    pub scale: u32,
    pub gif_range: GifRange,
    pub layout: SheetLayout,
    pub columns: u32,
    pub json: JsonFormat,
    /// También exportar `<nombre>_n.png` con el normal map.
    pub normal_map: bool,
    /// Carpeta `res://` donde quedará la hoja en el proyecto de Godot.
    pub godot_folder: String,
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self {
            format: ExportFormat::Sheet,
            scale: 1,
            gif_range: GifRange::All,
            layout: SheetLayout::Horizontal,
            columns: 4,
            json: JsonFormat::Hash,
            normal_map: false,
            godot_folder: "res://".into(),
        }
    }
}

pub enum ExportResult {
    Open,
    Cancel,
    /// Archivos escritos (rutas en escritorio, nombres descargados en web).
    Done(Vec<String>),
    Failed(String),
}

impl ExportDialog {
    fn frame_range(&self, editor: &Editor) -> (usize, usize) {
        let last = editor.frame_count() - 1;
        match self.gif_range {
            GifRange::All => (0, last),
            GifRange::CurrentTag => editor
                .sprite()
                .tag_at(editor.frame)
                .map_or((0, last), |t| (t.from, t.to)),
            GifRange::Selection => editor.selected_frames(),
        }
    }

    /// Genera los archivos a escribir. `file_name` es el principal (PNG o GIF); los demás
    /// (JSON, secuencia) se nombran a partir de él.
    pub fn build(
        &self,
        editor: &Editor,
        file_name: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, FileError> {
        // Las instancias y los tilemaps se hornean en capas normales para exportar.
        let baked = cs_render::bake_layers(
            &editor.document,
            editor.sprite(),
            cs_render::Bake::Everything,
        );
        let sprite = baked.as_ref().unwrap_or(editor.sprite());
        let name = editor.artboard().name.clone();
        let stem = files::stem(file_name).to_string();
        let json_name = format!("{stem}.json");
        let layout = match self.layout {
            SheetLayout::Grid { .. } => SheetLayout::Grid {
                columns: self.columns,
            },
            other => other,
        };
        let mut files = self.build_main(editor, file_name, &name, &stem, &json_name, layout)?;
        if self.normal_map && self.format.supports_normals() {
            let png = if self.format == ExportFormat::Png {
                export::export_normal_png(sprite, editor.frame, self.scale)?
            } else {
                export::export_normal_sheet(sprite, layout, self.scale)?
            };
            files.push((format!("{stem}_n.png"), png));
        }
        Ok(files)
    }

    fn build_main(
        &self,
        editor: &Editor,
        file_name: &str,
        name: &str,
        stem: &str,
        json_name: &str,
        layout: SheetLayout,
    ) -> Result<Vec<(String, Vec<u8>)>, FileError> {
        let baked = cs_render::bake_layers(
            &editor.document,
            editor.sprite(),
            cs_render::Bake::Everything,
        );
        let sprite = baked.as_ref().unwrap_or(editor.sprite());
        let json_name = json_name.to_string();
        let sheet_png = || -> Result<Vec<u8>, FileError> {
            Ok(export::export_sheet(sprite, name, file_name, layout, self.scale, self.json)?.png)
        };
        Ok(match self.format {
            ExportFormat::Png => vec![(
                file_name.to_string(),
                export::export_png(sprite, editor.frame, self.scale)?,
            )],
            ExportFormat::PngSequence => export::export_png_sequence(sprite, self.scale)?
                .into_iter()
                .enumerate()
                .map(|(i, png)| (format!("{stem}_{:03}.png", i + 1), png))
                .collect(),
            ExportFormat::Gif => {
                let (from, to) = self.frame_range(editor);
                vec![(
                    file_name.to_string(),
                    export::export_gif(sprite, from, to, self.scale)?,
                )]
            }
            ExportFormat::Godot => {
                let folder = self.godot_folder.trim_end_matches('/');
                let path = format!("{folder}/{file_name}");
                let tres = engines::godot_sprite_frames(sprite, &path, layout, self.scale);
                vec![
                    (file_name.to_string(), sheet_png()?),
                    (format!("{stem}.tres"), tres.into_bytes()),
                ]
            }
            ExportFormat::Unity => {
                let meta = engines::unity_meta(sprite, file_name, layout, self.scale);
                vec![
                    (file_name.to_string(), sheet_png()?),
                    (format!("{file_name}.meta"), meta.into_bytes()),
                ]
            }
            ExportFormat::Bevy => {
                let ron = engines::bevy_atlas(sprite, file_name, layout, self.scale);
                vec![
                    (file_name.to_string(), sheet_png()?),
                    (format!("{stem}.ron"), ron.into_bytes()),
                ]
            }
            ExportFormat::Sheet => {
                let sheet =
                    export::export_sheet(sprite, name, file_name, layout, self.scale, self.json)?;
                vec![
                    (file_name.to_string(), sheet.png),
                    (json_name, sheet.json.into_bytes()),
                ]
            }
            ExportFormat::Atlas => {
                let atlas =
                    export::export_atlas(&editor.document, file_name, self.scale, self.json)?;
                vec![
                    (file_name.to_string(), atlas.png),
                    (json_name, atlas.json.into_bytes()),
                ]
            }
        })
    }

    /// Genera y escribe (o descarga) los archivos junto a `target`. Lo flotante se confirma antes:
    /// el diálogo no es modal y se pudo haber movido algo mientras estaba abierto.
    pub fn export(
        &self,
        editor: &mut Editor,
        target: &SaveTarget,
        platform: &Platform,
    ) -> Result<Vec<String>, FileError> {
        editor.commit_floating();
        let mut written = Vec::new();
        for (name, bytes) in self.build(editor, &target.file_name)? {
            let file = target.sibling(&name);
            platform.write(&file, &bytes).map_err(FileError::Platform)?;
            written.push(file.path().map_or(name, |p| p.display().to_string()));
        }
        Ok(written)
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        platform: &Platform,
        skin: Option<&crate::skin::Skin>,
    ) -> ExportResult {
        let mut result = ExportResult::Open;
        let (_, closed) = crate::pixel_ui::app_window(
            ctx,
            skin,
            "export_window",
            "Export",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                for format in ExportFormat::ALL {
                    crate::widgets::radio_value(ui, &mut self.format, format, format.label());
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Scale");
                    widgets::number(ui, &mut self.scale, 1..=16, "×");
                });
                match self.format {
                    ExportFormat::Gif => {
                        ui.horizontal(|ui| {
                            ui.label("Frames");
                            crate::widgets::radio_value(
                                ui,
                                &mut self.gif_range,
                                GifRange::All,
                                "All",
                            );
                            crate::widgets::radio_value(
                                ui,
                                &mut self.gif_range,
                                GifRange::CurrentTag,
                                "Current tag",
                            );
                            crate::widgets::radio_value(
                                ui,
                                &mut self.gif_range,
                                GifRange::Selection,
                                "Selection",
                            );
                        });
                    }
                    format if format.uses_sheet() => {
                        ui.horizontal(|ui| {
                            ui.label("Layout");
                            crate::widgets::radio_value(
                                ui,
                                &mut self.layout,
                                SheetLayout::Horizontal,
                                "Horizontal",
                            );
                            crate::widgets::radio_value(
                                ui,
                                &mut self.layout,
                                SheetLayout::Vertical,
                                "Vertical",
                            );
                            let grid = SheetLayout::Grid {
                                columns: self.columns,
                            };
                            if ui
                                .radio(matches!(self.layout, SheetLayout::Grid { .. }), "Grid")
                                .clicked()
                            {
                                self.layout = grid;
                            }
                            if matches!(self.layout, SheetLayout::Grid { .. }) {
                                widgets::number(ui, &mut self.columns, 1..=256, " cols");
                            }
                        });
                        if format == ExportFormat::Sheet {
                            json_options(ui, &mut self.json);
                        }
                        if format == ExportFormat::Godot {
                            ui.horizontal(|ui| {
                                ui.label("Godot folder");
                                crate::widgets::text_line(ui, &mut self.godot_folder);
                            });
                        }
                    }
                    ExportFormat::Atlas => json_options(ui, &mut self.json),
                    _ => {}
                }
                if self.format.supports_normals() {
                    crate::widgets::checkbox(ui, &mut self.normal_map, "Also export normal map (_n.png)")
                        .tip("Lighting normals from the sprite's shape, for 2D lights in Godot/Unity/Bevy");
                }
                match self.format {
                    ExportFormat::Godot => {
                        ui.weak(
                            "Animations come from tags; durations become per-frame multipliers.",
                        );
                    }
                    ExportFormat::Unity => {
                        ui.weak("One sprite per frame; pivots come from slices with a pivot.");
                    }
                    ExportFormat::Bevy => {
                        ui.weak("Grid layout for TextureAtlasLayout::from_grid, plus animations and slices.");
                    }
                    _ => {}
                }
                let sprite = editor.sprite();
                let (w, h) = (sprite.width * self.scale, sprite.height * self.scale);
                ui.weak(match self.format {
                    ExportFormat::Atlas => format!("{} artboards", editor.document.artboards.len()),
                    _ => format!(
                        "{} · {w}×{h} px · {} frames",
                        editor.artboard().name,
                        sprite.frames.len()
                    ),
                });
                ui.add_space(6.0);
                match crate::pixel_ui::dialog_buttons(ui, skin, &["Export", "Cancel"]) {
                    Some(0) => result = self.pick_and_export(editor, platform),
                    Some(_) => result = ExportResult::Cancel,
                    None => {}
                }
            },
        );
        if closed || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return ExportResult::Cancel;
        }
        result
    }

    fn pick_and_export(&self, editor: &mut Editor, platform: &Platform) -> ExportResult {
        let ext = self.format.extension();
        let default_name = match self.format {
            ExportFormat::Atlas => format!("atlas.{ext}"),
            _ => format!("{}.{ext}", editor.artboard().name),
        };
        let filter = [FileFilter {
            name: if ext == "gif" { "GIF" } else { "PNG" },
            extensions: if ext == "gif" { &["gif"] } else { &["png"] },
        }];
        let Some(target) = platform.choose_save(&default_name, &filter) else {
            return ExportResult::Open;
        };
        match self.export(editor, &target, platform) {
            Ok(files) => ExportResult::Done(files),
            Err(e) => ExportResult::Failed(e.to_string()),
        }
    }
}

fn json_options(ui: &mut egui::Ui, json: &mut JsonFormat) {
    ui.horizontal(|ui| {
        ui.label("JSON");
        crate::widgets::radio_value(ui, json, JsonFormat::Hash, "Hash");
        crate::widgets::radio_value(ui, json, JsonFormat::Array, "Array");
    });
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;

    #[test]
    fn build_names_files_from_the_main_name() {
        let mut editor = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        editor.insert_empty_frame();
        let mut d = ExportDialog::default();
        let names = |d: &ExportDialog, main: &str| -> Vec<String> {
            d.build(&editor, main)
                .unwrap()
                .into_iter()
                .map(|(n, _)| n)
                .collect()
        };
        assert_eq!(names(&d, "hero.png"), vec!["hero.png", "hero.json"]);
        d.format = ExportFormat::PngSequence;
        assert_eq!(names(&d, "walk.png"), vec!["walk_001.png", "walk_002.png"]);
        d.format = ExportFormat::Gif;
        assert_eq!(names(&d, "anim.gif"), vec!["anim.gif"]);
        d.format = ExportFormat::Godot;
        d.normal_map = true;
        assert_eq!(
            names(&d, "hero.png"),
            vec!["hero.png", "hero.tres", "hero_n.png"]
        );
        d.format = ExportFormat::Unity;
        assert_eq!(
            names(&d, "hero.png"),
            vec!["hero.png", "hero.png.meta", "hero_n.png"]
        );
        d.format = ExportFormat::Bevy;
        d.normal_map = false;
        assert_eq!(names(&d, "hero.png"), vec!["hero.png", "hero.ron"]);
        let files = d.build(&editor, "hero.png").unwrap();
        assert!(String::from_utf8_lossy(&files[1].1).contains("columns: 2"));
    }

    #[test]
    fn export_writes_next_to_target() {
        let dir = std::env::temp_dir().join(format!("chimpsprite-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let target = SaveTarget::from_path(&dir.join("hero.png")).unwrap();
        let files = ExportDialog::default()
            .export(&mut editor, &target, &Platform::default())
            .unwrap();
        assert_eq!(files.len(), 2);
        assert!(dir.join("hero.png").exists() && dir.join("hero.json").exists());
        std::fs::remove_dir_all(dir).ok();
    }
    #[test]
    fn export_applies_floating_content_first() {
        use cs_commands::Command;
        use cs_core::{LayerId, Mask, PixelValue, Rgba};
        let dir = std::env::temp_dir().join(format!(
            "chimpsprite-export-floating-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, red)],
        });
        editor.tool = crate::editor::ToolKind::RectSelect;
        editor.set_selection(Some(Mask::from_rect(4, 4, (0, 0), (0, 0))));
        editor.pointer_press((0.5, 0.5), false, false, 0.1);
        editor.pointer_drag((2.5, 2.5));
        editor.pointer_release();
        assert!(editor.is_floating());
        let target = SaveTarget::from_path(&dir.join("hero.png")).unwrap();
        ExportDialog::default()
            .export(&mut editor, &target, &Platform::default())
            .unwrap();
        assert!(!editor.is_floating(), "applied before exporting");
        assert_eq!(editor.sprite().pixel(LayerId(1), 0, 2, 2), red);
        std::fs::remove_dir_all(dir).ok();
    }
}
