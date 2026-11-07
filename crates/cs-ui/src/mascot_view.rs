//! Chimpy en la app (ver `mascot.rs`, la lógica): cada frame le cuenta lo que pasa y, si está a
//! la vista, lo dibuja colgado del borde de arriba del lienzo (debajo de la regla), con el globo
//! de los tooltips (`tooltip.bubble`) a la derecha y los botones del kit adentro. Solo con Pixel
//! UI (el chango que lo despierta es de Pixel UI).

use eframe::egui::{self, Id, Rect, Sense, Shape, TextureHandle, pos2, vec2};

use crate::mascot::{self, Art, DocFacts, DocKind, Snapshot, Suggestion};
use crate::skin::{Skin, WidgetState};
use crate::{ChimpApp, editor::Editor, kit, pixel_rulers, pixel_ui, tab_bar};

/// De la esquina del lienzo (debajo de la regla y de su borde izquierdo) a Chimpy.
const LEFT: f32 = 6.0;
/// La fila de la cara de Chimpy en su arte: ahí apunta la cola del globo.
const FACE_ROW: f32 = 14.0;
/// Del borde derecho del arte de Chimpy a la cola del globo.
const GAP: f32 = 1.0;
/// Lo más ancho del texto del globo (después parte en líneas).
const TEXT_WIDTH: f32 = 170.0;
/// El alto de cada línea del texto del globo (las mayúsculas de la fuente chica son de 5).
const LINE_HEIGHT: f32 = 10.0;
/// Entre el texto y los botones, y entre un botón y otro.
const BUTTON_GAP: f32 = 4.0;
/// En Home (sin lienzo), de la barra de pestañas a Chimpy.
const HOME_TOP: f32 = 4.0;

/// Lo de Chimpy que vive en la app y no se guarda: sus texturas y dónde quedó en el último
/// frame (para saber si el puntero está encima).
#[derive(Default)]
pub(crate) struct MascotView {
    /// Se cargan la primera vez que se lo dibuja (`Some(None)` si el arte no se pudo leer).
    textures: Option<Option<(Art, Vec<TextureHandle>)>>,
    /// Chimpy y su globo en el último frame.
    rects: Vec<Rect>,
    /// El chango que corre arriba a la derecha (sus clics no cuentan como "ocupado").
    pub(crate) runner: Option<Rect>,
}

/// Lo que se ve del documento activo.
pub(crate) fn doc_facts(editor: &Editor, tab: usize) -> DocFacts {
    let kind = if editor.is_font() {
        DocKind::Font
    } else if editor.is_model() {
        DocKind::Model
    } else {
        DocKind::Sprite
    };
    DocFacts {
        place: (tab, u64::from(editor.active.0)),
        kind,
        frames: editor.sprite().frames.len(),
        onion: editor.onion_enabled,
        tool: editor.tool,
        selection: editor.selection.is_some(),
        symmetry: editor.symmetry().axes.any(),
        mirror_selection: editor.options.mirror_selection,
        artboards: editor.document.canvas_artboards().count(),
    }
}

/// Lo que se tocó en el globo o en Chimpy.
enum Touch {
    Button(usize),
    Chimpy,
}

