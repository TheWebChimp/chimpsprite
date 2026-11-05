//! Las UVs sobre la textura del modo Texture (abajo): el contorno de cada cara elegida (blanco con
//! borde oscuro de 1 punto) y sus vértices, y la caja de lo que se eligió (las caras completas o,
//! sin caras, el objeto o el grupo: [`Editor::uv_box_faces`]) con sus 8 asas, en puntos enteros
//! (con zoom entero, cada pixel de la textura mide puntos enteros). Un vértice es un punto libre:
//! arrastrarlo, con cualquier herramienta (también las de pintar), lo mueve solo a cualquier
//! pixel. Un asa (por fuera de la caja, para no tapar los vértices de sus esquinas) se arrastra
//! igual y escala las UVs de la caja respecto del lado opuesto, en pixeles enteros (Shift:
//! proporcional). Arrastrar dentro de la caja mueve sus UVs juntas con Alt (cualquier
//! herramienta) o, si la caja es la de las caras elegidas, también con Move (la de la fila
//! activa deja Move para los pixeles). Un clic sin arrastre no cambia las UVs (salvo con
//! Move, es de la herramienta: el lápiz pinta ahí); cada gesto es un paso de undo.

use cs_core::Model3D;

use super::*;
use crate::editor::{DRAG_THRESHOLD, UvOp};
use crate::pixel_cursors::INK;

/// Lado del cuadrito de cada vértice UV (y de cada asa de la caja), en puntos.
const UV_VERTEX_PX: i32 = 5;
/// Del punto de la caja (esquina o mitad de un lado) al centro de su asa, en puntos y por eje:
/// por fuera, con 1 punto de aire entre el borde oscuro del asa y el de un vértice ahí.
pub(crate) const UV_HANDLE_OFFSET: f32 = (UV_VERTEX_PX + 2) as f32;

/// Lo que hay bajo el puntero entre las UVs elegidas.
#[derive(Clone, Copy, Debug, PartialEq)]
enum UvHit {
    /// Un asa de la caja: `(x, y)`, −1 el lado menor, 1 el mayor, 0 ninguno.
    Handle((i8, i8)),
    Vertex {
        face: (usize, usize),
        vertex: usize,
    },
    Inside,
}

/// Las 8 asas de una caja, `(x, y)` por eje.
const UV_HANDLES: [(i8, i8); 8] = [
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// El cursor de redimensionar de un asa.
fn uv_handle_cursor(h: (i8, i8)) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match h {
        (0, _) => C::ResizeVertical,
        (_, 0) => C::ResizeHorizontal,
        (u, v) if u == v => C::ResizeNwSe,
        _ => C::ResizeNeSw,
    }
}

/// Un gesto de UV apretado: dónde (en pantalla), si ya pasó el umbral de arrastre, su cursor,
/// y, con Alt, el gesto que empieza al pasar el umbral (si no, el clic es de la herramienta).
#[derive(Clone, Copy, Debug)]
pub(super) struct UvPress {
    at: Pos2,
    moved: bool,
    cursor: egui::CursorIcon,
    pending: Option<UvOp>,
    /// Si se apretó con Alt (el clic sin arrastre le llega así a la herramienta).
    alt: bool,
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

    /// Las asas útiles de la caja `b` (en un eje sin medida, las que solo lo cambian a él no) y
    /// su centro en puntos enteros desde el origen de la textura (con `scale`).
    fn uv_handle_spots(
        scale: Vec2,
        ((x0, y0), (x1, y1)): ((i32, i32), (i32, i32)),
    ) -> Vec<((i8, i8), (i32, i32))> {
        let pick = |s: i8, lo: i32, hi: i32, k: f32| {
            let at = match s {
                -1 => lo as f32,
                1 => hi as f32,
                _ => (lo + hi) as f32 / 2.0,
            };
            (at * k + s as f32 * UV_HANDLE_OFFSET).round() as i32
        };
        UV_HANDLES
            .into_iter()
            .filter(|&(u, v)| !((u == 0 || x0 == x1) && (v == 0 || y0 == y1)))
            .map(|h| (h, (pick(h.0, x0, x1, scale.x), pick(h.1, y0, y1, scale.y))))
            .collect()
    }

