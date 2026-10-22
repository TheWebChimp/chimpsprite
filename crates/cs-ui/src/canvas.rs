use std::collections::HashMap;
use std::time::Duration;

use cs_core::{Artboard, ArtboardId, Axis, GridSettings, Guide, Mask, SymmetryMode};
use cs_tools::{ArtboardHit, Edges, hit_test};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};

use crate::canvas_size::{CanvasSizeDialog, GuideEdge};
use crate::editor::{Editor, ToolKind};
use crate::guides::{RULER_PX, SNAP_PX, SnapContext, after_drag, hit_guide, nice_step};
use crate::view::{ViewSettings, ZoomRequest};
use crate::{theme, widgets};

const ZOOM_LEVELS: [f32; 13] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0,
];
/// Tamaño en pantalla de cada cuadro del damero de transparencia.
const CHECKER_PX: f32 = 8.0;
/// Velocidad de las "marching ants" (pasos por segundo).
const ANTS_SPEED: f64 = 6.0;
/// Tolerancia en pantalla para tomar bordes de artboards.
const EDGE_SLOP_PX: f32 = 6.0;
/// Etiqueta con el nombre sobre cada artboard (arrastrable para moverlo).
const LABEL_SIZE: f32 = 12.0;
const LABEL_GAP: f32 = 4.0;
/// Los números de tile se dibujan solo si cada tile ocupa al menos esto en pantalla.
const TILE_NUMBER_MIN_PX: f32 = 16.0;

/// Vista del lienzo infinito: zoom, paneo y una textura por artboard.
pub struct CanvasView {
    pub zoom: f32,
    pan: Vec2,
    textures: HashMap<ArtboardId, TextureHandle>,
    checker: Option<TextureHandle>,
    rendered_revision: Option<u64>,
    fitted: bool,
    /// Píxel local del artboard activo bajo el cursor, para la barra de estado.
    pub hover: Option<(i32, i32)>,
    /// Ayuda para la barra de estado cuando el puntero está sobre una regla o una guía.
    pub guide_hint: Option<&'static str>,
    /// Guía del diálogo Canvas Size que se está arrastrando.
    dragging_guide: Option<GuideEdge>,
    /// El puntero está sobre la etiqueta de un artboard.
    over_label: bool,
    /// Las etiquetas se ven (si no, no se pueden arrastrar).
    names_visible: bool,
    /// Zoom pedido desde el menú; se aplica al conocer el tamaño del lienzo.
    pending_zoom: Option<ZoomRequest>,
    /// Guía que se está creando (desde una regla) o moviendo.
    ruler_drag: Option<GuideDrag>,
    /// Eje de simetría que se está arrastrando y centro provisional (medios píxeles).
    sym_drag: Option<(SymHandle, (i32, i32))>,
    /// El puntero está sobre una regla.
    over_ruler: bool,
    /// Dibujar el cursor de cuentagotas (el del sistema se oculta).
    draw_pipette: bool,
}

/// Margen (en pantalla) desde el borde del lienzo en el que empieza el auto-scroll.
const AUTOSCROLL_MARGIN: f32 = 24.0;
/// Velocidad máxima del auto-scroll en píxeles de pantalla por segundo.
const AUTOSCROLL_SPEED: f32 = 700.0;

/// Velocidad de auto-scroll (px/s) cuando el puntero arrastra cerca o fuera de `area`.
/// Negativa hacia la izquierda/arriba; crece con la distancia y se satura.
fn autoscroll_velocity(area: Rect, p: Pos2) -> Vec2 {
    let axis = |v: f32, min: f32, max: f32| {
        let overshoot = if v < min + AUTOSCROLL_MARGIN {
            v - (min + AUTOSCROLL_MARGIN)
        } else if v > max - AUTOSCROLL_MARGIN {
            v - (max - AUTOSCROLL_MARGIN)
        } else {
            0.0
        };
        (overshoot / (AUTOSCROLL_MARGIN * 6.0)).clamp(-1.0, 1.0) * AUTOSCROLL_SPEED
    };
    Vec2::new(
        axis(p.x, area.left(), area.right()),
        axis(p.y, area.top(), area.bottom()),
    )
}

/// Parte del eje de simetría que se arrastra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SymHandle {
    Center,
    X,
    Y,
}

fn has_vertical_axis(mode: SymmetryMode) -> bool {
    matches!(
        mode,
        SymmetryMode::Horizontal | SymmetryMode::Both | SymmetryMode::All
    )
}

fn has_horizontal_axis(mode: SymmetryMode) -> bool {
    matches!(
        mode,
        SymmetryMode::Vertical | SymmetryMode::Both | SymmetryMode::All
    )
}

/// Arrastre de una guía: `index` es la guía original si se está moviendo una existente.
#[derive(Clone, Copy, Debug)]
struct GuideDrag {
    index: Option<usize>,
    axis: Axis,
    position: i32,
    over_ruler: bool,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            zoom: 8.0,
            pan: Vec2::ZERO,
            textures: HashMap::new(),
            checker: None,
            rendered_revision: None,
            fitted: false,
            hover: None,
            guide_hint: None,
            dragging_guide: None,
            over_label: false,
            names_visible: true,
            pending_zoom: None,
            ruler_drag: None,
            sym_drag: None,
            over_ruler: false,
            draw_pipette: false,
        }
    }
}

