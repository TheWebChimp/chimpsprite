//! La ventanita de preview (como la de Aseprite): el artboard activo en el frame que se ve,
//! centrado sobre el damero. De fábrica, al zoom entero más grande que entra; la rueda lo cambia
//! de a uno, arrastrando se mueve el dibujo y con doble clic vuelve a como estaba (o con centrar).
//! Su play anima el dibujo por su cuenta (sin mover el timeline).
//!
//! Es un panel del workspace (fase D): flotando solo en su ventana (de fábrica, en la esquina de
//! abajo a la derecha, como antes), con Pixel UI su ventana es la del diseño
//! ([`Preview::window_contents`] y [`Preview::window_chrome`]: play y centrar en la barra de
//! título y la agarradera de la esquina que la agranda o la achica); acoplado (o con otras
//! pestañas), play y centrar van arriba de lo de adentro y el dibujo ocupa lo que queda
//! ([`Preview::contents`]).

use eframe::egui::{
    self, Color32, ColorImage, CursorIcon, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui,
    Vec2, vec2,
};
use eframe::emath::GuiRounding as _;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::theme;
use cs_core::{AnimDirection, ArtboardId, Playback};

/// El lugar del dibujo, de fábrica.
pub const VIEW: Vec2 = vec2(192.0, 144.0);
/// Lo más chico y lo más grande que se puede hacer con la agarradera.
pub(crate) const MIN_VIEW: Vec2 = vec2(96.0, 64.0);
const MAX_VIEW: Vec2 = vec2(720.0, 540.0);
/// La agarradera: un cuadro de este lado en la esquina de abajo a la derecha de la vista.
const GRIP: f32 = 9.0;
/// Los cuadros del damero, en puntos.
const CHECKER_CELL: f32 = 4.0;
const MAX_ZOOM: f32 = 32.0;
/// De la esquina de abajo a la derecha de la pantalla: ahí va su ventana de fábrica.
pub(crate) const OFFSET: Vec2 = vec2(-48.0, -48.0);
/// Entre el play y la vista.
const ROW_GAP: f32 = 3.0;
/// El alto de la fila de play y centrar acoplado (con Pixel UI, la zona de sus íconos).
const CONTROLS_HEIGHT: f32 = 9.0;
/// Lo más bajo de lo de adentro acoplado: la fila de controles y la vista más chica.
pub(crate) const MIN_HEIGHT: f32 = CONTROLS_HEIGHT + ROW_GAP + MIN_VIEW.y;

/// La reproducción propia del preview, como la del timeline: el tag del frame donde empezó (o
/// todos los frames), con la duración de cada frame.
struct Play {
    artboard: ArtboardId,
    playback: Playback,
    range: (usize, usize),
    direction: AnimDirection,
    clock: f64,
}

pub struct Preview {
    /// La textura propia: la de un frame que no es el del lienzo (reproduciendo) o cuando el
    /// lienzo muestra otra cosa (papel cebolla, máscara).
    texture: Option<TextureHandle>,
    /// Lo que se ve este frame: la textura del lienzo o la propia, y su tamaño.
    shown: Option<(egui::TextureId, Vec2)>,
    checker: Option<TextureHandle>,
    /// Lo que tiene la textura: el artboard, su frame y la revisión del documento.
    rendered: Option<(ArtboardId, usize, u64)>,
    /// El zoom elegido con la rueda; `None`, el que entra.
    zoom: Option<f32>,
    /// Cuánto se movió el dibujo arrastrando, en puntos.
    pan: Vec2,
    /// El tamaño de la vista.
    size: Vec2,
    play: Option<Play>,
    /// La textura del lienzo de este frame (ver `contents`).
    canvas: Option<(egui::TextureId, Vec2)>,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            texture: None,
            shown: None,
            checker: None,
            rendered: None,
            zoom: None,
            pan: Vec2::ZERO,
            size: VIEW,
            play: None,
            canvas: None,
        }
    }
}

impl Preview {
    /// Lo que mide su ventana del diseño (con Pixel UI, solo en su ventana: ver
    /// [`Self::window_contents`]): el diálogo con la vista adentro.
    pub(crate) fn window_size(&self, ui: &Ui, skin: &Skin) -> Vec2 {
        crate::pixel_ui::dialog_size(ui, skin, crate::dock::PREVIEW, self.size)
    }

