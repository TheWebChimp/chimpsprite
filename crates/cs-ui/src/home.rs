//! Pestaña Home: crear, abrir, demo y archivos recientes.

use eframe::egui::{self, RichText};

use crate::ChimpApp;
use crate::theme;

impl ChimpApp {
    pub(crate) fn home(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            ui.vertical(|ui| {
                // El logo animado; si no se pudo leer, el nombre en texto.
                let logo = self
                    .logo
                    .get_or_insert_with(|| crate::logo::Logo::embedded(&ctx));
                match logo {
                    Some(logo) => {
                        logo.show(ui);
                    }
                    None => {
                        ui.label(RichText::new("ChimpSprite").size(28.0).color(theme::ACCENT));
                    }
                }
                ui.label(RichText::new("Sprite & pixel-art editor").color(theme::LABEL));
                ui.add_space(16.0);
                if ui.button("📄  New file").clicked() {
                    self.new_document();
                }
                if ui.button("📂  Open file…").clicked() {
                    self.open_dialog(&ctx);
                }
                if ui.button("🐵  Open demo").clicked() {
                    self.open_demo();
                }
            });
            ui.add_space(48.0);
            #[cfg(not(target_arch = "wasm32"))]
            self.recent_lists(ui, &ctx);
            #[cfg(target_arch = "wasm32")]
            ui.vertical(|ui| {
                ui.label(RichText::new("In the browser").strong());
                ui.label("Files open from your computer and save as downloads.");
                ui.label("Unsaved documents are kept in this browser and offered back next time.");
            });
            // A la derecha, lo que queda: Pixnews, con una fila para elegir la fuente (en web,
            // solo el enlace: no todas mandan CORS).
            ui.add_space(48.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 16.0).max(0.0));
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.horizontal_wrapped(|ui| {
                        for source in crate::news::Source::ALL {
                            let on = self.news.source == source;
                            if ui.selectable_label(on, source.label()).clicked() {
                                self.news.select(source);
                            }
                        }
                    });
                    ui.add_space(6.0);
                    let action = self.news.ui(ui, self.news_fetcher.as_ref());
                    if let Some(crate::news::FeedAction::Open(url)) = action {
                        ctx.open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                #[cfg(target_arch = "wasm32")]
                ui.hyperlink_to(
                    "r/PixelArt · Top this week",
                    "https://www.reddit.com/r/PixelArt/top/?t=week",
                );
            });
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn recent_lists(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let files = self.recent.files().to_vec();
        let folders = self.recent.folders();
        ui.vertical(|ui| {
            ui.label(RichText::new("Recent files").strong());
            ui.add_space(4.0);
            if files.is_empty() {
                ui.weak("Nothing yet — files you open or save show up here.");
            }
            for path in files {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let dir = path
                    .parent()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default();
                ui.horizontal(|ui| {
                    if ui
                        .link(&name)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        if path.exists() {
                            self.platform
                                .open_path(&path, crate::platform::OpenPurpose::Open);
                        } else {
                            self.recent.remove(&path);
                            self.show_message("File not found", path.display().to_string());
                        }
                    }
                    ui.weak(dir);
                });
            }
            ui.add_space(16.0);
            ui.label(RichText::new("Recent folders").strong());
            ui.add_space(4.0);
            for dir in folders {
                let name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                ui.horizontal(|ui| {
                    if ui
                        .link(&name)
                        .on_hover_text(dir.display().to_string())
                        .clicked()
                    {
                        self.platform.open_file_in(
                            crate::file_actions::SPRITE_FILTERS,
                            &dir,
                            crate::platform::OpenPurpose::Open,
                            ctx,
                        );
                    }
                    ui.weak(dir.display().to_string());
                });
            }
        });
    }
}