impl CanvasView {
    /// Posición en pantalla del origen del mundo.
    fn origin(&self, rect: Rect) -> Pos2 {
        rect.center() + self.pan
    }

    fn to_screen(&self, rect: Rect, world: (i32, i32)) -> Pos2 {
        self.origin(rect) + Vec2::new(world.0 as f32, world.1 as f32) * self.zoom
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> (f32, f32) {
        let w = (p - self.origin(rect)) / self.zoom;
        (w.x, w.y)
    }

    fn fit(&mut self, rect: Rect, editor: &Editor) {
        let Some(((x0, y0), (x1, y1))) = editor.document.bounds() else {
            return;
        };
        let size = Vec2::new((x1 - x0) as f32, (y1 - y0) as f32);
        let max = (rect.size() * 0.85 / size).min_elem();
        self.zoom = ZOOM_LEVELS
            .iter()
            .copied()
            .rfind(|&z| z <= max)
            .unwrap_or(ZOOM_LEVELS[0]);
        let center = Vec2::new((x0 + x1) as f32, (y0 + y1) as f32) / 2.0;
        self.pan = -center * self.zoom;
        self.fitted = true;
    }

    fn zoom_at(&mut self, rect: Rect, anchor: Pos2, step_in: bool) {
        let i = ZOOM_LEVELS
            .iter()
            .position(|&z| z >= self.zoom)
            .unwrap_or(0);
        let next = if step_in {
            ZOOM_LEVELS[(i + 1).min(ZOOM_LEVELS.len() - 1)]
        } else {
            ZOOM_LEVELS[i.saturating_sub(1)]
        };
        let world = (anchor - self.origin(rect)) / self.zoom;
        self.zoom = next;
        self.pan = anchor - world * next - rect.center();
    }

    pub fn request_zoom(&mut self, request: ZoomRequest) {
        self.pending_zoom = Some(request);
    }

    fn apply_zoom(&mut self, rect: Rect, editor: &Editor, request: ZoomRequest) {
        match request {
            ZoomRequest::In => self.zoom_at(rect, rect.center(), true),
            ZoomRequest::Out => self.zoom_at(rect, rect.center(), false),
            ZoomRequest::Actual => {
                // Mantiene en el centro lo que está en el centro.
                let center = (rect.center() - self.origin(rect)) / self.zoom;
                self.zoom = 1.0;
                self.pan = -center;
            }
            ZoomRequest::Fit => self.fit(rect, editor),
            ZoomRequest::CenterActive => self.center_active(editor),
        }
    }

    /// Centra el artboard activo sin cambiar el zoom (p. ej. al entrar a un símbolo).
    fn center_active(&mut self, editor: &Editor) {
        let ((x0, y0), (x1, y1)) = editor.artboard().rect();
        let center = Vec2::new((x0 + x1) as f32, (y0 + y1) as f32) / 2.0;
        self.pan = -center * self.zoom;
    }

    fn checker(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.checker
            .get_or_insert_with(|| {
                let (a, b) = (theme::CHECKER_LIGHT, theme::CHECKER_DARK);
                let image = ColorImage::new([2, 2], vec![a, b, b, a]);
                ctx.load_texture("checker", image, TextureOptions::NEAREST_REPEAT)
            })
            .id()
    }

    /// Reglas superior e izquierda (sin la esquina).
    fn ruler_rects(rect: Rect) -> (Rect, Rect) {
        let top = Rect::from_min_max(
            rect.min + Vec2::new(RULER_PX, 0.0),
            Pos2::new(rect.right(), rect.top() + RULER_PX),
        );
        let left = Rect::from_min_max(
            rect.min + Vec2::new(0.0, RULER_PX),
            Pos2::new(rect.left() + RULER_PX, rect.bottom()),
        );
        (top, left)
    }

    /// Centro de simetría efectivo en pantalla (con el arrastre en curso, si lo hay).
    fn symmetry_center(&self, rect: Rect, editor: &Editor) -> (Pos2, (i32, i32)) {
        let a = editor.artboard();
        let c2 = self
            .sym_drag
            .map_or_else(|| a.symmetry_center2(), |(_, c)| c);
        let origin = self.to_screen(rect, a.position);
        (
            origin + Vec2::new(c2.0 as f32, c2.1 as f32) * (self.zoom / 2.0),
            c2,
        )
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
        if symmetry.mode == SymmetryMode::None {
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
        let (center, c2) = self.symmetry_center(rect, editor);
        let artboard = Rect::from_min_max(
            origin,
            origin
                + Vec2::new(editor.sprite().width as f32, editor.sprite().height as f32)
                    * self.zoom,
        );
        let handle = if p.distance(center) <= 7.0 {
            Some(SymHandle::Center)
        } else if has_vertical_axis(symmetry.mode)
            && (p.x - center.x).abs() <= 4.0
            && artboard.y_range().contains(p.y)
        {
            Some(SymHandle::X)
        } else if has_horizontal_axis(symmetry.mode)
            && (p.y - center.y).abs() <= 4.0
            && artboard.x_range().contains(p.x)
        {
            Some(SymHandle::Y)
        } else {
            None
        }?;
        let grabbable = matches!(
            editor.active_tool(),
            ToolKind::RectSelect | ToolKind::EllipseSelect | ToolKind::Move | ToolKind::Artboard
        ) || ctrl;
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

    /// Ejes de simetría punteados sobre el artboard activo y su centro.
    fn draw_symmetry(&self, painter: &egui::Painter, rect: Rect, editor: &Editor) {
        let mode = editor.symmetry().mode;
        if mode == SymmetryMode::None {
            return;
        }
        let a = editor.artboard();
        let ((x0, y0), (x1, y1)) = a.rect();
        let screen = Rect::from_min_max(
            self.to_screen(rect, (x0, y0)),
            self.to_screen(rect, (x1, y1)),
        );
        let painter = painter.with_clip_rect(screen.intersect(rect));
        let (c, _) = self.symmetry_center(rect, editor);
        let stroke = Stroke::new(1.5, theme::ACCENT);
        let mut segments = Vec::new();
        if has_vertical_axis(mode) {
            segments.push([
                Pos2::new(c.x, screen.top()),
                Pos2::new(c.x, screen.bottom()),
            ]);
        }
        if has_horizontal_axis(mode) {
            segments.push([
                Pos2::new(screen.left(), c.y),
                Pos2::new(screen.right(), c.y),
            ]);
        }
        let k = screen.width().max(screen.height());
        if matches!(mode, SymmetryMode::DiagonalDown | SymmetryMode::All) {
            segments.push([c - Vec2::splat(k), c + Vec2::splat(k)]);
        }
        if matches!(mode, SymmetryMode::DiagonalUp | SymmetryMode::All) {
            segments.push([c + Vec2::new(-k, k), c + Vec2::new(k, -k)]);
        }
        for seg in segments {
            painter.extend(egui::Shape::dashed_line(&seg, stroke, 6.0, 4.0));
        }
        painter.circle(
            c,
            4.0,
            theme::ACCENT,
            Stroke::new(1.0, theme::CANVAS_BORDER),
        );
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
        ctrl: bool,
    ) -> Option<egui::CursorIcon> {
        let origin = editor.artboard().position;
        // Copias para no tomar prestado `self` dentro de los closures.
        let (screen_origin, zoom) = (self.origin(rect), self.zoom);
        let local = move |p: Pos2| {
            let w = (p - screen_origin) / zoom;
            (w.x - origin.0 as f32, w.y - origin.1 as f32)
        };
        let on_ruler = |p: Pos2| {
            view.rulers && {
                let (top, left) = Self::ruler_rects(rect);
                top.contains(p)
                    || left.contains(p)
                    || p.x < rect.left() + RULER_PX && p.y < rect.top() + RULER_PX
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
            let (top, left) = Self::ruler_rects(rect);
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
        let grabbable = matches!(
            editor.active_tool(),
            ToolKind::RectSelect | ToolKind::EllipseSelect | ToolKind::Move | ToolKind::Artboard
        ) || ctrl;
        if view.lock_guides {
            self.guide_hint = Some("Guides are locked (View › Guides › Lock Guides)");
            return None;
        }
        if !grabbable {
            self.guide_hint =
                Some("Guide: hold Ctrl and drag to move it · drop it on a ruler to remove it");
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

    fn handle_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        dialog: Option<&mut CanvasSizeDialog>,
        view: &ViewSettings,
    ) {
        // Zoom con la rueda, anclado al cursor.
        if let Some(pointer) = response.hover_pos() {
            let scroll: f32 = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } => Some(delta.y),
                        _ => None,
                    })
                    .sum()
            });
            if scroll != 0.0 {
                self.zoom_at(rect, pointer, scroll > 0.0);
            }
        }

        self.draw_pipette = false;
        // Paneo con botón medio o espacio + arrastre.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.dragged_by(egui::PointerButton::Middle)
            || (space && response.dragged_by(egui::PointerButton::Primary))
        {
            self.pan += response.drag_delta();
        }

        self.hover = response
            .hover_pos()
            .map(|p| editor.to_local(self.to_world(rect, p)));

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
        let over_label = pointer
            .filter(|_| self.names_visible)
            .and_then(|p| self.label_at(ui, rect, editor, p));
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
            if pressed_primary && response.hovered() && over_guide.is_some() && !space {
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
        let sym_cursor = if modal || space {
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
        let ruler_cursor = if modal || space || sym_cursor.is_some() || self.sym_drag.is_some() {
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
                modifiers.command,
            )
        };
        let ruler_busy =
            ruler_cursor.is_some() || self.ruler_drag.is_some() || self.sym_drag.is_some();
        guide_cursor = guide_cursor.or(ruler_cursor);

        if !modal
            && !ruler_busy
            && self.dragging_guide.is_none()
            && !space
            && response.hovered()
            && !editor.is_painting()
            && (pressed_primary || pressed_secondary)
            && let Some(p) = pointer
        {
            let world = self.to_world(rect, p);
            match over_label {
                Some(id) if pressed_primary => editor.press_artboard_label(id, world),
                _ => editor.pointer_press(world, pressed_secondary, modifiers.alt, slop),
            }
        }
        // Auto-scroll: arrastrando cerca o fuera del borde, la vista se desplaza.
        let dragging = editor.is_painting() || self.ruler_drag.is_some() || self.sym_drag.is_some();
        if dragging
            && any_down
            && !space
            && !modal
            && let Some(p) = pointer
        {
            let area = if view.rulers {
                Rect::from_min_max(rect.min + Vec2::splat(RULER_PX), rect.max)
            } else {
                rect
            };
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
        if response.double_clicked() && editor.active_tool() == ToolKind::PolygonLasso {
            editor.finish_gesture();
        }
        if response.double_clicked()
            && editor.active_tool() == ToolKind::Move
            && editor.enter_instance()
        {
            self.center_active(editor);
        }

        let cursor = if space {
            Some(egui::CursorIcon::Grab)
        } else if guide_cursor.is_some() {
            guide_cursor
        } else if modal {
            response.hovered().then_some(egui::CursorIcon::Default)
        } else if !response.hovered() {
            None
        } else if ruler_busy {
            Some(egui::CursorIcon::Default)
        } else if editor.artboard_gesture().moving() {
            Some(egui::CursorIcon::Grabbing)
        } else if let Some(edges) = editor.artboard_gesture().resizing() {
            Some(artboard_cursor(Some(ArtboardHit::Edge(
                editor.active,
                edges,
            ))))
        } else if over_label.is_some() && !editor.is_painting() {
            Some(egui::CursorIcon::Grab)
        } else if editor.active_tool() == ToolKind::Artboard {
            let world = pointer.map(|p| self.to_world(rect, p));
            Some(artboard_cursor(
                world.and_then(|w| hit_test(&editor.document, w, slop)),
            ))
        } else if editor.active_tool() == ToolKind::Move
            || pointer.is_some_and(|p| {
                let world = self.to_world(rect, p);
                editor.moves_selection_at(world, modifiers.shift, modifiers.alt)
            })
        {
            Some(egui::CursorIcon::Move)
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

    /// `dialog`: diálogo Canvas Size abierto; sus guías se dibujan y se pueden arrastrar.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut Editor,
        mut dialog: Option<&mut CanvasSizeDialog>,
        view_settings: &ViewSettings,
    ) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        if !self.fitted && rect.width() > 0.0 {
            self.fit(rect, editor);
        }
        if let Some(request) = self.pending_zoom.take()
            && rect.width() > 0.0
        {
            self.apply_zoom(rect, editor, request);
        }
        self.names_visible = view_settings.shows(view_settings.artboard_names);
        editor.snap = Some(SnapContext {
            guides: if view_settings.shows(view_settings.guides) && view_settings.snap_to_guides {
                editor.guides().to_vec()
            } else {
                Vec::new()
            },
            grid: view_settings.snap_to_grid.then(|| editor.sprite().grid),
            tolerance: SNAP_PX / self.zoom,
        });
        self.handle_input(
            ui,
            &response,
            rect,
            editor,
            dialog.as_deref_mut(),
            view_settings,
        );
        let modal = dialog.is_some();
        let guide = dialog.and_then(|d| d.preview_rect(&editor.document));

        let view = editor.view();
        let document = view.document.as_ref();

        // Recompone las texturas si el documento cambió o hay un gesto en curso.
        let painting = editor.is_painting();
        if painting || self.rendered_revision != Some(editor.revision) {
            self.textures
                .retain(|id, _| document.artboard(*id).is_some());
            for artboard in &document.artboards {
                let sprite = &artboard.sprite;
                let frame = editor.frame_of(artboard.id);
                let onion = artboard.id == editor.active && editor.onion_enabled && !editor.playing;
                // Las instancias se resuelven a capas normales antes de componer.
                let frames = if onion {
                    cs_render::onion_frames(&editor.onion, frame, sprite.frames.len())
                } else {
                    frame..frame + 1
                };
                let sprite = &*cs_render::materialize(document, sprite, frames);
                let mut pixels = if onion {
                    cs_render::render_onion(sprite, frame, &editor.onion)
                } else {
                    cs_render::render_frame(sprite, frame)
                };
                // Editando una máscara, lo que oculta se ve en rojo.
                if artboard.id == editor.active && editor.editing_mask() {
                    cs_render::mask_overlay(sprite, editor.layer, &mut pixels);
                }
                let bytes: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                let size = [sprite.width as usize, sprite.height as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
                match self.textures.get_mut(&artboard.id) {
                    Some(tex) => tex.set(image, TextureOptions::NEAREST),
                    None => {
                        let name = format!("artboard-{}", artboard.id.0);
                        let tex = ctx.load_texture(name, image, TextureOptions::NEAREST);
                        self.textures.insert(artboard.id, tex);
                    }
                }
            }
            // Durante un gesto no marcamos como al día, para recomponer al terminar.
            self.rendered_revision = (!painting).then_some(editor.revision);
        }

        let checker = self.checker(&ctx);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::CANVAS_BG);

        for artboard in &document.artboards {
            let ((x0, y0), (x1, y1)) = artboard.rect();
            let screen = Rect::from_min_max(
                self.to_screen(rect, (x0, y0)),
                self.to_screen(rect, (x1, y1)),
            );
            let uv = Rect::from_min_max(Pos2::ZERO, (screen.size() / (CHECKER_PX * 2.0)).to_pos2());
            painter.image(checker, screen, uv, Color32::WHITE);
            if let Some(tex) = self.textures.get(&artboard.id) {
                let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
                painter.image(tex.id(), screen, full_uv, Color32::WHITE);
            }
            if view_settings.shows(view_settings.grid) {
                draw_grid(
                    &painter,
                    screen,
                    artboard.sprite.grid,
                    self.zoom,
                    view_settings.grid_color(),
                );
            }
            if self.zoom >= 8.0 && view_settings.shows(view_settings.pixel_grid) {
                let stroke = Stroke::new(1.0, theme::GRID);
                for x in 1..artboard.sprite.width {
                    painter.vline(
                        screen.left() + x as f32 * self.zoom,
                        screen.y_range(),
                        stroke,
                    );
                }
                for y in 1..artboard.sprite.height {
                    painter.hline(
                        screen.x_range(),
                        screen.top() + y as f32 * self.zoom,
                        stroke,
                    );
                }
            }
            // Modo Tiles: la grilla de tiles de la capa activa, independiente del grid de trabajo.
            if artboard.id == editor.active
                && view_settings.extras
                && let Some(space) = editor.tile_space()
            {
                draw_grid(
                    &painter,
                    screen,
                    GridSettings {
                        size: space.tile_size,
                        offset: space.offset,
                    },
                    self.zoom,
                    theme::TILE_GRID,
                );
            }
            let active = artboard.id == editor.active;
            let (border, label_color) = if active {
                (Stroke::new(2.0, theme::ACCENT), theme::ACCENT)
            } else {
                (Stroke::new(1.0, theme::CANVAS_BORDER), theme::LABEL)
            };
            painter.rect_stroke(screen, 0.0, border, StrokeKind::Outside);
            if self.names_visible {
                painter.text(
                    screen.left_top() - Vec2::new(0.0, LABEL_GAP),
                    Align2::LEFT_BOTTOM,
                    label_text(artboard),
                    FontId::proportional(LABEL_SIZE),
                    label_color,
                );
            }
        }

        if view_settings.shows(view_settings.guides) {
            let dragged = self.ruler_drag.and_then(|d| d.index);
            for artboard in &document.artboards {
                let ((x0, y0), (x1, y1)) = artboard.rect();
                let screen = Rect::from_min_max(
                    self.to_screen(rect, (x0, y0)),
                    self.to_screen(rect, (x1, y1)),
                );
                let active = artboard.id == editor.active;
                for (i, g) in artboard.guides.iter().enumerate() {
                    if active && Some(i) == dragged {
                        continue;
                    }
                    draw_guide(&painter, screen, *g, self.zoom, theme::GUIDE_LINE);
                }
            }
        }
        if let Some(drag) = self.ruler_drag {
            // La guía en movimiento cruza todo el lienzo; sobre la regla se ve tenue (se borrará).
            let a = editor.artboard();
            let world = (a.position.0 + drag.position, a.position.1 + drag.position);
            let color = if drag.over_ruler {
                theme::GUIDE_LINE.gamma_multiply(0.35)
            } else {
                theme::GUIDE_LINE
            };
            let stroke = Stroke::new(1.0, color);
            match drag.axis {
                Axis::Horizontal => {
                    let y = painter.round_to_pixel_center(self.to_screen(rect, world).y);
                    painter.hline(rect.x_range(), y, stroke)
                }
                Axis::Vertical => {
                    let x = painter.round_to_pixel_center(self.to_screen(rect, world).x);
                    painter.vline(x, rect.y_range(), stroke)
                }
            };
        }

        if !modal {
            self.draw_symmetry(&painter, rect, editor);
        }

        if let Some((min, max)) = guide {
            let r = Rect::from_min_max(self.to_screen(rect, min), self.to_screen(rect, max));
            let stroke = Stroke::new(1.0, theme::GUIDE);
            // Guías que cruzan todo el lienzo, como en Aseprite.
            painter.vline(r.left(), rect.y_range(), stroke);
            painter.vline(r.right(), rect.y_range(), stroke);
            painter.hline(rect.x_range(), r.top(), stroke);
            painter.hline(rect.x_range(), r.bottom(), stroke);
            painter.rect_stroke(r, 0.0, Stroke::new(2.0, theme::GUIDE), StrokeKind::Outside);
        }

        let active_origin = document
            .artboard(editor.active)
            .map(|a| self.to_screen(rect, a.position));
        if let Some(origin) = active_origin
            && (view_settings.shows(view_settings.slices)
                || editor.active_tool() == ToolKind::Slice)
            && let Some(artboard) = document.artboard(editor.active)
        {
            draw_slices(&painter, &artboard.sprite, editor, origin, self.zoom);
        }
        if let (Some(mask), Some(origin)) = (&view.selection, active_origin)
            && view_settings.shows(view_settings.selection_edges)
        {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_marching_ants(&painter, mask, origin, self.zoom, phase);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }
        if let Some(origin) = active_origin
            && view_settings.shows(view_settings.tile_numbers)
            && let Some(space) = editor.tile_space()
            && space.tile_size.0 as f32 * self.zoom >= TILE_NUMBER_MIN_PX
            && space.tile_size.1 as f32 * self.zoom >= TILE_NUMBER_MIN_PX
            && let Some(artboard) = document.artboard(editor.active)
        {
            let font = FontId::monospace(10.0);
            // Solo las celdas dentro del área visible del lienzo.
            let min = ((rect.min - origin) / self.zoom).floor();
            let max = ((rect.max - origin) / self.zoom).ceil();
            let view = (min.x as i32, min.y as i32, max.x as i32, max.y as i32);
            let cells = crate::tile_ops::numbered_cells_in(
                &artboard.sprite,
                editor.layer,
                editor.frame,
                view,
            );
            for ((x, y), number) in cells {
                // La celda parcial del borde empieza fuera del lienzo: el número se pega al borde.
                let (x, y) = (x.max(0), y.max(0));
                let at = origin + Vec2::new(x as f32, y as f32) * self.zoom + Vec2::splat(2.0);
                if !rect.contains(at) {
                    continue;
                }
                widgets::outlined_text(
                    &painter,
                    at,
                    Align2::LEFT_TOP,
                    &number.to_string(),
                    font.clone(),
                );
            }
        }

        // Contorno del pincel bajo el cursor.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let tool = editor.active_tool();
        if let (Some((hx, hy)), Some(origin)) = (self.hover, active_origin)
            && !modal
            && !self.over_label
            && !self.over_ruler
            && self.ruler_drag.is_none()
            && self.sym_drag.is_none()
            && !editor.artboard_gesture().is_active()
            && !space
            && response.hovered()
            && tool != ToolKind::Artboard
            && view_settings.shows(view_settings.brush_preview)
        {
            let size = if tool.uses_brush() {
                editor.options.brush_size
            } else {
                1
            };
            let offset = -((size as i32 - 1) / 2);
            // En modo Tiles el pincel cubre celdas enteras.
            let cells = editor.tile_space().filter(|_| tool.uses_cells());
            let r = match cells {
                Some(space) => {
                    let (cx, cy) = space.cell_at((hx, hy));
                    let (x0, y0) = space.cell_origin((cx + offset, cy + offset));
                    let (tw, th) = space.tile_size;
                    Rect::from_min_size(
                        origin + Vec2::new(x0 as f32, y0 as f32) * self.zoom,
                        Vec2::new((size * tw) as f32, (size * th) as f32) * self.zoom,
                    )
                }
                None => Rect::from_min_size(
                    origin + Vec2::new((hx + offset) as f32, (hy + offset) as f32) * self.zoom,
                    Vec2::splat(size as f32 * self.zoom),
                ),
            };
            // Las herramientas que pintan píxeles muestran el color primario bajo el cursor.
            if cells.is_none()
                && tool.paints()
                && tool != ToolKind::Eraser
                && tool != ToolKind::Bucket
            {
                let c = editor.resolve(editor.primary);
                painter.rect_filled(r, 0.0, Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
            }
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, theme::BRUSH_OUTLINE),
                StrokeKind::Inside,
            );
        }

        if view_settings.rulers {
            let pointer = response.hover_pos().or(ui.ctx().pointer_interact_pos());
            self.draw_rulers(&painter, rect, editor, pointer);
        }

        if self.draw_pipette
            && let Some(p) = response.hover_pos()
        {
            // Color que se tomaría: lo que se ve (todas las capas visibles).
            let color = self.hover.and_then(|(x, y)| {
                let s = editor.sprite();
                let inside = x >= 0 && y >= 0 && x < s.width as i32 && y < s.height as i32;
                // `composite_pixel` lee los tilemaps tile a tile: no hace falta materializar en
                // cada frame de hover.
                inside.then(|| cs_render::composite_pixel(s, editor.frame, x, y))
            });
            draw_pipette(&painter, p, color);
        }
    }

    /// Reglas en píxeles del artboard activo (origen en su esquina superior izquierda).
    fn draw_rulers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        pointer: Option<Pos2>,
    ) {
        let (top, left) = Self::ruler_rects(rect);
        let corner = Rect::from_min_size(rect.min, Vec2::splat(RULER_PX));
        for r in [top, left, corner] {
            painter.rect_filled(r, 0.0, theme::RULER_BG);
        }
        painter.hline(
            rect.x_range(),
            top.bottom(),
            Stroke::new(1.0, theme::CANVAS_BORDER),
        );
        painter.vline(
            left.right(),
            rect.y_range(),
            Stroke::new(1.0, theme::CANVAS_BORDER),
        );

        let origin = editor.artboard().position;
        let major = nice_step(self.zoom, 50.0);
        let minor = [major / 10, major / 5, major / 2]
            .into_iter()
            .find(|&m| m >= 1 && m as f32 * self.zoom >= 6.0)
            .unwrap_or(major);
        let font = FontId::monospace(9.0);
        let tick = Stroke::new(1.0, theme::RULER_TICK);
        // Rango local visible en cada eje.
        let (wx0, wy0) = self.to_world(rect, rect.min);
        let (wx1, wy1) = self.to_world(rect, rect.max);
        let span = |w0: f32, w1: f32, o: i32| {
            let a = ((w0 - o as f32) / minor as f32).floor() as i32 - 1;
            let b = ((w1 - o as f32) / minor as f32).ceil() as i32 + 1;
            a..=b
        };
        for k in span(wx0, wx1, origin.0) {
            let u = k * minor;
            let x = self.to_screen(rect, (origin.0 + u, 0)).x;
            if x < top.left() || x > top.right() {
                continue;
            }
            let is_major = u.rem_euclid(major) == 0;
            let h = if is_major { RULER_PX } else { RULER_PX * 0.35 };
            painter.vline(x, (top.bottom() - h)..=top.bottom(), tick);
            if is_major {
                painter.text(
                    Pos2::new(x + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    u.to_string(),
                    font.clone(),
                    theme::LABEL,
                );
            }
        }
        for k in span(wy0, wy1, origin.1) {
            let u = k * minor;
            let y = self.to_screen(rect, (0, origin.1 + u)).y;
            if y < left.top() || y > left.bottom() {
                continue;
            }
            let is_major = u.rem_euclid(major) == 0;
            let w = if is_major { RULER_PX } else { RULER_PX * 0.35 };
            painter.hline((left.right() - w)..=left.right(), y, tick);
            if is_major {
                painter.text(
                    Pos2::new(left.left() + 1.0, y + 2.0),
                    Align2::LEFT_TOP,
                    u.to_string(),
                    font.clone(),
                    theme::LABEL,
                );
            }
        }
        // Posición del cursor.
        if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
            let mark = Stroke::new(1.0, theme::ACCENT);
            painter.vline(p.x, top.y_range(), mark);
            painter.hline(left.x_range(), p.y, mark);
        }
    }
}

/// Cursor de cuentagotas: la punta está exactamente en `tip`; al lado, el color de debajo.
fn draw_pipette(painter: &egui::Painter, tip: Pos2, color: Option<cs_core::Rgba>) {
    let d = Vec2::new(1.0, -1.0).normalized();
    // Tamaño general del cursor.
    let k = 1.5;
    let dark = Color32::from_rgb(0x14, 0x14, 0x18);
    let needle_end = tip + d * (5.0 * k);
    let tube_end = tip + d * (14.0 * k);
    let bulb = tip + d * (18.0 * k);
    // Contorno oscuro y luego relleno claro, para que se vea sobre cualquier fondo.
    painter.line_segment([tip, needle_end], Stroke::new(3.0 * k, dark));
    painter.line_segment([needle_end, tube_end], Stroke::new(7.0 * k, dark));
    painter.circle_filled(bulb, 4.5 * k, dark);
    painter.line_segment([tip, needle_end], Stroke::new(1.2 * k, Color32::WHITE));
    painter.line_segment(
        [needle_end, tube_end],
        Stroke::new(4.0 * k, Color32::from_rgb(0xdd, 0xe6, 0xf0)),
    );
    let perp = Vec2::new(d.y, -d.x) * (4.5 * k);
    painter.line_segment(
        [tube_end - perp, tube_end + perp],
        Stroke::new(2.5 * k, dark),
    );
    painter.circle_filled(bulb, 3.0 * k, theme::ACCENT);
    // Muestra del color bajo la punta.
    let swatch = Rect::from_min_size(tip + Vec2::splat(8.0 * k), Vec2::splat(12.0 * k));
    painter.rect_filled(swatch.expand(1.0), 2.0, dark);
    match color {
        Some(c) if c.a > 0 => {
            painter.rect_filled(
                swatch,
                1.0,
                Color32::from_rgba_unmultiplied(c.r, c.g, c.b, 255),
            );
        }
        _ => {
            painter.rect_filled(swatch, 1.0, theme::CHECKER_LIGHT);
            painter.line_segment(
                [swatch.left_bottom(), swatch.right_top()],
                Stroke::new(1.5, Color32::from_rgb(0xd9, 0x57, 0x63)),
            );
        }
    }
}

/// Línea de una guía dentro del artboard (si cae dentro de él).
fn draw_guide(painter: &egui::Painter, screen: Rect, guide: Guide, zoom: f32, color: Color32) {
    let stroke = Stroke::new(1.0, color);
    match guide.axis {
        Axis::Horizontal => {
            let y = screen.top() + guide.position as f32 * zoom;
            if (screen.top()..=screen.bottom()).contains(&y) {
                painter.hline(screen.x_range(), painter.round_to_pixel_center(y), stroke);
            }
        }
        Axis::Vertical => {
            let x = screen.left() + guide.position as f32 * zoom;
            if (screen.left()..=screen.right()).contains(&x) {
                painter.vline(painter.round_to_pixel_center(x), screen.y_range(), stroke);
            }
        }
    }
}

/// Líneas del grid de trabajo dentro del artboard (se omiten si quedan a menos de 4 px).
fn draw_grid(painter: &egui::Painter, screen: Rect, grid: GridSettings, zoom: f32, color: Color32) {
    let (sx, sy) = (
        grid.size.0.max(1) as f32 * zoom,
        grid.size.1.max(1) as f32 * zoom,
    );
    if sx < 4.0 || sy < 4.0 {
        return;
    }
    let stroke = Stroke::new(1.0, color);
    let first = |offset: i32, step: f32| (offset as f32 * zoom).rem_euclid(step);
    let mut x = screen.left() + first(grid.offset.0, sx);
    while x < screen.right() {
        if x > screen.left() {
            painter.vline(x, screen.y_range(), stroke);
        }
        x += sx;
    }
    let mut y = screen.top() + first(grid.offset.1, sy);
    while y < screen.bottom() {
        if y > screen.top() {
            painter.hline(screen.x_range(), y, stroke);
        }
        y += sy;
    }
}

fn label_text(artboard: &Artboard) -> String {
    format!(
        "{}  {}×{}",
        artboard.name, artboard.sprite.width, artboard.sprite.height
    )
}

impl CanvasView {
    /// Artboard cuya etiqueta (nombre sobre el borde superior) está bajo `p`, el de más arriba primero.
    fn label_at(&self, ui: &egui::Ui, rect: Rect, editor: &Editor, p: Pos2) -> Option<ArtboardId> {
        editor.document.artboards.iter().rev().find_map(|artboard| {
            let galley = ui.painter().layout_no_wrap(
                label_text(artboard),
                FontId::proportional(LABEL_SIZE),
                Color32::WHITE,
            );
            let bottom_left = self.to_screen(rect, artboard.position) - Vec2::new(0.0, LABEL_GAP);
            let label =
                Rect::from_min_size(bottom_left - Vec2::new(0.0, galley.size().y), galley.size())
                    .expand(2.0);
            label.contains(p).then_some(artboard.id)
        })
    }
}

fn artboard_cursor(hit: Option<ArtboardHit>) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match hit {
        Some(ArtboardHit::Edge(_, e)) => match e {
            Edges {
                left: true,
                top: true,
                ..
            }
            | Edges {
                right: true,
                bottom: true,
                ..
            } => C::ResizeNwSe,
            Edges {
                left: true,
                bottom: true,
                ..
            }
            | Edges {
                right: true,
                top: true,
                ..
            } => C::ResizeNeSw,
            Edges { left: true, .. } | Edges { right: true, .. } => C::ResizeHorizontal,
            _ => C::ResizeVertical,
        },
        Some(ArtboardHit::Inside(_)) => C::Move,
        None => C::Crosshair,
    }
}

