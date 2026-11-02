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
use crate::editor::{EditOp, KeyOp};

pub(crate) const FRONT: usize = 0;
pub(crate) const PERSP: usize = 1;
pub(crate) const TOP: usize = 2;
pub(crate) const SIDE: usize = 3;

/// Colores provisionales (los de picoCAD): fondo de las vistas orto, de la perspectiva, aristas,
/// cuadrícula y separadores.
const ORTHO_BG: Color32 = Color32::from_rgb(0x83, 0x76, 0x9c);
const PERSP_BG: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
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
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        modal: bool,
    ) {
        // Los cuadrantes salen del área, así que ya valen para la entrada de este frame (también
        // en el primero del workspace).
        let quads = quadrants(rect);
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
        self.draw_model_edit(&painter, rect, editor, view.document.as_ref(), ui.ctx());
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
        {
            self.model_edit_press(editor, p, secondary, middle, modifiers);
        }
        self.model_drag_input(editor, pointer, any_down);
        self.model_edit_input(editor, pointer, edit_down, modifiers);
    }

    /// Dónde va el render de la perspectiva en el cuadrante `q` (a la resolución del artboard,
    /// a la escala entera más grande que quepa, mínimo 1, y centrado) y cuántos puntos mide cada
    /// pixel del render (el dibujo y los gestos usan lo mismo).
    fn persp_rect(&self, q: Rect, editor: &Editor) -> (Rect, Vec2) {
        let sprite = editor.sprite();
        let pixels = vec2(
            sprite.width.max(1) as f32 * self.ratio.x,
            sprite.height.max(1) as f32 * self.ratio.y,
        );
        let k = (q.width() / pixels.x)
            .min(q.height() / pixels.y)
            .floor()
            .max(1.0);
        let size = pixels * k;
        // En pixeles físicos enteros.
        let ppp = self.ppp.max(1.0);
        let min = ((q.center() - size / 2.0).to_vec2() * ppp).round() / ppp;
        (Rect::from_min_size(min.to_pos2(), size), self.ratio * k)
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
        let Some(model) = editor.model().cloned() else {
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
                if !sel.items.contains(&item) {
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
                .model()
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

    #[cfg(test)]
    pub(crate) fn model_edit_quadrants(&self) -> Option<[Rect; 4]> {
        self.edit_quads
    }

    /// La textura del render de la perspectiva, para los tests de la app.
    #[cfg(test)]
    pub(crate) fn model_persp_texture(&self) -> Option<egui::TextureId> {
        self.edit_persp.as_ref().map(|(_, t)| t.id())
    }

    /// Cómo se ve el mundo en el cuadrante de `view` (con el centro y el zoom del modo Edit, en
    /// puntos por 1/16; el zoom del lienzo no cuenta).
    pub(crate) fn edit_view_map(&self, editor: &Editor, view: OrthoView) -> Option<ViewMap> {
        let q = self.edit_quads?[quadrant_index(view)];
        let e = editor.model_edit();
        Some(ViewMap {
            view,
            center: e.center,
            scale: e.zoom,
            origin: (q.center().x, q.center().y),
        })
    }

    /// Los cuatro cuadrantes sobre `area` (todo el lienzo del workspace).
    fn draw_model_edit(
        &mut self,
        painter: &egui::Painter,
        area: Rect,
        editor: &Editor,
        doc: &Document,
        ctx: &egui::Context,
    ) {
        let quads = quadrants(area);
        self.edit_quads = Some(quads);
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
        for view in OrthoView::ALL {
            let q = quads[quadrant_index(view)];
            let Some(map) = self.edit_view_map(editor, view) else {
                continue;
            };
            let p = painter.with_clip_rect(q.intersect(painter.clip_rect()));
            draw_grid_lines(&p, q, &map, editor.model_edit().grid);
            draw_wireframe(&p, &map, model, sel, &face_edges, SELECTED);
            draw_selection_frame(&p, q, &map, sel, bounds, SELECTED);
            if let Some(cs_3d_box) = box_rect(editor, view) {
                p.rect_stroke(
                    cs_3d_box,
                    0.0,
                    Stroke::new(1.0, SELECTED),
                    egui::StrokeKind::Inside,
                );
            }
        }
        // Separadores de 1 punto y los nombres.
        let mid_x = quads[FRONT].right();
        let mid_y = quads[FRONT].bottom();
        painter.vline(mid_x, area.y_range(), Stroke::new(1.0, SEPARATOR));
        painter.hline(area.x_range(), mid_y, Stroke::new(1.0, SEPARATOR));
        let font = egui::TextStyle::Body.resolve(&ctx.global_style());
        for (i, label) in [
            (FRONT, "Front"),
            (PERSP, "Persp"),
            (TOP, "Top"),
            (SIDE, "Side"),
        ] {
            painter.text(
                quads[i].min + vec2(3.0, 2.0),
                egui::Align2::LEFT_TOP,
                label,
                font.clone(),
                EDGE,
            );
        }
    }

    /// La perspectiva: el render pixel del modelo a la resolución del artboard, a escala entera y
    /// centrado en el cuadrante; cacheado mientras no cambien el modelo, el documento ni el
    /// tamaño.
    fn draw_persp(
        &mut self,
        painter: &egui::Painter,
        q: Rect,
        editor: &Editor,
        doc: &Document,
        model: &cs_core::Model3D,
        ctx: &egui::Context,
    ) {
        let size = (editor.sprite().width.max(1), editor.sprite().height.max(1));
        let mut h = DefaultHasher::new();
        (model, editor.doc_version, size, editor.active).hash(&mut h);
        let key = h.finish();
        if self.edit_persp.as_ref().is_none_or(|(k, _)| *k != key) {
            let pixels = cs_render::model_render(doc, editor.artboard(), model, size);
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

/// La caja de selección en curso en la vista `view` (en pantalla).
fn box_rect(editor: &Editor, view: OrthoView) -> Option<Rect> {
    match editor.model_edit().op {
        Some(EditOp::Box { map, from, to, .. }) if map.view == view => {
            Some(Rect::from_two_pos(pos(from), pos(to)))
        }
        _ => None,
    }
}

/// Las líneas de la cuadrícula (cada `grid` 1/16) que caen en el cuadrante, si no quedan muy
/// juntas.
fn draw_grid_lines(p: &egui::Painter, q: Rect, map: &ViewMap, grid: i32) {
    let step = grid.max(1) as f32;
    if step * map.scale < GRID_MIN_PX {
        return;
    }
    let (u0, v1) = map.to_plane((q.left(), q.top()));
    let (u1, v0) = map.to_plane((q.right(), q.bottom()));
    let stroke = Stroke::new(1.0, GRID_LINE);
    let mut u = (u0 / step).floor() * step;
    while u <= u1 {
        let x = map.origin.0 + (u - map.view.plane(map.center).0) * map.scale;
        p.vline(x.round() + 0.5, q.y_range(), stroke);
        u += step;
    }
    let mut v = (v0 / step).floor() * step;
    while v <= v1 {
        let y = map.origin.1 - (v - map.view.plane(map.center).1) * map.scale;
        p.hline(q.x_range(), y.round() + 0.5, stroke);
        v += step;
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

/// Todas las aristas; lo elegido en el color de acento: en modo Objeto, todas las aristas del
/// objeto; en Arista, las suyas; en Cara, las de la cara (`face_edges`) y su punto central; en
/// Vértice, los cuadritos. Los vértices se ven como cuadritos fuera del modo Objeto.
fn draw_wireframe(
    p: &egui::Painter,
    map: &ViewMap,
    model: &cs_core::Model3D,
    sel: &cs_3d::Selection,
    face_edges: &HashSet<(usize, u32, u32)>,
    accent: Color32,
) {
    for (o, obj) in model.objects.iter().enumerate() {
        let scr: Vec<Pos2> = cs_3d::world_points(obj)
            .iter()
            .map(|&w| pos(map.to_screen(w)))
            .collect();
        let whole = sel.items.contains(&Item::Object(o));
        for (a, b) in cs_3d::mesh_edges(&obj.mesh) {
            let on = whole
                || sel.items.contains(&Item::Edge(o, a, b))
                || face_edges.contains(&(o, a, b));
            let color = if on { accent } else { EDGE };
            p.line_segment([scr[a as usize], scr[b as usize]], Stroke::new(1.0, color));
        }
        match sel.mode {
            SelectMode::Object => {}
            SelectMode::Face => {
                for (f, face) in obj.mesh.faces.iter().enumerate() {
                    let vs: Vec<Pos2> = face
                        .verts
                        .iter()
                        .filter_map(|&v| scr.get(v as usize).copied())
                        .collect();
                    if vs.is_empty() {
                        continue;
                    }
                    let c = vs.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / vs.len() as f32;
                    let on = sel.items.contains(&Item::Face(o, f));
                    p.rect_filled(
                        Rect::from_center_size(c.to_pos2(), Vec2::splat(VERTEX_PX)),
                        0.0,
                        if on { accent } else { EDGE },
                    );
                }
            }
            _ => {
                for (v, s) in scr.iter().enumerate() {
                    let on = sel.items.contains(&Item::Vertex(o, v as u32));
                    let r = Rect::from_center_size(*s, Vec2::splat(VERTEX_PX));
                    p.rect_filled(r, 0.0, if on { accent } else { SEPARATOR });
                }
            }
        }
    }
}

/// Las asas de la caja `bounds` de la selección (no con un solo vértice) y la cruz punteada por
/// su centro.
fn draw_selection_frame(
    p: &egui::Painter,
    q: Rect,
    map: &ViewMap,
    sel: &cs_3d::Selection,
    bounds: Option<(cs_3d::P3, cs_3d::P3)>,
    accent: Color32,
) {
    let Some(b) = bounds else {
        return;
    };
    let c = pos(map.to_screen(cs_3d::center(b)));
    let dash = Stroke::new(1.0, GUIDE);
    p.extend(egui::Shape::dashed_line(
        &[pos2(c.x, q.top()), pos2(c.x, q.bottom())],
        dash,
        2.0,
        2.0,
    ));
    p.extend(egui::Shape::dashed_line(
        &[pos2(q.left(), c.y), pos2(q.right(), c.y)],
        dash,
        2.0,
        2.0,
    ));
    if sel.mode == SelectMode::Vertex && sel.items.len() == 1 {
        return;
    }
    for h in Handle::ALL
        .into_iter()
        .filter(|&h| useful_handle(map.view, b, h))
    {
        let r = Rect::from_center_size(handle_pos(map, b, h), Vec2::splat(HANDLE_PX));
        p.rect_filled(r, 0.0, accent);
    }
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
