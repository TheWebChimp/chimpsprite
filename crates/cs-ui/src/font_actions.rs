//! Lo de la app para exportar e importar fuentes pixel: el diálogo Build Font y dónde se
//! guardan sus archivos, y File → Import Font… (con su diálogo para los TTF/OTF).

use eframe::egui;

use crate::ChimpApp;
use crate::font_io::{self, BuildFontDialog, BuildFormat, OpenedFont};
use crate::font_sheet_dialog::GlyphSheetDialog;
use crate::platform::{FileFilter, OpenPurpose};
use crate::tilemap_dialog::DialogResult;

const TTF_FILTER: &[FileFilter] = &[FileFilter {
    name: "TrueType font",
    extensions: &["ttf"],
}];
const FNT_FILTER: &[FileFilter] = &[FileFilter {
    name: "BMFont",
    extensions: &["fnt"],
}];
const SHEET_FILTER: &[FileFilter] = &[FileFilter {
    name: "PNG image",
    extensions: &["png"],
}];

/// Lo que dice el aviso de Auto-build accents.
pub(crate) fn accent_summary(report: &cs_commands::font::AccentReport) -> String {
    let mut lines = vec![match report.built.len() {
        0 => "No accented letters were built.".to_owned(),
        1 => "1 accented letter built.".to_owned(),
        n => format!("{n} accented letters built."),
    }];
    if !report.cramped.is_empty() {
        let chars: String = report.cramped.iter().collect();
        lines.push(format!("No room for the 1 px gap: {chars}"));
    }
    if report.kept > 0 {
        lines.push(format!("{} already drawn (left as they are).", report.kept));
    }
    if !report.missing.is_empty() {
        lines.push(format!(
            "{} need a base letter or a mark (´ ` ^ ~ ¨ ˚ ¸ …) drawn first.",
            report.missing.len()
        ));
    }
    lines.join("\n")
}

impl ChimpApp {
    /// Build ▾ → `format` (o File → Export Font…): el diálogo Build Font de la fuente activa.
    pub(crate) fn open_build_font(&mut self, format: Option<BuildFormat>) {
        let editor = &self.doc.editor;
        let Some(mode) = editor.font().map(|f| f.mode) else {
            return;
        };
        let format = format.unwrap_or(BuildFormat::default_for(mode));
        self.build_font = BuildFontDialog::new(editor, format);
    }