    /// El marco y la barra de su ventana del diseño en `rect` (con Pixel UI, solo en su ventana),
    /// después de [`Self::window_contents`]: el marco va en `background` (el lugar que se dejó
    /// debajo de lo de adentro, con el pintor de su capa) y en la barra, sin botón (como en el
    /// diseño), play o pausa (si hay animación) y centrar, y la ○ de cerrar. `ui` es de la capa de
    /// la ventana, encima de lo demás de ella. Devuelve si se cerró (y para la reproducción).
    pub(crate) fn window_chrome(
        &mut self,
        ui: &mut Ui,
        editor: &Editor,
        skin: &Skin,
        rect: Rect,
        background: (&egui::Painter, egui::layers::ShapeIdx),
    ) -> bool {
        let chrome = crate::pixel_ui::dialog_chrome(
            ui,
            skin,
            egui::Id::new("preview"),
            rect,
            crate::dock::PREVIEW,
            true,
            false,
        );
        background
            .0
            .set(background.1, egui::Shape::Vec(chrome.shapes));
        let animated = editor.sprite().frames.len() > 1;
        let left = chrome.bar.left() + BAR_LEFT;
        let (play, center) = bar_icons(ui, skin, chrome.bar, left, animated, self.playing());
        self.controls(editor, play, center);
        if chrome.closed {
            self.stop();
        }
        chrome.closed
    }

    /// Lo que piden sus controles: prender o apagar la reproducción y centrar.
    fn controls(&mut self, editor: &Editor, play: bool, center: bool) {
        if play {
            self.toggle_play(editor);
        }
        if center {
            self.zoom = None;
            self.pan = Vec2::ZERO;
        }
    }

    /// Para la reproducción (al cerrarlo o apagarlo).
    pub(crate) fn stop(&mut self) {
        self.play = None;
    }

    /// Si se ve como entró: al zoom que entra y sin mover.
    #[cfg(test)]
    pub(crate) fn centered(&self) -> bool {
        self.zoom.is_none() && self.pan == Vec2::ZERO
    }

    /// Reproduciendo (un tag con repeat que ya terminó sus vueltas se queda quieto en su último
    /// frame, pero ya no cuenta).
    pub fn playing(&self) -> bool {
        self.play
            .as_ref()
            .is_some_and(|p| p.playback.remaining != Some(0))
    }

    /// Prende o apaga la reproducción del artboard activo.
    pub(crate) fn toggle_play(&mut self, editor: &Editor) {
        // Si ya había terminado sus vueltas, vuelve a empezar.
        if self
            .play
            .take()
            .is_some_and(|p| p.playback.remaining != Some(0))
        {
            return;
        }
        let sprite = editor.sprite();
        let frame = editor.frame;
        let (range, direction, repeat) = match sprite.tag_at(frame) {
            Some(tag) => ((tag.from, tag.to), tag.direction, tag.repeat),
            None => ((0, sprite.frames.len() - 1), AnimDirection::Forward, 0),
        };
        self.play = Some(Play {
            artboard: editor.active,
            playback: Playback::new(frame, direction).repeating(repeat),
            range,
            direction,
            clock: 0.0,
        });
    }

    /// Avanza la reproducción `dt_ms` milisegundos. Si cambió el artboard activo, se para.
    fn tick(&mut self, editor: &Editor, dt_ms: f64) {
        let Some(play) = &mut self.play else {
            return;
        };
        if play.artboard != editor.active {
            self.play = None;
            return;
        }
        let frames = &editor.sprite().frames;
        play.clock += dt_ms;
        for _ in 0..frames.len() * 4 {
            let Some(frame) = frames.get(play.playback.frame) else {
                break;
            };
            let duration = frame.duration_ms.max(1) as f64;
            if play.clock < duration {
                break;
            }
            play.clock -= duration;
            // Un tag con repeat se queda quieto en su último frame al terminar sus vueltas.
            if !play.playback.step(play.range, play.direction) {
                play.clock = 0.0;
                break;
            }
        }
        play.clock = play.clock.min(1000.0);
    }

