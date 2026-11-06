//! El modo Anim del workspace de un modelo (como el panel Animation de picoCAD 2): arriba, la
//! Persp con el modelo en el tiempo actual (orbitar, mover el objetivo, rueda y Home como en
//! modo Texture; un clic elige el objeto). Abajo, la barra de la reproducción (⏮ ▶ ⏭, el tiempo,
//! Length, Loop y Add) y el timeline: la columna de nombres (una pista por fila del outliner), la
//! regla de los segundos, los bloques de cada pista (en carriles si se pisan) y la línea del
//! tiempo actual; a la derecha, el panel del bloque elegido.

use eframe::egui::{Ui, UiBuilder, pos2, vec2};

use cs_core::{Axis3, Clip, ClipMotion, Curve, Model3D, Node};

use super::model_edit::{PERSP_BG, bar_split, cad_header, header_split};
use super::texture_mode::texture_panes;
use super::*;
use crate::bar_fields::{self, BarLook};
use crate::editor::{
    ANIM_LENGTH, ClipEdge, FINE_SNAP_MS, MIN_CLIP_MS, NewClip, Row, SNAP_MS, row_clips,
};
use crate::kit::button::ButtonState;
use crate::skin::Skin;

/// La columna de los nombres, el alto de la regla y el de un carril de una pista.
pub(crate) const NAMES_W: f32 = 96.0;
pub(crate) const RULER_H: f32 = 13.0;
pub(crate) const LANE_H: f32 = 13.0;
/// El panel del bloque, a la derecha del timeline.
pub(crate) const PANEL_W: f32 = 176.0;
/// Aire a los lados del área de los bloques (para agarrar los bordes en 0 y al final).
const LANE_PAD: f32 = 4.0;
/// Cuánto del borde de un bloque lo estira en vez de moverlo.
const EDGE_GRAB: f32 = 3.0;
/// La sangría de cada nivel en la columna de nombres, el ícono y su aire.
const INDENT: f32 = 8.0;
const ICON: f32 = 9.0;
const NAME_GAP: f32 = 3.0;
/// Las mayúsculas de un texto en una fila de 13.
const CAPS_TOP: f32 = 3.0;
/// El aire del panel del bloque.
const PANEL_PAD: f32 = 6.0;

/// Dónde quedó cada parte del modo Anim en este frame (en pantalla).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AnimPanes {
    pub top: Rect,
    pub bar: Rect,
    /// La columna de los nombres de las pistas (debajo de la regla).
    pub names: Rect,
    /// La regla de los segundos (encima de las pistas).
    pub ruler: Rect,
    /// Las pistas con sus bloques.
    pub tracks: Rect,
    pub panel: Rect,
}

impl AnimPanes {
    /// Las partes de debajo de la barra (`below`): el timeline a la izquierda y el panel del
    /// bloque a la derecha, con una línea de 1 entre los dos.
    fn new(top: Rect, bar: Rect, below: Rect) -> AnimPanes {
        let panel_w = PANEL_W.min((below.width() / 2.0).floor());
        let split = below.right() - panel_w;
        let panel = Rect::from_min_max(pos2(split, below.top()), below.max);
        let timeline = Rect::from_min_max(below.min, pos2(split - 1.0, below.bottom()));
        let names_right = (timeline.left() + NAMES_W).min(timeline.right());
        let ruler_bottom = (timeline.top() + RULER_H).min(timeline.bottom());
        AnimPanes {
            top,
            bar,
            names: Rect::from_min_max(
                pos2(timeline.left(), ruler_bottom),
                pos2(names_right, timeline.bottom()),
            ),
            ruler: Rect::from_min_max(
                pos2(names_right + 1.0, timeline.top()),
                pos2(timeline.right(), ruler_bottom),
            ),
            tracks: Rect::from_min_max(pos2(names_right + 1.0, ruler_bottom + 1.0), timeline.max),
            panel,
        }
    }

    fn timeline(&self) -> Rect {
        Rect::from_min_max(pos2(self.names.left(), self.ruler.top()), self.tracks.max)
    }
}

/// De tiempo (ms) a x de pantalla en el área de los bloques: la animación entera a lo ancho.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TimeMap {
    left: f32,
    width: f32,
    length: f32,
}

impl TimeMap {
    pub(crate) fn new(tracks: Rect, length: u32) -> TimeMap {
        TimeMap {
            left: tracks.left() + LANE_PAD,
            width: (tracks.width() - 2.0 * LANE_PAD).max(1.0),
            length: length.max(1) as f32,
        }
    }

    pub(crate) fn x(&self, ms: f32) -> f32 {
        (self.left + ms / self.length * self.width).round()
    }

    pub(crate) fn ms(&self, x: f32) -> f32 {
        (x - self.left) / self.width * self.length
    }
}

/// Una pista del timeline: una fila del outliner con sus bloques.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Track {
    pub row: Row,
    pub depth: usize,
    pub name: String,
    pub icon: &'static str,
    /// El carril de cada bloque y cuántos hay.
    pub lanes: Vec<usize>,
    pub lane_count: usize,
    /// Desde arriba de las pistas (sin el desplazamiento).
    pub top: f32,
}

