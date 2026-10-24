//! Interfaz egui de ChimpSprite: capa delgada sobre el motor.

mod anim_ops;
mod appearance;
mod autosave;
mod canvas;
mod canvas_size;
mod capture;
mod clipboard;
mod demo;
mod editor;
mod export_dialog;
mod file_actions;
mod mask_ops;
mod palette_ops;
mod palette_ui;
mod slice_ops;
mod tab_bar;
use cs_io::files;
mod guides;
mod home;
mod instance_ops;
#[cfg(test)]
mod layout_tests;
mod lospec_client;
#[cfg(not(target_arch = "wasm32"))]
mod lospec_examples;
mod lospec_tab;
mod palette_library;
mod panels;
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
        match canvas_size::show(ctx, dialog) {
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
            match dialog.show(ctx) {
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
            match dialog.show(ctx) {
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

    fn update_grid_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.grid_dialog else {
            return;
        };
        match dialog.show(ctx) {
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
        if ctx.input_mut(|i| i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::H))) {
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
        let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
        let v = &mut self.view;
        ui.add(egui::Checkbox::new(&mut v.extras, "Extras"))
            .on_hover_text("Ctrl+H: show or hide all extras");
        ui.menu_button("Show", |ui| {
            ui.checkbox(&mut v.rulers, "Rulers");
            ui.add_enabled_ui(v.extras, |ui| {
                ui.checkbox(&mut v.guides, "Guides");
                ui.checkbox(&mut v.pixel_grid, "Pixel Grid");
                ui.checkbox(&mut v.grid, "Grid");
                ui.checkbox(&mut v.selection_edges, "Selection Edges");
                ui.checkbox(&mut v.slices, "Slices");
                ui.checkbox(&mut v.brush_preview, "Brush Preview");
                ui.checkbox(&mut v.artboard_names, "Artboard Names");
                ui.checkbox(&mut v.tile_numbers, "Tile Numbers");
            });
        });
        ui.menu_button("Guides", |ui| {
            ui.checkbox(&mut self.view.guides, "Show Guides");
            ui.checkbox(&mut self.view.snap_to_guides, "Snap to Guides");
            ui.checkbox(&mut self.view.lock_guides, "Lock Guides");
            ui.separator();
            let has_guides = !self.doc.editor.guides().is_empty();
            if ui
                .add_enabled(has_guides, egui::Button::new("Clear Guides"))
                .clicked()
            {
                self.doc.editor.set_guides(Vec::new());
                ui.close();
            }
            ui.weak("Drag from a ruler to add a guide;\ndrag it back to remove it.");
        });
        ui.menu_button("Grid", |ui| {
            ui.checkbox(&mut self.view.grid, "Show Grid");
            ui.checkbox(&mut self.view.snap_to_grid, "Snap to Grid");
            if ui.button("Grid Settings…").clicked() {
                self.grid_dialog = Some(view::GridDialog::new(&self.doc.editor, &self.view));
                ui.close();
            }
        });
        ui.separator();
        ui.add(egui::Checkbox::new(
            &mut self.doc.editor.onion_enabled,
            "Onion Skin",
        ))
        .on_hover_text("F3");
        ui.add(egui::Checkbox::new(&mut self.view.timeline, "Timeline"))
            .on_hover_text("Tab");
        if !cfg!(target_arch = "wasm32") {
            let mut fullscreen = self.fullscreen;
            if ui
                .add(egui::Checkbox::new(&mut fullscreen, "Full Screen"))
                .on_hover_text("F11")
                .changed()
            {
                self.toggle_fullscreen(ui.ctx());
            }
        }
        ui.separator();
        ui.checkbox(&mut self.view.pixel_ui, "Pixel UI");
        ui.add_enabled_ui(self.view.pixel_ui, |ui| {
            ui.menu_button("UI Scale", |ui| {
                for scale in 1..=3u8 {
                    ui.radio_value(&mut self.view.ui_scale, scale, format!("{scale}×"));
                }
            });
        });
        ui.separator();
        let canvas = &mut self.doc.canvas;
        if ui.add(item("Zoom In", "+")).clicked() {
            canvas.request_zoom(view::ZoomRequest::In);
        }
        if ui.add(item("Zoom Out", "-")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Out);
        }
        if ui.add(item("Actual Size", "Ctrl+0")).clicked() {
            canvas.request_zoom(view::ZoomRequest::Actual);
        }
        if ui.add(item("Fit to Screen", "Ctrl+Shift+0")).clicked() {
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
        if ctx.egui_wants_keyboard_input() || self.doc.editor.is_painting() {
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
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                let ctx = ui.ctx().clone();
                let item = |text: &str, keys: &str| egui::Button::new(text).shortcut_text(keys);
                if ui.add(item("New", "Ctrl+N")).clicked() {
                    self.new_document();
                }
                if ui.add(item("Open…", "Ctrl+O")).clicked() {
                    self.open_dialog(&ctx);
                }
                ui.separator();
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
                ui.separator();
                if ui
                    .add_enabled(has_doc, egui::Button::new("Import PNG as Layer…"))
                    .clicked()
                {
                    self.import_layer(&ctx);
                }
                if ui.add_enabled(has_doc, item("Export…", "Ctrl+E")).clicked() {
                    self.open_export();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.request_quit(&ctx);
                }
            });
            ui.add_enabled_ui(has_doc, |ui| {
                ui.menu_button("Sprite", |ui| {
                    if ui.button("Canvas Size…").clicked() {
                        self.open_canvas_size();
                    }
                });
                ui.menu_button("View", |ui| self.view_menu(ui));
                ui.menu_button("Edit", |ui| {
                    let undo = egui::Button::new("Undo").shortcut_text("Ctrl+Z");
                    if ui
                        .add_enabled(self.doc.editor.history.can_undo(), undo)
                        .clicked()
                    {
                        self.doc.editor.undo();
                    }
                    let redo = egui::Button::new("Redo").shortcut_text("Ctrl+Y");
                    if ui
                        .add_enabled(self.doc.editor.history.can_redo(), redo)
                        .clicked()
                    {
                        self.doc.editor.redo();
                    }
                    ui.separator();
                    let has_selection = self.doc.editor.selection.is_some();
                    let cut = egui::Button::new("Cut").shortcut_text("Ctrl+X");
                    if ui.add_enabled(has_selection, cut).clicked() {
                        self.cut(ui.ctx());
                    }
                    let copy = egui::Button::new("Copy").shortcut_text("Ctrl+C");
                    if ui.add_enabled(has_selection, copy).clicked() {
                        self.copy(ui.ctx());
                    }
                    // En escritorio puede haber una imagen de otra app aunque no se haya copiado nada.
                    let can_paste = self.clipboard.is_some() || cfg!(not(target_arch = "wasm32"));
                    let paste = egui::Button::new("Paste").shortcut_text("Ctrl+V");
                    if ui.add_enabled(can_paste, paste).clicked() {
                        self.paste(clipboard::PasteRequest::System {
                            new_artboard: false,
                        });
                    }
                    let as_artboard =
                        egui::Button::new("Paste as New Artboard").shortcut_text("Ctrl+Shift+V");
                    if ui.add_enabled(can_paste, as_artboard).clicked() {
                        self.paste(clipboard::PasteRequest::System { new_artboard: true });
                    }
                    let clear = egui::Button::new("Clear").shortcut_text("Del");
                    if ui.add_enabled(has_selection, clear).clicked() {
                        self.doc.editor.clear_selection_content();
                    }
                });
                ui.menu_button("Layer", |ui| {
                    if ui.button("New Tilemap Layer…").clicked() {
                        self.new_tilemap =
                            Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_tilemap = self.doc.editor.can_convert_to_tilemap();
                    if ui
                        .add_enabled(to_tilemap, egui::Button::new("Convert to Tilemap…"))
                        .on_disabled_hover_text("Select an image layer")
                        .clicked()
                    {
                        self.convert_tilemap =
                            Some(tilemap_dialog::TileSizeDialog::new(&self.doc.editor));
                        ui.close();
                    }
                    let to_image = self.doc.editor.can_convert_to_image();
                    if ui
                        .add_enabled(to_image, egui::Button::new("Convert to Image Layer"))
                        .on_disabled_hover_text("Select a tilemap layer")
                        .clicked()
                    {
                        if !self.doc.editor.convert_to_image() {
                            self.show_message("Convert to Image Layer", CONVERT_TO_IMAGE_FAILED);
                        }
                        ui.close();
                    }
                    ui.separator();
                    let layer = self.doc.editor.layer;
                    timeline::instance_menu(ui, &mut self.doc.editor);
                    ui.separator();
                    timeline::mask_menu(ui, &mut self.doc.editor, layer);
                });
                ui.menu_button("Select", |ui| {
                    let editor = &mut self.doc.editor;
                    let has_selection = editor.selection.is_some();
                    if ui
                        .add(egui::Button::new("All").shortcut_text("Ctrl+A"))
                        .clicked()
                    {
                        editor.select_all();
                    }
                    let deselect = egui::Button::new("Deselect").shortcut_text("Ctrl+D");
                    if ui.add_enabled(has_selection, deselect).clicked() {
                        editor.deselect();
                    }
                    let reselect = egui::Button::new("Reselect").shortcut_text("Ctrl+Shift+D");
                    if ui
                        .add_enabled(!has_selection && editor.can_reselect(), reselect)
                        .clicked()
                    {
                        editor.reselect();
                    }
                    let inverse = egui::Button::new("Inverse").shortcut_text("Ctrl+Shift+I");
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
                if ui.small_button("×").on_hover_text("Close").clicked() {
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
        // Con Pixel UI, como en el mockup: el menú de 14 de alto y las pestañas de 23, sin línea
        // entre los dos; la barra de pestañas termina en un separador grabado (en vez de la línea
        // de egui). Detrás, contra el borde derecho, el degradé del final de la barra.
        let fade = self.pixel.skin().and_then(|skin| {
            tab_bar::fade_shapes(skin, ui.max_rect(), ui.ctx().pixels_per_point())
        });
        let paint_fade = |ui: &egui::Ui| {
            if let Some(fade) = &fade {
                ui.painter().extend(fade.iter().cloned());
            }
        };
        let pixel = self.pixel.skin().is_some();
        let mut menu = egui::Panel::top("menu").show_separator_line(!pixel);
        if pixel {
            // Sin margen abajo y con `MENU_TEXT_TOP` arriba: el texto queda en las filas del mockup.
            let frame = egui::Frame::side_top_panel(ui.style()).inner_margin(egui::Margin {
                left: 8,
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
        egui::Panel::bottom("status").show(ui, |ui| {
            let notice = self
                .autosaver
                .disabled_reason
                .as_ref()
                .map(|r| format!("Autosave off: {r}"));
            if self.has_doc() {
                panels::status_bar(ui, &self.doc.editor, &self.doc.canvas, notice.as_deref());
            } else {
                ui.label(format!("ChimpSprite {}", env!("CARGO_PKG_VERSION")));
            }
        });
        if self.has_doc() {
            Some(self.editor_panels(ui, modal))
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.home(ui));
            });
            None
        }
    }

    /// Paneles del editor (cuando hay un documento activo). Devuelve el rect del lienzo.
    fn editor_panels(&mut self, ui: &mut egui::Ui, modal: bool) -> egui::Rect {
        let skin = self.pixel.skin();
        // La barra de opciones, a todo el ancho debajo de las pestañas: 19 de alto con Pixel UI,
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
        // La línea de tiempo antes que el panel derecho: pasa por debajo de él, como siempre.
        if self.view.timeline {
            egui::Panel::bottom("timeline")
                .resizable(true)
                .default_size(190.0)
                .show(ui, |ui| {
                    ui.add_enabled_ui(!modal, |ui| {
                        timeline::show(ui, &mut self.doc.editor, &mut self.doc.timeline)
                    });
                });
        }
        if std::mem::take(&mut self.doc.timeline.new_tilemap) {
            self.new_tilemap = Some(tilemap_dialog::NewTilemapDialog::new(&self.doc.editor));
        }
        egui::Panel::right("palette")
            .default_size(220.0)
            .show(ui, |ui| {
                let open = egui::ScrollArea::vertical()
                    .show(ui, |ui| {
                        if self.doc.editor.tiles_mode() {
                            panels::tileset(ui, &mut self.doc.editor, &mut self.doc.tile_thumbs);
                            // El selector de color sigue a mano (lo usa 2B).
                            ui.add_space(10.0);
                            panels::color_picker(ui, &mut self.doc.editor);
                        } else {
                            // Con un diálogo abierto, el menú de paleta no abre otro encima.
                            if let Some(action) = panels::palette(ui, &mut self.doc.editor, !modal)
                                && !modal
                            {
                                self.palette_action(ui.ctx(), action);
                            }
                        }
                        ui.add_space(12.0);
                        let open = ui
                            .add_enabled_ui(!modal, |ui| {
                                panels::artboards(
                                    ui,
                                    &mut self.doc.editor,
                                    &mut self.doc.artboard_name,
                                )
                            })
                            .inner;
                        ui.add_space(12.0);
                        let entered = ui
                            .add_enabled_ui(!modal, |ui| panels::instance(ui, &mut self.doc.editor))
                            .inner;
                        if entered {
                            self.doc
                                .canvas
                                .request_zoom(view::ZoomRequest::CenterActive);
                        }
                        ui.add_enabled_ui(!modal, |ui| panels::slices(ui, &mut self.doc.editor));
                        open
                    })
                    .inner;
                if open {
                    self.open_canvas_size();
                }
            });
        egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                self.doc.canvas.show(
                    ui,
                    &mut self.doc.editor,
                    self.canvas_size.as_mut(),
                    &self.view,
                )
            })
            .response
            .rect
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
