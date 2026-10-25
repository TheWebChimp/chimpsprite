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
mod demo;
#[cfg(test)]
mod dialog_tests;
mod dock;
mod editor;
mod export_dialog;
mod fg_bg;
mod file_actions;
mod mask_ops;
mod menus;
mod palette_ops;
mod palette_ui;
mod slice_ops;
mod tab_bar;
use crate::pixel_ui::Tip as _;
use cs_io::files;
mod guides;
mod home;
mod instance_ops;
#[cfg(test)]
mod layout_tests;
mod logo;
mod lospec_client;
#[cfg(not(target_arch = "wasm32"))]
mod lospec_examples;
mod lospec_tab;
#[cfg(not(target_arch = "wasm32"))]
mod news;
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
mod recent;
pub mod skin;
mod tabs;
mod theme;
mod tile_ops;
mod tilemap_dialog;
mod timeline;
mod tool_group;
#[cfg(test)]
mod tool_tests;
mod transform;
mod transform_box;
mod view;
mod widgets;

use cs_core::{ColorMode, Sprite};
use eframe::egui::{self, Key, KeyboardShortcut, Modifiers};

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

/// Texto que queda en el portapapeles del sistema al copiar píxeles (ver `shortcuts`).
const CLIPBOARD_MARKER: &str = "ChimpSprite pixels";

