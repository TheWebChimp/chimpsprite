//! El workspace de un modelo 3D (ChimpCAD, como picoCAD 2): todo el área del lienzo se parte en
//! cuatro cuadrantes, Front | Persp arriba y Top | Side abajo. Las vistas ortográficas son
//! wireframes (todas las aristas) con la cuadrícula, la selección, su caja con asas y las guías
//! punteadas; la perspectiva es el render pixel a la resolución del artboard, a escala entera.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};

use cs_3d::{Handle, Item, OrthoView, SelectMode, ViewMap};
use cs_core::Document;
use eframe::egui::{pos2, vec2};

use super::*;
use crate::editor::{CadMode, EditOp, KeyOp};

pub(crate) const FRONT: usize = 0;
pub(crate) const PERSP: usize = 1;
pub(crate) const TOP: usize = 2;
pub(crate) const SIDE: usize = 3;

/// Colores provisionales (los de picoCAD): fondo de las vistas orto, de la perspectiva, aristas,
/// cuadrícula y separadores.
const ORTHO_BG: Color32 = Color32::from_rgb(0x83, 0x76, 0x9c);
pub(super) const PERSP_BG: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
const EDGE: Color32 = Color32::from_rgb(0xc2, 0xc3, 0xc7);
const GRID_LINE: Color32 = Color32::from_rgba_premultiplied(0x10, 0x10, 0x20, 0x30);
// Provisional: el amarillo de PICO-8, que se ve sobre el morado de las vistas (el color de
// selección del skin es casi del mismo tono).
const SELECTED: Color32 = Color32::from_rgb(0xff, 0xec, 0x27);
const GUIDE: Color32 = Color32::from_rgb(0x5f, 0x57, 0x4f);
const SEPARATOR: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
/// Lado de los cuadritos de los vértices y de las asas, en puntos.
const VERTEX_PX: f32 = 3.0;
pub(crate) const HANDLE_PX: f32 = 5.0;
/// Con líneas más juntas que esto (en puntos) la cuadrícula no se dibuja.
const GRID_MIN_PX: f32 = 4.0;
/// Con S apretada a menos de esto (en puntos) del centro, el factor se mide contra
/// [`SCALE_REF_PX`] (si no, casi cualquier movimiento lo dispararía o lo dejaría en 0).
const SCALE_NEAR_PX: f32 = 6.0;
const SCALE_REF_PX: f32 = 24.0;

/// Los cuadrantes del área `screen`: Front, Persp, Top y Side. La parten a la mitad en puntos
/// enteros (con ancho impar, la columna de más es de la derecha).
pub(crate) fn quadrants(screen: Rect) -> [Rect; 4] {
    let x = screen.left() + (screen.width() / 2.0).floor();
    let y = screen.top() + (screen.height() / 2.0).floor();
    [
        Rect::from_min_max(screen.min, pos2(x, y)),
        Rect::from_min_max(pos2(x, screen.top()), pos2(screen.right(), y)),
        Rect::from_min_max(pos2(screen.left(), y), pos2(x, screen.bottom())),
        Rect::from_min_max(pos2(x, y), screen.max),
    ]
}

pub(crate) fn quadrant_view(i: usize) -> Option<OrthoView> {
    match i {
        FRONT => Some(OrthoView::Front),
        TOP => Some(OrthoView::Top),
        SIDE => Some(OrthoView::Side),
        _ => None,
    }
}

fn quadrant_index(view: OrthoView) -> usize {
    match view {
        OrthoView::Front => FRONT,
        OrthoView::Top => TOP,
        OrthoView::Side => SIDE,
    }
}

fn pos(p: (f32, f32)) -> Pos2 {
    pos2(p.0, p.1)
}

