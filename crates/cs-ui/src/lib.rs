//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod anim_ops;
mod app_actions;
mod app_menus;
mod appearance;
mod artboard_sheets;
mod artboard_window;
mod autosave;
mod bake_dialog;
mod bar_fields;
mod canvas;
mod canvas_size;
mod capture;
mod clipboard;
mod color_panel;
#[cfg(not(target_arch = "wasm32"))]
mod dafont;
mod demo;
#[cfg(test)]
mod dialog_tests;
mod dock;
mod edit_ops;
mod editor;
mod expand_dialog;
mod export_dialog;
mod fg_bg;
mod file_actions;
mod filter_dialog;
mod filters;
mod font_actions;
mod font_dialog;
mod font_io;
mod font_kern_panel;
mod font_overlay;
mod font_panel;
mod font_sheet_dialog;
mod frame_dialog;
mod gif_dialog;
mod mask_ops;
mod menus;
mod model_dialog;
mod model_panel;
mod palette_ops;
mod palette_ui;
mod scripts;
mod select_dialogs;
mod sheet_dialog;
mod slice_ops;
mod sprite_dialogs;
mod tab_bar;
use crate::pixel_ui::Tip as _;
use cs_io::files;
mod groom;
mod guides;
mod home;
mod instance_ops;
mod keyboard_shortcuts;
mod keymap;
mod kit;
mod layer_ops;
#[cfg(test)]
mod layout_tests;
mod library_panel;
mod logo;
mod lospec_client;
#[cfg(not(target_arch = "wasm32"))]
mod lospec_examples;
mod lospec_tab;
mod lumen;
#[cfg(not(target_arch = "wasm32"))]
mod news;
mod notes;
mod object_ops;
mod palette_library;
mod panel_kit;
mod panels;
mod pixel_cursors;
mod pixel_draw;
mod pixel_home;
mod pixel_rulers;
mod pixel_sections;
mod pixel_selection;
mod pixel_status;
mod pixel_ui;
mod platform;
mod preferences;
mod preview;
mod recent;
pub mod skin;
mod sound;
mod symbol_dialog;
mod symbol_ops;
mod tabs;
mod theme;
mod tile_ops;
mod tiled_dialog;
mod tilemap_dialog;
mod timeline;
mod timeline_clip;
mod tool_group;
#[cfg(test)]
mod tool_tests;
mod transform;
mod transform_box;
mod tween_ops;
mod undo_history;
mod view;
mod widgets;

use cs_core::{ColorMode, Sprite};
use eframe::egui::{self, Key, Modifiers};

pub use editor::{Editor, ToolKind};

/// Aviso cuando `Convert to Image Layer` no puede dibujar la capa.
const CONVERT_TO_IMAGE_FAILED: &str =
    "Could not convert the layer: a tilemap would exceed the image size limit.";

const NEW_SPRITE_SIZE: u32 = 64;
/// Con Pixel UI, del mono que corre (arriba a la derecha) al borde derecho de la ventana.
const RUNNER_RIGHT: f32 = 5.0;
const RECENT_KEY: &str = "chimpsprite.recent";
const VIEW_KEY: &str = "chimpsprite.view";
const DEFAULT_PALETTE_KEY: &str = "chimpsprite.default_palette";
const TOOL_OPTIONS_KEY: &str = "chimpsprite.tool_options";
/// El pincel personalizado (Edit → New Brush): sus puntos y colores.
const CUSTOM_BRUSH_KEY: &str = "chimpsprite.custom_brush";
/// Los atajos elegidos en Edit → Keyboard Shortcuts.
const KEYMAP_KEY: &str = "chimpsprite.keymap";

/// Texto que queda en el portapapeles del sistema al copiar píxeles (ver `shortcuts`).
const CLIPBOARD_MARKER: &str = "ChimpSprite pixels";

pub struct ChimpApp {
    /// El ancho guardado de la columna de colores ya rige y el usuario la tocó (o la eligió):
    /// desde ahí se guarda lo que él arrastre. Antes, el ancho guardado se impone cada cuadro.
    palette_width_settled: bool,
    /// Documento de la pestaña activa (un marcador vacío si se está en Home).
    doc: tabs::DocState,
    tabs: tabs::Tabs,
    capture: capture::Capture,
    canvas_size: Option<canvas_size::CanvasSizeDialog>,
    /// El diálogo del menú Sprite abierto (propiedades, duplicar, tamaño).
    sprite_dialog: Option<sprite_dialogs::SpriteDialog>,
    /// Las fuentes pixel de dafont.com para la herramienta de texto (solo escritorio).
    #[cfg(not(target_arch = "wasm32"))]
    dafont: dafont::DafontPanel,
    /// El panel Groom del dock.
    groom: groom::GroomPanel,
    /// El panel Lumen del dock.
    lumen: lumen::LumenPanel,
    /// El panel Font del dock.
    font_panel: font_panel::FontPanelState,
    /// El panel 3D del dock.
    model_panel: model_panel::ModelPanelState,
    platform: platform::Platform,
    autosaver: autosave::Autosaver,
    restore: Option<Vec<autosave::Restore>>,
    recent: recent::RecentFiles,
    view: view::ViewSettings,
    grid_dialog: Option<view::GridDialog>,
    /// El logo animado de Home: se carga la primera vez que se muestra Home (`None` hasta
    /// entonces; `Some(None)` si no se pudo leer).
    logo: Option<Option<logo::Logo>>,
    /// El mono que corre arriba a la derecha con Pixel UI (como `logo`).
    runner: Option<Option<logo::Logo>>,
    /// Pixnews, las novedades de Home, y cómo se piden (en los tests, sin red). Solo escritorio.
    #[cfg(not(target_arch = "wasm32"))]
    news: news::News,
    #[cfg(not(target_arch = "wasm32"))]
    news_fetcher: Box<dyn news::Fetcher>,
    new_tilemap: Option<tilemap_dialog::NewTilemapDialog>,
    /// `File → New Font…`.
    new_font: Option<font_dialog::NewFontDialog>,
    /// Build ▾ del panel FontForge y File → Export Font….
    build_font: Option<font_io::BuildFontDialog>,
    /// File → Import Font… de un TTF/OTF.
    import_font: Option<Box<font_io::ImportFontDialog>>,
    /// FontForge → Generate ▾ → Glyphs from image….
    glyph_sheet: Option<Box<font_sheet_dialog::GlyphSheetDialog>>,
    /// `File → New 3D Model…`.
    new_model: Option<model_dialog::NewModelDialog>,
    /// Bake… del panel 3D.
    bake: Option<bake_dialog::BakeDialog>,
    /// GIF settings… del panel 3D.
    gif: Option<gif_dialog::GifDialog>,
    /// La ventana Edit → Undo History abierta.
    undo_history: bool,
    /// La ventana Edit → Keyboard Shortcuts abierta.
    shortcuts: Option<keyboard_shortcuts::ShortcutsWindow>,
    /// Los atajos de teclado (los de fábrica con los elegidos encima).
    keymap: keymap::Keymap,
    /// La ventana Edit → Preferences abierta.
    preferences: bool,
    /// El diálogo de un filtro de Edit (Replace Color, Adjustments, FX), con su vista previa.
    filter_dialog: Option<filter_dialog::FilterDialog>,
    /// Los últimos valores de Outline que se dejaron (menos el color), para la próxima vez.
    last_outline: Option<(filters::OutlineFx, filters::FilterTarget)>,
    /// Select → Color Range… o Modify abierto.
    select_dialog: Option<select_dialogs::SelectDialog>,
    /// Frame → Frame Properties… abierto.
    frame_dialog: Option<frame_dialog::FramePropertiesDialog>,
    /// File → Import Sprite Sheet… abierto.
    sheet_dialog: Option<sheet_dialog::SheetDialog>,
    /// Expand to Spritesheet… (menú de un artboard) abierto.
    expand_dialog: Option<expand_dialog::ExpandDialog>,
    /// `InputState::time` del último avance de Play in Place (`None` sin nada animándose).
    play_in_place_time: Option<f64>,
    /// La ventanita de preview (abierta con `view.preview`).
    preview: preview::Preview,
    convert_tilemap: Option<tilemap_dialog::TileSizeDialog>,
    /// Layer → Convert to Symbol… (F8) abierto.
    symbol_dialog: Option<symbol_dialog::SymbolDialog>,
    /// Desarrollo (`CHIMPSPRITE_DEMO_TILES`): al abrir el documento, activar su capa tilemap.
    demo_tiles: bool,
    fullscreen: bool,
    pending: Option<file_actions::Pending>,
    message: Option<file_actions::Message>,
    export: Option<export_dialog::ExportDialog>,
    /// Opciones de la última exportación, para reabrir el diálogo igual.
    last_export: export_dialog::ExportDialog,
    /// File → Export Tiled Map… abierto.
    tiled_export: Option<tiled_dialog::TiledDialog>,
    /// Opciones de la última exportación a Tiled.
    last_tiled: tiled_dialog::TiledDialog,
    /// Cerrar sin preguntar (ya se confirmó).
    allow_close: bool,
    title: String,
    /// Lo último copiado o cortado (compartido entre pestañas).
    clipboard: Option<clipboard::Clip>,
    /// Lo copiado del timeline (frames o cels). Ctrl+V pega lo último que se copió: esto si
    /// `paste_timeline`, si no los píxeles.
    timeline_clipboard: Option<timeline_clip::TimelineClip>,
    paste_timeline: bool,
    palettes: palette_library::PaletteLibrary,
    presets_window: Option<palette_ui::PresetsWindow>,
    /// El panel Lospec del dock (mientras está abierto).
    lospec_dock: Option<palette_ui::PresetsWindow>,
    preset_name: Option<palette_ui::NameDialog>,
    palette_confirm: Option<palette_ui::Confirm>,
    /// Ya llegó el `Paste` de este Ctrl+V (ver `clipboard::paste_request`).
    pasted: bool,
    /// View → Pixel UI: skin, fuente y escala aplicadas.
    pixel: appearance::PixelUi,
    /// Opciones de herramienta de la última pestaña abierta (o las guardadas al arrancar). Cada
    /// pestaña nueva empieza con las de la pestaña activa.
    tool_options: editor::ToolOptions,
}

