use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use cs_core::{Artboard, ArtboardId, Axis, GridSettings, Guide, Mask};
use cs_tools::{ArtboardHit, BrushShape, Edges, hit_test};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};

use crate::artboard_window::{ArtboardWindow, TitleHit, WindowLayout};
use crate::canvas_size::{CanvasSizeDialog, GuideEdge};
use crate::editor::{Editor, MAX_BRUSH_SIZE, ToolKind};
use crate::guides::{RULER_PX, SNAP_PX, SnapContext, after_drag, hit_guide, nice_step};
use crate::menus;
use crate::pixel_cursors::{self, PixelCursor};
use crate::pixel_rulers::{self, PixelRulers};
use crate::pixel_selection;
use crate::skin::Skin;
use crate::transform_box::{self, Handle, TransformBox, TransformHit};
use crate::view::{ViewSettings, ZoomRequest};
use crate::{theme, widgets};

const ZOOM_LEVELS: [f32; 13] = [
    0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0,
];
/// Lado de cada cuadro del damero de transparencia, en píxeles del sprite (como Aseprite): siempre
/// 16×16 del dibujo, así que crece y se achica con el zoom. Empieza en la esquina del artboard.
const CHECKER_CELL: f32 = 16.0;
/// Velocidad de las "marching ants" (pasos por segundo).
const ANTS_SPEED: f64 = 6.0;
/// Tolerancia en pantalla para tomar bordes de artboards.
const EDGE_SLOP_PX: f32 = 6.0;
/// Etiqueta con el nombre sobre cada artboard (arrastrable para moverlo).
const LABEL_SIZE: f32 = 12.0;
const LABEL_GAP: f32 = 4.0;
/// Los números de tile se dibujan solo si cada tile ocupa al menos esto en pantalla.
const TILE_NUMBER_MIN_PX: f32 = 16.0;
/// Lado en pantalla de cada handle de la selección, sea cual sea el zoom.
const HANDLE_PX: f32 = 8.0;
/// Franja en pantalla por fuera de cada esquina que rota la selección.
const ROTATE_PX: f32 = 12.0;

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
    /// Dibujar el cursor de rotación (egui no trae uno; el del sistema se oculta).
    draw_rotate: bool,
    /// Con Pixel UI, el cursor del diseño a dibujar (el del sistema se oculta).
    pixel_cursor: Option<PixelCursor>,
    /// Nombre de artboard en edición (doble clic en su etiqueta) y si ya se le dio el foco.
    renaming: Option<(ArtboardId, String, bool)>,
    /// Clic derecho sobre un slice con la herramienta Slice: al soltar se abre su menú.
    slice_menu: bool,
    /// Clic derecho dentro de la selección con una herramienta de selección o Move: al soltar se
    /// abre su menú (voltear…).
    selection_menu: bool,
    /// Diálogo Slice Properties abierto (para el slice seleccionado).
    pub slice_properties: bool,
    /// Las notas (post-its) del lienzo: su arrastre, su edición y su cursor.
    pub notes: crate::notes::NotesView,
    /// Lo pedido desde el menú "≡" de un artboard que abre un diálogo de la app (la app lo toma
    /// y lo deja en `None`).
    pub artboard_request: Option<ArtboardRequest>,
    /// Ctrl+rueda de touchpad: píxeles acumulados que todavía no suman un paso de grosor.
    wheel_rest: f32,
    /// Color alrededor de los artboards (`background_for`); lo pone la app en cada frame.
    pub background: Color32,
    /// Las reglas del skin con Pixel UI (`None`: las de siempre); lo pone la app en cada frame.
    pub pixel_rulers: Option<PixelRulers>,
    /// Si las reglas se ven en este frame (tapan el borde de la vista).
    rulers_shown: bool,
    /// Con Pixel UI, la ventana del skin alrededor de cada artboard (`None`: el borde y la
    /// etiqueta de siempre); lo pone la app en cada frame.
    pub artboard_window: Option<ArtboardWindow>,
    /// Con Pixel UI: los cursores y la selección del diseño; lo pone la app en cada frame.
    pub pixel_ui: bool,
    /// Con Pixel UI, el color de los ejes de simetría del skin (y se dibujan como en Aseprite);
    /// lo pone la app en cada frame.
    pub symmetry_color: Option<Color32>,
    /// Con Pixel UI, los cursores del skin; lo pone la app en cada frame.
    pub cursors: Option<pixel_cursors::CursorSet>,
    /// Píxeles físicos por punto del último frame: [`Self::origin`] se alinea a ellos.
    ppp: f32,
}

/// Color alrededor de los artboards: `canvas` del skin con Pixel UI; si no, el del tema.
pub fn background_for(skin: Option<&Skin>) -> Color32 {
    skin.map_or(theme::CANVAS_BG, |skin| skin.colors().canvas)
}

/// Margen (en pantalla) desde el borde del lienzo en el que empieza el auto-scroll.
const AUTOSCROLL_MARGIN: f32 = 24.0;
/// Velocidad máxima del auto-scroll en píxeles de pantalla por segundo.
const AUTOSCROLL_SPEED: f32 = 700.0;

/// Píxeles de rueda (touchpad) que suman un paso de grosor con Ctrl+rueda.
const BRUSH_WHEEL_PX: f32 = 50.0;

/// Pasos enteros de grosor de un evento de rueda (positivo hacia arriba). Las ruedas de líneas o
/// páginas dan un paso por evento; las de píxeles se acumulan en `rest` hasta `BRUSH_WHEEL_PX`.
fn brush_wheel_steps(rest: &mut f32, unit: egui::MouseWheelUnit, dy: f32) -> i32 {
    if dy == 0.0 {
        return 0;
    }
    match unit {
        egui::MouseWheelUnit::Point => {
            *rest += dy;
            let steps = (*rest / BRUSH_WHEEL_PX).trunc();
            *rest -= steps * BRUSH_WHEEL_PX;
            steps as i32
        }
        egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => dy.signum() as i32,
    }
}

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
            draw_rotate: false,
            pixel_cursor: None,
            renaming: None,
            slice_menu: false,
            selection_menu: false,
            slice_properties: false,
            notes: Default::default(),
            artboard_request: None,
            wheel_rest: 0.0,
            background: theme::CANVAS_BG,
            pixel_rulers: None,
            rulers_shown: false,
            artboard_window: None,
            pixel_ui: false,
            cursors: None,
            symmetry_color: None,
            ppp: 1.0,
        }
    }
}

impl CanvasView {
    /// Posición en pantalla del origen del mundo, alineada a píxeles físicos (`self.ppp`).
    ///
    /// Es el único lugar donde se redondea la vista: con zoom entero (en píxeles físicos), las
    /// coordenadas enteras del mundo caen justo en píxeles de pantalla, así que la textura, las
    /// guías, los overlays y la entrada (`to_screen` / `to_world`) coinciden. Se redondea al
    /// usarlo, no en `pan`: el pan guardado sigue fraccionario y los arrastres no derivan.
    fn origin(&self, rect: Rect) -> Pos2 {
        let ppp = self.ppp;
        (((rect.center() + self.pan).to_vec2() * ppp).round() / ppp).to_pos2()
    }

    /// Dónde va una nota nueva: centrada en lo que se ve (su tamaño es de interfaz).
    pub fn new_note_position(&self) -> (i32, i32) {
        let size = Vec2::new(
            crate::notes::NEW_SIZE.0 as f32,
            crate::notes::NEW_SIZE.1 as f32,
        );
        let w = (-self.pan - size / 2.0) / self.zoom;
        (w.x.round() as i32, w.y.round() as i32)
    }