impl CanvasView {
    /// El workspace del modelo en `rect` (todo el lienzo): encuadra las vistas si hace falta, la
    /// entrada (solo los gestos del modo Edit) y el dibujo de los cuatro cuadrantes. `modal`: con
    /// el diálogo Canvas Size abierto no empieza ningún gesto.
    pub(super) fn show_model_workspace(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        modal: bool,
    ) {
        let area = workspace_area(ui, rect);
        self.texture_panes = None;
        self.mode_rect = Some(mode_buttons_rect(ui, area));
        // Los cuadrantes salen del área, así que ya valen para la entrada de este frame (también
        // en el primero del workspace).
        let quads = quadrants(area);
        self.edit_quads = Some(quads);
        // El zoom del lienzo no aplica aquí.
        self.pending_zoom = None;
        if editor.model_edit().fit {
            let q = quads[FRONT];
            editor.fit_model_views(q.width().min(q.height()));
        }
        self.workspace_input(ui, response, editor, modal);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.background);
        let view = editor.view();
        self.draw_model_edit(ui, &painter, quads, editor, view.document.as_ref());
        drop(view);
        mode_buttons(ui, area, editor);
    }

    /// La entrada del workspace: Ctrl + rueda (zoom de las vistas orto o distancia de la cámara
    /// en Persp) y los botones del modo Edit. La rueda sin Ctrl, el zoom y el paneo del lienzo,
    /// la mano y las herramientas no hacen nada.
    fn workspace_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        editor: &mut Editor,
        modal: bool,
    ) {
        // Nada del lienzo bajo el puntero: ni pixel, ni cursores dibujados, ni menús.
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
        // Los restos de rueda de touchpad siguen sumando mientras Ctrl siga apretado.
        let mut keep_rest = ui.input(|i| i.modifiers.command);
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
            for (unit, dy, command) in wheel {
                if !command {
                    keep_rest = false;
                    continue;
                }
                let steps = brush_wheel_steps(&mut self.wheel_rest, unit, dy);
                // Sobre una vista orto, zoom de las tres; sobre la perspectiva, la cámara.
                if steps != 0 && !self.model_edit_wheel(editor, pointer, steps) {
                    Self::model_wheel(editor, steps);
                }
            }
        }
        if !keep_rest {
            self.wheel_rest = 0.0;
        }
        let (pressed, secondary, middle, any_down, edit_down, pointer, modifiers) = ui.input(|i| {
            let pressed = |b| i.pointer.button_pressed(b);
            (
                pressed(egui::PointerButton::Primary)
                    || pressed(egui::PointerButton::Secondary)
                    || pressed(egui::PointerButton::Middle),
                pressed(egui::PointerButton::Secondary),
                pressed(egui::PointerButton::Middle),
                i.pointer.primary_down() || i.pointer.secondary_down(),
                i.pointer.primary_down() || i.pointer.secondary_down() || i.pointer.middle_down(),
                i.pointer.interact_pos(),
                i.modifiers,
            )
        });
        // Con un arrastre en curso, el otro botón no empieza nada. Un gesto de teclado (G o S)
        // espera un clic: confirma con el izquierdo, cancela con el derecho.
        let busy = self.model_drag.is_some() || editor.model_edit().op.is_some();
        let keyboard_op = matches!(
            editor.model_edit().op,
            Some(EditOp::Move { keyboard: true, .. } | EditOp::Scale { .. })
        );
        if pressed
            && !modal
            && response.hovered()
            && (!busy || (keyboard_op && self.model_drag.is_none()))
            && let Some(p) = pointer
            && !self.mode_rect.is_some_and(|r| r.contains(p))
        {
            self.model_edit_press(editor, p, secondary, middle, modifiers);
        }
        self.model_drag_input(editor, pointer, any_down);
        self.model_edit_input(editor, pointer, edit_down, modifiers);
    }

    /// Dónde va el render de la perspectiva en el cuadrante `q` (a la resolución del artboard,
    /// a la escala entera más grande que quepa, mínimo 1, y centrado) y cuántos puntos mide cada
    /// pixel del render (el dibujo y los gestos usan lo mismo).
    pub(super) fn persp_rect(&self, q: Rect, editor: &Editor) -> (Rect, Vec2) {
        // El del modelo (en modo Texture el activo es su textura).
        let sprite = model_sprite(editor);
        let (rx, ry) = sprite.pixel_ratio;
        let ratio = vec2(rx.max(1) as f32, ry.max(1) as f32);
        let pixels = vec2(
            sprite.width.max(1) as f32 * ratio.x,
            sprite.height.max(1) as f32 * ratio.y,
        );
        let k = (q.width() / pixels.x)
            .min(q.height() / pixels.y)
            .floor()
            .max(1.0);
        let size = pixels * k;
        // En pixeles físicos enteros.
        let ppp = self.ppp.max(1.0);
        let min = ((q.center() - size / 2.0).to_vec2() * ppp).round() / ppp;
        (Rect::from_min_size(min.to_pos2(), size), ratio * k)
    }

    fn quadrant_at(&self, p: Pos2) -> Option<usize> {
        self.edit_quads?.iter().position(|q| q.contains(p))
    }

    /// Una prensa en un cuadrante del workspace. En la perspectiva, la cámara o elegir un objeto
    /// (`model_press_at` con el punto en pixeles del artboard). `false` si no fue del modo Edit.
    pub(super) fn model_edit_press(
        &mut self,
        editor: &mut Editor,
        p: Pos2,
        secondary: bool,
        middle: bool,
        modifiers: egui::Modifiers,
    ) -> bool {
        let Some(i) = self.quadrant_at(p) else {
            return false;
        };
        // Con un G o S de teclado en curso, el clic lo cierra sea cual sea el cuadrante: el
        // izquierdo confirma y el derecho o el central cancelan.
        let keyboard = matches!(
            editor.model_edit().op,
            Some(EditOp::Move { keyboard: true, .. } | EditOp::Scale { .. })
        );
        if keyboard {
            if secondary || middle {
                editor.cancel_model_op();
            } else {
                editor.finish_model_op();
            }
            return true;
        }
        let Some(view) = quadrant_view(i) else {
            // El botón central no hace nada en la perspectiva.
            if !middle {
                self.persp_press(editor, i, p, secondary, modifiers);
            }
            return true;
        };
        let Some(map) = self.edit_view_map(editor, view) else {
            return false;
        };
        let at = (p.x, p.y);
        if secondary || middle {
            editor.start_model_op(EditOp::Pan {
                map,
                from: at,
                center: editor.model_edit().center,
            });
            return true;
        }
        let Some(model) = editor.edited_model().cloned() else {
            return false;
        };
        let sel = editor.model_edit().selection.clone();
        // Un vértice ya elegido bajo el puntero gana sobre el asa que cae encima: se mueve (con
        // varios vértices, las asas de la caja caen justo en los de los extremos).
        if !modifiers.shift
            && let Some(grab) = selected_vertex_at(&model, &map, &sel, at)
        {
            editor.start_model_op(EditOp::Move {
                map,
                from: at,
                grab,
                keyboard: false,
            });
            // Un clic sin arrastre deja solo lo que se presionó (la arista, en modo Edge).
            let item = match sel.mode {
                SelectMode::Vertex => cs_3d::pick(&model, &map, SelectMode::Vertex, at),
                _ => cs_3d::pick(&model, &map, SelectMode::Edge, at),
            };
            editor.model_edit.click_item = item.filter(|i| sel.items.contains(i));
            return true;
        }
        // Con Shift solo las esquinas (escalan igual en los dos ejes); en un lado, Shift + clic
        // sigue agregando o quitando de la selección.
        if let Some(b) = cs_3d::bounds(&model, &sel)
            && !(sel.mode == SelectMode::Vertex && sel.items.len() == 1)
            && let Some(handle) = Handle::ALL.into_iter().find(|&h| {
                (!modifiers.shift || (h.u != 0 && h.v != 0))
                    && useful_handle(view, b, h)
                    && (handle_pos(&map, b, h) - p).length() <= HANDLE_PX
            })
        {
            editor.start_model_op(EditOp::Handle {
                map,
                from: at,
                handle,
                bounds: b,
            });
            return true;
        }
        match cs_3d::pick(&model, &map, sel.mode, at) {
            Some(item) if modifiers.shift => {
                let mut next = sel;
                next.toggle(item);
                editor.set_model_selection(next);
            }
            Some(item) => {
                let already = sel.items.contains(&item);
                if !already {
                    editor.set_model_selection(cs_3d::Selection {
                        mode: sel.mode,
                        items: [item].into(),
                    });
                }
                let now = editor.model_edit().selection.clone();
                let grab = match item {
                    Item::Vertex(o, v) => cs_3d::world_points(&model.objects[o])[v as usize],
                    _ => cs_3d::bounds(&model, &now).map_or([0.0; 3], cs_3d::center),
                };
                editor.start_model_op(EditOp::Move {
                    map,
                    from: at,
                    grab,
                    keyboard: false,
                });
                // Ya elegido: sin arrastre, queda como la única selección.
                editor.model_edit.click_item = already.then_some(item);
            }
            None => editor.start_model_op(EditOp::Box {
                map,
                from: at,
                to: at,
                add: modifiers.shift,
            }),
        }
        true
    }

    /// La prensa en la perspectiva: el punto, del render escalado en el cuadrante a pixeles del
    /// artboard.
    fn persp_press(
        &mut self,
        editor: &mut Editor,
        i: usize,
        p: Pos2,
        secondary: bool,
        modifiers: egui::Modifiers,
    ) {
        let Some(q) = self.edit_quads.map(|qs| qs[i]) else {
            return;
        };
        let (r, scale) = self.persp_rect(q, editor);
        let local = (
            ((p.x - r.left()) / scale.x).floor() as i32,
            ((p.y - r.top()) / scale.y).floor() as i32,
        );
        self.model_press_at(editor, p, local, secondary, modifiers, Vec2::ONE / scale);
    }

    /// Cada frame: el gesto sigue al puntero; al soltar (si no es de teclado) termina. Un G o S
    /// pedido por el teclado empieza aquí, en la vista bajo el puntero.
    pub(super) fn model_edit_input(
        &mut self,
        editor: &mut Editor,
        pointer: Option<Pos2>,
        down: bool,
        modifiers: egui::Modifiers,
    ) {
        if let Some(req) = editor.model_edit.key_request.take()
            && let Some(p) = pointer
            && let Some(view) = self.quadrant_at(p).and_then(quadrant_view)
            && let Some(map) = self.edit_view_map(editor, view)
            && let Some(b) = editor
                .edited_model()
                .and_then(|m| cs_3d::bounds(m, &editor.model_edit().selection))
        {
            let c = cs_3d::center(b);
            let op = match req {
                KeyOp::Grab => EditOp::Move {
                    map,
                    from: (p.x, p.y),
                    grab: c,
                    keyboard: true,
                },
                KeyOp::Scale => {
                    let s = map.to_screen(c);
                    let d = (pos2(s.0, s.1) - p).length();
                    EditOp::Scale {
                        map,
                        center: c,
                        from_dist: if d < SCALE_NEAR_PX { SCALE_REF_PX } else { d },
                    }
                }
            };
            editor.start_model_op(op);
        }
        let Some(op) = editor.model_edit().op else {
            return;
        };
        if let Some(p) = pointer {
            editor.update_model_op((p.x, p.y), modifiers.command, modifiers.shift);
        }
        let keyboard = matches!(
            op,
            EditOp::Move { keyboard: true, .. } | EditOp::Scale { .. }
        );
        if !down && !keyboard {
            editor.finish_model_op();
        }
    }

    /// Ctrl + rueda sobre una vista orto: zoom en las tres. `true` si fue de una vista orto.
    pub(super) fn model_edit_wheel(&mut self, editor: &mut Editor, p: Pos2, steps: i32) -> bool {
        let Some(view) = self.quadrant_at(p).and_then(quadrant_view) else {
            return false;
        };
        let Some(map) = self.edit_view_map(editor, view) else {
            return false;
        };
        editor.zoom_model_views(&map, (p.x, p.y), steps);
        true
    }

    /// Los dos paneles del modo Texture (arriba el modelo, abajo la textura), para los tests.
    #[cfg(test)]
    pub(crate) fn texture_panes(&self) -> Option<(Rect, Rect)> {
        self.texture_panes
    }

    #[cfg(test)]
    pub(crate) fn model_edit_quadrants(&self) -> Option<[Rect; 4]> {
        self.edit_quads
    }

    /// La textura del render de la perspectiva, para los tests de la app.
    #[cfg(test)]
    pub(crate) fn model_persp_texture(&self) -> Option<egui::TextureId> {
        self.edit_persp.as_ref().map(|(_, t)| t.id())
    }

    /// La clave del render de la perspectiva (cambia cuando se vuelve a renderizar).
    #[cfg(test)]
    pub(crate) fn model_persp_key(&self) -> Option<u64> {
        self.edit_persp.as_ref().map(|(k, _)| *k)
    }

    /// Las texturas de las vistas orto (Front, Top, Side) y su clave, para los tests de la app.
    #[cfg(test)]
    pub(crate) fn model_ortho_textures(&self) -> [Option<(u64, egui::TextureId)>; 3] {
        self.edit_ortho
            .each_ref()
            .map(|o| o.as_ref().map(|(k, t)| (*k, t.id())))
    }

    /// Cómo se ve el mundo en el cuadrante de `view` (con el centro y el zoom del modo Edit, en
    /// puntos por 1/16; el zoom del lienzo no cuenta). El centro de la vista cae en un punto
    /// entero, como los pixeles de la imagen de la vista.
    pub(crate) fn edit_view_map(&self, editor: &Editor, view: OrthoView) -> Option<ViewMap> {
        let q = self.edit_quads?[quadrant_index(view)];
        let e = editor.model_edit();
        Some(ViewMap {
            view,
            center: e.center,
            scale: e.zoom,
            origin: (q.center().x.floor(), q.center().y.floor()),
        })
    }

    /// Los cuatro cuadrantes `quads` (los de [`quadrants`] del área del workspace). Las vistas
    /// orto son pixel art: cada una es una imagen de su tamaño en puntos (un pixel por punto),
    /// dibujada con NEAREST y cacheada mientras no cambie lo que muestra.
    fn draw_model_edit(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        quads: [Rect; 4],
        editor: &Editor,
        doc: &Document,
    ) {
        let ctx = ui.ctx();
        let area = quads[FRONT].union(quads[SIDE]);
        let Some(model) = editor.edit_model_shown() else {
            return;
        };
        for (i, q) in quads.iter().enumerate() {
            painter.rect_filled(*q, 0.0, if i == PERSP { PERSP_BG } else { ORTHO_BG });
        }
        self.draw_persp(painter, quads[PERSP], editor, doc, model, ctx);
        // La caja de la selección y las aristas de las caras elegidas, una vez por frame para
        // las tres vistas.
        let sel = &editor.model_edit().selection;
        let bounds = cs_3d::bounds(model, sel);
        let face_edges = selected_face_edges(model, sel);
        let grid = editor.model_edit().grid;
        for (k, view) in OrthoView::ALL.into_iter().enumerate() {
            let q = quads[quadrant_index(view)];
            let Some(map) = self.edit_view_map(editor, view) else {
                continue;
            };
            // La imagen en puntos enteros: el pixel `(x, y)` es el punto `min + (x, y)`.
            let min = q.min.round();
            let size = q.max.round() - min;
            let (w, h) = (size.x.max(0.0) as usize, size.y.max(0.0) as usize);
            if w == 0 || h == 0 {
                continue;
            }
            let boxed = box_ends(editor, view);
            let mut hasher = DefaultHasher::new();
            (model, sel, grid, view, w, h).hash(&mut hasher);
            [
                min.x,
                min.y,
                map.origin.0,
                map.origin.1,
                map.scale,
                map.center[0],
                map.center[1],
                map.center[2],
            ]
            .map(f32::to_bits)
            .hash(&mut hasher);
            boxed
                .map(|(a, b)| [a.0, a.1, b.0, b.1].map(f32::to_bits))
                .hash(&mut hasher);
            let key = hasher.finish();
            if self.edit_ortho[k]
                .as_ref()
                .is_none_or(|(old, _)| *old != key)
            {
                let img = OrthoScene {
                    map: &map,
                    min: (min.x, min.y),
                    grid,
                    model,
                    sel,
                    bounds,
                    face_edges: &face_edges,
                    boxed,
                }
                .image(w, h);
                let pixels = img
                    .pixels
                    .iter()
                    .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
                    .collect();
                let image = egui::ColorImage::new([w, h], pixels);
                let opts = egui::TextureOptions::NEAREST;
                match &mut self.edit_ortho[k] {
                    Some((old, t)) => {
                        t.set(image, opts);
                        *old = key;
                    }
                    None => {
                        let name = format!("model_edit_ortho_{k}");
                        self.edit_ortho[k] = Some((key, ctx.load_texture(name, image, opts)));
                    }
                }
            }
            if let Some((_, t)) = &self.edit_ortho[k] {
                let full = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
                let p = painter.with_clip_rect(q.intersect(painter.clip_rect()));
                p.image(t.id(), Rect::from_min_size(min, size), full, Color32::WHITE);
            }
        }
        // Separadores de 1 punto (la primera columna y la primera fila de los cuadrantes de la
        // derecha y de abajo) y los nombres, en la fuente pixel y en puntos enteros.
        let mid_x = quads[FRONT].right().round();
        let mid_y = quads[FRONT].bottom().round();
        painter.rect_filled(
            Rect::from_x_y_ranges(mid_x..=mid_x + 1.0, area.y_range()),
            0.0,
            SEPARATOR,
        );
        painter.rect_filled(
            Rect::from_x_y_ranges(area.x_range(), mid_y..=mid_y + 1.0),
            0.0,
            SEPARATOR,
        );
        for (i, label) in [
            (FRONT, "Front"),
            (PERSP, "Persp"),
            (TOP, "Top"),
            (SIDE, "Side"),
        ] {
            let at = quads[i].min.round();
            let (text, _) = crate::pixel_draw::caps_text(ui, label, at.x + 3.0, at.y + 3.0, EDGE);
            painter.add(text);
        }
    }

    /// La perspectiva: el render pixel del modelo a la resolución de su artboard, a escala entera
    /// y centrado en el cuadrante; cacheado mientras no cambien el modelo, el documento ni el
    /// tamaño. En modo Texture lleva encima el contorno amarillo de las caras elegidas, en
    /// pixeles del render.
    pub(super) fn draw_persp(
        &mut self,
        painter: &egui::Painter,
        q: Rect,
        editor: &Editor,
        doc: &Document,
        model: &cs_core::Model3D,
        ctx: &egui::Context,
    ) {
        let Some(artboard) = doc.artboard(editor.model_id()) else {
            return;
        };
        let sprite = &artboard.sprite;
        let size = (sprite.width.max(1), sprite.height.max(1));
        let faces = if editor.cad_mode() == CadMode::Texture {
            editor.selected_faces()
        } else {
            Vec::new()
        };
        // Con `revision`: lo que se pinta en la textura se ve en vivo. A mitad de un trazo ni la
        // revisión ni la versión cambian: mientras se pinta se renderiza en cada pasada (`doc` es
        // la vista con el trazo en curso).
        let live = editor.is_painting().then(|| ctx.cumulative_pass_nr());
        let mut h = DefaultHasher::new();
        (
            model,
            editor.doc_version,
            editor.revision,
            size,
            artboard.id,
            &faces,
            live,
        )
            .hash(&mut h);
        let key = h.finish();
        if self.edit_persp.as_ref().is_none_or(|(k, _)| *k != key) {
            let mut pixels = cs_render::model_render(doc, artboard, model, size);
            if !faces.is_empty() {
                // El contorno depende del modelo, el tamaño y las caras, no de la textura: al
                // pintar se reusa.
                let mut h = DefaultHasher::new();
                (model, size, &faces).hash(&mut h);
                let outline_key = h.finish();
                if self
                    .edit_outline
                    .as_ref()
                    .is_none_or(|(k, _)| *k != outline_key)
                {
                    let outline = cs_render::model_face_outline(model, size, &faces);
                    self.edit_outline = Some((outline_key, outline));
                }
                let outline = self.edit_outline.as_ref().map_or(&[][..], |(_, o)| &o[..]);
                let [r, g, b, _] = SELECTED.to_srgba_unmultiplied();
                let yellow = cs_core::Rgba::new(r, g, b, 255);
                for (p, &on) in pixels.iter_mut().zip(outline) {
                    if on {
                        *p = yellow;
                    }
                }
            }
            let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
            let image =
                egui::ColorImage::from_rgba_unmultiplied([size.0 as usize, size.1 as usize], &rgba);
            let opts = egui::TextureOptions::NEAREST;
            match &mut self.edit_persp {
                Some((k, t)) => {
                    t.set(image, opts);
                    *k = key;
                }
                None => {
                    self.edit_persp = Some((key, ctx.load_texture("model_edit_persp", image, opts)))
                }
            }
        }
        if let Some((_, t)) = &self.edit_persp {
            let full = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
            let (r, _) = self.persp_rect(q, editor);
            // A escala 1 puede no caber: no se sale del cuadrante.
            let p = painter.with_clip_rect(q.intersect(painter.clip_rect()));
            p.image(t.id(), r, full, Color32::WHITE);
        }
    }
}

