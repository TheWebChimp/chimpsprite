//! Ventana Palette Presets, menú ⋯ del panel de paleta y sus diálogos.

use cs_io::palette_files::{self, NamedPalette, PaletteFormat};
use eframe::egui;

use crate::ChimpApp;
use crate::palette_library::{Preset, PresetSource};
use crate::panels::{self, PaletteAction};
use crate::platform::{self, FileFilter, OpenPurpose};

const LOAD_FILTERS: &[FileFilter] = &[FileFilter {
    name: "Palettes",
    extensions: &["gpl", "hex", "pal", "png", "ase", "aseprite"],
}];

const SAVE_FILTERS: &[FileFilter] = &[
    FileFilter {
        name: "GIMP Palette",
        extensions: &["gpl"],
    },
    FileFilter {
        name: "Hex",
        extensions: &["hex"],
    },
    FileFilter {
        name: "JASC Palette",
        extensions: &["pal"],
    },
];

/// Colores visibles por fila en la ventana.
const STRIP_COLORS: usize = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PresetsTab {
    #[default]
    Library,
}

#[derive(Default)]
pub struct PresetsWindow {
    pub tab: PresetsTab,
    search: String,
}

/// Pedir el nombre para "Save as Preset" (y para "Set as Default" si la paleta no es un preset).
pub struct NameDialog {
    name: String,
    then_default: bool,
}

/// Confirmaciones pendientes.
pub enum Confirm {
    Replace {
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    },
    Delete(String),
}

/// Formato al guardar, según la extensión elegida (`.gpl` si no se reconoce).
fn save_format(file_name: &str) -> PaletteFormat {
    match palette_files::format_for(file_name) {
        Some(f @ (PaletteFormat::Hex | PaletteFormat::Pal)) => f,
        _ => PaletteFormat::Gpl,
    }
}

fn matches_search(name: &str, search: &str) -> bool {
    name.to_lowercase().contains(&search.trim().to_lowercase())
}

impl ChimpApp {
    pub(crate) fn palette_action(&mut self, ctx: &egui::Context, action: PaletteAction) {
        match action {
            PaletteAction::OpenPresets => {
                self.palettes.reload();
                self.presets_window = Some(PresetsWindow::default());
            }
            PaletteAction::Load => {
                self.platform
                    .open_file(LOAD_FILTERS, OpenPurpose::LoadPalette, ctx)
            }
            PaletteAction::Save => self.save_palette_file(),
            PaletteAction::SaveAsPreset => {
                self.preset_name = Some(NameDialog {
                    name: self.doc.tab_name_stem(),
                    then_default: false,
                })
            }
            PaletteAction::SetAsDefault => self.set_current_as_default(),
            PaletteAction::LoadDefault => {
                let palette = self.palettes.default_palette();
                self.doc.editor.apply_palette(palette);
            }
            PaletteAction::OpenFolder => {
                if let Some(dir) = self.palettes.folder() {
                    let _ = std::fs::create_dir_all(&dir);
                    if let Err(e) = platform::open_folder(&dir) {
                        self.show_message("Could not open the folder", e);
                    }
                }
            }
        }
    }

    pub(crate) fn load_palette_bytes(&mut self, name: &str, bytes: &[u8]) {
        match palette_files::read_palette(name, bytes) {
            Ok(p) => {
                self.doc.editor.apply_palette(p.palette);
            }
            Err(e) => self.show_message("Could not load the palette", e.to_string()),
        }
    }

    fn current_palette(&self, name: String) -> NamedPalette {
        NamedPalette {
            name,
            palette: self.doc.editor.sprite().palette.clone(),
        }
    }

    fn save_palette_file(&mut self) {
        let suggested = format!("{}.gpl", self.doc.tab_name_stem());
        let Some(target) = self.platform.choose_save(&suggested, SAVE_FILTERS) else {
            return;
        };
        let palette = self.current_palette(target.stem().to_string());
        let bytes = palette_files::write_palette(save_format(&target.file_name), &palette)
            .expect("Gpl, Hex y Pal se pueden escribir");
        if let Err(e) = self.platform.write(&target, &bytes) {
            self.show_message("Could not save the palette", e);
        }
    }

    /// Si la paleta actual ya es un preset, lo marca; si no, primero pide un nombre.
    fn set_current_as_default(&mut self) {
        let current = &self.doc.editor.sprite().palette;
        let existing = self
            .palettes
            .user()
            .iter()
            .chain(self.palettes.builtin())
            .find(|p| p.palette == *current)
            .map(|p| p.name.clone());
        match existing {
            Some(name) => self.palettes.set_default(Some(name)),
            None => {
                self.preset_name = Some(NameDialog {
                    name: self.doc.tab_name_stem(),
                    then_default: true,
                })
            }
        }
    }

    /// Guarda en la biblioteca y, si se pidió, la marca como la paleta por defecto.
    pub(crate) fn store_preset(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        let name = palette.name.clone();
        match self.palettes.save(&palette, &comments) {
            Ok(()) if then_default => self.palettes.set_default(Some(name)),
            Ok(()) => {}
            Err(e) => self.show_message("Could not save the preset", e),
        }
    }

    /// Pregunta si reemplazar cuando ya existe un preset de usuario con ese nombre.
    pub(crate) fn save_preset_asking(
        &mut self,
        palette: NamedPalette,
        comments: Vec<String>,
        then_default: bool,
    ) {
        if self.palettes.has_user(&palette.name) {
            self.palette_confirm = Some(Confirm::Replace {
                palette,
                comments,
                then_default,
            });
        } else {
            self.store_preset(palette, comments, then_default);
        }
    }