impl Track {
    pub(crate) fn height(&self) -> f32 {
        self.lane_count as f32 * LANE_H
    }
}

/// Las pistas de `model`: las filas que se ven en el outliner (lo de adentro de un grupo cerrado
/// no), en orden.
pub(crate) fn tracks(model: &Model3D) -> Vec<Track> {
    fn walk(model: &Model3D, nodes: &[Node], depth: usize, out: &mut Vec<Track>, top: &mut f32) {
        for n in nodes {
            let (row, name, icon, clips, open, children) = match n {
                Node::Object(o) => {
                    let Some(obj) = model.objects.get(*o) else {
                        continue;
                    };
                    let icon = crate::model_panel::kind_icon(obj.shown_kind());
                    (Row::Object(*o), &obj.name, icon, &obj.anim, false, None)
                }
                Node::Group { group, children } => {
                    let Some(g) = model.groups.get(*group) else {
                        continue;
                    };
                    let icon = "icon.outliner.folder";
                    (
                        Row::Group(*group),
                        &g.name,
                        icon,
                        &g.anim,
                        g.open,
                        Some(children),
                    )
                }
            };
            let (lanes, lane_count) = cs_3d::clip_lanes(clips);
            let track = Track {
                row,
                depth,
                name: name.clone(),
                icon,
                lanes,
                lane_count,
                top: *top,
            };
            *top += track.height();
            out.push(track);
            if let (true, Some(children)) = (open, children) {
                walk(model, children, depth + 1, out, top);
            }
        }
    }
    let mut out = Vec::new();
    let mut top = 0.0;
    walk(model, &model.tree, 0, &mut out, &mut top);
    out
}

/// Dónde va el bloque `i` de `track` (con las pistas desplazadas `scroll` hacia arriba).
fn clip_rect(
    map: &TimeMap,
    tracks: Rect,
    scroll: f32,
    track: &Track,
    clip: &Clip,
    i: usize,
) -> Rect {
    let x0 = map.x(clip.start_ms as f32);
    let x1 = map.x(clip.end_ms() as f32).max(x0 + 3.0);
    let lane = track.lanes.get(i).copied().unwrap_or(0) as f32;
    let y = tracks.top() - scroll + track.top + lane * LANE_H;
    Rect::from_min_max(pos2(x0, y + 1.0), pos2(x1, y + LANE_H - 1.0))
}

/// Lo que hay del timeline bajo `p`: un bloque (y qué parte) o una pista vacía.
#[derive(Clone, Copy, Debug, PartialEq)]
enum TimelineHit {
    Clip(Row, usize, ClipEdge),
    Track(Row),
}

fn timeline_hit(
    model: &Model3D,
    tracks_list: &[Track],
    panes: &AnimPanes,
    map: &TimeMap,
    scroll: f32,
    p: Pos2,
) -> Option<TimelineHit> {
    let in_tracks = panes.tracks.contains(p);
    let in_names = panes.names.contains(p);
    if !in_tracks && !in_names {
        return None;
    }
    let y = p.y - panes.tracks.top() + scroll;
    let track = tracks_list
        .iter()
        .find(|t| (t.top..t.top + t.height()).contains(&y))?;
    if in_tracks && let Some(clips) = row_clips(model, track.row) {
        // El de más adelante gana (se dibuja encima).
        for (i, clip) in clips.iter().enumerate().rev() {
            let r = clip_rect(map, panes.tracks, scroll, track, clip, i);
            if !r.expand2(vec2(1.0, 0.0)).contains(p) {
                continue;
            }
            let edge = if p.x <= r.left() + EDGE_GRAB {
                ClipEdge::Start
            } else if p.x >= r.right() - EDGE_GRAB {
                ClipEdge::End
            } else {
                ClipEdge::Body
            };
            return Some(TimelineHit::Clip(track.row, i, edge));
        }
    }
    Some(TimelineHit::Track(track.row))
}

/// El texto corto de un bloque: el tipo y su eje o valor.
fn clip_label(clip: &Clip) -> String {
    match clip.motion {
        ClipMotion::Rotate { axis, turns } => {
            format!("rot {} {}", axis.label(), turns_text(turns))
        }
        ClipMotion::Move { .. } => "move".to_owned(),
        ClipMotion::Scale { .. } => "scale".to_owned(),
    }
}

