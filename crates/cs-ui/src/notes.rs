//! Las notas (post-its) sobre el lienzo: se dibujan encima de los artboards con la pieza `note`
//! del skin (o un rect amarillo sin ella), se mueven desde la barra, se agrandan desde la esquina
//! de abajo a la derecha, se escriben con doble clic, el "≡" abre su menú y el "○" las borra.
//!
//! Su esquina está en coordenadas de mundo (se mueven con la vista) y su tamaño en puntos de
//! interfaz (no cambia con el zoom). Cada gesto (mover, agrandar, escribir) es un paso de undo.

use cs_core::Note;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, StrokeKind, Vec2, pos2, vec2};

use crate::editor::Editor;
use crate::skin::{Skin, WidgetState};
use crate::{menus, pixel_ui};

const PIECE: &str = "note";
/// El papel de una nota nueva (el del diseño, sin la sombra) y el mínimo al agrandarla.
pub const NEW_SIZE: (u32, u32) = (86, 89);
const MIN_SIZE: Vec2 = vec2(60.0, 44.0);
/// La barra: sus 12 filas (con la línea de abajo), el "≡" y el "○" del diseño.
const BAR_H: f32 = 12.0;
const MENU: Rect = Rect {
    min: pos2(2.0, 1.0),
    max: pos2(12.0, 11.0),
};
/// El "○", contado desde la derecha de la nota (sin la sombra).
const CLOSE_FROM_RIGHT: f32 = 14.0;
const CLOSE_W: f32 = 10.0;
/// La sombra del diseño: 1 columna a la derecha y 2 filas abajo, fuera del papel.
const SHADOW: Vec2 = vec2(1.0, 2.0);
/// El texto: adentro del papel, debajo de la barra.
const TEXT_MARGIN: f32 = 5.0;
/// La esquina para agrandar.
const RESIZE: f32 = 10.0;
const PAPER: Color32 = Color32::from_rgb(248, 197, 58);
const INK: Color32 = Color32::from_rgb(5, 4, 3);

/// Qué parte de una nota está bajo el puntero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteHit {
    Bar,
    Menu,
    Close,
    Resize,
    Body,
}

#[derive(Clone, Copy, Debug)]
enum Gesture {
    Move,
    Resize,
}

/// Un arrastre en curso: la nota, qué se hace y desde dónde (pantalla y la nota de antes).
#[derive(Clone, Debug)]
struct Drag {
    id: u32,
    gesture: Gesture,
    from: Pos2,
    before: Note,
}

/// El estado de las notas en el lienzo.
#[derive(Default)]
pub struct NotesView {
    drag: Option<Drag>,
    /// La nota que se está escribiendo y su texto (se guarda al salir del campo).
    editing: Option<(u32, String)>,
    /// El cursor que piden las notas en este frame (encima de una, o arrastrando).
    pub cursor: Option<egui::CursorIcon>,
}

/// El papel de una nota en pantalla (sin la sombra): su esquina en el mundo y su tamaño en puntos.
fn paper(note: &Note, to_screen: &dyn Fn((i32, i32)) -> Pos2) -> Rect {
    let min = to_screen(note.position).round();
    Rect::from_min_size(min, vec2(note.size.0 as f32, note.size.1 as f32))
}

fn close_rect(paper: Rect) -> Rect {
    Rect::from_min_size(
        pos2(paper.right() - CLOSE_FROM_RIGHT, paper.top() + 1.0),
        vec2(CLOSE_W, BAR_H - 2.0),
    )
}

fn text_rect(paper: Rect) -> Rect {
    Rect::from_min_max(
        paper.min + vec2(TEXT_MARGIN, BAR_H + TEXT_MARGIN),
        paper.max - Vec2::splat(TEXT_MARGIN),
    )
}

/// Qué parte de `paper` (una nota en pantalla) está en `p`.
fn hit_part(paper: Rect, p: Pos2) -> Option<NoteHit> {
    if !paper.contains(p) {
        return None;
    }
    let local = p - paper.min;
    Some(if MENU.contains(local.to_pos2()) {
        NoteHit::Menu
    } else if close_rect(paper).contains(p) {
        NoteHit::Close
    } else if local.y < BAR_H {
        NoteHit::Bar
    } else if paper.right() - p.x <= RESIZE && paper.bottom() - p.y <= RESIZE {
        NoteHit::Resize
    } else {
        NoteHit::Body
    })
}

impl NotesView {
    /// La nota de más arriba bajo `p` y qué parte.
    pub fn hit(
        &self,
        editor: &Editor,
        to_screen: &dyn Fn((i32, i32)) -> Pos2,
        p: Pos2,
    ) -> Option<(u32, NoteHit)> {
        editor
            .document
            .notes
            .iter()
            .rev()
            .find_map(|n| hit_part(paper(n, to_screen), p).map(|h| (n.id, h)))
    }

