use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use cs_core::{Artboard, ArtboardId, Axis, GridSettings, Guide, LayerId, Mask};
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

mod input;
mod rulers;

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
/// Alto de la franja de la migaja de pan del modo símbolo (arriba del lienzo).
const CRUMBS_PX: f32 = 14.0;

/// Los artboards que muestra el lienzo, en su orden: los del lienzo o, en el modo símbolo, solo
/// el que se edita.
fn shown_artboards<'a>(
    document: &'a cs_core::Document,
    editor: &Editor,
) -> impl DoubleEndedIterator<Item = &'a Artboard> + use<'a> {
    let only = editor.in_symbol_mode().then_some(editor.active);
    document.artboards.iter().filter(move |a| match only {
        Some(id) => a.id == id,
        None => !a.in_library,
    })
}
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
    /// La revisión con la que se compusieron todos los artboards: mientras se pinta, los que no
    /// son el activo (ni muestran instancias) no cambian y no se recomponen.
    all_rendered: Option<u64>,
    /// Cuántas veces se compuso un artboard (para los tests).
    #[cfg(test)]
    composed: u32,
    /// La capa cuya máscara se veía en rojo (editándola) al componer: si cambia, se recompone
    /// aunque el documento no haya cambiado.
    rendered_mask: Option<LayerId>,
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
    /// La proporción de píxel del artboard activo (Sprite → Properties): con 2:1 cada píxel se
    /// ve el doble de ancho. Se toma al empezar cada frame.
    ratio: Vec2,
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
    /// Por qué falló el último soltar de un símbolo de la biblioteca; el app lo muestra.
    pub drop_error: Option<&'static str>,
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
    /// Se pidió "Convert to Symbol…" desde el menú de la selección (la app lo toma).
    pub symbol_request: bool,
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
    /// El zoom y el paneo del documento al entrar al modo símbolo: se restauran al salir.
    symbol_return_view: Option<(f32, Vec2)>,
    /// Si se ve la migaja de pan (modo símbolo) en este frame.
    crumbs_shown: bool,
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
            all_rendered: None,
            #[cfg(test)]
            composed: 0,
            rendered_mask: None,
            fitted: false,
            hover: None,
            guide_hint: None,
            dragging_guide: None,
            over_label: false,
            names_visible: true,
            pending_zoom: None,
            ratio: Vec2::splat(1.0),
            ruler_drag: None,
            sym_drag: None,
            over_ruler: false,
            draw_pipette: false,
            draw_rotate: false,
            pixel_cursor: None,
            renaming: None,
            slice_menu: false,
            drop_error: None,
            selection_menu: false,
            slice_properties: false,
            notes: Default::default(),
            artboard_request: None,
            symbol_request: false,
            wheel_rest: 0.0,
            background: theme::CANVAS_BG,
            pixel_rulers: None,
            rulers_shown: false,
            artboard_window: None,
            pixel_ui: false,
            cursors: None,
            symmetry_color: None,
            ppp: 1.0,
            symbol_return_view: None,
            crumbs_shown: false,
        }
    }
}

