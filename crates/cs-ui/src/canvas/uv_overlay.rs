//! Las UVs de las caras elegidas sobre la textura del modo Texture (abajo): el contorno de cada
//! cara (blanco con borde oscuro de 1 punto), sus vértices y las asas de la caja de todas, en
//! puntos enteros (con zoom entero, cada pixel de la textura mide puntos enteros). Con la
//! herramienta Move, o con Alt y cualquier otra, arrastrar dentro de un contorno mueve las UVs,
//! un asa las escala (Shift: proporcional) y un vértice se mueve solo. Un clic sin arrastre no
//! cambia nada; cada gesto es un paso de undo.

use cs_core::Model3D;

use super::*;
use crate::editor::{DRAG_THRESHOLD, UvOp};
use crate::pixel_cursors::INK;

/// Aire entre la caja de las UVs y sus asas, en puntos (así las de las esquinas no tapan los
/// vértices).
const UV_HANDLE_GAP: f32 = 2.0;
/// Del punto de la caja (esquina o mitad de un lado) al centro de su asa, en puntos y por eje.
#[cfg(test)]
pub(crate) const UV_HANDLE_OFFSET: f32 = UV_HANDLE_GAP + pixel_selection::HANDLE / 2.0;
/// Lado del cuadrito de cada vértice UV, en puntos.
const UV_VERTEX_PX: i32 = 3;

/// Lo que hay bajo el puntero entre las UVs elegidas.
#[derive(Clone, Copy, Debug, PartialEq)]
enum UvHit {
    Vertex { face: (usize, usize), vertex: usize },
    Handle(cs_3d::Handle),
    Inside,
}

/// Un gesto de UV apretado: dónde (en pantalla), si ya pasó el umbral de arrastre, su cursor y
/// lo que se le suma al puntero (en pixeles de la textura) para que el asa agarrada caiga justo
/// en el borde de la caja.
#[derive(Clone, Copy, Debug)]
pub(super) struct UvPress {
    at: Pos2,
    moved: bool,
    cursor: egui::CursorIcon,
    offset: (f32, f32),
}

/// Si el asa `h` de la caja `((x0, y0), (x1, y1))` puede cambiar algo (en un eje sin medida no).
fn useful_handle(((x0, y0), (x1, y1)): (cs_3d::UvPoint, cs_3d::UvPoint), h: cs_3d::Handle) -> bool {
    !((h.u == 0 || x0 == x1) && (h.v == 0 || y0 == y1))
}

/// El punto de la caja que mueve el asa `h` (esquina o mitad de un lado), en pixeles.
fn handle_point(
    ((x0, y0), (x1, y1)): (cs_3d::UvPoint, cs_3d::UvPoint),
    h: cs_3d::Handle,
) -> (f32, f32) {
    let pick = |s: i8, lo: i32, hi: i32| match s {
        -1 => lo as f32,
        1 => hi as f32,
        _ => (lo + hi) as f32 / 2.0,
    };
    (pick(h.u, x0, x1), pick(h.v, y0, y1))
}

/// El cursor de redimensionar de un asa.
fn uv_handle_cursor(h: cs_3d::Handle) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match (h.u, h.v) {
        (0, _) => C::ResizeVertical,
        (_, 0) => C::ResizeHorizontal,
        (u, v) if u == v => C::ResizeNwSe,
        _ => C::ResizeNeSw,
    }
}