/// Las vueltas (milésimas) como en picoCAD 2: "0.3", "1", "-0.25".
fn turns_text(milli: i32) -> String {
    let s = format!("{:.3}", milli as f32 / 1000.0);
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// El tiempo como en picoCAD 2 ("3.85/5s").
pub(crate) fn time_text(t: u32, length: u32) -> String {
    let secs = |ms: u32| {
        let s = format!("{:.2}", ms as f32 / 1000.0);
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    };
    format!("{}/{}s", secs(t), secs(length))
}

/// Los colores del timeline: los del skin o, sin Pixel UI, los de egui.
#[derive(Clone, Copy)]
struct Look {
    bg: Color32,
    ruler_bg: Color32,
    line: Color32,
    tick: Color32,
    text: Color32,
    selected: Color32,
    playhead: Color32,
    block: Color32,
    block_on: Color32,
}

impl Look {
    fn new(ui: &Ui, skin: Option<&Skin>) -> Look {
        match skin {
            Some(skin) => {
                let c = skin.colors();
                Look {
                    bg: c.panel,
                    ruler_bg: c.background,
                    line: crate::pixel_ui::control_border(ui.ctx()),
                    tick: c.ruler_tick,
                    text: c.text,
                    selected: c.selection,
                    playhead: c.accent,
                    block: c.panel,
                    block_on: c.selection,
                }
            }
            None => {
                let v = ui.visuals();
                Look {
                    bg: v.extreme_bg_color,
                    ruler_bg: v.faint_bg_color,
                    line: v.widgets.noninteractive.bg_stroke.color,
                    tick: v.weak_text_color(),
                    text: v.text_color(),
                    selected: v.selection.bg_fill,
                    playhead: v.error_fg_color,
                    block: v.widgets.inactive.bg_fill,
                    block_on: v.selection.bg_fill,
                }
            }
        }
    }
}

impl CanvasView {
    /// El workspace del modelo en modo Anim, en `rect` (todo el lienzo).
    pub(super) fn show_anim_mode(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        rect: Rect,
        editor: &mut Editor,
        modal: bool,
    ) {
        let (header, area) = header_split(rect);
        let skin = crate::pixel_ui::frame_skin(ui.ctx());
        let skin = skin.as_deref();
        let height = crate::panels::options_bar_height(skin);
        let (object_bar, area) = bar_split(area, height);
        let (top, below) = texture_panes(area);
        let (bar, rest) = bar_split(below, height);
        let panes = AnimPanes::new(top, bar, rest);
        self.edit_quads = None;
        self.texture_panes = Some((top, rest));
        self.anim_panes = Some(panes);
        self.header_rect = Some(header);
        self.object_bar_rect = Some(object_bar);
        self.pending_zoom = None;
        editor.model_edit.key_request = None;
        // La reproducción avanza con el reloj de la UI.
        let now = ui.input(|i| i.time);
        if editor.tick_anim(now) {
            ui.ctx().request_repaint();
        }
        self.texture_top_input(ui, response, top, editor, modal);
        self.timeline_input(ui, &panes, editor, modal);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.background);
        painter.rect_filled(top, 0.0, PERSP_BG);
        let view = editor.view();
        if let Some(model) = editor.edit_model_shown() {
            let ctx = ui.ctx().clone();
            self.draw_persp(&painter, top, editor, view.document.as_ref(), model, &ctx);
        }
        drop(view);
        let line = top.bottom().round();
        painter.rect_filled(
            Rect::from_x_y_ranges(area.x_range(), line..=line + 1.0),
            0.0,
            crate::pixel_ui::control_border(ui.ctx()),
        );
        self.draw_timeline(ui, &painter, &panes, editor, skin);
        cad_header(ui, header, editor);
        super::object_bar::object_bar(ui, object_bar, editor);
        transport_bar(ui, bar, editor);
        clip_panel(ui, panes.panel, editor, skin);
    }

    /// La entrada del timeline: la rueda lo desplaza; en la regla, presionar y arrastrar mueve
    /// el tiempo; en un bloque, lo elige y lo mueve o lo estira (Shift: ajuste fino); en una
    /// pista vacía o en su nombre, la elige.
    fn timeline_input(
        &mut self,
        ui: &egui::Ui,
        panes: &AnimPanes,
        editor: &mut Editor,
        modal: bool,
    ) {
        let timeline = panes.timeline();
        let response = ui.interact(
            timeline,
            ui.id().with("anim_timeline"),
            Sense::click_and_drag(),
        );
        let Some(model) = editor.edit_model_shown().cloned() else {
            return;
        };
        let list = tracks(&model);
        let map = TimeMap::new(panes.tracks, model.anim_ms);
        // La rueda, si las pistas no caben.
        let content: f32 = list.iter().map(Track::height).sum();
        let max_scroll = (content - panes.tracks.height()).max(0.0);
        if response.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            self.anim_scroll -= dy;
        }
        self.anim_scroll = self.anim_scroll.clamp(0.0, max_scroll);
        let scroll = self.anim_scroll;
        let (pressed, down, pointer, shift) = ui.input(|i| {
            (
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.interact_pos(),
                i.modifiers.shift,
            )
        });
        let step = if shift { FINE_SNAP_MS } else { SNAP_MS };
        // Un gesto en curso sigue al puntero hasta soltar.
        if editor.anim().drag.is_some() {
            if !down {
                editor.end_clip_drag();
            } else if let Some(p) = pointer {
                editor.drag_clip_to(map.ms(p.x), step);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            return;
        }
        if self.anim_scrub {
            match (down, pointer) {
                (true, Some(p)) => editor.set_anim_time(map.ms(p.x).round().max(0.0) as u32),
                _ => self.anim_scrub = false,
            }
            return;
        }
        let Some(p) = response.hover_pos() else {
            return;
        };
        let hit = timeline_hit(&model, &list, panes, &map, scroll, p);
        match hit {
            Some(TimelineHit::Clip(_, _, ClipEdge::Start | ClipEdge::End)) => {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal)
            }
            Some(TimelineHit::Clip(..)) => ui.ctx().set_cursor_icon(egui::CursorIcon::Grab),
            _ => {}
        }
        if !pressed || modal || !response.hovered() {
            return;
        }
        if panes.ruler.contains(p) {
            self.anim_scrub = true;
            editor.set_anim_time(map.ms(p.x).round().max(0.0) as u32);
            return;
        }
        match hit {
            Some(TimelineHit::Clip(row, i, edge)) => {
                editor.start_clip_drag(row, i, edge, map.ms(p.x));
            }
            Some(TimelineHit::Track(row)) => {
                editor.select_clip(None);
                editor.set_active_row(Some(row), false);
            }
            None => {}
        }
    }

    /// El timeline: la regla, los nombres, las pistas con sus bloques y la línea del tiempo.
    fn draw_timeline(
        &self,
        ui: &Ui,
        painter: &egui::Painter,
        panes: &AnimPanes,
        editor: &Editor,
        skin: Option<&Skin>,
    ) {
        let look = Look::new(ui, skin);
        let ppp = ui.ctx().pixels_per_point();
        let timeline = panes.timeline();
        painter.rect_filled(timeline, 0.0, look.bg);
        painter.rect_filled(panes.ruler, 0.0, look.ruler_bg);
        // Las líneas: debajo de la regla, a la derecha de los nombres y a la izquierda del
        // panel.
        let line = |r: Rect| painter.rect_filled(r, 0.0, look.line);
        line(Rect::from_x_y_ranges(
            timeline.x_range(),
            panes.ruler.bottom()..=panes.ruler.bottom() + 1.0,
        ));
        line(Rect::from_x_y_ranges(
            panes.names.right()..=panes.names.right() + 1.0,
            timeline.y_range(),
        ));
        line(Rect::from_x_y_ranges(
            panes.panel.left() - 1.0..=panes.panel.left(),
            panes.panel.y_range(),
        ));
        let Some(model) = editor.edit_model_shown() else {
            return;
        };
        let list = tracks(model);
        let map = TimeMap::new(panes.tracks, model.anim_ms);
        let scroll = self.anim_scroll;
        let length = model.anim_ms.max(1);
        // La regla: rayitas cada 100 ms (si caben), más largas cada 500 y cada segundo con su
        // número.
        let ruler = painter.with_clip_rect(panes.ruler);
        let px_per_100 = map.x(100.0) - map.x(0.0);
        let mut t = 0;
        while t <= length {
            let x = map.x(t as f32);
            let (h, label) = match t {
                _ if t % 1000 == 0 => (5.0, true),
                _ if t % 500 == 0 => (3.0, false),
                _ => (2.0, false),
            };
            if px_per_100 >= 3.0 || t % 500 == 0 {
                ruler.rect_filled(
                    Rect::from_min_max(
                        pos2(x, panes.ruler.bottom() - h),
                        pos2(x + 1.0, panes.ruler.bottom()),
                    ),
                    0.0,
                    look.tick,
                );
            }
            if label {
                let text = format!("{}s", t / 1000);
                let draw = |left: f32| {
                    crate::pixel_draw::caps_text_in(
                        ui,
                        egui::TextStyle::Small,
                        &text,
                        left,
                        panes.ruler.top() + 2.0,
                        look.text,
                    )
                };
                let (mut shape, width) = draw(x + 2.0);
                // El del final, a la izquierda de su rayita (si no, se corta).
                if x + 2.0 + width > panes.ruler.right() {
                    shape = draw(x - 1.0 - width).0;
                }
                ruler.add(shape);
            }
            t += 100;
        }
        // Las pistas y sus nombres.
        let names = painter.with_clip_rect(panes.names);
        let lanes = painter.with_clip_rect(panes.tracks);
        let active = editor.active_row();
        let chosen = editor.selected_clip().map(|(r, i, _)| (r, i));
        for track in &list {
            let y = panes.tracks.top() - scroll + track.top;
            let row_rect = Rect::from_min_max(
                pos2(panes.names.left(), y),
                pos2(panes.names.right(), y + track.height()),
            );
            if active == Some(track.row) {
                names.rect_filled(row_rect, 0.0, look.selected);
            }
            let x = panes.names.left() + 3.0 + track.depth as f32 * INDENT;
            if let Some(skin) = skin {
                names.extend(crate::pixel_draw::piece_at(
                    skin,
                    track.icon,
                    pos2(x, y + 2.0),
                    ppp,
                ));
            }
            let (text, _) = crate::pixel_draw::caps_text(
                ui,
                &track.name,
                x + ICON + NAME_GAP,
                y + CAPS_TOP,
                look.text,
            );
            names.add(text);
            // La línea de abajo de la pista, de lado a lado.
            let bottom = y + track.height() - 1.0;
            for r in [panes.names, panes.tracks] {
                painter.with_clip_rect(r).rect_filled(
                    Rect::from_x_y_ranges(r.x_range(), bottom..=bottom + 1.0),
                    0.0,
                    look.ruler_bg,
                );
            }
            let Some(clips) = row_clips(model, track.row) else {
                continue;
            };
            for (i, clip) in clips.iter().enumerate() {
                let r = clip_rect(&map, panes.tracks, scroll, track, clip, i);
                let on = chosen == Some((track.row, i));
                match skin {
                    Some(skin) => {
                        let state = if on {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Normal
                        };
                        lanes.extend(crate::kit::button::frame_shapes(skin, r, state, ppp));
                        if on {
                            // El elegido, con un contorno del color de la línea del tiempo.
                            lanes.rect_stroke(
                                r.expand(1.0),
                                0.0,
                                Stroke::new(1.0, look.playhead),
                                egui::StrokeKind::Inside,
                            );
                        }
                        let color = crate::kit::button::text_color(skin.colors(), state);
                        let text_painter =
                            lanes.with_clip_rect(r.shrink2(vec2(2.0, 0.0)).intersect(panes.tracks));
                        let (text, _) = crate::pixel_draw::caps_text_in(
                            ui,
                            egui::TextStyle::Small,
                            &clip_label(clip),
                            r.left() + 3.0,
                            r.top() + 3.0,
                            color,
                        );
                        text_painter.add(text);
                    }
                    None => {
                        let fill = if on { look.block_on } else { look.block };
                        lanes.rect_filled(r, 2.0, fill);
                        lanes.rect_stroke(
                            r,
                            2.0,
                            Stroke::new(1.0, look.line),
                            egui::StrokeKind::Inside,
                        );
                        lanes.with_clip_rect(r.intersect(panes.tracks)).text(
                            r.left_center() + vec2(3.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            clip_label(clip),
                            egui::TextStyle::Small.resolve(ui.style()),
                            look.text,
                        );
                    }
                }
            }
        }
        // La línea del tiempo actual, de la regla hasta abajo, con su cabeza en la regla.
        let x = map.x(editor.anim().time_ms.min(length) as f32);
        let all = painter.with_clip_rect(Rect::from_min_max(
            pos2(panes.ruler.left(), panes.ruler.top()),
            panes.tracks.max,
        ));
        all.rect_filled(
            Rect::from_min_max(
                pos2(x, panes.ruler.top()),
                pos2(x + 1.0, panes.tracks.bottom()),
            ),
            0.0,
            look.playhead,
        );
        all.rect_filled(
            Rect::from_min_max(
                pos2(x - 2.0, panes.ruler.bottom() - 4.0),
                pos2(x + 3.0, panes.ruler.bottom()),
            ),
            0.0,
            look.playhead,
        );
    }
}

