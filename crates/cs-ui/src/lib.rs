//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod anim_ops;
mod app_actions;
mod app_menus;
mod appearance;
mod artboard_window;
mod autosave;
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
mod export_dialog;
mod fg_bg;
mod file_actions;
mod filter_dialog;
mod filters;
mod frame_dialog;
mod mask_ops;
mod menus;
mod palette_ops;
mod palette_ui;
mod select_dialogs;
mod slice_ops;
mod sprite_dialogs;
mod tab_bar;
use crate::pixel_ui::Tip as _;
use cs_io::files;
mod guides;
mod home;
mod instance_ops;
mod keyboard_shortcuts;
mod keymap;
mod layer_ops;
#[cfg(test)]
mod layout_tests;
mod logo;
mod lospec_client;
#[cfg(not(target_arch = "wasm32"))]
mod lospec_examples;
mod lospec_tab;
#[cfg(not(target_arch = "wasm32"))]
mod news;
mod notes;
mod palette_library;
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
mod tabs;
mod theme;
mod tile_ops;
mod tilemap_dialog;
mod timeline;
mod timeline_clip;
mod tool_group;
#[cfg(test)]
mod tool_tests;
mod transform;
mod transform_box;
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
    /// Select → Color Range… o Modify abierto.
    select_dialog: Option<select_dialogs::SelectDialog>,
    /// Frame → Frame Properties… abierto.
    frame_dialog: Option<frame_dialog::FramePropertiesDialog>,
    /// La ventanita de preview (abierta con `view.preview`).
    preview: preview::Preview,
    convert_tilemap: Option<tilemap_dialog::TileSizeDialog>,
    /// Desarrollo (`CHIMPSPRITE_DEMO_TILES`): al abrir el documento, activar su capa tilemap.
    demo_tiles: bool,
    fullscreen: bool,
    pending: Option<file_actions::Pending>,
    message: Option<file_actions::Message>,
    export: Option<export_dialog::ExportDialog>,
    /// Opciones de la última exportación, para reabrir el diálogo igual.
    last_export: export_dialog::ExportDialog,
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
            doc: tabs::DocState::untitled(""),
            tabs: tabs::Tabs::default(),
            capture: capture::Capture::from_env(),
            canvas_size: None,
            sprite_dialog: None,
            #[cfg(not(target_arch = "wasm32"))]
            dafont: Default::default(),
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
            undo_history: false,
            shortcuts: None,
            keymap: keymap::Keymap::default(),
            preferences: false,
            filter_dialog: None,
            select_dialog: None,
            frame_dialog: None,
            preview: preview::Preview::default(),
            convert_tilemap: None,
            demo_tiles: std::env::var_os("CHIMPSPRITE_DEMO_TILES").is_some(),
            fullscreen: false,
            pending: None,
            message: None,
            export: None,
            last_export: export_dialog::ExportDialog::default(),
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

    /// El panel Dafont del dock, junto a su botón (`anchor`). La fuente elegida queda en la
    /// herramienta de texto, a su tamaño y nítida.
    #[cfg(not(target_arch = "wasm32"))]
    fn dafont_panel(&mut self, ctx: &egui::Context, anchor: egui::Rect, modal: bool) {
        let mut picked = None;
        let dafont = &mut self.dafont;
        dock::flyout(
            ctx,
            dock::DockPanel::Dafont,
            anchor,
            self.pixel.skin(),
            dafont::WIDTH,
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
        if !matches!(
            dialog.show(ctx, &mut self.doc.editor, skin),
            tilemap_dialog::DialogResult::Open
        ) {
            self.filter_dialog = None;
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

    /// Abre el diálogo de `filter` (con sus valores de entrada).
    fn open_filter(&mut self, filter: filters::Filter) {
        self.filter_dialog = Some(filter_dialog::FilterDialog::new(filter));
    }

    /// Barra de pestañas: Home + documentos.
    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut switch_to = None;
        let mut close = None;
        let mut home = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if ui.selectable_label(!self.has_doc(), "🏠 Home").clicked() {
                home = true;
            }
            for (i, tab) in self.tabs.infos(&self.doc).into_iter().enumerate() {
                let active = self.tabs.active() == Some(i);
                let label = format!("{}{}", tab.name, if tab.dirty { " •" } else { "" });
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
            self.tabs.switch(&mut self.doc, i);
        }
        if let Some(i) = close {
            self.request_close_tab(i);
        }
    }

    /// Los paneles de la ventana, en el orden en que egui les reparte el espacio: el primero que
    /// se declara se queda con el borde. Devuelve el rect del lienzo (`None` en Home).
    fn layout(&mut self, ui: &mut egui::Ui, modal: bool) -> Option<egui::Rect> {
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
            let infos = self.tabs.infos(&self.doc);
            let active = self.tabs.active();
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
                Some(tab_bar::TabAction::Switch(i)) => self.tabs.switch(&mut self.doc, i),
                Some(tab_bar::TabAction::Close(i)) => self.request_close_tab(i),
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
            status.show(ui, |ui| {
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
        }
        if self.has_doc() {
            Some(self.editor_panels(ui, modal))
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
            panel.show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| {
                    if self.pixel.skin().is_some() {
                        self.pixel_home(ui);
                    } else {
                        self.home(ui);
                    }
                });
            });
            None
        }
    }

    /// Paneles del editor (cuando hay un documento activo). Devuelve el rect del lienzo.
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) -> egui::Rect {
        let skin = self.pixel.skin();
        // La barra de opciones, a todo el ancho debajo de las pestañas: 21 de alto con Pixel UI,
        // con el separador grabado abajo.
        panels::options_bar_panel(ui.style(), skin).show(ui, |ui| {
            ui.add_enabled_ui(!modal, |ui| {
                panels::tool_options_bar(ui, &mut self.doc.editor, skin)
            });
        });
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
        let mut panel = egui::Panel::right("palette").default_size(220.0);
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
                    self.doc
                        .canvas
                        .request_zoom(view::ZoomRequest::CenterActive);
                }
            })
        });
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
        let dock_out = egui::Panel::right("dock")
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
            .inner;
        if self.view.timeline {
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
        let canvas_texture = self.doc.canvas.plain_texture(&self.doc.editor);
        if self.view.preview
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
            _ => {}
        }
        if !matches!(dock_out.app_panel, Some((dock::DockPanel::Lospec, _))) {
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

        let modal = self.is_modal();
        self.layout(ui, modal);
        self.update_canvas_size(&ctx);
        self.update_sprite_dialog(&ctx);
        self.update_filter_dialog(&ctx);
        self.update_select_dialog(&ctx);
        self.update_frame_dialog(&ctx);
        self.update_settings_windows(&ctx);
        self.update_palette_windows(&ctx, modal);
        self.update_grid_dialog(&ctx);
        self.show_slice_properties(&ctx);
        self.artboard_request();
        self.update_tilemap_dialogs(&ctx);
        self.show_dialogs(&ctx);
        self.show_restore(&ctx);
        self.autosave(&ctx);
        self.update_title(&ctx);
        platform::set_unsaved(self.any_dirty());

        self.capture.update(&ctx);
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

/// Las teclas sin Ctrl ni Alt que no se reasignan porque dependen de lo que pasa: Enter confirma
/// lo que está a medias o flotando, Esc cancela, las flechas mueven lo seleccionado, Delete
/// borra y, en modo Tiles, X e Y voltean el tile del pincel. Devuelve si la usó (si no, va al
/// registro de atajos).
pub(crate) fn editor_key(editor: &mut Editor, key: Key, _shift: bool) -> bool {
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
        Key::X if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_X),
        Key::Y if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_Y),
        // Con un rango de cels en el timeline y nada seleccionado en el lienzo, Delete los borra.
        Key::Delete | Key::Backspace if editor.has_cel_range() && editor.selection.is_none() => {
            editor.clear_selected_cels()
        }
        // Con colores de la paleta seleccionados, Delete es para ellos (lo hace la paleta).
        Key::Delete | Key::Backspace
            if !editor.palette_range().is_some_and(|r| r.end() > r.start()) =>
        {
            editor.clear_selection_content()
        }
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