impl ChimpApp {
    /// Chimpy en este frame: `canvas` es el rect del lienzo (con sus reglas), `None` en Home;
    /// `screen`, la ventana.
    pub(crate) fn update_mascot(
        &mut self,
        ctx: &egui::Context,
        canvas: Option<Rect>,
        screen: Rect,
        modal: bool,
    ) {
        let Some(skin) = pixel_ui::frame_skin(ctx) else {
            self.mascot_view.rects.clear();
            return;
        };
        let now = ctx.input(|i| i.time);
        let doc = self
            .has_doc()
            .then(|| doc_facts(&self.doc.editor, self.tabs.active().unwrap_or(0)));
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let over = |r: &Rect| pointer.is_some_and(|p| r.contains(p));
        let hovered = self.mascot_view.rects.iter().any(over);
        let on_runner = self.mascot_view.runner.as_ref().is_some_and(over);
        let (down, input, key_pressed) = ctx.input(|i| {
            (
                i.pointer.any_down(),
                !i.events.is_empty(),
                i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Key { pressed: true, .. })),
            )
        });
        let painting = self.has_doc() && self.doc.editor.is_painting();
        let snap = Snapshot {
            doc,
            busy: modal
                || ctx.egui_wants_keyboard_input()
                || painting
                || (down && !hovered && !on_runner),
            input,
            key_pressed,
            hovered,
        };
        self.mascot.observe(now, &snap, &self.keymap);

        let Some(pose) = self.mascot.pose(now) else {
            self.mascot_view.rects.clear();
            if self.mascot.settings.enabled && ctx.input(|i| i.focused) {
                // Los consejos automáticos llegan con la app quieta (y no con otra ventana
                // al frente: sin el foco no se repinta, como el mono que corre).
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
            return;
        };
        let textures = self.mascot_view.textures.get_or_insert_with(|| {
            let art = Art::decode(mascot::ART)?;
            let frames = art
                .frames
                .iter()
                .enumerate()
                .map(|(i, pixels)| {
                    let rgba: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
                    let image = egui::ColorImage::from_rgba_unmultiplied(art.size, &rgba);
                    ctx.load_texture(format!("mascot.{i}"), image, egui::TextureOptions::NEAREST)
                })
                .collect();
            Some((art, frames))
        });
        let Some((art, frames)) = textures.as_ref() else {
            return;
        };
        // Debajo de la regla de arriba y a la derecha de la de la izquierda.
        let clip = match canvas {
            Some(rect) => {
                let ruler = if self.view.rulers {
                    pixel_rulers::RULER_PX
                } else {
                    0.0
                };
                Rect::from_min_max(rect.min + vec2(ruler, ruler), rect.max)
            }
            None => screen
                .with_min_y(screen.top() + tab_bar::MENU_HEIGHT + tab_bar::TABS_HEIGHT + HOME_TOP),
        };
        let size = vec2(art.size[0] as f32, art.size[1] as f32);
        // En puntos enteros: el arte va a la grilla de la UI.
        let chimp = Rect::from_min_size(
            pos2(clip.left() + LEFT, clip.top() - (1.0 - pose.drop) * size.y).round(),
            size,
        );
        let texture = frames[art.frame(pose.anim, pose.clock).min(frames.len() - 1)].id();
        let tip = self.mascot.tip().cloned();
        let touch = egui::Area::new(Id::new("chimpy"))
            .order(egui::Order::Middle)
            .fixed_pos(clip.min)
            .constrain(false)
            .show(ctx, |ui| {
                let ppp = ui.ctx().pixels_per_point();
                let seen = chimp.intersect(clip);
                let mut rects = vec![seen];
                ui.painter().with_clip_rect(clip).image(
                    texture,
                    chimp,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
                let mut touch = None;
                let response = ui.interact(seen, Id::new("chimpy_body"), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, mascot::NAME)
                });
                if response.clicked() {
                    touch = Some(Touch::Chimpy);
                }
                if pose.bubble
                    && let Some(tip) = &tip
                    && let Some(piece) = skin.piece(pixel_ui::BUBBLE_PIECE, WidgetState::Normal)
                {
                    let tail = pos2(chimp.right() + GAP, chimp.top() + FACE_ROW);
                    let (rect, shapes, buttons) = bubble(ui, &skin, piece, tail, tip, ppp);
                    ui.painter().extend(shapes);
                    let label = ui.allocate_rect(rect, Sense::hover());
                    label.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &tip.text)
                    });
                    for (i, (at, text)) in buttons.into_iter().enumerate() {
                        let id = Id::new(("chimpy_button", i));
                        if kit::button::button_at(ui, &skin, at, id, text).clicked() {
                            touch = Some(Touch::Button(i));
                        }
                    }
                    rects.push(rect);
                }
                (rects, touch)
            })
            .inner;
        let (rects, touch) = touch;
        self.mascot_view.rects = rects;
        match touch {
            Some(Touch::Chimpy) => self.mascot.ask(now, &self.keymap),
            Some(Touch::Button(i)) => {
                if let Some(suggestion) = self.mascot.answer(i, now) {
                    self.accept_suggestion(ctx, suggestion);
                }
            }
            None => {}
        }
        // Mientras baja o sube, cada frame; colgado, lo que pide la animación.
        if pose.drop < 1.0 || !pose.bubble {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Hace lo que Chimpy sugirió.
    pub(crate) fn accept_suggestion(&mut self, ctx: &egui::Context, suggestion: Suggestion) {
        match suggestion {
            Suggestion::Run(action) => {
                if !action.needs_document() || self.has_doc() {
                    self.run_action(ctx, action);
                }
            }
            Suggestion::OnionSkin if self.has_doc() => self.doc.editor.onion_enabled = true,
            Suggestion::MirrorSelection if self.has_doc() => {
                self.doc.editor.options.mirror_selection = true;
            }
            Suggestion::PlayInPlace if self.has_doc() => {
                let editor = &mut self.doc.editor;
                let id = editor.active;
                if editor.can_play_in_place(id) && !editor.is_playing_in_place(id) {
                    editor.toggle_play_in_place(id);
                }
            }
            Suggestion::OnionSkin | Suggestion::MirrorSelection | Suggestion::PlayInPlace => {}
        }
    }
}