/// La barra de la reproducción en `rect`: ⏮ ▶/⏸ ⏭, el tiempo, Length, Loop y Add (los bloques
/// que se agregan a la pista elegida).
fn transport_bar(ui: &mut egui::Ui, rect: Rect, editor: &mut Editor) {
    let skin = crate::pixel_ui::frame_skin(ui.ctx());
    let skin = skin.as_deref();
    let builder = UiBuilder::new().max_rect(rect).id_salt("cad_anim_bar");
    let mut bar_ui = ui.new_child(builder);
    bar_ui.set_clip_rect(rect);
    crate::panels::bar_panel("cad_anim_bar", bar_ui.style(), skin).show(&mut bar_ui, |ui| {
        crate::panels::bar_row(ui, skin, "cad_anim_bar_scroll", |ui, look| {
            transport(ui, editor, look);
        });
    });
}

/// Lo de adentro de la barra.
fn transport(ui: &mut egui::Ui, editor: &mut Editor, look: Option<BarLook>) {
    use crate::panels::{GROUP_GAP, LABEL_FIELD_GAP, gap};
    let length = editor.anim_length();
    let playing = editor.anim().playing;
    // ⏮ ▶ ⏭.
    let clicked = match look {
        Some(look) => play_group(ui, look, playing),
        None => {
            let mut clicked = None;
            for (k, label) in ["⏮", if playing { "⏸" } else { "▶" }, "⏭"]
                .into_iter()
                .enumerate()
            {
                if ui.button(label).clicked() {
                    clicked = Some(k);
                }
            }
            clicked
        }
    };
    match clicked {
        Some(0) => editor.set_anim_time(0),
        Some(1) => editor.toggle_anim_play(),
        Some(2) => editor.set_anim_time(length),
        _ => {}
    }
    let time = time_text(editor.anim().time_ms, length);
    match look {
        Some(look) => {
            gap(ui, GROUP_GAP);
            bar_fields::label(ui, look, &time);
            gap(ui, GROUP_GAP);
            bar_fields::label(ui, look, "Length");
            gap(ui, LABEL_FIELD_GAP);
        }
        None => {
            ui.label(time);
            ui.separator();
            ui.label("Length");
        }
    }
    // Lo que dura la animación: un arrastre o lo escrito es un paso de undo.
    let mut ms = length;
    let response = match look {
        Some(look) => {
            bar_fields::number_with_suffix(ui, look, "anim_length", &mut ms, ANIM_LENGTH, "ms")
        }
        None => ui.add(
            egui::DragValue::new(&mut ms)
                .range(ANIM_LENGTH)
                .suffix(" ms"),
        ),
    };
    let group = egui::Id::new("cad_anim_length");
    if ms != length {
        editor.set_anim_length_grouped(ms, group);
    }
    if !response.has_focus() && !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group.value());
    }
    let mut looping = editor.anim().looping;
    match look {
        Some(look) => {
            gap(ui, GROUP_GAP);
            bar_fields::checkbox(ui, look, &mut looping, "Loop");
            gap(ui, GROUP_GAP);
            bar_fields::label(ui, look, "Add");
            gap(ui, LABEL_FIELD_GAP);
        }
        None => {
            ui.checkbox(&mut looping, "Loop");
            ui.separator();
            ui.label("Add");
        }
    }
    if looping != editor.anim().looping {
        editor.set_anim_looping(looping);
    }
    let can_add = editor.active_row().is_some();
    let added = ui
        .add_enabled_ui(can_add, |ui| match look {
            Some(look) => add_group(ui, look),
            None => {
                let mut added = None;
                for kind in NewClip::ALL {
                    if ui.button(kind.label()).clicked() {
                        added = Some(kind);
                    }
                }
                added
            }
        })
        .inner;
    if let Some(kind) = added {
        editor.add_clip(kind);
    }
}

