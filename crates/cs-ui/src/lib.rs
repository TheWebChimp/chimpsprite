//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod anim_ops;
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
mod mask_ops;
mod menus;
mod palette_ops;
mod palette_ui;
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
        // Desarrollo: abre el diálogo con un ancho de ejemplo para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_CANVAS_SIZE").is_some() && app.has_doc() {
            app.open_canvas_size();
            if let Some(d) = &mut app.canvas_size {
                d.set_anchor(0, 1);
                d.set_width(d.width() + 24);
            }
        }
        // Desarrollo: la herramienta de selección (sus opciones en la barra) para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_SELECT").is_some() && app.has_doc() {
            app.doc.editor.set_tool(ToolKind::RectSelect);
        }
        // Desarrollo: cuatro colores de la paleta seleccionados, para capturas.
        // (`a,b` elige de la a a la b.)
        if let Ok(range) = std::env::var("CHIMPSPRITE_DEMO_PALETTE_SELECTION")
            && app.has_doc()
        {
            let mut ends = range.split(',').filter_map(|n| n.trim().parse().ok());
            let a = ends.next().unwrap_or(2);
            app.doc.editor.palette_selection = Some((a, ends.next().unwrap_or(a + 3)));
        }
        // Desarrollo: el diálogo de Replace Color (o, con "hue", el de Hue/Saturation).
        if let Ok(which) = std::env::var("CHIMPSPRITE_DEMO_FILTER")
            && app.has_doc()
        {
            let editor = &app.doc.editor;
            let filter = match which.as_str() {
                "curve" => {
                    let mut curve = filters::Curve::identity();
                    curve.insert(96, 160);
                    filters::Filter::ColorCurve(curve)
                }
                "hue" => filters::Filter::HueSaturation {
                    hue: 90,
                    saturation: 0,
                    lightness: 0,
                },
                _ => filters::Filter::ReplaceColor {
                    from: editor.resolve(editor.primary),
                    to: editor.resolve(editor.secondary),
                    tolerance: 0,
                },
            };
            app.open_filter(filter);
        }
        // Desarrollo: Keyboard Shortcuts o Preferences abiertas, para capturas.
        match std::env::var("CHIMPSPRITE_DEMO_WINDOW").as_deref() {
            Ok("shortcuts") => app.shortcuts = Some(Default::default()),
            Ok("preferences") => app.preferences = true,
            _ => {}
        }
        // Desarrollo: la ventana Undo History con unos pasos, para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_UNDO_HISTORY").is_some() && app.has_doc() {
            let editor = &mut app.doc.editor;
            editor.add_layer(cs_core::LayerKind::Image);
            editor.insert_empty_frame();
            editor.undo();
            app.undo_history = true;
        }
        // Desarrollo: un rango de cels del timeline ("fila,frame,fila,frame"), para capturas.
        if let Ok(range) = std::env::var("CHIMPSPRITE_DEMO_CELS")
            && app.has_doc()
        {
            let n: Vec<usize> = range
                .split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect();
            let order = app.doc.editor.timeline_layers();
            if let [ra, fa, rb, fb] = n[..]
                && let (Some(&a), Some(&b)) = (order.get(ra), order.get(rb))
            {
                app.doc.editor.select_layer(a);
                app.doc.editor.select_cels((a, fa), (b, fb));
            }
        }
        // Desarrollo: una nota con texto, para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_NOTE").is_some() && app.has_doc() {
            // A la derecha del primer artboard (la vista todavía no se acomodó).
            let at = app.doc.editor.document.artboards[0].rect().1;
            if let Some(id) = app.doc.editor.add_note((at.0 + 4, 0)) {
                app.doc
                    .editor
                    .set_note_text(id, "Revisar la animación del mono 🐒".into());
                app.doc.editor.set_note_color(id, cs_core::NoteColor::Blue);
            }
        }
        // Desarrollo: el panel Dafont del dock abierto, para capturas.
        #[cfg(not(target_arch = "wasm32"))]
        if std::env::var_os("CHIMPSPRITE_DEMO_DAFONT").is_some() && app.has_doc() {
            dock::set_open(&cc.egui_ctx, Some(dock::DockPanel::Dafont));
            // Con `detail`, el detalle de la fuente de la app.
            if std::env::var("CHIMPSPRITE_DEMO_DAFONT").as_deref() == Ok("detail")
                && let Ok(font) =
                    cs_tools::text::FontChoice::load("ChimpSprite", appearance::PIXEL_FONT)
            {
                app.dafont.demo_detail(font);
            }
        }
        // Desarrollo: el panel Lospec del dock abierto, para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_LOSPEC").is_some() && app.has_doc() {
            dock::set_open(&cc.egui_ctx, Some(dock::DockPanel::Lospec));
        }
        // Desarrollo: dos grupos (uno colapsado), para capturas de sus carpetitas.
        if std::env::var_os("CHIMPSPRITE_DEMO_GROUP").is_some() && app.has_doc() {
            let editor = &mut app.doc.editor;
            editor.add_layer(cs_core::LayerKind::Group);
            if let Some(id) = editor.sprite().layers.last().map(|l| l.id) {
                editor.toggle_collapsed(id);
            }
            editor.add_layer(cs_core::LayerKind::Group);
        }
        // Desarrollo: abre Sprite Size para capturas.
        if std::env::var_os("CHIMPSPRITE_DEMO_SPRITE_SIZE").is_some() && app.has_doc() {
            app.open_sprite_dialog(|a| {
                sprite_dialogs::SpriteDialog::Size(sprite_dialogs::SpriteSizeDialog::new(a))
            });
        }
        // Desarrollo: abre el diálogo de nueva capa tilemap para capturas (con --demo).
        if std::env::var_os("CHIMPSPRITE_DEMO_NEW_TILEMAP").is_some() && app.has_doc() {
            app.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&app.doc.editor));
        }
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

    /// El menú Sprite, como el de Aseprite.
    fn sprite_menu(&mut self, ui: &mut egui::Ui) {
        use cs_commands::sprite_ops::{self, Rotation};
        use sprite_dialogs::{DuplicateDialog, PropertiesDialog, SpriteDialog, SpriteSizeDialog};
        const NO_TRANSFORM: &str = "Not available with instance or tilemap layers";
        if ui
            .add(menus::item(
                "Properties",
                keymap::label(ui.ctx(), keymap::Action::Properties),
            ))
            .clicked()
        {
            self.open_sprite_dialog(|a| SpriteDialog::Properties(PropertiesDialog::new(a)));
        }
        let indexed = matches!(
            self.doc.editor.sprite().color_mode,
            cs_core::ColorMode::Indexed { .. }
        );
        menus::submenu(ui, "Color Mode", |ui| {
            for (label, to_indexed) in [("RGB Color", false), ("Indexed", true)] {
                if ui
                    .add(menus::choice(indexed == to_indexed, label))
                    .clicked()
                {
                    if indexed != to_indexed {
                        self.doc.editor.set_color_mode(to_indexed);
                    }
                    ui.close();
                }
            }
        });
        menus::separator(ui);
        if ui.add(menus::plain("Duplicate")).clicked() {
            self.open_sprite_dialog(|a| SpriteDialog::Duplicate(DuplicateDialog::new(a)));
        }
        menus::separator(ui);
        let transformable = sprite_ops::can_transform(self.doc.editor.sprite());
        let size = menus::item(
            "Sprite Size",
            keymap::label(ui.ctx(), keymap::Action::SpriteSize),
        );
        if ui
            .add_enabled(transformable, size)
            .on_disabled_hover_text(NO_TRANSFORM)
            .clicked()
        {
            self.open_sprite_dialog(|a| SpriteDialog::Size(SpriteSizeDialog::new(a)));
        }
        if ui
            .add(menus::item(
                "Canvas Size",
                keymap::label(ui.ctx(), keymap::Action::CanvasSize),
            ))
            .clicked()
        {
            self.open_canvas_size();
        }
        ui.add_enabled_ui(transformable, |ui| {
            menus::submenu(ui, "Rotate Canvas", |ui| {
                for (label, rotation) in [
                    ("180°", Rotation::Half),
                    ("90° CW", Rotation::Clockwise),
                    ("90° CCW", Rotation::CounterClockwise),
                ] {
                    if ui.add(menus::plain(label)).clicked() {
                        self.doc.editor.rotate_canvas(rotation);
                        ui.close();
                    }
                }
            })
            .response
            .on_disabled_hover_text(NO_TRANSFORM);
        });
        menus::separator(ui);
        let has_selection = self.doc.editor.selection.is_some();
        if ui
            .add_enabled(has_selection, menus::plain("Crop"))
            .clicked()
        {
            self.doc.editor.crop_to_selection();
        }
        if ui.add(menus::plain("Trim")).clicked() {
            self.doc.editor.trim_canvas();
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

    /// Atajos del menú View (funcionan aunque no haya foco en el lienzo).
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        let v = &mut self.view;
        ui.add(menus::check(&mut v.extras, "Extras")).tip(format!(
            "{}: show or hide all extras",
            keymap::label(ui.ctx(), keymap::Action::Extras)
        ));
        menus::submenu(ui, "Show", |ui| {
            ui.add(menus::check(&mut v.rulers, "Rulers"));
            ui.add_enabled_ui(v.extras, |ui| {
                ui.add(menus::check(&mut v.guides, "Guides"));
                ui.add(menus::check(&mut v.pixel_grid, "Pixel Grid"));
                ui.add(menus::check(&mut v.grid, "Grid"));
                ui.add(menus::check(&mut v.selection_edges, "Selection Edges"));
                ui.add(menus::check(&mut v.slices, "Slices"));
                ui.add(menus::check(&mut v.brush_preview, "Brush Preview"));
                ui.add(menus::check(&mut v.artboard_names, "Artboard Names"));
                ui.add(menus::check(&mut v.tile_numbers, "Tile Numbers"));
            });
        });
        menus::submenu(ui, "Guides", |ui| {
            ui.add(menus::check(&mut self.view.guides, "Show Guides"));
            ui.add(menus::check(
                &mut self.view.snap_to_guides,
                "Snap to Guides",
            ));
            ui.add(menus::check(&mut self.view.lock_guides, "Lock Guides"));
            menus::separator(ui);
            let has_guides = !self.doc.editor.guides().is_empty();
            if ui
                .add_enabled(has_guides, menus::plain("Clear Guides"))
                .clicked()
            {
                self.doc.editor.set_guides(Vec::new());
                ui.close();
            }
            ui.weak("Drag from a ruler to add a guide;\ndrag it back to remove it.");
        });
        menus::submenu(ui, "Grid", |ui| {
            ui.add(menus::check(&mut self.view.grid, "Show Grid"));
            ui.add(menus::check(&mut self.view.snap_to_grid, "Snap to Grid"));
            if ui.add(menus::plain("Grid Settings")).clicked() {
                self.grid_dialog = Some(view::GridDialog::new(&self.doc.editor, &self.view));
                ui.close();
            }
        });
        menus::separator(ui);
        ui.add(menus::check(
            &mut self.doc.editor.onion_enabled,
            "Onion Skin",
        ))
        .tip("F3");
        ui.add(menus::check(&mut self.view.timeline, "Timeline"))
            .tip("Tab");
        if !cfg!(target_arch = "wasm32") {
            let mut fullscreen = self.fullscreen;
            if ui
                .add(menus::check(&mut fullscreen, "Full Screen"))
                .tip("F11")
                .changed()
            {
                self.toggle_fullscreen(ui.ctx());
            }
        }
        menus::separator(ui);
        ui.add(menus::check(&mut self.view.pixel_ui, "Pixel UI"));
        ui.add_enabled_ui(self.view.pixel_ui, |ui| {
            menus::submenu(ui, "UI Scale", |ui| {
                for scale in 1..=3u8 {
                    // El radio de los menús del diseño.
                    let label = format!("{scale}×");
                    if ui
                        .add(menus::choice(self.view.ui_scale == scale, &label))
                        .clicked()
                    {
                        self.view.ui_scale = scale;
                    }
                }
            });
        });
        menus::separator(ui);
        let canvas = &mut self.doc.canvas;
        if ui
            .add(menus::item(
                "Zoom In",
                keymap::label(ui.ctx(), keymap::Action::ZoomIn),
            ))
            .clicked()
        {
            canvas.request_zoom(view::ZoomRequest::In);
        }
        if ui
            .add(menus::item(
                "Zoom Out",
                keymap::label(ui.ctx(), keymap::Action::ZoomOut),
            ))
            .clicked()
        {
            canvas.request_zoom(view::ZoomRequest::Out);
        }
        if ui
            .add(menus::item(
                "Actual Size",
                keymap::label(ui.ctx(), keymap::Action::ActualSize),
            ))
            .clicked()
        {
            canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ui
            .add(menus::item(
                "Fit to Screen",
                keymap::label(ui.ctx(), keymap::Action::FitToScreen),
            ))
            .clicked()
        {
            canvas.request_zoom(view::ZoomRequest::Fit);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Los diálogos son modales: manejan sus propias teclas.
        if self.is_modal() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let painting = self.has_doc() && self.doc.editor.is_painting();
        if self.has_doc() && !typing && !painting {
            self.clipboard_events(ctx);
        }
        let presses: Vec<(Key, Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        for (key, modifiers) in presses {
            // Primero las fijas, que dependen de lo que pasa (confirmar, cancelar, mover lo
            // seleccionado, borrar).
            if self.has_doc()
                && !typing
                && !painting
                && !modifiers.command
                && !modifiers.alt
                && editor_key(&mut self.doc.editor, key, modifiers.shift)
            {
                continue;
            }
            let Some(action) = self.keymap.action_for(key, modifiers) else {
                continue;
            };
            let allowed = match action.gate() {
                keymap::Gate::Always => true,
                keymap::Gate::NotTyping => !typing,
                keymap::Gate::NotPainting => !typing && !painting,
            };
            if !allowed || (action.needs_document() && !self.has_doc()) {
                continue;
            }
            // Que no le llegue también a egui (Tab mueve el foco, por ejemplo).
            ctx.input_mut(|i| i.consume_key(modifiers, key));
            self.run_action(ctx, action);
            if self.is_modal() {
                return;
            }
        }
    }

    /// Hace lo de un atajo.
    fn run_action(&mut self, ctx: &egui::Context, action: keymap::Action) {
        use keymap::Action::*;
        let editor = &mut self.doc.editor;
        match action {
            NewFile => self.new_document(),
            Open => self.open_dialog(ctx),
            Save => {
                self.save();
            }
            SaveAs => {
                self.save_as();
            }
            Export => self.open_export(),
            CloseTab => {
                if let Some(i) = self.tabs.active() {
                    self.request_close_tab(i);
                }
            }
            NextTab => self.tabs.cycle(&mut self.doc, true),
            PreviousTab => self.tabs.cycle(&mut self.doc, false),
            Undo => editor.undo(),
            Redo => editor.redo(),
            Fill => {
                editor.fill_selection();
            }
            Stroke => editor.stroke_selection(),
            FlipHorizontal => {
                editor.flip(true);
            }
            FlipVertical => {
                editor.flip(false);
            }
            Transform => {
                editor.transform_selection();
            }
            NewBrush => {
                editor.new_brush();
            }
            NewArtboardFromSelection => self.new_artboard_from_selection(),
            Invert => {
                editor.invert_colors();
            }
            KeyboardShortcuts => self.shortcuts = Some(Default::default()),
            Preferences => self.preferences = true,
            SelectAll => editor.select_all(),
            Deselect => editor.deselect(),
            Reselect => editor.reselect(),
            Inverse => editor.invert_selection(),
            Properties => self.open_sprite_dialog(|a| {
                sprite_dialogs::SpriteDialog::Properties(sprite_dialogs::PropertiesDialog::new(a))
            }),
            SpriteSize => {
                if cs_commands::sprite_ops::can_transform(editor.sprite()) {
                    self.open_sprite_dialog(|a| {
                        sprite_dialogs::SpriteDialog::Size(sprite_dialogs::SpriteSizeDialog::new(a))
                    });
                }
            }
            CanvasSize => self.open_canvas_size(),
            NewLayer => editor.add_layer(cs_core::LayerKind::Image),
            ClippingMask => {
                let layer = editor.layer;
                editor.toggle_clip(layer);
            }
            LayerAbove => editor.step_layer(true),
            LayerBelow => editor.step_layer(false),
            PlayPause => editor.toggle_play(),
            NextFrame => editor.next_frame(),
            PreviousFrame => editor.prev_frame(),
            NewFrame => editor.duplicate_frame(),
            OnionSkin => editor.onion_enabled = !editor.onion_enabled,
            Extras => self.view.extras = !self.view.extras,
            Timeline => self.view.timeline = !self.view.timeline,
            FullScreen => {
                if !cfg!(target_arch = "wasm32") {
                    self.toggle_fullscreen(ctx);
                }
            }
            ZoomIn => self.doc.canvas.request_zoom(view::ZoomRequest::In),
            ZoomOut => self.doc.canvas.request_zoom(view::ZoomRequest::Out),
            ActualSize => self.doc.canvas.request_zoom(view::ZoomRequest::Actual),
            FitToScreen => self.doc.canvas.request_zoom(view::ZoomRequest::Fit),
            SwapColors => editor.swap_colors(),
            Tool(kind) => editor.set_tool(kind),
        }
    }

    /// Cortar, copiar y pegar: egui los manda como eventos (no teclas); Paste solo llega si el
    /// portapapeles del sistema tiene texto, por eso `copy` deja ahí un marcador.
    fn clipboard_events(&mut self, ctx: &egui::Context) {
        let (copy, cut) = ctx.input(|i| {
            let has = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
            (
                has(|e| matches!(e, egui::Event::Copy)),
                has(|e| matches!(e, egui::Event::Cut)),
            )
        });
        let paste =
            ctx.input(|i| clipboard::paste_request(&i.events, i.modifiers, &mut self.pasted));
        // Ctrl+Shift+C también llega como Copy: es Copy Merged.
        if copy && ctx.input(|i| i.modifiers.shift) {
            self.copy_merged(ctx);
        } else if copy {
            self.copy(ctx);
        } else if cut {
            self.cut(ctx);
        } else if let Some(request) = paste {
            self.paste(request);
        }
    }

    /// Copia lo seleccionado en el lienzo; sin selección, del timeline (frames o cels).
    fn copy(&mut self, ctx: &egui::Context) {
        if self.doc.editor.selection.is_none() {
            if let Some(clip) = self.doc.editor.copy_timeline() {
                self.timeline_clipboard = Some(clip);
                self.paste_timeline = true;
                ctx.copy_text(CLIPBOARD_MARKER.into());
            }
        } else if let Some(clip) = self.doc.editor.copy() {
            self.clipboard = Some(clip);
            self.paste_timeline = false;
            ctx.copy_text(CLIPBOARD_MARKER.into());
        }
    }

    fn cut(&mut self, ctx: &egui::Context) {
        if self.doc.editor.selection.is_none() {
            if let Some(clip) = self.doc.editor.cut_timeline() {
                self.timeline_clipboard = Some(clip);
                self.paste_timeline = true;
                ctx.copy_text(CLIPBOARD_MARKER.into());
            }
        } else if let Some(clip) = self.doc.editor.cut() {
            self.clipboard = Some(clip);
            self.paste_timeline = false;
            ctx.copy_text(CLIPBOARD_MARKER.into());
        }
    }

    /// Pega lo copiado en ChimpSprite o una imagen de otra app (centrada, como en Aseprite).
    fn paste(&mut self, request: clipboard::PasteRequest) {
        use clipboard::PasteRequest;
        let (system, new_artboard) = match request {
            PasteRequest::Internal { new_artboard } => (None, new_artboard),
            PasteRequest::System { new_artboard } => (clipboard::system_image(), new_artboard),
        };
        let editor = &mut self.doc.editor;
        // Lo último copiado fue del timeline (y no hay una imagen de otra app): eso.
        if system.is_none()
            && !new_artboard
            && self.paste_timeline
            && let Some(clip) = &self.timeline_clipboard
        {
            editor.paste_timeline(clip);
            return;
        }
        let clip = match system {
            Some(image) => {
                let sprite = editor.sprite();
                image.centered_in(sprite.width, sprite.height)
            }
            None => match &self.clipboard {
                Some(clip) => clip.clone(),
                None => return,
            },
        };
        if new_artboard {
            editor.paste_in_new_artboard(&clip);
        } else {
            editor.paste(&clip);
        }
    }

    /// El menú Frame, como el de Aseprite.
    fn frame_menu(&mut self, ui: &mut egui::Ui) {
        let editor = &mut self.doc.editor;
        let last = editor.frame_count() - 1;
        if ui
            .add(menus::item(
                "New Frame",
                keymap::label(ui.ctx(), keymap::Action::NewFrame),
            ))
            .clicked()
        {
            editor.duplicate_frame();
        }
        if ui.add(menus::plain("New Empty Frame")).clicked() {
            editor.insert_empty_frame();
        }
        if ui
            .add_enabled(last > 0, menus::plain("Delete Frame"))
            .clicked()
        {
            editor.remove_frame();
        }
        menus::separator(ui);
        if ui.add(menus::plain("New Tag")).clicked() {
            editor.add_tag();
        }
        menus::separator(ui);
        if ui.add(menus::plain("First Frame")).clicked() {
            editor.set_frame(0);
        }
        if ui
            .add(menus::item(
                "Previous Frame",
                keymap::label(ui.ctx(), keymap::Action::PreviousFrame),
            ))
            .clicked()
        {
            editor.prev_frame();
        }
        if ui
            .add(menus::item(
                "Next Frame",
                keymap::label(ui.ctx(), keymap::Action::NextFrame),
            ))
            .clicked()
        {
            editor.next_frame();
        }
        if ui.add(menus::plain("Last Frame")).clicked() {
            editor.set_frame(last);
        }
        let play = if editor.playing {
            "Stop Animation"
        } else {
            "Play Animation"
        };
        if ui
            .add(menus::item(
                play,
                keymap::label(ui.ctx(), keymap::Action::PlayPause),
            ))
            .clicked()
        {
            editor.toggle_play();
        }
        menus::separator(ui);
        ui.add(menus::check(&mut editor.onion_enabled, "Onion Skin"))
            .tip("F3");
    }

    /// El menú Help.
    fn help_menu(&mut self, ui: &mut egui::Ui) {
        let shortcuts = menus::item(
            "Keyboard Shortcuts…",
            keymap::label(ui.ctx(), keymap::Action::KeyboardShortcuts),
        );
        if ui.add(shortcuts).clicked() {
            self.shortcuts = Some(Default::default());
        }
        menus::separator(ui);
        if ui.add(menus::plain("About ChimpSprite")).clicked() {
            let version = format!("ChimpSprite {}", env!("CARGO_PKG_VERSION"));
            self.show_message(
                "About ChimpSprite",
                format!(
                    "{version}
A pixel art editor."
                ),
            );
        }
    }

    /// El menú Edit, como el de Aseprite.
    fn edit_menu(&mut self, ui: &mut egui::Ui) {
        let undo = menus::item("Undo", keymap::label(ui.ctx(), keymap::Action::Undo));
        if ui
            .add_enabled(self.doc.editor.history.can_undo(), undo)
            .clicked()
        {
            self.doc.editor.undo();
        }
        let redo = menus::item("Redo", keymap::label(ui.ctx(), keymap::Action::Redo));
        if ui
            .add_enabled(self.doc.editor.history.can_redo(), redo)
            .clicked()
        {
            self.doc.editor.redo();
        }
        if ui
            .add(menus::check(&mut self.undo_history, "Undo History"))
            .clicked()
        {
            ui.close();
        }
        menus::separator(ui);
        let has_selection = self.doc.editor.selection.is_some();
        // Sin selección, cortan y copian del timeline (frames o cels).
        if ui.add(menus::item("Cut", "Ctrl+X")).clicked() {
            self.cut(ui.ctx());
        }
        if ui.add(menus::item("Copy", "Ctrl+C")).clicked() {
            self.copy(ui.ctx());
        }
        let merged = menus::item("Copy Merged", "Ctrl+Shift+C");
        if ui.add_enabled(has_selection, merged).clicked() {
            self.copy_merged(ui.ctx());
        }
        // En escritorio puede haber una imagen de otra app aunque no se haya copiado nada.
        let can_paste = self.clipboard.is_some()
            || self.timeline_clipboard.is_some()
            || cfg!(not(target_arch = "wasm32"));
        let paste = menus::item("Paste", "Ctrl+V");
        if ui.add_enabled(can_paste, paste).clicked() {
            self.paste(clipboard::PasteRequest::System {
                new_artboard: false,
            });
        }
        ui.add_enabled_ui(can_paste, |ui| {
            menus::submenu(ui, "Paste Special", |ui| {
                let as_artboard = menus::item("Paste as New Artboard", "Ctrl+Shift+V");
                if ui.add(as_artboard).clicked() {
                    self.paste(clipboard::PasteRequest::System { new_artboard: true });
                    ui.close();
                }
                if ui.add(menus::plain("Paste as New Layer")).clicked() {
                    self.paste_as_new_layer();
                    ui.close();
                }
            });
        });
        let delete = menus::item("Delete", "Del");
        if ui.add_enabled(has_selection, delete).clicked() {
            self.doc.editor.clear_selection_content();
        }
        menus::separator(ui);
        if ui
            .add_enabled(
                has_selection,
                menus::item("Fill", keymap::label(ui.ctx(), keymap::Action::Fill)),
            )
            .clicked()
        {
            self.doc.editor.fill_selection();
        }
        if ui
            .add_enabled(
                has_selection,
                menus::item("Stroke", keymap::label(ui.ctx(), keymap::Action::Stroke)),
            )
            .clicked()
        {
            self.doc.editor.stroke_selection();
        }
        menus::separator(ui);
        menus::submenu(ui, "Rotate", |ui| {
            use edit_ops::Turn;
            for (label, turn) in [
                ("180°", Turn::Half),
                ("90° CW", Turn::Clockwise),
                ("90° CCW", Turn::CounterClockwise),
            ] {
                if ui.add(menus::plain(label)).clicked() {
                    self.doc.editor.rotate_content(turn);
                    ui.close();
                }
            }
        });
        if ui
            .add(menus::item(
                "Flip Horizontal",
                keymap::label(ui.ctx(), keymap::Action::FlipHorizontal),
            ))
            .clicked()
        {
            self.doc.editor.flip(true);
        }
        if ui
            .add(menus::item(
                "Flip Vertical",
                keymap::label(ui.ctx(), keymap::Action::FlipVertical),
            ))
            .clicked()
        {
            self.doc.editor.flip(false);
        }
        if ui
            .add(menus::item(
                "Transform",
                keymap::label(ui.ctx(), keymap::Action::Transform),
            ))
            .clicked()
        {
            self.doc.editor.transform_selection();
        }
        menus::submenu(ui, "Shift", |ui| {
            for (label, dx, dy) in [
                ("Left", -1, 0),
                ("Right", 1, 0),
                ("Up", 0, -1),
                ("Down", 0, 1),
            ] {
                if ui.add(menus::plain(label)).clicked() {
                    self.doc.editor.shift_content(dx, dy);
                    ui.close();
                }
            }
        });
        menus::separator(ui);
        if ui
            .add_enabled(
                has_selection,
                menus::item(
                    "New Brush",
                    keymap::label(ui.ctx(), keymap::Action::NewBrush),
                ),
            )
            .clicked()
        {
            self.doc.editor.new_brush();
        }
        let from_selection = menus::item(
            "New Artboard From Selection",
            keymap::label(ui.ctx(), keymap::Action::NewArtboardFromSelection),
        );
        if ui.add_enabled(has_selection, from_selection).clicked() {
            self.new_artboard_from_selection();
        }
        menus::separator(ui);
        use filters::{Filter, Kernel};
        let editor = &self.doc.editor;
        let (primary, secondary) = (
            editor.resolve(editor.primary),
            editor.resolve(editor.secondary),
        );
        if ui.add(menus::plain("Replace Color…")).clicked() {
            self.open_filter(Filter::ReplaceColor {
                from: primary,
                to: secondary,
                tolerance: 0,
            });
        }
        if ui
            .add(menus::item(
                "Invert",
                keymap::label(ui.ctx(), keymap::Action::Invert),
            ))
            .clicked()
        {
            self.doc.editor.invert_colors();
        }
        let mut open = None;
        menus::submenu(ui, "Adjustments", |ui| {
            if ui.add(menus::plain("Brightness/Contrast…")).clicked() {
                open = Some(Filter::BrightnessContrast {
                    brightness: 0,
                    contrast: 0,
                });
                ui.close();
            }
            if ui.add(menus::plain("Color Curve…")).clicked() {
                open = Some(Filter::ColorCurve(filters::Curve::identity()));
                ui.close();
            }
            if ui.add(menus::plain("Hue/Saturation…")).clicked() {
                open = Some(Filter::HueSaturation {
                    hue: 0,
                    saturation: 0,
                    lightness: 0,
                });
                ui.close();
            }
        });
        menus::submenu(ui, "FX", |ui| {
            if ui.add(menus::plain("Outline…")).clicked() {
                open = Some(Filter::Outline {
                    color: primary,
                    square: false,
                });
                ui.close();
            }
            if ui.add(menus::plain("Convolution Matrix…")).clicked() {
                open = Some(Filter::Convolution(Kernel::Blur));
                ui.close();
            }
            if ui.add(menus::plain("Despeckle…")).clicked() {
                open = Some(Filter::Despeckle { size: 3 });
                ui.close();
            }
        });
        if let Some(filter) = open {
            self.open_filter(filter);
        }
        if ui.add(menus::plain("Insert Text")).clicked() {
            self.doc.editor.set_tool(editor::ToolKind::Text);
        }
        if ui.add(menus::plain("New Note")).clicked() {
            self.new_note();
        }
        menus::separator(ui);
        let shortcuts = menus::item(
            "Keyboard Shortcuts…",
            keymap::label(ui.ctx(), keymap::Action::KeyboardShortcuts),
        );
        if ui.add(shortcuts).clicked() {
            self.shortcuts = Some(Default::default());
        }
        if ui
            .add(menus::item(
                "Preferences…",
                keymap::label(ui.ctx(), keymap::Action::Preferences),
            ))
            .clicked()
        {
            self.preferences = true;
        }
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

    /// Abre el diálogo de `filter` (con sus valores de entrada).
    fn open_filter(&mut self, filter: filters::Filter) {
        self.filter_dialog = Some(filter_dialog::FilterDialog::new(filter));
    }

    /// Edit → Copy Merged: lo seleccionado con todas las capas visibles.
    fn copy_merged(&mut self, ctx: &egui::Context) {
        if let Some(clip) = self.doc.editor.copy_merged() {
            self.clipboard = Some(clip);
            self.paste_timeline = false;
            ctx.copy_text(CLIPBOARD_MARKER.into());
        }
    }

    /// Lo que pegaría Ctrl+V en el lienzo: una imagen de otra app o lo copiado aquí.
    fn paste_source(&self) -> Option<clipboard::Clip> {
        let sprite = self.doc.editor.sprite();
        clipboard::system_image()
            .map(|image| image.centered_in(sprite.width, sprite.height))
            .or_else(|| self.clipboard.clone())
    }

    /// Edit → Paste Special → Paste as New Layer: en una capa nueva, arriba de la actual.
    fn paste_as_new_layer(&mut self) {
        if let Some(clip) = self.paste_source() {
            self.doc.editor.add_layer(cs_core::LayerKind::Image);
            self.doc.editor.paste(&clip);
        }
    }

    /// Edit → New Artboard From Selection: un artboard nuevo con lo seleccionado de la capa.
    fn new_artboard_from_selection(&mut self) {
        if let Some(clip) = self.doc.editor.copy() {
            self.doc.editor.paste_in_new_artboard(&clip);
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let has_doc = self.has_doc();
        // Con Pixel UI, los menús del diseño: el menú empieza `MENU_TEXT_TOP` arriba del
        // contenido del panel.
        let bar_top = ui.max_rect().top() - f32::from(tab_bar::MENU_TEXT_TOP);
        let look = self
            .pixel
            .skin()
            .map(|skin| menus::MenuLook::new(skin, bar_top));
        let look = look.as_ref();
        let mut bar = egui::MenuBar::new();
        // Con Pixel UI, el espaciado de los títulos del mockup (solo la barra; los paneles
        // tienen su estilo en `menus`).
        if let Some(color) = self.pixel.skin().map(|skin| skin.colors().home_text) {
            bar = bar.style(move |style: &mut egui::Style| {
                egui::containers::menu::menu_style(style);
                style.spacing.button_padding.x = tab_bar::MENU_ITEM_PADDING;
                style.spacing.item_spacing.x = tab_bar::MENU_ITEM_GAP;
                style.visuals.override_text_color = Some(color);
            });
        }
        bar.ui(ui, |ui| {
            menus::bar_menu(ui, look, "File", |ui| {
                let ctx = ui.ctx().clone();
                let item = menus::item;
                if ui
                    .add(item("New", keymap::label(&ctx, keymap::Action::NewFile)))
                    .clicked()
                {
                    self.new_document();
                }
                if ui
                    .add(item("Open", keymap::label(&ctx, keymap::Action::Open)))
                    .clicked()
                {
                    self.open_dialog(&ctx);
                }
                menus::separator(ui);
                let (save, save_as) = if self.platform.saves_by_download() {
                    ("Save (download)", "Save As (download)")
                } else {
                    ("Save", "Save As")
                };
                if ui
                    .add_enabled(
                        has_doc,
                        item(save, keymap::label(&ctx, keymap::Action::Save)),
                    )
                    .clicked()
                {
                    self.save();
                }
                if ui
                    .add_enabled(
                        has_doc,
                        item(save_as, keymap::label(&ctx, keymap::Action::SaveAs)),
                    )
                    .clicked()
                {
                    self.save_as();
                }
                let close = item(
                    "Close",
                    if cfg!(target_arch = "wasm32") {
                        String::new()
                    } else {
                        keymap::label(&ctx, keymap::Action::CloseTab)
                    },
                );
                if ui.add_enabled(has_doc, close).clicked()
                    && let Some(i) = self.tabs.active()
                {
                    self.request_close_tab(i);
                }
                menus::separator(ui);
                if ui
                    .add_enabled(has_doc, menus::plain("Import PNG as Layer"))
                    .clicked()
                {
                    self.import_layer(&ctx);
                }
                if ui
                    .add_enabled(
                        has_doc,
                        item("Export", keymap::label(&ctx, keymap::Action::Export)),
                    )
                    .clicked()
                {
                    self.open_export();
                }
                menus::separator(ui);
                if ui.add(menus::plain("Quit")).clicked() {
                    self.request_quit(&ctx);
                }
            });
            ui.add_enabled_ui(has_doc, |ui| {
                menus::bar_menu(ui, look, "Edit", |ui| self.edit_menu(ui));
                menus::bar_menu(ui, look, "Sprite", |ui| self.sprite_menu(ui));
                menus::bar_menu(ui, look, "Layer", |ui| {
                    if ui.add(menus::plain("New Tilemap Layer")).clicked() {
                        self.new_tilemap =
                            Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_tilemap = self.doc.editor.can_convert_to_tilemap();
                    if ui
                        .add_enabled(to_tilemap, menus::plain("Convert to Tilemap"))
                        .on_disabled_hover_text("Select an image layer")
                        .clicked()
                    {
                        self.convert_tilemap =
                            Some(tilemap_dialog::TileSizeDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_image = self.doc.editor.can_convert_to_image();
                    if ui
                        .add_enabled(to_image, menus::plain("Convert to Image Layer"))
                        .on_disabled_hover_text("Select a tilemap layer")
                        .clicked()
                    {
                        if !self.doc.editor.convert_to_image() {
                            self.show_message("Convert to Image Layer", CONVERT_TO_IMAGE_FAILED);
                        }
                        ui.close();
                    }
                    menus::separator(ui);
                    let layer = self.doc.editor.layer;
                    timeline::instance_menu(ui, &mut self.doc.editor);
                    menus::separator(ui);
                    timeline::mask_menu(ui, &mut self.doc.editor, layer);
                });
                menus::bar_menu(ui, look, "Frame", |ui| self.frame_menu(ui));
                menus::bar_menu(ui, look, "Select", |ui| {
                    let editor = &mut self.doc.editor;
                    let has_selection = editor.selection.is_some();
                    if ui
                        .add(menus::item(
                            "All",
                            keymap::label(ui.ctx(), keymap::Action::SelectAll),
                        ))
                        .clicked()
                    {
                        editor.select_all();
                    }
                    let deselect = menus::item(
                        "Deselect",
                        keymap::label(ui.ctx(), keymap::Action::Deselect),
                    );
                    if ui.add_enabled(has_selection, deselect).clicked() {
                        editor.deselect();
                    }
                    let reselect = menus::item(
                        "Reselect",
                        keymap::label(ui.ctx(), keymap::Action::Reselect),
                    );
                    if ui
                        .add_enabled(!has_selection && editor.can_reselect(), reselect)
                        .clicked()
                    {
                        editor.reselect();
                    }
                    let inverse =
                        menus::item("Inverse", keymap::label(ui.ctx(), keymap::Action::Inverse));
                    if ui.add(inverse).clicked() {
                        editor.invert_selection();
                    }
                });
                menus::bar_menu(ui, look, "View", |ui| self.view_menu(ui));
            });
            menus::bar_menu(ui, look, "Help", |ui| self.help_menu(ui));
        });
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
        if self.view.preview
            && self
                .preview
                .show(ui.ctx(), &self.doc.editor, self.pixel.skin())
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