pub struct ChimpApp {
    /// Documento de la pestaña activa (un marcador vacío si se está en Home).
    doc: tabs::DocState,
    tabs: tabs::Tabs,
    capture: capture::Capture,
    canvas_size: Option<canvas_size::CanvasSizeDialog>,
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
    palettes: palette_library::PaletteLibrary,
    presets_window: Option<palette_ui::PresetsWindow>,
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
            palettes,
            presets_window: None,
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
    fn view_shortcuts(&mut self, ctx: &egui::Context) {
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        // Ctrl+H es borrar hacia atrás en algunos campos de texto: con uno enfocado, le toca a él.
        if !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::H)))
        {
            self.view.extras = !self.view.extras;
        }
        let fit = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Num0);
        if ctx.input_mut(|i| i.consume_shortcut(&fit)) {
            self.doc.canvas.request_zoom(view::ZoomRequest::Fit);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Num0))) {
            self.doc.canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (zoom_in, zoom_out, tab, f11) = ctx.input(|i| {
            let plain = !i.modifiers.command && !i.modifiers.alt;
            (
                plain && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                plain && i.key_pressed(Key::Minus),
                plain && !i.modifiers.shift && i.key_pressed(Key::Tab),
                i.key_pressed(Key::F11),
            )
        });
        if zoom_in {
            self.doc.canvas.request_zoom(view::ZoomRequest::In);
        }
        if zoom_out {
            self.doc.canvas.request_zoom(view::ZoomRequest::Out);
        }
        if tab {
            self.view.timeline = !self.view.timeline;
        }
        if f11 && !cfg!(target_arch = "wasm32") {
            self.toggle_fullscreen(ctx);
        }
    }

    fn view_menu(&mut self, ui: &mut egui::Ui) {
        let v = &mut self.view;
        ui.add(menus::check(&mut v.extras, "Extras"))
            .tip("Ctrl+H: show or hide all extras");
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
            if ui.add(menus::plain("Grid Settings…")).clicked() {
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
                    ui.radio_value(&mut self.view.ui_scale, scale, format!("{scale}×"));
                }
            });
        });
        menus::separator(ui);
        let canvas = &mut self.doc.canvas;
        if ui.add(menus::item("Zoom In", "+")).clicked() {
            canvas.request_zoom(view::ZoomRequest::In);
        }
        if ui.add(menus::item("Zoom Out", "-")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Out);
        }
        if ui.add(menus::item("Actual Size", "Ctrl+0")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ui
            .add(menus::item("Fit to Screen", "Ctrl+Shift+0"))
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
        let shortcut = |m, k| KeyboardShortcut::new(m, k);
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::N))) {
            self.new_document();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::O))) {
            self.open_dialog(ctx);
        }
        let prev_tab = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::Tab);
        if ctx.input_mut(|i| i.consume_shortcut(&prev_tab)) {
            self.tabs.cycle(&mut self.doc, false);
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Tab))) {
            self.tabs.cycle(&mut self.doc, true);
        }
        if !self.has_doc() {
            return;
        }
        self.view_shortcuts(ctx);
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::W)))
            && let Some(i) = self.tabs.active()
        {
            self.request_close_tab(i);
        }
        let save_as = shortcut(Modifiers::COMMAND | Modifiers::SHIFT, Key::S);
        if ctx.input_mut(|i| i.consume_shortcut(&save_as)) {
            self.save_as();
        } else if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S))) {
            self.save();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::E))) {
            self.open_export();
        }
        // Nuevo, abrir, pestañas, cerrar, guardar y exportar (arriba y aquí) no chocan con la
        // edición de texto y valen aun escribiendo. Lo que sigue sí choca: deshacer/rehacer y
        // seleccionar todo son del campo de texto enfocado (si no, tocarían el arte), y el resto
        // (deseleccionar, invertir, recorte) se deja quieto para no actuar a espaldas del campo.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let redo_shift = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        let redo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
        let undo = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        if ctx.input_mut(|i| i.consume_shortcut(&redo_shift) || i.consume_shortcut(&redo)) {
            self.doc.editor.redo();
        } else if ctx.input_mut(|i| i.consume_shortcut(&undo)) {
            self.doc.editor.undo();
        }
        let select_all = KeyboardShortcut::new(Modifiers::COMMAND, Key::A);
        let reselect = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::D);
        let deselect = KeyboardShortcut::new(Modifiers::COMMAND, Key::D);
        let inverse = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::I);
        let clipping = KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::ALT, Key::G);
        if ctx.input_mut(|i| i.consume_shortcut(&clipping)) {
            let layer = self.doc.editor.layer;
            self.doc.editor.toggle_clip(layer);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&select_all)) {
            self.doc.editor.select_all();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&reselect)) {
            self.doc.editor.reselect();
        } else if ctx.input_mut(|i| i.consume_shortcut(&deselect)) {
            self.doc.editor.deselect();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&inverse)) {
            self.doc.editor.invert_selection();
        }
        if self.doc.editor.is_painting() {
            return;
        }
        // egui convierte Ctrl+C/X/V en eventos (no teclas); Paste solo llega si el portapapeles
        // del sistema tiene texto, por eso `copy` deja ahí un marcador.
        let (copy, cut) = ctx.input(|i| {
            let has = |f: fn(&egui::Event) -> bool| i.events.iter().any(f);
            (
                has(|e| matches!(e, egui::Event::Copy)),
                has(|e| matches!(e, egui::Event::Cut)),
            )
        });
        let paste =
            ctx.input(|i| clipboard::paste_request(&i.events, i.modifiers, &mut self.pasted));
        if copy {
            self.copy(ctx);
        } else if cut {
            self.cut(ctx);
        } else if let Some(request) = paste {
            self.paste(request);
        }
        let new_frame = KeyboardShortcut::new(Modifiers::ALT, Key::N);
        if ctx.input_mut(|i| i.consume_shortcut(&new_frame)) {
            self.doc.editor.duplicate_frame();
        }
        let pressed: Vec<(Key, bool)> = ctx.input(|i| {
            if i.modifiers.command || i.modifiers.alt {
                return Vec::new();
            }
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } => Some((*key, modifiers.shift)),
                    _ => None,
                })
                .collect()
        });
        for (key, shift) in pressed {
            let editor = &mut self.doc.editor;
            let letter = key.name().chars().next().filter(|_| key.name().len() == 1);
            if let Some(tool) = letter.and_then(|c| ToolKind::from_key(c, shift)) {
                editor.set_tool(tool);
                continue;
            }
            editor_key(editor, key, shift);
        }
    }

    fn copy(&mut self, ctx: &egui::Context) {
        if let Some(clip) = self.doc.editor.copy() {
            self.clipboard = Some(clip);
            ctx.copy_text(CLIPBOARD_MARKER.into());
        }
    }

    fn cut(&mut self, ctx: &egui::Context) {
        if let Some(clip) = self.doc.editor.cut() {
            self.clipboard = Some(clip);
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
                if ui.add(item("New", "Ctrl+N")).clicked() {
                    self.new_document();
                }
                if ui.add(item("Open…", "Ctrl+O")).clicked() {
                    self.open_dialog(&ctx);
                }
                menus::separator(ui);
                let (save, save_as) = if self.platform.saves_by_download() {
                    ("Save (download)", "Save As… (download)")
                } else {
                    ("Save", "Save As…")
                };
                if ui.add_enabled(has_doc, item(save, "Ctrl+S")).clicked() {
                    self.save();
                }
                if ui
                    .add_enabled(has_doc, item(save_as, "Ctrl+Shift+S"))
                    .clicked()
                {
                    self.save_as();
                }
                let close = item(
                    "Close",
                    if cfg!(target_arch = "wasm32") {
                        ""
                    } else {
                        "Ctrl+W"
                    },
                );
                if ui.add_enabled(has_doc, close).clicked()
                    && let Some(i) = self.tabs.active()
                {
                    self.request_close_tab(i);
                }
                menus::separator(ui);
                if ui
                    .add_enabled(has_doc, menus::plain("Import PNG as Layer…"))
                    .clicked()
                {
                    self.import_layer(&ctx);
                }
                if ui.add_enabled(has_doc, item("Export…", "Ctrl+E")).clicked() {
                    self.open_export();
                }
                menus::separator(ui);
                if ui.add(menus::plain("Quit")).clicked() {
                    self.request_quit(&ctx);
                }
            });
            ui.add_enabled_ui(has_doc, |ui| {
                menus::bar_menu(ui, look, "Sprite", |ui| {
                    if ui.add(menus::plain("Canvas Size…")).clicked() {
                        self.open_canvas_size();
                    }
                });
                menus::bar_menu(ui, look, "View", |ui| self.view_menu(ui));
                menus::bar_menu(ui, look, "Edit", |ui| {
                    let undo = menus::item("Undo", "Ctrl+Z");
                    if ui
                        .add_enabled(self.doc.editor.history.can_undo(), undo)
                        .clicked()
                    {
                        self.doc.editor.undo();
                    }
                    let redo = menus::item("Redo", "Ctrl+Y");
                    if ui
                        .add_enabled(self.doc.editor.history.can_redo(), redo)
                        .clicked()
                    {
                        self.doc.editor.redo();
                    }
                    menus::separator(ui);
                    let has_selection = self.doc.editor.selection.is_some();
                    let cut = menus::item("Cut", "Ctrl+X");
                    if ui.add_enabled(has_selection, cut).clicked() {
                        self.cut(ui.ctx());
                    }
                    let copy = menus::item("Copy", "Ctrl+C");
                    if ui.add_enabled(has_selection, copy).clicked() {
                        self.copy(ui.ctx());
                    }
                    // En escritorio puede haber una imagen de otra app aunque no se haya copiado nada.
                    let can_paste = self.clipboard.is_some() || cfg!(not(target_arch = "wasm32"));
                    let paste = menus::item("Paste", "Ctrl+V");
                    if ui.add_enabled(can_paste, paste).clicked() {
                        self.paste(clipboard::PasteRequest::System {
                            new_artboard: false,
                        });
                    }
                    let as_artboard = menus::item("Paste as New Artboard", "Ctrl+Shift+V");
                    if ui.add_enabled(can_paste, as_artboard).clicked() {
                        self.paste(clipboard::PasteRequest::System { new_artboard: true });
                    }
                    let clear = menus::item("Clear", "Del");
                    if ui.add_enabled(has_selection, clear).clicked() {
                        self.doc.editor.clear_selection_content();
                    }
                    menus::separator(ui);
                    if ui.add(menus::item("Flip Horizontal", "Shift+H")).clicked() {
                        self.doc.editor.flip(true);
                    }
                    if ui.add(menus::item("Flip Vertical", "Shift+V")).clicked() {
                        self.doc.editor.flip(false);
                    }
                });
                menus::bar_menu(ui, look, "Layer", |ui| {
                    if ui.add(menus::plain("New Tilemap Layer…")).clicked() {
                        self.new_tilemap =
                            Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_tilemap = self.doc.editor.can_convert_to_tilemap();
                    if ui
                        .add_enabled(to_tilemap, menus::plain("Convert to Tilemap…"))
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
                menus::bar_menu(ui, look, "Select", |ui| {
                    let editor = &mut self.doc.editor;
                    let has_selection = editor.selection.is_some();
                    if ui.add(menus::item("All", "Ctrl+A")).clicked() {
                        editor.select_all();
                    }
                    let deselect = menus::item("Deselect", "Ctrl+D");
                    if ui.add_enabled(has_selection, deselect).clicked() {
                        editor.deselect();
                    }
                    let reselect = menus::item("Reselect", "Ctrl+Shift+D");
                    if ui
                        .add_enabled(!has_selection && editor.can_reselect(), reselect)
                        .clicked()
                    {
                        editor.reselect();
                    }
                    let inverse = menus::item("Inverse", "Ctrl+Shift+I");
                    if ui.add(inverse).clicked() {
                        editor.invert_selection();
                    }
                });
            });
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
            panel = panel.min_size(color_panel::panel_min_size(ui, skin));
        }
        panel.show(ui, |ui| {
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
            egui::ScrollArea::vertical().show(ui, |ui| {
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
        self.view.palette_locked = self.doc.editor.palette_locked;
        // El dock de "otros paneles", a la izquierda del derecho y también a todo el alto.
        let mut dock_frame = egui::Frame::side_top_panel(ui.style());
        dock_frame.inner_margin.left = 0;
        dock_frame.inner_margin.right = dock::RIGHT_MARGIN;
        let lospec_open = self.lospec_open();
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
                        lospec_open,
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
        if dock_out.toggle_lospec {
            self.toggle_lospec();
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
        self.update_palette_windows(&ctx, modal);
        self.update_grid_dialog(&ctx);
        self.show_slice_properties(&ctx);
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
        eframe::set_value(
            storage,
            DEFAULT_PALETTE_KEY,
            &self.palettes.stored_default(),
        );
    }
}

/// Teclas sin Ctrl/Alt que actúan sobre el editor (las letras de herramienta van aparte).
pub(crate) fn editor_key(editor: &mut Editor, key: Key, shift: bool) {
    match key {
        Key::N if shift => editor.add_layer(cs_core::LayerKind::Image),
        // Como en Aseprite: Shift+H y Shift+V voltean lo seleccionado (o la capa).
        Key::H if shift => {
            editor.flip(true);
        }
        Key::V if shift => {
            editor.flip(false);
        }
        Key::Enter if editor.is_pending() => editor.finish_gesture(),
        Key::Enter if editor.is_floating() => {
            editor.commit_floating();
        }
        Key::Escape => editor.escape(),
        Key::Enter => editor.toggle_play(),
        Key::Comma | Key::ArrowLeft => editor.prev_frame(),
        Key::Period | Key::ArrowRight => editor.next_frame(),
        Key::ArrowUp => editor.step_layer(true),
        Key::ArrowDown => editor.step_layer(false),
        Key::F3 => editor.onion_enabled = !editor.onion_enabled,
        // En modo Tiles no hay color secundario: X e Y voltean el tile del pincel.
        Key::X if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_X),
        Key::Y if editor.tiles_mode() => editor.toggle_brush_flip(cs_core::Tile::FLIP_Y),
        Key::X => editor.swap_colors(),
        Key::Delete | Key::Backspace => editor.clear_selection_content(),
        _ => {}
    }
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
    fn enter_applies_floating_content_and_escape_cancels_it() {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.set_tool(ToolKind::RectSelect);
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        let drag = |e: &mut Editor| {
            e.pointer_press((0.5, 0.5), false, false, 0.1);
            e.pointer_drag((2.5, 0.5));
            e.pointer_release();
        };
        drag(&mut e);
        assert!(e.is_floating());
        editor_key(&mut e, Key::Escape, false);
        assert!(
            !e.is_floating() && e.selection.is_some(),
            "Esc cancels, keeps the selection"
        );
        drag(&mut e);
        editor_key(&mut e, Key::Enter, false);
        assert!(!e.is_floating(), "Enter applies");
        assert!(!e.playing, "instead of playing the animation");
        assert!(e.history.can_undo());
        editor_key(&mut e, Key::Escape, false);
        assert!(e.selection.is_none(), "with nothing else, Esc deselects");
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