    pub(crate) fn update_palette_windows(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.presets_window = None;
            self.preset_name = None;
            self.palette_confirm = None;
            return;
        }
        self.update_presets_window(ctx);
        self.update_name_dialog(ctx);
        self.update_palette_confirm(ctx);
    }

    fn update_presets_window(&mut self, ctx: &egui::Context) {
        let Some(mut window) = self.presets_window.take() else {
            return;
        };
        let mut open = true;
        let mut apply = None;
        let mut delete = None;
        let mut set_default = None;
        egui::Window::new("Palette Presets")
            .open(&mut open)
            .default_size([420.0, 480.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut window.tab, PresetsTab::Library, "Library");
                });
                ui.separator();
                match window.tab {
                    PresetsTab::Library => {
                        ui.horizontal(|ui| {
                            ui.label("Search");
                            ui.text_edit_singleline(&mut window.search);
                        });
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let default = self.palettes.default_name();
                            for (title, list) in [
                                ("Built-in", self.palettes.builtin()),
                                ("User", self.palettes.user()),
                            ] {
                                ui.strong(title);
                                let shown: Vec<&Preset> = list
                                    .iter()
                                    .filter(|p| matches_search(&p.name, &window.search))
                                    .collect();
                                if shown.is_empty() {
                                    ui.weak("No palettes");
                                }
                                for preset in shown {
                                    let is_default = default
                                        .is_some_and(|d| d.eq_ignore_ascii_case(&preset.name));
                                    let response = preset_row(ui, preset, is_default);
                                    if response.clicked() {
                                        apply = Some(preset.palette.clone());
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Set as Default").clicked() {
                                            set_default = Some(preset.name.clone());
                                            ui.close();
                                        }
                                        if preset.source == PresetSource::User
                                            && ui.button("Delete").clicked()
                                        {
                                            delete = Some(preset.name.clone());
                                            ui.close();
                                        }
                                    });
                                }
                                ui.add_space(8.0);
                            }
                        });
                    }
                }
            });
        if let Some(palette) = apply {
            self.doc.editor.apply_palette(palette);
        }
        if let Some(name) = set_default {
            self.palettes.set_default(Some(name));
        }
        if let Some(name) = delete {
            self.palette_confirm = Some(Confirm::Delete(name));
        }
        if open {
            self.presets_window = Some(window);
        }
    }

    fn update_name_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.preset_name.take() else {
            return;
        };
        let mut result = None;
        centered_dialog("Save as Preset").show(ctx, |ui| {
            let response = ui.text_edit_singleline(&mut dialog.name);
            response.request_focus();
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let valid = !dialog.name.trim().is_empty();
                if ui.add_enabled(valid, egui::Button::new("Save")).clicked() || (enter && valid) {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            });
        });
        match result {
            None => self.preset_name = Some(dialog),
            Some(false) => {}
            Some(true) => {
                let palette = self.current_palette(dialog.name.trim().to_string());
                self.save_preset_asking(palette, Vec::new(), dialog.then_default);
            }
        }
    }

    fn update_palette_confirm(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.palette_confirm.take() else {
            return;
        };
        let (title, body, ok) = match &confirm {
            Confirm::Replace { palette, .. } => (
                "Replace preset?",
                format!("A preset named \"{}\" already exists.", palette.name),
                "Replace",
            ),
            Confirm::Delete(name) => (
                "Delete preset?",
                format!("\"{name}\" will be removed from your presets."),
                "Delete",
            ),
        };
        let mut result = None;
        centered_dialog(title).show(ctx, |ui| {
            ui.label(body);
            ui.horizontal(|ui| {
                if ui.button(ok).clicked() {
                    result = Some(true);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    result = Some(false);
                }
            });
        });
        match (result, confirm) {
            (None, confirm) => self.palette_confirm = Some(confirm),
            (Some(false), _) => {}
            (
                Some(true),
                Confirm::Replace {
                    palette,
                    comments,
                    then_default,
                },
            ) => self.store_preset(palette, comments, then_default),
            (Some(true), Confirm::Delete(name)) => {
                if let Err(e) = self.palettes.delete(&name) {
                    self.show_message("Could not delete the preset", e);
                }
            }
        }
    }
}

/// Diálogo fijo en el centro, como "Unsaved changes" (`file_actions.rs`).
fn centered_dialog(title: &str) -> egui::Window<'static> {
    egui::Window::new(title.to_string())
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
}

/// Fila de la ventana: nombre, cantidad de colores y tira. Toda la fila es clicable.
fn preset_row(ui: &mut egui::Ui, preset: &Preset, is_default: bool) -> egui::Response {
    let inner = ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(&preset.name);
            ui.weak(format!("{} colors", preset.palette.len()));
            if !preset.author.is_empty() {
                ui.weak(format!("· {}", preset.author));
            }
            if is_default {
                ui.weak("· default");
            }
        });
        panels::palette_strip(ui, &preset.palette, STRIP_COLORS, 8.0);
    });
    ui.interact(
        inner.response.rect,
        inner.response.id.with("row"),
        egui::Sense::click(),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_format_follows_the_extension() {
        assert_eq!(save_format("a.hex"), PaletteFormat::Hex);
        assert_eq!(save_format("a.PAL"), PaletteFormat::Pal);
        assert_eq!(save_format("a.gpl"), PaletteFormat::Gpl);
        assert_eq!(save_format("a"), PaletteFormat::Gpl);
        assert_eq!(save_format("a.png"), PaletteFormat::Gpl);
    }

    #[test]
    fn search_matches_case_insensitive_substrings() {
        assert!(matches_search("Sweetie 16", "sweet"));
        assert!(matches_search("Sweetie 16", ""));
        assert!(!matches_search("PICO-8", "nes"));
    }
}