/// Los botones ⏮ ▶ ⏭ (un grupo del kit con los íconos del timeline; reproduciendo, el del
/// medio es la pausa). Devuelve en cuál se hizo clic.
fn play_group(ui: &mut egui::Ui, look: BarLook, playing: bool) -> Option<usize> {
    use crate::kit::group::{self, GroupItem};
    let play_icon = if playing { "" } else { "timeline.icon.play" };
    let play_label = if playing { "Pause" } else { "Play" };
    let items = [
        GroupItem::icon("timeline.icon.first", "Go to start"),
        GroupItem::icon(play_icon, play_label),
        GroupItem::icon("timeline.icon.last", "Go to end"),
    ];
    let width = group::width(ui, &items);
    let (rect, _) = ui.allocate_exact_size(vec2(width, bar_fields::FIELD_HEIGHT), Sense::hover());
    let at = pos2(rect.left().round(), look.top + bar_fields::FIELD_TOP);
    let id = ui.id().with("cad_anim_play");
    let painter = ui.painter().clone();
    let responses = group::group_responses_at(ui, look.skin, &painter, id, at, &items, &|_| false);
    if playing {
        let rects = group::segment_rects(ui, &items, at);
        let down = if responses[1].is_pointer_button_down_on() {
            crate::kit::button::PRESS_DROP
        } else {
            0.0
        };
        crate::timeline::pause_icon(ui, look.skin, rects[1].min + vec2(2.0, 2.0 + down));
    }
    responses.iter().position(|r| r.clicked())
}