    /// Si las notas tienen el puntero (encima de una o arrastrando): entonces las herramientas
    /// no reciben el clic.
    pub fn busy(&self) -> bool {
        self.drag.is_some()
    }

    /// La entrada de las notas: el cursor, mover desde la barra, agrandar desde la esquina, el
    /// doble clic para escribir y el "○". `zoom`: puntos por píxel del mundo (para mover).
    #[allow(clippy::too_many_arguments)]
    pub fn input(
        &mut self,
        editor: &mut Editor,
        to_screen: &dyn Fn((i32, i32)) -> Pos2,
        zoom: f32,
        pointer: Option<Pos2>,
        pressed: bool,
        down: bool,
        double_clicked: bool,
    ) {
        self.cursor = None;
        let hit = pointer.and_then(|p| self.hit(editor, to_screen, p));
        if let Some(drag) = self.drag.clone() {
            match pointer {
                Some(p) if down => {
                    let delta = p - drag.from;
                    let mut note = drag.before.clone();
                    match drag.gesture {
                        Gesture::Move => {
                            note.position = (
                                drag.before.position.0 + (delta.x / zoom).round() as i32,
                                drag.before.position.1 + (delta.y / zoom).round() as i32,
                            );
                            self.cursor = Some(egui::CursorIcon::Grabbing);
                        }
                        Gesture::Resize => {
                            let size = (vec2(drag.before.size.0 as f32, drag.before.size.1 as f32)
                                + delta)
                                .max(MIN_SIZE)
                                .round();
                            note.size = (size.x as u32, size.y as u32);
                            self.cursor = Some(egui::CursorIcon::ResizeNwSe);
                        }
                    }
                    // Mientras se arrastra, sin historia: se ve en vivo y al soltar es un paso.
                    if let Some(n) = editor.document.notes.iter_mut().find(|n| n.id == drag.id) {
                        *n = note;
                    }
                }
                _ => {
                    // Al soltar: el gesto entero en un paso de undo (desde la nota de antes).
                    let after = editor.document.notes.clone();
                    if let Some(n) = editor.document.notes.iter_mut().find(|n| n.id == drag.id) {
                        *n = drag.before.clone();
                    }
                    if after != editor.document.notes {
                        editor.set_notes(after);
                    }
                    self.drag = None;
                }
            }
            return;
        }
        let Some((id, part)) = hit else {
            return;
        };
        self.cursor = Some(match part {
            NoteHit::Bar => egui::CursorIcon::Grab,
            NoteHit::Menu | NoteHit::Close => egui::CursorIcon::PointingHand,
            NoteHit::Resize => egui::CursorIcon::ResizeNwSe,
            NoteHit::Body => egui::CursorIcon::Text,
        });
        if double_clicked && part == NoteHit::Body {
            self.start_editing(editor, id);
            return;
        }
        if !pressed {
            return;
        }
        let Some(before) = editor.document.notes.iter().find(|n| n.id == id).cloned() else {
            return;
        };
        let from = pointer.expect("hit has a pointer");
        match part {
            NoteHit::Bar => {
                self.finish_editing(editor);
                self.drag = Some(Drag {
                    id,
                    gesture: Gesture::Move,
                    from,
                    before,
                });
            }
            NoteHit::Resize => {
                self.finish_editing(editor);
                self.drag = Some(Drag {
                    id,
                    gesture: Gesture::Resize,
                    from,
                    before,
                });
            }
            NoteHit::Close => {
                self.finish_editing(editor);
                editor.remove_note(id);
            }
            NoteHit::Menu | NoteHit::Body => {}
        }
    }

    pub fn start_editing(&mut self, editor: &mut Editor, id: u32) {
        self.finish_editing(editor);
        if let Some(note) = editor.document.notes.iter().find(|n| n.id == id) {
            self.editing = Some((id, note.text.clone()));
        }
    }

    /// Guarda el texto que se estaba escribiendo (un paso de undo, si cambió).
    pub fn finish_editing(&mut self, editor: &mut Editor) {
        if let Some((id, text)) = self.editing.take() {
            editor.set_note_text(id, text);
        }
    }

