//! Los atajos de teclado y las acciones que disparan (las del keymap y las del portapapeles).

use crate::*;

impl ChimpApp {
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
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
            DuplicateLayer => {
                editor.duplicate_layer();
            }
            MergeDown => {
                editor.merge_down();
            }
            Flatten => {
                editor.flatten();
            }
            ConvertToSymbol => self.open_convert_to_symbol(),
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
    pub(crate) fn copy(&mut self, ctx: &egui::Context) {
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

    pub(crate) fn cut(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn paste(&mut self, request: clipboard::PasteRequest) {
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

    /// Edit → Copy Merged: lo seleccionado con todas las capas visibles.
    pub(crate) fn copy_merged(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn paste_as_new_layer(&mut self) {
        if let Some(clip) = self.paste_source() {
            self.doc.editor.add_layer(cs_core::LayerKind::Image);
            self.doc.editor.paste(&clip);
        }
    }

    /// Edit → New Artboard From Selection: un artboard nuevo con lo seleccionado de la capa.
    pub(crate) fn new_artboard_from_selection(&mut self) {
        if let Some(clip) = self.doc.editor.copy() {
            self.doc.editor.paste_in_new_artboard(&clip);
        }
    }
}
