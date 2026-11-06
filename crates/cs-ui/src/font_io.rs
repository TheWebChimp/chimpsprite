//! Exportar (Build) e importar fuentes pixel: los formatos del menú *Build ▾*, el diálogo Build
//! Font y el diálogo Import Font (TTF/OTF, con vista previa). El JSON de BitFontMaker2 se
//! importa sin diálogo.

use std::sync::Arc;

use cs_core::{Artboard, ArtboardId, FontBuild, FontMode};
use cs_io::font::ttf_import::{self, CharRange, TtfImport, TtfInfo};
use cs_io::font::{bitfontmaker, bmfont, ttf};
use eframe::egui::{self, Color32, Rect, Sense, Vec2, vec2};

use crate::editor::Editor;
use crate::platform::FileFilter;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// Lo que se puede importar desde File → Import Font… y Library → Fonts.
pub(crate) const IMPORT_FILTERS: &[FileFilter] = &[
    FileFilter {
        name: "Fonts",
        extensions: &["ttf", "otf", "json"],
    },
    FileFilter {
        name: "TrueType / OpenType",
        extensions: &["ttf", "otf"],
    },
    FileFilter {
        name: "BitFontMaker2 JSON",
        extensions: &["json"],
    },
];

/// Si `name` es un archivo de fuente que se importa (por su extensión).
pub(crate) fn is_font_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".ttf") || lower.ends_with(".otf")
}

/// A qué se exporta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildFormat {
    /// TrueType de contornos (la silueta, en 1 bit o en color).
    Ttf,
    /// TrueType con COLR + CPAL (solo fuentes de color).
    ColorTtf,
    /// BMFont: `.fnt` de texto + atlas `.png`.
    BmFont,
}

impl BuildFormat {
    pub const ALL: [BuildFormat; 3] =
        [BuildFormat::Ttf, BuildFormat::ColorTtf, BuildFormat::BmFont];

    pub fn label(self) -> &'static str {
        match self {
            BuildFormat::Ttf => "TTF",
            BuildFormat::ColorTtf => "Color TTF (COLR)",
            BuildFormat::BmFont => "BMFont (.fnt + .png)",
        }
    }

    /// El de siempre para una fuente de `mode`: la de color, con sus colores.
    pub fn default_for(mode: FontMode) -> Self {
        match mode {
            FontMode::OneBit => BuildFormat::Ttf,
            FontMode::Color => BuildFormat::ColorTtf,
        }
    }

    /// El TTF de color solo tiene sentido en una fuente de color.
    pub fn available(self, mode: FontMode) -> bool {
        self != BuildFormat::ColorTtf || mode == FontMode::Color
    }
}

/// Un nombre de archivo desde el nombre de la fuente: sin lo que no puede ir en un archivo.
pub(crate) fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    match stem.trim_matches('.') {
        "" => "font".to_owned(),
        s => s.to_owned(),
    }
}

/// Los archivos de la fuente `artboard` en `format` con nombre `stem`: el TTF, o el `.fnt` y su
/// `.png` (al que apunta el `.fnt`).
pub(crate) fn build_files(
    artboard: &Artboard,
    format: BuildFormat,
    family: &str,
    stem: &str,
) -> Result<Vec<(String, Vec<u8>)>, String> {
    match format {
        BuildFormat::Ttf | BuildFormat::ColorTtf => {
            let options = ttf::TtfOptions {
                family: family.to_owned(),
                color: format == BuildFormat::ColorTtf,
            };
            let bytes = ttf::export_ttf(artboard, &options).map_err(|e| e.to_string())?;
            Ok(vec![(format!("{stem}.ttf"), bytes)])
        }
        BuildFormat::BmFont => {
            let png = format!("{stem}.png");
            let mut font = artboard.clone();
            if let Some(info) = font.font.as_mut()
                && !family.trim().is_empty()
            {
                info.name = family.trim().to_owned();
            }
            let bm = bmfont::export_bmfont(&font, &png).map_err(|e| e.to_string())?;
            Ok(vec![
                (format!("{stem}.fnt"), bm.fnt.into_bytes()),
                (png, bm.png),
            ])
        }
    }
}