/// En la memoria de egui: la fila de arriba de la barra de estado de este frame.
fn status_top_id() -> egui::Id {
    egui::Id::new("app_status_top")
}

impl ChimpApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let launch = platform::launch_options();
        let recent = cc
            .storage
            .and_then(|s| eframe::get_value(s, RECENT_KEY))
            .unwrap_or_default();
        let view = cc
            .storage
            .and_then(|s| eframe::get_value(s, VIEW_KEY))
            .unwrap_or_default();
        let mut palettes = palette_library::PaletteLibrary::for_platform();
        palettes.set_stored_default(
            cc.storage
                .and_then(|s| eframe::get_value::<Option<String>>(s, DEFAULT_PALETTE_KEY))
                .flatten(),
        );
        let mut app = Self::with_parts(recent, view, palettes);
        app.tool_options = cc
            .storage
            .and_then(|s| eframe::get_value(s, TOOL_OPTIONS_KEY))
            .unwrap_or_default();
        if let Some(brush) = cc.storage.and_then(|s| {
            eframe::get_value::<Vec<((i32, i32), cs_core::Rgba)>>(s, CUSTOM_BRUSH_KEY)
        }) {
            cs_raster::set_custom_brush(brush);
        }
        if let Some(settings) = cc
            .storage
            .and_then(|s| eframe::get_value::<keymap::KeymapSettings>(s, KEYMAP_KEY))
        {
            app.keymap = keymap::Keymap::from_settings(&settings);
        }
        if launch.demo {
            app.open_demo();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &launch.open {
            app.platform.open_path(path, platform::OpenPurpose::Open);
        }
        // Sesiones guardadas en el navegador (solo web).
        if !launch.demo
            && let Some(text) = autosave::storage::load()
        {
            match autosave::decode(&text) {
                Ok(docs) if !docs.is_empty() => app.restore = Some(docs),
                Ok(_) => {}
                Err(e) => {
                    log::warn!("discarding unreadable autosave: {e}");
                    autosave::storage::clear();
                }
            }
        }
        platform::install_unload_guard();
        // Desarrollo: lo que piden las variables `CHIMPSPRITE_DEMO_*`, para capturas.
        demo::startup(&mut app, &cc.egui_ctx);
        app
    }

    /// La app sin ventana, con lo guardado ya leído. `new` la usa y los tests la arman directo.
    fn with_parts(
        recent: recent::RecentFiles,
        view: view::ViewSettings,
        palettes: palette_library::PaletteLibrary,
    ) -> Self {
        Self {
            palette_width_settled: false,
            doc: tabs::DocState::untitled(""),
            tabs: tabs::Tabs::default(),
            capture: capture::Capture::from_env(),
            canvas_size: None,
            sprite_dialog: None,
            #[cfg(not(target_arch = "wasm32"))]
            dafont: Default::default(),
            groom: Default::default(),
            lumen: Default::default(),
            font_panel: Default::default(),
            model_panel: Default::default(),
            platform: platform::Platform::default(),
            autosaver: autosave::Autosaver::default(),
            restore: None,
            recent,
            view,
            grid_dialog: None,
            logo: None,
            runner: None,
            #[cfg(not(target_arch = "wasm32"))]
            news: news::News::default(),
            #[cfg(not(target_arch = "wasm32"))]
            news_fetcher: news::default_fetcher(),
            new_tilemap: None,
            new_font: None,
            build_font: None,
            import_font: None,
            glyph_sheet: None,
            new_model: None,
            bake: None,
            gif: None,
            undo_history: false,
            shortcuts: None,
            keymap: keymap::Keymap::default(),
            preferences: false,
            filter_dialog: None,
            last_outline: None,
            select_dialog: None,
            frame_dialog: None,
            sheet_dialog: None,
            expand_dialog: None,
            play_in_place_time: None,
            preview: preview::Preview::default(),
            convert_tilemap: None,
            symbol_dialog: None,
            demo_tiles: std::env::var_os("CHIMPSPRITE_DEMO_TILES").is_some(),
            fullscreen: false,
            pending: None,
            message: None,
            export: None,
            last_export: export_dialog::ExportDialog::default(),
            tiled_export: None,
            last_tiled: tiled_dialog::TiledDialog::default(),
            allow_close: false,
            title: String::new(),
            clipboard: None,
            timeline_clipboard: None,
            paste_timeline: false,
            palettes,
            presets_window: None,
            lospec_dock: None,
            preset_name: None,
            palette_confirm: None,
            pasted: false,
            pixel: appearance::PixelUi::from_env(),
            tool_options: editor::ToolOptions::default(),
        }
    }

    /// `false` en Home.
    pub(crate) fn has_doc(&self) -> bool {
        self.tabs.active().is_some()
    }

    fn open_canvas_size(&mut self) {
        // Una fuente tiene la celda fija.
        if !self.doc.editor.can_resize_sprite() {
            return;
        }
        self.canvas_size = Some(canvas_size::CanvasSizeDialog::new(
            self.doc.editor.artboard(),
        ));
    }

    /// Una nota (post-it) nueva al centro de lo que se ve, lista para escribir.
    fn new_note(&mut self) {
        let at = self.doc.canvas.new_note_position();
        if let Some(id) = self.doc.editor.add_note(at) {
            self.doc
                .canvas
                .notes
                .start_editing(&mut self.doc.editor, id);
        }
    }

    fn open_sprite_dialog(
        &mut self,
        dialog: impl FnOnce(&cs_core::Artboard) -> sprite_dialogs::SpriteDialog,
    ) {
        self.sprite_dialog = Some(dialog(self.doc.editor.artboard()));
    }

    fn update_sprite_dialog(&mut self, ctx: &egui::Context) {
        use sprite_dialogs::SpriteDialog;
        if !self.has_doc() {
            self.sprite_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.sprite_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Cancel => self.sprite_dialog = None,
            tilemap_dialog::DialogResult::Accept => {
                let editor = &mut self.doc.editor;
                match self.sprite_dialog.take() {
                    Some(SpriteDialog::Properties(d)) => {
                        editor.rename_active_artboard(d.name);
                        if let Some(index) = d.transparent_index {
                            editor.set_transparent_index(index);
                        }
                        editor.set_pixel_ratio(d.pixel_ratio);
                    }
                    Some(SpriteDialog::Duplicate(d)) => {
                        editor.duplicate_artboard(d.name);
                    }
                    Some(SpriteDialog::Size(d)) => {
                        editor.scale_sprite(d.width, d.height);
                    }
                    None => {}
                }
            }
        }
    }

    fn update_canvas_size(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.canvas_size else {
            return;
        };
        if self.tabs.active().is_none()
            || self.doc.editor.document.artboard(dialog.artboard).is_none()
        {
            self.canvas_size = None;
            return;
        }
        match canvas_size::show(ctx, dialog, self.pixel.skin()) {
            canvas_size::DialogResult::Open => {}
            canvas_size::DialogResult::Accept => {
                if let Some(cmd) = dialog.command(&self.doc.editor.document) {
                    self.doc.editor.execute_doc(cmd);
                }
                self.canvas_size = None;
            }
            canvas_size::DialogResult::Cancel => self.canvas_size = None,
        }
    }

    /// Desarrollo: activa la capa tilemap de más arriba y los números de tile (para capturas).
    fn demo_tiles(&mut self) {
        if !self.demo_tiles || !self.has_doc() {
            return;
        }
        self.demo_tiles = false;
        self.view.tile_numbers = true;
        let editor = &mut self.doc.editor;
        let top = editor
            .sprite()
            .layers
            .iter()
            .rev()
            .find(|l| l.kind == cs_core::LayerKind::Tilemap)
            .map(|l| l.id);
        if let Some(id) = top {
            editor.select_layer(id);
        }
    }

    /// El panel Groom del dock y lo que pide.
    fn groom_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let primary = self.doc.editor.resolve(self.doc.editor.primary);
        let mut action = None;
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
        let groom = &mut self.groom;
        let width = groom::WIDTH;
        dock::flyout(
            ctx,
            dock::DockPanel::Groom,
            anchor,
            self.pixel.skin(),
            width,
            true,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| action = groom.ui(ui, primary, &palettes));
            },
        );
        match action {
            Some(groom::GroomAction::Import) => {
                self.platform
                    .open_file(groom::FILTER, platform::OpenPurpose::GroomImage, ctx);
            }
            Some(groom::GroomAction::BrowseLospec) => {
                dock::set_open(ctx, Some(dock::DockPanel::Lospec));
            }
            Some(groom::GroomAction::UseLayer) => {
                let (name, image) = self.current_layer_image();
                self.groom.load(name, image);
            }
            Some(groom::GroomAction::NewSprite(g)) => self.bitmap_to_sprite(&g.name, &g.image),
            Some(groom::GroomAction::AddArtboard(g)) => self.bitmap_to_artboard(g.name, &g.image),
            Some(groom::GroomAction::AddLayer(g)) => self.bitmap_to_layer(&g.name, &g.image),
            None => {}
        }
    }

    /// La capa actual en el frame actual, como imagen, y su nombre.
    fn current_layer_image(&self) -> (String, cs_io::export::Bitmap) {
        let editor = &self.doc.editor;
        let sprite = editor.sprite();
        let pixels = (0..sprite.height as i32)
            .flat_map(|y| (0..sprite.width as i32).map(move |x| (x, y)))
            .map(|(x, y)| editor.resolve(sprite.pixel(editor.layer, editor.frame, x, y)))
            .collect();
        let name = sprite
            .layer(editor.layer)
            .map_or("Layer".to_string(), |l| l.name.clone());
        let image = cs_io::export::Bitmap {
            width: sprite.width,
            height: sprite.height,
            pixels,
        };
        (name, image)
    }

    /// Un sprite nuevo, en su pestaña, con `image` en su única capa.
    fn bitmap_to_sprite(&mut self, name: &str, image: &cs_io::export::Bitmap) {
        self.open_sheet_sprite(name, bitmap_sprite(image));
    }

    /// Un artboard nuevo con `image` en su única capa.
    fn bitmap_to_artboard(&mut self, name: String, image: &cs_io::export::Bitmap) {
        self.doc
            .editor
            .add_artboard_with_sprite(name, bitmap_sprite(image));
    }

    /// `image` como capa nueva del sprite actual.
    fn bitmap_to_layer(&mut self, name: &str, image: &cs_io::export::Bitmap) {
        match image.to_png() {
            Ok(bytes) => self.import_layer_bytes(
                &file_actions::Loaded {
                    name: format!("{name}.png"),
                    target: None,
                    bytes,
                },
                false,
            ),
            Err(e) => self.show_message("Could not add the layer", e.to_string()),
        }
    }

    /// El panel Lumen del dock y lo que pide.
    fn lumen_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let mut action = None;
        let lumen = &mut self.lumen;
        dock::flyout(
            ctx,
            dock::DockPanel::Lumen,
            anchor,
            self.pixel.skin(),
            lumen::WIDTH,
            true,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| action = lumen.ui(ui));
            },
        );
        match action {
            Some(lumen::LumenAction::Browse(kind)) => {
                let purpose = platform::OpenPurpose::LumenMap(kind);
                self.platform.open_file(groom::FILTER, purpose, ctx);
            }
            Some(lumen::LumenAction::UseLayer) => {
                let (name, image) = self.current_layer_image();
                self.lumen.load(name, image);
            }
            Some(lumen::LumenAction::Export(file_name, image)) => {
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
            Some(lumen::LumenAction::NewSprite(name, image)) => {
                self.bitmap_to_sprite(&name, &image)
            }
            Some(lumen::LumenAction::AddArtboard(name, image)) => {
                self.bitmap_to_artboard(name, &image)
            }
            Some(lumen::LumenAction::AddLayer(name, image)) => self.bitmap_to_layer(&name, &image),
            None => {}
        }
    }

    /// El panel Font del dock y lo que pide.
    fn font_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let mut action = None;
        let skin = self.pixel.skin();
        let editor = &mut self.doc.editor;
        let state = &mut self.font_panel;
        dock::flyout_fixed(
            ctx,
            dock::DockPanel::Font,
            anchor,
            skin,
            font_panel::WIDTH,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    action = font_panel::show(ui, editor, skin, state);
                });
            },
        );
        match action {
            Some(font_panel::FontAction::NewFont) => self.new_font = Some(Default::default()),
            Some(font_panel::FontAction::Build(format)) => self.open_build_font(Some(format)),
            Some(font_panel::FontAction::AutoBuildAccents) => self.auto_build_accents(),
            Some(font_panel::FontAction::FillFromTtf) => self.choose_fill_ttf(ctx),
            Some(font_panel::FontAction::GlyphsFromImage) => self.choose_glyph_sheet(ctx),
            None => {}
        }
    }

    /// El panel 3D del dock y lo que pide.
    fn model_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let mut action = None;
        let skin = self.pixel.skin();
        let editor = &mut self.doc.editor;
        let state = &mut self.model_panel;
        dock::flyout_fixed(
            ctx,
            dock::DockPanel::Model,
            anchor,
            skin,
            model_panel::WIDTH,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    action = model_panel::show(ui, editor, skin, state);
                });
            },
        );
        self.model_action(action);
    }

    /// Lo que pidió el panel 3D.
    fn model_action(&mut self, action: Option<model_panel::ModelAction>) {
        match action {
            Some(model_panel::ModelAction::NewModel) => self.new_model = Some(Default::default()),
            Some(model_panel::ModelAction::Bake) if self.doc.editor.edited_model().is_some() => {
                self.bake = Some(bake_dialog::BakeDialog::new(&self.doc.editor));
            }
            Some(model_panel::ModelAction::Gif) if self.doc.editor.edited_model().is_some() => {
                self.gif = Some(gif_dialog::GifDialog::new(&self.doc.editor));
            }
            Some(model_panel::ModelAction::Shading) => self.model_panel.shading = true,
            Some(model_panel::ModelAction::TextureSize(side)) => {
                self.palette_confirm = Some(palette_ui::Confirm::TextureSize(side));
            }
            _ => {}
        }
    }

    /// El panel Library del dock y lo que pide.
    fn library_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let mut action = None;
        let skin = self.pixel.skin();
        let editor = &mut self.doc.editor;
        let state = &mut self.doc.library;
        dock::flyout(
            ctx,
            dock::DockPanel::Library,
            anchor,
            skin,
            library_panel::WIDTH,
            true,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    action = library_panel::show(ui, editor, skin, state);
                });
            },
        );
        let Some(action) = action else {
            return;
        };
        let editor = &mut self.doc.editor;
        match action {
            library_panel::LibraryAction::Rename(id, name) => {
                editor.rename_artboard(id, name);
            }
            library_panel::LibraryAction::Duplicate(id) => {
                let Some(source) = editor.document.artboard(id) else {
                    return;
                };
                let mut copy = source.clone();
                copy.id = editor.document.next_artboard_id();
                copy.name = format!("{} copy", source.name);
                // Una fuente lleva su nombre también en su info: la copia es de otra familia
                // ("Mine copy Bold").
                if let Some(font) = &mut copy.font {
                    font.name = format!("{} copy", font.name);
                    copy.name = font.full_name();
                }
                let new = copy.id;
                let index = editor.document.artboards.len();
                if editor.execute_doc(cs_commands::DocCommand::AddArtboard {
                    index,
                    artboard: Box::new(copy),
                }) {
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
                if let Some(cmd) = cs_commands::symbol::place_on_canvas(&editor.document, id)
                    && editor.execute_doc(cmd)
                {
                    editor.activate(id);
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

    /// El panel Dafont del dock, junto a su botón (`anchor`). La fuente elegida queda en la
    /// herramienta de texto, a su tamaño y nítida.
    #[cfg(not(target_arch = "wasm32"))]
    fn dafont_panel(&mut self, ctx: &egui::Context, anchor: dock::Anchor, modal: bool) {
        let mut picked = None;
        let dafont = &mut self.dafont;
        dock::flyout(
            ctx,
            dock::DockPanel::Dafont,
            anchor,
            self.pixel.skin(),
            dafont::WIDTH,
            true,
            |ui| {
                ui.add_enabled_ui(!modal, |ui| picked = dafont.ui(ui));
            },
        );
        if let Some(picked) = picked {
            let style = &mut self.doc.editor.text_style;
            style.font = picked.font;
            style.size = picked.size_px.unwrap_or(16);
            style.antialias = false;
        }
    }

    fn update_font_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.new_font else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let (name, cell, mode) = (dialog.name.trim().to_owned(), dialog.cell, dialog.mode);
                let name = if name.is_empty() {
                    "New Font".to_owned()
                } else {
                    name
                };
                self.new_font = None;
                // Desde Home no hay pestaña: se abre un documento nuevo como File → New.
                if !self.has_doc() {
                    self.new_document();
                }
                self.create_font(ctx, &name, cell, mode);
            }
            tilemap_dialog::DialogResult::Cancel => self.new_font = None,
        }
    }

    /// Crea la fuente `name` en el documento abierto, abre su panel en el dock y la encuadra al
    /// lado de él. `false` si no se pudo.
    pub(crate) fn create_font(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        cell: (u32, u32),
        mode: cs_core::FontMode,
    ) -> bool {
        if !self.doc.editor.new_font(name, cell, mode) {
            return false;
        }
        if let Some(id) = self.doc.editor.workspace() {
            self.tabs.open_view(&mut self.doc, id);
        }
        self.show_font_beside_panel(ctx);
        true
    }

    /// Abre el panel Font (si estaba cerrado) y encuadra el glifo al lado de él: lo que hace
    /// falta al entrar al workspace de una fuente.
    fn show_font_beside_panel(&mut self, ctx: &egui::Context) {
        dock::set_open(ctx, Some(dock::DockPanel::Font));
        self.doc
            .canvas
            .request_zoom(view::ZoomRequest::FitActiveBeside {
                right_inset: dock::flyout_reach(font_panel::WIDTH),
            });
    }

    /// Encuadra el modelo al entrar a su pestaña. Su panel 3D va fijo al lado del workspace (ya
    /// fuera del lienzo): el panel abierto del dock, el del lienzo, no cambia.
    fn show_model_beside_panel(&mut self) {
        self.doc
            .canvas
            .request_zoom(view::ZoomRequest::FitActiveBeside { right_inset: 0.0 });
    }

    /// El panel del workspace en el que se está: el de la fuente o el del modelo.
    fn show_workspace_panel(&mut self, ctx: &egui::Context) {
        if self.doc.editor.is_font() {
            self.show_font_beside_panel(ctx);
        } else if self.doc.editor.workspace_model().is_some() {
            self.show_model_beside_panel();
        }
    }

    /// Después de elegir la pestaña `tab` de la barra: la de la vista de un modelo o una fuente
    /// abre su panel.
    fn after_tab_action(&mut self, ctx: &egui::Context, tab: tabs::TabRef) {
        if matches!(tab, tabs::TabRef::View(..)) && self.doc.editor.workspace().is_some() {
            self.show_workspace_panel(ctx);
        }
    }

    fn update_model_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.new_model else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let (name, size) = (dialog.name.trim().to_owned(), dialog.size);
                let name = if name.is_empty() {
                    "Model".to_owned()
                } else {
                    name
                };
                self.new_model = None;
                // Desde Home no hay pestaña: se abre un documento nuevo como File → New.
                if !self.has_doc() {
                    self.new_document();
                }
                self.create_model(&name, size);
            }
            tilemap_dialog::DialogResult::Cancel => self.new_model = None,
        }
    }

    /// Bake…: al aceptar, hornea el modelo con el que se abrió (aunque ya no sea el activo; si
    /// ya no existe como modelo, el diálogo se cierra) en un artboard nuevo.
    fn update_bake_dialog(&mut self, ctx: &egui::Context) {
        let still_model = self.has_doc()
            && self.bake.as_ref().is_none_or(|d| {
                self.doc
                    .editor
                    .document
                    .artboard(d.model())
                    .is_some_and(|a| a.model.is_some())
            });
        if !still_model {
            self.bake = None;
            return;
        }
        let Some(dialog) = &mut self.bake else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin(), &self.doc.editor.document) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let (model, options) = (dialog.model(), dialog.chosen());
                self.bake = None;
                self.finish_bake(model, options);
            }
            tilemap_dialog::DialogResult::Cancel => self.bake = None,
        }
    }

    /// GIF settings…: al aceptar, arma el archivo del modelo con el que se abrió (si ya no existe
    /// como modelo, el diálogo se cierra) y pregunta dónde guardarlo (en web, lo descarga). Si no
    /// se elige dónde, el diálogo sigue abierto.
    fn update_gif_dialog(&mut self, ctx: &egui::Context) {
        let still_model = self.has_doc()
            && self.gif.as_ref().is_none_or(|d| {
                self.doc
                    .editor
                    .document
                    .artboard(d.model())
                    .is_some_and(|a| a.model.is_some())
            });
        if !still_model {
            self.gif = None;
            return;
        }
        let Some(dialog) = &mut self.gif else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin(), &self.doc.editor.document) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let bytes = match dialog.build(&self.doc.editor.document) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        // El diálogo sigue abierto: se puede bajar la escala o los frames.
                        return self.show_message("Could not export", e);
                    }
                };
                let (name, ext) = (dialog.file_name(), dialog.format.extension());
                let filters = [platform::FileFilter {
                    name: if ext == "gif" { "GIF" } else { "PNG" },
                    extensions: if ext == "gif" { &["gif"] } else { &["png"] },
                }];
                let Some(target) = self.platform.choose_save(&name, &filters) else {
                    return;
                };
                self.gif = None;
                if let Err(e) = self.platform.write(&target, &bytes) {
                    self.show_message("Could not export", e);
                }
            }
            tilemap_dialog::DialogResult::Cancel => self.gif = None,
        }
    }

    /// La ventana Shading del panel 3D: se cierra con la × o cuando ya no hay modelo que editar.
    pub(crate) fn update_shading_window(&mut self, ctx: &egui::Context, modal: bool) {
        if !self.model_panel.shading {
            return;
        }
        if !self.has_doc() || self.doc.editor.edited_model().is_none() {
            self.model_panel.shading = false;
            return;
        }
        let skin = self.pixel.skin();
        if model_panel::shading_window(ctx, &mut self.doc.editor, skin, modal) {
            self.model_panel.shading = false;
        }
    }

    /// Hornea `model` y muestra el resultado: el artboard horneado queda activo en el lienzo del
    /// documento (se sale de la pestaña del modelo, si se horneó desde ella).
    pub(crate) fn finish_bake(
        &mut self,
        model: cs_core::ArtboardId,
        options: cs_render::BakeOptions,
    ) {
        if !self.doc.editor.bake_artboard(model, options) {
            return;
        }
        let Some(baked) = self.doc.editor.document.artboards.last().map(|a| a.id) else {
            return;
        };
        self.tabs.show_doc(&mut self.doc);
        self.doc.editor.activate(baked);
        self.doc
            .canvas
            .request_zoom(view::ZoomRequest::CenterActive);
    }

    /// Crea el modelo 3D `name` en el documento abierto, abre su pestaña (con el panel 3D) y lo
    /// encuadra. `false` si no se pudo.
    pub(crate) fn create_model(&mut self, name: &str, size: (u32, u32)) -> bool {
        if !self.doc.editor.new_model(name, size) {
            return false;
        }
        if let Some(id) = self.doc.editor.workspace() {
            self.tabs.open_view(&mut self.doc, id);
        }
        self.show_model_beside_panel();
        true
    }

    fn update_tilemap_dialogs(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.new_tilemap = None;
            self.convert_tilemap = None;
            return;
        }
        if let Some(dialog) = &mut self.new_tilemap {
            match dialog.show(ctx, self.pixel.skin()) {
                tilemap_dialog::DialogResult::Open => {}
                tilemap_dialog::DialogResult::Accept => {
                    let choice = dialog.choice();
                    self.new_tilemap = None;
                    if !self.doc.editor.add_tilemap_layer(&choice) {
                        self.show_message(
                            "New Tilemap Layer",
                            "Could not create the tilemap layer: check the tileset and the \
                             tile size (1-256 px per side).",
                        );
                    }
                }
                tilemap_dialog::DialogResult::Cancel => self.new_tilemap = None,
            }
        }
        if let Some(dialog) = &mut self.convert_tilemap {
            match dialog.show(ctx, self.pixel.skin()) {
                tilemap_dialog::DialogResult::Open => {}
                tilemap_dialog::DialogResult::Accept => {
                    let size = dialog.tile_size;
                    self.convert_tilemap = None;
                    if !self.doc.editor.convert_to_tilemap(size) {
                        self.show_message(
                            "Convert to Tilemap",
                            format!(
                                "Could not convert the layer: it would need more than {} \
                                 different tiles, or a cel would exceed the tilemap size limit. \
                                 Try a larger tile size.",
                                cs_core::MAX_TILESET_TILES
                            ),
                        );
                    }
                }
                tilemap_dialog::DialogResult::Cancel => self.convert_tilemap = None,
            }
        }
    }

    /// F8: abre el diálogo de Convert to Symbol o, si no se puede convertir, dice por qué.
    pub(crate) fn open_convert_to_symbol(&mut self) {
        match self.doc.editor.can_convert_to_symbol() {
            Err(e) => self.show_message("Convert to Symbol", e.to_string()),
            Ok(()) => {
                self.symbol_dialog = Some(symbol_dialog::SymbolDialog::new(&self.doc.editor));
            }
        }
    }

    fn update_symbol_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.symbol_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.symbol_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let (name, timing) = (dialog.name.clone(), dialog.timing);
                self.symbol_dialog = None;
                if let Err(e) = self.doc.editor.convert_to_symbol(&name, timing) {
                    self.show_message("Convert to Symbol", e.to_string());
                }
            }
            tilemap_dialog::DialogResult::Cancel => self.symbol_dialog = None,
        }
    }

    /// Lo pedido desde el menú "≡" de un artboard en el lienzo: los diálogos del menú Sprite.
    fn artboard_request(&mut self) {
        use canvas::ArtboardRequest;
        use sprite_dialogs::{PropertiesDialog, SpriteDialog, SpriteSizeDialog};
        let Some(request) = self.doc.canvas.artboard_request.take() else {
            return;
        };
        if !self.has_doc() {
            return;
        }
        match request {
            ArtboardRequest::Properties => {
                self.open_sprite_dialog(|a| SpriteDialog::Properties(PropertiesDialog::new(a)));
            }
            ArtboardRequest::SpriteSize => {
                self.open_sprite_dialog(|a| SpriteDialog::Size(SpriteSizeDialog::new(a)));
            }
            ArtboardRequest::CanvasSize => self.open_canvas_size(),
            ArtboardRequest::ExpandSheet => self.open_expand_sheet(),
            ArtboardRequest::SliceFrames => self.open_slice_frames(),
        }
    }

    /// La ventana Slice Properties (pedida desde el menú del slice en el lienzo).
    fn show_slice_properties(&mut self, ctx: &egui::Context) {
        if !self.has_doc() || !self.doc.canvas.slice_properties {
            return;
        }
        let editor = &mut self.doc.editor;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            self.pixel.skin(),
            "slice_properties_window",
            "Slice Properties",
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| panels::slice_properties(ui, editor),
        );
        if closed {
            self.doc.canvas.slice_properties = false;
        }
    }

    fn update_grid_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.grid_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            view::GridDialogResult::Open => {}
            view::GridDialogResult::Accept => {
                let [r, g, b, a] = dialog.color.to_srgba_unmultiplied();
                self.view.grid_color = [r, g, b, a];
                self.view.grid = true;
                if self.tabs.active().is_some()
                    && let Some(cmd) = dialog.command(&self.doc.editor)
                {
                    self.doc.editor.execute_doc(cmd);
                }
                self.grid_dialog = None;
            }
            view::GridDialogResult::Cancel => self.grid_dialog = None,
        }
    }

    fn toggle_fullscreen(&mut self, ctx: &egui::Context) {
        self.fullscreen = !self.fullscreen;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
    }

    /// Las ventanas Keyboard Shortcuts y Preferences.
    fn update_settings_windows(&mut self, ctx: &egui::Context) {
        let skin = self.pixel.skin();
        if let Some(window) = &mut self.shortcuts
            && keyboard_shortcuts::show(ctx, window, &mut self.keymap, skin)
        {
            self.shortcuts = None;
        }
        if self.preferences {
            let out = preferences::show(ctx, &mut self.view, skin);
            if out.closed {
                self.preferences = false;
            }
            if out.grid_settings && self.has_doc() {
                self.preferences = false;
                self.grid_dialog = Some(view::GridDialog::new(&self.doc.editor, &self.view));
            }
        }
    }

    /// El diálogo de un filtro: OK deja la vista previa; cancelar la quita (lo hace el diálogo).
    fn update_filter_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.filter_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.filter_dialog else {
            return;
        };
        let skin = self.pixel.skin();
        let result = dialog.show(ctx, &mut self.doc.editor, skin);
        if let Some(outline) = dialog.committed_outline() {
            self.last_outline = Some(outline);
        }
        if !matches!(result, tilemap_dialog::DialogResult::Open) {
            self.filter_dialog = None;
        }
    }

    /// Avanza Play in Place con el reloj de egui (`stable_dt` en reposo es el estimado, no el
    /// real) y pide repintar justo para el próximo cambio de frame.
    fn tick_play_in_place(&mut self, ctx: &egui::Context) {
        if !self.has_doc() || !self.doc.editor.any_playing_in_place() {
            self.play_in_place_time = None;
            return;
        }
        let now = ctx.input(|i| i.time);
        let dt_ms = self
            .play_in_place_time
            .map_or(0.0, |t| (now - t) * 1000.0)
            .max(0.0);
        self.play_in_place_time = Some(now);
        if let (Some(ms), _) = self.doc.editor.tick_play_in_place(dt_ms) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(ms.max(1.0) / 1000.0));
        }
    }

    fn update_sheet_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.sheet_dialog else {
            return;
        };
        match dialog.show(ctx, self.pixel.skin()) {
            tilemap_dialog::DialogResult::Open => {}
            tilemap_dialog::DialogResult::Accept => {
                let name = dialog.name.clone();
                let slice = dialog.purpose != sheet_dialog::SheetPurpose::Import;
                match dialog.sprite() {
                    Some(sprite) => {
                        self.sheet_dialog = None;
                        if slice {
                            self.add_sliced_artboard(&name, sprite);
                        } else {
                            self.open_sheet_sprite(&name, sprite);
                        }
                    }
                    None => self.show_message(
                        "Nothing to import",
                        "The frame size does not fit in the image.",
                    ),
                }
            }
            tilemap_dialog::DialogResult::Cancel => self.sheet_dialog = None,
        }
    }

    fn update_frame_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.frame_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.frame_dialog else {
            return;
        };
        let skin = self.pixel.skin();
        if !matches!(
            dialog.show(ctx, &mut self.doc.editor, skin),
            tilemap_dialog::DialogResult::Open
        ) {
            self.frame_dialog = None;
        }
    }

    fn update_select_dialog(&mut self, ctx: &egui::Context) {
        if !self.has_doc() {
            self.select_dialog = None;
            return;
        }
        let Some(dialog) = &mut self.select_dialog else {
            return;
        };
        let skin = self.pixel.skin();
        if !matches!(
            dialog.show(ctx, &mut self.doc.editor, skin),
            tilemap_dialog::DialogResult::Open
        ) {
            self.select_dialog = None;
        }
    }

    /// Abre el diálogo de `filter` (con sus valores de entrada). Outline vuelve con los últimos
    /// valores que se dejaron (con el color de `filter`) y Tiled como View → Tiled Mode.
    fn open_filter(&mut self, filter: filters::Filter) {
        let mut target = filters::FilterTarget::default();
        let filter = match filter {
            filters::Filter::Outline(fx) => {
                let mut fx = match self.last_outline {
                    Some((last, last_target)) => {
                        target = last_target;
                        filters::OutlineFx {
                            color: fx.color,
                            ..last
                        }
                    }
                    None => fx,
                };
                let tiled = self.doc.editor.tiled;
                filter_dialog::set_tiled(&mut fx, tiled != editor::TiledMode::None, tiled);
                filters::Filter::Outline(fx)
            }
            other => other,
        };
        self.filter_dialog = Some(filter_dialog::FilterDialog::new(filter).with_target(target));
    }

    /// Barra de pestañas: Home + documentos.
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut switch_to = None;
        let mut close = None;
        let mut home = false;
        let bar = self.tabs.bar(&self.doc);
        let shown = self.tabs.bar_active(&self.doc);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if ui.selectable_label(shown.is_none(), "🏠 Home").clicked() {
                home = true;
            }
            for (i, tab) in self.tabs.bar_infos(&self.doc).into_iter().enumerate() {
                let active = shown == Some(i);
                let icon = match tab.icon {
                    tabs::TabIcon::None => "",
                    tabs::TabIcon::Model => "▣ ",
                    tabs::TabIcon::Font => "Aa ",
                    tabs::TabIcon::Piece(_) => "",
                };
                let label = format!("{icon}{}{}", tab.name, if tab.dirty { " •" } else { "" });
                ui.add_space(4.0);
                let response = ui.selectable_label(active, label);
                if response.clicked() {
                    switch_to = Some(i);
                }
                if response.middle_clicked() {
                    close = Some(i);
                }
                if ui.small_button("×").tip("Close").clicked() {
                    close = Some(i);
                }
            }
        });
        if home {
            self.go_home();
        }
        if let Some(i) = switch_to {
            self.tab_action(bar[i], false);
            self.after_tab_action(ui.ctx(), bar[i]);
        }
        if let Some(i) = close {
            self.tab_action(bar[i], true);
        }
    }

    /// Los paneles de la ventana, en el orden en que egui les reparte el espacio: el primero que
    /// se declara se queda con el borde. Devuelve el rect del lienzo (`None` en Home).
    fn layout(&mut self, ui: &mut egui::Ui, modal: bool) -> Option<egui::Rect> {
        // Las pestañas de vista de lo que ya no existe (o ya no es modelo ni fuente) se van.
        self.tabs.sync_views(&mut self.doc);
        // Entrar al workspace de un modelo desde el lienzo (doble clic en una instancia, Edit
        // Symbol) también abre su pestaña de vista y su panel.
        if let Some(id) = self.doc.editor.take_view_request() {
            self.tabs.open_view(&mut self.doc, id);
            self.show_workspace_panel(ui.ctx());
        }
        // Con Pixel UI, como en el mockup: el menú de 16 de alto y las pestañas de 23, sin línea
        // entre los dos; la barra de pestañas termina en un separador grabado (en vez de la línea
        // de egui). Detrás, contra el borde derecho, el degradé del final de la barra.
        let window = ui.max_rect();
        let fade = self.pixel.skin().and_then(|skin| {
            let screen = ui.max_rect();
            let bottom = screen.top() + tab_bar::FADE_BOTTOM;
            tab_bar::fade_shapes(skin, screen, bottom, ui.ctx().pixels_per_point())
        });
        let paint_fade = |ui: &egui::Ui| {
            if let Some(fade) = &fade {
                ui.painter().extend(fade.iter().cloned());
            }
        };
        let pixel = self.pixel.skin().is_some();
        if !pixel {
            // Sin Pixel UI no hay caja de los colores: ninguna pestaña queda editando el
            // secundario para cuando se vuelva a prender.
            for doc in self.tabs.docs_mut(&mut self.doc) {
                doc.editor.editing_secondary = false;
            }
        }
        let mut menu = egui::Panel::top("menu").show_separator_line(!pixel);
        if pixel {
            // Sin margen abajo y con `MENU_TEXT_TOP` arriba: las mayúsculas quedan en la fila
            // `MENU_CAPS_TOP`, con lugar arriba para la caja del título abierto.
            let frame = egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin {
                left: tab_bar::MENU_LEFT,
                right: 8,
                top: tab_bar::MENU_TEXT_TOP,
                bottom: 0,
            });
            menu = menu.exact_size(tab_bar::MENU_HEIGHT).frame(frame);
        }
        menu.show(ui, |ui| {
            paint_fade(ui);
            ui.add_enabled_ui(!modal, |ui| self.menu_bar(ui));
        });
        if let Some(skin) = self.pixel.skin() {
            let infos = self.tabs.bar_infos(&self.doc);
            let active = self.tabs.bar_active(&self.doc);
            let action = egui::Panel::top("tabs")
                .exact_size(tab_bar::TABS_HEIGHT)
                .frame(egui::Frame::side_top_panel(ui.style()).inner_margin(0))
                .show_separator_line(false)
                .show(ui, |ui| {
                    paint_fade(ui);
                    ui.add_enabled_ui(!modal, |ui| {
                        tab_bar::pixel_tab_bar(ui, skin, &infos, active)
                    })
                    .inner
                })
                .inner;
            // El mono que corre, sobre el degradé: arriba de todo y a 5 del borde derecho, como en
            // el diseño (frame 4).
            let ctx = ui.ctx().clone();
            if let Some(runner) = self.runner.get_or_insert_with(|| logo::Logo::running(&ctx)) {
                let at = egui::pos2(
                    window.right() - RUNNER_RIGHT - runner.size().x,
                    window.top(),
                );
                runner.paint_at(ui, at.round());
            }
            match action {
                Some(tab_bar::TabAction::Home) => self.go_home(),
                Some(tab_bar::TabAction::Switch(i)) => {
                    let tab = self.tabs.bar(&self.doc)[i];
                    self.tab_action(tab, false);
                    self.after_tab_action(&ctx, tab);
                }
                Some(tab_bar::TabAction::Close(i)) => {
                    self.tab_action(self.tabs.bar(&self.doc)[i], true)
                }
                None => {}
            }
        } else {
            egui::Panel::top("tabs").show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.tab_bar(ui));
            });
        }
        // Antes que los laterales: la barra de estado va a todo el ancho.
        // Con Pixel UI, la del diseño: 20 de alto, sin el margen ni la línea de egui.
        let skin = self.pixel.skin();
        let mut status = egui::Panel::bottom("status");
        if let Some(skin) = skin {
            status = status
                .exact_size(pixel_status::HEIGHT)
                .frame(egui::Frame::NONE.fill(skin.colors().background))
                .show_separator_line(false);
        }
        {
            let shown = status.show(ui, |ui| {
                let notice = self
                    .autosaver
                    .disabled_reason
                    .as_ref()
                    .map(|r| format!("Autosave off: {r}"));
                if self.has_doc() {
                    match skin {
                        Some(skin) => panels::pixel_status_bar(
                            ui,
                            &self.doc.editor,
                            &self.doc.canvas,
                            notice.as_deref(),
                            skin,
                        ),
                        None => panels::status_bar(
                            ui,
                            &self.doc.editor,
                            &self.doc.canvas,
                            notice.as_deref(),
                        ),
                    }
                } else {
                    // En Home, la versión (con Pixel UI, en la barra del diseño).
                    let version = format!("ChimpSprite {}", env!("CARGO_PKG_VERSION"));
                    match skin {
                        Some(skin) => pixel_status::show(
                            ui,
                            skin,
                            &[pixel_status::Section {
                                icon: None,
                                text: version,
                                color: None,
                                min_text: None,
                            }],
                        ),
                        None => {
                            ui.label(version);
                        }
                    }
                }
            });
            // Hasta dónde pintan los paneles de arriba (`editor_panels`).
            ui.data_mut(|d| d.insert_temp(status_top_id(), shown.response.rect.top()));
        }
        if self.has_doc() {
            let rect = self.editor_panels(ui, modal);
            if let Some(why) = self.doc.canvas.drop_error.take() {
                self.show_message("Library", why);
            }
            Some(rect)
        } else {
            // Con Pixel UI, el Home del diseño (frame 5) sobre el fondo del skin.
            let frame = self
                .pixel
                .skin()
                .map(|skin| egui::Frame::NONE.fill(skin.colors().background));
            let mut panel = egui::CentralPanel::default();
            if let Some(frame) = frame {
                panel = panel.frame(frame);
            }
            // Las páginas (el UI Catalog) van sobre el mismo fondo (`background`, #ddcebf), como
            // pidió el diseñador.
            let page = self.tabs.page();
            panel.show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| match (page, self.pixel.skin()) {
                    (Some(tabs::Page::Catalog), Some(skin)) => kit::catalog::show(ui, skin),
                    (Some(tabs::Page::Catalog), None) => {
                        ui.label("The UI Catalog shows the Pixel UI components: turn on View → Pixel UI.");
                    }
                    (None, Some(_)) => self.pixel_home(ui),
                    (None, None) => self.home(ui),
                });
            });
            None
        }
    }

    /// Paneles del editor (cuando hay un documento activo). Devuelve el rect del lienzo.
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) -> egui::Rect {
        let skin = self.pixel.skin();
        // Nada de lo de arriba pinta sobre la barra de estado: si el contenido de un panel no
        // entra en una ventana baja, egui lo alarga hacia abajo (y con él su borde y el resalte
        // de redimensionar).
        if let Some(top) = ui.data(|d| d.get_temp::<f32>(status_top_id())) {
            ui.set_clip_rect(ui.clip_rect().with_max_y(top));
        }
        // La barra de opciones, a todo el ancho debajo de las pestañas: 21 de alto con Pixel UI,
        // con el separador grabado abajo. En la pestaña de un modelo (ChimpCAD) no va: su alto
        // es del área de abajo (en modo Texture, el área la pone debajo de su encabezado).
        if self.doc.editor.model_edit_on() {
            panels::text_window(ui.ctx(), &mut self.doc.editor, skin);
        } else {
            panels::options_bar_panel(ui.style(), skin).show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    panels::tool_options_bar(ui, &mut self.doc.editor, skin)
                });
            });
        }
        // La toolbar antes que la línea de tiempo: va a todo el alto, hasta la barra de estado, y
        // la línea de tiempo empieza a su derecha.
        // Alrededor de los artboards: el color `canvas` del skin con Pixel UI.
        self.doc.canvas.background = canvas::background_for(skin);
        // Con Pixel UI, las reglas del mockup.
        self.doc.canvas.pixel_rulers = skin.map(pixel_rulers::PixelRulers::from_skin);
        // Con Pixel UI, cada artboard es una ventana del skin.
        self.doc.canvas.artboard_window = skin.map(artboard_window::ArtboardWindow::from_skin);
        // Con Pixel UI, los cursores y la selección del diseño.
        self.doc.canvas.pixel_ui = skin.is_some();
        self.doc.canvas.cursors = skin.map(pixel_cursors::CursorSet::from_skin);
        self.doc.canvas.symmetry_color = skin.map(|s| s.colors().symmetry);
        let (width, frame) = match skin {
            // Con Pixel UI, la columna de íconos: la caja más el margen a cada lado, sin línea
            // contra el lienzo (el mockup no la tiene). Arriba, el margen del mockup hasta la
            // barra de opciones.
            Some(skin) => {
                let margin = egui::Margin {
                    left: pixel_ui::TOOLBAR_MARGIN,
                    right: pixel_ui::TOOLBAR_MARGIN,
                    top: pixel_ui::TOOLBAR_TOP_MARGIN,
                    bottom: 2,
                };
                (
                    pixel_ui::toolbar_width(skin),
                    egui::Frame::side_top_panel(ui.style()).inner_margin(margin),
                )
            }
            // Sin Pixel UI, la barra con nombres.
            None => (184.0, egui::Frame::side_top_panel(ui.style())),
        };
        egui::Panel::left("tools")
            .resizable(false)
            .exact_size(width)
            .frame(frame)
            .show_separator_line(skin.is_none())
            .show(ui, |ui| {
                // Con la UI a 2× o 3× y la ventana baja, la barra no cabe: scroll vertical.
                panels::tools_scroll_area().show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| panels::toolbar(ui, &mut self.doc.editor, skin));
                });
            });
        // El panel derecho antes que la línea de tiempo: va a todo el alto, hasta la barra de
        // estado, y la línea de tiempo termina a su izquierda.
        // Lo pedido desde el panel se resuelve después: el panel tiene prestado el skin.
        let mut palette_action = None;
        // El candado de la paleta es de la app (se guarda con la vista), no de cada pestaña.
        self.doc.editor.palette_locked = self.view.palette_locked;
        let tiles = self.doc.editor.tiles_mode();
        // El ancho guardado de la columna de colores; sin uno, el de fábrica.
        let saved_width = self.view.palette_width.unwrap_or(220.0).max(1.0);
        // Los primeros cuadros egui guarda el ancho del panel con medidas que todavía cambian
        // (la fuente y la escala del skin rigen después): el guardado se impone hasta que el
        // usuario toca algo, y recién entonces se guarda lo que arrastre.
        let palette_id = egui::Id::new("palette");
        if !self.palette_width_settled {
            if ui.input(|i| i.pointer.any_down()) {
                self.palette_width_settled = true;
            } else if let (Some(width), Some(state)) = (
                self.view.palette_width,
                egui::containers::panel::PanelState::load(ui.ctx(), palette_id),
            ) {
                let mut outer_rect = state.outer_rect;
                outer_rect.min.x = outer_rect.max.x - width.max(1.0);
                ui.data_mut(|d| {
                    d.insert_persisted(
                        palette_id,
                        egui::containers::panel::PanelState { outer_rect },
                    )
                });
            }
        }
        let mut panel = egui::Panel::right("palette").default_size(saved_width);
        if let Some(skin) = skin {
            // Con Pixel UI no se achica más que la fila más ancha que no se parte, con los
            // márgenes del panel.
            panel = panel
                .min_size(color_panel::panel_min_size(ui, skin))
                .show_separator_line(false);
        }
        let palette_panel = panel.show(ui, |ui| {
            // Con un diálogo abierto, el menú de paleta no abre otro encima.
            let palette = |ui: &mut egui::Ui, editor: &mut editor::Editor, height: &mut f32| {
                panels::palette(ui, editor, !modal, skin, height).filter(|_| !modal)
            };
            // Con Pixel UI, la paleta y el editor de color van arriba, fuera del scroll: el
            // editor se ve siempre entero (si falta lugar, se achica la caja de la paleta).
            let fixed = skin.is_some() && !tiles;
            if fixed {
                palette_action = palette(ui, &mut self.doc.editor, &mut self.view.palette_height);
            }
            crate::pixel_ui::scroll(ui, egui::ScrollArea::vertical(), |ui| {
                if tiles {
                    panels::tileset(ui, &mut self.doc.editor, &mut self.doc.tile_thumbs);
                    // El selector de color sigue a mano (lo usa 2B).
                    ui.add_space(10.0);
                    panels::color_picker(ui, &mut self.doc.editor, skin);
                } else if !fixed {
                    palette_action =
                        palette(ui, &mut self.doc.editor, &mut self.view.palette_height);
                }
                // Artboards y Slices van en el dock (a la izquierda de este panel).
                ui.add_space(12.0);
                let entered = ui
                    .add_enabled_ui(!modal, |ui| panels::instance(ui, &mut self.doc.editor))
                    .inner;
                if entered {
                    // A un símbolo de la biblioteca: se ve solo él, ajustado; a uno del lienzo,
                    // se centra.
                    let zoom = if self.doc.editor.in_symbol_mode() {
                        view::ZoomRequest::Fit
                    } else {
                        view::ZoomRequest::CenterActive
                    };
                    self.doc.canvas.request_zoom(zoom);
                }
            })
        });
        // Lo que el usuario arrastró se guarda con la vista (el panel ya respetó su mínimo).
        let width = palette_panel.response.rect.width();
        if self.palette_width_settled && self.view.palette_width != Some(width) {
            self.view.palette_width = Some(width);
        }
        // Con Pixel UI, la línea del borde grabada (la oscura y su brillo), como las demás. Arriba,
        // la oscura cruza el brillo de la línea de la barra de opciones (como los separadores de
        // la barra).
        if let Some(skin) = skin {
            let rect = palette_panel.response.rect;
            let ppp = ui.ctx().pixels_per_point();
            let left = rect.left().round();
            ui.painter().add(pixel_ui::etched_vline(
                left,
                rect.y_range(),
                skin.colors(),
                ppp,
            ));
            let cross =
                egui::Rect::from_min_size(egui::pos2(left, rect.top() - 1.0), egui::vec2(1.0, 1.0));
            ui.painter().add(egui::Shape::rect_filled(
                eframe::emath::GuiRounding::round_to_pixels(cross, ppp),
                0.0,
                skin.colors().canvas,
            ));
        }
        self.view.palette_locked = self.doc.editor.palette_locked;
        // El dock de "otros paneles", a la izquierda del derecho y también a todo el alto.
        let mut dock_frame = egui::Frame::side_top_panel(ui.style());
        dock_frame.inner_margin.left = 0;
        dock_frame.inner_margin.right = dock::RIGHT_MARGIN;
        // En la pestaña de un modelo (ChimpCAD) no va la columna del dock: en su lugar, el panel
        // 3D fijo, que no se cierra. El panel abierto del dock (el del lienzo) queda como estaba.
        let model_tab = self.doc.editor.model_edit_on();
        let mut model_action = None;
        let dock_out = if model_tab {
            let editor = &mut self.doc.editor;
            let state = &mut self.model_panel;
            egui::Panel::right("model_panel")
                .resizable(false)
                .exact_size(dock::fixed_panel_width(model_panel::WIDTH))
                .frame(dock_frame)
                .show_separator_line(skin.is_none())
                .show(ui, |ui| {
                    dock::fixed_panel(ui, skin, model_panel::WIDTH, |ui| {
                        ui.add_enabled_ui(!modal, |ui| {
                            model_action = model_panel::show(ui, editor, skin, state);
                        });
                    });
                });
            dock::DockOutput::default()
        } else {
            egui::Panel::right("dock")
                .resizable(false)
                .exact_size(dock::WIDTH)
                .frame(dock_frame)
                .show_separator_line(skin.is_none())
                .show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| {
                        dock::show(
                            ui,
                            &mut self.doc.editor,
                            &mut self.doc.artboard_name,
                            skin,
                            &mut self.view.timeline,
                            &mut self.view.preview,
                        )
                    })
                    .inner
                })
                .inner
        };
        // Un artboard de fuente o modelo no tiene timeline: sus frames son los glifos. La preferencia queda igual para los sprites.
        // Tampoco en la pestaña de un modelo en modo Texture (el activo es su textura).
        if self.view.timeline
            && !self.doc.editor.is_structure_locked()
            && !self.doc.editor.model_edit_on()
        {
            let mut panel = egui::Panel::bottom("timeline")
                .resizable(true)
                .default_size(190.0);
            // Con Pixel UI, la caja del diseño sobre el fondo del skin, sin la línea de egui.
            if let Some(skin) = skin {
                panel = panel
                    .frame(timeline::pixel_panel_frame(skin))
                    .show_separator_line(false);
            }
            panel.show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    timeline::show(ui, &mut self.doc.editor, &mut self.doc.timeline, skin)
                });
            });
        }
        if std::mem::take(&mut self.doc.timeline.new_tilemap) {
            self.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
        }
        // Después de los paneles: lo pedido desde el derecho necesita `self` entero.
        if let Some(action) = palette_action {
            self.palette_action(ui.ctx(), action);
        }
        if dock_out.canvas_size {
            self.open_canvas_size();
        }
        self.model_action(model_action);
        let canvas_texture = self.doc.canvas.plain_texture(&self.doc.editor);
        // En la pestaña de un modelo no está su botón (no hay dock): la ventana se esconde sin
        // apagarse y vuelve con el lienzo.
        if self.view.preview
            && !model_tab
            && self.preview.show(
                ui.ctx(),
                &self.doc.editor,
                self.pixel.skin(),
                canvas_texture,
            )
        {
            self.view.preview = false;
        }
        if self.undo_history
            && undo_history::show(ui.ctx(), &mut self.doc.editor, self.pixel.skin())
        {
            self.undo_history = false;
        }
        // Los paneles del dock que son de la app.
        match dock_out.app_panel {
            Some((dock::DockPanel::Lospec, anchor)) => self.lospec_panel(ui.ctx(), anchor, modal),
            #[cfg(not(target_arch = "wasm32"))]
            Some((dock::DockPanel::Dafont, anchor)) => self.dafont_panel(ui.ctx(), anchor, modal),
            Some((dock::DockPanel::Groom, anchor)) => self.groom_panel(ui.ctx(), anchor, modal),
            Some((dock::DockPanel::Lumen, anchor)) => self.lumen_panel(ui.ctx(), anchor, modal),
            Some((dock::DockPanel::Library, anchor)) => self.library_panel(ui.ctx(), anchor, modal),
            Some((dock::DockPanel::Font, anchor)) => self.font_panel(ui.ctx(), anchor, modal),
            Some((dock::DockPanel::Model, anchor)) => self.model_panel(ui.ctx(), anchor, modal),
            _ => {}
        }
        // En la pestaña del modelo, el de Lospec queda abierto para cuando se vuelva al lienzo.
        if !model_tab && !matches!(dock_out.app_panel, Some((dock::DockPanel::Lospec, _))) {
            self.close_lospec_panel();
        }
        // Otra vez: el panel derecho necesita `self` entero.
        let Some(skin) = self.pixel.skin() else {
            return egui::CentralPanel::no_frame()
                .show(ui, |ui| {
                    self.doc.canvas.show(
                        ui,
                        &mut self.doc.editor,
                        self.canvas_size.as_mut(),
                        &self.view,
                    )
                })
                .response
                .rect;
        };
        // Con Pixel UI, como en el mockup: el lienzo y sus reglas en una caja, con 5 px de fondo
        // alrededor (a la izquierda, los de la barra de íconos; a la derecha, 3 hasta el dock).
        let margin = pixel_ui::WORKSPACE_MARGIN;
        let frame = egui::Frame::new()
            .fill(skin.colors().background)
            .inner_margin(egui::Margin {
                left: 0,
                right: dock::LEFT_AIR,
                top: margin,
                bottom: margin,
            });
        let doc = &mut self.doc;
        let (dialog, view) = (self.canvas_size.as_mut(), &self.view);
        egui::CentralPanel::default()
            .frame(frame)
            .show(ui, |ui| {
                let inner = pixel_ui::workspace_box(ui, skin, ui.max_rect());
                ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
                    doc.canvas.show(ui, &mut doc.editor, dialog, view)
                });
                inner
            })
            .inner
    }
}