    /// El frame que se ve: el de la reproducción propia o el del editor.
    fn frame(&self, editor: &Editor) -> usize {
        let count = editor.sprite().frames.len();
        match &self.play {
            Some(play) if play.playback.frame < count => play.playback.frame,
            _ => editor.frame_of(editor.active),
        }
    }

    /// Lo que se ve: la textura del lienzo si muestra este mismo frame; si no, la propia,
    /// recompuesta si cambió el artboard, el frame o el documento (o hay un trazo).
    fn update(&mut self, ctx: &egui::Context, editor: &Editor) {
        let frame = self.frame(editor);
        if frame == editor.frame_of(editor.active)
            && let Some(canvas) = self.canvas
        {
            self.shown = Some(canvas);
            // La propia ya no hace falta.
            self.texture = None;
            self.rendered = None;
            return;
        }
        let view = editor.view();
        let document = view.document.as_ref();
        let Some(artboard) = document.artboard(editor.active) else {
            return;
        };
        let key = (artboard.id, frame, editor.revision);
        if let Some(texture) = &self.texture
            && self.rendered == Some(key)
            && !editor.is_painting()
        {
            self.shown = Some((texture.id(), texture.size_vec2()));
            return;
        }
        let sprite = &*cs_render::materialize(document, &artboard.sprite, frame..frame + 1);
        let pixels = cs_render::render_frame(sprite, frame);
        let bytes: Vec<u8> = pixels.iter().flat_map(|c| [c.r, c.g, c.b, c.a]).collect();
        let size = [sprite.width as usize, sprite.height as usize];
        let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
        match &mut self.texture {
            Some(tex) => tex.set(image, TextureOptions::NEAREST),
            None => {
                self.texture = Some(ctx.load_texture("preview", image, TextureOptions::NEAREST));
            }
        }
        self.rendered = Some(key);
        self.shown = self.texture.as_ref().map(|t| (t.id(), t.size_vec2()));
    }

    /// Lo de adentro de su ventana del diseño (con Pixel UI, solo en su ventana): la vista, del
    /// tamaño elegido con la agarradera (play y centrar van en la barra de título: ver
    /// [`Self::window_chrome`]). `canvas`: la textura del lienzo con el frame del editor tal cual,
    /// si la hay (se usa en vez de componer otra). `ui` recorta en el rect de la ventana (la
    /// agarradera va contra su borde).
    pub(crate) fn window_contents(
        &mut self,
        ui: &mut Ui,
        editor: &Editor,
        skin: &Skin,
        canvas: Option<(egui::TextureId, Vec2)>,
    ) {
        self.canvas = canvas;
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        self.view(ui, editor, Some(skin), self.size, true);
    }

    /// Lo de adentro como panel (acoplado, o en una ventana con pestañas; sin Pixel UI, también
    /// solo en su ventana): arriba play o pausa y centrar (con Pixel UI, sus íconos sin botón,
    /// como en la barra de su ventana; sin él, botones de egui) y debajo la vista, de todo lo que
    /// queda (el tamaño lo pone su grupo). `canvas`, como en [`Self::window_contents`].
    pub fn contents(
        &mut self,
        ui: &mut Ui,
        editor: &Editor,
        skin: Option<&Skin>,
        canvas: Option<(egui::TextureId, Vec2)>,
    ) {
        self.canvas = canvas;
        ui.spacing_mut().item_spacing.y = ROW_GAP;
        let playing = self.playing();
        let (play, center) = match skin {
            Some(skin) => {
                let width = ui.available_width();
                let (row, _) = ui.allocate_exact_size(vec2(width, CONTROLS_HEIGHT), Sense::hover());
                let animated = editor.sprite().frames.len() > 1;
                // El primer ícono a la orilla (su zona de clic, 2 más ancha, empieza en ella).
                bar_icons(ui, skin, row, row.left() + 2.0, animated, playing)
            }
            None => {
                ui.horizontal(|ui| {
                    let label = if playing { "Pause" } else { "Play" };
                    let play = ui.selectable_label(playing, label).clicked();
                    (play, ui.button("Center").clicked())
                })
                .inner
            }
        };
        self.controls(editor, play, center);
        let size = ui.available_size().floor().max(Vec2::splat(1.0));
        self.view(ui, editor, skin, size, false);
    }