    fn to_screen(&self, rect: Rect, world: (i32, i32)) -> Pos2 {
        self.origin(rect) + Vec2::new(world.0 as f32, world.1 as f32) * self.zoom
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> (f32, f32) {
        let w = (p - self.origin(rect)) / self.zoom;
        (w.x, w.y)
    }

    /// Si se dibuja el cursor del diseño `kind` (Pixel UI y el skin lo trae).
    fn can_draw(&self, kind: PixelCursor) -> bool {
        self.pixel_ui && self.cursors.as_ref().is_some_and(|c| c.has(kind))
    }

    /// Color que se ve en el punto de pantalla `p` del lienzo: el píxel del artboard de más arriba
    /// sobre el damero, o el fondo fuera de los artboards (para invertir los cursores).
    fn color_under(&self, rect: Rect, editor: &Editor, p: Pos2) -> Color32 {
        let world = self.to_world(rect, p);
        let Some(artboard) = editor
            .document
            .artboard_at(world)
            .and_then(|id| editor.document.artboard(id))
        else {
            return self.background;
        };
        let (ax, ay) = artboard.position;
        let (x, y) = (world.0.floor() as i32 - ax, world.1.floor() as i32 - ay);
        let sprite = if artboard.id == editor.active {
            editor.sprite()
        } else {
            &artboard.sprite
        };
        let c = cs_render::composite_pixel(sprite, editor.frame_of(artboard.id), x, y);
        let cell = CHECKER_CELL as i32;
        let checker = if (x.div_euclid(cell) + y.div_euclid(cell)) % 2 == 0 {
            theme::CHECKER_LIGHT
        } else {
            theme::CHECKER_DARK
        };
        let a = c.a as f32 / 255.0;
        let mix = |v: u8, b: u8| (v as f32 * a + b as f32 * (1.0 - a)).round() as u8;
        Color32::from_rgb(
            mix(c.r, checker.r()),
            mix(c.g, checker.g()),
            mix(c.b, checker.b()),
        )
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

    /// La textura del artboard `id`, para los tests de la app.
    #[cfg(test)]
    pub(crate) fn artboard_texture(&self, id: ArtboardId) -> Option<egui::TextureId> {
        self.textures.get(&id).map(|t| t.id())
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

    /// Grosor de las reglas: el del mockup con Pixel UI; si no, el de siempre.
    fn ruler_px(&self) -> f32 {
        if self.pixel_rulers.is_some() {
            pixel_rulers::RULER_PX
        } else {
            RULER_PX
        }
    }

    /// Reglas superior e izquierda (sin la esquina), de `px` de grosor.
    fn ruler_rects(rect: Rect, px: f32) -> (Rect, Rect) {
        let top = Rect::from_min_max(
            rect.min + Vec2::new(px, 0.0),
            Pos2::new(rect.right(), rect.top() + px),
        );
        let left = Rect::from_min_max(
            rect.min + Vec2::new(0.0, px),
            Pos2::new(rect.left() + px, rect.bottom()),
        );
        (top, left)
    }

    /// Lo que se ve del lienzo: `rect` sin las reglas (si se ven).
    fn visible(&self, rect: Rect) -> Rect {
        if !self.rulers_shown {
            return rect;
        }
        let px = self.ruler_px();
        Rect::from_min_max(rect.min + Vec2::splat(px), rect.max)
    }

    /// Centro de simetría efectivo en pantalla (con el arrastre en curso, si lo hay).
    fn symmetry_center(&self, rect: Rect, editor: &Editor) -> (Pos2, (i32, i32)) {
        self.symmetry_center_of(rect, editor.artboard())
    }

    /// Como `symmetry_center`, del artboard `a` (el que se ve: mientras se mueve, en su lugar
    /// nuevo).
    fn symmetry_center_of(&self, rect: Rect, a: &cs_core::Artboard) -> (Pos2, (i32, i32)) {
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

    /// Ejes de simetría punteados sobre el artboard activo y su centro, como se ve en `document`
    /// (mientras se arrastra el artboard, se mueven con él).
    fn draw_symmetry(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        document: &cs_core::Document,
    ) {
        let a = document
            .artboard(editor.active)
            .unwrap_or_else(|| editor.artboard());
        let axes = a.symmetry.axes;
        if !axes.any() {
            return;
        }
        let ((x0, y0), (x1, y1)) = a.rect();
        let screen = Rect::from_min_max(
            self.to_screen(rect, (x0, y0)),
            self.to_screen(rect, (x1, y1)),
        );
        let painter = painter.with_clip_rect(screen.intersect(rect));
        let (c, _) = self.symmetry_center_of(rect, a);
        if let Some(color) = self.symmetry_color {
            // Pegados a lo visible: con zoom, los handles quedan en el borde de la vista aunque
            // el artboard siga más allá.
            draw_pixel_symmetry(
                &painter,
                axes,
                c,
                screen.intersect(self.visible(rect)),
                color,
            );
            return;
        }
        let stroke = Stroke::new(1.5, theme::ACCENT);
        let mut segments = Vec::new();
        if axes.vertical {
            segments.push([
                Pos2::new(c.x, screen.top()),
                Pos2::new(c.x, screen.bottom()),
            ]);
        }
        if axes.horizontal {
            segments.push([
                Pos2::new(screen.left(), c.y),
                Pos2::new(screen.right(), c.y),
            ]);
        }
        let k = screen.width().max(screen.height());
        if axes.diagonal_down {
            segments.push([c - Vec2::splat(k), c + Vec2::splat(k)]);
        }
        if axes.diagonal_up {
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

    fn handle_input(
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
            let in_area = pointer.filter(|p| area.contains(*p));
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
        // Antes de cualquier uso de `origin` en este frame (zoom, entrada, dibujo).
        self.ppp = ctx.pixels_per_point();
        self.rulers_shown = view_settings.rulers;
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
        painter.rect_filled(rect, 0.0, self.background);

        let ppp = ctx.pixels_per_point();
        for artboard in &document.artboards {
            // Alineada a píxeles de pantalla: el damero y la textura caen justo en píxeles y la
            // ventana de Pixel UI se arma alrededor de este mismo rect (blanco de 1 px exacto).
            // Con zoom entero en píxeles físicos `origin` ya lo deja alineado y el redondeo no
            // cambia nada; solo actúa con zoom fraccionario (p. ej. 50 %), donde los bordes del
            // artboard caen entre píxeles y se apartan de `to_screen` en menos de medio píxel.
            let screen = WindowLayout::pixels_on_screen(self.pixel_rect(rect, artboard), ppp);
            let active = artboard.id == editor.active;
            // Con Pixel UI, la ventana del skin: el marco va detrás de los píxeles. Si la ventana
            // queda fuera del lienzo, no se arma ni se dibuja nada del artboard.
            let window = match &self.artboard_window {
                Some(w) => {
                    if !WindowLayout::frame_around(screen, ppp).intersects(rect) {
                        continue;
                    }
                    Some((w, WindowLayout::around(screen, ppp)))
                }
                None => None,
            };
            if let Some((w, layout)) = &window {
                painter.extend(w.frame_shapes(layout, active, ppp));
            }
            let ((x0, y0), (x1, y1)) = artboard.rect();
            let uv = checker_uv((x1 - x0) as u32, (y1 - y0) as u32);
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
            let renaming = self.renaming.as_ref().is_some_and(|r| r.0 == artboard.id);
            if let Some((w, layout)) = &window {
                // La barra de título reemplaza a la etiqueta. Sin el borde naranja del activo:
                // taparía el borde y la barra de la ventana; los inactivos se ven apagados.
                let title = if self.names_visible && !renaming {
                    label_text(artboard)
                } else {
                    String::new()
                };
                painter.extend(w.bar_shapes(&painter, layout, &title, active, ppp));
                continue;
            }
            let (border, label_color) = if active {
                (Stroke::new(2.0, theme::ACCENT), theme::ACCENT)
            } else {
                (Stroke::new(1.0, theme::CANVAS_BORDER), theme::LABEL)
            };
            painter.rect_stroke(screen, 0.0, border, StrokeKind::Outside);
            if self.names_visible && !renaming {
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
            self.draw_symmetry(&painter, rect, editor, document);
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
            draw_marching_ants(&painter, mask, origin, self.zoom, phase, self.pixel_ui);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }
        if let Some(origin) = active_origin
            && handles_active(editor)
            && let Some(b) = editor.transform_box()
        {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_transform_box(&painter, &b, origin, self.zoom, phase, self.pixel_ui);
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
        let tool = editor.active_tool();
        // Con Espacio o Hand se está por mover la vista: sin contorno.
        let space = ui.input(|i| i.key_down(egui::Key::Space)) || tool == ToolKind::Hand;
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
            // Bloqueada (modo Tiles o editando una máscara): no actúa, así que sin contorno.
            && !editor.tool_blocked()
            && view_settings.shows(view_settings.brush_preview)
        {
            let size = if tool.uses_brush() {
                editor.options.brush_size
            } else {
                1
            };
            let shape = editor.options.brush_shape;
            let offset = -((size as i32 - 1) / 2);
            // En modo Tiles el pincel cubre celdas enteras.
            let cells = editor.tile_space().filter(|_| tool.uses_cells());
            // Las herramientas que pintan píxeles muestran el color primario bajo el cursor.
            let fill = (cells.is_none()
                && tool.paints()
                && tool != ToolKind::Eraser
                && tool != ToolKind::Bucket
                && tool != ToolKind::Blur)
                .then(|| {
                    let c = editor.resolve(editor.primary);
                    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
                });
            let stroke = Stroke::new(1.0, theme::BRUSH_OUTLINE);
            if cells.is_none() && tool.uses_brush() && shape == BrushShape::Round && size > 2 {
                // Redondo: filas rellenas y el borde píxel a píxel (en 1 y 2 es un cuadrado).
                let points = cs_raster::brush_points((hx, hy), size, shape);
                let at = |(x, y): (i32, i32)| origin + Vec2::new(x as f32, y as f32) * self.zoom;
                if let Some(fill) = fill {
                    for (y, x0, x1) in brush_rows(&points) {
                        let row = Rect::from_min_max(at((x0, y)), at((x1 + 1, y + 1)));
                        painter.rect_filled(row, 0.0, fill);
                    }
                }
                for segment in outline_segments(&points, at, stroke.width) {
                    painter.line_segment(segment, stroke);
                }
            } else {
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
                if let Some(fill) = fill {
                    painter.rect_filled(r, 0.0, fill);
                }
                painter.rect_stroke(r, 0.0, stroke, StrokeKind::Inside);
            }
        }

        // Las notas (post-its): encima de los artboards y debajo de las reglas.
        {
            let (origin, zoom) = (self.origin(rect), self.zoom);
            let to_screen = move |(x, y): (i32, i32)| origin + Vec2::new(x as f32, y as f32) * zoom;
            let skin = crate::pixel_ui::frame_skin(ui.ctx());
            let note_painter = painter.with_clip_rect(self.canvas_area(rect, view_settings));
            let menus = self
                .notes
                .show(ui, &note_painter, editor, skin.as_deref(), &to_screen);
            self.notes.menus(ui, editor, menus);
        }
        if view_settings.rulers {
            let pointer = response.hover_pos().or(ui.ctx().pointer_interact_pos());
            match self.pixel_rulers {
                Some(look) => {
                    // La carita: sorprendida si se arrastra una guía, callada si están
                    // bloqueadas; si no, parpadea cada tanto.
                    use pixel_rulers::{BLINK_EVERY, BLINK_FOR, Mood};
                    let time = ui.input(|i| i.time);
                    let phase = time % BLINK_EVERY;
                    let mood = if self.ruler_drag.is_some() {
                        Mood::Surprised
                    } else if view_settings.lock_guides {
                        Mood::Quiet
                    } else if phase < BLINK_FOR {
                        Mood::Blink
                    } else {
                        Mood::Normal
                    };
                    if matches!(mood, Mood::Normal | Mood::Blink) {
                        let wait = if phase < BLINK_FOR {
                            BLINK_FOR - phase
                        } else {
                            BLINK_EVERY - phase
                        };
                        ui.ctx()
                            .request_repaint_after(Duration::from_secs_f64(wait.max(0.01)));
                    }
                    let look = look.with_mood(mood);
                    self.draw_pixel_rulers(&painter, rect, editor, pointer, &look)
                }
                None => self.draw_rulers(&painter, rect, editor, pointer),
            }
        }

        // Color que tomaría el cuentagotas: lo que se ve (todas las capas visibles).
        let picked = || {
            self.hover.and_then(|(x, y)| {
                let s = editor.sprite();
                let inside = x >= 0 && y >= 0 && x < s.width as i32 && y < s.height as i32;
                // `composite_pixel` lee los tilemaps tile a tile: no hace falta materializar en
                // cada frame de hover.
                inside.then(|| cs_render::composite_pixel(s, editor.frame, x, y))
            })
        };
        if self.draw_pipette
            && let Some(p) = response.hover_pos()
        {
            draw_pipette(&painter, p, picked());
        }
        if let Some(cursor) = self.pixel_cursor
            && let Some(p) = response.hover_pos()
        {
            // El píxel del sprite bajo el puntero, en pantalla y alineado a píxeles físicos.
            let (wx, wy) = self.to_world(rect, p);
            let (x, y) = (wx.floor() as i32, wy.floor() as i32);
            let cell = Rect::from_min_max(
                self.to_screen(rect, (x, y)),
                self.to_screen(rect, (x + 1, y + 1)),
            );
            let ppp = self.ppp;
            let cell = Rect::from_min_max(
                (cell.min.to_vec2() * ppp).round().to_pos2() / ppp,
                (cell.max.to_vec2() * ppp).round().to_pos2() / ppp,
            );
            let color = (cursor == PixelCursor::Eyedropper).then(picked).flatten();
            let under = |q: Pos2| self.color_under(rect, editor, q);
            let cursors = self.cursors.as_ref().expect("checked by can_draw");
            pixel_cursors::draw(&painter, cursors, cursor, p, cell, color, under);
        }
        if self.draw_rotate
            && let Some(p) = response.hover_pos()
        {
            draw_rotate_cursor(&painter, p);
        }
        self.rename_field(ui, rect, editor);
        // La ventana Slice Properties la muestra la app (con el skin): `slice_properties`.
        self.slice_properties &= editor.slice.is_some();
    }

    /// Reglas en píxeles del artboard activo (origen en su esquina superior izquierda).
    fn draw_rulers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        pointer: Option<Pos2>,
    ) {
        let (top, left) = Self::ruler_rects(rect, RULER_PX);
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
        let minor = minor_step(major, self.zoom);
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

/// Las marcas visibles de una regla: `(valor local, posición en pantalla, es grande)` para el
/// rango de mundo `w0..w1`, con el artboard en `origin` del eje. `screen` pasa de mundo a
/// pantalla.
fn ruler_ticks(
    w0: f32,
    w1: f32,
    origin: i32,
    major: i32,
    minor: i32,
    screen: impl Fn(i32) -> f32,
) -> impl Iterator<Item = (i32, f32, bool)> {
    let a = ((w0 - origin as f32) / minor as f32).floor() as i32 - 1;
    let b = ((w1 - origin as f32) / minor as f32).ceil() as i32 + 1;
    (a..=b).map(move |k| {
        let u = k * minor;
        (u, screen(origin + u), u.rem_euclid(major) == 0)
    })
}

/// La marca chica: un décimo, un quinto o la mitad de la grande, la primera que mida al menos
/// 6 puntos en pantalla.
fn minor_step(major: i32, zoom: f32) -> i32 {
    [major / 10, major / 5, major / 2]
        .into_iter()
        .find(|&m| m >= 1 && m as f32 * zoom >= 6.0)
        .unwrap_or(major)
}

impl CanvasView {
    /// Las reglas con Pixel UI, como en el mockup (ver `pixel_rulers`): la misma escala que las
    /// de siempre, con marcas grandes cada `MAJOR_MIN_PX` como mínimo. Cada marca va en la
    /// columna (o fila) de píxeles justo antes de su borde: el 0 del mockup cae sobre la línea
    /// oscura y el lienzo empieza al lado.
    fn draw_pixel_rulers(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        pointer: Option<Pos2>,
        look: &PixelRulers,
    ) {
        painter.extend(look.frame_shapes(rect));
        let origin = editor.artboard().position;
        let major = nice_step(self.zoom, pixel_rulers::MAJOR_MIN_PX);
        let minor = minor_step(major, self.zoom);
        let (wx0, wy0) = self.to_world(rect, rect.min);
        let (wx1, wy1) = self.to_world(rect, rect.max);
        // Cada regla recorta lo suyo: las marcas no pisan la esquina ni el lienzo.
        let clip = |strip: Rect| painter.with_clip_rect(strip.intersect(painter.clip_rect()));
        let top = clip(PixelRulers::top_strip(rect));
        let screen_x = |x: i32| self.to_screen(rect, (x, 0)).x.round() - 1.0;
        for (u, x, is_major) in ruler_ticks(wx0, wx1, origin.0, major, minor, screen_x) {
            top.add(look.top_tick(x, rect.top(), is_major));
            if is_major {
                top.extend(look.top_label(u, x, rect.top()));
            }
        }
        let left = clip(PixelRulers::left_strip(rect));
        let screen_y = |y: i32| self.to_screen(rect, (0, y)).y.round() - 1.0;
        for (u, y, is_major) in ruler_ticks(wy0, wy1, origin.1, major, minor, screen_y) {
            left.add(look.left_tick(y, rect.left(), is_major));
            if is_major {
                left.extend(look.left_label(u, y, rect.left()));
            }
        }
        if let Some(p) = pointer.filter(|p| rect.contains(*p)) {
            painter.add(look.pointer_shapes(rect, p));
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
    /// Campo para renombrar el artboard sobre su etiqueta: Enter o clic afuera aplica, Esc cancela.
    fn rename_field(&mut self, ui: &mut egui::Ui, rect: Rect, editor: &mut Editor) {
        let Some((id, mut name, focused)) = self.renaming.take() else {
            return;
        };
        let Some(artboard) = editor.document.artboard(id) else {
            return;
        };
        let (field, font) = match self.artboard_window {
            // Con Pixel UI, sobre el título de la barra.
            Some(_) => {
                let ppp = ui.ctx().pixels_per_point();
                let layout = WindowLayout::around(self.pixel_rect(rect, artboard), ppp);
                let area = layout.text.unwrap_or(layout.bar);
                let font = ArtboardWindow::font();
                let height = font.size + 2.0;
                let field = Rect::from_min_size(
                    Pos2::new(area.left(), layout.bar.center().y - height / 2.0),
                    Vec2::new(area.width().max(80.0), height),
                );
                (field, font)
            }
            None => {
                let bottom_left =
                    self.to_screen(rect, artboard.position) - Vec2::new(0.0, LABEL_GAP);
                let size = Vec2::new(160.0, LABEL_SIZE + 6.0);
                let field = Rect::from_min_size(bottom_left - Vec2::new(0.0, size.y), size);
                (field, FontId::proportional(LABEL_SIZE))
            }
        };
        let response = ui.put(
            field,
            egui::TextEdit::singleline(&mut name)
                .font(font)
                .margin(Vec2::new(2.0, 1.0)),
        );
        if !focused {
            response.request_focus();
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            return;
        }
        if focused && response.lost_focus() {
            let name = name.trim();
            if !name.is_empty() {
                editor.activate(id);
                editor.rename_active_artboard(name.to_owned());
            }
            return;
        }
        self.renaming = Some((id, name, true));
    }

    /// Área de píxeles del artboard en pantalla.
    fn pixel_rect(&self, rect: Rect, artboard: &Artboard) -> Rect {
        let ((x0, y0), (x1, y1)) = artboard.rect();
        Rect::from_min_max(
            self.to_screen(rect, (x0, y0)),
            self.to_screen(rect, (x1, y1)),
        )
    }

    /// Lo visible del lienzo: `rect` sin las reglas, si se muestran.
    fn canvas_area(&self, rect: Rect, view: &ViewSettings) -> Rect {
        if view.rulers {
            Rect::from_min_max(rect.min + Vec2::splat(self.ruler_px()), rect.max)
        } else {
            rect
        }
    }

    /// El menú del "≡" de cada ventana de artboard (con Pixel UI): se abre al hacer clic, como un
    /// desplegable de la barra de menús.
    fn artboard_menus(&mut self, ui: &egui::Ui, rect: Rect, editor: &mut Editor) {
        if self.artboard_window.is_none() {
            return;
        }
        let ppp = ui.ctx().pixels_per_point();
        let menus: Vec<(ArtboardId, Rect)> = editor
            .document
            .artboards
            .iter()
            .filter_map(|a| {
                let layout = WindowLayout::around(self.pixel_rect(rect, a), ppp);
                Some((a.id, layout.menu?))
            })
            .filter(|(_, r)| rect.intersects(*r))
            .collect();
        for (id, menu_rect) in menus {
            let response = ui.interact(
                menu_rect,
                ui.id().with(("artboard_menu", id)),
                Sense::click(),
            );
            let content = |ui: &mut egui::Ui| self.artboard_menu(ui, editor, id);
            match crate::pixel_ui::frame_skin(ui.ctx()) {
                Some(skin) => {
                    let look = menus::MenuLook::new(&skin, menu_rect.top());
                    menus::dropdown(&look, &response, menu_rect, content);
                }
                None => {
                    egui::Popup::menu(&response).show(content);
                }
            }
        }
    }

    /// Lo que hay en el menú "≡" del artboard `id`.
    fn artboard_menu(&mut self, ui: &mut egui::Ui, editor: &mut Editor, id: ArtboardId) {
        editor.activate(id);
        if ui.add(menus::plain("Rename")).clicked()
            && let Some(artboard) = editor.document.artboard(id)
        {
            self.renaming = Some((id, artboard.name.clone(), false));
            ui.close();
        }
        if ui.add(menus::item("Properties", "Ctrl+P")).clicked() {
            self.artboard_request = Some(ArtboardRequest::Properties);
            ui.close();
        }
        if ui.add(menus::plain("Duplicate")).clicked() {
            let name = format!("{} Copy", editor.artboard().name);
            editor.duplicate_artboard(name);
            ui.close();
        }
        menus::separator(ui);
        let transformable = cs_commands::sprite_ops::can_transform(editor.sprite());
        if ui
            .add_enabled(transformable, menus::item("Sprite Size", "Ctrl+Alt+I"))
            .clicked()
        {
            self.artboard_request = Some(ArtboardRequest::SpriteSize);
            ui.close();
        }
        if ui.add(menus::item("Canvas Size", "C")).clicked() {
            self.artboard_request = Some(ArtboardRequest::CanvasSize);
            ui.close();
        }
        menus::separator(ui);
        let can_delete = editor.document.artboards.len() > 1;
        if ui.add_enabled(can_delete, menus::plain("Delete")).clicked() {
            editor.remove_active_artboard();
            ui.close();
        }
    }

    /// Artboard cuya etiqueta (nombre sobre el borde superior) o, con Pixel UI, cuya barra de
    /// título está bajo `p`, y qué parte; el de más arriba primero. La barra de Pixel UI responde
    /// aunque los nombres estén ocultos (la ventana se ve igual). Solo dentro de `area` (lo
    /// visible del lienzo): bajo las reglas o fuera del lienzo no hay barra.
    fn title_at(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        area: Rect,
        editor: &Editor,
        p: Pos2,
    ) -> Option<(ArtboardId, TitleHit)> {
        if !area.contains(p) {
            return None;
        }
        let ppp = ui.ctx().pixels_per_point();
        if self.artboard_window.is_some() {
            // Decide la ventana de más arriba que tiene a `p` (marco, barra o píxeles): las barras
            // de los de abajo quedan tapadas.
            let (artboard, pixels) = editor.document.artboards.iter().rev().find_map(|a| {
                let pixels = self.pixel_rect(rect, a);
                WindowLayout::frame_around(pixels, ppp)
                    .contains(p)
                    .then_some((a, pixels))
            })?;
            let hit = WindowLayout::around(pixels, ppp).hit(p)?;
            // Con la herramienta Artboard, cerca del borde de arriba o de sus esquinas (con la
            // tolerancia de la herramienta) redimensionar gana a la barra.
            if editor.active_tool() == ToolKind::Artboard {
                let slop = EDGE_SLOP_PX / self.zoom;
                let world = self.to_world(rect, p);
                if let Some(ArtboardHit::Edge(id, _)) = hit_test(&editor.document, world, slop)
                    && id == artboard.id
                {
                    return None;
                }
            }
            return Some((artboard.id, hit));
        }
        editor.document.artboards.iter().rev().find_map(|artboard| {
            if !self.names_visible {
                return None;
            }
            let galley = ui.painter().layout_no_wrap(
                label_text(artboard),
                FontId::proportional(LABEL_SIZE),
                Color32::WHITE,
            );
            let bottom_left = self.to_screen(rect, artboard.position) - Vec2::new(0.0, LABEL_GAP);
            let label =
                Rect::from_min_size(bottom_left - Vec2::new(0.0, galley.size().y), galley.size())
                    .expand(2.0);
            label.contains(p).then_some((artboard.id, TitleHit::Bar))
        })
    }
}

/// Lo que el menú "≡" de un artboard le pide a la app (abrir uno de sus diálogos).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtboardRequest {
    Properties,
    SpriteSize,
    CanvasSize,
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

/// Cursor de redimensionar según la dirección del handle (sin tener en cuenta la rotación).
fn handle_cursor(handle: Handle) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match handle.dirs() {
        (0, _) => C::ResizeVertical,
        (_, 0) => C::ResizeHorizontal,
        (x, y) if x == y => C::ResizeNwSe,
        _ => C::ResizeNeSw,
    }
}

/// Se muestran (y se pueden tomar) los handles: no durante el gesto de otra herramienta.
fn handles_active(editor: &Editor) -> bool {
    editor.shows_handles() && (!editor.is_painting() || editor.is_transforming())
}

/// Contorno (rotado) de la caja de transformación y sus 8 handles de tamaño fijo en pantalla,
/// por fuera de la caja (donde se toman). Con `pixel` (Pixel UI) y sin rotar, los del diseño.
fn draw_transform_box(
    painter: &egui::Painter,
    b: &TransformBox,
    origin: Pos2,
    zoom: f32,
    phase: i32,
    pixel: bool,
) {
    let at = |(x, y): (f32, f32)| origin + Vec2::new(x, y) * zoom;
    if pixel && b.angle == 0.0 {
        let r = Rect::from_min_max(
            at((b.x0 as f32, b.y0 as f32)),
            at((b.x1 as f32, b.y1 as f32)),
        );
        pixel_selection::paint(painter, pixel_selection::rect_outline(r, phase));
        for handle in Handle::ALL {
            pixel_selection::draw_handle(painter, pixel_selection::handle_rect(r, handle.dirs()));
        }
        return;
    }
    let outline: Vec<Pos2> = b.corners().into_iter().map(at).collect();
    painter.add(egui::Shape::closed_line(
        outline,
        Stroke::new(1.0, theme::ACCENT),
    ));
    for handle in Handle::ALL {
        let center = b.handle_center(handle, HANDLE_PX / 2.0 / zoom);
        let r = Rect::from_center_size(at(center), Vec2::splat(HANDLE_PX));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    }
}

/// Con Pixel UI, los ejes de simetría como en Aseprite: líneas de 1 punto en `color`, enteras
/// de borde a borde del artboard (`screen`). Las rectas llevan un handle de 5×5 (los de la
/// selección) en cada punta, adentro del artboard; las diagonales son una escalera de píxeles
/// que pasa por el centro `c`.
fn draw_pixel_symmetry(
    painter: &egui::Painter,
    axes: cs_core::SymmetryAxes,
    c: Pos2,
    screen: Rect,
    color: Color32,
) {
    let handle = crate::pixel_selection::HANDLE;
    let half = (handle / 2.0).floor();
    let screen = Rect::from_min_max(screen.min.round(), screen.max.round());
    let (x, y) = (c.x.floor(), c.y.floor());
    let px = |r: Rect| egui::Shape::rect_filled(r, 0.0, color);
    let mut shapes = Vec::new();
    let mut handles = Vec::new();
    if axes.vertical {
        let line = Rect::from_x_y_ranges(
            x..=x + 1.0,
            screen.top() + handle..=screen.bottom() - handle,
        );
        shapes.push(px(line));
        for top in [screen.top(), screen.bottom() - handle] {
            handles.push(Rect::from_min_size(
                Pos2::new(x - half, top),
                Vec2::splat(handle),
            ));
        }
    }
    if axes.horizontal {
        let line = Rect::from_x_y_ranges(
            screen.left() + handle..=screen.right() - handle,
            y..=y + 1.0,
        );
        shapes.push(px(line));
        for left in [screen.left(), screen.right() - handle] {
            handles.push(Rect::from_min_size(
                Pos2::new(left, y - half),
                Vec2::splat(handle),
            ));
        }
    }
    // Las diagonales: un punto por fila, desde el centro hacia los dos lados.
    let rows = (screen.height().max(screen.width())) as i32;
    for (on, dir) in [(axes.diagonal_down, 1.0), (axes.diagonal_up, -1.0)] {
        if !on {
            continue;
        }
        for i in -rows..=rows {
            let at = Pos2::new(x + dir * i as f32, y + i as f32);
            if screen.contains(at) {
                shapes.push(px(Rect::from_min_size(at, Vec2::splat(1.0))));
            }
        }
    }
    painter.extend(shapes);
    for r in handles {
        crate::pixel_selection::draw_handle(painter, r);
    }
}

/// Flecha curva (sentido horario) en el puntero, para rotar: egui no trae ese cursor.
fn draw_rotate_cursor(painter: &egui::Painter, at: Pos2) {
    use std::f32::consts::PI;
    let r = 7.0;
    let arc: Vec<Pos2> = (0..=12)
        .map(|i| at + Vec2::angled(-PI / 2.0 + i as f32 * (1.5 * PI / 12.0)) * r)
        .collect();
    let end = arc[arc.len() - 1];
    // Al final del arco (a la izquierda) el sentido horario apunta hacia arriba.
    let head = vec![end + Vec2::new(-3.0, 4.0), end, end + Vec2::new(3.0, 4.0)];
    for (width, color) in [(3.0, Color32::BLACK), (1.5, Color32::WHITE)] {
        let stroke = Stroke::new(width, color);
        painter.add(egui::Shape::line(arc.clone(), stroke));
        painter.add(egui::Shape::line(head.clone(), stroke));
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

/// Dibuja el borde de la máscara con segmentos blanco/negro que se desplazan con `phase`. Con
/// `pixel` (Pixel UI), el del diseño: 1 punto con tramos de 4.
fn draw_marching_ants(
    painter: &egui::Painter,
    mask: &Mask,
    origin: Pos2,
    zoom: f32,
    phase: i32,
    pixel: bool,
) {
    let at = |x: i32, y: i32| origin + Vec2::new(x as f32, y as f32) * zoom;
    if pixel {
        let clip = painter.clip_rect();
        let p = |x: i32, y: i32| {
            let s = at(x, y);
            (s.x.round() as i32, s.y.round() as i32)
        };
        for (x, y) in mask.points() {
            let (l, t) = p(x, y);
            let (r, b) = p(x + 1, y + 1);
            let px = Rect::from_min_max(
                Pos2::new(l as f32, t as f32),
                Pos2::new(r as f32 + 1.0, b as f32 + 1.0),
            );
            if !clip.intersects(px) {
                continue;
            }
            // Cada lado del píxel sin vecino seleccionado es borde.
            let mut dashes = Vec::new();
            if !mask.contains(x, y - 1) {
                dashes.extend(pixel_selection::hline(l, r, t, phase));
            }
            if !mask.contains(x, y + 1) {
                dashes.extend(pixel_selection::hline(l, r, b, phase));
            }
            if !mask.contains(x - 1, y) {
                dashes.extend(pixel_selection::vline(l, t, b, phase));
            }
            if !mask.contains(x + 1, y) {
                dashes.extend(pixel_selection::vline(r, t, b, phase));
            }
            pixel_selection::paint(painter, dashes);
        }
        return;
    }
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

/// Filas `(y, x0, x1)` de un pincel, de punta a punta: las formas del pincel no tienen huecos
/// dentro de una fila.
fn brush_rows(points: &[(i32, i32)]) -> Vec<(i32, i32, i32)> {
    let mut rows: BTreeMap<i32, (i32, i32)> = BTreeMap::new();
    for &(x, y) in points {
        let r = rows.entry(y).or_insert((x, x));
        r.0 = r.0.min(x);
        r.1 = r.1.max(x);
    }
    rows.into_iter().map(|(y, (x0, x1))| (y, x0, x1)).collect()
}

/// Borde de un pincel: cada lado de píxel sin vecino en `points`, como segmento entre dos
/// esquinas de píxel. Van en sentido horario en pantalla: el pincel queda a la derecha de cada
/// segmento, o sea hacia `(-dy, dx)`.
fn brush_edges(points: &[(i32, i32)]) -> Vec<((i32, i32), (i32, i32))> {
    let set: HashSet<(i32, i32)> = points.iter().copied().collect();
    let mut out = Vec::new();
    for &(x, y) in points {
        if !set.contains(&(x, y - 1)) {
            out.push(((x, y), (x + 1, y)));
        }
        if !set.contains(&(x, y + 1)) {
            out.push(((x + 1, y + 1), (x, y + 1)));
        }
        if !set.contains(&(x - 1, y)) {
            out.push(((x, y + 1), (x, y)));
        }
        if !set.contains(&(x + 1, y)) {
            out.push(((x + 1, y), (x + 1, y + 1)));
        }
    }
    out
}

/// Segmentos en pantalla del borde de un pincel (`at` lleva una esquina de píxel a pantalla), a
/// medio trazo hacia adentro como `StrokeKind::Inside` del cuadrado. En las esquinas interiores
/// el segmento que llega se alarga un ancho de trazo: si no, entre los dos lados queda un hueco
/// de `width`×`width`. Las exteriores ya se tocan y no se alargan (sobresaldrían del pincel).
fn outline_segments(
    points: &[(i32, i32)],
    at: impl Fn((i32, i32)) -> Pos2,
    width: f32,
) -> Vec<[Pos2; 2]> {
    let edges = brush_edges(points);
    let next: HashMap<(i32, i32), (i32, i32)> = edges.iter().copied().collect();
    edges
        .iter()
        .map(|&(a, b)| {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let dir = Vec2::new(dx as f32, dy as f32);
            let inward = Vec2::new(-dir.y, dir.x) * (width / 2.0);
            // Sentido horario: el lado siguiente dobla a la izquierda en una esquina interior.
            let inner = next.get(&b).is_some_and(|&c| {
                let (ex, ey) = (c.0 - b.0, c.1 - b.1);
                dx * ey - dy * ex < 0
            });
            let reach = if inner { dir * width } else { Vec2::ZERO };
            [at(a) + inward, at(b) + inward + reach]
        })
        .collect()
}

/// Coordenadas de la textura del damero (un par claro/oscuro por repetición) para un artboard de
/// `width`×`height` píxeles del sprite: no dependen del zoom.
fn checker_uv(width: u32, height: u32) -> Rect {
    let size = Vec2::new(width as f32, height as f32);
    Rect::from_min_max(Pos2::ZERO, (size / (CHECKER_CELL * 2.0)).to_pos2())
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Rgba, Sprite};
    use eframe::emath::GuiRounding as _;
    use egui::{Event, Key, PointerButton, RawInput};

    use super::*;

    #[test]
    fn checker_cells_are_16_sprite_pixels_at_any_zoom() {
        // Un artboard de 64×64 lleva 4×4 cuadros (la textura repite 2×2 veces su par
        // claro/oscuro) al 100 % y al 800 %: al hacer zoom los cuadros crecen con el dibujo.
        assert_eq!(checker_uv(64, 64).max, Pos2::new(2.0, 2.0));
        assert_eq!(checker_uv(40, 8).max, Pos2::new(1.25, 0.25));
        assert_eq!(checker_uv(40, 8).min, Pos2::ZERO);
    }

    /// Lienzo de 800×600 manejado con eventos crudos, con el reloj avanzando 50 ms por frame.
    struct Harness {
        ctx: egui::Context,
        canvas: CanvasView,
        editor: Editor,
        view: ViewSettings,
        time: f64,
        rect: Rect,
        /// Cursor pedido en el último frame.
        cursor: egui::CursorIcon,
        /// Lo dibujado en el último frame, aplanado.
        shapes: Vec<egui::Shape>,
        /// La textura de la nota sin el puntero encima, `note.plain` (con `with_note`).
        note_texture: Option<egui::TextureId>,
    }

    impl Harness {
        fn new() -> Self {
            let mut h = Self {
                ctx: egui::Context::default(),
                canvas: CanvasView::default(),
                editor: Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba)),
                view: ViewSettings::default(),
                time: 0.0,
                rect: Rect::NOTHING,
                cursor: egui::CursorIcon::Default,
                shapes: Vec::new(),
                note_texture: None,
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let mut rect = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::no_frame().show(ui, |ui| {
                    rect = ui.max_rect();
                    self.canvas.show(ui, &mut self.editor, None, &self.view);
                });
            });
            output.textures_delta.clear();
            self.rect = rect;
            self.cursor = output.platform_output.cursor_icon;
            self.shapes.clear();
            for clipped in output.shapes {
                flatten(clipped.shape, &mut self.shapes);
            }
        }

        /// Rects de las mallas con la textura `id` en el último frame.
        fn meshes(&self, id: egui::TextureId) -> Vec<Rect> {
            self.shapes
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Mesh(m) if m.texture_id == id => Some(m.calc_bounds()),
                    _ => None,
                })
                .collect()
        }

        /// Textos del último frame con su rect.
        fn texts(&self) -> Vec<(String, Rect)> {
            self.shapes
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Text(t) => Some((
                        t.galley.text().to_owned(),
                        Rect::from_min_size(t.pos, t.galley.size()),
                    )),
                    _ => None,
                })
                .collect()
        }

        /// Área de píxeles del artboard `i` en pantalla.
        fn pixels_of(&self, i: usize) -> Rect {
            let ((x0, y0), (x1, y1)) = self.editor.document.artboards[i].rect();
            Rect::from_min_max(
                self.canvas.to_screen(self.rect, (x0, y0)),
                self.canvas.to_screen(self.rect, (x1, y1)),
            )
        }

        fn click(&mut self, pos: Pos2, button: PointerButton) {
            let ev = |pressed| Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![ev(true)]);
            self.frame(vec![ev(false)]);
        }

        fn key(&mut self, key: Key) {
            let ev = |pressed| Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: Default::default(),
            };
            self.frame(vec![ev(true), ev(false)]);
        }

        /// Un evento de rueda con el puntero sobre el píxel (8, 8).
        /// `modifiers` quedan mantenidos (`ModifiersChanged`) hasta el próximo cambio.
        fn wheel(&mut self, unit: egui::MouseWheelUnit, dy: f32, modifiers: egui::Modifiers) {
            let pos = self.screen((8, 8));
            self.frame(vec![
                Event::ModifiersChanged(modifiers),
                Event::PointerMoved(pos),
            ]);
            self.frame(vec![Event::MouseWheel {
                unit,
                delta: Vec2::new(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers,
            }]);
        }

        /// Centro en pantalla del píxel local `p` del artboard activo.
        fn screen(&self, p: (i32, i32)) -> Pos2 {
            let (x, y) = self.editor.artboard().position;
            self.canvas.to_screen(self.rect, (x + p.0, y + p.1))
                + Vec2::splat(self.canvas.zoom / 2.0)
        }

        /// Esquina en pantalla del píxel local `p` del artboard activo (no su centro).
        fn corner(&self, p: (i32, i32)) -> Pos2 {
            let (x, y) = self.editor.artboard().position;
            self.canvas.to_screen(self.rect, (x + p.0, y + p.1))
        }

        /// Arrastre con el botón principal de `from` a `to` (en pantalla), en varios frames.
        fn drag(&mut self, from: Pos2, to: Pos2) {
            self.drag_with(from, to, PointerButton::Primary);
        }

        /// Arrastre con `button` de `from` a `to` (en pantalla), en varios frames.
        fn drag_with(&mut self, from: Pos2, to: Pos2, button: PointerButton) {
            let button = |pos, pressed| Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Default::default(),
            };
            self.frame(vec![Event::PointerMoved(from)]);
            self.frame(vec![button(from, true)]);
            self.frame(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
            self.frame(vec![Event::PointerMoved(to)]);
            self.frame(vec![button(to, false)]);
        }

        /// Un punto rojo en `at` y la selección `a`–`b` con el marco rectangular.
        fn selected_dot(&mut self, at: (i32, i32), a: (i32, i32), b: (i32, i32)) -> PixelValue {
            let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
            self.editor.execute(Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(at.0, at.1, red)],
            });
            self.editor.set_tool(ToolKind::RectSelect);
            self.editor
                .set_selection(Some(Mask::from_rect(16, 16, a, b)));
            self.frame(vec![]);
            red
        }
    }

    #[test]
    fn the_other_button_cancels_a_stroke_without_starting_another() {
        let mut h = Harness::new();
        let t = h.editor.sprite().color_mode.transparent();
        let event = |pos, button, pressed| Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        let (a, b) = (h.screen((2, 2)), h.screen((6, 2)));
        h.frame(vec![Event::PointerMoved(a)]);
        h.frame(vec![event(a, PointerButton::Primary, true)]);
        h.frame(vec![Event::PointerMoved(b)]);
        assert!(h.editor.is_painting());
        // El derecho, con el izquierdo todavía abajo: se cancela.
        h.frame(vec![event(b, PointerButton::Secondary, true)]);
        assert!(!h.editor.is_painting(), "cancelled, and no new stroke");
        h.frame(vec![event(b, PointerButton::Secondary, false)]);
        h.frame(vec![event(b, PointerButton::Primary, false)]);
        h.frame(vec![]);
        for x in 2..=6 {
            assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, x, 2), t, "({x}, 2)");
        }
    }