/// El grupo Add: un botón por tipo de bloque. Devuelve el elegido.
fn add_group(ui: &mut egui::Ui, look: BarLook) -> Option<NewClip> {
    use crate::kit::group::{self, GroupItem};
    let items: Vec<GroupItem> = NewClip::ALL
        .iter()
        .map(|k| GroupItem::text(k.label()).enabled(ui.is_enabled()))
        .collect();
    let width = group::width(ui, &items);
    let (rect, _) = ui.allocate_exact_size(vec2(width, bar_fields::FIELD_HEIGHT), Sense::hover());
    let at = pos2(rect.left().round(), look.top + bar_fields::FIELD_TOP);
    let id = ui.id().with("cad_anim_add");
    let painter = ui.painter().clone();
    group::button_group_at(ui, look.skin, &painter, id, at, &items, &|_| false)
        .map(|i| NewClip::ALL[i])
}

/// El panel del bloque elegido en `rect` (como la ventanita de picoCAD 2): su tipo, sus valores,
/// Curve, Ping pong, Repeat, Start, Length y Delete. Cada cambio es un paso de undo (un arrastre
/// o lo escrito en un campo, uno solo).
fn clip_panel(ui: &mut egui::Ui, rect: Rect, editor: &mut Editor, skin: Option<&Skin>) {
    let fill = match skin {
        Some(skin) => skin.colors().panel,
        None => ui.visuals().panel_fill,
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let inner = rect.shrink(PANEL_PAD);
    if inner.width() <= 0.0 || inner.height() <= 0.0 {
        return;
    }
    let builder = UiBuilder::new()
        .max_rect(inner)
        .id_salt("cad_anim_clip_panel")
        .layout(egui::Layout::top_down(egui::Align::Min));
    let mut panel = ui.new_child(builder);
    panel.set_clip_rect(rect);
    panel.spacing_mut().item_spacing.y = 3.0;
    let ui = &mut panel;
    let Some((row, i, clip)) = editor.selected_clip() else {
        ui.label("No block");
        ui.add_space(4.0);
        ui.label("Choose a track and use");
        ui.label("Add, or click a block.");
        return;
    };
    // El título y Delete arriba (como la papelera de picoCAD 2); lo demás, con scroll si no
    // cabe.
    let mut delete = false;
    ui.horizontal(|ui| {
        ui.label(clip.motion.name());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let r = match skin {
                Some(skin) => crate::kit::button::button(ui, skin, "Delete"),
                None => ui.button("Delete"),
            };
            delete = r.clicked();
        });
    });
    let length = editor.anim_length();
    let mut next = clip;
    egui::ScrollArea::vertical()
        .id_salt("cad_anim_clip_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            use crate::bake_dialog::{check, number, row};
            motion_fields(ui, skin, &mut next.motion);
            row(ui, "Curve", |ui| {
                crate::widgets::combo(ui, "anim_curve", next.curve.label(), |ui| {
                    for c in Curve::ALL {
                        crate::widgets::choice(ui, &mut next.curve, c, c.label());
                    }
                });
            });
            row(ui, "", |ui| {
                check(ui, skin, &mut next.ping_pong, "Ping pong")
            });
            row(ui, "Repeat", |ui| {
                number(ui, skin, "anim_repeat", &mut next.repeat, 1..=99, 1);
            });
            row(ui, "Start", |ui| {
                let max = length.saturating_sub(MIN_CLIP_MS);
                number(ui, skin, "anim_start", &mut next.start_ms, 0..=max, SNAP_MS);
            });
            row(ui, "Length", |ui| {
                let max = length.saturating_sub(next.start_ms).max(MIN_CLIP_MS);
                let range = MIN_CLIP_MS..=max;
                number(
                    ui,
                    skin,
                    "anim_duration",
                    &mut next.duration_ms,
                    range,
                    SNAP_MS,
                );
            });
        });
    let group = egui::Id::new("cad_anim_clip");
    if next != clip {
        editor.set_clip_grouped(row, i, next, group);
    }
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    if !typing && !ui.input(|i| i.pointer.any_down()) {
        editor.end_group_of(group.value());
    }
    if delete {
        editor.delete_clip();
    }
}

