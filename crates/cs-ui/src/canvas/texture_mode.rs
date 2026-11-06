//! El modo Texture del workspace de un modelo (como picoCAD 2): el área a la izquierda del panel
//! se parte arriba/abajo. Arriba, el render del modelo (clic elige caras; orbitar, mover el
//! objetivo y la rueda como en Persp) con el contorno amarillo de las caras elegidas. Abajo, la
//! fila de opciones de la herramienta (la barra de siempre: pincel, tinta, simetría…) y la
//! textura por el camino de siempre del lienzo (un solo artboard), con su propio zoom entero
//! y desplazamiento (`texture_view`): herramientas, cursor, cuadrícula de pixeles y atajos
//! funcionan ahí.

use eframe::egui::pos2;

use super::model_edit::{PERSP_BG, bar_split, cad_header, header_split};
use super::*;
use crate::editor::TextureView;

/// Las dos partes del área: arriba el modelo y abajo la textura, con una línea de 1 punto entre
/// las dos a la mitad, en puntos enteros.
pub(crate) fn texture_panes(area: Rect) -> (Rect, Rect) {
    let mid = area.top() + (area.height() / 2.0).floor();
    (
        Rect::from_min_max(area.min, pos2(area.right(), mid)),
        Rect::from_min_max(pos2(area.left(), (mid + 1.0).min(area.bottom())), area.max),
    )
}

