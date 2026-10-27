//! La entrada del lienzo: el puntero con las herramientas, las guías, los ejes de simetría y
//! la caja de transformar.

use super::*;

impl CanvasView {
    /// Si `p` cae sobre un artboard que va delante del activo.
    fn under_front_artboard(&self, rect: Rect, editor: &Editor, p: Pos2, ppp: f32) -> bool {
        let view = editor.view();
        let document = view.document.as_ref();
        let covers = self.covers(rect, document, ppp);
        document
            .artboards
            .iter()
            .position(|a| a.id == editor.active)
            .is_some_and(|k| covers[k + 1..].iter().any(|c| c.contains(p)))
    }

    /// Mover el eje de simetría (centro o líneas). Igual que las guías: con Ctrl o con
    /// Select/Move/Artboard.
    #[allow(clippy::too_many_arguments)]
    fn symmetry_input(
        &mut self,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        pointer: Option<Pos2>,
        pressed: bool,
        down: bool,
        ctrl: bool,
    ) -> Option<egui::CursorIcon> {
        let symmetry = editor.symmetry();
        if !symmetry.axes.any() {
            self.sym_drag = None;
            return None;
        }
        let cursor = |h: SymHandle| match h {
            SymHandle::Center => egui::CursorIcon::Move,
            SymHandle::X => egui::CursorIcon::ResizeHorizontal,
            SymHandle::Y => egui::CursorIcon::ResizeVertical,
        };
        let origin = self.to_screen(rect, editor.artboard().position);
        let half = self.zoom / 2.0;
        let to_c2 = |p: Pos2| {
            let v = (p - origin) / half;
            (v.x.round() as i32, v.y.round() as i32)
        };
        if let Some((handle, c2)) = self.sym_drag {
            if down {
                if let Some(p) = pointer {
                    let (x, y) = to_c2(p);
                    let c2 = match handle {
                        SymHandle::Center => (x, y),
                        SymHandle::X => (x, c2.1),
                        SymHandle::Y => (c2.0, y),
                    };
                    self.sym_drag = Some((handle, c2));
                }
                return Some(cursor(handle));
            }
            editor.set_symmetry(cs_core::Symmetry {
                center2: Some(c2),
                ..symmetry
            });
            self.sym_drag = None;
            return None;
        }
        if editor.is_painting() || !response.hovered() {
            return None;
        }
        let p = pointer?;
        // Debajo de un artboard de delante no se ve: tampoco se agarra.
        if self.under_front_artboard(rect, editor, p, response.ctx.pixels_per_point()) {
            return None;
        }
        let (center, c2) = self.symmetry_center(rect, editor);
        // Lo visible del artboard: los handles se pegan al borde de la vista (como se dibujan).
        let artboard = Rect::from_min_max(
            origin,
            origin
                + Vec2::new(editor.sprite().width as f32, editor.sprite().height as f32)
                    * self.zoom,
        )
        .intersect(self.visible(rect));
        let handle = if p.distance(center) <= 7.0 {
            Some(SymHandle::Center)
        } else if symmetry.axes.vertical
            && (p.x - center.x).abs() <= 4.0
            && artboard.y_range().contains(p.y)
        {
            Some(SymHandle::X)
        } else if symmetry.axes.horizontal
            && (p.y - center.y).abs() <= 4.0
            && artboard.x_range().contains(p.x)
        {
            Some(SymHandle::Y)
        } else {
            None
        }?;
        // El control de la simetría (el centro y los handles de las puntas de cada eje) se agarra
        // con cualquier herramienta; el resto de la línea, solo con las que no dibujan (o con
        // Ctrl), para poder dibujar sobre el eje.
        let end = crate::pixel_selection::HANDLE + 2.0;
        let near_end = |v: f32, range: egui::Rangef| v - range.min <= end || range.max - v <= end;
        let on_control = match handle {
            SymHandle::Center => true,
            SymHandle::X => near_end(p.y, artboard.y_range()),
            SymHandle::Y => near_end(p.x, artboard.x_range()),
        };
        let grabbable = on_control
            || ctrl
            || matches!(
                editor.active_tool(),
                ToolKind::RectSelect
                    | ToolKind::EllipseSelect
                    | ToolKind::Move
                    | ToolKind::Artboard
            );
        if !grabbable {
            self.guide_hint = Some("Symmetry axis: hold Ctrl and drag to move it");
            return None;
        }
        self.guide_hint = Some("Drag to move the symmetry axis");
        if pressed {
            self.sym_drag = Some((handle, c2));
        }
        Some(cursor(handle))
    }