/// El área del workspace en `rect` (todo el lienzo): lo que se ve a la izquierda del panel del
/// dock abierto (con el panel cerrado, todo).
pub(super) fn workspace_area(ui: &egui::Ui, rect: Rect) -> Rect {
    let mut area = rect;
    if let Some(panel) = crate::dock::open_flyout_rect(ui.ctx())
        && panel.intersects(rect)
    {
        area.max.x = panel.left().floor().clamp(rect.left() + 1.0, rect.right());
    }
    area
}

/// El sprite del modelo que se edita (el del workspace, aunque el activo sea su textura).
pub(super) fn model_sprite(editor: &Editor) -> &cs_core::Sprite {
    editor
        .document
        .artboard(editor.model_id())
        .map_or_else(|| editor.sprite(), |a| &a.sprite)
}

/// Aire entre los botones Model / Texture y la esquina de arriba a la derecha del área.
const MODE_AIR: f32 = 3.0;

/// Las opciones del grupo de los modos.
fn mode_items() -> [crate::kit::group::GroupItem<'static>; 2] {
    use crate::kit::group::GroupItem;
    [GroupItem::text("Model"), GroupItem::text("Texture")]
}

/// Dónde van los botones Model / Texture en `area` (arriba a la derecha). Los gestos de las
/// vistas de debajo no empiezan ahí: el clic es de los botones.
pub(super) fn mode_buttons_rect(ui: &egui::Ui, area: Rect) -> Rect {
    let size = match crate::pixel_ui::frame_skin(ui.ctx()) {
        Some(_) => vec2(
            crate::kit::group::width(ui, &mode_items()),
            crate::kit::button::HEIGHT,
        ),
        None => vec2(120.0, 20.0),
    };
    let at = pos2(area.right() - MODE_AIR - size.x, area.top() + MODE_AIR).round();
    Rect::from_min_size(at, size)
}