/// Un sprite RGBA con `bitmap` en su única capa (lo que sale de Groom o de Lumen).
fn bitmap_sprite(bitmap: &cs_io::export::Bitmap) -> cs_core::Sprite {
    let (w, h) = (bitmap.width, bitmap.height);
    let mut sprite = cs_core::Sprite::new(w, h, cs_core::ColorMode::Rgba);
    let image =
        cs_core::Image::from_buffer(w, h, cs_core::PixelBuffer::Rgba(bitmap.pixels.clone()))
            .expect("same size");
    let id = cs_core::ImageId(1);
    sprite.images.insert(id, std::sync::Arc::new(image));
    let layer = sprite.layers[0].id;
    sprite.cels.insert(
        (layer, 0),
        cs_core::Cel {
            position: (0, 0),
            opacity: 255,
            image: id,
        },
    );
    sprite
}

fn new_editor(palette: cs_core::Palette) -> Editor {
    let mut sprite = Sprite::new(NEW_SPRITE_SIZE, NEW_SPRITE_SIZE, ColorMode::Rgba);
    sprite.palette = palette;
    Editor::with_sprite(sprite)
}

impl eframe::App for ChimpApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // Pixel UI: fuente, escala entera, estilo y skin (con su recarga en vivo).
        self.pixel.update(&ctx, &self.view);
        // Los atajos de este frame, para los menús y los tooltips.
        keymap::publish(&ctx, &self.keymap);
        self.handle_close_request(&ctx);
        self.handle_dropped_files(&ctx);
        self.handle_platform_events(&ctx);
        self.demo_tiles();
        if self.has_doc() {
            // Lo flotante se confirma o cancela si la capa, el frame o la herramienta cambiaron.
            self.doc.editor.sync_floating();
        }
        self.handle_shortcuts(&ctx);
        if self.has_doc() && self.doc.editor.playing {
            let dt_ms = ctx.input(|i| i.stable_dt) as f64 * 1000.0;
            self.doc.editor.tick(dt_ms);
            ctx.request_repaint();
        }
        self.tick_play_in_place(&ctx);

        let modal = self.is_modal();
        self.layout(ui, modal);
        self.update_canvas_size(&ctx);
        self.update_sprite_dialog(&ctx);
        self.update_filter_dialog(&ctx);
        self.update_select_dialog(&ctx);
        self.update_frame_dialog(&ctx);
        self.update_sheet_dialog(&ctx);
        self.update_expand_dialog(&ctx);
        self.update_settings_windows(&ctx);
        self.update_palette_windows(&ctx, modal);
        self.update_grid_dialog(&ctx);
        self.show_slice_properties(&ctx);
        self.artboard_request();
        // F8 pedido desde el menú de una capa o de la selección del lienzo.
        if std::mem::take(&mut self.doc.timeline.convert_symbol)
            | std::mem::take(&mut self.doc.canvas.symbol_request)
        {
            self.open_convert_to_symbol();
        }
        self.update_tilemap_dialogs(&ctx);
        self.update_font_dialog(&ctx);
        self.update_build_font_dialog(&ctx);
        self.update_import_font_dialog(&ctx);
        self.update_glyph_sheet_dialog(&ctx);
        self.update_model_dialog(&ctx);
        self.update_bake_dialog(&ctx);
        self.update_gif_dialog(&ctx);
        self.update_shading_window(&ctx, modal);
        self.update_symbol_dialog(&ctx);
        self.show_dialogs(&ctx);
        self.show_restore(&ctx);
        self.autosave(&ctx);
        self.update_title(&ctx);
        platform::set_unsaved(self.any_dirty());

        self.capture.update(&ctx);
        sound::end_frame(
            &ctx,
            self.view.ui_sounds,
            self.view.sound_pack,
            self.view.sound_volume,
        );
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, RECENT_KEY, &self.recent);
        eframe::set_value(storage, VIEW_KEY, &self.view);
        eframe::set_value(storage, TOOL_OPTIONS_KEY, &self.current_tool_options());
        eframe::set_value(storage, CUSTOM_BRUSH_KEY, &cs_raster::custom_brush());
        eframe::set_value(storage, KEYMAP_KEY, &self.keymap.settings());
        eframe::set_value(
            storage,
            DEFAULT_PALETTE_KEY,
            &self.palettes.stored_default(),
        );
    }
}

