//! Los paneles de la app (los flotantes del dock: Artboards, Slices, Groom, Lumen, FontForge, 3D,
//! Library, Dafont, Lospec; y las regiones fijas: Tools, Color, Timeline, el contenido de
//! Preview): se dibujan con `draw_panel`, que solo toma prestados los campos que cada uno usa, y
//! lo que piden se aplica después con `apply_panel_action`, con la app entera. `panel_flyout`
//! hospeda los del dock flotando junto a su botón, con lo que dice su ficha del registro; las
//! regiones fijas siguen en sus paneles de `editor_panels`.

use eframe::egui;

#[cfg(not(target_arch = "wasm32"))]
use crate::dafont;
use crate::panel_registry::{Body, Host, PanelId};
use crate::{
    ChimpApp, dock, font_panel, groom, library_panel, lumen, model_panel, palette_ui, panels,
    platform, tilemap_dialog, timeline, view,
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
    /// Abrir el diálogo Canvas Size (desde Artboards).
    OpenCanvasSize,
    /// Lo pedido desde el encabezado de la paleta (Color).
    Palette(panels::PaletteAction),
    /// New Tilemap Layer, desde la línea de tiempo.
    NewTilemap,
    /// La × del Preview.
    ClosePreview,
}

impl ChimpApp {
    /// El contenido del panel `id` en `ui` y lo que pidió. Lo que lo rodea (el marco, el
    /// tamaño, el scroll de la toolbar, lo que apaga un diálogo modal) es de su anfitrión,
    /// salvo en Color, que maneja el modal adentro: aquí va sin modal; donde puede haber un
    /// diálogo modal abierto, usa [`Self::draw_panel_modal`].
    pub(crate) fn draw_panel(&mut self, ui: &mut egui::Ui, id: PanelId) -> Option<PanelAction> {
        self.draw_panel_in(ui, id, None, false)
    }

    /// Como [`Self::draw_panel`], con un diálogo modal abierto (`modal`): Color no lo apaga
    /// entero (la paleta sigue viva), solo los menús de la paleta y la instancia.
    pub(crate) fn draw_panel_modal(
        &mut self,
        ui: &mut egui::Ui,
        id: PanelId,
        modal: bool,
    ) -> Option<PanelAction> {
        self.draw_panel_in(ui, id, None, modal)
    }