    /// Dibuja las notas en `painter` y, si hay una en edición, su campo de texto en `ui`. Con
    /// `skin` (Pixel UI), la pieza del diseño. Devuelve, por nota, el rect de su "≡" (para su menú).
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        editor: &mut Editor,
        skin: Option<&Skin>,
        to_screen: &dyn Fn((i32, i32)) -> Pos2,
    ) -> Vec<(u32, Rect)> {
        let ppp = ui.ctx().pixels_per_point();
        let piece = skin.and_then(|s| s.piece(PIECE, WidgetState::Normal));
        let font = egui::TextStyle::Body.resolve(ui.style());
        let mut menus = Vec::new();
        for note in &editor.document.notes {
            let paper = paper(note, to_screen);
            if !painter.clip_rect().intersects(paper.expand(4.0)) {
                continue;
            }
            menus.push((note.id, MENU.translate(paper.min.to_vec2())));
            match piece {
                // La pieza trae la sombra: 1 columna y 2 filas más que el papel.
                Some(piece) => painter.extend(pixel_ui::piece_shapes(
                    piece,
                    Rect::from_min_max(paper.min, paper.max + SHADOW),
                    ppp,
                )),
                None => {
                    painter.rect_filled(
                        paper.translate(SHADOW),
                        2.0,
                        Color32::from_black_alpha(80),
                    );
                    painter.rect_filled(paper, 2.0, PAPER);
                    painter.rect_stroke(paper, 2.0, Stroke::new(1.0, INK), StrokeKind::Inside);
                    painter.text(
                        paper.min + vec2(4.0, 1.0),
                        egui::Align2::LEFT_TOP,
                        "≡",
                        font.clone(),
                        INK,
                    );
                    painter.text(
                        close_rect(paper).center(),
                        egui::Align2::CENTER_CENTER,
                        "○",
                        font.clone(),
                        INK,
                    );
                }
            }
            let editing = self.editing.as_ref().is_some_and(|(id, _)| *id == note.id);
            if !editing {
                let area = text_rect(paper);
                let galley =
                    painter.layout(note.text.clone(), font.clone(), INK, area.width().max(1.0));
                painter
                    .with_clip_rect(area.intersect(painter.clip_rect()))
                    .galley(area.min, galley, INK);
            }
        }
        // La nota en edición: un campo de texto sin marco sobre su papel.
        let editing = self
            .editing
            .as_ref()
            .and_then(|(id, _)| editor.document.notes.iter().find(|n| n.id == *id))
            .map(|n| text_rect(paper(n, to_screen)));
        if let (Some(area), Some((_, text))) = (editing, self.editing.as_mut()) {
            let edit = egui::TextEdit::multiline(text)
                .frame(egui::Frame::NONE)
                .text_color(INK)
                .desired_width(area.width())
                .id_salt("note_text");
            let response = ui.put(area, edit);
            if !response.has_focus() && !response.lost_focus() {
                response.request_focus();
            }
            let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
            if response.lost_focus() || escape {
                self.finish_editing(editor);
            }
        }
        menus
    }

    /// El menú del "≡" de cada nota (se abre al hacer clic, como el de los artboards).
    pub fn menus(&mut self, ui: &egui::Ui, editor: &mut Editor, menus: Vec<(u32, Rect)>) {
        for (id, rect) in menus {
            let response = ui.interact(rect, ui.id().with(("note_menu", id)), egui::Sense::click());
            let content = |ui: &mut egui::Ui| {
                if ui.add(menus::plain("Edit Text")).clicked() {
                    self.start_editing(editor, id);
                    ui.close();
                }
                if ui.add(menus::plain("Bring to Front")).clicked() {
                    editor.note_to_front(id);
                    ui.close();
                }
                menus::separator(ui);
                if ui.add(menus::plain("Delete")).clicked() {
                    self.finish_editing(editor);
                    editor.remove_note(id);
                    ui.close();
                }
            };
            match pixel_ui::frame_skin(ui.ctx()) {
                Some(skin) => {
                    let look = menus::MenuLook::new(&skin, rect.top());
                    menus::dropdown(&look, &response, rect, content);
                }
                None => {
                    egui::Popup::menu(&response).show(content);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_parts_of_a_note_like_the_design() {
        let paper = Rect::from_min_size(pos2(100.0, 50.0), vec2(87.0, 91.0));
        assert_eq!(hit_part(paper, pos2(105.0, 55.0)), Some(NoteHit::Menu));
        assert_eq!(hit_part(paper, pos2(178.0, 55.0)), Some(NoteHit::Close));
        assert_eq!(hit_part(paper, pos2(140.0, 55.0)), Some(NoteHit::Bar));
        assert_eq!(hit_part(paper, pos2(140.0, 100.0)), Some(NoteHit::Body));
        assert_eq!(hit_part(paper, pos2(183.0, 137.0)), Some(NoteHit::Resize));
        assert_eq!(hit_part(paper, pos2(99.0, 55.0)), None);
    }

    #[test]
    fn the_shape_of_the_shadow_matches_the_piece() {
        let ctx = egui::Context::default();
        let skin = Skin::embedded(&ctx);
        let piece = skin.piece(PIECE, WidgetState::Normal).expect("note piece");
        // La pieza es el papel de una nota nueva más la sombra.
        assert_eq!(
            vec2(piece.size[0] as f32, piece.size[1] as f32),
            vec2(NEW_SIZE.0 as f32, NEW_SIZE.1 as f32) + SHADOW
        );
    }
}
