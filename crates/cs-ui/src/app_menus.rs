//! Los menús de la barra de arriba (File, Edit, Sprite, Frame, View y Help).

use crate::*;

impl ChimpApp {
    /// File → Open Recent: los archivos recientes (el que ya no existe se quita de la lista).
    #[cfg(not(target_arch = "wasm32"))]
    fn open_recent_menu(&mut self, ui: &mut egui::Ui) {
        let files = self.recent.files().to_vec();
        ui.add_enabled_ui(!files.is_empty(), |ui| {
            menus::submenu(ui, "Open Recent", |ui| {
                for path in files {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string();
                    if ui
                        .add(menus::plain(&name))
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        if path.exists() {
                            self.platform.open_path(&path, platform::OpenPurpose::Open);
                        } else {
                            self.recent.remove(&path);
                            self.show_message("File not found", path.display().to_string());
                        }
                        ui.close();
                    }
                }
            });
        });
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
        let current = sprite_ops::ColorTarget::of(self.doc.editor.sprite().color_mode);
        menus::submenu(ui, "Color Mode", |ui| {
            for target in sprite_ops::ColorTarget::ALL {
                if ui
                    .add(menus::choice(current == target, target.label()))
                    .clicked()
                {
                    if current != target {
                        self.doc.editor.set_color_mode(target);
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
        menus::submenu(ui, "Tiled Mode", |ui| {
            let editor = &mut self.doc.editor;
            for mode in editor::TiledMode::ALL {
                if ui
                    .add(menus::choice(editor.tiled == mode, mode.label()))
                    .clicked()
                {
                    editor.tiled = mode;
                    editor.revision += 1;
                }
            }
        });
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
        let (a, b) = editor.selected_frames();
        if ui.add(menus::plain("Frame Properties…")).clicked() {
            self.frame_dialog = Some(frame_dialog::FramePropertiesDialog::new(&self.doc.editor));
            ui.close();
            return;
        }
        if ui
            .add_enabled(a != b, menus::plain("Reverse Frames"))
            .on_disabled_hover_text("Select two or more frames")
            .clicked()
        {
            editor.reverse_frames();
        }
        // Cel Properties: la opacidad del cel actual (o de los elegidos).
        let cels = if editor.frame_range.is_some() || editor.has_cel_range() {
            editor.selected_cels()
        } else {
            vec![(editor.layer, editor.frame)]
        };
        timeline::cel_opacity_row(ui, editor, &cels);
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
                open = Some(Filter::ColorCurve {
                    curve: filters::Curve::identity(),
                    channels: Default::default(),
                });
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

    pub(crate) fn menu_bar(&mut self, ui: &mut egui::Ui) {
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
                #[cfg(not(target_arch = "wasm32"))]
                self.open_recent_menu(ui);
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
                if !self.platform.saves_by_download()
                    && ui
                        .add_enabled(has_doc, menus::plain("Save Copy As…"))
                        .clicked()
                {
                    self.save_copy_as();
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
                    let editor = &mut self.doc.editor;
                    let ctx = ui.ctx().clone();
                    let action = |a| keymap::label(&ctx, a);
                    if ui
                        .add(menus::item(
                            "Duplicate",
                            action(keymap::Action::DuplicateLayer),
                        ))
                        .clicked()
                    {
                        editor.duplicate_layer();
                    }
                    let merge = editor.merge_target().is_some();
                    if ui
                        .add_enabled(
                            merge,
                            menus::item("Merge Down", action(keymap::Action::MergeDown)),
                        )
                        .on_disabled_hover_text("Needs an image layer below, in the same group")
                        .clicked()
                    {
                        editor.merge_down();
                    }
                    if ui
                        .add(menus::item("Flatten", action(keymap::Action::Flatten)))
                        .clicked()
                    {
                        editor.flatten();
                    }
                    menus::separator(ui);
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
                    menus::separator(ui);
                    if ui.add(menus::plain("Color Range…")).clicked() {
                        self.select_dialog =
                            Some(select_dialogs::SelectDialog::color_range(&self.doc.editor));
                        ui.close();
                    }
                    ui.add_enabled_ui(has_selection, |ui| {
                        menus::submenu(ui, "Modify", |ui| {
                            for how in select_dialogs::Modify::ALL {
                                if ui.add(menus::plain(&format!("{}…", how.label()))).clicked() {
                                    self.select_dialog =
                                        select_dialogs::SelectDialog::modify(&self.doc.editor, how);
                                    ui.close();
                                }
                            }
                        });
                    });
                });
                menus::bar_menu(ui, look, "View", |ui| self.view_menu(ui));
            });
            menus::bar_menu(ui, look, "Help", |ui| self.help_menu(ui));
        });
    }
}
