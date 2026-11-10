//! Los paneles del dock que pone la app (Groom, Lumen, FontForge, 3D, Library, Dafont, Lospec):
//! se dibujan con `draw_panel`, que solo toma prestados los campos que cada uno usa, y lo que
//! piden se aplica después con `apply_panel_action`, con la app entera. `panel_flyout` los
//! hospeda flotando junto a su botón, con lo que dice su ficha del registro.

use eframe::egui;

#[cfg(not(target_arch = "wasm32"))]
use crate::dafont;
use crate::panel_registry::{Body, Host, PanelId};
use crate::{
    ChimpApp, dock, font_panel, groom, library_panel, lumen, model_panel, palette_ui, platform,
    view,
};

/// Lo que pidió un panel de la app al dibujarse: se aplica con [`ChimpApp::apply_panel_action`].
pub(crate) enum PanelAction {
    Groom(groom::GroomAction),
    Lumen(lumen::LumenAction),
    Font(font_panel::FontAction),
    Model(model_panel::ModelAction),
    Library(library_panel::LibraryAction),
    /// La fuente elegida en dafont.
    #[cfg(not(target_arch = "wasm32"))]
    Dafont(dafont::Picked),
    Lospec(palette_ui::LospecAction),
}

impl ChimpApp {
    /// El contenido del panel `id` en `ui` y lo que pidió. Los que dibuja el mismo dock
    /// (Artboards, Slices) y los que no son del dock no ponen nada.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "los hospedajes fijos (el panel 3D de la pestaña de un modelo) lo usarán"
        )
    )]
    pub(crate) fn draw_panel(&mut self, ui: &mut egui::Ui, id: PanelId) -> Option<PanelAction> {
        self.draw_panel_in(ui, id, None)
    }

    /// Como [`Self::draw_panel`]; `lospec_bottom` es hasta dónde llega la lista de Lospec (abajo
    /// de la columna del dock). Va como parámetro privado y no en un campo de la app: solo
    /// Lospec lo usa y solo `panel_flyout` lo sabe; sin él, la lista se mide como en Palette
    /// Presets.
    fn draw_panel_in(
        &mut self,
        ui: &mut egui::Ui,
        id: PanelId,
        lospec_bottom: Option<f32>,
    ) -> Option<PanelAction> {
        // Solo préstamos de campos sueltos: el skin junto al editor y al estado de cada panel.
        let skin = self.pixel.skin();
        match id {
            PanelId::Groom => {
                let primary = self.doc.editor.resolve(self.doc.editor.primary);
                // Las paletas para ajustar el resultado: la del documento y las de la biblioteca.
                let current = groom::PaletteChoice {
                    name: groom::CURRENT_PALETTE.into(),
                    colors: self.doc.editor.sprite().palette.colors().to_vec(),
                };
                let library = self.palettes.user().iter().chain(self.palettes.builtin());
                let palettes: Vec<groom::PaletteChoice> = std::iter::once(current)
                    .chain(library.map(|p| groom::PaletteChoice {
                        name: p.name.clone(),
                        colors: p.palette.colors().to_vec(),
                    }))
                    .collect();
                self.groom
                    .ui(ui, primary, &palettes)
                    .map(PanelAction::Groom)
            }
            PanelId::Lumen => self.lumen.ui(ui).map(PanelAction::Lumen),
            PanelId::Font => font_panel::show(ui, &mut self.doc.editor, skin, &mut self.font_panel)
                .map(PanelAction::Font),
            PanelId::Model => {
                model_panel::show(ui, &mut self.doc.editor, skin, &mut self.model_panel)
                    .map(PanelAction::Model)
            }
            PanelId::Library => {
                library_panel::show(ui, &mut self.doc.editor, skin, &mut self.doc.library)
                    .map(PanelAction::Library)
            }
            #[cfg(not(target_arch = "wasm32"))]
            PanelId::Dafont => self.dafont.ui(ui).map(PanelAction::Dafont),
            PanelId::Lospec => self.draw_lospec(ui, lospec_bottom).map(PanelAction::Lospec),
            // Artboards y Slices los dibuja el dock; los demás no son paneles flotantes.
            PanelId::Artboards
            | PanelId::Slices
            | PanelId::Tools
            | PanelId::Color
            | PanelId::Timeline
            | PanelId::Preview => None,
        }
    }

    /// Lo que pidió un panel de la app: necesita la app entera, por eso va después de dibujarlo.
    pub(crate) fn apply_panel_action(&mut self, ctx: &egui::Context, action: PanelAction) {
        match action {
            PanelAction::Groom(action) => match action {
                groom::GroomAction::Import => {
                    self.platform
                        .open_file(groom::FILTER, platform::OpenPurpose::GroomImage, ctx);
                }
                groom::GroomAction::BrowseLospec => {
                    dock::set_open(ctx, Some(dock::DockPanel::Lospec));
                }
                groom::GroomAction::UseLayer => {
                    let (name, image) = self.current_layer_image();
                    self.groom.load(name, image);
                }
                groom::GroomAction::NewSprite(g) => self.bitmap_to_sprite(&g.name, &g.image),
                groom::GroomAction::AddArtboard(g) => self.bitmap_to_artboard(g.name, &g.image),
                groom::GroomAction::AddLayer(g) => self.bitmap_to_layer(&g.name, &g.image),
            },
            PanelAction::Lumen(action) => match action {
                lumen::LumenAction::Browse(kind) => {
                    let purpose = platform::OpenPurpose::LumenMap(kind);
                    self.platform.open_file(groom::FILTER, purpose, ctx);
                }
                lumen::LumenAction::UseLayer => {
                    let (name, image) = self.current_layer_image();
                    self.lumen.load(name, image);
                }
                lumen::LumenAction::Export(file_name, image) => {
                    let png = match image.to_png() {
                        Ok(png) => png,
                        Err(e) => return self.show_message("Could not export", e.to_string()),
                    };
                    let filters = [platform::FileFilter {
                        name: "PNG",
                        extensions: &["png"],
                    }];
                    if let Some(target) = self.platform.choose_save(&file_name, &filters)
                        && let Err(e) = self.platform.write(&target, &png)
                    {
                        self.show_message("Could not export", e);
                    }
                }
                lumen::LumenAction::NewSprite(name, image) => self.bitmap_to_sprite(&name, &image),
                lumen::LumenAction::AddArtboard(name, image) => {
                    self.bitmap_to_artboard(name, &image)
                }
                lumen::LumenAction::AddLayer(name, image) => self.bitmap_to_layer(&name, &image),
            },
            PanelAction::Font(action) => match action {
                font_panel::FontAction::NewFont => self.new_font = Some(Default::default()),
                font_panel::FontAction::Build(format) => self.open_build_font(Some(format)),
                font_panel::FontAction::AutoBuildAccents => self.auto_build_accents(),
                font_panel::FontAction::FillFromTtf => self.choose_fill_ttf(ctx),
                font_panel::FontAction::GlyphsFromImage => self.choose_glyph_sheet(ctx),
                font_panel::FontAction::OpenFont(id) => {
                    self.tabs.open_view(&mut self.doc, id);
                    self.show_font_beside_panel(ctx);
                }
                font_panel::FontAction::NewVariant => self.open_new_variant(),
            },
            PanelAction::Model(action) => self.model_action(Some(action)),
            PanelAction::Library(action) => self.library_action(ctx, action),
            // La fuente elegida queda en la herramienta de texto, a su tamaño y nítida.
            #[cfg(not(target_arch = "wasm32"))]
            PanelAction::Dafont(picked) => {
                let style = &mut self.doc.editor.text_style;
                style.font = picked.font;
                style.size = picked.size_px.unwrap_or(16);
                style.antialias = false;
            }
            PanelAction::Lospec(action) => self.lospec_action(Some(action)),
        }
    }

    /// Lo que pidió el panel Library.
    fn library_action(&mut self, ctx: &egui::Context, action: library_panel::LibraryAction) {
        let editor = &mut self.doc.editor;
        match action {
            library_panel::LibraryAction::Rename(id, name) => {
                editor.rename_artboard(id, name);
            }
            library_panel::LibraryAction::Duplicate(id) => {
                if let Some(new) = editor.duplicate_library_item(id) {
                    editor.library_selected = Some(new);
                }
            }
            library_panel::LibraryAction::PlaceOnCanvas(id)
                if editor
                    .document
                    .artboard(id)
                    .is_some_and(|a| a.model.is_some()) =>
            {
                // Un modelo no sale de la biblioteca: pone una instancia en el centro del
                // artboard activo del lienzo (o el primero), como al arrastrarlo.
                let target = editor
                    .document
                    .artboard(editor.active)
                    .filter(|a| a.on_canvas())
                    .or_else(|| editor.document.canvas_artboards().next());
                if let Some(a) = target {
                    let ((x0, y0), (x1, y1)) = a.rect();
                    let at = ((x0 + x1) / 2, (y0 + y1) / 2);
                    let _ = editor.drop_symbol(id, at);
                }
            }
            library_panel::LibraryAction::PlaceOnCanvas(id) => {
                if editor.place_on_canvas(id) {
                    self.doc
                        .canvas
                        .request_zoom(view::ZoomRequest::CenterActive);
                }
            }
            library_panel::LibraryAction::Delete(id) => {
                if editor.execute_doc(cs_commands::DocCommand::RemoveArtboard { id })
                    && editor.library_selected == Some(id)
                {
                    editor.library_selected = None;
                }
            }
            library_panel::LibraryAction::Open(id) => {
                self.tabs.open_view(&mut self.doc, id);
                if self.doc.editor.workspace() == Some(id) {
                    self.show_workspace_panel(ctx);
                }
            }
            library_panel::LibraryAction::ImportFont => self.choose_font_file(ctx),
            library_panel::LibraryAction::Edit(id) => {
                if editor.enter_symbol(id) {
                    self.doc.canvas.request_zoom(view::ZoomRequest::Fit);
                }
            }
        }
    }

    /// El panel `id` flotando junto a su botón del dock (`anchor`), con el ancho y el cuerpo de
    /// su ficha; con un diálogo modal abierto (`modal`), apagado. Lo que pide se aplica al final.
    pub(crate) fn panel_flyout(
        &mut self,
        ctx: &egui::Context,
        id: PanelId,
        anchor: dock::Anchor,
        modal: bool,
    ) {
        let spec = id.spec();
        let Host::Flyout(body) = spec.host else {
            return;
        };
        // El skin compartido: el contenido toma prestada la app entera mientras se dibuja.
        let skin = self.pixel.shared_skin();
        let skin = skin.as_deref();
        // La lista de Lospec, hasta abajo de la columna del dock.
        let lospec_bottom = anchor.column.bottom() - dock::FLYOUT_MARGIN as f32;
        let mut action = None;
        let contents = |ui: &mut egui::Ui| {
            // La franja del logo de Lospec queda fuera de lo que apaga el modal, como siempre.
            if id == PanelId::Lospec {
                palette_ui::lospec_band(ui, skin);
            }
            ui.add_enabled_ui(!modal, |ui| {
                action = self.draw_panel_in(ui, id, Some(lospec_bottom));
            });
        };
        let width = spec.reference_width;
        match body {
            Body::Fixed => dock::flyout_fixed(ctx, id, anchor, skin, width, contents),
            Body::Fit | Body::Tall => {
                dock::flyout(ctx, id, anchor, skin, width, body == Body::Tall, contents)
            }
        }
        if let Some(action) = action {
            self.apply_panel_action(ctx, action);
        }
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;

    use crate::ChimpApp;
    use crate::palette_library::PaletteLibrary;
    use crate::palette_library::test_util::NoPresets;
    use crate::view::ViewSettings;

    /// La app con Pixel UI y una pestaña nueva.
    fn test_app() -> ChimpApp {
        let view = ViewSettings {
            pixel_ui: true,
            ..Default::default()
        };
        let palettes = PaletteLibrary::new(Box::new(NoPresets));
        let mut app = ChimpApp::with_parts(Default::default(), view, palettes);
        app.new_document();
        app
    }

    /// Un frame de `app` con `contents` en el panel central: Pixel UI se aplica antes, como en
    /// la app.
    fn frame(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        mut contents: impl FnMut(&mut ChimpApp, &mut egui::Ui),
    ) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.pixel.update(ui.ctx(), &app.view);
            egui::CentralPanel::default().show(ui, |ui| contents(app, ui));
        });
        // Las texturas (fuentes, skin e imágenes de los paneles) no se suben a ninguna GPU.
        output.textures_delta.clear();
    }

    #[test]
    fn every_dock_panel_draws_through_the_registry() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        // La escala de Pixel UI rige desde el segundo frame.
        for _ in 0..2 {
            frame(&ctx, &mut app, |_, _| {});
        }
        assert!(app.pixel.skin().is_some(), "Pixel UI on");
        for &id in crate::panel_registry::PanelId::DOCK {
            frame(&ctx, &mut app, |app, ui| {
                // Del ancho de su ficha, como en el flotante.
                ui.set_width(id.spec().reference_width);
                assert!(app.draw_panel(ui, id).is_none(), "{id:?}");
            });
        }
    }
}