/// Los botones **Model** y **Texture** (un grupo del kit; el del modo actual, presionado)
/// arriba a la derecha de `area`, en los dos modos. Un clic cambia de modo.
pub(super) fn mode_buttons(ui: &mut egui::Ui, area: Rect, editor: &mut Editor) {
    use crate::kit::group;
    let items = mode_items();
    let modes = [CadMode::Model, CadMode::Texture];
    let current = editor.cad_mode();
    let id = egui::Id::new("cad_mode_buttons");
    let rect = mode_buttons_rect(ui, area);
    let chosen = match crate::pixel_ui::frame_skin(ui.ctx()) {
        Some(skin) => {
            let painter = ui.painter().clone();
            let active = |i: usize| modes[i] == current;
            group::button_group_at(ui, &skin, &painter, id, rect.min, &items, &active)
        }
        // Sin Pixel UI, dos botones de texto de egui que se pueden elegir.
        None => {
            let mut chosen = None;
            let builder = egui::UiBuilder::new()
                .id_salt(id)
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center));
            ui.scope_builder(builder, |ui| {
                for (i, item) in items.iter().enumerate() {
                    if ui
                        .selectable_label(modes[i] == current, item.label)
                        .clicked()
                    {
                        chosen = Some(i);
                    }
                }
            });
            chosen
        }
    };
    if let Some(i) = chosen {
        editor.set_cad_mode(modes[i]);
    }
}