impl CanvasView {
    /// La textura del artboard activo y su tamaño si muestra el frame del editor tal cual (sin
    /// papel cebolla ni la máscara en rojo): la vista previa la reusa en vez de componer otra.
    pub fn plain_texture(&self, editor: &Editor) -> Option<(egui::TextureId, Vec2)> {
        let onion = editor.onion_enabled && !editor.playing;
        if onion || editor.editing_mask() {
            return None;
        }
        let texture = self.textures.get(&editor.active)?;
        Some((texture.id(), texture.size_vec2()))
    }

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
        let w = (-self.pan - size / 2.0) / self.scale();
        (w.x.round() as i32, w.y.round() as i32)
    }

    /// Cuánto mide un píxel en pantalla, a lo ancho y a lo alto: el zoom por la proporción de
    /// píxel (con 1:1, el zoom en los dos).
    pub(crate) fn scale(&self) -> Vec2 {
        self.ratio * self.zoom
    }

    fn to_screen(&self, rect: Rect, world: (i32, i32)) -> Pos2 {
        self.origin(rect) + Vec2::new(world.0 as f32, world.1 as f32) * self.scale()
    }

    fn to_world(&self, rect: Rect, p: Pos2) -> (f32, f32) {
        let w = (p - self.origin(rect)) / self.scale();
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
            .shown_artboard_at(world)
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

    /// Ajusta el zoom a todo el lienzo (en el modo símbolo, al símbolo).
    fn fit(&mut self, rect: Rect, editor: &Editor) {
        let bounds = if editor.in_symbol_mode() {
            Some(editor.artboard().rect())
        } else {
            editor.document.bounds()
        };
        let Some(((x0, y0), (x1, y1))) = bounds else {
            return;
        };
        let size = Vec2::new((x1 - x0) as f32, (y1 - y0) as f32) * self.ratio;
        let max = (rect.size() * 0.85 / size).min_elem();
        self.zoom = ZOOM_LEVELS
            .iter()
            .copied()
            .rfind(|&z| z <= max)
            .unwrap_or(ZOOM_LEVELS[0]);
        let center = Vec2::new((x0 + x1) as f32, (y0 + y1) as f32) / 2.0;
        self.pan = -center * self.scale();
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
        let world = (anchor - self.origin(rect)) / self.scale();
        self.zoom = next;
        self.pan = anchor - world * self.scale() - rect.center();
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
                let center = (rect.center() - self.origin(rect)) / self.scale();
                self.zoom = 1.0;
                self.pan = -center * self.scale();
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
        self.pan = -center * self.scale();
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
            origin + Vec2::new(c2.0 as f32, c2.1 as f32) * (self.scale() / 2.0),
            c2,
        )
    }

    /// Con Tiled Mode, las copias del artboard activo alrededor de él hasta los bordes de la vista
    /// (detrás de los artboards; el original se dibuja con los demás).
    fn draw_tiled(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        document: &cs_core::Document,
        checker: egui::TextureId,
        ppp: f32,
    ) {
        let tiled = editor.tiled;
        if tiled == crate::editor::TiledMode::None {
            return;
        }
        let (Some(artboard), Some(texture)) = (
            document.artboard(editor.active),
            self.textures.get(&editor.active),
        ) else {
            return;
        };
        let screen = WindowLayout::pixels_on_screen(self.pixel_rect(rect, artboard), ppp);
        let size = screen.size();
        if size.x < 1.0 || size.y < 1.0 {
            return;
        }
        // Cuántas copias entran a cada lado (con un tope por si el zoom es muy chico).
        let span = |lo: f32, hi: f32, from: f32, step: f32| {
            let first = ((lo - from) / step).floor().max(-64.0) as i32;
            let last = ((hi - from) / step).ceil().min(64.0) as i32;
            first..=last
        };
        let xs = if tiled.x() {
            span(rect.left(), rect.right(), screen.left(), size.x)
        } else {
            0..=0
        };
        let ys = if tiled.y() {
            span(rect.top(), rect.bottom(), screen.top(), size.y)
        } else {
            0..=0
        };
        let ((x0, y0), (x1, y1)) = artboard.rect();
        let uv = checker_uv((x1 - x0) as u32, (y1 - y0) as u32);
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        for j in ys {
            for i in xs.clone() {
                if (i, j) == (0, 0) {
                    continue;
                }
                let copy = screen.translate(Vec2::new(i as f32 * size.x, j as f32 * size.y));
                if !copy.intersects(rect) {
                    continue;
                }
                painter.image(checker, copy, uv, Color32::WHITE);
                painter.image(texture.id(), copy, full_uv, Color32::WHITE);
            }
        }
    }

    /// Lo que ocupa en pantalla cada artboard de `document`, en su orden (con su ventana, con
    /// Pixel UI).
    fn covers(
        &self,
        rect: Rect,
        document: &cs_core::Document,
        editor: &Editor,
        ppp: f32,
    ) -> Vec<Rect> {
        shown_artboards(document, editor)
            .map(|a| {
                let pixels = WindowLayout::pixels_on_screen(self.pixel_rect(rect, a), ppp);
                if self.artboard_window.is_some() {
                    WindowLayout::frame_around(pixels, ppp)
                } else {
                    pixels
                }
            })
            .collect()
    }

    /// Ejes de simetría punteados sobre el artboard activo y su centro, como se ve en `document`
    /// (mientras se arrastra el artboard, se mueven con él), sin lo que tapan los artboards de
    /// delante (`covers`).
    fn draw_symmetry(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        editor: &Editor,
        document: &cs_core::Document,
        covers: &[Rect],
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
                covers,
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
        for seg in segments
            .into_iter()
            .flat_map(|s| segment_uncovered(s, covers))
        {
            painter.extend(egui::Shape::dashed_line(&seg, stroke, 6.0, 4.0));
        }
        if !covers.iter().any(|r| r.contains(c)) {
            painter.circle(
                c,
                4.0,
                theme::ACCENT,
                Stroke::new(1.0, theme::CANVAS_BORDER),
            );
        }
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
        if let Some(payload) = response.dnd_release_payload::<crate::library_panel::SymbolPayload>()
            && let Some(pos) = response.hover_pos()
        {
            let w = self.to_world(rect, pos);
            if let Err(why) =
                editor.drop_symbol(payload.0, (w.0.floor() as i32, w.1.floor() as i32))
            {
                self.drop_error = Some(why);
            }
        }
        // Antes de cualquier uso de `origin` en este frame (zoom, entrada, dibujo).
        self.ppp = ctx.pixels_per_point();
        let (rx, ry) = editor.sprite().pixel_ratio;
        self.ratio = Vec2::new(rx.max(1) as f32, ry.max(1) as f32);
        self.rulers_shown = view_settings.rulers;
        self.crumbs_shown = editor.in_symbol_mode();
        self.sync_symbol_view(editor);
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
        // Si la entrada salió del modo símbolo, la vista del documento vuelve ya.
        self.crumbs_shown = editor.in_symbol_mode();
        self.sync_symbol_view(editor);
        let modal = dialog.is_some();
        let guide = dialog.and_then(|d| d.preview_rect(&editor.document));

        let view = editor.view();
        let document = view.document.as_ref();

        // Recompone las texturas si el documento cambió o hay un gesto en curso.
        let painting = editor.is_painting();
        let mask = editor.editing_mask().then_some(editor.layer);
        if painting || self.rendered_revision != Some(editor.revision) || self.rendered_mask != mask
        {
            self.rendered_mask = mask;
            self.textures
                .retain(|id, _| document.artboard(*id).is_some());
            let mut skipped = false;
            for artboard in shown_artboards(document, editor) {
                let sprite = &artboard.sprite;
                let unchanged = painting
                    && self.all_rendered == Some(editor.revision)
                    && artboard.id != editor.active
                    && self.textures.contains_key(&artboard.id)
                    && !sprite.layers.iter().any(|l| l.instance.is_some());
                if unchanged {
                    skipped = true;
                    continue;
                }
                #[cfg(test)]
                {
                    self.composed += 1;
                }
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
            if !skipped {
                self.all_rendered = Some(editor.revision);
            }
            // Durante un gesto no marcamos como al día, para recomponer al terminar.
            self.rendered_revision = (!painting).then_some(editor.revision);
        }

        let checker = self.checker(&ctx);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.background);

        let ppp = ctx.pixels_per_point();
        self.draw_tiled(&painter, rect, editor, document, checker, ppp);
        for artboard in shown_artboards(document, editor) {
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
                    self.scale(),
                    view_settings.grid_color(),
                );
            }
            if self.zoom >= 8.0 && view_settings.shows(view_settings.pixel_grid) {
                let stroke = Stroke::new(1.0, theme::GRID);
                for x in 1..artboard.sprite.width {
                    painter.vline(
                        screen.left() + x as f32 * self.scale().x,
                        screen.y_range(),
                        stroke,
                    );
                }
                for y in 1..artboard.sprite.height {
                    painter.hline(
                        screen.x_range(),
                        screen.top() + y as f32 * self.scale().y,
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
                    self.scale(),
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

        // Lo que ocupa cada artboard en pantalla: las guías y la simetría de uno no se ven encima
        // de los que van delante.
        let covers = self.covers(rect, document, editor, ui.ctx().pixels_per_point());
        if view_settings.shows(view_settings.guides) {
            let dragged = self.ruler_drag.and_then(|d| d.index);
            for (k, artboard) in shown_artboards(document, editor).enumerate() {
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
                    draw_guide(
                        &painter,
                        screen,
                        *g,
                        self.scale(),
                        theme::GUIDE_LINE,
                        &covers[k + 1..],
                    );
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
            // Los artboards de delante del activo tapan sus ejes.
            let front = shown_artboards(document, editor)
                .position(|a| a.id == editor.active)
                .map_or(&[][..], |k| &covers[k + 1..]);
            self.draw_symmetry(&painter, rect, editor, document, front);
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
            draw_slices(&painter, &artboard.sprite, editor, origin, self.scale());
        }
        if let (Some(mask), Some(origin)) = (&view.selection, active_origin)
            && view_settings.shows(view_settings.selection_edges)
        {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_marching_ants(&painter, mask, origin, self.scale(), phase, self.pixel_ui);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }
        if let Some(origin) = active_origin
            && handles_active(editor)
            && let Some(b) = editor.transform_box()
        {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_transform_box(&painter, &b, origin, self.scale(), phase, self.pixel_ui);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }
        if let Some(origin) = active_origin
            && view_settings.shows(view_settings.tile_numbers)
            && let Some(space) = editor.tile_space()
            && space.tile_size.0 as f32 * self.scale().x >= TILE_NUMBER_MIN_PX
            && space.tile_size.1 as f32 * self.scale().y >= TILE_NUMBER_MIN_PX
            && let Some(artboard) = document.artboard(editor.active)
        {
            let font = FontId::monospace(10.0);
            // Solo las celdas dentro del área visible del lienzo.
            let min = ((rect.min - origin) / self.scale()).floor();
            let max = ((rect.max - origin) / self.scale()).ceil();
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
                let at = origin + Vec2::new(x as f32, y as f32) * self.scale() + Vec2::splat(2.0);
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
                let at = |(x, y): (i32, i32)| origin + Vec2::new(x as f32, y as f32) * self.scale();
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
                            origin + Vec2::new(x0 as f32, y0 as f32) * self.scale(),
                            Vec2::new((size * tw) as f32, (size * th) as f32) * self.scale(),
                        )
                    }
                    None => Rect::from_min_size(
                        origin
                            + Vec2::new((hx + offset) as f32, (hy + offset) as f32) * self.scale(),
                        Vec2::splat(size as f32) * self.scale(),
                    ),
                };
                if let Some(fill) = fill {
                    painter.rect_filled(r, 0.0, fill);
                }
                painter.rect_stroke(r, 0.0, stroke, StrokeKind::Inside);
            }
        }

        // Las notas (post-its): encima de los artboards y debajo de las reglas. En el modo
        // símbolo no se ven (son del documento).
        if !editor.in_symbol_mode() {
            let (origin, scale) = (self.origin(rect), self.scale());
            let to_screen =
                move |(x, y): (i32, i32)| origin + Vec2::new(x as f32, y as f32) * scale;
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
        // La migaja de pan del modo símbolo, debajo de la regla de arriba.
        self.breadcrumb(ui, &painter, rect, view_settings, editor);

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
/// Una guía de un artboard (en `screen`), sin los tramos que tapan los artboards de adelante
/// (`covers`).
fn draw_guide(
    painter: &egui::Painter,
    screen: Rect,
    guide: Guide,
    zoom: Vec2,
    color: Color32,
    covers: &[Rect],
) {
    let stroke = Stroke::new(1.0, color);
    match guide.axis {
        Axis::Horizontal => {
            let y = screen.top() + guide.position as f32 * zoom.y;
            if (screen.top()..=screen.bottom()).contains(&y) {
                let cut: Vec<_> = covers
                    .iter()
                    .filter(|c| c.y_range().contains(y))
                    .map(|c| c.x_range())
                    .collect();
                for span in uncovered(screen.x_range(), &cut) {
                    painter.hline(span, painter.round_to_pixel_center(y), stroke);
                }
            }
        }
        Axis::Vertical => {
            let x = screen.left() + guide.position as f32 * zoom.x;
            if (screen.left()..=screen.right()).contains(&x) {
                let cut: Vec<_> = covers
                    .iter()
                    .filter(|c| c.x_range().contains(x))
                    .map(|c| c.y_range())
                    .collect();
                for span in uncovered(screen.y_range(), &cut) {
                    painter.vline(painter.round_to_pixel_center(x), span, stroke);
                }
            }
        }
    }
}

/// Los tramos de `range` que no tapa ninguno de `cut`.
fn uncovered(range: egui::Rangef, cut: &[egui::Rangef]) -> Vec<egui::Rangef> {
    let mut spans = vec![range];
    for c in cut {
        spans = spans
            .into_iter()
            .flat_map(|s| {
                let before = egui::Rangef::new(s.min, s.max.min(c.min));
                let after = egui::Rangef::new(s.min.max(c.max), s.max);
                [before, after].into_iter().filter(|r| r.max > r.min)
            })
            .collect();
    }
    spans
}

/// Líneas del grid de trabajo dentro del artboard (se omiten si quedan a menos de 4 px).
fn draw_grid(
    painter: &egui::Painter,
    screen: Rect,
    grid: GridSettings,
    zoom: Vec2,
    color: Color32,
) {
    let (sx, sy) = (
        grid.size.0.max(1) as f32 * zoom.x,
        grid.size.1.max(1) as f32 * zoom.y,
    );
    if sx < 4.0 || sy < 4.0 {
        return;
    }
    let stroke = Stroke::new(1.0, color);
    let first = |offset: i32, scale: f32, step: f32| (offset as f32 * scale).rem_euclid(step);
    let mut x = screen.left() + first(grid.offset.0, zoom.x, sx);
    while x < screen.right() {
        if x > screen.left() {
            painter.vline(x, screen.y_range(), stroke);
        }
        x += sx;
    }
    let mut y = screen.top() + first(grid.offset.1, zoom.y, sy);
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
    /// Sin la migaja de pan, si se ve.
    fn canvas_area(&self, rect: Rect, view: &ViewSettings) -> Rect {
        let mut area = self.under_rulers(rect, view);
        if self.crumbs_shown {
            area.min.y += CRUMBS_PX;
        }
        area
    }

    /// `rect` sin las reglas, si se muestran.
    fn under_rulers(&self, rect: Rect, view: &ViewSettings) -> Rect {
        if view.rulers {
            Rect::from_min_max(rect.min + Vec2::splat(self.ruler_px()), rect.max)
        } else {
            rect
        }
    }

    /// La franja de la migaja de pan: arriba del lienzo, debajo de la regla.
    fn crumbs_strip(&self, rect: Rect, view: &ViewSettings) -> Option<Rect> {
        let area = self.under_rulers(rect, view);
        self.crumbs_shown.then(|| {
            Rect::from_min_size(area.min, Vec2::new(area.width(), CRUMBS_PX)).intersect(rect)
        })
    }

    /// Guarda la vista del documento al entrar al modo símbolo y la restaura al salir.
    fn sync_symbol_view(&mut self, editor: &Editor) {
        if editor.in_symbol_mode() {
            self.symbol_return_view.get_or_insert((self.zoom, self.pan));
        } else if let Some((zoom, pan)) = self.symbol_return_view.take() {
            (self.zoom, self.pan) = (zoom, pan);
        }
    }

    /// La migaja de pan del modo símbolo: "Document > Inner > …" en una franja con el fondo de
    /// los paneles. Un clic en una parte vuelve a ese nivel.
    fn breadcrumb(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        view: &ViewSettings,
        editor: &mut Editor,
    ) {
        let Some(strip) = self.crumbs_strip(rect, view) else {
            return;
        };
        let (background, text) = match crate::pixel_ui::frame_skin(ui.ctx()) {
            Some(skin) => {
                let c = skin.colors();
                (c.panel, c.text)
            }
            None => (ui.visuals().panel_fill, ui.visuals().text_color()),
        };
        painter.rect_filled(strip, 0.0, background);
        // Tapa el lienzo: los clics en la franja no llegan a las herramientas.
        ui.interact(strip, ui.id().with("symbol_crumbs"), Sense::click());
        let font = egui::TextStyle::Body.resolve(ui.style());
        // La fuente pixel no tiene `›`: si no está, `>`.
        let separator = if crate::appearance::pixel_font_has('›') {
            " › "
        } else {
            " > "
        };
        let parts = editor.symbol_breadcrumb();
        let last = parts.len() - 1;
        let mut x = strip.left() + 4.0;
        let mut target = None;
        for (i, name) in parts.into_iter().enumerate() {
            if i > 0 {
                let galley = painter.layout_no_wrap(separator.into(), font.clone(), text);
                let at = Pos2::new(x, strip.center().y - galley.size().y / 2.0).round();
                x += galley.size().x;
                painter.galley(at, galley, text);
            }
            let galley = painter.layout_no_wrap(name.clone(), font.clone(), text);
            let at = Pos2::new(x, strip.center().y - galley.size().y / 2.0).round();
            let part = Rect::from_min_size(at, galley.size());
            x += galley.size().x;
            let response = ui.interact(part, ui.id().with(("symbol_crumb", i)), Sense::click());
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Link, i < last, &name));
            // Las de antes del nivel actual se pueden tocar: se subrayan al pasar.
            if i < last && response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                let y = part.bottom() - 1.0;
                painter.hline(part.x_range(), y, Stroke::new(1.0, text));
            }
            painter.galley(at, galley, text);
            if i < last && response.clicked() {
                target = Some(i);
            }
        }
        if let Some(depth) = target {
            editor.exit_symbol_to(depth);
            ui.ctx().request_repaint();
        }
    }

    /// El menú del "≡" de cada ventana de artboard (con Pixel UI): se abre al hacer clic, como un
    /// desplegable de la barra de menús.
    fn artboard_menus(&mut self, ui: &egui::Ui, rect: Rect, editor: &mut Editor) {
        if self.artboard_window.is_none() {
            return;
        }
        let ppp = ui.ctx().pixels_per_point();
        let menus: Vec<(ArtboardId, Rect)> = shown_artboards(&editor.document, editor)
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
        // En el modo símbolo la ventana es la del símbolo: duplicarlo, mandarlo a la biblioteca o
        // borrarlo (sin la confirmación de la biblioteca) no van desde acá.
        let symbol = editor.in_symbol_mode();
        if !symbol && ui.add(menus::plain("Duplicate")).clicked() {
            let name = format!("{} Copy", editor.artboard().name);
            editor.duplicate_artboard(name);
            ui.close();
        }
        if !symbol
            && ui
                .add_enabled(
                    editor.document.canvas_count() > 1,
                    menus::plain("Move to Library"),
                )
                .clicked()
        {
            editor.move_active_to_library();
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
        if symbol {
            return;
        }
        menus::separator(ui);
        let can_delete = editor.document.canvas_count() > 1;
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
            let shown = shown_artboards(&editor.document, editor);
            let (artboard, pixels) = shown.rev().find_map(|a| {
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
        shown_artboards(&editor.document, editor)
            .rev()
            .find_map(|artboard| {
                if !self.names_visible {
                    return None;
                }
                let galley = ui.painter().layout_no_wrap(
                    label_text(artboard),
                    FontId::proportional(LABEL_SIZE),
                    Color32::WHITE,
                );
                let bottom_left =
                    self.to_screen(rect, artboard.position) - Vec2::new(0.0, LABEL_GAP);
                let label = Rect::from_min_size(
                    bottom_left - Vec2::new(0.0, galley.size().y),
                    galley.size(),
                )
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
    zoom: Vec2,
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
        let center = b.handle_center(handle, HANDLE_PX / 2.0 / zoom.min_elem());
        let r = Rect::from_center_size(at(center), Vec2::splat(HANDLE_PX));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    }
}

/// Con Pixel UI, los ejes de simetría como en Aseprite: líneas de 1 punto en `color`, enteras
/// de borde a borde del artboard (`screen`). Las rectas llevan un handle de 5×5 (los de la
/// selección) en cada punta, adentro del artboard; las diagonales son una escalera de píxeles
/// que pasa por el centro `c`. Lo que tapan los artboards de delante (`covers`) no se ve.
fn draw_pixel_symmetry(
    painter: &egui::Painter,
    axes: cs_core::SymmetryAxes,
    c: Pos2,
    screen: Rect,
    color: Color32,
    covers: &[Rect],
) {
    let handle = crate::pixel_selection::HANDLE;
    let half = (handle / 2.0).floor();
    let screen = Rect::from_min_max(screen.min.round(), screen.max.round());
    let (x, y) = (c.x.floor(), c.y.floor());
    let mut shapes = Vec::new();
    let mut px = |r: Rect| {
        shapes.extend(
            rect_uncovered(r, covers)
                .into_iter()
                .map(|r| egui::Shape::rect_filled(r, 0.0, color)),
        );
    };
    let mut handles = Vec::new();
    if axes.vertical {
        let line = Rect::from_x_y_ranges(
            x..=x + 1.0,
            screen.top() + handle..=screen.bottom() - handle,
        );
        px(line);
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
        px(line);
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
                px(Rect::from_min_size(at, Vec2::splat(1.0)));
            }
        }
    }
    painter.extend(shapes);
    // Un handle que asoma debajo de otro artboard no se dibuja.
    for r in handles {
        if !covers.iter().any(|c| c.intersects(r)) {
            crate::pixel_selection::draw_handle(painter, r);
        }
    }
}

/// Los pedazos de `r` que no tapa ninguno de `covers`.
fn rect_uncovered(r: Rect, covers: &[Rect]) -> Vec<Rect> {
    let mut pieces = vec![r];
    for c in covers {
        pieces = pieces
            .into_iter()
            .flat_map(|p| {
                let i = p.intersect(*c);
                if i.width() <= 0.0 || i.height() <= 0.0 {
                    return vec![p];
                }
                // Arriba y abajo a todo lo ancho; a los lados, del alto de lo tapado.
                let sides = [
                    Rect::from_min_max(p.min, Pos2::new(p.right(), i.top())),
                    Rect::from_min_max(Pos2::new(p.left(), i.bottom()), p.max),
                    Rect::from_min_max(
                        Pos2::new(p.left(), i.top()),
                        Pos2::new(i.left(), i.bottom()),
                    ),
                    Rect::from_min_max(
                        Pos2::new(i.right(), i.top()),
                        Pos2::new(p.right(), i.bottom()),
                    ),
                ];
                sides
                    .into_iter()
                    .filter(|s| s.width() > 0.0 && s.height() > 0.0)
                    .collect()
            })
            .collect();
    }
    pieces
}

/// Los tramos del segmento `seg` que no tapa ninguno de `covers`.
fn segment_uncovered(seg: [Pos2; 2], covers: &[Rect]) -> Vec<[Pos2; 2]> {
    let [a, b] = seg;
    let d = b - a;
    // Lo tapado por cada uno, en fracciones del segmento (0 en `a`, 1 en `b`).
    let cut: Vec<egui::Rangef> = covers
        .iter()
        .filter_map(|c| {
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            for (p, dp, lo, hi) in [
                (a.x, d.x, c.left(), c.right()),
                (a.y, d.y, c.top(), c.bottom()),
            ] {
                if dp == 0.0 {
                    if p < lo || p > hi {
                        return None;
                    }
                } else {
                    let (u, v) = ((lo - p) / dp, (hi - p) / dp);
                    t0 = t0.max(u.min(v));
                    t1 = t1.min(u.max(v));
                }
            }
            (t1 > t0).then(|| egui::Rangef::new(t0, t1))
        })
        .collect();
    uncovered(egui::Rangef::new(0.0, 1.0), &cut)
        .into_iter()
        .map(|r| [a + d * r.min, a + d * r.max])
        .collect()
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
    zoom: Vec2,
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
    zoom: Vec2,
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
mod tests;