/// Rectángulos de los slices del frame actual, con nombre y pivote; el seleccionado, más grueso.
fn draw_slices(
    painter: &egui::Painter,
    sprite: &cs_core::Sprite,
    editor: &Editor,
    origin: Pos2,
    zoom: f32,
) {
    let at = |x: i32, y: i32| origin + Vec2::new(x as f32, y as f32) * zoom;
    for (i, slice) in sprite.slices.iter().enumerate() {
        let Some(key) = slice.key_at(editor.frame) else {
            continue;
        };
        let (x, y, w, h) = key.bounds;
        let r = Rect::from_min_max(at(x, y), at(x + w as i32, y + h as i32));
        let c = slice.color;
        let color = Color32::from_rgb(c.r, c.g, c.b);
        let selected = editor.slice == Some(i);
        painter.rect_filled(
            r,
            0.0,
            color.gamma_multiply(if selected { 0.18 } else { 0.08 }),
        );
        painter.rect_stroke(
            r,
            0.0,
            Stroke::new(if selected { 2.0 } else { 1.0 }, color),
            StrokeKind::Inside,
        );
        // 9-slice: líneas del centro de lado a lado del slice.
        if let Some((cx, cy, cw, ch)) = key.center {
            let stroke = Stroke::new(1.0, color.gamma_multiply(0.8));
            for gx in [x + cx, x + cx + cw as i32] {
                painter.line_segment([at(gx, y), at(gx, y + h as i32)], stroke);
            }
            for gy in [y + cy, y + cy + ch as i32] {
                painter.line_segment([at(x, gy), at(x + w as i32, gy)], stroke);
            }
        }
        painter.text(
            r.left_top() + Vec2::new(2.0, -1.0),
            Align2::LEFT_BOTTOM,
            &slice.name,
            FontId::proportional(11.0),
            color,
        );
        if let Some((px, py)) = key.pivot {
            let p = at(x + px, y + py);
            let s = 5.0;
            let stroke = Stroke::new(1.5, Color32::WHITE);
            painter.line_segment([p - Vec2::new(s, 0.0), p + Vec2::new(s, 0.0)], stroke);
            painter.line_segment([p - Vec2::new(0.0, s), p + Vec2::new(0.0, s)], stroke);
            painter.circle_stroke(p, 2.5, Stroke::new(1.0, color));
        }
    }
}