/// Los extremos (en pantalla) de la caja de selección en curso en la vista `view`.
fn box_ends(editor: &Editor, view: OrthoView) -> Option<((f32, f32), (f32, f32))> {
    match editor.model_edit().op {
        Some(EditOp::Box { map, from, to, .. }) if map.view == view => Some((from, to)),
        _ => None,
    }
}

/// Un vértice elegido bajo `at`: en modo Vértice, uno de los elegidos; en Arista, el extremo de
/// una arista elegida. Su punto en el mundo (el que se agarra al moverlo).
fn selected_vertex_at(
    model: &cs_core::Model3D,
    map: &ViewMap,
    sel: &cs_3d::Selection,
    at: (f32, f32),
) -> Option<cs_3d::P3> {
    if !matches!(sel.mode, SelectMode::Vertex | SelectMode::Edge) {
        return None;
    }
    let Some(Item::Vertex(o, v)) = cs_3d::pick(model, map, SelectMode::Vertex, at) else {
        return None;
    };
    let on = match sel.mode {
        SelectMode::Vertex => sel.items.contains(&Item::Vertex(o, v)),
        _ => sel
            .items
            .iter()
            .any(|i| matches!(*i, Item::Edge(eo, a, b) if eo == o && (a == v || b == v))),
    };
    on.then(|| cs_3d::world_points(&model.objects[o])[v as usize])
}