    pub(crate) fn update_build_font_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.build_font else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            DialogResult::Open => {}
            DialogResult::Cancel => self.build_font = None,
            DialogResult::Accept => {
                if let Some(dialog) = self.build_font.take() {
                    self.build_font_files(&dialog);
                }
            }
        }
    }

    /// Pregunta dónde guardar y escribe los archivos de `dialog` (el BMFont: el `.fnt` donde se
    /// eligió y su `.png` al lado, con el mismo nombre). Con la familia, el primero va donde se
    /// eligió y los demás a su lado, cada uno con su nombre `Familia-Estilo`.
    fn build_font_files(&mut self, dialog: &BuildFontDialog) {
        let targets = dialog.targets();
        let family = targets.len() > 1;
        let document = &self.doc.editor.document;
        let files = if family {
            font_io::build_family_files(document, &targets, dialog.format, &dialog.family)
        } else {
            Ok(Vec::new())
        };
        let mut files = match files {
            Ok(files) => files,
            Err(e) => return self.show_message("Could not build the font", e),
        };
        let suggested = match (family, files.first()) {
            (true, Some((name, _))) => name.clone(),
            _ => match dialog.format {
                BuildFormat::BmFont => format!("{}.fnt", dialog.stem()),
                _ => format!("{}.ttf", dialog.stem()),
            },
        };
        let filters = match dialog.format {
            BuildFormat::BmFont => FNT_FILTER,
            _ => TTF_FILTER,
        };
        let Some(target) = self.platform.choose_save(&suggested, filters) else {
            return;
        };
        if !family {
            let Some(artboard) = self.doc.editor.document.artboard(dialog.artboard) else {
                return;
            };
            let stem = target.stem().to_owned();
            files = match font_io::build_files(artboard, dialog.format, &dialog.family, &stem) {
                Ok(files) => files,
                Err(e) => return self.show_message("Could not build the font", e),
            };
        }
        for (i, (name, bytes)) in files.iter().enumerate() {
            // El primero va donde se eligió; los demás (el atlas), a su lado.
            let file = if i == 0 {
                target.clone()
            } else {
                target.sibling(name)
            };
            if let Err(e) = self.platform.write(&file, bytes) {
                return self.show_message("Could not build the font", e);
            }
        }
    }

    /// File → Import Font… y Library → Fonts → Import…: elegir un TTF, OTF o JSON.
    pub(crate) fn choose_font_file(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(font_io::IMPORT_FILTERS, OpenPurpose::ImportFont, ctx);
    }

    /// Un archivo de fuente leído: el diálogo de un TTF/OTF o, si es de BitFontMaker2, la fuente.
    pub(crate) fn opened_font_file(&mut self, ctx: &egui::Context, name: &str, bytes: Vec<u8>) {
        match font_io::open_font_file(name, bytes) {
            Ok(OpenedFont::Dialog(dialog)) => self.import_font = Some(dialog),
            Ok(OpenedFont::Ready(build)) => self.add_imported_font(ctx, build),
            Err(e) => self.show_message("Could not import the font", e),
        }
    }

    pub(crate) fn update_import_font_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.import_font else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            DialogResult::Open => {}
            DialogResult::Cancel => self.import_font = None,
            DialogResult::Accept if dialog.fill.is_some() => {
                let fill = dialog.fill.expect("filling");
                let result = dialog.fill_glyphs();
                self.import_font = None;
                match result {
                    Ok(glyphs) => self.fill_font(fill.artboard, &glyphs, fill.replace),
                    Err(e) => self.show_message("Could not read the font", e),
                }
            }
            DialogResult::Accept => {
                let result = dialog.import();
                self.import_font = None;
                match result {
                    Ok(build) => self.add_imported_font(ctx, build),
                    Err(e) => self.show_message("Could not import the font", e),
                }
            }
        }
    }

    /// Generate ▾ → Auto-build accents: arma los acentuados que se puedan y dice qué pasó.
    pub(crate) fn auto_build_accents(&mut self) {
        if !self.doc.editor.is_font() {
            return;
        }
        let report = self.doc.editor.auto_build_accents(None, false);
        self.show_message("Auto-build Accents", accent_summary(&report));
    }

    /// Generate ▾ → Fill from TTF…: elegir el TTF/OTF.
    pub(crate) fn choose_fill_ttf(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(font_io::FILL_FILTERS, OpenPurpose::FillFont, ctx);
    }

    /// El TTF/OTF leído para rellenar la fuente activa: su diálogo.
    pub(crate) fn opened_fill_font(&mut self, bytes: Vec<u8>) {
        match font_io::ImportFontDialog::new_fill(bytes, &self.doc.editor) {
            Ok(dialog) => self.import_font = Some(Box::new(dialog)),
            Err(e) => self.show_message("Could not read the font", e),
        }
    }

    /// Generate ▾ → Glyphs from image…: elegir el PNG.
    pub(crate) fn choose_glyph_sheet(&mut self, ctx: &egui::Context) {
        self.platform
            .open_file(SHEET_FILTER, OpenPurpose::FontSheet, ctx);
    }

    /// El PNG leído: el diálogo Glyphs from Image para la fuente activa.
    pub(crate) fn opened_glyph_sheet(&mut self, bytes: &[u8]) {
        match cs_io::import::decode_png(bytes) {
            Ok(sheet) => {
                self.glyph_sheet = GlyphSheetDialog::new(&self.doc.editor, sheet).map(Box::new);
            }
            Err(e) => self.show_message("Could not read the image", e.to_string()),
        }
    }

    pub(crate) fn update_glyph_sheet_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.glyph_sheet else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            DialogResult::Open => {}
            DialogResult::Cancel => self.glyph_sheet = None,
            DialogResult::Accept => {
                let (artboard, replace) = (dialog.artboard, dialog.replace);
                let glyphs = dialog.glyphs();
                self.glyph_sheet = None;
                self.fill_font(artboard, &glyphs, replace);
            }
        }
    }

    /// Pone `glyphs` en la fuente `artboard` (si sigue activa) y dice cuántos.
    fn fill_font(
        &mut self,
        artboard: cs_core::ArtboardId,
        glyphs: &[cs_core::GlyphBuild],
        replace: bool,
    ) {
        let editor = &mut self.doc.editor;
        if editor.active != artboard || !editor.is_font() {
            return self.show_message("Generate Glyphs", "The font is not active anymore.");
        }
        let count = editor.fill_glyphs(glyphs, replace);
        let body = match count {
            0 => "No glyphs to fill: they are all drawn already.".to_owned(),
            1 => "1 glyph filled.".to_owned(),
            n => format!("{n} glyphs filled."),
        };
        self.show_message("Generate Glyphs", body);
    }

    /// Agrega la fuente importada al documento abierto (desde Home, a uno nuevo), abre su
    /// pestaña y el panel FontForge, como New Font….
    pub(crate) fn add_imported_font(&mut self, ctx: &egui::Context, build: cs_core::FontBuild) {
        if !self.has_doc() {
            self.new_document();
        }
        let (font, sprite) = cs_core::build_font(build);
        if !self.doc.editor.add_font(font, sprite) {
            return;
        }
        if let Some(id) = self.doc.editor.workspace() {
            self.tabs.open_view(&mut self.doc, id);
        }
        self.show_font_beside_panel(ctx);
    }
}
