//! Diálogo "Export": formato, alcance, escala y opciones de spritesheet.

use std::path::{Path, PathBuf};

use cs_io::export::{self, JsonFormat, SheetLayout};
use eframe::egui;

use crate::editor::Editor;
use crate::files::{FileError, write_atomic};
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
}

impl ExportFormat {
    const ALL: [ExportFormat; 5] = [
        ExportFormat::Png,
        ExportFormat::PngSequence,
        ExportFormat::Gif,
        ExportFormat::Sheet,
        ExportFormat::Atlas,
    ];

    fn label(self) -> &'static str {
        match self {
            ExportFormat::Png => "PNG (current frame)",
            ExportFormat::PngSequence => "PNG sequence",
            ExportFormat::Gif => "Animated GIF",
            ExportFormat::Sheet => "Spritesheet + JSON",
            ExportFormat::Atlas => "Atlas of all artboards + JSON",
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
        }
    }
}

pub enum ExportResult {
    Open,
    Cancel,
    /// Archivos escritos.
    Done(Vec<PathBuf>),
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

    /// Escribe los archivos; `path` es el elegido por el usuario (PNG o GIF principal).
    pub fn export(&self, editor: &Editor, path: &Path) -> Result<Vec<PathBuf>, FileError> {
        let sprite = editor.sprite();
        let name = editor.artboard().name.clone();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("sheet.png")
            .to_string();
        let json_path = path.with_extension("json");
        let mut written = Vec::new();
        match self.format {
            ExportFormat::Png => {
                write_atomic(path, &export::export_png(sprite, editor.frame, self.scale)?)?;
                written.push(path.to_path_buf());
            }
            ExportFormat::PngSequence => {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("frame");
                for (i, png) in export::export_png_sequence(sprite, self.scale)?
                    .into_iter()
                    .enumerate()
                {
                    let p = path.with_file_name(format!("{stem}_{:03}.png", i + 1));
                    write_atomic(&p, &png)?;
                    written.push(p);
                }
            }
            ExportFormat::Gif => {
                let (from, to) = self.frame_range(editor);
                write_atomic(path, &export::export_gif(sprite, from, to, self.scale)?)?;
                written.push(path.to_path_buf());
            }
            ExportFormat::Sheet => {
                let layout = match self.layout {
                    SheetLayout::Grid { .. } => SheetLayout::Grid {
                        columns: self.columns,
                    },
                    other => other,
                };
                let sheet =
                    export::export_sheet(sprite, &name, &file_name, layout, self.scale, self.json)?;
                write_atomic(path, &sheet.png)?;
                write_atomic(&json_path, sheet.json.as_bytes())?;
                written.extend([path.to_path_buf(), json_path]);
            }
            ExportFormat::Atlas => {
                let atlas =
                    export::export_atlas(&editor.document, &file_name, self.scale, self.json)?;
                write_atomic(path, &atlas.png)?;
                write_atomic(&json_path, atlas.json.as_bytes())?;
                written.extend([path.to_path_buf(), json_path]);
            }
        }
        Ok(written)
    }

    pub fn show(&mut self, ctx: &egui::Context, editor: &Editor) -> ExportResult {
        let mut result = ExportResult::Open;
        let mut open = true;
        egui::Window::new("Export")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                for format in ExportFormat::ALL {
                    ui.radio_value(&mut self.format, format, format.label());
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
                            ui.radio_value(&mut self.gif_range, GifRange::All, "All");
                            ui.radio_value(
                                &mut self.gif_range,
                                GifRange::CurrentTag,
                                "Current tag",
                            );
                            ui.radio_value(&mut self.gif_range, GifRange::Selection, "Selection");
                        });
                    }
                    ExportFormat::Sheet => {
                        ui.horizontal(|ui| {
                            ui.label("Layout");
                            ui.radio_value(&mut self.layout, SheetLayout::Horizontal, "Horizontal");
                            ui.radio_value(&mut self.layout, SheetLayout::Vertical, "Vertical");
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
                        json_options(ui, &mut self.json);
                    }
                    ExportFormat::Atlas => json_options(ui, &mut self.json),
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
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Export…").clicked() {
                        result = self.pick_and_export(editor);
                    }
                    if ui.button("Cancel").clicked() {
                        result = ExportResult::Cancel;
                    }
                });
            });
        if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            return ExportResult::Cancel;
        }
        result
    }

    fn pick_and_export(&self, editor: &Editor) -> ExportResult {
        let ext = self.format.extension();
        let default_name = match self.format {
            ExportFormat::Atlas => format!("atlas.{ext}"),
            _ => format!("{}.{ext}", editor.artboard().name),
        };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(default_name)
            .save_file()
        else {
            return ExportResult::Open;
        };
        match self.export(editor, &path) {
            Ok(files) => ExportResult::Done(files),
            Err(e) => ExportResult::Failed(e.to_string()),
        }
    }
}

fn json_options(ui: &mut egui::Ui, json: &mut JsonFormat) {
    ui.horizontal(|ui| {
        ui.label("JSON");
        ui.radio_value(json, JsonFormat::Hash, "Hash");
        ui.radio_value(json, JsonFormat::Array, "Array");
    });
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};

    use super::*;

    #[test]
    fn exports_write_expected_files() {
        let dir = std::env::temp_dir().join(format!("chimpsprite-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        editor.insert_empty_frame();
        let mut d = ExportDialog::default();
        let files = d.export(&editor, &dir.join("hero.png")).unwrap();
        assert_eq!(files, vec![dir.join("hero.png"), dir.join("hero.json")]);
        d.format = ExportFormat::PngSequence;
        let files = d.export(&editor, &dir.join("walk.png")).unwrap();
        assert_eq!(
            files,
            vec![dir.join("walk_001.png"), dir.join("walk_002.png")]
        );
        d.format = ExportFormat::Gif;
        assert!(d.export(&editor, &dir.join("anim.gif")).is_ok());
        assert!(dir.join("anim.gif").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