/// Las aristas `(objeto, a, b)` (con `a < b`) de las caras elegidas, para pintarlas sin recorrer
/// la selección por cada arista.
fn selected_face_edges(
    model: &cs_core::Model3D,
    sel: &cs_3d::Selection,
) -> HashSet<(usize, u32, u32)> {
    let mut out = HashSet::new();
    for item in &sel.items {
        if let Item::Face(o, f) = *item
            && let Some(face) = model.objects.get(o).and_then(|obj| obj.mesh.faces.get(f))
        {
            let vs = &face.verts;
            for k in 0..vs.len() {
                let (x, y) = (vs[k], vs[(k + 1) % vs.len()]);
                out.insert((o, x.min(y), x.max(y)));
            }
        }
    }
    out
}

/// Lo que muestra una vista orto, para pintarla en pixel art.
struct OrthoScene<'a> {
    map: &'a ViewMap,
    /// El punto de pantalla del pixel `(0, 0)` (entero).
    min: (f32, f32),
    grid: i32,
    model: &'a cs_core::Model3D,
    sel: &'a cs_3d::Selection,
    bounds: Option<(cs_3d::P3, cs_3d::P3)>,
    face_edges: &'a HashSet<(usize, u32, u32)>,
    boxed: Option<((f32, f32), (f32, f32))>,
}

/// El color como pixel de la imagen.
fn rgba(c: Color32) -> cs_3d::Rgba {
    c.to_srgba_unmultiplied()
}