    /// Como [`Self::draw_panel_modal`]; `lospec_bottom` es hasta dónde llega la lista de Lospec
    /// (abajo de la columna del dock). Va como parámetro privado y no en un campo de la app:
    /// solo Lospec lo usa y solo `panel_flyout` lo sabe; sin él, la lista se mide como en
    /// Palette Presets.
    fn draw_panel_in(
        &mut self,
        ui: &mut egui::Ui,
        id: PanelId,
        lospec_bottom: Option<f32>,
        modal: bool,
    ) -> Option<PanelAction> {
        // Solo préstamos de campos sueltos: el skin junto al editor y al estado de cada panel.
        // `modal` solo lo usa Color: a los demás los apaga su anfitrión (`add_enabled_ui`).
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
            PanelId::Artboards => {
                let (editor, name) = (&mut self.doc.editor, &mut self.doc.artboard_name);
                let canvas_size = match skin {
                    Some(skin) => panels::pixel_artboards(ui, editor, name, skin),
                    None => panels::artboards(ui, editor, name),
                };
                canvas_size.then_some(PanelAction::OpenCanvasSize)
            }
            PanelId::Slices => {
                match skin {
                    Some(skin) => panels::pixel_slices(ui, &mut self.doc.editor, skin),
                    None => panels::slices(ui, &mut self.doc.editor),
                }
                None
            }
            PanelId::Tools => {
                panels::toolbar(ui, &mut self.doc.editor, skin);
                None
            }
            PanelId::Color => self.draw_color(ui, modal),
            PanelId::Timeline => {
                timeline::show(ui, &mut self.doc.editor, &mut self.doc.timeline, skin);
                std::mem::take(&mut self.doc.timeline.new_tilemap)
                    .then_some(PanelAction::NewTilemap)
            }
            PanelId::Preview => {
                // La textura del lienzo, si muestra el frame del editor tal cual.
                let canvas = self.doc.canvas.plain_texture(&self.doc.editor);
                self.preview.contents(ui, &self.doc.editor, skin, canvas);
                None
            }
        }
    }

    /// La columna de colores: la paleta (con Pixel UI, arriba y fuera del scroll, con el editor
    /// de color siempre entero), o en modo tiles el tileset y el selector de color, y la
    /// instancia. Con un diálogo modal (`modal`), los menús de la paleta no abren otro encima y
    /// la instancia queda apagada.
    fn draw_color(&mut self, ui: &mut egui::Ui, modal: bool) -> Option<PanelAction> {
        let skin = self.pixel.skin();
        let tiles = self.doc.editor.tiles_mode();
        let editor = &mut self.doc.editor;
        let height = &mut self.view.palette_height;
        let palette = |ui: &mut egui::Ui, editor: &mut crate::editor::Editor, height: &mut f32| {
            panels::palette(ui, editor, !modal, skin, height)
                .filter(|_| !modal)
                .map(PanelAction::Palette)
        };
        let mut action = None;
        // Con Pixel UI, la paleta y el editor de color van arriba, fuera del scroll: el editor
        // se ve siempre entero (si falta lugar, se achica la caja de la paleta).
        let fixed = skin.is_some() && !tiles;
        if fixed {
            action = palette(ui, editor, height);
        }
        crate::pixel_ui::scroll(ui, egui::ScrollArea::vertical(), |ui| {
            if tiles {
                panels::tileset(ui, editor, &mut self.doc.tile_thumbs);
                // El selector de color sigue a mano (lo usa 2B).
                ui.add_space(10.0);
                panels::color_picker(ui, editor, skin);
            } else if !fixed {
                action = palette(ui, editor, height);
            }
            // Artboards y Slices van en el dock (a la izquierda de este panel).
            ui.add_space(12.0);
            let entered = ui
                .add_enabled_ui(!modal, |ui| panels::instance(ui, editor))
                .inner;
            if entered {
                // A un símbolo de la biblioteca: se ve solo él, ajustado; a uno del lienzo,
                // se centra.
                // Se pide en el acto (el lienzo es otro campo): así no le gana a una acción de
                // la paleta del mismo frame.
                let zoom = if editor.in_symbol_mode() {
                    view::ZoomRequest::Fit
                } else {
                    view::ZoomRequest::CenterActive
                };
                self.doc.canvas.request_zoom(zoom);
            }
        });
        action
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
            PanelAction::OpenCanvasSize => self.open_canvas_size(),
            PanelAction::Palette(action) => self.palette_action(ctx, action),
            PanelAction::NewTilemap => {
                self.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
            }
            PanelAction::ClosePreview => self.view.preview = false,
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
            debug_assert!(
                false,
                "{id:?} no es un panel flotante (su host no es Flyout)"
            );
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
                action = self.draw_panel_in(ui, id, Some(lospec_bottom), modal);
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
        test_app_with(true)
    }

    /// La app con (o sin) Pixel UI y una pestaña nueva.
    fn test_app_with(pixel_ui: bool) -> ChimpApp {
        let view = ViewSettings {
            pixel_ui,
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
        contents: impl FnMut(&mut ChimpApp, &mut egui::Ui),
    ) {
        frame_with(ctx, app, Vec::new(), contents);
    }

    /// Como [`frame`], con los eventos `events`; devuelve la salida del frame.
    fn frame_with(
        ctx: &egui::Context,
        app: &mut ChimpApp,
        events: Vec<egui::Event>,
        mut contents: impl FnMut(&mut ChimpApp, &mut egui::Ui),
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            app.pixel.update(ui.ctx(), &app.view);
            egui::CentralPanel::default().show(ui, |ui| contents(app, ui));
        });
        // Las texturas (fuentes, skin e imágenes de los paneles) no se suben a ninguna GPU.
        output.textures_delta.clear();
        output
    }

    /// El nodo de accesibilidad con la etiqueta `label` en `output`.
    fn node(output: &egui::FullOutput, label: &str) -> Option<egui::accesskit::NodeId> {
        let update = output.platform_output.accesskit_update.as_ref()?;
        update
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some(label))
            .map(|(id, _)| *id)
    }

    /// Un clic de accesibilidad en el nodo `target` (como los de un lector de pantalla).
    fn accesskit_click(target: egui::accesskit::NodeId) -> egui::Event {
        egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::Click,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node: target,
            data: None,
        })
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

    #[test]
    fn artboards_and_slices_draw_through_the_registry_with_and_without_pixel_ui() {
        use crate::panel_registry::PanelId;
        for pixel_ui in [true, false] {
            let mut app = test_app_with(pixel_ui);
            let ctx = egui::Context::default();
            for _ in 0..2 {
                frame(&ctx, &mut app, |_, _| {});
            }
            for id in [PanelId::Artboards, PanelId::Slices] {
                frame(&ctx, &mut app, |app, ui| {
                    ui.set_width(id.spec().reference_width);
                    // Dibujarlos solos no pide nada: Canvas Size es un botón.
                    assert!(
                        app.draw_panel(ui, id).is_none(),
                        "{id:?} pixel_ui={pixel_ui}"
                    );
                });
            }
        }
    }

    #[test]
    fn the_fixed_regions_and_preview_draw_through_the_registry() {
        use crate::panel_registry::PanelId;
        for pixel_ui in [true, false] {
            let mut app = test_app_with(pixel_ui);
            let ctx = egui::Context::default();
            for _ in 0..2 {
                frame(&ctx, &mut app, |_, _| {});
            }
            for id in [
                PanelId::Tools,
                PanelId::Color,
                PanelId::Timeline,
                PanelId::Preview,
            ] {
                frame(&ctx, &mut app, |app, ui| {
                    // Dibujarlos solos no pide nada, pero ponen su contenido.
                    let drawn = ui.scope(|ui| assert!(app.draw_panel(ui, id).is_none()));
                    let rect = drawn.response.rect;
                    assert!(
                        rect.width() > 0.0 && rect.height() > 0.0,
                        "{id:?} pixel_ui={pixel_ui}: {rect:?}"
                    );
                });
            }
        }
    }

    #[test]
    fn the_model_panel_draws_through_the_registry_in_a_model_tab() {
        use crate::panel_registry::PanelId;
        let mut app = test_app();
        assert!(app.doc.editor.new_model("Ship", (32, 32)));
        let id = app.doc.editor.workspace().expect("the workspace");
        app.tabs.open_view(&mut app.doc, id);
        assert!(app.doc.editor.model_edit_on());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            frame(&ctx, &mut app, |_, _| {});
        }
        frame(&ctx, &mut app, |app, ui| {
            ui.set_width(PanelId::Model.spec().reference_width);
            let drawn = ui.scope(|ui| assert!(app.draw_panel(ui, PanelId::Model).is_none()));
            assert!(drawn.response.rect.height() > 0.0);
        });
    }

    #[test]
    fn a_palette_action_and_entering_an_instance_in_the_same_frame_both_happen() {
        use super::PanelAction;
        use crate::panel_registry::PanelId;
        use cs_core::{PixelValue, Rgba};
        // Sin Pixel UI (Presets ⏷ es un botón con nombre), con una instancia elegida en su capa.
        let mut app = test_app_with(false);
        let e = &mut app.doc.editor;
        e.execute(cs_commands::Command::DrawPixels {
            layer: e.layer,
            frame: 0,
            pixels: vec![(2, 2, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        });
        e.convert_to_symbol("S", cs_commands::symbol::SymbolTiming::Graphic)
            .expect("the symbol");
        assert!(e.on_object_layer() && e.instance_key().is_some());
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let color = |app: &mut ChimpApp, ui: &mut egui::Ui| app.draw_panel(ui, PanelId::Color);
        let output = frame_with(&ctx, &mut app, Vec::new(), |app, ui| {
            color(app, ui);
        });
        let presets = node(&output, "Presets ⏷").expect("Presets ⏷");
        let edit = node(&output, "Edit Symbol").expect("Edit Symbol");
        assert_eq!(app.doc.canvas.pending_zoom(), None);
        // Los dos clics en el mismo frame.
        let mut action = None;
        let events = vec![accesskit_click(presets), accesskit_click(edit)];
        frame_with(&ctx, &mut app, events, |app, ui| action = color(app, ui));
        assert!(
            matches!(
                action,
                Some(PanelAction::Palette(
                    crate::panels::PaletteAction::OpenPresets
                ))
            ),
            "the palette action"
        );
        assert!(app.doc.editor.in_symbol_mode(), "entered the symbol");
        assert_eq!(
            app.doc.canvas.pending_zoom(),
            Some(crate::view::ZoomRequest::Fit),
            "and the canvas fits it"
        );
    }

    #[test]
    fn the_fixed_region_actions_do_what_their_panels_did() {
        use super::PanelAction;
        let mut app = test_app();
        let ctx = egui::Context::default();
        // La × del Preview lo apaga.
        app.view.preview = true;
        app.apply_panel_action(&ctx, PanelAction::ClosePreview);
        assert!(!app.view.preview);
        // New Tilemap Layer desde la línea de tiempo abre su diálogo.
        assert!(app.new_tilemap.is_none());
        app.apply_panel_action(&ctx, PanelAction::NewTilemap);
        assert!(app.new_tilemap.is_some());
        // Lo del encabezado de la paleta va a `palette_action`.
        assert!(app.presets_window.is_none());
        app.apply_panel_action(
            &ctx,
            PanelAction::Palette(crate::panels::PaletteAction::OpenPresets),
        );
        assert!(app.presets_window.is_some());
    }

    #[test]
    fn open_canvas_size_action_opens_the_dialog() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        assert!(app.canvas_size.is_none());
        app.apply_panel_action(&ctx, super::PanelAction::OpenCanvasSize);
        assert!(app.canvas_size.is_some());
    }
}