/// Los valores del tipo del bloque: el eje y las vueltas, el desplazamiento (en unidades) o la
/// escala (en %).
fn motion_fields(ui: &mut Ui, skin: Option<&Skin>, motion: &mut ClipMotion) {
    use crate::bake_dialog::{number, row};
    match motion {
        ClipMotion::Rotate { axis, turns } => {
            row(ui, "Axis", |ui| {
                for a in Axis3::ALL {
                    let on = *axis == a;
                    let clicked = match skin {
                        Some(skin) => crate::kit::check::radio(ui, skin, on, a.label()).clicked(),
                        None => ui.radio(on, a.label()).clicked(),
                    };
                    if clicked {
                        *axis = a;
                    }
                }
            });
            row(ui, "Turns", |ui| {
                let mut v = *turns as f32 / 1000.0;
                let before = v;
                number(ui, skin, "anim_turns", &mut v, -16.0..=16.0, 0.05);
                if v != before {
                    *turns = (v * 1000.0).round() as i32;
                }
            });
        }
        ClipMotion::Move { offset } => {
            let f = cs_core::FIXED as f32;
            for (label, v) in [
                ("X", &mut offset.x),
                ("Y", &mut offset.y),
                ("Z", &mut offset.z),
            ] {
                row(ui, label, |ui| {
                    let mut u = *v as f32 / f;
                    let before = u;
                    number(
                        ui,
                        skin,
                        &format!("anim_move_{label}"),
                        &mut u,
                        -64.0..=64.0,
                        0.25,
                    );
                    if u != before {
                        *v = (u * f).round() as i32;
                    }
                });
            }
        }
        ClipMotion::Scale { factor } => {
            for (label, v) in [
                ("X %", &mut factor.x),
                ("Y %", &mut factor.y),
                ("Z %", &mut factor.z),
            ] {
                row(ui, label, |ui| {
                    let mut pct = *v / 10;
                    let before = pct;
                    number(
                        ui,
                        skin,
                        &format!("anim_scale_{label}"),
                        &mut pct,
                        1..=1000,
                        10,
                    );
                    if pct != before {
                        *v = pct * 10;
                    }
                });
            }
        }
    }
}

impl CanvasView {
    /// Las partes del modo Anim del último frame (para los tests de la app).
    #[cfg(test)]
    pub(crate) fn anim_panes(&self) -> Option<AnimPanes> {
        self.anim_panes
    }
}