impl OrthoScene<'_> {
    /// El pixel donde cae el punto de pantalla `s`: el punto entero que lo contiene (el mismo
    /// redondeo para todo, así lo dibujado cae donde se elige).
    fn px(&self, s: (f32, f32)) -> (i32, i32) {
        (
            (s.0 - self.min.0).floor() as i32,
            (s.1 - self.min.1).floor() as i32,
        )
    }

    /// La imagen de `w × h`: el fondo, la cuadrícula, la cruz punteada por el centro de la
    /// selección, las aristas (las elegidas encima), los cuadritos de los vértices o de las
    /// caras, las asas y la caja de selección en curso.
    fn image(&self, w: usize, h: usize) -> cs_3d::WireImage {
        let mut img = cs_3d::WireImage::new(w, h, rgba(ORTHO_BG));
        self.grid_lines(&mut img);
        if let Some(b) = self.bounds {
            let (cx, cy) = self.px(self.map.to_screen(cs_3d::center(b)));
            let guide = rgba(GUIDE);
            img.dashed_line(cx, 0, cx, h as i32 - 1, guide);
            img.dashed_line(0, cy, w as i32 - 1, cy, guide);
        }
        self.wireframe(&mut img);
        self.handles(&mut img);
        if let Some((a, b)) = self.boxed {
            let ((x0, y0), (x1, y1)) = (self.px(a), self.px(b));
            img.rect_outline(x0, y0, x1, y1, rgba(SELECTED));
        }
        img
    }

    /// Las líneas de la cuadrícula (cada `grid` 1/16) que caen en la imagen, si no quedan muy
    /// juntas. Con la cuenta de [`ViewMap::to_screen`]: un vértice sobre la cuadrícula cae en su
    /// línea.
    fn grid_lines(&self, img: &mut cs_3d::WireImage) {
        let map = self.map;
        let step = self.grid.max(1) as f32;
        if step * map.scale < GRID_MIN_PX {
            return;
        }
        let (w, h) = (img.width as i32, img.height as i32);
        let (u0, v1) = map.to_plane(self.min);
        let (u1, v0) = map.to_plane((self.min.0 + w as f32, self.min.1 + h as f32));
        let (cu, cv) = map.view.plane(map.center);
        let color = rgba(over(ORTHO_BG, GRID_LINE));
        let mut u = (u0 / step).floor() * step;
        while u <= u1 {
            let (x, _) = self.px((map.origin.0 + (u - cu) * map.scale, self.min.1));
            img.line(x, 0, x, h - 1, color);
            u += step;
        }
        let mut v = (v0 / step).floor() * step;
        while v <= v1 {
            let (_, y) = self.px((self.min.0, map.origin.1 - (v - cv) * map.scale));
            img.line(0, y, w - 1, y, color);
            v += step;
        }
    }

    /// Todas las aristas, las elegidas en amarillo y encima: en modo Objeto, todas las aristas
    /// del objeto; en Arista, las suyas; en Cara, las de la cara (`face_edges`) y su punto
    /// central; en Vértice, los cuadritos. Los vértices se ven como cuadritos fuera del modo
    /// Objeto.
    fn wireframe(&self, img: &mut cs_3d::WireImage) {
        let (sel, model) = (self.sel, self.model);
        let scr: Vec<Vec<(f32, f32)>> = model
            .objects
            .iter()
            .map(|obj| {
                cs_3d::world_points(obj)
                    .iter()
                    .map(|&w| self.map.to_screen(w))
                    .collect()
            })
            .collect();
        for selected in [false, true] {
            for (o, obj) in model.objects.iter().enumerate() {
                let whole = sel.items.contains(&Item::Object(o));
                for (a, b) in cs_3d::mesh_edges(&obj.mesh) {
                    let on = whole
                        || sel.items.contains(&Item::Edge(o, a, b))
                        || self.face_edges.contains(&(o, a, b));
                    if on != selected {
                        continue;
                    }
                    if let (Some(&pa), Some(&pb)) = (scr[o].get(a as usize), scr[o].get(b as usize))
                    {
                        let ((x0, y0), (x1, y1)) = (self.px(pa), self.px(pb));
                        img.line(x0, y0, x1, y1, rgba(if on { SELECTED } else { EDGE }));
                    }
                }
            }
        }
        let side = VERTEX_PX as i32;
        let dot = |img: &mut cs_3d::WireImage, s: (f32, f32), c: Color32| {
            let (x, y) = self.px(s);
            img.fill_rect(x - side / 2, y - side / 2, side, side, rgba(c));
        };
        for (o, obj) in model.objects.iter().enumerate() {
            match sel.mode {
                SelectMode::Object => {}
                SelectMode::Face => {
                    for (f, face) in obj.mesh.faces.iter().enumerate() {
                        let vs: Vec<(f32, f32)> = face
                            .verts
                            .iter()
                            .filter_map(|&v| scr[o].get(v as usize).copied())
                            .collect();
                        if vs.is_empty() {
                            continue;
                        }
                        let k = vs.len() as f32;
                        let c = (
                            vs.iter().map(|p| p.0).sum::<f32>() / k,
                            vs.iter().map(|p| p.1).sum::<f32>() / k,
                        );
                        let on = sel.items.contains(&Item::Face(o, f));
                        dot(img, c, if on { SELECTED } else { EDGE });
                    }
                }
                _ => {
                    for (v, &s) in scr[o].iter().enumerate() {
                        let on = sel.items.contains(&Item::Vertex(o, v as u32));
                        dot(img, s, if on { SELECTED } else { SEPARATOR });
                    }
                }
            }
        }
    }

    /// Las asas de la caja de la selección (no con un solo vértice), solo las que pueden cambiar
    /// algo en esta vista.
    fn handles(&self, img: &mut cs_3d::WireImage) {
        let Some(b) = self.bounds else {
            return;
        };
        if self.sel.mode == SelectMode::Vertex && self.sel.items.len() == 1 {
            return;
        }
        let side = HANDLE_PX as i32;
        for h in Handle::ALL
            .into_iter()
            .filter(|&h| useful_handle(self.map.view, b, h))
        {
            let s = handle_pos(self.map, b, h);
            let (x, y) = self.px((s.x, s.y));
            img.fill_rect(x - side / 2, y - side / 2, side, side, rgba(SELECTED));
        }
    }
}

/// `top` (premultiplicado, con transparencia) encima del color opaco `bg`.
fn over(bg: Color32, top: Color32) -> Color32 {
    let k = 255 - top.a() as u32;
    let mix = |b: u8, t: u8| (t as u32 + (b as u32 * k + 127) / 255).min(255) as u8;
    Color32::from_rgb(
        mix(bg.r(), top.r()),
        mix(bg.g(), top.g()),
        mix(bg.b(), top.b()),
    )
}

/// Si el asa `h` puede cambiar algo: no si sus ejes no tienen medida en la vista (una arista
/// recta o una cara de canto juntan esas asas en su mitad, y taparían arrastrarla de ahí).
fn useful_handle(view: OrthoView, b: (cs_3d::P3, cs_3d::P3), h: Handle) -> bool {
    let ((u0, v0), (u1, v1)) = cs_3d::plane_box(view, b);
    !((h.u == 0 || u0 == u1) && (h.v == 0 || v0 == v1))
}