    #[test]
    fn dragging_a_corner_handle_scales_and_enter_applies() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        let t = h.editor.sprite().color_mode.transparent();
        // Los handles quedan por fuera de la caja: se toma la esquina un poco hacia afuera.
        let out = Vec2::splat(3.0);
        h.drag(h.corner((4, 4)) + out, h.corner((6, 6)) + out);
        assert_eq!(
            h.editor.transform_box(),
            Some(TransformBox::new(2, 2, 4, 4))
        );
        assert_eq!(
            h.editor.sprite().pixel(LayerId(1), 0, 3, 3),
            t,
            "still floating"
        );
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert!(!h.editor.is_floating());
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            assert_eq!(
                h.editor.sprite().pixel(LayerId(1), 0, x, y),
                red,
                "({x}, {y})"
            );
        }
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 4, 4), t);
    }

    #[test]
    fn dragging_just_outside_a_corner_rotates() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (5, 5));
        let t = h.editor.sprite().color_mode.transparent();
        let center = h.corner((4, 4));
        // 16 px por fuera de la esquina NE: más allá del handle (10 px), dentro de la franja.
        let start = h.corner((6, 2)) + Vec2::new(16.0, -16.0);
        let v = start - center;
        h.drag(start, center + Vec2::new(-v.y, v.x)); // un cuarto de vuelta horario
        let angle = h.editor.transform_box().unwrap().angle;
        assert!(
            (angle - std::f32::consts::FRAC_PI_2).abs() < 1e-3,
            "{angle}"
        );
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 2, 2), t);
    }

    #[test]
    fn dragging_inside_the_selection_still_moves_it() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        h.drag(h.screen((2, 2)), h.screen((5, 2)));
        assert_eq!(
            h.editor.transform_box(),
            Some(TransformBox::new(5, 2, 2, 2))
        );
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
    }

    /// Zoom 4 (los handles de 10 px cubren más de un píxel del sprite).
    fn zoomed_out() -> Harness {
        let mut h = Harness::new();
        h.canvas.zoom = 4.0;
        h.canvas.pan = -Vec2::splat(8.0) * 4.0;
        h.frame(vec![]);
        h
    }

    #[test]
    fn a_small_selection_moves_from_inside_at_low_zoom() {
        let mut h = zoomed_out();
        let red = h.selected_dot((2, 2), (2, 2), (3, 3));
        h.drag(h.screen((2, 2)), h.screen((5, 2)));
        assert_eq!(
            h.editor.transform_box(),
            Some(TransformBox::new(5, 2, 2, 2))
        );
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 2), red);
    }

    #[test]
    fn pressing_just_inside_an_edge_moves_instead_of_scaling() {
        let mut h = zoomed_out();
        h.selected_dot((2, 2), (2, 2), (5, 5));
        // 2 px de pantalla adentro del lado E, a media altura.
        let from = h.corner((6, 4)) - Vec2::new(2.0, 0.0);
        h.drag(from, from + Vec2::new(12.0, 0.0));
        assert_eq!(
            h.editor.transform_box(),
            Some(TransformBox::new(5, 2, 4, 4))
        );
    }

    #[test]
    fn no_handles_during_another_selection_gesture() {
        let mut h = Harness::new();
        h.selected_dot((2, 2), (2, 2), (5, 5));
        let press = |pos| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        };
        // Un marco nuevo desde fuera de la selección pasa por encima del handle SE.
        let from = h.screen((12, 12));
        let se = h.corner((6, 6)) + Vec2::new(3.0, 3.0);
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![press(from)]);
        h.frame(vec![Event::PointerMoved(from.lerp(se, 0.5))]);
        h.frame(vec![Event::PointerMoved(se)]);
        h.frame(vec![]);
        assert!(h.editor.is_painting() && !h.editor.is_transforming());
        assert_eq!(h.cursor, egui::CursorIcon::Crosshair);
    }

    #[test]
    fn with_pixel_ui_the_tools_draw_the_design_cursors() {
        let mut h = Harness::new();
        h.canvas.pixel_ui = true;
        // Sin los cursores del skin, los del sistema.
        h.frame(vec![Event::PointerMoved(h.screen((3, 3)))]);
        h.frame(vec![]);
        assert_eq!(h.cursor, egui::CursorIcon::Crosshair, "no skin cursors");
        let data = crate::skin::decode(crate::skin::EMBEDDED).expect("the embedded skin");
        h.canvas.cursors = Some(pixel_cursors::CursorSet::from_pieces(|name| {
            data.pieces.get(name).map(|p| (p.size, p.rgba.clone()))
        }));
        // Los píxeles de los cursores: de un punto, con una de las dos tintas.
        let ink = |h: &Harness| -> Vec<Rect> {
            h.shapes
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Rect(r)
                        if r.rect.size() == Vec2::splat(1.0)
                            && [pixel_cursors::INK, Color32::WHITE].contains(&r.fill) =>
                    {
                        Some(r.rect)
                    }
                    _ => None,
                })
                .collect()
        };
        let p = h.screen((3, 3)) + Vec2::splat(0.5);
        for tool in [ToolKind::Pencil, ToolKind::RectSelect, ToolKind::Eyedropper] {
            h.editor.set_tool(tool);
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            assert_eq!(h.cursor, egui::CursorIcon::None, "{tool:?}");
            assert!(!ink(&h).is_empty(), "{tool:?}");
        }
        // Las esquinas de selección, por fuera del píxel (3, 3).
        h.editor.set_tool(ToolKind::RectSelect);
        h.frame(vec![]);
        let cell = Rect::from_min_max(h.corner((3, 3)), h.corner((4, 4)));
        let bounds = ink(&h).iter().fold(Rect::NOTHING, |b, r| b.union(*r));
        assert_eq!(bounds, cell.expand(3.0));
        // Cada píxel se invierte según lo de debajo: sobre un píxel negro, blanco; sobre uno
        // blanco, la tinta.
        let fill_at = |h: &Harness, q: Pos2| {
            h.shapes.iter().rev().find_map(|s| match s {
                egui::Shape::Rect(r) if r.rect.size() == Vec2::splat(1.0) && r.rect.contains(q) => {
                    Some(r.fill)
                }
                _ => None,
            })
        };
        for (color, expected) in [
            (Rgba::rgb(0, 0, 0), Color32::WHITE),
            (Rgba::rgb(255, 255, 255), pixel_cursors::INK),
        ] {
            h.editor.execute(Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(2, 2, PixelValue::Rgba(color))],
            });
            h.frame(vec![]);
            // El vértice de arriba a la izquierda, sobre el píxel (2, 2) del sprite.
            let q = h.corner((3, 3)) - Vec2::splat(0.5);
            assert_eq!(fill_at(&h, q), Some(expected), "{color:?}");
        }
        // Sin Pixel UI, el del sistema.
        h.canvas.pixel_ui = false;
        h.editor.set_tool(ToolKind::Pencil);
        h.frame(vec![]);
        assert_eq!(h.cursor, egui::CursorIcon::Crosshair);
    }

    #[test]
    fn right_click_inside_the_selection_opens_its_menu_to_flip() {
        let mut h = Harness::new();
        let red = h.selected_dot((2, 3), (2, 2), (5, 5));
        h.click(h.screen((3, 3)), PointerButton::Secondary);
        h.frame(vec![]);
        let item = h
            .texts()
            .into_iter()
            .find(|(t, _)| t == "Flip Horizontal")
            .map(|(_, r)| r)
            .unwrap_or_else(|| panic!("no menu in {:?}", h.texts()));
        assert!(h.texts().iter().any(|(t, _)| t == "Flip Vertical"));
        assert!(
            !h.editor.is_floating(),
            "the right click does not reach the tool"
        );
        h.click(item.center(), PointerButton::Primary);
        h.frame(vec![]);
        assert!(h.editor.is_floating());
        crate::editor_key(&mut h.editor, Key::Enter, false);
        assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 5, 3), red);
    }

    #[test]
    fn the_symmetry_axis_moves_with_the_artboard_while_dragging_it() {
        let mut h = Harness::new();
        let blue = Color32::from_rgb(0, 73, 255);
        h.canvas.symmetry_color = Some(blue);
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                ..Default::default()
            },
            center2: None,
        });
        h.editor.set_tool(ToolKind::Artboard);
        h.frame(vec![]);
        let axis_x = |h: &Harness| {
            h.shapes
                .iter()
                .find_map(|s| match s {
                    egui::Shape::Rect(r)
                        if r.fill == blue && r.rect.width() == 1.0 && r.rect.height() > 1.0 =>
                    {
                        Some(r.rect.left())
                    }
                    _ => None,
                })
                .expect("the vertical axis")
        };
        let before = axis_x(&h);
        // Se agarra el artboard y se mueve 3 píxeles, sin soltar.
        let from = h.screen((4, 4));
        let to = from + Vec2::new(3.0 * h.canvas.zoom, 0.0);
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![press(from, true)]);
        h.frame(vec![Event::PointerMoved(from.lerp(to, 0.5))]);
        h.frame(vec![Event::PointerMoved(to)]);
        assert_eq!(axis_x(&h), before + 3.0 * h.canvas.zoom);
    }

    #[test]
    fn with_pixel_ui_the_symmetry_axes_are_one_point_lines_with_handles() {
        let mut h = Harness::new();
        let blue = Color32::from_rgb(0, 73, 255);
        h.canvas.symmetry_color = Some(blue);
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                diagonal_down: true,
                ..Default::default()
            },
            center2: None,
        });
        h.frame(vec![]);
        let fills = |color: Color32| -> Vec<Rect> {
            h.shapes
                .iter()
                .filter_map(|s| match s {
                    egui::Shape::Rect(r) if r.fill == color => Some(r.rect),
                    _ => None,
                })
                .collect()
        };
        let pixels = h.pixels_of(0);
        let lines = fills(blue);
        // El eje vertical: 1 punto de ancho, de handle a handle (5 adentro de cada borde).
        let vertical = lines
            .iter()
            .find(|r| r.width() == 1.0 && r.height() > 1.0)
            .expect("the vertical axis");
        assert_eq!(vertical.top(), pixels.top().round() + 5.0);
        assert_eq!(vertical.bottom(), pixels.bottom().round() - 5.0);
        // La diagonal: una escalera de puntos.
        assert!(
            lines
                .iter()
                .filter(|r| r.size() == Vec2::splat(1.0))
                .count()
                > 10
        );
        // Los handles de 5×5 (tinta), arriba y abajo del eje.
        let handles: Vec<Rect> = fills(crate::pixel_cursors::INK)
            .into_iter()
            .filter(|r| r.size() == Vec2::splat(5.0))
            .collect();
        assert_eq!(handles.len(), 2, "{handles:?}");
        assert!(
            handles
                .iter()
                .all(|r| r.center().x.floor() == vertical.left())
        );
    }

    #[test]
    fn with_zoom_the_symmetry_handles_stick_to_the_edges_of_the_view() {
        let mut h = Harness::new();
        h.canvas.symmetry_color = Some(Color32::from_rgb(0, 73, 255));
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                ..Default::default()
            },
            center2: None,
        });
        h.frame(vec![]);
        // Tan cerca que el artboard pasa los bordes de la vista.
        h.canvas.zoom = 64.0;
        h.canvas.request_zoom(ZoomRequest::CenterActive);
        h.frame(vec![]);
        h.frame(vec![]);
        let pixels = h.pixels_of(0);
        assert!(pixels.top() < h.rect.top() && pixels.bottom() > h.rect.bottom());
        let view = h.canvas.visible(h.rect);
        let handles: Vec<Rect> = h
            .shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Rect(r)
                    if r.fill == crate::pixel_cursors::INK && r.rect.size() == Vec2::splat(5.0) =>
                {
                    Some(r.rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(handles.len(), 2, "{handles:?}");
        let top = handles.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
        let bottom = handles.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
        assert_eq!(top, view.top().round(), "under the ruler");
        assert_eq!(bottom, view.bottom().round(), "at the bottom of the view");
    }

    #[test]
    fn the_symmetry_control_moves_with_any_tool() {
        let mut h = Harness::new();
        h.canvas.symmetry_color = Some(Color32::from_rgb(0, 73, 255));
        h.editor.set_tool(ToolKind::Pencil);
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                ..Default::default()
            },
            center2: None,
        });
        h.frame(vec![]);
        let before = h.editor.symmetry().center2;
        let pixels = h.pixels_of(0);
        // El handle de arriba del eje vertical (en el centro del artboard).
        let from = Pos2::new(pixels.center().x, pixels.top() + 2.0);
        let to = from + Vec2::new(h.canvas.zoom * 2.0, 0.0);
        let press = |pos, pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![press(from, true)]);
        h.frame(vec![Event::PointerMoved(to)]);
        h.frame(vec![press(to, false)]);
        h.frame(vec![]);
        assert_ne!(h.editor.symmetry().center2, before, "the axis moved");
        let sprite = h.editor.sprite();
        let t = sprite.color_mode.transparent();
        let layer = h.editor.layer;
        let drawn = (0..sprite.width as i32)
            .flat_map(|x| (0..sprite.height as i32).map(move |y| (x, y)))
            .any(|(x, y)| sprite.pixel(layer, 0, x, y) != t);
        assert!(!drawn, "the pencil did not draw");
    }

    #[test]
    fn hovering_the_handles_shows_resize_and_rotate_cursors() {
        let mut h = Harness::new();
        h.selected_dot((2, 2), (2, 2), (5, 5));
        let hover = |h: &mut Harness, p: Pos2| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            h.cursor
        };
        // Los handles quedan por fuera de la caja.
        let (se, ne, n, w) = (
            h.corner((6, 6)) + Vec2::new(3.0, 3.0),
            h.corner((6, 2)) + Vec2::new(3.0, -3.0),
            h.corner((4, 2)) + Vec2::new(0.0, -3.0),
            h.corner((2, 4)) + Vec2::new(-3.0, 0.0),
        );
        let inside = h.screen((3, 3));
        let outside = h.corner((6, 2)) + Vec2::new(16.0, -16.0);
        assert_eq!(hover(&mut h, se), egui::CursorIcon::ResizeNwSe);
        assert_eq!(hover(&mut h, ne), egui::CursorIcon::ResizeNeSw);
        assert_eq!(hover(&mut h, n), egui::CursorIcon::ResizeVertical);
        assert_eq!(hover(&mut h, w), egui::CursorIcon::ResizeHorizontal);
        assert_eq!(hover(&mut h, inside), egui::CursorIcon::Move);
        assert_eq!(
            hover(&mut h, outside),
            egui::CursorIcon::None,
            "rotate: drawn by hand"
        );
        // Con una herramienta de dibujo no hay handles.
        h.editor.set_tool(ToolKind::Pencil);
        assert_eq!(hover(&mut h, se), egui::CursorIcon::Crosshair);
    }

    #[test]
    fn dragging_the_selection_keeps_the_move_cursor_off_the_artboard() {
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::RectSelect);
        h.editor
            .set_selection(Some(cs_core::Mask::from_rect(16, 16, (2, 2), (6, 6))));
        let ev = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let from = h.screen((4, 4));
        // Fuera del artboard pero dentro del lienzo.
        let off = Pos2::new(h.rect.right() - 2.0, from.y);
        let world = h.canvas.to_world(h.rect, off);
        assert!(h.editor.document.artboard_at(world).is_none(), "{world:?}");
        h.frame(vec![Event::PointerMoved(from)]);
        assert_eq!(h.cursor, egui::CursorIcon::Move);
        h.frame(vec![ev(from, true)]);
        h.frame(vec![Event::PointerMoved(from + Vec2::new(5.0, 0.0))]);
        h.frame(vec![Event::PointerMoved(off)]);
        h.frame(vec![]);
        assert!(h.editor.is_transforming());
        assert_eq!(h.cursor, egui::CursorIcon::Move, "still moving");
        h.frame(vec![ev(off, false)]);
        h.frame(vec![]);
        assert!(!h.editor.is_transforming());
    }

    #[test]
    fn double_clicking_an_artboard_label_renames_it() {
        let mut h = Harness::new();
        let label = h.canvas.to_screen(h.rect, h.editor.artboard().position)
            - Vec2::new(-10.0, LABEL_GAP + LABEL_SIZE / 2.0);
        h.click(label, PointerButton::Primary);
        h.click(label, PointerButton::Primary);
        h.frame(vec![]);
        assert!(h.canvas.renaming.is_some(), "double click opens the field");
        let mut erase = vec![];
        for _ in 0..20 {
            for pressed in [true, false] {
                erase.push(Event::Key {
                    key: Key::Backspace,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: Default::default(),
                });
            }
        }
        h.frame(erase);
        h.frame(vec![Event::Text("Hero".into())]);
        h.key(Key::Enter);
        h.frame(vec![]);
        assert_eq!(h.editor.artboard().name, "Hero");
        assert!(h.canvas.renaming.is_none());
        assert_eq!(
            h.editor.artboard().position,
            (0, 0),
            "the clicks did not move it"
        );
    }

    #[test]
    fn right_clicking_a_slice_selects_it_and_opens_its_menu() {
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Slice);
        let drag = |e: &mut Editor, a: (f32, f32), b: (f32, f32)| {
            e.pointer_press(a, false, false, 0.1);
            e.pointer_drag(b);
            e.pointer_release();
        };
        drag(&mut h.editor, (2.5, 2.5), (6.5, 6.5));
        drag(&mut h.editor, (10.5, 10.5), (12.5, 12.5));
        assert_eq!(h.editor.slice, Some(1));
        let before = h.editor.sprite().slices.clone();
        h.frame(vec![]);
        h.click(h.screen((4, 4)), PointerButton::Secondary);
        h.frame(vec![]);
        assert_eq!(h.editor.slice, Some(0));
        assert!(egui::Popup::is_any_open(&h.ctx), "context menu");
        assert_eq!(h.editor.sprite().slices, before, "nothing created or moved");
        // Fuera de un slice no hay menú.
        h.key(Key::Escape);
        h.click(h.screen((14, 1)), PointerButton::Secondary);
        h.frame(vec![]);
        assert!(!egui::Popup::is_any_open(&h.ctx));
    }

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
    #[test]
    fn a_right_drag_does_not_lift_or_move_the_selection() {
        for tool in [ToolKind::Move, ToolKind::RectSelect] {
            let mut h = Harness::new();
            let red = h.selected_dot((2, 2), (2, 2), (3, 3));
            let t = h.editor.sprite().color_mode.transparent();
            h.editor.set_tool(tool);
            h.frame(vec![]);
            let version = h.editor.doc_version;
            h.drag_with(h.screen((2, 2)), h.screen((6, 6)), PointerButton::Secondary);
            assert!(!h.editor.is_floating(), "{tool:?}");
            assert_eq!(h.editor.doc_version, version, "{tool:?}");
            assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 2, 2), red);
            assert_eq!(h.editor.sprite().pixel(LayerId(1), 0, 6, 6), t);
        }
    }

    #[test]
    fn the_hand_tool_pans_the_view_without_touching_the_document() {
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Hand);
        let from = h.screen((8, 8));
        h.frame(vec![Event::PointerMoved(from)]);
        assert_eq!(h.cursor, egui::CursorIcon::Grab, "at rest");
        let (pan, version) = (h.canvas.pan, h.editor.doc_version);
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let to = from + Vec2::new(40.0, 0.0);
        h.frame(vec![button(from, true)]);
        h.frame(vec![Event::PointerMoved(from + Vec2::new(20.0, 0.0))]);
        assert_eq!(h.cursor, egui::CursorIcon::Grabbing, "while dragging");
        h.frame(vec![Event::PointerMoved(to)]);
        h.frame(vec![button(to, false)]);
        let moved = h.canvas.pan - pan;
        assert!(moved.x > 15.0 && moved.y == 0.0, "{moved:?}");
        assert_eq!(h.editor.doc_version, version, "nothing painted");
        assert!(h.editor.sprite().cels.is_empty());
        h.frame(vec![]);
        assert_eq!(h.cursor, egui::CursorIcon::Grab);
        assert_eq!(h.editor.tool, ToolKind::Hand);
    }

    #[test]
    fn any_tool_moves_a_guide_unless_they_are_locked() {
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Pencil);
        // Una guía horizontal en la fila 8, desde la regla.
        let ruler = Pos2::new(h.rect.center().x, h.rect.top() + RULER_PX / 2.0);
        h.drag(ruler, h.screen((8, 8)));
        assert_eq!(h.editor.guides()[0].position, 8);
        let before = h.editor.sprite().clone();
        // Con el lápiz se agarra y se mueve (no pinta).
        let on_guide = h.canvas.to_screen(h.rect, (4, 8));
        let target = h.canvas.to_screen(h.rect, (4, 12));
        h.drag(on_guide, target);
        assert_eq!(h.editor.guides()[0].position, 12, "moved with the pencil");
        assert_eq!(*h.editor.sprite(), before, "the pencil did not draw");
        // Bloqueadas, no se mueven.
        h.view.lock_guides = true;
        let on_guide = h.canvas.to_screen(h.rect, (4, 12));
        h.drag(on_guide, h.screen((4, 14)));
        assert_eq!(h.editor.guides()[0].position, 12, "locked");
    }

    #[test]
    fn with_the_hand_tool_a_ruler_drag_still_creates_a_guide() {
        // El mismo arrastre con Pencil: la vista solo se mueve por el auto-scroll del borde.
        let ruler_drag = |tool| {
            let mut h = Harness::new();
            h.editor.set_tool(tool);
            let ruler = Pos2::new(h.rect.center().x, h.rect.top() + RULER_PX / 2.0);
            h.frame(vec![Event::PointerMoved(ruler)]);
            assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical, "over the ruler");
            h.drag(ruler, h.screen((8, 8)));
            h.frame(vec![]);
            assert_eq!(h.editor.guides().len(), 1, "a guide was created");
            assert_eq!(h.editor.guides()[0].axis, Axis::Horizontal);
            (h.editor.guides()[0].position, h.canvas.pan)
        };
        assert_eq!(
            ruler_drag(ToolKind::Hand),
            ruler_drag(ToolKind::Pencil),
            "the hand did not pan"
        );
    }

    #[test]
    fn with_pixel_rulers_the_ruler_is_12_thick_for_guides_and_the_pointer() {
        let mut h = Harness::new();
        let skin = Skin::embedded(&h.ctx);
        h.canvas.pixel_rulers = Some(crate::pixel_rulers::PixelRulers::from_skin(&skin));
        h.frame(vec![]);
        // La fila 11 (la línea oscura) todavía es regla; la 12 ya es lienzo.
        let on = Pos2::new(h.rect.center().x, h.rect.top() + 11.5);
        h.frame(vec![Event::PointerMoved(on)]);
        assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical, "over the ruler");
        let off = Pos2::new(h.rect.center().x, h.rect.top() + 12.5);
        h.frame(vec![Event::PointerMoved(off)]);
        assert_ne!(h.cursor, egui::CursorIcon::ResizeVertical, "on the canvas");
        h.drag(on, h.screen((8, 8)));
        h.frame(vec![]);
        assert_eq!(h.editor.guides().len(), 1, "a guide was created");
        assert_eq!(h.editor.guides()[0].axis, Axis::Horizontal);
        // Las coordenadas no cambian: el mismo píxel bajo el mismo punto.
        let p = h.screen((3, 5));
        h.frame(vec![Event::PointerMoved(p)]);
        assert_eq!(h.canvas.hover, Some((3, 5)));
    }

    #[test]
    fn with_the_hand_tool_an_artboard_label_is_not_renamed_or_moved() {
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Hand);
        let label = h.canvas.to_screen(h.rect, h.editor.artboard().position)
            - Vec2::new(-10.0, LABEL_GAP + LABEL_SIZE / 2.0);
        h.click(label, PointerButton::Primary);
        h.click(label, PointerButton::Primary);
        h.frame(vec![]);
        assert!(h.canvas.renaming.is_none(), "no rename field");
        assert_eq!(h.cursor, egui::CursorIcon::Grab);
        assert_eq!(h.editor.artboard().position, (0, 0));
    }

    #[test]
    fn ctrl_wheel_changes_the_brush_size_of_brush_tools() {
        use egui::{Modifiers, MouseWheelUnit};
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Pencil);
        let zoom = h.canvas.zoom;
        h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 2, "one step up");
        assert_eq!(h.canvas.zoom, zoom, "no zoom");
        h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
        h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 1, "not below 1");
        h.editor.options.brush_size = 32;
        h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 32, "not above 32");
        // Touchpad: 50 px por paso.
        h.editor.options.brush_size = 1;
        h.wheel(MouseWheelUnit::Point, 30.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 1, "30 px are not a step yet");
        h.wheel(MouseWheelUnit::Point, 30.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 2);
        assert_eq!(h.canvas.zoom, zoom, "still no zoom");
    }

    #[test]
    fn ctrl_wheel_does_nothing_during_a_stroke() {
        use egui::{Modifiers, MouseWheelUnit};
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Pencil);
        let zoom = h.canvas.zoom;
        let pos = h.screen((8, 8));
        h.frame(vec![Event::PointerMoved(pos)]);
        h.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }]);
        assert!(h.editor.is_painting());
        h.wheel(MouseWheelUnit::Line, 1.0, Modifiers::COMMAND);
        assert_eq!(h.editor.options.brush_size, 1, "same size mid-stroke");
        assert_eq!(h.canvas.zoom, zoom, "and no zoom");
    }

    #[test]
    fn ctrl_wheel_zooms_without_a_brush_and_the_plain_wheel_always_zooms() {
        use egui::{Modifiers, MouseWheelUnit};
        for tool in [ToolKind::Hand, ToolKind::RectSelect] {
            let mut h = Harness::new();
            h.editor.set_tool(tool);
            let zoom = h.canvas.zoom;
            h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::COMMAND);
            assert!(h.canvas.zoom < zoom, "{tool:?} zooms out");
            assert_eq!(h.editor.options.brush_size, 1);
        }
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Pencil);
        let zoom = h.canvas.zoom;
        h.wheel(MouseWheelUnit::Line, -1.0, Modifiers::NONE);
        assert!(h.canvas.zoom < zoom, "without Ctrl it zooms");
        assert_eq!(h.editor.options.brush_size, 1);
    }

    #[test]
    fn pixel_wheel_steps_add_up_to_50_px() {
        use egui::MouseWheelUnit::{Line, Point};
        let mut rest = 0.0;
        assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 0);
        assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 0);
        assert_eq!(brush_wheel_steps(&mut rest, Point, 20.0), 1); // 60: quedan 10
        assert_eq!(brush_wheel_steps(&mut rest, Point, 120.0), 2); // 130: quedan 30
        assert_eq!(brush_wheel_steps(&mut rest, Point, -80.0), -1); // -50: quedan 0
        assert_eq!(
            brush_wheel_steps(&mut rest, Line, 3.0),
            1,
            "one per line event"
        );
        assert_eq!(brush_wheel_steps(&mut rest, Line, -0.5), -1);
        assert_eq!(brush_wheel_steps(&mut rest, Line, 0.0), 0);
    }

    #[test]
    fn with_pixel_ui_the_canvas_surround_is_the_skin_canvas_color() {
        let ctx = egui::Context::default();
        let skin = Skin::embedded(&ctx);
        assert_eq!(background_for(None), theme::CANVAS_BG);
        assert_eq!(
            background_for(Some(&skin)),
            Color32::from_rgb(189, 164, 153)
        );
        // El lienzo pinta su fondo con ese color.
        let mut h = Harness::new();
        h.canvas.background = background_for(Some(&skin));
        let mut shapes = Vec::new();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            ..Default::default()
        };
        let output = h.ctx.run_ui(input, |ui| {
            egui::CentralPanel::no_frame().show(ui, |ui| {
                h.canvas.show(ui, &mut h.editor, None, &h.view);
            });
        });
        for clipped in output.shapes {
            shapes.push(clipped.shape);
        }
        let fills_with_canvas = shapes.iter().any(
            |s| matches!(s, egui::Shape::Rect(r) if r.fill == Color32::from_rgb(189, 164, 153)),
        );
        assert!(fills_with_canvas);
    }

    #[test]
    fn a_click_with_both_straight_axes_paints_four_pixels() {
        let mut h = Harness::new();
        h.editor.set_symmetry(cs_core::Symmetry {
            axes: cs_core::SymmetryAxes {
                vertical: true,
                horizontal: true,
                ..cs_core::SymmetryAxes::NONE
            },
            center2: None,
        });
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        h.click(h.screen((1, 2)), PointerButton::Primary);
        let primary = h.editor.primary;
        let sprite = h.editor.sprite();
        let painted: Vec<(i32, i32)> = (0..16)
            .flat_map(|y| (0..16).map(move |x| (x, y)))
            .filter(|&(x, y)| sprite.pixel(LayerId(1), 0, x, y) == primary)
            .collect();
        assert_eq!(painted, vec![(1, 2), (14, 2), (1, 13), (14, 13)]);
    }

    #[test]
    fn the_round_brush_outline_follows_its_pixels() {
        use cs_tools::BrushShape;
        // Redondo de 3: una cruz de 5 píxeles, con 12 lados de borde.
        let cross = cs_raster::brush_points((0, 0), 3, BrushShape::Round);
        let edges = brush_edges(&cross);
        assert_eq!(edges.len(), 12);
        assert!(edges.contains(&((0, -1), (1, -1))), "top of the cross");
        assert!(!edges.contains(&((-1, -1), (0, -1))), "no square corner");
        assert_eq!(brush_rows(&cross), vec![(-1, 0, 0), (0, -1, 1), (1, 0, 0)]);
        let square = cs_raster::brush_points((0, 0), 2, BrushShape::Square);
        assert_eq!(brush_edges(&square).len(), 8);
        // Sentido horario: el pincel queda hacia (-dy, dx) de cada lado (ahí se dibuja el trazo).
        for (a, b) in edges {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let inside = (a.0.min(b.0) + (-dy).min(0), a.1.min(b.1) + dx.min(0));
            assert!(cross.contains(&inside), "{a:?}->{b:?}");
        }
    }

    #[test]
    fn round_outline_segments_close_the_inner_corners() {
        use cs_tools::BrushShape;
        // Zoom 10 y trazo de 1: medio píxel de pantalla hacia adentro.
        let cross = cs_raster::brush_points((0, 0), 3, BrushShape::Round);
        let at = |(x, y): (i32, i32)| Pos2::new(x as f32 * 10.0, y as f32 * 10.0);
        let segments = outline_segments(&cross, at, 1.0);
        assert_eq!(segments.len(), 12);
        // Arriba del brazo izquierdo, hacia la derecha: termina en una esquina interior y se
        // alarga un ancho de trazo para tapar el hueco con el lado del brazo de arriba.
        assert!(
            segments.contains(&[Pos2::new(-10.0, 0.5), Pos2::new(1.0, 0.5)]),
            "{segments:?}"
        );
        // Arriba del brazo de arriba: dos esquinas exteriores, sin alargar (no sobresale).
        assert!(segments.contains(&[Pos2::new(0.0, -9.5), Pos2::new(10.0, -9.5)]));
        // Cuadrado de 2: solo esquinas exteriores.
        let square = cs_raster::brush_points((0, 0), 2, BrushShape::Square);
        for [a, b] in outline_segments(&square, at, 1.0) {
            for p in [a, b] {
                assert!(
                    (0.0..=20.0).contains(&p.x) && (0.0..=20.0).contains(&p.y),
                    "{p:?} sticks out"
                );
            }
        }
    }

    #[test]
    fn leftover_wheel_pixels_do_not_carry_into_the_next_gesture() {
        use egui::{Modifiers, MouseWheelUnit::Point};
        let mut h = Harness::new();
        h.editor.set_tool(ToolKind::Pencil);
        let ctrl = Modifiers::COMMAND;
        // Soltar Ctrl.
        h.wheel(Point, 30.0, ctrl);
        h.frame(vec![Event::ModifiersChanged(Modifiers::NONE)]);
        h.wheel(Point, 30.0, ctrl);
        assert_eq!(h.editor.options.brush_size, 1, "Ctrl released in between");
        // Una rueda sin Ctrl.
        h.wheel(Point, 1.0, Modifiers::NONE);
        h.wheel(Point, 30.0, ctrl);
        assert_eq!(h.editor.options.brush_size, 1, "plain wheel in between");
        // Otra herramienta.
        h.editor.set_tool(ToolKind::Hand);
        h.wheel(Point, 30.0, ctrl);
        h.editor.set_tool(ToolKind::Pencil);
        h.wheel(Point, 30.0, ctrl);
        assert_eq!(h.editor.options.brush_size, 1, "tool changed in between");
        // Sin nada en el medio sí se suman.
        h.wheel(Point, 30.0, ctrl);
        assert_eq!(h.editor.options.brush_size, 2);
    }

    #[test]
    fn entering_the_mask_mid_blur_stroke_leaves_no_stroke_hanging() {
        use crate::mask_ops::NewMask;
        let mut h = Harness::new();
        h.editor.add_mask(LayerId(1), NewMask::RevealAll);
        h.editor.set_edit_mask(false);
        h.editor.set_tool(ToolKind::Blur);
        let (from, to) = (h.screen((4, 4)), h.screen((8, 4)));
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(from)]);
        h.frame(vec![button(from, true)]);
        assert!(h.editor.is_painting());
        h.editor.set_edit_mask(true);
        h.frame(vec![Event::PointerMoved(to)]);
        h.frame(vec![button(to, false)]);
        h.frame(vec![]);
        assert!(!h.editor.is_painting(), "no stroke left hanging");
    }

    /// Aplana los `Shape::Vec`.
    fn flatten(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.into_iter().for_each(|s| flatten(s, out)),
            other => out.push(other),
        }
    }

    /// Con Pixel UI: la ventana del skin alrededor de los artboards.
    fn with_window(h: &mut Harness) -> Skin {
        let skin = Skin::embedded(&h.ctx);
        h.canvas.artboard_window = Some(ArtboardWindow::from_skin(&skin));
        h.frame(vec![]);
        skin
    }

    fn piece_id(skin: &Skin, name: &str) -> egui::TextureId {
        skin.piece(name, crate::skin::WidgetState::Normal)
            .unwrap_or_else(|| panic!("{name}"))
            .texture
            .id()
    }

    fn union(rects: &[Rect]) -> Rect {
        rects.iter().fold(Rect::NOTHING, |a, r| a.union(*r))
    }

    #[test]
    fn with_pixel_ui_the_window_wraps_the_pixels_and_its_bar_does_not_zoom() {
        let mut h = Harness::new();
        let skin = with_window(&mut h);
        let mut bars = Vec::new();
        for zoom in [1.0, 8.0, 16.0] {
            h.canvas.zoom = zoom;
            h.frame(vec![]);
            let pixels = h.pixels_of(0);
            let frame = union(&h.meshes(piece_id(&skin, "artboard.frame")));
            // Borde + 1 de blanco a los lados y abajo; barra de 13 + 1 de blanco arriba.
            assert_eq!(
                frame,
                Rect::from_min_max(
                    pixels.min - Vec2::new(2.0, 14.0),
                    pixels.max + Vec2::new(2.0, 2.0)
                ),
                "zoom {zoom}"
            );
            bars.push(pixels.top() - frame.top());
            // Los íconos van en la barra, encima del área de píxeles.
            for name in ["artboard.menu", "window.close"] {
                let icon = union(&h.meshes(piece_id(&skin, name)));
                assert!(
                    icon.bottom() <= pixels.top() && icon.top() >= frame.top(),
                    "{name} {icon:?}"
                );
            }
            // El nombre va en la barra, en lugar de la etiqueta de arriba (a zoom 1 el
            // artboard de 16 no tiene lugar para el nombre y a 8 se corta con "...").
            if zoom < 16.0 {
                continue;
            }
            let label = label_text(h.editor.artboard());
            let (_, text) = h
                .texts()
                .into_iter()
                .find(|(t, _)| *t == label)
                .unwrap_or_else(|| panic!("the name in the bar: {:?}", h.texts()));
            assert!(text.center().y > frame.top() && text.center().y < pixels.top());
            // Centrado en la franja de 13 que empieza debajo de la fila rosa agregada arriba.
            let band_center = frame.top() + 1.0 + 13.0 / 2.0;
            assert_eq!(text.top(), (band_center - text.height() / 2.0).round());
        }
        assert_eq!(
            bars,
            vec![14.0; 3],
            "the bar (13) and the white (1) do not zoom"
        );
    }

    #[test]
    fn with_pixel_ui_the_pixels_and_the_checker_do_not_move() {
        let mut h = Harness::new();
        let id = h.editor.artboard().id;
        let drawn = |h: &Harness| {
            let tex = h.canvas.textures[&id].id();
            let checker = h.canvas.checker.as_ref().unwrap().id();
            (h.meshes(tex), h.meshes(checker))
        };
        let off = drawn(&h);
        assert_eq!(off.0, vec![h.pixels_of(0)]);
        with_window(&mut h);
        assert_eq!(drawn(&h), off);
    }

    #[test]
    fn dragging_the_title_bar_moves_the_artboard() {
        let mut h = Harness::new();
        h.canvas.zoom = 8.0;
        with_window(&mut h);
        let from = h.pixels_of(0).left_top() + Vec2::new(60.0, -6.0);
        h.frame(vec![Event::PointerMoved(from)]);
        assert_eq!(h.cursor, egui::CursorIcon::Grab);
        h.drag(from, from + Vec2::new(16.0, 8.0));
        assert_eq!(h.editor.artboard().position, (2, 1));
        assert!(h.editor.sprite().cels.is_empty(), "nothing painted");
    }

    #[test]
    fn double_clicking_the_title_bar_renames_the_artboard() {
        let mut h = Harness::new();
        with_window(&mut h);
        let bar = h.pixels_of(0).left_top() + Vec2::new(40.0, -6.0);
        h.click(bar, PointerButton::Primary);
        h.click(bar, PointerButton::Primary);
        h.frame(vec![]);
        assert!(h.canvas.renaming.is_some(), "double click opens the field");
        assert_eq!(h.editor.artboard().position, (0, 0));
    }

    #[test]
    fn the_menu_icon_opens_the_artboard_menu_and_the_circle_deletes_it() {
        let mut h = Harness::new();
        h.editor.add_artboard(8, 8);
        let first = h.editor.document.artboards[0].id;
        assert_ne!(h.editor.active, first);
        with_window(&mut h);
        let pixels = h.pixels_of(0);
        // "≡": lo selecciona y abre su menú, sin moverlo.
        let menu = pixels.left_top() + Vec2::new(-2.0 + 2.0 + 4.5, -14.0 + 7.5);
        h.click(menu, PointerButton::Primary);
        assert_eq!(h.editor.active, first);
        assert_eq!(
            h.editor.artboard().position,
            (0, 0),
            "a click does not move it"
        );
        assert!(h.canvas.renaming.is_none());
        h.frame(vec![]);
        let texts: Vec<String> = h
            .shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
        for item in ["Rename", "Duplicate", "Canvas Size", "Delete"] {
            assert!(texts.iter().any(|t| t == item), "{item} in {texts:?}");
        }
        h.click(menu, PointerButton::Primary);
        // "○", a 2 px del borde derecho de la ventana: borra el artboard (se deshace).
        let circle = Pos2::new(pixels.right() + 2.0 - 2.0 - 4.5, pixels.top() - 14.0 + 7.5);
        h.click(circle, PointerButton::Primary);
        assert!(h.editor.document.artboard(first).is_none(), "deleted");
        h.editor.undo();
        assert!(h.editor.document.artboard(first).is_some(), "undone");
    }

    /// Una nota en el mundo (0, 0) con Pixel UI y su papel en pantalla.
    fn with_note(h: &mut Harness) -> (u32, Rect) {
        let skin = with_window(h);
        // Sin el puntero encima, la nota desde la línea de la barra (la pieza de la barra,
        // recortada).
        h.note_texture = Some(piece_id(&skin, "note"));
        crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
        h.editor.set_tool(ToolKind::Pencil);
        let id = h.editor.add_note((0, 0)).unwrap();
        h.frame(vec![]);
        let at = h.canvas.to_screen(h.rect, (0, 0)).round();
        let size = crate::notes::NEW_SIZE;
        (
            id,
            Rect::from_min_size(at, Vec2::new(size.0 as f32, size.1 as f32)),
        )
    }

    #[test]
    fn a_click_on_a_note_writes_in_it() {
        let mut h = Harness::new();
        let (id, paper) = with_note(&mut h);
        let at = paper.center();
        let press = |pressed| Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        h.frame(vec![Event::PointerMoved(at)]);
        h.frame(vec![press(true)]);
        h.frame(vec![press(false)]);
        h.frame(vec![]);
        h.frame(vec![Event::Text("hola".into())]);
        h.frame(vec![]);
        h.canvas.notes.finish_editing(&mut h.editor);
        let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(note.text, "hola");
    }

    #[test]
    fn typing_in_a_new_note_writes_its_text() {
        let mut h = Harness::new();
        let (id, _) = with_note(&mut h);
        h.canvas.notes.start_editing(&mut h.editor, id);
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![Event::Text("hola".into())]);
        h.frame(vec![]);
        h.canvas.notes.finish_editing(&mut h.editor);
        let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(note.text, "hola");
    }

    #[test]
    fn a_note_is_drawn_over_the_artboard_and_takes_the_clicks() {
        let mut h = Harness::new();
        let (_, paper) = with_note(&mut h);
        let texture = h.note_texture.unwrap();
        let drawn = h
            .shapes
            .iter()
            .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == texture));
        assert!(drawn, "the note piece");
        let before = h.editor.sprite().clone();
        // Un clic en el cuerpo de la nota, sobre el artboard: no pinta.
        h.click(paper.center(), PointerButton::Primary);
        assert_eq!(*h.editor.sprite(), before, "the pencil did not draw");
    }

    #[test]
    fn far_away_a_note_is_its_pin_and_opens_with_the_pointer_over_it() {
        let mut h = Harness::new();
        with_note(&mut h);
        let texture = h.note_texture.unwrap();
        let drew_note = |h: &Harness| {
            h.shapes
                .iter()
                .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == texture))
        };
        assert!(drew_note(&h), "near: the note");
        // Lejos (el artboard de 16 a 2 puntos por píxel: 32), la nota taparía todo: su pin.
        h.canvas.zoom = 2.0;
        h.frame(vec![egui::Event::PointerMoved(Pos2::new(790.0, 590.0))]);
        assert!(!drew_note(&h), "far: only its pin");
        let pin = h.canvas.to_screen(h.rect, (0, 0)).round() + Vec2::new(5.0, 11.0 + 5.0);
        h.frame(vec![egui::Event::PointerMoved(pin)]);
        h.frame(vec![]);
        assert!(drew_note(&h), "the pointer over the pin opens it");
    }

    #[test]
    fn the_bar_of_a_note_shows_only_with_the_pointer_over_it() {
        let mut h = Harness::new();
        let skin = with_window(&mut h);
        let bar = piece_id(&skin, "note");
        crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
        h.editor.add_note((0, 0)).unwrap();
        h.frame(vec![]);
        let paper = h.canvas.to_screen(h.rect, (0, 0)).round();
        let width = crate::notes::NEW_SIZE.0 as f32;
        // Sin la barra, su línea es el borde de arriba del papel (en tinta).
        let top_line = |h: &Harness| {
            h.shapes.iter().any(|s| {
                matches!(s, egui::Shape::Rect(r)
                    if r.fill == Color32::from_rgb(5, 4, 3)
                        && r.rect == Rect::from_min_size(paper + Vec2::new(0.0, 11.0), Vec2::new(width, 1.0)))
            })
        };
        let drew = |h: &Harness| {
            h.shapes
                .iter()
                .any(|s| matches!(s, egui::Shape::Mesh(m) if m.texture_id == bar))
        };
        assert!(drew(&h) && top_line(&h), "away: from the line of the bar");
        h.frame(vec![egui::Event::PointerMoved(
            paper + Vec2::new(40.0, 40.0),
        )]);
        h.frame(vec![]);
        assert!(drew(&h) && !top_line(&h), "hovered: with its bar");
    }

    #[test]
    fn the_color_strip_shows_with_the_bar_and_a_swatch_changes_the_color() {
        let mut h = Harness::new();
        let (id, paper) = with_note(&mut h);
        // Con el puntero en la nota aparece la tira; el rosa es el segundo cuadrito.
        h.frame(vec![egui::Event::PointerMoved(paper.center())]);
        h.frame(vec![]);
        let pink = Pos2::new(paper.right() + 3.0 + 8.0, paper.top() + 1.0 + 8.0 + 11.0);
        h.click(pink, PointerButton::Primary);
        let color = |h: &Harness| {
            h.editor
                .document
                .notes
                .iter()
                .find(|n| n.id == id)
                .unwrap()
                .color
        };
        assert_eq!(color(&h), cs_core::NoteColor::Pink);
        h.editor.undo();
        assert_eq!(color(&h), cs_core::NoteColor::Yellow, "one step");
    }

    #[test]
    fn dragging_the_bar_moves_the_note_in_one_undo_step() {
        let mut h = Harness::new();
        let (id, paper) = with_note(&mut h);
        let zoom = h.canvas.zoom;
        let bar = paper.left_top() + Vec2::new(40.0, 5.0);
        // La barra sale con el puntero sobre el papel.
        h.frame(vec![Event::PointerMoved(paper.center())]);
        h.drag(bar, bar + Vec2::new(zoom * 3.0, zoom * 2.0));
        let note = |h: &Harness| {
            h.editor
                .document
                .notes
                .iter()
                .find(|n| n.id == id)
                .cloned()
                .unwrap()
        };
        assert_eq!(note(&h).position, (3, 2));
        h.editor.undo();
        assert_eq!(note(&h).position, (0, 0), "one step");
    }

    #[test]
    fn the_corner_resizes_and_the_circle_deletes_the_note() {
        let mut h = Harness::new();
        let (id, paper) = with_note(&mut h);
        let corner = paper.right_bottom() - Vec2::splat(3.0);
        h.drag(corner, corner + Vec2::new(30.0, 20.0));
        let note = h
            .editor
            .document
            .notes
            .iter()
            .find(|n| n.id == id)
            .cloned()
            .unwrap();
        assert_eq!(
            note.size,
            (crate::notes::NEW_SIZE.0 + 30, crate::notes::NEW_SIZE.1 + 20)
        );
        // El "○", contra el borde derecho de la nota ya agrandada.
        let circle = Pos2::new(paper.right() + 30.0 - 9.0, paper.top() + 6.0);
        h.click(circle, PointerButton::Primary);
        assert!(h.editor.document.notes.is_empty(), "deleted");
        h.editor.undo();
        assert_eq!(h.editor.document.notes.len(), 1, "undone");
    }

    #[test]
    fn typing_in_a_note_saves_its_text_in_one_step() {
        let mut h = Harness::new();
        let (id, _) = with_note(&mut h);
        h.canvas.notes.start_editing(&mut h.editor, id);
        h.frame(vec![]);
        h.frame(vec![egui::Event::Text("hola".into())]);
        h.frame(vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }]);
        h.frame(vec![]);
        let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(note.text, "hola");
        h.editor.undo();
        let note = h.editor.document.notes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(note.text, "", "one step");
    }

    /// Dónde está dibujado el texto `label` en el último frame (los "…" del menú pixel son los
    /// de la fuente, `tab_bar::ellipsis`).
    fn text_rect(h: &Harness, label: &str) -> Option<Rect> {
        let shown = label.replace('…', crate::tab_bar::ellipsis());
        h.shapes.iter().find_map(|s| match s {
            egui::Shape::Text(t) if t.galley.text() == shown => Some(t.visual_bounding_rect()),
            _ => None,
        })
    }

    /// Con Pixel UI (el desplegable pixel, anclado al "≡" de 9 px) se abre sin tronar y cada
    /// opción cierra el menú.
    #[test]
    fn every_item_of_the_artboard_menu_works() {
        for item in [
            "Rename",
            "Properties",
            "Duplicate",
            "Sprite Size",
            "Canvas Size",
            "Delete",
        ] {
            let mut h = Harness::new();
            h.editor.add_artboard(8, 8);
            let skin = with_window(&mut h);
            // Con el skin del frame, como en la app: el menú es el desplegable pixel.
            crate::pixel_ui::set_bubble(&h.ctx, Some(std::sync::Arc::new(skin)));
            let pixels = h.pixels_of(0);
            let menu = pixels.left_top() + Vec2::new(-2.0 + 2.0 + 4.5, -14.0 + 7.5);
            h.click(menu, PointerButton::Primary);
            h.frame(vec![]);
            let at = text_rect(&h, item).unwrap_or_else(|| panic!("no {item}"));
            h.click(at.center(), PointerButton::Primary);
            for _ in 0..3 {
                h.frame(vec![]);
            }
            assert!(text_rect(&h, item).is_none(), "{item}: the menu closed");
        }
    }

    #[test]
    fn inactive_windows_are_dimmed_and_the_active_one_is_not() {
        let mut h = Harness::new();
        h.editor.add_artboard(16, 16);
        h.canvas.zoom = 4.0;
        let skin = with_window(&mut h);
        let id = piece_id(&skin, "artboard.frame");
        let tints: Vec<(Rect, Color32)> = h
            .shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Mesh(m) if m.texture_id == id => {
                    Some((m.calc_bounds(), m.vertices[0].color))
                }
                _ => None,
            })
            .collect();
        let (first, second) = (h.pixels_of(0), h.pixels_of(1));
        let tint_of = |pixels: Rect| {
            tints
                .iter()
                .filter(|(r, _)| r.intersects(pixels.expand(1.0)))
                .map(|(_, c)| *c)
                .collect::<HashSet<_>>()
        };
        // El activo es el último que se agregó.
        assert_eq!(tint_of(second), HashSet::from([Color32::WHITE]));
        let dim = tint_of(first);
        assert_eq!(dim.len(), 1);
        assert!(!dim.contains(&Color32::WHITE), "{dim:?}");
    }

    #[test]
    fn a_narrow_artboard_keeps_its_bar_inside_the_window() {
        let mut h = Harness::new();
        let skin = with_window(&mut h);
        let label = label_text(h.editor.artboard());
        // 16 px de artboard a 1.5 (ventana de 28): íconos sin nombre; a 1 y 0.5, solo la barra.
        for (zoom, icons) in [(1.5, true), (1.0, false), (0.5, false)] {
            h.canvas.zoom = zoom;
            h.frame(vec![]);
            let pixels = h.pixels_of(0);
            let frames = h.meshes(piece_id(&skin, "artboard.frame"));
            let frame = union(&frames);
            assert_eq!(frame.width(), pixels.width() + 4.0, "zoom {zoom}");
            assert_eq!(pixels.top() - frame.top(), 14.0, "zoom {zoom}");
            for name in ["artboard.menu", "window.close"] {
                let drawn = h.meshes(piece_id(&skin, name));
                assert_eq!(!drawn.is_empty(), icons, "{name} at {zoom}");
                assert!(drawn.iter().all(|r| frame.contains_rect(*r)), "{name}");
            }
            assert!(
                h.texts().iter().all(|(t, _)| !t.starts_with(&label[..3])),
                "no name at {zoom}"
            );
        }
    }

    /// Dos artboards a zoom 8 con la herramienta Artboard: el de atrás (0) en (0, 0) y el de
    /// adelante (1) en `front`, el de atrás activo.
    fn two_windows(front: (i32, i32)) -> (Harness, ArtboardId, ArtboardId) {
        let mut h = Harness::new();
        h.canvas.zoom = 8.0;
        h.editor.add_artboard(16, 16);
        h.editor.document.artboards[1].position = front;
        let (back, front) = (
            h.editor.document.artboards[0].id,
            h.editor.document.artboards[1].id,
        );
        h.editor.activate(back);
        h.editor.set_tool(ToolKind::Artboard);
        with_window(&mut h);
        (h, back, front)
    }

    #[test]
    fn a_bar_covered_by_another_artboard_does_not_take_the_click() {
        // Los píxeles del de adelante tapan toda la barra del de atrás.
        let (mut h, back, front) = two_windows((0, -16));
        let at = h.pixels_of(0).left_top() + Vec2::new(60.0, -10.0);
        assert!(h.pixels_of(1).contains(at));
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::Move, "the front artboard");
        h.drag(at, at + Vec2::new(16.0, 8.0));
        assert_eq!(h.editor.active, front);
        assert_eq!(h.editor.document.artboards[0].position, (0, 0));
        assert_eq!(h.editor.document.artboards[1].position, (2, -15));
        assert_ne!(h.editor.active, back);
    }

    #[test]
    fn a_visible_bar_under_another_window_still_moves_its_artboard() {
        let (mut h, back, _) = two_windows((0, -40));
        let at = h.pixels_of(0).left_top() + Vec2::new(60.0, -10.0);
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::Grab);
        h.drag(at, at + Vec2::new(16.0, 8.0));
        assert_eq!(h.editor.active, back);
        assert_eq!(h.editor.document.artboards[0].position, (2, 1));
        assert_eq!(h.editor.document.artboards[1].position, (0, -40));
    }

    /// Un artboard de 16×16 a zoom 8, con Pixel UI y la herramienta Artboard.
    fn artboard_tool_window() -> Harness {
        let mut h = Harness::new();
        h.canvas.zoom = 8.0;
        h.editor.set_tool(ToolKind::Artboard);
        with_window(&mut h);
        h
    }

    fn rect_of(h: &Harness) -> ((i32, i32), (u32, u32)) {
        let a = &h.editor.document.artboards[0];
        (a.position, (a.sprite.width, a.sprite.height))
    }

    #[test]
    fn with_the_artboard_tool_the_top_edge_resizes_from_the_bar() {
        let mut h = artboard_tool_window();
        let at = h.pixels_of(0).center_top() - Vec2::new(0.0, 3.0);
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::ResizeVertical);
        h.drag(at, at - Vec2::new(0.0, 16.0));
        assert_eq!(rect_of(&h), ((0, -2), (16, 18)));
    }

    #[test]
    fn with_the_artboard_tool_the_middle_of_the_bar_moves() {
        let mut h = artboard_tool_window();
        let at = h.pixels_of(0).center_top() - Vec2::new(0.0, 10.0);
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::Grab);
        h.drag(at, at + Vec2::new(16.0, 8.0));
        assert_eq!(rect_of(&h), ((2, 1), (16, 16)));
    }

    #[test]
    fn with_the_artboard_tool_a_top_corner_resizes_from_the_bar() {
        let mut h = artboard_tool_window();
        // Sobre la barra (y el "≡"), a 2 de la esquina de los píxeles.
        let at = h.pixels_of(0).left_top() + Vec2::new(2.0, -3.0);
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::ResizeNwSe);
        h.drag(at, at - Vec2::new(16.0, 16.0));
        assert_eq!(rect_of(&h), ((-2, -2), (18, 18)));
        let mut h = artboard_tool_window();
        let at = h.pixels_of(0).right_top() + Vec2::new(-2.0, -3.0);
        h.frame(vec![Event::PointerMoved(at)]);
        assert_eq!(h.cursor, egui::CursorIcon::ResizeNeSw);
        h.drag(at, at + Vec2::new(16.0, -16.0));
        assert_eq!(rect_of(&h), ((0, -2), (18, 18)));
    }

    #[test]
    fn the_first_row_of_pixels_is_not_the_bar() {
        let mut h = Harness::new();
        h.canvas.zoom = 8.0;
        with_window(&mut h);
        let at = h.pixels_of(0).center_top();
        h.frame(vec![Event::PointerMoved(at)]);
        assert_ne!(h.cursor, egui::CursorIcon::Grab);
        assert!(!h.canvas.over_label);
    }

    #[test]
    fn an_off_screen_artboard_draws_no_window() {
        let mut h = Harness::new();
        let skin = with_window(&mut h);
        h.canvas.pan += Vec2::new(5000.0, 0.0);
        h.frame(vec![]);
        for name in ["artboard.frame", "artboard.menu", "window.close"] {
            assert!(h.meshes(piece_id(&skin, name)).is_empty(), "{name}");
        }
        let checker = h.canvas.checker.as_ref().unwrap().id();
        assert!(h.meshes(checker).is_empty());
    }

    #[test]
    fn without_pixel_ui_the_label_and_the_orange_border_stay() {
        let h = Harness::new();
        let pixels = h.pixels_of(0);
        let label = label_text(h.editor.artboard());
        let (_, text) = h
            .texts()
            .into_iter()
            .find(|(t, _)| *t == label)
            .expect("the label");
        assert_eq!(text.bottom(), pixels.top() - LABEL_GAP);
        assert_eq!(text.left(), pixels.left());
        let border = h.shapes.iter().any(|s| {
            matches!(s, egui::Shape::Rect(r)
                if r.rect == pixels && r.stroke == Stroke::new(2.0, theme::ACCENT))
        });
        assert!(border, "the orange border of the active artboard");
        let skin = Skin::embedded(&h.ctx);
        assert!(h.meshes(piece_id(&skin, "artboard.frame")).is_empty());
    }

    #[test]
    fn with_a_fractional_pan_the_frame_hugs_the_drawn_pixels_with_one_white_point() {
        let mut h = Harness::new();
        let skin = with_window(&mut h);
        let id = h.editor.artboard().id;
        for zoom in [1.0, 8.0] {
            for pan in [
                Vec2::new(0.37, 0.81),
                Vec2::new(0.62, 0.5),
                Vec2::new(0.5, 0.12),
            ] {
                h.canvas.zoom = zoom;
                h.canvas.pan = pan;
                h.frame(vec![]);
                let at = format!("zoom {zoom}, pan {pan:?}");
                // Los píxeles y el damero se dibujan alineados a píxeles de pantalla.
                let pixels = h.pixels_of(0).round_to_pixels(1.0);
                let tex = h.canvas.textures[&id].id();
                assert_eq!(h.meshes(tex), vec![pixels], "texture, {at}");
                let checker = h.canvas.checker.as_ref().unwrap().id();
                assert_eq!(h.meshes(checker), vec![pixels], "checker, {at}");
                // El marco: borde (1) + blanco (1) a los lados y abajo, barra (13) + blanco (1)
                // arriba; ni un píxel más ni menos.
                let frame = union(&h.meshes(piece_id(&skin, "artboard.frame")));
                assert_eq!(
                    frame,
                    Rect::from_min_max(
                        pixels.min - Vec2::new(2.0, 14.0),
                        pixels.max + Vec2::new(2.0, 2.0)
                    ),
                    "{at}"
                );
            }
        }
    }

    /// Las líneas de 1 px dibujadas con `color` (guías), como `(x o y, eje)`.
    fn lines_of(h: &Harness, color: Color32) -> Vec<(f32, Axis)> {
        h.shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::LineSegment { points, stroke } if stroke.color == color => {
                    let [a, b] = *points;
                    Some(if a.x == b.x {
                        (a.x, Axis::Vertical)
                    } else {
                        (a.y, Axis::Horizontal)
                    })
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn with_a_fractional_pan_texture_corners_guides_and_hover_agree() {
        let mut h = Harness::new();
        h.view.extras = true;
        h.view.guides = true;
        h.editor.set_guides(vec![
            Guide {
                axis: Axis::Vertical,
                position: 3,
            },
            Guide {
                axis: Axis::Horizontal,
                position: 5,
            },
        ]);
        let id = h.editor.artboard().id;
        h.canvas.zoom = 8.0;
        for pan in [
            Vec2::new(0.37, 0.81),
            Vec2::new(0.62, 0.5),
            Vec2::new(-0.5, 0.12),
            Vec2::new(-3.7, 11.49),
        ] {
            h.canvas.pan = pan;
            h.frame(vec![]);
            let at = format!("pan {pan:?}");
            // La textura dibujada y `to_screen` de las esquinas son el mismo rect.
            let drawn = h.meshes(h.canvas.textures[&id].id());
            assert_eq!(drawn, vec![h.pixels_of(0)], "texture vs to_screen, {at}");
            let drawn = drawn[0];
            // Las guías caen en la fila/columna de píxeles de pantalla que empieza en su borde.
            let mut guides = lines_of(&h, theme::GUIDE_LINE);
            guides.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut expected = vec![
                (drawn.left() + 3.0 * 8.0 + 0.5, Axis::Vertical),
                (drawn.top() + 5.0 * 8.0 + 0.5, Axis::Horizontal),
            ];
            expected.sort_by(|a, b| a.0.total_cmp(&b.0));
            assert_eq!(guides, expected, "guides, {at}");
            // El píxel bajo el puntero es el que se ve: justo a cada lado del borde (3, 3).
            let edge = drawn.min + Vec2::splat(3.0 * 8.0);
            h.frame(vec![Event::PointerMoved(edge + Vec2::splat(0.05))]);
            assert_eq!(h.canvas.hover, Some((3, 3)), "inside, {at}");
            h.frame(vec![Event::PointerMoved(edge - Vec2::splat(0.05))]);
            assert_eq!(h.canvas.hover, Some((2, 2)), "outside, {at}");
        }
    }

    #[test]
    fn the_origin_snaps_to_physical_pixels() {
        let mut h = Harness::new();
        for ppp in [1.0, 1.5, 2.0] {
            h.canvas.ppp = ppp;
            h.canvas.pan = Vec2::new(0.37, -0.81);
            let o = h.canvas.origin(h.rect) * ppp;
            assert_eq!(o, o.round(), "ppp {ppp}");
            let corner = h.canvas.to_screen(h.rect, (3, -2)) * ppp;
            assert_eq!(corner, corner.round(), "ppp {ppp}");
        }
    }

    #[test]
    fn panning_keeps_the_stored_pan_and_zoom_keeps_the_point_under_the_cursor() {
        let mut h = Harness::new();
        h.canvas.zoom = 8.0;
        let start = h.canvas.origin(h.rect);
        // Pans fraccionarios repetidos: el pan guardado no se redondea, no hay deriva.
        for _ in 0..10 {
            h.canvas.pan += Vec2::new(0.3, -0.3);
        }
        h.frame(vec![]);
        let moved = h.canvas.origin(h.rect) - start;
        assert_eq!(moved, Vec2::new(3.0, -3.0));
        // Zoom hacia el cursor: el punto del mundo bajo el cursor sigue ahí (a < 1 px físico).
        for ppp in [1.0, 2.0] {
            h.canvas.ppp = ppp;
            for step_in in [true, true, false, true, false, false] {
                let anchor = Pos2::new(311.37, 207.81);
                let (wx, wy) = h.canvas.to_world(h.rect, anchor);
                h.canvas.zoom_at(h.rect, anchor, step_in);
                let back = h.canvas.origin(h.rect) + Vec2::new(wx, wy) * h.canvas.zoom;
                let off = (back - anchor) * ppp;
                assert!(
                    off.x.abs() <= 1.0 && off.y.abs() <= 1.0,
                    "ppp {ppp}, zoom {}: {off:?}",
                    h.canvas.zoom
                );
            }
        }
    }
}