    /// La vista de `size`: el dibujo sobre el damero (la rueda, el arrastre y el doble clic) y, si
    /// `grip`, la agarradera de la esquina.
    fn view(&mut self, ui: &mut Ui, editor: &Editor, skin: Option<&Skin>, size: Vec2, grip: bool) {
        if self.playing() {
            let dt_ms = ui.input(|i| i.stable_dt).min(0.1) as f64 * 1000.0;
            self.tick(editor, dt_ms);
            ui.ctx().request_repaint();
        }
        self.update(ui.ctx(), editor);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let ppp = ui.ctx().pixels_per_point();
        let painter = ui.painter_at(rect);
        let background = skin.map_or(theme::CANVAS_BG, |s| s.colors().canvas);
        painter.rect_filled(rect, 0.0, background);
        if grip {
            self.grip(ui, rect, skin);
        }
        let Some((texture, texture_size)) = self.shown else {
            return;
        };
        if response.double_clicked() {
            self.zoom = None;
            self.pan = Vec2::ZERO;
        } else if response.dragged() {
            self.pan += response.drag_delta();
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }
        let mut zoom = self
            .zoom
            .unwrap_or_else(|| fit_zoom(texture_size, rect.size()));
        // La rueda: un paso por movimiento.
        if response.hovered() {
            let steps: Vec<bool> = ui.input(|i| {
                i.raw
                    .events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::MouseWheel { delta, .. } if delta.y != 0.0 => {
                            Some(delta.y > 0.0)
                        }
                        _ => None,
                    })
                    .collect()
            });
            for closer in steps {
                let next = step_zoom(zoom, closer);
                // Lo que estaba en el centro de la vista sigue ahí.
                self.pan *= next / zoom;
                zoom = next;
                self.zoom = Some(zoom);
            }
        }
        let shown = texture_size * zoom;
        // En puntos enteros: los píxeles del dibujo caen justo en los de la pantalla.
        let min = (rect.center() - shown / 2.0 + self.pan).round();
        let image = Rect::from_min_size(min, shown).round_to_pixels(ppp);
        let checker = self.checker(ui.ctx());
        let uv = Rect::from_min_max(Pos2::ZERO, (shown / (CHECKER_CELL * 2.0)).to_pos2());
        painter.image(checker, image, uv, Color32::WHITE);
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(texture, image, full, Color32::WHITE);
        // La agarradera va encima del dibujo: la del diseño o, sin ella, rayitas.
        if !grip {
            return;
        }
        match skin.filter(|s| {
            s.piece(GRIP_PIECE, crate::skin::WidgetState::Normal)
                .is_some()
        }) {
            // En la esquina de la ventana, contra su borde (como en el diseño): pasando el
            // margen de adentro.
            Some(skin) => {
                let at = grip_corner(rect, Some(skin)) - grip_size(Some(skin));
                ui.painter()
                    .extend(crate::pixel_draw::piece_at(skin, GRIP_PIECE, at, ppp));
            }
            None => painter.extend(grip_shapes(rect, skin, ppp)),
        }
    }

    /// La agarradera de la esquina: arrastrándola cambia el tamaño de la vista.
    fn grip(&mut self, ui: &mut Ui, view: Rect, skin: Option<&Skin>) {
        let corner = grip_corner(view, skin);
        let rect = Rect::from_min_max(corner - grip_size(skin), corner);
        let response = ui.interact(rect, ui.id().with("preview_grip"), Sense::drag());
        if response.hovered() || response.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
        }
        if response.dragged() {
            self.size = (self.size + response.drag_delta())
                .clamp(MIN_VIEW, MAX_VIEW)
                .round();
        }
    }

    fn checker(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.checker
            .get_or_insert_with(|| {
                let (a, b) = (theme::CHECKER_LIGHT, theme::CHECKER_DARK);
                let image = ColorImage::new([2, 2], vec![a, b, b, a]);
                ctx.load_texture("preview_checker", image, TextureOptions::NEAREST_REPEAT)
            })
            .id()
    }
}