/// Build Font: el formato, el nombre de la familia y lo que se va a escribir.
pub struct BuildFontDialog {
    pub artboard: ArtboardId,
    pub format: BuildFormat,
    pub family: String,
    mode: FontMode,
}

impl BuildFontDialog {
    /// Para la fuente activa, con `format` (si no se puede en esta fuente, el de siempre).
    pub fn new(editor: &Editor, format: BuildFormat) -> Option<Self> {
        let font = editor.font()?;
        let format = if format.available(font.mode) {
            format
        } else {
            BuildFormat::default_for(font.mode)
        };
        Some(BuildFontDialog {
            artboard: editor.active,
            format,
            family: font.name.clone(),
            mode: font.mode,
        })
    }

    /// El nombre de los archivos (sin extensión).
    pub fn stem(&self) -> String {
        file_stem(&self.family)
    }

    /// Lo que se va a escribir, para leerlo en el diálogo.
    pub fn files_label(&self) -> String {
        let stem = self.stem();
        match self.format {
            BuildFormat::BmFont => format!("{stem}.fnt + {stem}.png"),
            _ => format!("{stem}.ttf"),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let buttons = ["Build", "Cancel"];
        let (result, open) =
            dialog_window_with(ctx, skin, "build_font", "Build Font", buttons, |ui| {
                egui::Grid::new("build_font_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Format:");
                        ui.vertical(|ui| {
                            for format in BuildFormat::ALL {
                                let enabled = format.available(self.mode);
                                ui.add_enabled_ui(enabled, |ui| {
                                    widgets::radio_value(
                                        ui,
                                        &mut self.format,
                                        format,
                                        format.label(),
                                    )
                                })
                                .response
                                .on_disabled_hover_text("Only for color fonts");
                            }
                        });
                        ui.end_row();
                        ui.label("Family:");
                        widgets::text_line_named(ui, &mut self.family, "Family name");
                        ui.end_row();
                        ui.label("Writes:");
                        ui.label(self.files_label());
                        ui.end_row();
                    });
                ui.add_space(2.0);
                ui.weak(match self.format {
                    BuildFormat::Ttf => "Pixel outlines: crisp at multiples of the cell height.",
                    BuildFormat::ColorTtf => "COLR layers, with the silhouette as a fallback.",
                    BuildFormat::BmFont => "Text .fnt and a .png atlas for game engines.",
                });
            });
        finish(ctx, open, result)
    }
}

/// El texto de la vista previa de Import Font.
const PREVIEW_TEXT: &str = "AaBbGg 0123";
/// La escala de la vista previa, el ancho de su papel y su aire.
const PREVIEW_SCALE: u32 = 2;
const PREVIEW_W: f32 = 260.0;
const PREVIEW_PAD: f32 = 4.0;
const PREVIEW_INK: Color32 = Color32::from_rgb(5, 4, 3);

/// Import Font: cómo rasterizar un TTF/OTF (tamaño, umbral, caracteres y modo), con vista
/// previa.
pub struct ImportFontDialog {
    bytes: Arc<[u8]>,
    pub info: TtfInfo,
    pub px: u32,
    pub threshold: u32,
    pub chars: CharRange,
    pub color: bool,
    /// La vista previa y las opciones con que se hizo.
    preview: Option<(TtfImport, Option<Artboard>)>,
}

impl ImportFontDialog {
    pub fn new(bytes: Vec<u8>) -> Result<Self, String> {
        let info = ttf_import::ttf_info(&bytes).map_err(|e| e.to_string())?;
        Ok(ImportFontDialog {
            bytes: bytes.into(),
            px: info.suggested_px,
            threshold: 50,
            chars: CharRange::All,
            color: info.color,
            info,
            preview: None,
        })
    }

    pub fn options(&self) -> TtfImport {
        TtfImport {
            name: String::new(),
            px: self.px,
            threshold: self.threshold,
            chars: self.chars,
            color: self.color && self.info.color,
        }
    }

    /// La fuente con las opciones elegidas.
    pub fn import(&self) -> Result<FontBuild, String> {
        ttf_import::import_ttf(&self.bytes, &self.options()).map_err(|e| e.to_string())
    }