impl CanvasView {
    /// El workspace del modelo en modo Texture, en `rect` (todo el lienzo).
    pub(super) fn show_texture_mode(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        dialog: Option<&mut CanvasSizeDialog>,
        view_settings: &ViewSettings,
    ) {
        // Debajo del encabezado de ChimpCAD, la barrita del objeto (como en modo Model) y las
        // dos partes: arriba el render, pegado a la barrita como en el mockup; abajo, la fila de
        // opciones de la herramienta (para pintar la textura: el tamaño del pincel, la tinta…)
        // encima de la textura, junto a lo que pinta. Van aquí y no arriba de todo para que el
        // encabezado no se mueva al cambiar de modo.
        let (header, area) = header_split(rect);
        let skin = crate::pixel_ui::frame_skin(ui.ctx());
        let skin = skin.as_deref();
        let height = crate::panels::options_bar_height(skin);
        let (object_bar, area) = bar_split(area, height);
        let (top, below) = texture_panes(area);
        let (bar, bottom) = bar_split(below, height);
        self.edit_quads = None;
        self.texture_panes = Some((top, bottom));
        self.header_rect = Some(header);
        self.object_bar_rect = Some(object_bar);
        // G y S de teclado son de las vistas orto: aquí no esperan a nadie.
        editor.model_edit.key_request = None;
        self.texture_top_input(ui, response, top, editor, dialog.is_some());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.background);
        if editor.is_workspace_texture(editor.active) {
            // Abajo, el lienzo de siempre con su propia respuesta (la de arriba no la ve).
            let pane = ui.interact(
                bottom,
                ui.id().with("texture_pane"),
                Sense::click_and_drag(),
            );
            self.show_texture_pane(ui, &pane, bottom, editor, dialog, view_settings);
        } else {
            self.clear_canvas_hover();
            self.drop_uv_press(editor);
            self.pending_zoom = None;
            self.no_texture(ui, bottom, editor);
        }
        painter.rect_filled(top, 0.0, PERSP_BG);
        let view = editor.view();
        if let Some(model) = editor.edit_model_shown() {
            let ctx = ui.ctx().clone();
            self.draw_persp(&painter, top, editor, view.document.as_ref(), model, &ctx);
        }
        drop(view);
        let line = top.bottom().round();
        // El borde oscuro de los controles: el azul de las vistas no se vería sobre el render.
        painter.rect_filled(
            Rect::from_x_y_ranges(area.x_range(), line..=line + 1.0),
            0.0,
            crate::pixel_ui::control_border(ui.ctx()),
        );
        cad_header(ui, header, editor);
        // Las barras, encima del fondo ya pintado.
        super::object_bar::object_bar(ui, object_bar, editor);
        let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).id_salt("cad_options"));
        bar_ui.set_clip_rect(bar);
        crate::panels::options_bar_panel(bar_ui.style(), skin).show(&mut bar_ui, |ui| {
            crate::panels::tool_options_bar(ui, editor, skin);
        });
    }

    /// Nada del lienzo bajo el puntero (sin textura no hay pixel, cursores dibujados ni menús).
    fn clear_canvas_hover(&mut self) {
        self.hover = None;
        self.draw_pipette = false;
        self.draw_rotate = false;
        self.pixel_cursor = None;
        self.guide_hint = None;
        self.over_label = false;
        self.over_ruler = false;
        self.advance_line = None;
        self.selection_menu = false;
        self.object_menu = false;
        self.slice_menu = false;
    }

    /// La entrada del render de arriba: la rueda (con o sin Ctrl) acerca o aleja la cámara; Home
    /// encuadra el modelo; los botones eligen caras, orbitan o mueven el objetivo
    /// (`texture_press_at`); el arrastre sigue al puntero.
    fn texture_top_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        top: Rect,
        editor: &mut Editor,
        modal: bool,
    ) {
        let over = |p: Pos2| top.contains(p);
        // Hacia el punto bajo el puntero, como en Persp. Los restos de rueda de touchpad siguen
        // sumando mientras el puntero siga arriba (la textura de abajo tiene los suyos).
        if let Some(p) = response
            .hover_pos()
            .filter(|p| response.hovered() && over(*p))
        {
            for (unit, dy) in model_input::wheel_events(ui) {
                let steps = brush_wheel_steps(&mut self.model_wheel_rest, unit, dy);
                if steps != 0 {
                    let at = self.persp_map_in(top, editor).map(|map| (map, p));
                    let now = ui.input(|i| i.time);
                    self.model_wheel(editor, steps, at, now);
                }
            }
        } else {
            self.model_wheel_rest = 0.0;
        }
        // Home con el puntero arriba encuadra el modelo, como en modo Model (abajo es de la
        // textura). No mientras se escribe en un campo ni durante un arrastre.
        let home = ui.input(|i| {
            i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Home,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if modifiers.is_none()
                )
            })
        });
        if home
            && !modal
            && self.model_drag.is_none()
            && !ui.ctx().egui_wants_keyboard_input()
            && response.hover_pos().is_some_and(over)
        {
            editor.frame_model();
        }
        let (pressed, secondary, middle, any_down, pointer, modifiers) = ui.input(|i| {
            let pressed = |b| i.pointer.button_pressed(b);
            (
                pressed(egui::PointerButton::Primary)
                    || pressed(egui::PointerButton::Secondary)
                    || pressed(egui::PointerButton::Middle),
                pressed(egui::PointerButton::Secondary),
                pressed(egui::PointerButton::Middle),
                i.pointer.primary_down() || i.pointer.secondary_down() || i.pointer.middle_down(),
                i.pointer.interact_pos(),
                i.modifiers,
            )
        });
        if pressed
            && !modal
            && response.hovered()
            && self.model_drag.is_none()
            && let Some(p) = pointer.filter(|p| over(*p))
            && !self.header_rect.is_some_and(|r| r.contains(p))
        {
            let (r, scale) = self.persp_rect(top, editor);
            let local = (
                ((p.x - r.left()) / scale.x).floor() as i32,
                ((p.y - r.top()) / scale.y).floor() as i32,
            );
            if middle {
                self.model_pan_press(editor, p, Vec2::ONE / scale);
            } else {
                self.texture_press_at(editor, p, local, secondary, modifiers, Vec2::ONE / scale);
            }
        }
        self.model_drag_input(editor, pointer, any_down);
    }

    /// La textura en el panel de abajo: el lienzo de siempre en `bottom` con el zoom y el
    /// desplazamiento de `texture_view` (al entrar, encuadrada), sin reglas, notas, ventana ni
    /// etiqueta. La vista del documento se guarda y vuelve igual.
    fn show_texture_pane(
        &mut self,
        ui: &mut egui::Ui,
        pane: &egui::Response,
        bottom: Rect,
        editor: &mut Editor,
        dialog: Option<&mut CanvasSizeDialog>,
        view_settings: &ViewSettings,
    ) {
        let document_view = (self.zoom, self.pan, self.fitted);
        let window = self.artboard_window.take();
        // Se encuadra al entrar y, mientras la vista sea la encuadrada, si el panel cambia.
        let shown = editor.model_edit().texture_view;
        let refit = match (shown, self.texture_fit) {
            (None, _) => true,
            (Some(v), Some((r, fitted))) => v == fitted && r != bottom,
            (Some(_), None) => false,
        };
        match shown.filter(|_| !refit) {
            Some(v) => (self.zoom, self.pan) = (v.zoom.max(1) as f32, Vec2::new(v.pan.0, v.pan.1)),
            None => {
                self.fit_texture(bottom, editor);
                self.texture_fit = Some((bottom, self.texture_view()));
            }
        }
        // Caras recién elegidas arriba: encuadradas.
        if std::mem::take(&mut editor.model_edit.focus_uvs) {
            self.focus_uvs(bottom, editor);
        }
        self.fitted = true;
        let mut view = *view_settings;
        view.rulers = false;
        view.artboard_names = false;
        self.rulers_shown = false;
        self.texture_pane = true;
        self.show_artboards(ui, pane, bottom, editor, dialog, &view);
        self.texture_pane = false;
        editor.model_edit.texture_view = Some(self.texture_view());
        self.texture_zoom = Some(self.zoom);
        (self.zoom, self.pan, self.fitted) = document_view;
        self.artboard_window = window;
    }

    /// El zoom (entero) y el desplazamiento de ahora, como vista de la textura.
    fn texture_view(&self) -> TextureView {
        TextureView {
            zoom: self.zoom.round().max(1.0) as u32,
            pan: (self.pan.x, self.pan.y),
        }
    }

    /// Encuadra la textura (el artboard activo) en `bottom`: el zoom entero más grande con el
    /// que cabe con aire (de 1 a 64) y centrada.
    pub(super) fn fit_texture(&mut self, bottom: Rect, editor: &Editor) {
        let ((x0, y0), (x1, y1)) = editor.artboard().rect();
        let size = Vec2::new((x1 - x0).max(1) as f32, (y1 - y0).max(1) as f32) * self.ratio;
        self.zoom = (bottom.size() * 0.85 / size)
            .min_elem()
            .floor()
            .clamp(1.0, 64.0);
        let center = Vec2::new((x0 + x1) as f32, (y0 + y1) as f32) / 2.0;
        self.pan = -center * self.scale();
    }

    /// Encuadra en `bottom` la caja de las UVs de las caras elegidas: el zoom entero más grande
    /// con el que cabe con aire (de 1 a 64) y centrada.
    pub(super) fn focus_uvs(&mut self, bottom: Rect, editor: &Editor) {
        let faces = editor.selected_faces();
        let Some(((x0, y0), (x1, y1))) = editor
            .edited_model()
            .and_then(|m| cs_3d::uv_bounds(m, &faces))
        else {
            return;
        };
        let size = Vec2::new((x1 - x0).max(1) as f32, (y1 - y0).max(1) as f32) * self.ratio;
        self.zoom = (bottom.size() * 0.85 / size)
            .min_elem()
            .floor()
            .clamp(1.0, 64.0);
        let (ax, ay) = editor.artboard().position;
        let center = Vec2::new((ax * 2 + x0 + x1) as f32, (ay * 2 + y0 + y1) as f32) / 2.0;
        self.pan = -center * self.scale();
    }

    /// Sin textura: "No texture" y el botón **New Texture** (el del panel 3D), centrados en
    /// `bottom`.
    fn no_texture(&mut self, ui: &mut egui::Ui, bottom: Rect, editor: &mut Editor) {
        let painter = ui.painter_at(bottom);
        let label = "No texture";
        let skin = crate::pixel_ui::frame_skin(ui.ctx());
        let color = skin.as_ref().map_or(theme::LABEL, |s| s.colors().text);
        let button_h = match &skin {
            Some(_) => kit::button::HEIGHT,
            None => 20.0,
        };
        // El texto y el botón, uno sobre otro con 6 puntos de aire, centrados en el panel.
        let text_w = kit::button::ink_width(ui, label);
        let block_h = 7.0 + 6.0 + button_h;
        let top = (bottom.center().y - block_h / 2.0).round();
        let (text, _) = crate::pixel_draw::caps_text(
            ui,
            label,
            (bottom.center().x - text_w / 2.0).round(),
            top,
            color,
        );
        painter.add(text);
        let new_label = "New Texture";
        let clicked = match &skin {
            Some(skin) => {
                let w = kit::button::width(ui, new_label);
                let at = pos2((bottom.center().x - w / 2.0).round(), top + 13.0);
                let r = Rect::from_min_size(at, Vec2::new(w, kit::button::HEIGHT));
                let id = egui::Id::new("texture_mode_new_texture");
                kit::button::button_at(ui, skin, r, id, new_label).clicked()
            }
            None => {
                let size = Vec2::new(100.0, button_h);
                let at = pos2((bottom.center().x - size.x / 2.0).round(), top + 13.0);
                ui.put(Rect::from_min_size(at, size), egui::Button::new(new_label))
                    .clicked()
            }
        };
        if clicked && editor.new_texture() {
            // La textura nueva es la que se pinta.
            if let Some(t) = editor.workspace_texture() {
                editor.activate(t);
            }
            editor.model_edit.texture_view = None;
        }
    }
}