/// Las piezas del diseño: el play y centrar de la barra de título, y la agarradera de la
/// esquina.
const PLAY_PIECE: &str = "preview.play";
const PAUSE_PIECE: &str = "preview.pause";
const CENTER_PIECE: &str = "preview.center";
const GRIP_PIECE: &str = "preview.grip";
/// Del borde izquierdo de la barra al primer ícono, y entre uno y otro.
const BAR_LEFT: f32 = 6.0;
const BAR_GAP: f32 = 5.0;

/// Del contenido de la ventana a su borde de afuera, a la derecha y abajo (la línea oscura incluida). La agarradera del
/// diseño los trae (y la esquina de la ventana, sin redondear como la del marco).
const WINDOW_EDGE: f32 = 3.0;

/// La esquina de abajo a la derecha de la agarradera: con la del diseño, la de afuera de la
/// ventana (pasando el margen de adentro y el borde); si no, la de la vista.
fn grip_corner(view: Rect, skin: Option<&Skin>) -> Pos2 {
    let designed = skin.is_some_and(|s| {
        s.piece(GRIP_PIECE, crate::skin::WidgetState::Normal)
            .is_some()
    });
    if designed {
        view.max + Vec2::splat(crate::pixel_ui::PADDING + WINDOW_EDGE)
    } else {
        view.max
    }
}

/// El lado de la agarradera: el de su pieza o el de las rayitas.
fn grip_size(skin: Option<&Skin>) -> Vec2 {
    skin.and_then(|s| s.piece(GRIP_PIECE, crate::skin::WidgetState::Normal))
        .map_or(Vec2::splat(GRIP), |p| {
            vec2(p.size[0] as f32, p.size[1] as f32)
        })
}