    /// Crear (desde las reglas), mover y borrar guías. Devuelve el cursor a mostrar si el
    /// puntero está ocupado por las reglas/guías (y en ese caso las herramientas no reciben el clic).
    #[allow(clippy::too_many_arguments)]
    fn guide_input(
        &mut self,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        view: &ViewSettings,
        pointer: Option<Pos2>,
        pressed: bool,
        down: bool,
    ) -> Option<egui::CursorIcon> {
        let origin = editor.artboard().position;
        // Copias para no tomar prestado `self` dentro de los closures.
        let (screen_origin, zoom) = (self.origin(rect), self.zoom);
        let local = move |p: Pos2| {
            let w = (p - screen_origin) / zoom;
            (w.x - origin.0 as f32, w.y - origin.1 as f32)
        };
        let ruler_px = self.ruler_px();
        let on_ruler = |p: Pos2| {
            view.rulers && {
                let (top, left) = Self::ruler_rects(rect, ruler_px);
                top.contains(p)
                    || left.contains(p)
                    || p.x < rect.left() + ruler_px && p.y < rect.top() + ruler_px
            }
        };
        let axis_cursor = |axis: Axis| match axis {
            Axis::Horizontal => egui::CursorIcon::ResizeVertical,
            Axis::Vertical => egui::CursorIcon::ResizeHorizontal,
        };
        self.over_ruler = pointer.is_some_and(|p| response.hovered() && on_ruler(p));

        if let Some(mut drag) = self.ruler_drag {
            if down {
                if let Some(p) = pointer {
                    let (lx, ly) = local(p);
                    drag.position = match drag.axis {
                        Axis::Horizontal => ly.round() as i32,
                        Axis::Vertical => lx.round() as i32,
                    };
                    drag.over_ruler = on_ruler(p);
                    self.ruler_drag = Some(drag);
                }
                return Some(axis_cursor(drag.axis));
            }
            let new = (!drag.over_ruler).then_some(Guide {
                axis: drag.axis,
                position: drag.position,
            });
            let guides = after_drag(editor.guides(), drag.index, new);
            editor.set_guides(guides);
            self.ruler_drag = None;
            return None;
        }
        if editor.is_painting() || !response.hovered() {
            return None;
        }
        let p = pointer?;
        if on_ruler(p) {
            let (top, left) = Self::ruler_rects(rect, ruler_px);
            let axis = if top.contains(p) {
                Some(Axis::Horizontal)
            } else if left.contains(p) {
                Some(Axis::Vertical)
            } else {
                None
            };
            if pressed && let Some(axis) = axis {
                self.ruler_drag = Some(GuideDrag {
                    index: None,
                    axis,
                    position: 0,
                    over_ruler: true,
                });
            }
            if axis.is_some() {
                self.guide_hint = Some("Drag from the ruler to create a guide");
            }
            return Some(axis.map_or(egui::CursorIcon::Default, axis_cursor));
        }
        if !view.shows(view.guides) {
            return None;
        }
        let index = hit_guide(editor.guides(), local(p), SNAP_PX / self.zoom)?;
        // Con cualquier herramienta: la guía bajo el puntero se agarra, salvo que estén
        // bloqueadas.
        if view.lock_guides {
            self.guide_hint = Some("Guides are locked (View › Guides › Lock Guides)");
            return None;
        }
        self.guide_hint = Some("Drag to move the guide · drop it on a ruler to remove it");
        let guide = editor.guides()[index];
        if pressed {
            self.ruler_drag = Some(GuideDrag {
                index: Some(index),
                axis: guide.axis,
                position: guide.position,
                over_ruler: false,
            });
        }
        Some(axis_cursor(guide.axis))
    }