/// El globo con `tip` y la punta de la cola en `tail`: su rect, lo que se pinta (la pieza y el
/// texto) y dónde va cada botón (con su texto).
fn bubble(
    ui: &egui::Ui,
    skin: &Skin,
    piece: &crate::skin::Piece,
    tail: egui::Pos2,
    tip: &mascot::Tip,
    ppp: f32,
) -> (Rect, Vec<Shape>, Vec<(Rect, &'static str)>) {
    use pixel_ui::{BUBBLE_PADDING, BUBBLE_SHADOW, BUBBLE_TAIL, BUBBLE_TIP_ROW};
    let color = skin.colors().text;
    let font = egui::TextStyle::Small.resolve(ui.style());
    // Las líneas más juntas que las de la fuente: el texto es largo y se lee como un párrafo.
    let format = egui::TextFormat {
        line_height: Some(LINE_HEIGHT),
        ..egui::TextFormat::simple(font, color)
    };
    let mut job = egui::text::LayoutJob::single_section(tip.text.clone(), format);
    job.wrap.max_width = TEXT_WIDTH;
    let galley = ui.painter().layout_job(job);
    let labels = tip.buttons();
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| kit::button::width(ui, l).round())
        .collect();
    let row = widths.iter().sum::<f32>() + BUTTON_GAP * (widths.len().saturating_sub(1)) as f32;
    let text_height = pixel_ui::caps_block_height(&galley);
    let body = vec2(
        2.0 * BUBBLE_PADDING + galley.size().x.max(row),
        2.0 * BUBBLE_PADDING + text_height + BUTTON_GAP + kit::button::HEIGHT + BUBBLE_SHADOW,
    )
    .ceil();
    let rect = Rect::from_min_size(
        pos2(tail.x, tail.y - BUBBLE_TIP_ROW).round(),
        vec2(BUBBLE_TAIL + body.x, body.y),
    );
    let left = rect.left() + BUBBLE_TAIL + BUBBLE_PADDING;
    let top = rect.top() + BUBBLE_PADDING;
    let at = tab_bar::text_pos(&galley, left, top, ppp);
    let mut shapes = pixel_ui::piece_shapes(piece, rect, ppp);
    shapes.push(Shape::galley(at, galley, color));
    let mut x = left;
    let y = (top + text_height + BUTTON_GAP).round();
    let buttons = labels
        .into_iter()
        .zip(widths)
        .map(|(label, w)| {
            let at = Rect::from_min_size(pos2(x, y), vec2(w, kit::button::HEIGHT));
            x += w + BUTTON_GAP;
            (at, label)
        })
        .collect();
    (rect, shapes, buttons)
}

#[cfg(test)]
#[path = "mascot_view_tests.rs"]
mod tests;