/// Los puntos enteros de la línea de `a` a `b` (Bresenham, las dos puntas incluidas).
fn line_points((x0, y0): (i32, i32), (x1, y1): (i32, i32)) -> Vec<(i32, i32)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut out = Vec::with_capacity((dx - dy) as usize + 1);
    loop {
        out.push((x, y));
        if (x, y) == (x1, y1) {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

impl CanvasView {
    /// Dónde cae en pantalla el punto `uv` de la textura (el artboard activo).
    fn uv_screen(&self, rect: Rect, editor: &Editor, uv: (f32, f32)) -> Pos2 {
        self.to_screen(rect, editor.artboard().position) + Vec2::new(uv.0, uv.1) * self.scale()
    }

    /// El punto de pantalla `p` en pixeles de la textura (fraccionario).
    fn uv_point(&self, rect: Rect, editor: &Editor, p: Pos2) -> (f32, f32) {
        let (x, y) = self.to_world(rect, p);
        let (ax, ay) = editor.artboard().position;
        (x - ax as f32, y - ay as f32)
    }

    /// Las asas útiles de la caja `b` y su rect en pantalla (por fuera de la caja).
    fn uv_handles(
        &self,
        rect: Rect,
        editor: &Editor,
        b: (cs_3d::UvPoint, cs_3d::UvPoint),
    ) -> Vec<(cs_3d::Handle, Rect)> {
        let at = |(x, y): cs_3d::UvPoint| self.uv_screen(rect, editor, (x as f32, y as f32));
        let r = Rect::from_min_max(at(b.0), at(b.1)).expand(UV_HANDLE_GAP);
        cs_3d::Handle::ALL
            .into_iter()
            .filter(|&h| useful_handle(b, h))
            .map(|h| {
                let dirs = (h.u as i32, h.v as i32);
                (h, pixel_selection::handle_rect(r, dirs))
            })
            .collect()
    }

    /// Lo que hay en `p` entre las UVs de las caras elegidas: un vértice gana sobre un asa y un
    /// asa sobre el adentro de un contorno.
    fn uv_hit(&self, rect: Rect, editor: &Editor, model: &Model3D, p: Pos2) -> Option<UvHit> {
        let faces = editor.selected_faces();
        let uvs_of = |(o, f): (usize, usize)| {
            model
                .objects
                .get(o)
                .and_then(|obj| obj.mesh.faces.get(f))
                .map(|face| face.uvs.as_slice())
        };
        let reach = UV_VERTEX_PX as f32 / 2.0 + 1.0;
        for &face in &faces {
            for (vertex, &(x, y)) in uvs_of(face).unwrap_or_default().iter().enumerate() {
                let s = self.uv_screen(rect, editor, (x as f32, y as f32));
                if (p.x - s.x).abs() <= reach && (p.y - s.y).abs() <= reach {
                    return Some(UvHit::Vertex { face, vertex });
                }
            }
        }
        if let Some(b) = cs_3d::uv_bounds(model, &faces)
            && let Some((h, _)) = self
                .uv_handles(rect, editor, b)
                .into_iter()
                .find(|(_, r)| r.expand(1.0).contains(p))
        {
            return Some(UvHit::Handle(h));
        }
        let at = self.uv_point(rect, editor, p);
        faces
            .iter()
            .any(|&f| cs_3d::uv_polygon_contains(uvs_of(f).unwrap_or_default(), at))
            .then_some(UvHit::Inside)
    }

    /// Los gestos de UV en la textura de abajo (`rect`: su panel). Con un gesto en curso lo
    /// sigue y al soltar lo termina (sin pasar el umbral, lo cancela); si no, con Move o con
    /// Alt, una prensa sobre un vértice, un asa o un contorno lo empieza. Devuelve el cursor si
    /// el puntero es de las UVs (y entonces la herramienta no recibe la prensa).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn uv_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        pointer: Option<Pos2>,
        (pressed, secondary): (bool, bool),
        down: bool,
        modifiers: egui::Modifiers,
    ) -> Option<egui::CursorIcon> {
        self.uv_hover = false;
        if editor.model_edit().uv_op.is_some() {
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let Some(press) = self.uv_press.filter(|_| !escape && !secondary) else {
                // Esc o el otro botón lo cancelan (y uno que quedó sin prensa, también).
                editor.cancel_uv_op();
                self.uv_press = None;
                return None;
            };
            if !down {
                if press.moved {
                    editor.finish_uv_op();
                } else {
                    editor.cancel_uv_op();
                }
                self.uv_press = None;
                return Some(press.cursor);
            }
            if let Some(p) = pointer
                && (press.moved || (p - press.at).length() >= DRAG_THRESHOLD)
            {
                self.uv_press = Some(UvPress {
                    moved: true,
                    ..press
                });
                let (x, y) = self.uv_point(rect, editor, p);
                let at = (x + press.offset.0, y + press.offset.1);
                editor.update_uv_op(at, modifiers.shift);
            }
            self.uv_hover = true;
            return Some(press.cursor);
        }
        self.uv_press = None;
        let tool = editor.active_tool();
        if !(tool == ToolKind::Move || modifiers.alt) || editor.is_painting() || !response.hovered()
        {
            return None;
        }
        let p = pointer?;
        let model = editor.edited_model()?.clone();
        let hit = self.uv_hit(rect, editor, &model, p)?;
        let cursor = match hit {
            UvHit::Handle(h) => uv_handle_cursor(h),
            _ => egui::CursorIcon::Move,
        };
        if pressed {
            let at = self.uv_point(rect, editor, p);
            let faces = editor.selected_faces();
            let (op, offset) = match hit {
                UvHit::Vertex { face, vertex } => (
                    UvOp::Vertex {
                        face,
                        vertex,
                        from: at,
                    },
                    (0.0, 0.0),
                ),
                UvHit::Inside => (UvOp::Move { from: at }, (0.0, 0.0)),
                UvHit::Handle(handle) => {
                    let bounds = cs_3d::uv_bounds(&model, &faces)?;
                    let (hx, hy) = handle_point(bounds, handle);
                    (UvOp::Scale { handle, bounds }, (hx - at.0, hy - at.1))
                }
            };
            editor.end_group();
            editor.start_uv_op(op);
            self.uv_press = Some(UvPress {
                at: p,
                moved: false,
                cursor,
                offset,
            });
        }
        self.uv_hover = true;
        Some(cursor)
    }

    /// Las UVs de las caras elegidas sobre la textura (en el panel de abajo): el contorno de cada
    /// una, sus vértices y las asas de la caja de todas. Del modelo que se ve (con la vista previa
    /// del gesto).
    pub(super) fn draw_uv_overlay(&self, painter: &egui::Painter, rect: Rect, editor: &Editor) {
        let faces = editor.selected_faces();
        let Some(model) = editor.edit_model_shown().filter(|_| !faces.is_empty()) else {
            return;
        };
        let origin = self.uv_screen(rect, editor, (0.0, 0.0));
        let scale = self.scale();
        // Cada UV en puntos enteros desde `origin` (con zoom entero, exacto).
        let local = |(x, y): cs_3d::UvPoint| {
            (
                (x as f32 * scale.x).round() as i32,
                (y as f32 * scale.y).round() as i32,
            )
        };
        let cell = |(x, y): (i32, i32), grow: f32| {
            Rect::from_min_size(origin + Vec2::new(x as f32, y as f32), Vec2::splat(1.0))
                .expand(grow)
        };
        // Los tramos de cada lado: uno solo si es recto; si no, punto a punto.
        let mut runs: Vec<((i32, i32), (i32, i32))> = Vec::new();
        let mut vertices: Vec<(i32, i32)> = Vec::new();
        for &(o, f) in &faces {
            let Some(face) = model.objects.get(o).and_then(|obj| obj.mesh.faces.get(f)) else {
                continue;
            };
            let pts: Vec<(i32, i32)> = face.uvs.iter().map(|&uv| local(uv)).collect();
            for (k, &a) in pts.iter().enumerate() {
                let b = pts[(k + 1) % pts.len()];
                if a.0 == b.0 || a.1 == b.1 {
                    runs.push(((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1))));
                } else {
                    runs.extend(line_points(a, b).into_iter().map(|p| (p, p)));
                }
            }
            vertices.extend(pts);
        }
        let span = |(a, b): ((i32, i32), (i32, i32)), grow: f32| cell(a, grow).union(cell(b, grow));
        // El borde oscuro debajo de todo y lo blanco encima.
        for &r in &runs {
            painter.rect_filled(span(r, 1.0), 0.0, INK);
        }
        let half = UV_VERTEX_PX / 2;
        let dot = |v: (i32, i32)| {
            Rect::from_min_size(
                origin + Vec2::new((v.0 - half) as f32, (v.1 - half) as f32),
                Vec2::splat(UV_VERTEX_PX as f32),
            )
        };
        for &v in &vertices {
            painter.rect_filled(dot(v).expand(1.0), 0.0, INK);
        }
        for &r in &runs {
            painter.rect_filled(span(r, 0.0), 0.0, Color32::WHITE);
        }
        for &v in &vertices {
            painter.rect_filled(dot(v), 0.0, Color32::WHITE);
        }
        if let Some(b) = cs_3d::uv_bounds(model, &faces) {
            for (_, r) in self.uv_handles(rect, editor, b) {
                pixel_selection::draw_handle(painter, r);
            }
        }
    }
}
