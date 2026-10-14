use std::time::Duration;

use cs_core::Mask;
use eframe::egui::{
    self, Color32, ColorImage, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle,
    TextureOptions, Vec2,
};

use crate::editor::{Editor, ToolKind};
use crate::theme;

const ZOOM_LEVELS: [f32; 12] = [
    1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0,
];
/// Tamaño en pantalla de cada cuadro del damero de transparencia.
const CHECKER_PX: f32 = 8.0;
/// Velocidad de las "marching ants" (pasos por segundo).
const ANTS_SPEED: f64 = 6.0;

/// Vista del lienzo: zoom, paneo y la textura del sprite compuesto.
pub struct CanvasView {
    pub zoom: f32,
    pan: Vec2,
    texture: Option<TextureHandle>,
    checker: Option<TextureHandle>,
    rendered_revision: Option<u64>,
    /// Selección a dibujar (la del documento o la del gesto en curso).
    selection: Option<Mask>,
    fitted: bool,
    /// Píxel del sprite bajo el cursor, para la barra de estado.
    pub hover: Option<(i32, i32)>,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            zoom: 8.0,
            pan: Vec2::ZERO,
            texture: None,
            checker: None,
            rendered_revision: None,
            selection: None,
            fitted: false,
            hover: None,
        }
    }
}

impl CanvasView {
    /// Fuerza a recomponer y reajustar el zoom (p. ej. tras crear un sprite nuevo).
    pub fn reset(&mut self) {
        self.rendered_revision = None;
        self.fitted = false;
        self.pan = Vec2::ZERO;
    }

    fn origin(&self, rect: Rect, sprite_size: Vec2) -> Pos2 {
        rect.center() + self.pan - sprite_size * self.zoom / 2.0
    }

    fn fit(&mut self, rect: Rect, sprite_size: Vec2) {
        let max = (rect.size() * 0.85 / sprite_size).min_elem();
        self.zoom = ZOOM_LEVELS
            .iter()
            .copied()
            .rfind(|&z| z <= max)
            .unwrap_or(1.0);
        self.pan = Vec2::ZERO;
        self.fitted = true;
    }

    fn zoom_at(&mut self, rect: Rect, sprite_size: Vec2, anchor: Pos2, step_in: bool) {
        let i = ZOOM_LEVELS
            .iter()
            .position(|&z| z >= self.zoom)
            .unwrap_or(0);
        let next = if step_in {
            ZOOM_LEVELS[(i + 1).min(ZOOM_LEVELS.len() - 1)]
        } else {
            ZOOM_LEVELS[i.saturating_sub(1)]
        };
        let sprite_pos = (anchor - self.origin(rect, sprite_size)) / self.zoom;
        self.zoom = next;
        let new_origin = anchor - sprite_pos * next;
        self.pan = new_origin - rect.center() + sprite_size * next / 2.0;
    }

    /// Recompone si el documento cambió o hay un gesto en curso (vista previa).
    fn update_texture(&mut self, ctx: &egui::Context, editor: &Editor) {
        let painting = editor.is_painting();
        if !painting && self.rendered_revision == Some(editor.revision) && self.texture.is_some() {
            return;
        }
        let view = editor.frame_view();
        let bytes: Vec<u8> = view
            .pixels
            .iter()
            .flat_map(|c| [c.r, c.g, c.b, c.a])
            .collect();
        let size = [editor.sprite.width as usize, editor.sprite.height as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
        match &mut self.texture {
            Some(tex) => tex.set(image, TextureOptions::NEAREST),
            None => self.texture = Some(ctx.load_texture("sprite", image, TextureOptions::NEAREST)),
        }
        self.selection = view.selection;
        // Durante un gesto no marcamos como al día, para recomponer al terminar.
        self.rendered_revision = (!painting).then_some(editor.revision);
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
    ) {
        let sprite_size = Vec2::new(editor.sprite.width as f32, editor.sprite.height as f32);

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
                self.zoom_at(rect, sprite_size, pointer, scroll > 0.0);
            }
        }

        // Paneo con botón medio o espacio + arrastre.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if response.dragged_by(egui::PointerButton::Middle)
            || (space && response.dragged_by(egui::PointerButton::Primary))
        {
            self.pan += response.drag_delta();
        }

        let origin = self.origin(rect, sprite_size);
        let zoom = self.zoom;
        let to_sprite = |p: Pos2| {
            let s = (p - origin) / zoom;
            (s.x.floor() as i32, s.y.floor() as i32)
        };
        self.hover = response.hover_pos().map(to_sprite);

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
        if !space
            && response.hovered()
            && !editor.is_painting()
            && (pressed_primary || pressed_secondary)
            && let Some(p) = pointer
        {
            editor.pointer_press(to_sprite(p), pressed_secondary, modifiers.alt);
        }
        if editor.is_painting() {
            if any_down {
                if let Some(p) = pointer {
                    editor.pointer_drag(to_sprite(p));
                }
            } else {
                editor.pointer_release();
            }
        }

        let cursor = if space {
            Some(egui::CursorIcon::Grab)
        } else if !response.hovered() {
            None
        } else if editor.active_tool() == ToolKind::Move {
            Some(egui::CursorIcon::Move)
        } else {
            Some(egui::CursorIcon::Crosshair)
        };
        if let Some(c) = cursor {
            ui.ctx().set_cursor_icon(c);
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, editor: &mut Editor) {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        let ctx = ui.ctx().clone();
        let sprite_size = Vec2::new(editor.sprite.width as f32, editor.sprite.height as f32);
        if !self.fitted && rect.width() > 0.0 {
            self.fit(rect, sprite_size);
        }
        self.handle_input(ui, &response, rect, editor);
        self.update_texture(&ctx, editor);

        let origin = self.origin(rect, sprite_size);
        let checker = self.checker(&ctx);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::CANVAS_BG);

        let canvas_rect = Rect::from_min_size(origin, sprite_size * self.zoom);
        let uv = Rect::from_min_max(
            Pos2::ZERO,
            (canvas_rect.size() / (CHECKER_PX * 2.0)).to_pos2(),
        );
        painter.image(checker, canvas_rect, uv, Color32::WHITE);
        if let Some(tex) = &self.texture {
            let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(tex.id(), canvas_rect, full_uv, Color32::WHITE);
        }

        if self.zoom >= 8.0 {
            let stroke = Stroke::new(1.0, theme::GRID);
            for x in 1..editor.sprite.width {
                let px = canvas_rect.left() + x as f32 * self.zoom;
                painter.vline(px, canvas_rect.y_range(), stroke);
            }
            for y in 1..editor.sprite.height {
                let py = canvas_rect.top() + y as f32 * self.zoom;
                painter.hline(canvas_rect.x_range(), py, stroke);
            }
        }
        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0, theme::CANVAS_BORDER),
            StrokeKind::Outside,
        );

        if let Some(mask) = &self.selection {
            let phase = (ctx.input(|i| i.time) * ANTS_SPEED) as i32;
            draw_marching_ants(&painter, mask, origin, self.zoom, phase);
            ctx.request_repaint_after(Duration::from_millis((1000.0 / ANTS_SPEED) as u64));
        }

        // Contorno del pincel bajo el cursor.
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        if let Some((hx, hy)) = self.hover
            && !space
            && response.hovered()
        {
            let size = if editor.active_tool().uses_brush() {
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