/// A la izquierda de la barra de título (desde `left`), centrados en su alto: el play
/// (reproduciendo, la pausa), solo si hay animación, y centrar (volver al zoom y al lugar de
/// entrada). Devuelve cuál se tocó.
fn bar_icons(
    ui: &mut Ui,
    skin: &Skin,
    bar: Rect,
    left: f32,
    animated: bool,
    playing: bool,
) -> (bool, bool) {
    let size = |name: &str| {
        skin.piece(name, crate::skin::WidgetState::Normal)
            .map_or(vec2(5.0, 5.0), |p| vec2(p.size[0] as f32, p.size[1] as f32))
    };
    let mut x = left;
    // Cada ícono en un lugar de `slot` de ancho (el play y la pausa, el de la más ancha: así
    // centrar no se mueve al cambiar entre ellos).
    let mut icon = |ui: &mut Ui, name: &str, label: &str, slot: f32| {
        let s = size(name);
        let at = egui::pos2(x, (bar.center().y - s.y / 2.0).floor());
        let hit = Rect::from_min_size(egui::pos2(x, at.y), vec2(slot, s.y)).expand(2.0);
        x += slot + BAR_GAP;
        // Un poco más grande que el dibujo, para atinarle.
        let response = ui.interact(hit, ui.id().with(("preview_bar", label)), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        (at, response.on_hover_text(label))
    };
    let mut play = false;
    if animated {
        let (piece, label) = if playing {
            (PAUSE_PIECE, "Pause")
        } else {
            (PLAY_PIECE, "Play")
        };
        let slot = size(PLAY_PIECE).x.max(size(PAUSE_PIECE).x);
        let (at, response) = icon(ui, piece, label, slot);
        crate::kit::icon::paint(ui, skin, piece, at, &response);
        play = response.clicked();
    }
    let (at, center) = icon(ui, CENTER_PIECE, "Center", size(CENTER_PIECE).x);
    crate::kit::icon::paint(ui, skin, CENTER_PIECE, at, &center);
    (play, center.clicked())
}

/// La agarradera: tres rayitas en diagonal, con la tinta de los íconos y su luz.
fn grip_shapes(view: Rect, skin: Option<&Skin>, ppp: f32) -> Vec<egui::Shape> {
    let (ink, light) = skin.map_or((Color32::GRAY, Color32::WHITE), |s| {
        (s.colors().outline, s.colors().highlight)
    });
    let corner = view.max - Vec2::splat(2.0);
    let mut shapes = Vec::new();
    // Rayitas de 2, 4 y 6 píxeles en diagonal, una cada 3.
    for n in [2.0f32, 5.0, 8.0] {
        for i in 0..n as i32 {
            let i = i as f32;
            let at = Pos2::new(corner.x - n + 1.0 + i, corner.y - i);
            for (color, shift) in [(light, Vec2::new(1.0, 1.0)), (ink, Vec2::ZERO)] {
                let pixel = Rect::from_min_size(at + shift, Vec2::splat(1.0));
                shapes.push(egui::Shape::rect_filled(
                    pixel.round_to_pixels(ppp),
                    0.0,
                    color,
                ));
            }
        }
    }
    shapes
}

/// El zoom entero más grande con el que el dibujo de `size` entra en `view`; si no entra ni a 1,
/// el que lo achica justo.
fn fit_zoom(size: Vec2, view: Vec2) -> f32 {
    let fit = (view.x / size.x).min(view.y / size.y);
    if fit >= 1.0 {
        fit.floor().min(MAX_ZOOM)
    } else {
        fit
    }
}

/// Un paso de zoom: de a uno arriba de 1 y a la mitad o al doble abajo.
fn step_zoom(zoom: f32, closer: bool) -> f32 {
    match (closer, zoom >= 1.0) {
        (true, true) => (zoom.floor() + 1.0).min(MAX_ZOOM),
        (true, false) => (zoom * 2.0).min(1.0),
        (false, true) if zoom > 1.0 => zoom.ceil() - 1.0,
        (false, _) => (zoom / 2.0).max(1.0 / 16.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{ColorMode, Sprite};

    #[test]
    fn the_zoom_fits_the_drawing_in_whole_steps() {
        assert_eq!(fit_zoom(vec2(16.0, 16.0), VIEW), 9.0);
        assert_eq!(fit_zoom(vec2(64.0, 32.0), VIEW), 3.0);
        assert_eq!(fit_zoom(vec2(2.0, 2.0), VIEW), MAX_ZOOM);
        assert_eq!(fit_zoom(vec2(384.0, 144.0), VIEW), 0.5);
        assert_eq!(fit_zoom(vec2(64.0, 32.0), vec2(400.0, 300.0)), 6.0);
    }

    #[test]
    fn the_wheel_steps_the_zoom() {
        assert_eq!(step_zoom(3.0, true), 4.0);
        assert_eq!(step_zoom(3.0, false), 2.0);
        assert_eq!(step_zoom(1.0, false), 0.5);
        assert_eq!(step_zoom(0.5, true), 1.0);
        assert_eq!(step_zoom(MAX_ZOOM, true), MAX_ZOOM);
    }

    #[test]
    fn its_play_runs_on_its_own_with_the_frame_durations() {
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        editor.insert_empty_frame();
        editor.insert_empty_frame();
        editor.set_frame(0);
        let mut preview = Preview::default();
        preview.toggle_play(&editor);
        assert!(preview.playing());
        let duration = editor.sprite().frames[0].duration_ms as f64;
        preview.tick(&editor, duration - 1.0);
        assert_eq!(preview.frame(&editor), 0);
        preview.tick(&editor, 1.0);
        assert_eq!(preview.frame(&editor), 1);
        assert_eq!(editor.frame, 0, "the timeline does not move");
        preview.tick(&editor, duration * 2.0);
        assert_eq!(preview.frame(&editor), 0, "it loops");
        preview.toggle_play(&editor);
        assert!(!preview.playing());
    }

    #[test]
    fn it_shows_the_canvas_texture_unless_it_plays_another_frame() {
        let ctx = egui::Context::default();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        editor.insert_empty_frame();
        editor.set_frame(0);
        let canvas = (egui::TextureId::User(7), vec2(8.0, 8.0));
        let mut preview = Preview {
            canvas: Some(canvas),
            ..Default::default()
        };
        preview.update(&ctx, &editor);
        assert_eq!(preview.shown, Some(canvas), "the canvas one");
        assert!(preview.texture.is_none(), "no texture of its own");
        // Reproduciendo, en el frame 1 (el lienzo sigue en el 0): la suya.
        preview.toggle_play(&editor);
        let duration = editor.sprite().frames[0].duration_ms as f64;
        preview.tick(&editor, duration);
        preview.update(&ctx, &editor);
        assert!(preview.texture.is_some());
        assert_ne!(preview.shown, Some(canvas));
    }
}