    pub(super) fn handle_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        dialog: Option<&mut CanvasSizeDialog>,
        view: &ViewSettings,
    ) {
        // Rueda: zoom anclado al cursor. Con Ctrl (Cmd en macOS) y una herramienta de pincel
        // cambia el grosor. Se leen los eventos crudos con los modificadores de cada evento. egui
        // convierte Ctrl+rueda en `zoom_delta` y no en scroll, y nadie usa `zoom_delta`, así que
        // la interfaz no se agranda ni los paneles se desplazan.
        // Durante un trazo el grosor no cambia: Ctrl+rueda se ignora (ni grosor ni zoom).
        let painting = editor.is_painting();
        let brush = editor.active_tool().uses_brush();
        // Los restos de rueda de touchpad solo siguen sumando mientras dura el gesto: Ctrl
        // mantenido, herramienta de pincel y ninguna rueda sin Ctrl en el medio.
        let mut keep_rest = brush && ui.input(|i| i.modifiers.command);
        if let Some(pointer) = response.hover_pos() {
            let wheel: Vec<(egui::MouseWheelUnit, f32, bool)> = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel {
                            unit,
                            delta,
                            modifiers,
                            ..
                        } => Some((*unit, delta.y, modifiers.command)),
                        _ => None,
                    })
                    .collect()
            });
            let mut scroll = 0.0;
            for (unit, dy, command) in wheel {
                if command && brush && painting {
                    continue;
                }
                if command && brush {
                    let steps = brush_wheel_steps(&mut self.wheel_rest, unit, dy);
                    if steps != 0 {
                        let size = editor.options.brush_size as i32 + steps;
                        editor.options.brush_size = size.clamp(1, MAX_BRUSH_SIZE as i32) as u32;
                        // Options ya se dibujó en este frame: otro frame muestra el valor nuevo.
                        ui.ctx().request_repaint();
                    }
                } else {
                    keep_rest = false;
                    scroll += dy;
                }
            }
            if scroll != 0.0 {
                self.zoom_at(rect, pointer, scroll > 0.0);
            }
        }
        if !keep_rest {
            self.wheel_rest = 0.0;
        }

        self.draw_pipette = false;
        self.draw_rotate = false;
        self.pixel_cursor = None;
        // Paneo con botón medio o espacio + arrastre. La herramienta Hand se comporta como
        // Espacio mantenido: el botón principal mueve la vista y no llega a ninguna herramienta.
        // A diferencia de Espacio, con Hand las reglas, guías y el eje de simetría siguen
        // funcionando; mientras se arrastra uno de ellos la vista no se mueve.
        let space_key = ui.input(|i| i.key_down(egui::Key::Space));
        let hand = editor.active_tool() == ToolKind::Hand;
        let hand_pans = hand
            && self.ruler_drag.is_none()
            && self.sym_drag.is_none()
            && self.dragging_guide.is_none();
        if response.dragged_by(egui::PointerButton::Middle)
            || ((space_key || hand_pans) && response.dragged_by(egui::PointerButton::Primary))
        {
            self.pan += response.drag_delta();
        }

        self.hover = response
            .hover_pos()
            .map(|p| editor.to_local(self.to_world(rect, p)));

        // La herramienta Zoom: un clic acerca en el puntero; el derecho (o Alt) aleja.
        if editor.active_tool() == ToolKind::Zoom
            && self.ruler_drag.is_none()
            && let Some(p) = response.hover_pos()
        {
            let (primary, secondary, alt) = ui.input(|i| {
                (
                    i.pointer.button_clicked(egui::PointerButton::Primary),
                    i.pointer.button_clicked(egui::PointerButton::Secondary),
                    i.modifiers.alt,
                )
            });
            if primary || secondary {
                self.zoom_at(rect, p, primary && !alt);
            }
        }

        // Pintura: estado crudo del puntero, para que un clic sin arrastre también cuente.
        let (pressed_primary, pressed_secondary, any_down, pointer, modifiers) = ui.input(|i| {
            (
                i.pointer.button_pressed(egui::PointerButton::Primary),
                i.pointer.button_pressed(egui::PointerButton::Secondary),
                i.pointer.primary_down() || i.pointer.secondary_down(),
                i.pointer.interact_pos(),
                i.modifiers,
            )
        });
        editor.options.constrain = modifiers.shift;
        let slop = EDGE_SLOP_PX / self.zoom;
        let area = self.canvas_area(rect, view);
        let over_label = pointer.and_then(|p| self.title_at(ui, rect, area, editor, p));
        self.over_label = over_label.is_some();

        // Con el diálogo Canvas Size abierto el lienzo es modal: solo guías, paneo y zoom.
        let modal = dialog.is_some();
        let over_guide = match (&dialog, pointer) {
            (Some(d), Some(p)) if self.dragging_guide.is_none() && !editor.is_painting() => {
                self.guide_at(rect, d, editor, p)
            }
            _ => None,
        };
        if let Some(d) = dialog {
            if pressed_primary && response.hovered() && over_guide.is_some() && !space_key {
                self.dragging_guide = over_guide;
            }
            if let Some(edge) = self.dragging_guide {
                match pointer {
                    Some(p) if any_down => {
                        let (wx, wy) = self.to_world(rect, p);
                        let coord = if edge.is_vertical() { wx } else { wy };
                        d.set_edge(edge, coord.round() as i32, &editor.document);
                    }
                    _ if !any_down => self.dragging_guide = None,
                    _ => {}
                }
            }
        } else {
            self.dragging_guide = None;
        }
        let mut guide_cursor = self.dragging_guide.or(over_guide).map(|edge| {
            if edge.is_vertical() {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::ResizeVertical
            }
        });
        // Eje de simetría, reglas y guías (no con el diálogo Canvas Size abierto).
        self.guide_hint = None;
        let sym_cursor = if modal || space_key {
            None
        } else {
            self.symmetry_input(
                response,
                rect,
                editor,
                pointer,
                pressed_primary,
                any_down,
                modifiers.command,
            )
        };
        let ruler_cursor = if modal || space_key || sym_cursor.is_some() || self.sym_drag.is_some()
        {
            sym_cursor
        } else {
            self.guide_input(
                response,
                rect,
                editor,
                view,
                pointer,
                pressed_primary,
                any_down,
            )
        };
        let ruler_busy =
            ruler_cursor.is_some() || self.ruler_drag.is_some() || self.sym_drag.is_some();
        guide_cursor = guide_cursor.or(ruler_cursor);
        // Las notas van encima de todo: sobre una (o arrastrándola), las herramientas no reciben
        // el clic.
        let (origin, zoom) = (self.origin(rect), self.zoom);
        let to_screen = move |(x, y): (i32, i32)| origin + Vec2::new(x as f32, y as f32) * zoom;
        if !modal && !space_key && !hand && !ruler_busy && !editor.is_painting() {
            // Solo si lo de arriba en el puntero es el lienzo: con un menú o una ventana encima,
            // las notas de abajo no lo ven.
            let on_top = |p: Pos2| ui.ctx().layer_id_at(p) == Some(response.layer_id);
            let in_area = pointer.filter(|p| area.contains(*p) && on_top(*p));
            self.notes.input(
                editor,
                &to_screen,
                zoom,
                in_area,
                pressed_primary && response.hovered(),
                any_down,
                response.double_clicked(),
            );
        } else {
            self.notes.cursor = None;
        }
        let over_note = self.notes.busy() || self.notes.cursor.is_some();

        // Con un trazo a medias, el clic del otro botón lo cancela (y no empieza otro).
        let cancelled = editor.is_painting()
            && ((pressed_primary && editor.cancel_with_other_button(false))
                || (pressed_secondary && editor.cancel_with_other_button(true)));

        if !modal
            && !cancelled
            && !ruler_busy
            && !over_note
            && self.dragging_guide.is_none()
            && !space_key
            && !hand
            && response.hovered()
            && !editor.is_painting()
            && (pressed_primary || pressed_secondary)
            && let Some(p) = pointer
        {
            let world = self.to_world(rect, p);
            let slice = (pressed_secondary && editor.active_tool() == ToolKind::Slice)
                .then(|| editor.slice_at(editor.to_local(world)))
                .flatten();
            self.slice_menu = slice.is_some();
            let tool = editor.active_tool();
            self.selection_menu = pressed_secondary
                && slice.is_none()
                && (tool.is_selection() || tool == ToolKind::Move)
                && editor.selection.as_ref().is_some_and(|m| {
                    let (x, y) = editor.to_local(world);
                    m.contains(x, y)
                });
            // Un handle o la franja de rotación ganan sobre el gesto de la herramienta.
            let transform = if pressed_primary {
                self.transform_hit(rect, editor, p)
            } else {
                None
            };
            match over_label {
                Some((id, TitleHit::Bar)) if pressed_primary => {
                    editor.press_artboard_label(id, world)
                }
                // "≡": lo activa; su menú se abre al soltar (`artboard_menus`).
                Some((id, TitleHit::Menu)) if pressed_primary => editor.activate(id),
                // "○": borra el artboard (se deshace con Ctrl+Z; el último no se borra).
                Some((id, TitleHit::Close)) if pressed_primary => {
                    if editor.document.artboards.len() > 1 {
                        editor.activate(id);
                        editor.remove_active_artboard();
                    }
                }
                _ if slice.is_some() => {
                    editor.end_group();
                    editor.slice = slice;
                }
                // El clic derecho abre el menú de la selección en vez de llegar a la herramienta.
                _ if self.selection_menu => {}
                _ => match transform {
                    Some(hit) => editor.press_transform(hit, world),
                    None => editor.pointer_press(world, pressed_secondary, modifiers.alt, slop),
                },
            }
        }
        self.artboard_menus(ui, rect, editor);
        if self.selection_menu {
            menus::context_menu(response, |ui| {
                if ui.add(menus::item("Flip Horizontal", "Shift+H")).clicked() {
                    editor.flip(true);
                    ui.close();
                }
                if ui.add(menus::item("Flip Vertical", "Shift+V")).clicked() {
                    editor.flip(false);
                    ui.close();
                }
            });
        }
        if self.slice_menu {
            menus::context_menu(response, |ui| {
                if ui.add(menus::plain("Slice Properties")).clicked() {
                    self.slice_properties = true;
                    ui.close();
                }
                if ui.add(menus::plain("Duplicate Slice")).clicked() {
                    editor.duplicate_slice();
                    ui.close();
                }
                if ui.add(menus::plain("Delete Slice")).clicked() {
                    editor.delete_slice();
                    ui.close();
                }
            });
        }
        // Auto-scroll: arrastrando cerca o fuera del borde, la vista se desplaza.
        let dragging = editor.is_painting() || self.ruler_drag.is_some() || self.sym_drag.is_some();
        if dragging
            && any_down
            && !space_key
            && !modal
            && let Some(p) = pointer
        {
            let velocity = autoscroll_velocity(area, p);
            if velocity != Vec2::ZERO {
                let dt = ui.input(|i| i.stable_dt).min(0.05);
                self.pan -= velocity * dt;
                ui.ctx().request_repaint();
            }
        }
        if editor.is_painting() {
            if any_down {
                if let Some(p) = pointer {
                    editor.pointer_drag(self.to_world(rect, p));
                }
                let dt_ms = ui.input(|i| i.stable_dt).min(0.1) * 1000.0;
                editor.tool_tick(dt_ms);
                if editor.active_tool() == ToolKind::Spray {
                    ui.ctx().request_repaint();
                }
            } else {
                editor.pointer_release();
            }
        } else if response.hovered()
            && let Some(p) = pointer
        {
            editor.pointer_hover(self.to_world(rect, p));
        }
        // Con Hand el doble clic sobre la etiqueta no la renombra: Hand solo mueve la vista.
        if response.double_clicked()
            && !hand
            && let Some((id, TitleHit::Bar)) = over_label
            && let Some(artboard) = editor.document.artboard(id)
        {
            self.renaming = Some((id, artboard.name.clone(), false));
        }
        if response.double_clicked()
            && matches!(
                editor.active_tool(),
                ToolKind::PolygonLasso | ToolKind::Polygon
            )
        {
            editor.finish_gesture();
        }
        if response.double_clicked()
            && editor.active_tool() == ToolKind::Move
            && editor.enter_instance()
        {
            self.center_active(editor);
        }

        let transform_cursor = editor
            .transform_drag_hit()
            .or_else(|| pointer.and_then(|p| self.transform_hit(rect, editor, p)));
        let hand_dragging = hand_pans && response.dragged_by(egui::PointerButton::Primary);
        let cursor = if space_key || hand_dragging {
            Some(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            })
        } else if guide_cursor.is_some() {
            guide_cursor
        } else if hand {
            // En reposo; sobre las reglas o una guía manda su cursor (arriba).
            response.hovered().then_some(egui::CursorIcon::Grab)
        } else if editor.active_tool() == ToolKind::Zoom && !ruler_busy {
            response.hovered().then_some(if modifiers.alt {
                egui::CursorIcon::ZoomOut
            } else {
                egui::CursorIcon::ZoomIn
            })
        } else if modal {
            response.hovered().then_some(egui::CursorIcon::Default)
        } else if !response.hovered() {
            None
        } else if ruler_busy {
            Some(egui::CursorIcon::Default)
        } else if let Some(c) = self.notes.cursor {
            Some(c)
        } else if editor.artboard_gesture().moving() {
            Some(egui::CursorIcon::Grabbing)
        } else if let Some(edges) = editor.artboard_gesture().resizing() {
            Some(artboard_cursor(Some(ArtboardHit::Edge(
                editor.active,
                edges,
            ))))
        } else if let Some((_, hit)) = over_label.filter(|_| !editor.is_painting()) {
            Some(match hit {
                TitleHit::Bar => egui::CursorIcon::Grab,
                TitleHit::Menu => egui::CursorIcon::PointingHand,
                TitleHit::Close => egui::CursorIcon::Default,
            })
        } else if let Some(hit) = transform_cursor {
            match hit {
                TransformHit::Rotate => {
                    if self.can_draw(PixelCursor::Rotate) {
                        self.pixel_cursor = Some(PixelCursor::Rotate);
                    } else {
                        self.draw_rotate = true;
                    }
                    Some(egui::CursorIcon::None)
                }
                TransformHit::Handle(handle) => Some(handle_cursor(handle)),
                TransformHit::Move => Some(egui::CursorIcon::Move),
            }
        } else if editor.active_tool() == ToolKind::Artboard {
            let world = pointer.map(|p| self.to_world(rect, p));
            Some(artboard_cursor(
                world.and_then(|w| hit_test(&editor.document, w, slop)),
            ))
        } else if editor.active_tool() == ToolKind::Move
            // Durante el arrastre sigue Move aunque el puntero salga de lo seleccionado o del
            // artboard, o cambien los modificadores.
            || editor.is_transforming()
            || pointer.is_some_and(|p| {
                let world = self.to_world(rect, p);
                editor.moves_selection_at(world, modifiers.shift, modifiers.alt)
            })
        {
            Some(egui::CursorIcon::Move)
        } else if let Some(kind) = Some(match editor.active_tool() {
            ToolKind::Eyedropper => PixelCursor::Eyedropper,
            tool if tool.is_selection() => PixelCursor::Marquee,
            _ => PixelCursor::Cross,
        })
        .filter(|&k| self.can_draw(k))
        {
            self.pixel_cursor = Some(kind);
            Some(egui::CursorIcon::None)
        } else if editor.active_tool() == ToolKind::Eyedropper {
            self.draw_pipette = true;
            Some(egui::CursorIcon::None)
        } else {
            Some(egui::CursorIcon::Crosshair)
        };
        if let Some(c) = cursor {
            ui.ctx().set_cursor_icon(c);
        }
    }

    /// Guía bajo `p` (en pantalla), si hay una a menos de `EDGE_SLOP_PX`.
    fn guide_at(
        &self,
        rect: Rect,
        dialog: &CanvasSizeDialog,
        editor: &Editor,
        p: Pos2,
    ) -> Option<GuideEdge> {
        let (min, max) = dialog.preview_rect(&editor.document)?;
        let (a, b) = (self.to_screen(rect, min), self.to_screen(rect, max));
        [
            (GuideEdge::Left, (p.x - a.x).abs()),
            (GuideEdge::Right, (p.x - b.x).abs()),
            (GuideEdge::Top, (p.y - a.y).abs()),
            (GuideEdge::Bottom, (p.y - b.y).abs()),
        ]
        .into_iter()
        .filter(|(_, d)| *d <= EDGE_SLOP_PX)
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .map(|(edge, _)| edge)
    }

    /// Handle o franja de rotación bajo `p` (en pantalla), si se muestran los handles.
    fn transform_hit(&self, rect: Rect, editor: &Editor, p: Pos2) -> Option<TransformHit> {
        if !handles_active(editor) {
            return None;
        }
        let b = editor.transform_box()?;
        let (wx, wy) = self.to_world(rect, p);
        let (ax, ay) = editor.artboard().position;
        let local = (wx - ax as f32, wy - ay as f32);
        let handle = (HANDLE_PX / 2.0 + 1.0) / self.zoom;
        transform_box::hit(&b, local, handle, ROTATE_PX / self.zoom)
    }
}
