//! Los atajos de teclado y las acciones que disparan (las del keymap y las del portapapeles).

use crate::panel_registry::PanelId;
use crate::*;
use cs_commands::ops::Reorder;

impl ChimpApp {
    pub(crate) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // El par elegido que mostró la vista Kerning el frame de antes (si se cerró, ya no).
        let kern_pair = self.font_panel.kern_arrows.take();
        // Los diálogos son modales: manejan sus propias teclas.
        if self.is_modal() {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let painting = self.has_doc() && self.doc.editor.is_painting();
        // En el modo Model del workspace de un modelo el sprite (escondido) no se toca: ni copiar
        // ni pegar. En modo Texture se copia y se pega en la textura.
        let model = self.has_doc() && self.doc.editor.pixel_actions_blocked();
        if self.has_doc() && !typing && !painting && !model {
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
        // Supr con el puntero sobre el outliner del panel 3D borra sus filas elegidas: lo hace
        // el outliner (no lo elegido en las vistas).
        let on_outliner = crate::model_panel::outliner::pointer_over_list(ctx);
        let space = ctx.input(|i| i.key_down(Key::Space));
        for (key, modifiers) in presses {
            // Space+Tab y Space+1/2/3 en una capa tilemap, como en Aseprite.
            if space
                && self.has_doc()
                && !typing
                && !painting
                && !modifiers.command
                && !modifiers.alt
                && tilemap_space_key(&mut self.doc.editor, key)
            {
                ctx.input_mut(|i| i.consume_key(modifiers, key));
                continue;
            }
            if on_outliner && matches!(key, Key::Delete | Key::Backspace) {
                continue;
            }
            // Supr con el puntero sobre una nota la borra (ahí la nota tapa el lienzo).
            if self.has_doc()
                && !typing
                && !painting
                && modifiers.is_none()
                && matches!(key, Key::Delete | Key::Backspace)
                && self.doc.canvas.notes.delete_hovered(&mut self.doc.editor)
            {
                ctx.input_mut(|i| i.consume_key(modifiers, key));
                continue;
            }
            // Con un par elegido en la vista Kerning, ←/→ lo corren 1 px (5 con Shift), un paso
            // de undo cada una, como FontForge (en vez de mover lo seleccionado o cambiar de
            // frame).
            if let Some((a, b)) = kern_pair
                && self.has_doc()
                && !typing
                && !painting
                && !modifiers.command
                && !modifiers.alt
                && matches!(key, Key::ArrowLeft | Key::ArrowRight)
            {
                let step = if modifiers.shift { 5 } else { 1 };
                let delta = if key == Key::ArrowLeft { -step } else { step };
                self.doc.editor.nudge_kerning(a, b, delta);
                ctx.input_mut(|i| i.consume_key(modifiers, key));
                continue;
            }
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
            // Ctrl+E y Ctrl+D del modo Model de ChimpCAD (Extrude y Subdivide).
            if self.has_doc()
                && !typing
                && !painting
                && modifiers.command
                && !modifiers.alt
                && !modifiers.shift
                && cad_command_key(&mut self.doc.editor, key)
            {
                ctx.input_mut(|i| i.consume_key(modifiers, key));
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
            if self.has_doc()
                && self.doc.editor.pixel_actions_blocked()
                && !action.in_model_workspace()
            {
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
    pub(crate) fn run_action(&mut self, ctx: &egui::Context, action: keymap::Action) {
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
                if cs_commands::sprite_ops::can_transform(editor.sprite())
                    && editor.can_resize_sprite()
                {
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
            // Como las flechas y Delete sobre objetos: solo con Move (ver `can_arrange_objects`,
            // que también habilita el menú Layer > Arrange).
            // En el modo Model de ChimpCAD, Duplicate Object duplica los objetos 3D (o el grupo
            // activo del outliner).
            DuplicateObject if editor.pixel_actions_blocked() => {
                editor.duplicate_active();
            }
            BringForward | SendBackward | BringToFront | SendToBack | DuplicateObject
            | DeleteObject
                if !editor.can_arrange_objects() => {}
            BringForward => {
                editor.reorder_selected(Reorder::Forward);
            }
            SendBackward => {
                editor.reorder_selected(Reorder::Backward);
            }
            BringToFront => {
                editor.reorder_selected(Reorder::Front);
            }
            SendToBack => {
                editor.reorder_selected(Reorder::Back);
            }
            DuplicateObject => {
                editor.duplicate_selected();
            }
            DeleteObject => {
                editor.delete_selected_objects();
            }
            PlayPause => editor.toggle_play(),
            NextFrame => editor.next_frame(),
            PreviousFrame => editor.prev_frame(),
            NewFrame => editor.duplicate_frame(),
            OnionSkin => editor.onion_enabled = !editor.onion_enabled,
            Extras => self.view.extras = !self.view.extras,
            // Su hoja sale del árbol del workspace y vuelve a su lugar (`Workspace::sync_timeline`).
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
        // Un link de Coolors o una lista de colores en el portapapeles: es una paleta.
        let palette_text = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Paste(text)
                    if text != CLIPBOARD_MARKER
                        && cs_io::coolors::parse_palette_text(text)
                            .is_some_and(|p| p.is_confident()) =>
                {
                    Some(text.clone())
                }
                _ => None,
            })
        });
        // Ctrl+Shift+C también llega como Copy: es Copy Merged.
        if copy && ctx.input(|i| i.modifiers.shift) {
            self.copy_merged(ctx);
        } else if copy {
            self.copy(ctx);
        } else if cut {
            self.cut(ctx);
        } else if let Some(text) = palette_text {
            self.paste_palette_text(&text);
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

    /// Lo que se ve ahora del timeline y del Preview (va con el acomodo que se deja).
    fn shown(&self) -> workspace::Shown {
        workspace::Shown {
            timeline: self.view.timeline,
            preview: self.view.preview,
        }
    }

    /// Lo que se ve en el acomodo que llega: `workspace::show` lo pone (o lo saca) del árbol.
    fn show_shown(&mut self, shown: workspace::Shown) {
        self.view.timeline = shown.timeline;
        self.view.preview = shown.preview;
    }

    /// Window → Workspace → Reset <nombre>: el de ahora vuelve a su versión de fábrica (o, uno del
    /// usuario, a como se creó). El ancho elegido de la columna derecha se queda (es del usuario,
    /// como lo demás de la vista): sin los paneles acoplados que la ensanchaban, vuelve a él. Es de
    /// la app, no del documento: sin deshacer.
    pub(crate) fn reset_workspace(&mut self) {
        let shown = self.workspaces.reset(&mut self.workspace);
        self.show_shown(shown);
    }

    /// Window → Workspace → <nombre>: el acomodo de ahora se queda con el workspace que se deja y
    /// llega el de `name` como se dejó (o, si nunca se cambió, el de fábrica o como se creó).
    pub(crate) fn switch_workspace(&mut self, name: &str) {
        let shown = self.shown();
        if let Some(shown) = self.workspaces.switch(name, &mut self.workspace, shown) {
            self.show_shown(shown);
        }
    }

    /// Si el panel `id` está abierto (con su ✓ en Window): acoplado o flotando (el timeline y el
    /// Preview ocultos, no); en la pestaña de un modelo, el 3D, que lo muestra su panel fijo. En
    /// una tira, no.
    pub(crate) fn panel_open(&self, id: PanelId) -> bool {
        let w = &self.workspace;
        w.docked(id)
            || w.floating(id)
            || (id == PanelId::Model && self.has_doc() && self.doc.editor.model_edit_on())
    }

    /// Un panel de Window: abierto, se enfoca donde está (su pestaña al frente y, si flota, su
    /// ventana encima de las demás); el timeline o el Preview apagados se prenden (vuelven a su
    /// lugar y ahí se enfocan igual); uno de una tira se acopla como con "Dock" del menú de su
    /// ícono (ver `Workspace::dock_from_strip`: con el foco, la toolbar a un lado del lienzo).
    pub(crate) fn window_panel(&mut self, ctx: &egui::Context, id: PanelId) {
        let w = &self.workspace;
        let placed = w.docked(id) || w.floating(id);
        match id {
            // Vuelven a su lugar en `workspace::show` (al sincronizarse), que ahí los enfoca.
            PanelId::Timeline if !self.view.timeline => {
                self.view.timeline = true;
                self.workspace.pending_focus = Some(id);
            }
            PanelId::Preview if !self.view.preview => {
                self.view.preview = true;
                self.workspace.pending_focus = Some(id);
            }
            // Ya lo muestra el panel fijo de la pestaña del modelo (y no está en el árbol).
            PanelId::Model if self.panel_open(id) && !placed => {}
            _ => {
                if self.workspace.strip_of(id).is_some() {
                    self.workspace.dock_from_strip(id);
                }
                self.workspace.bring_forward(ctx, id);
            }
        }
        // El árbol cambia en el frame siguiente (el timeline y el Preview, al sincronizarse).
        ctx.request_repaint();
    }
}

/// El diálogo de Window → Workspace → New Workspace… o Delete Workspace… (con los componentes del
/// kit, como "Save as Preset").
pub(crate) enum WorkspaceDialog {
    /// El nombre del nuevo (se crea con el acomodo de ahora). `focused`: el foco se pide hasta que
    /// el campo lo tiene una vez (después el usuario puede moverlo).
    New { name: String, focused: bool },
    /// Cuál de los del usuario borrar.
    Delete { name: String },
}

impl ChimpApp {
    /// Abre New Workspace… (sin nombre) o Delete Workspace… (con el primero que se puede borrar).
    pub(crate) fn open_workspace_dialog(&mut self, delete: bool) {
        self.workspace_dialog = Some(if delete {
            let first = self.workspaces.deletable().first().map(|n| n.to_string());
            match first {
                Some(name) => WorkspaceDialog::Delete { name },
                None => return,
            }
        } else {
            WorkspaceDialog::New {
                name: String::new(),
                focused: false,
            }
        });
    }

    /// El diálogo de los workspaces, si está abierto: Save crea uno con el nombre (si no es uno
    /// que ya existe ni está vacío; si choca, dice por qué) y lo deja como el de ahora; Delete
    /// borra el elegido. Enter acepta en el campo; Escape o la × cancelan.
    pub(crate) fn update_workspace_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.workspace_dialog.take() else {
            return;
        };
        let skin = self.pixel.skin();
        let workspaces = &self.workspaces;
        let center = (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO);
        let mut done = None;
        let closed = match &mut dialog {
            WorkspaceDialog::New { name, focused } => {
                let (_, closed) = pixel_ui::app_window(
                    ctx,
                    skin,
                    "new_workspace_window",
                    "New Workspace",
                    true,
                    center,
                    |ui| {
                        let response = widgets::text_line_named(ui, name, "Name");
                        // Hasta que lo tiene (sin Pixel UI, el primer frame de la ventana de egui
                        // es su pasada de medir y ahí el pedido se pierde).
                        if !*focused {
                            response.request_focus();
                            *focused = response.has_focus();
                        }
                        let enter =
                            response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let problem = workspaces.name_problem(name);
                        // Vacío no hace falta decirlo: Save no hace nada.
                        if let Some(why) = problem.filter(|_| !name.trim().is_empty()) {
                            ui.label(why);
                        }
                        ui.add_space(6.0);
                        let clicked = pixel_ui::dialog_buttons(ui, skin, &["Save", "Cancel"]);
                        if clicked == Some(0) || enter {
                            match problem {
                                None => done = Some(true),
                                // No se puede: el campo vuelve a pedir el foco (Save o Enter
                                // se lo quitaron), para corregir el nombre ahí.
                                Some(_) => *focused = false,
                            }
                        }
                        if clicked == Some(1) {
                            done = Some(false);
                        }
                    },
                );
                closed
            }
            WorkspaceDialog::Delete { name } => {
                let names: Vec<String> = workspaces
                    .deletable()
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                let (_, closed) = pixel_ui::app_window(
                    ctx,
                    skin,
                    "delete_workspace_window",
                    "Delete Workspace",
                    true,
                    center,
                    |ui| {
                        ui.label("Workspace to delete:");
                        for n in &names {
                            widgets::radio_value(ui, name, n.clone(), n);
                        }
                        ui.add_space(6.0);
                        match pixel_ui::dialog_buttons(ui, skin, &["Delete", "Cancel"]) {
                            Some(0) => done = Some(true),
                            Some(_) => done = Some(false),
                            None => {}
                        }
                    },
                );
                closed
            }
        };
        if closed || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            done = Some(false);
        }
        match (done, dialog) {
            (None, dialog) => self.workspace_dialog = Some(dialog),
            (Some(false), _) => {}
            (Some(true), WorkspaceDialog::New { name, .. }) => {
                let shown = self.shown();
                // `name_problem` ya lo revisó.
                let _ = self.workspaces.create(&name, &self.workspace, shown);
            }
            (Some(true), WorkspaceDialog::Delete { name }) => {
                self.workspaces.delete(&name);
            }
        }
    }
}