/// Dibuja el borde de la máscara con segmentos blanco/negro que se desplazan con `phase`.
fn draw_marching_ants(painter: &egui::Painter, mask: &Mask, origin: Pos2, zoom: f32, phase: i32) {
    let at = |x: i32, y: i32| origin + Vec2::new(x as f32, y as f32) * zoom;
    let color = |n: i32| {
        if (n + phase).rem_euclid(2) == 0 {
            Color32::WHITE
        } else {
            Color32::BLACK
        }
    };
    for (x, y) in mask.points() {
        // Cada lado del píxel sin vecino seleccionado es borde.
        let edges = [
            (!mask.contains(x, y - 1), at(x, y), at(x + 1, y), x),
            (!mask.contains(x, y + 1), at(x, y + 1), at(x + 1, y + 1), x),
            (!mask.contains(x - 1, y), at(x, y), at(x, y + 1), y),
            (!mask.contains(x + 1, y), at(x + 1, y), at(x + 1, y + 1), y),
        ];
        for (is_edge, a, b, n) in edges {
            if is_edge {
                painter.line_segment([a, b], Stroke::new(1.0, color(n)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoscroll_only_near_edges_and_grows_with_distance() {
        let area = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 300.0));
        assert_eq!(
            autoscroll_velocity(area, Pos2::new(200.0, 150.0)),
            Vec2::ZERO
        );
        let near = autoscroll_velocity(area, Pos2::new(390.0, 150.0));
        let far = autoscroll_velocity(area, Pos2::new(460.0, 150.0));
        assert!(near.x > 0.0 && far.x > near.x && near.y == 0.0);
        let up_left = autoscroll_velocity(area, Pos2::new(-500.0, -500.0));
        assert_eq!(
            up_left,
            Vec2::new(-AUTOSCROLL_SPEED, -AUTOSCROLL_SPEED),
            "saturates"
        );
    }
}
