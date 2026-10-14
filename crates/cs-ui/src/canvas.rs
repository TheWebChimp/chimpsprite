use std::collections::HashMap;
use std::time::Duration;

use cs_core::{Artboard, ArtboardId, Mask};
use cs_tools::{ArtboardHit, Edges, hit_test};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Pos2, Rect, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Vec2,
};

use crate::canvas_size::{CanvasSizeDialog, GuideEdge};
use crate::editor::{Editor, ToolKind};
use crate::theme;

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
    /// Guía del diálogo Canvas Size que se está arrastrando.
    dragging_guide: Option<GuideEdge>,
    /// El puntero está sobre la etiqueta de un artboard.
    over_label: bool,
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
            dragging_guide: None,
            over_label: false,
        }
    }
}

impl CanvasView {
    /// Fuerza a recomponer y reajustar el zoom (p. ej. tras crear un documento nuevo).
    pub fn reset(&mut self) {
        self.rendered_revision = None;
        self.fitted = false;
        self.textures.clear();
    }

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

    fn checker(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.checker
            .get_or_insert_with(|| {
                let (a, b) = (theme::CHECKER_LIGHT, theme::CHECKER_DARK);
                let image = ColorImage::new([2, 2], vec![a, b, b, a]);
                ctx.load_texture("checker", image, TextureOptions::NEAREST_REPEAT)
            })
            .id()
    }

    fn handle_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        dialog: Option<&mut CanvasSizeDialog>,
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
        let over_label = pointer.and_then(|p| self.label_at(ui, rect, editor, p));
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
        let guide_cursor = self.dragging_guide.or(over_guide).map(|edge| {
            if edge.is_vertical() {
                egui::CursorIcon::ResizeHorizontal
            } else {
                egui::CursorIcon::ResizeVertical
            }
        });

        if !modal
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
        if editor.is_painting() {
            if any_down {
                if let Some(p) = pointer {
                    editor.pointer_drag(self.to_world(rect, p));
                }
            } else {
                editor.pointer_release();
            }
        }

        let cursor = if space {
            Some(egui::CursorIcon::Grab)
        } else if guide_cursor.is_some() {
            guide_cursor
        } else if modal {
            response.hovered().then_some(egui::CursorIcon::Default)
        } else if !response.hovered() {
            None
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
        } else if editor.active_tool() == ToolKind::Move {
            Some(egui::CursorIcon::Move)
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
    ) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        if !self.fitted && rect.width() > 0.0 {
            self.fit(rect, editor);
        }
        self.handle_input(ui, &response, rect, editor, dialog.as_deref_mut());
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
                let pixels = cs_render::render_frame(sprite, frame_for(editor, artboard.id));
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
            if self.zoom >= 8.0 {
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
            let active = artboard.id == editor.active;
            let (border, label_color) = if active {
                (Stroke::new(2.0, theme::ACCENT), theme::ACCENT)
            } else {
                (Stroke::new(1.0, theme::CANVAS_BORDER), theme::LABEL)
            };
            painter.rect_stroke(screen, 0.0, border, StrokeKind::Outside);
            painter.text(
                screen.left_top() - Vec2::new(0.0, LABEL_GAP),
                Align2::LEFT_BOTTOM,
                label_text(artboard),
                FontId::proportional(LABEL_SIZE),
                label_color,
            );
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
        if let (Some(mask), Some(origin)) = (&view.selection, active_origin) {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_marching_ants(&painter, mask, origin, self.zoom, phase);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }

        // Contorno del pincel bajo el cursor.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let tool = editor.active_tool();
        if let (Some((hx, hy)), Some(origin)) = (self.hover, active_origin)
            && !modal
            && !self.over_label
            && !editor.artboard_gesture().is_active()
            && !space
            && response.hovered()
            && tool != ToolKind::Artboard
        {
            let size = if tool.uses_brush() {
                editor.options.brush_size
            } else {
                1
            };
            let offset = -((size as i32 - 1) / 2);
            let r = Rect::from_min_size(
                origin + Vec2::new((hx + offset) as f32, (hy + offset) as f32) * self.zoom,
                Vec2::splat(size as f32 * self.zoom),
            );
            painter.rect_stroke(
                r,
                0.0,
                Stroke::new(1.0, theme::BRUSH_OUTLINE),
                StrokeKind::Inside,
            );
        }
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

/// Frame a mostrar de cada artboard: el activo usa el frame actual; los demás, el primero.
fn frame_for(editor: &Editor, id: ArtboardId) -> usize {
    if id == editor.active { editor.frame } else { 0 }
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