    /// Lo que hay en `p` entre las UVs: un asa de la caja gana sobre un vértice de las caras
    /// elegidas y un vértice sobre el adentro de la caja.
    fn uv_hit(&self, rect: Rect, editor: &Editor, model: &Model3D, p: Pos2) -> Option<UvHit> {
        let reach = UV_VERTEX_PX as f32 / 2.0 + 1.0;
        let near = |s: Pos2| (p.x - s.x).abs() <= reach && (p.y - s.y).abs() <= reach;
        let bounds = cs_3d::uv_bounds(model, &editor.uv_box_faces());
        if let Some(b) = bounds {
            let origin = self.uv_screen(rect, editor, (0.0, 0.0));
            let hit = Self::uv_handle_spots(self.scale(), b)
                .into_iter()
                .find(|&(_, (x, y))| near(origin + Vec2::new(x as f32, y as f32)));
            if let Some((h, _)) = hit {
                return Some(UvHit::Handle(h));
            }
        }
        let faces = editor.selected_faces();
        let uvs_of = |(o, f): (usize, usize)| {
            model
                .objects
                .get(o)
                .and_then(|obj| obj.mesh.faces.get(f))
                .map(|face| face.uvs.as_slice())
        };
        for &face in &faces {
            for (vertex, &(x, y)) in uvs_of(face).unwrap_or_default().iter().enumerate() {
                if near(self.uv_screen(rect, editor, (x as f32, y as f32))) {
                    return Some(UvHit::Vertex { face, vertex });
                }
            }
        }
        let ((x0, y0), (x1, y1)) = bounds?;
        let (x, y) = self.uv_point(rect, editor, p);
        (x >= x0 as f32 && x <= x1 as f32 && y >= y0 as f32 && y <= y1 as f32)
            .then_some(UvHit::Inside)
    }