    /// La vista previa con las opciones de ahora (se rehace solo si cambiaron).
    fn preview(&mut self) -> Option<&Artboard> {
        let options = self.options();
        if self.preview.as_ref().is_none_or(|(o, _)| *o != options) {
            let chars: Vec<char> = PREVIEW_TEXT.chars().collect();
            let artboard = ttf_import::import_chars(&self.bytes, &chars, &options)
                .ok()
                .map(preview_artboard);
            self.preview = Some((options, artboard));
        }
        self.preview.as_ref().and_then(|(_, a)| a.as_ref())
    }

    pub fn show(&mut self, ctx: &egui::Context, skin: Option<&Skin>) -> DialogResult {
        let buttons = ["Import", "Cancel"];
        let (result, open) =
            dialog_window_with(ctx, skin, "import_font", "Import Font", buttons, |ui| {
                let kind = if self.info.pixel_grid {
                    format!("pixel font, {} px grid", self.info.suggested_px)
                } else {
                    "outline font".to_owned()
                };
                ui.label(format!("{} ({kind})", self.info.family));
                let note = if self.info.chars > ttf_import::MAX_GLYPHS {
                    format!(
                        "{} characters (All takes the first {})",
                        self.info.chars,
                        ttf_import::MAX_GLYPHS
                    )
                } else {
                    format!("{} characters", self.info.chars)
                };
                ui.weak(note);
                ui.add_space(4.0);
                egui::Grid::new("import_font_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Size:");
                        widgets::number(ui, &mut self.px, ttf_import::PX_RANGE, " px");
                        ui.end_row();
                        ui.label("Threshold:");
                        widgets::number(ui, &mut self.threshold, 1..=100, "%");
                        ui.end_row();
                        ui.label("Characters:");
                        let chars = &mut self.chars;
                        widgets::combo(ui, "import_font_chars", chars.label(), |ui| {
                            for range in CharRange::ALL {
                                widgets::choice(ui, chars, range, range.label());
                            }
                        });
                        ui.end_row();
                        ui.label("Mode:");
                        let has_color = self.info.color;
                        let color = &mut self.color;
                        ui.horizontal(|ui| {
                            widgets::radio_value(ui, color, false, "1 bit");
                            ui.add_enabled_ui(has_color, |ui| {
                                widgets::radio_value(ui, color, true, "Color")
                            })
                            .response
                            .on_disabled_hover_text("The font has no color layers (COLR)");
                        });
                        ui.end_row();
                    });
                ui.add_space(6.0);
                if let Some(artboard) = self.preview() {
                    paint_preview(ui, artboard);
                }
            });
        finish(ctx, open, result)
    }
}

/// Un artboard de fuente (solo para verla) desde lo importado.
fn preview_artboard(build: FontBuild) -> Artboard {
    let (font, sprite) = cs_core::build_font(build);
    Artboard {
        model: None,
        id: ArtboardId(0),
        name: font.name.clone(),
        position: (0, 0),
        sprite,
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: false,
        font: Some(font),
    }
}

/// [`PREVIEW_TEXT`] escrito con la fuente, en tinta oscura sobre blanco (como el Test Drive).
fn paint_preview(ui: &mut egui::Ui, artboard: &Artboard) {
    let Some(view) = artboard.font_view() else {
        return;
    };
    let height = (view.cell.1 * PREVIEW_SCALE) as f32 + 2.0 * PREVIEW_PAD;
    let (rect, _) = ui.allocate_exact_size(vec2(PREVIEW_W, height), Sense::hover());
    let paper = Rect::from_min_size(rect.min.round(), rect.size());
    let painter = ui.painter().with_clip_rect(paper.intersect(ui.clip_rect()));
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    let origin = paper.min + Vec2::splat(PREVIEW_PAD);
    for (x, y, c) in cs_tools::text::font_text(&view, PREVIEW_TEXT, PREVIEW_SCALE) {
        let color = c.map_or(PREVIEW_INK, |c| {
            Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
        });
        painter.rect_filled(
            Rect::from_min_size(origin + vec2(x as f32, y as f32), Vec2::splat(1.0)),
            0.0,
            color,
        );
    }
}