/// Dónde se ve el asa `h` de la caja `b` (en pantalla).
pub(crate) fn handle_pos(map: &ViewMap, b: (cs_3d::P3, cs_3d::P3), h: Handle) -> Pos2 {
    let ((u0, v0), (u1, v1)) = cs_3d::plane_box(map.view, b);
    let pick = |s: i8, lo: f32, hi: f32| match s {
        -1 => lo,
        1 => hi,
        _ => (lo + hi) / 2.0,
    };
    let w = map
        .view
        .from_plane((pick(h.u, u0, u1), pick(h.v, v0, v1)), map.center);
    pos(map.to_screen(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{Model3D, Object3D};

    fn cube() -> Model3D {
        Model3D {
            objects: vec![Object3D {
                name: "Cube".into(),
                mesh: cs_3d::primitive_mesh(&cs_3d::Primitive::Cube),
                transform: Default::default(),
                double_sided: false,
            }],
            ..Model3D::default()
        }
    }

    /// Front, centrada en el origen, 2 puntos por 1/16 y el centro en el punto (50, 50): el
    /// cubo (de −16 a 16) cae de 18 a 82.
    const MAP: ViewMap = ViewMap {
        view: OrthoView::Front,
        center: [0.0; 3],
        scale: 2.0,
        origin: (50.0, 50.0),
    };

    fn render(model: &Model3D, sel: &cs_3d::Selection, grid: i32) -> cs_3d::WireImage {
        render_with(model, sel, grid, None)
    }

    fn render_with(
        model: &Model3D,
        sel: &cs_3d::Selection,
        grid: i32,
        boxed: Option<((f32, f32), (f32, f32))>,
    ) -> cs_3d::WireImage {
        OrthoScene {
            map: &MAP,
            min: (0.0, 0.0),
            grid,
            model,
            sel,
            bounds: cs_3d::bounds(model, sel),
            face_edges: &selected_face_edges(model, sel),
            boxed,
        }
        .image(100, 100)
    }

    fn at(img: &cs_3d::WireImage, x: i32, y: i32) -> Color32 {
        let c = img.get(x, y).expect("in the image");
        Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
    }

    #[test]
    fn the_edges_are_one_pixel_lines_on_the_point_grid() {
        let m = cube();
        let img = render(&m, &cs_3d::Selection::default(), 1);
        // El borde de arriba, 1 pixel de alto y sin suavizado.
        assert_eq!(at(&img, 40, 18), EDGE);
        assert_eq!(at(&img, 40, 17), ORTHO_BG);
        assert_eq!(at(&img, 40, 19), ORTHO_BG);
        // En modo Objeto, sin cuadritos: la esquina es de la arista.
        assert_eq!(at(&img, 18, 18), EDGE);
        assert_eq!(at(&img, 17, 17), ORTHO_BG);
        // Ningún color a medias: solo los de la paleta de la vista.
        let ok = [EDGE, ORTHO_BG].map(rgba);
        assert!(img.pixels.iter().all(|c| ok.contains(c)));
    }

    #[test]
    fn the_grid_skips_close_lines_and_falls_on_whole_points() {
        let m = Model3D::default();
        // Cada 1/16 son 2 puntos: muy juntas.
        let none = render(&m, &cs_3d::Selection::default(), 1);
        assert!(none.pixels.iter().all(|c| *c == rgba(ORTHO_BG)));
        // Cada 4/16 son 8 puntos: columnas 2, 10, …, 50, … (por el centro en 50).
        let img = render(&m, &cs_3d::Selection::default(), 4);
        let line = rgba(over(ORTHO_BG, GRID_LINE));
        assert_ne!(line, rgba(ORTHO_BG));
        for x in [2, 10, 50, 98] {
            assert_eq!(img.get(x, 33), Some(line), "column {x}");
        }
        for x in [1, 3, 49, 51] {
            assert_eq!(img.get(x, 33), Some(rgba(ORTHO_BG)), "column {x}");
        }
        // Y las filas, igual hacia arriba y hacia abajo del centro.
        for y in [2, 42, 50, 58, 98] {
            assert_eq!(img.get(33, y), Some(line), "row {y}");
        }
        assert_eq!(img.get(33, 51), Some(rgba(ORTHO_BG)));
    }

    #[test]
    fn vertices_handles_and_guides_are_pixel_blocks() {
        let m = cube();
        // Un vértice elegido: su cuadrito de 3 × 3 amarillo, sin asas, con la cruz punteada.
        let one = cs_3d::Selection {
            mode: SelectMode::Vertex,
            items: [Item::Vertex(0, 7)].into(),
        };
        let p = MAP.to_screen(cs_3d::world_points(&m.objects[0])[7]);
        let (x, y) = (p.0.floor() as i32, p.1.floor() as i32);
        let img = render(&m, &one, 1);
        for (dx, dy) in [(-1, -1), (0, 0), (1, 1), (1, -1)] {
            assert_eq!(at(&img, x + dx, y + dy), SELECTED);
        }
        assert_ne!(at(&img, x + 2, y + 2), SELECTED);
        // Los otros vértices, en el color de los separadores.
        assert_eq!(at(&img, 17, 83), SEPARATOR);
        // La cruz por el vértice: 2 sí y 2 no desde el borde.
        let (gx, gy) = (x, y);
        assert_eq!(at(&img, gx, 0), GUIDE);
        assert_eq!(at(&img, gx, 1), GUIDE);
        assert_eq!(at(&img, gx, 2), ORTHO_BG);
        assert_eq!(at(&img, 0, gy), GUIDE);
        assert_eq!(at(&img, 2, gy), ORTHO_BG);
        // Todo el objeto: aristas amarillas y asas de 5 × 5 en las esquinas y los lados.
        let all = cs_3d::Selection {
            mode: SelectMode::Object,
            items: [Item::Object(0)].into(),
        };
        let img = render(&m, &all, 1);
        assert_eq!(at(&img, 40, 18), SELECTED);
        for (hx, hy) in [(16, 16), (20, 20), (48, 80), (52, 84), (84, 50)] {
            assert_eq!(at(&img, hx, hy), SELECTED, "handle at {hx}, {hy}");
        }
        assert_ne!(at(&img, 15, 15), SELECTED);
        assert_ne!(at(&img, 21, 21), SELECTED);
    }

    #[test]
    fn the_box_select_is_a_one_pixel_outline() {
        let m = Model3D::default();
        let sel = cs_3d::Selection::default();
        let img = render_with(&m, &sel, 1, Some(((30.2, 20.7), (10.4, 10.9))));
        assert_eq!(at(&img, 10, 10), SELECTED);
        assert_eq!(at(&img, 30, 20), SELECTED);
        assert_eq!(at(&img, 20, 10), SELECTED);
        assert_eq!(at(&img, 11, 11), ORTHO_BG);
        assert_eq!(at(&img, 31, 20), ORTHO_BG);
    }
}