/// En el modo Model del workspace de un modelo, Ctrl+E extruye y Ctrl+D divide las caras
/// elegidas: ganan sobre Export y Deselect (que aquí no aplica), aunque no haya nada que extruir
/// o dividir. Devuelve si la usó (si no, va al registro de atajos).
pub(crate) fn cad_command_key(editor: &mut Editor, key: Key) -> bool {
    if !editor.pixel_actions_blocked() || editor.cad_mode() != editor::CadMode::Model {
        return false;
    }
    match key {
        Key::E => editor.request_extrude(),
        Key::D => {
            editor.subdivide_model_selection();
        }
        _ => return false,
    }
    true
}

/// Con Space apretado y una capa tilemap activa, como en Aseprite: Tab alterna Pixels y Tiles, y
/// 1, 2 y 3 eligen el modo de tileset Manual, Auto y Stack. Devuelve si usó la tecla.
pub(crate) fn tilemap_space_key(editor: &mut Editor, key: Key) -> bool {
    use cs_commands::tiles::TilesetMode;
    if editor.tile_space().is_none() {
        return false;
    }
    let mode = match key {
        Key::Tab => {
            let pixels = editor.tilemap_pixels();
            editor.set_tilemap_pixels(!pixels);
            return true;
        }
        Key::Num1 => TilesetMode::Manual,
        Key::Num2 => TilesetMode::Auto,
        Key::Num3 => TilesetMode::Stack,
        _ => return false,
    };
    editor.options.tileset_mode = mode;
    true
}