/// Dónde se dibujó el bloque `i` de la pista `row` en el último frame (para los tests de la
/// app).
#[cfg(test)]
pub(crate) fn anim_mode_block(
    canvas: &CanvasView,
    editor: &Editor,
    row: Row,
    i: usize,
) -> Option<Rect> {
    let panes = canvas.anim_panes?;
    let model = editor.edit_model_shown()?;
    let list = tracks(model);
    let track = list.iter().find(|t| t.row == row)?;
    let clip = row_clips(model, row)?.get(i)?;
    let map = TimeMap::new(panes.tracks, model.anim_ms);
    Some(clip_rect(
        &map,
        panes.tracks,
        canvas.anim_scroll,
        track,
        clip,
        i,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_time_is_written_like_picocad() {
        assert_eq!(time_text(3850, 5000), "3.85/5s");
        assert_eq!(time_text(0, 2000), "0/2s");
        assert_eq!(time_text(1200, 2500), "1.2/2.5s");
        assert_eq!(turns_text(300), "0.3");
        assert_eq!(turns_text(-250), "-0.25");
        assert_eq!(turns_text(1000), "1");
    }

    #[test]
    fn the_time_map_fits_the_animation_in_the_tracks() {
        let tracks = Rect::from_min_size(pos2(100.0, 0.0), vec2(208.0, 50.0));
        let map = TimeMap::new(tracks, 2000);
        assert_eq!(map.x(0.0), 104.0);
        assert_eq!(map.x(2000.0), 304.0);
        assert_eq!(map.x(1000.0), 204.0);
        assert_eq!(map.ms(204.0), 1000.0);
    }

    #[test]
    fn tracks_follow_the_outliner_and_overlaps_add_lanes() {
        let mut m = Model3D::default();
        let obj = |name: &str| cs_core::Object3D {
            name: name.into(),
            mesh: Default::default(),
            transform: Default::default(),
            double_sided: false,
            kind: None,
            hidden: false,
            locked: false,
            anim: Vec::new(),
        };
        m.objects = vec![obj("box"), obj("lid")];
        m.groups.push(cs_core::Group3D {
            name: "hinge".into(),
            transform: Default::default(),
            hidden: false,
            locked: false,
            open: true,
            anim: vec![
                Clip::new(
                    ClipMotion::Rotate {
                        axis: Axis3::X,
                        turns: -250,
                    },
                    0,
                    1000,
                ),
                Clip::new(
                    ClipMotion::Rotate {
                        axis: Axis3::X,
                        turns: 250,
                    },
                    500,
                    1000,
                ),
            ],
        });
        m.tree = vec![
            Node::Object(0),
            Node::Group {
                group: 0,
                children: vec![Node::Object(1)],
            },
        ];
        let list = tracks(&m);
        let rows: Vec<_> = list
            .iter()
            .map(|t| (t.row, t.depth, t.lane_count))
            .collect();
        assert_eq!(
            rows,
            [
                (Row::Object(0), 0, 1),
                (Row::Group(0), 0, 2),
                (Row::Object(1), 1, 1)
            ]
        );
        assert_eq!(list[2].top, 3.0 * LANE_H);
        // Cerrado, lo de adentro no se ve.
        m.groups[0].open = false;
        assert_eq!(tracks(&m).len(), 2);
    }

    #[test]
    fn hitting_a_block_tells_the_edge() {
        let mut m = Model3D::default();
        m.objects.push(cs_core::Object3D {
            name: "o".into(),
            mesh: Default::default(),
            transform: Default::default(),
            double_sided: false,
            kind: None,
            hidden: false,
            locked: false,
            anim: vec![Clip::new(
                ClipMotion::Move {
                    offset: Default::default(),
                },
                500,
                1000,
            )],
        });
        m.normalize_tree();
        let top = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 100.0));
        let bar = Rect::from_min_size(pos2(0.0, 100.0), vec2(400.0, 20.0));
        let below = Rect::from_min_size(pos2(0.0, 120.0), vec2(600.0, 100.0));
        let panes = AnimPanes::new(top, bar, below);
        let list = tracks(&m);
        let map = TimeMap::new(panes.tracks, m.anim_ms);
        let y = panes.tracks.top() + 6.0;
        let at = |ms: f32| pos2(map.x(ms), y);
        let hit = |p| timeline_hit(&m, &list, &panes, &map, 0.0, p);
        assert_eq!(
            hit(at(1000.0)),
            Some(TimelineHit::Clip(Row::Object(0), 0, ClipEdge::Body))
        );
        assert_eq!(
            hit(at(500.0)),
            Some(TimelineHit::Clip(Row::Object(0), 0, ClipEdge::Start))
        );
        assert_eq!(
            hit(at(1500.0) - vec2(1.0, 0.0)),
            Some(TimelineHit::Clip(Row::Object(0), 0, ClipEdge::End))
        );
        assert_eq!(hit(at(200.0)), Some(TimelineHit::Track(Row::Object(0))));
        assert_eq!(
            hit(pos2(panes.names.left() + 5.0, y)),
            Some(TimelineHit::Track(Row::Object(0)))
        );
        // Debajo de la última pista, nada.
        assert_eq!(hit(pos2(map.x(1000.0), y + 40.0)), None);
        // El panel queda a la derecha con su ancho.
        assert_eq!(panes.panel.width(), PANEL_W);
        assert_eq!(panes.tracks.right(), panes.panel.left() - 1.0);
    }
}
