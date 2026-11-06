//! Lo de la app para exportar e importar fuentes pixel: el diálogo Build Font y dónde se
//! guardan sus archivos, y File → Import Font… (con su diálogo para los TTF/OTF).

use eframe::egui;

use crate::ChimpApp;
use crate::font_io::{self, BuildFontDialog, BuildFormat, OpenedFont};
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
    /// eligió y su `.png` al lado, con el mismo nombre).
    fn build_font_files(&mut self, dialog: &BuildFontDialog) {
        let Some(artboard) = self.doc.editor.document.artboard(dialog.artboard) else {
            return;
        };
        let (suggested, filters) = match dialog.format {
            BuildFormat::BmFont => (format!("{}.fnt", dialog.stem()), FNT_FILTER),
            _ => (format!("{}.ttf", dialog.stem()), TTF_FILTER),
        };
        let Some(target) = self.platform.choose_save(&suggested, filters) else {
            return;
        };
        let stem = target.stem().to_owned();
        let files = match font_io::build_files(artboard, dialog.format, &dialog.family, &stem) {
            Ok(files) => files,
            Err(e) => return self.show_message("Could not build the font", e),
        };
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