/// Las teclas sin Ctrl ni Alt que no se reasignan porque dependen de lo que pasa: Enter confirma
/// lo que está a medias o flotando, Esc cancela, las flechas mueven lo seleccionado, Delete
/// borra y, en modo Tiles, X e Y voltean el tile del pincel. Devuelve si la usó (si no, va al
/// registro de atajos).
pub(crate) fn editor_key(editor: &mut Editor, key: Key, shift: bool) -> bool {
    // Con Move y objetos seleccionados, Delete los borra y (sin marquesina) las flechas los
    // corren 1 px (10 con Shift).
    let step = if shift { 10 } else { 1 };
    let palette = editor.palette_range().is_some_and(|r| r.end() > r.start());
    let objects = editor.tool == ToolKind::Move && !editor.selected_objects().is_empty();
    let nudge_objects = objects && editor.selection.is_none() && !editor.can_transform();
    // En el workspace del modelo (siempre en modo Edit): 1–4 cambian qué se elige, G y S
    // arrancan un gesto que sigue al puntero, Enter lo confirma, Esc lo cancela (o vacía la
    // selección) y las flechas mueven un paso de cuadrícula (no cambian de frame ni de capa).
    // Con Shift, G y S siguen con sus otros atajos. W, E y R eligen el gizmo (Move, Rotate,
    // Scale; la misma otra vez lo apaga). Home encuadra el modelo. En modo Model, M une lo elegido
    // y Supr / Retroceso lo borran (Ctrl+E y Ctrl+D, en `cad_command_key`). En modo Texture las
    // teclas son las de siempre, sobre la textura (Home, sobre el render de arriba, lo ve el
    // lienzo).
    if editor.pixel_actions_blocked() {
        use cs_3d::SelectMode;
        // En modo Anim: reproducir, borrar el bloque, mover el tiempo (ver `anim_key`).
        if editor.cad_mode() == editor::CadMode::Anim && editor.anim_key(key, shift) {
            return true;
        }
        let mode = match key {
            _ if shift => None,
            Key::Num1 => Some(SelectMode::Object),
            Key::Num2 => Some(SelectMode::Vertex),
            Key::Num3 => Some(SelectMode::Edge),
            Key::Num4 => Some(SelectMode::Face),
            _ => None,
        };
        if let Some(mode) = mode {
            editor.set_select_mode(mode);
            return true;
        }
        let busy = editor.model_edit().op.is_some();
        let gizmo_drag = matches!(editor.model_edit().op, Some(editor::EditOp::Gizmo { .. }));
        // Un gesto de teclado (G, S o el Extrude) en curso: G y S no lo cambian por otro (un
        // Extrude se perdería).
        let keyboard_op = matches!(
            editor.model_edit().op,
            Some(
                editor::EditOp::Move { keyboard: true, .. }
                    | editor::EditOp::Scale { .. }
                    | editor::EditOp::Extrude { .. }
            )
        );
        // Los gizmos solo existen en modo Model (no en Texture sin textura).
        let model_mode = editor.cad_mode() == editor::CadMode::Model;
        let mut handled = true;
        match key {
            // Durante un arrastre del gizmo, G y S no lo cambian por un gesto de teclado ni
            // W, E y R de gizmo.
            Key::G | Key::S | Key::W | Key::E | Key::R if !shift && gizmo_drag => {}
            Key::G | Key::S if !shift && keyboard_op => {}
            Key::G if !shift => editor.request_model_key_op(editor::KeyOp::Grab),
            Key::S if !shift => editor.request_model_key_op(editor::KeyOp::Scale),
            Key::W | Key::E | Key::R if !shift && !model_mode => {}
            Key::W if !shift => editor.toggle_gizmo(editor::Gizmo::Move),
            Key::E if !shift => editor.toggle_gizmo(editor::Gizmo::Rotate),
            Key::R if !shift => editor.toggle_gizmo(editor::Gizmo::Scale),
            // Unir y borrar no cortan un gesto en curso.
            Key::M | Key::Delete | Key::Backspace if model_mode && busy => {}
            Key::M if !shift && model_mode => {
                editor.merge_model_selection();
            }
            // Con un grupo activo en modo Object, el grupo entero (como el botón Delete).
            Key::Delete | Key::Backspace if model_mode => {
                editor.delete_from_panel();
            }
            Key::Enter if busy => editor.finish_model_op(),
            // Home encuadra todo el modelo (no durante un gesto).
            Key::Home if !busy => {
                editor.frame_model();
            }
            Key::Escape if busy => {
                editor.cancel_model_op();
            }
            Key::Escape => {
                let mode = editor.model_edit().selection.mode;
                editor.set_model_selection(cs_3d::Selection {
                    mode,
                    items: Default::default(),
                });
            }
            // Las flechas no mueven nada durante un gesto.
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if busy => {}
            Key::ArrowLeft => {
                editor.nudge_model_selection(-1, 0);
            }
            Key::ArrowRight => {
                editor.nudge_model_selection(1, 0);
            }
            Key::ArrowUp => {
                editor.nudge_model_selection(0, -1);
            }
            Key::ArrowDown => {
                editor.nudge_model_selection(0, 1);
            }
            _ => handled = false,
        }
        if handled {
            return true;
        }
        // Lo demás (Enter con algo flotando, Delete, X/Y de tiles…) tocaría el sprite escondido
        // del modelo: pasa al registro de atajos, que lo filtra.
        return false;
    }
    match key {
        Key::Enter if editor.is_pending() => editor.finish_gesture(),
        Key::Enter if editor.is_floating() => {
            editor.commit_floating();
        }
        Key::Escape => editor.escape(),
        Key::ArrowLeft if editor.can_transform() => {
            editor.nudge(-1, 0);
        }
        Key::ArrowRight if editor.can_transform() => {
            editor.nudge(1, 0);
        }
        Key::ArrowUp if editor.can_transform() => {
            editor.nudge(0, -1);
        }
        Key::ArrowDown if editor.can_transform() => {
            editor.nudge(0, 1);
        }
        // Con Move en un glifo compuesto, el componente elegido.
        Key::ArrowLeft if editor.can_nudge_component() => {
            editor.nudge_component(-step, 0);
        }
        Key::ArrowRight if editor.can_nudge_component() => {
            editor.nudge_component(step, 0);
        }
        Key::ArrowUp if editor.can_nudge_component() => {
            editor.nudge_component(0, -step);
        }
        Key::ArrowDown if editor.can_nudge_component() => {
            editor.nudge_component(0, step);
        }
        Key::ArrowLeft if nudge_objects => {
            editor.nudge_selected_objects(-step, 0);
        }
        Key::ArrowRight if nudge_objects => {
            editor.nudge_selected_objects(step, 0);
        }
        Key::ArrowUp if nudge_objects => {
            editor.nudge_selected_objects(0, -step);
        }
        Key::ArrowDown if nudge_objects => {
            editor.nudge_selected_objects(0, step);
        }
        Key::X if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_X),
        Key::Y if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_Y),
        // Los objetos seleccionados se borran antes que el rango de cels o la selección (pero
        // con colores de la paleta seleccionados, Delete es para ellos).
        Key::Delete | Key::Backspace if objects && !palette => {
            editor.delete_selected_objects();
        }
        // Con un rango de cels en el timeline y nada seleccionado en el lienzo, Delete los borra.
        Key::Delete | Key::Backspace if editor.has_cel_range() && editor.selection.is_none() => {
            editor.clear_selected_cels()
        }
        // Con colores de la paleta seleccionados, Delete es para ellos (lo hace la paleta).
        Key::Delete | Key::Backspace if !palette => editor.clear_selection_content(),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use cs_core::Mask;

    use super::*;

    #[test]
    fn convert_failure_message_has_no_stray_spaces() {
        assert!(!super::CONVERT_TO_IMAGE_FAILED.contains("  "));
    }

    #[test]
    fn enter_and_escape_apply_floating_content_where_it_was_moved() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        // Arrastra lo seleccionado 2 px a la derecha desde la columna `x`.
        let drag = |e: &mut Editor, x: f32| {
            e.pointer_press((x, 0.5), false, false, 0.1);
            e.pointer_drag((x + 2.0, 0.5));
            e.pointer_release();
        };
        drag(&mut e, 0.5);
        assert!(e.is_floating());
        editor_key(&mut e, Key::Escape, false);
        assert!(!e.is_floating(), "Esc applies");
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(
            bounds,
            Some(((2, 0), (3, 1))),
            "the selection stays where it was moved"
        );
        assert!(e.history.can_undo());
        drag(&mut e, 2.5);
        editor_key(&mut e, Key::Enter, false);
        assert!(!e.is_floating(), "Enter applies");
        assert!(!e.playing, "instead of playing the animation");
        assert!(e.history.can_undo());
        editor_key(&mut e, Key::Escape, false);
        assert!(e.selection.is_none(), "with nothing else, Esc deselects");
    }
    #[test]
    fn arrows_move_the_selection_one_pixel() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let red = cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0));
        let layer = e.layer;
        e.execute(cs_commands::Command::DrawPixels {
            layer,
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (1, 1))));
        editor_key(&mut e, Key::ArrowRight, false);
        editor_key(&mut e, Key::ArrowDown, false);
        editor_key(&mut e, Key::ArrowDown, false);
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((2, 3), (2, 3))));
        assert_eq!(e.frame, 0, "not the next frame");
        editor_key(&mut e, Key::Enter, false);
        assert_eq!(e.sprite().pixel(layer, 0, 2, 3), red, "the pixels moved");
        assert_ne!(e.sprite().pixel(layer, 0, 1, 1), red);
        // Sin selección, las flechas siguen siendo frames y capas.
        e.deselect();
        assert!(!e.can_transform());
    }

    #[test]
    fn enter_with_a_pending_gesture_finishes_the_gesture_first() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.set_tool(ToolKind::PolygonLasso);
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        // Un lazo a medio hacer (fuera de la selección) y, a la vez, algo flotando.
        for p in [(4.5, 4.5), (7.5, 4.5), (7.5, 7.5)] {
            e.pointer_press(p, false, false, 0.1);
            e.pointer_release();
        }
        assert!(e.is_pending());
        assert!(e.lift());
        editor_key(&mut e, Key::Enter, false);
        assert!(!e.is_pending(), "Enter finishes the lasso");
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((4, 4), (7, 7))), "the lasso is the selection");
        assert!(!e.is_floating());
    }
}