/// Lo que sale de abrir un archivo de fuente: el diálogo de un TTF/OTF o la fuente ya lista (el
/// JSON de BitFontMaker2).
pub(crate) enum OpenedFont {
    Dialog(Box<ImportFontDialog>),
    Ready(FontBuild),
}

/// Lee el archivo de fuente `name` (por su extensión: `.json` es de BitFontMaker2; lo demás, un
/// TTF/OTF).
pub(crate) fn open_font_file(name: &str, bytes: Vec<u8>) -> Result<OpenedFont, String> {
    if name.to_ascii_lowercase().ends_with(".json") {
        let text = String::from_utf8_lossy(&bytes);
        let mut build = bitfontmaker::import_bitfontmaker(&text).map_err(|e| e.to_string())?;
        if build.name == "BitFontMaker2" {
            build.name = crate::files::stem(name).to_owned();
        }
        return Ok(OpenedFont::Ready(build));
    }
    ImportFontDialog::new(bytes).map(|d| OpenedFont::Dialog(Box::new(d)))
}

/// Un TTF de prueba: "Tiny", celda 6×8, solo una `A`.
#[cfg(test)]
pub(crate) fn sample_ttf() -> Vec<u8> {
    let artboard = preview_artboard(FontBuild {
        name: "Tiny".into(),
        mode: FontMode::OneBit,
        cell: (6, 8),
        metrics: cs_core::FontMetrics::for_cell(8),
        letter_spacing: 1,
        space_width: 2,
        monospace: false,
        glyphs: vec![cs_core::GlyphBuild {
            ch: 'A',
            advance: None,
            pixels: vec![(0, 2, None), (1, 2, None)],
        }],
    });
    let files = build_files(&artboard, BuildFormat::Ttf, "", "tiny").unwrap();
    files.into_iter().next().unwrap().1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmfont_files_are_the_fnt_and_its_png() {
        let artboard = preview_artboard(FontBuild {
            name: "Tiny".into(),
            mode: FontMode::OneBit,
            cell: (6, 8),
            metrics: cs_core::FontMetrics::for_cell(8),
            letter_spacing: 1,
            space_width: 2,
            monospace: false,
            glyphs: Vec::new(),
        });
        let files = build_files(&artboard, BuildFormat::BmFont, "Big Name", "out").unwrap();
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["out.fnt", "out.png"]);
        let fnt = String::from_utf8(files[0].1.clone()).unwrap();
        assert!(fnt.contains("face=\"Big Name\""));
        assert!(fnt.contains("file=\"out.png\""));
    }

    #[test]
    fn file_stems_drop_what_cannot_go_in_a_file_name() {
        assert_eq!(file_stem(" My/Font: 2 "), "My_Font_ 2");
        assert_eq!(file_stem("..."), "font");
        assert_eq!(file_stem(""), "font");
    }

    #[test]
    fn the_color_ttf_is_only_for_color_fonts() {
        assert!(!BuildFormat::ColorTtf.available(FontMode::OneBit));
        assert!(BuildFormat::ColorTtf.available(FontMode::Color));
        assert_eq!(
            BuildFormat::default_for(FontMode::Color),
            BuildFormat::ColorTtf
        );
        assert!(BuildFormat::BmFont.available(FontMode::OneBit));
    }

    #[test]
    fn a_ttf_opens_the_import_dialog_with_its_suggested_size() {
        let Ok(OpenedFont::Dialog(dialog)) = open_font_file("tiny.ttf", sample_ttf()) else {
            panic!("a dialog");
        };
        assert_eq!(dialog.px, 8);
        assert!(dialog.info.pixel_grid);
        let build = dialog.import().unwrap();
        assert_eq!(build.cell, (6, 8));
        assert_eq!(build.glyphs.len(), 1);
        assert!(open_font_file("broken.ttf", vec![1, 2, 3]).is_err());
    }

    #[test]
    fn a_bitfontmaker_json_is_ready_and_named_after_the_file() {
        let json = br#"{"65":[0,0,0,4,4,0,0,0,0,0,0,0,0,0,0,0]}"#.to_vec();
        let Ok(OpenedFont::Ready(build)) = open_font_file("Mine.JSON", json) else {
            panic!("ready");
        };
        assert_eq!(build.name, "Mine");
        assert_eq!(build.glyphs.len(), 1);
    }
}
