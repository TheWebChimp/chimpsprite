//! El modo Edit del artboard 3D en el lienzo (como picoCAD 2): el área del artboard se parte en
//! cuatro cuadrantes, Front | Persp arriba y Top | Side abajo. Las vistas ortográficas son
//! wireframes (todas las aristas) con la cuadrícula, la selección, su caja con asas y las guías
//! punteadas; la perspectiva es el render pixel a la mitad de resolución.

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
const GUIDE: Color32 = Color32::from_rgb(0x5f, 0x57, 0x4f);
const SEPARATOR: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x53);
/// Lado de los cuadritos de los vértices y de las asas, en puntos.
const VERTEX_PX: f32 = 3.0;
pub(crate) const HANDLE_PX: f32 = 5.0;
/// Con líneas más juntas que esto (en puntos) la cuadrícula no se dibuja.
const GRID_MIN_PX: f32 = 4.0;

/// Los cuadrantes del área `screen` de un artboard de `size` pixeles: Front, Persp, Top y Side.
/// Cortan en pixeles enteros del artboard (con ancho impar, la columna de más es de la derecha).
pub(crate) fn quadrants(screen: Rect, (w, h): (u32, u32)) -> [Rect; 4] {
    let sx = screen.width() / w.max(1) as f32;
    let sy = screen.height() / h.max(1) as f32;
    let x = screen.left() + (w / 2) as f32 * sx;
    let y = screen.top() + (h / 2) as f32 * sy;
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
    /// El tamaño en pixeles del render de la perspectiva en el cuadrante `q` (el dibujo y los
    /// gestos usan el mismo).
    fn persp_size(&self, q: Rect) -> (u32, u32) {
        (
            (q.width() / self.scale().x).round().max(1.0) as u32,
            (q.height() / self.scale().y).round().max(1.0) as u32,
        )
    }

    fn quadrant_at(&self, p: Pos2) -> Option<usize> {
        self.edit_quads?.iter().position(|q| q.contains(p))
    }

    /// Una prensa en el área del modelo en el modo Edit. En la perspectiva sigue lo de siempre
    /// (`model_press` con el punto en pixeles del artboard entero). `true` si la prensa fue del
    /// modo Edit.
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
        if !modifiers.shift
            && let Some(b) = cs_3d::bounds(&model, &sel)
            && !(sel.mode == SelectMode::Vertex && sel.items.len() == 1)
            && let Some(handle) = Handle::ALL
                .into_iter()
                .find(|&h| (handle_pos(&map, b, h) - p).length() <= HANDLE_PX)
        {
            editor.start_model_op(EditOp::Handle {
                map,
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

    /// La prensa en la perspectiva: como fuera del modo Edit, con el punto escalado del render
    /// del cuadrante al artboard entero.
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
        let (w, h) = (editor.sprite().width as f32, editor.sprite().height as f32);
        let (pw, ph) = self.persp_size(q);
        let (pw, ph) = (pw as f32, ph as f32);
        let local = (
            ((p.x - q.left()) / self.scale().x * w / pw).floor() as i32,
            ((p.y - q.top()) / self.scale().y * h / ph).floor() as i32,
        );
        self.model_press_at(editor, p, local, secondary, modifiers, h / ph);
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
                    EditOp::Scale {
                        map,
                        center: c,
                        from_dist: (pos2(s.0, s.1) - p).length().max(1.0),
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

    /// Cómo se ve el mundo en el cuadrante de `view` (con el centro y el zoom del modo Edit).
    pub(crate) fn edit_view_map(&self, editor: &Editor, view: OrthoView) -> Option<ViewMap> {
        let q = self.edit_quads?[quadrant_index(view)];
        let e = editor.model_edit();
        Some(ViewMap {
            view,
            center: e.center,
            scale: e.zoom * self.scale().x,
            origin: (q.center().x, q.center().y),
        })
    }

    pub(super) fn draw_model_edit(
        &mut self,
        painter: &egui::Painter,
        screen: Rect,
        editor: &Editor,
        doc: &Document,
        ctx: &egui::Context,
    ) {
        let sprite = editor.sprite();
        let quads = quadrants(screen, (sprite.width, sprite.height));
        self.edit_quads = Some(quads);
        let Some(model) = editor.edit_model_shown().cloned() else {
            return;
        };
        let accent =
            crate::pixel_ui::frame_skin(ctx).map_or(theme::ACCENT, |s| s.colors().selection);
        for (i, q) in quads.iter().enumerate() {
            painter.rect_filled(*q, 0.0, if i == PERSP { PERSP_BG } else { ORTHO_BG });
        }
        self.draw_persp(painter, quads[PERSP], editor, doc, &model, ctx);
        for view in OrthoView::ALL {
            let q = quads[quadrant_index(view)];
            let Some(map) = self.edit_view_map(editor, view) else {
                continue;
            };
            let p = painter.with_clip_rect(q.intersect(painter.clip_rect()));
            draw_grid_lines(&p, q, &map, editor.model_edit().grid);
            draw_wireframe(&p, &map, &model, &editor.model_edit().selection, accent);
            draw_selection_frame(&p, q, &map, &model, editor, accent);
            if let Some(cs_3d_box) = box_rect(editor, view) {
                p.rect_stroke(
                    cs_3d_box,
                    0.0,
                    Stroke::new(1.0, accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        // Separadores de 1 punto y los nombres.
        let mid_x = quads[FRONT].right();
        let mid_y = quads[FRONT].bottom();
        painter.vline(mid_x, screen.y_range(), Stroke::new(1.0, SEPARATOR));
        painter.hline(screen.x_range(), mid_y, Stroke::new(1.0, SEPARATOR));
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

    /// La perspectiva: el render pixel del modelo a la medida del cuadrante en pixeles del
    /// artboard (la mitad), cacheado mientras no cambien el modelo, el documento ni el tamaño.
    fn draw_persp(
        &mut self,
        painter: &egui::Painter,
        q: Rect,
        editor: &Editor,
        doc: &Document,
        model: &cs_core::Model3D,
        ctx: &egui::Context,
    ) {
        let size = self.persp_size(q);
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
            painter.image(t.id(), q, full, Color32::WHITE);
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

/// Todas las aristas; lo elegido en el color de acento: en modo Objeto, todas las aristas del
/// objeto; en Arista, las suyas; en Cara, las de la cara y su punto central; en Vértice, los
/// cuadritos. Los vértices se ven como cuadritos fuera del modo Objeto.
fn draw_wireframe(
    p: &egui::Painter,
    map: &ViewMap,
    model: &cs_core::Model3D,
    sel: &cs_3d::Selection,
    accent: Color32,
) {
    for (o, obj) in model.objects.iter().enumerate() {
        let scr: Vec<Pos2> = cs_3d::world_points(obj)
            .iter()
            .map(|&w| pos(map.to_screen(w)))
            .collect();
        let whole = sel.items.contains(&Item::Object(o));
        let face_edge = |a: u32, b: u32| {
            sel.items.iter().any(|i| match *i {
                Item::Face(fo, f) if fo == o => {
                    let vs = &obj.mesh.faces[f].verts;
                    (0..vs.len()).any(|k| {
                        let (x, y) = (vs[k], vs[(k + 1) % vs.len()]);
                        (x.min(y), x.max(y)) == (a, b)
                    })
                }
                _ => false,
            })
        };
        for (a, b) in cs_3d::mesh_edges(&obj.mesh) {
            let on = whole || sel.items.contains(&Item::Edge(o, a, b)) || face_edge(a, b);
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

/// Las asas de la caja de la selección (no con un solo vértice) y la cruz punteada por su centro.
fn draw_selection_frame(
    p: &egui::Painter,
    q: Rect,
    map: &ViewMap,
    model: &cs_core::Model3D,
    editor: &Editor,
    accent: Color32,
) {
    let sel = &editor.model_edit().selection;
    let Some(b) = cs_3d::bounds(model, sel) else {
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
    for h in Handle::ALL {
        let r = Rect::from_center_size(handle_pos(map, b, h), Vec2::splat(HANDLE_PX));
        p.rect_filled(r, 0.0, accent);
    }
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