    /// Los gestos de UV en la textura de abajo (`rect`: su panel). Una prensa sobre un asa, un
    /// vértice o la caja con Move empieza el gesto; con otra herramienta (sobre un asa o un
    /// vértice, o con Alt también dentro de la caja) el gesto espera al umbral de arrastre: un
    /// clic sin arrastre se le pasa a la herramienta tal cual (el gotero de las que pintan con
    /// Alt, etc.) y un arrastre cambia las UVs (también con las de selección, que con Alt
    /// restarían). Sin pasar el umbral nada cambia. Con Move y la selección de las caras, una
    /// prensa fuera de las UVs no hace nada (no corta los pixeles de la cara). Devuelve el cursor
    /// si el puntero es de las UVs (y entonces la herramienta no recibe la prensa).
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
        if let Some(press) = self.uv_press {
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if escape || secondary {
                // Esc o el otro botón lo cancelan.
                editor.cancel_uv_op();
                self.uv_press = None;
                return None;
            }
            if !down {
                if press.moved {
                    editor.finish_uv_op();
                } else {
                    editor.cancel_uv_op();
                    // Un clic sin arrastre (salvo con Move): la herramienta recibe el clic que se
                    // guardó, con Alt si lo tenía.
                    if press.pending.is_some() {
                        let world = self.to_world(rect, press.at);
                        let slop = EDGE_SLOP_PX / self.zoom;
                        editor.pointer_press(world, false, press.alt, slop);
                        editor.pointer_release();
                    }
                }
                self.uv_press = None;
                return Some(press.cursor);
            }
            if let Some(p) = pointer
                && (press.moved || (p - press.at).length() >= DRAG_THRESHOLD)
            {
                if let Some(op) = press.pending {
                    editor.end_group();
                    editor.start_uv_op(op);
                }
                self.uv_press = Some(UvPress {
                    moved: true,
                    pending: None,
                    ..press
                });
                editor.update_uv_op(self.uv_point(rect, editor, p), modifiers.shift);
            }
            self.uv_hover = true;
            return Some(press.cursor);
        }
        // Un gesto que quedó sin prensa (p. ej. el panel dejó de verse) no sigue.
        editor.cancel_uv_op();
        let tool = editor.active_tool();
        let moving = tool == ToolKind::Move;
        if editor.is_painting() || !response.hovered() {
            return None;
        }
        let p = pointer?;
        let model = editor.edited_model()?.clone();
        // Lo de adentro de la caja mueve las UVs con Alt o, si la caja es la de las caras
        // elegidas, también con Move. La caja de la fila activa (sin caras) siempre está: con
        // Move sin Alt, lo de adentro es de la herramienta (mover pixeles o la selección), como
        // fuera del modo Texture. Las asas y los vértices, con cualquier herramienta.
        let inside_moves = modifiers.alt || (moving && !editor.selected_faces().is_empty());
        let Some(hit) = self
            .uv_hit(rect, editor, &model, p)
            .filter(|h| inside_moves || !matches!(h, UvHit::Inside))
        else {
            // Move sobre la selección de las caras, fuera de sus UVs: nada (ni el menú del clic
            // derecho, que sí sigue).
            return (moving && !secondary && editor.selection_is_face_mask())
                .then_some(egui::CursorIcon::Default);
        };
        let cursor = match hit {
            UvHit::Handle(h) => uv_handle_cursor(h),
            _ => egui::CursorIcon::Move,
        };
        if pressed {
            let at = self.uv_point(rect, editor, p);
            let op = match hit {
                UvHit::Handle(handle) => UvOp::Scale {
                    handle,
                    from: at,
                    bounds: cs_3d::uv_bounds(&model, &editor.uv_box_faces())?,
                },
                UvHit::Vertex { face, vertex } => UvOp::Vertex {
                    face,
                    vertex,
                    from: at,
                },
                UvHit::Inside => UvOp::Move { from: at },
            };
            // Con Move empieza ya; si no, al pasar el umbral (un clic es de la herramienta).
            let pending = if moving {
                editor.end_group();
                editor.start_uv_op(op);
                None
            } else {
                Some(op)
            };
            self.uv_press = Some(UvPress {
                at: p,
                moved: false,
                cursor,
                pending,
                alt: modifiers.alt,
            });
        }
        self.uv_hover = true;
        Some(cursor)
    }

    /// Olvida la prensa de UV a medias (el panel de la textura no se ve o no recibe la entrada):
    /// si no, al volver una prensa con Alt vieja le pasaría un clic a la herramienta.
    pub(super) fn drop_uv_press(&mut self, editor: &mut Editor) {
        if self.uv_press.take().is_some() {
            editor.cancel_uv_op();
        }
    }

    /// Las UVs sobre la textura (en el panel de abajo): el contorno de cada cara elegida y sus
    /// vértices, y la caja de lo elegido con sus asas. Del modelo que se ve (con la vista previa
    /// del gesto).
    pub(super) fn draw_uv_overlay(&self, painter: &egui::Painter, rect: Rect, editor: &Editor) {
        let faces = editor.selected_faces();
        let boxed = editor.uv_box_faces();
        let Some(model) = editor.edit_model_shown().filter(|_| !boxed.is_empty()) else {
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
        // La caja (sus 4 lados) y sus asas, dibujadas como los vértices.
        if let Some(b) = cs_3d::uv_bounds(model, &boxed) {
            let ((x0, y0), (x1, y1)) = (local(b.0), local(b.1));
            runs.extend([
                ((x0, y0), (x1, y0)),
                ((x0, y1), (x1, y1)),
                ((x0, y0), (x0, y1)),
                ((x1, y0), (x1, y1)),
            ]);
            vertices.extend(Self::uv_handle_spots(scale, b).into_iter().map(|(_, p)| p));
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
    }
}
