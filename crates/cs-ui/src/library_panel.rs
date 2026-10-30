//! El panel Library del dock: los símbolos de la biblioteca. Arriba, la vista previa animada del
//! elegido; abajo, la lista con su miniatura, su nombre y cuántas instancias lo usan. Cada fila se
//! renombra con F2, tiene su menú (Edit Symbol, Rename, Duplicate, Place on Canvas, Delete) y se
//! arrastra al lienzo (ver [`SymbolPayload`]).

use std::collections::HashMap;

use cs_core::{ArtboardId, Document, Sprite};
use cs_io::export::Bitmap;
use eframe::egui::{self, Color32, Rect, Sense, TextureHandle, Ui, pos2, vec2};

use crate::editor::Editor;
use crate::menus;
use crate::panel_kit::{box_color, child, label_with_hint, notched_outline};
use crate::pixel_ui;
use crate::skin::Skin;

/// Ancho del contenido: el mismo que Groom y Lumen.
pub const WIDTH: f32 = crate::groom::WIDTH;
/// Alto de la caja de la vista previa.
const PREVIEW_HEIGHT: f32 = 96.0;
/// Alto de cada fila de la lista.
const ROW_HEIGHT: f32 = 20.0;
/// Lado de la miniatura de cada fila.
const THUMB: f32 = 16.0;
/// Aire a la izquierda de la miniatura y entre ella y el nombre.
const ROW_PAD: f32 = 4.0;

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryAction {
    Edit(ArtboardId),
    PlaceOnCanvas(ArtboardId),
    Delete(ArtboardId),
    Duplicate(ArtboardId),
    Rename(ArtboardId, String),
}

/// Lo que se arrastra desde una fila: el símbolo (lo suelta el lienzo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SymbolPayload(pub ArtboardId);

/// El estado del panel entre frames.
#[derive(Default)]
pub struct LibraryState {
    /// La fila que se está renombrando y lo escrito.
    pub renaming: Option<(ArtboardId, String)>,
    /// El símbolo con instancias que se pidió borrar (espera la confirmación).
    pub confirm_delete: Option<ArtboardId>,
    /// Darle el foco al campo del nombre en el próximo frame.
    focus_rename: bool,
    /// La textura de la vista previa: de qué símbolo, frame y versión del documento.
    preview: Option<(PreviewKey, TextureHandle)>,
    /// Las miniaturas, con la versión del documento de la que salieron.
    thumbs: HashMap<ArtboardId, (u64, TextureHandle)>,
    /// Las filas de la lista y la versión del documento de la que salieron (no se recalculan
    /// mientras solo se anima la vista previa).
    rows: Option<(u64, Vec<Row>)>,
}

/// Una fila de la lista: id, nombre y usos.
type Row = (ArtboardId, String, usize);

impl LibraryState {
    /// Las filas de `editor`, de la memoria si el documento no cambió.
    fn rows(&mut self, editor: &Editor) -> Vec<Row> {
        let version = editor.doc_version;
        if let Some((v, rows)) = &self.rows
            && *v == version
        {
            return rows.clone();
        }
        let rows = library_rows(&editor.document);
        self.rows = Some((version, rows.clone()));
        rows
    }

    /// Olvida lo que apunta a símbolos que ya no están en `rows` (un undo o un borrado): el
    /// renombrado, la confirmación y las miniaturas.
    fn prune(&mut self, rows: &[Row]) {
        let present = |id: ArtboardId| rows.iter().any(|(r, _, _)| *r == id);
        if self.renaming.as_ref().is_some_and(|(id, _)| !present(*id)) {
            self.renaming = None;
            self.focus_rename = false;
        }
        if self.confirm_delete.is_some_and(|id| !present(id)) {
            self.confirm_delete = None;
        }
        self.thumbs.retain(|id, _| present(*id));
    }
}

/// De qué es la textura de la vista previa: símbolo, frame y versión del documento.
type PreviewKey = (ArtboardId, usize, u64);

/// Las filas de la lista: (id, nombre, usos), ordenadas por nombre (sin distinguir mayúsculas).
pub fn library_rows(doc: &Document) -> Vec<Row> {
    let mut rows: Vec<_> = doc
        .library()
        .map(|a| (a.id, a.name.clone(), doc.instance_uses(a.id)))
        .collect();
    rows.sort_by_cached_key(|(_, name, _)| name.to_lowercase());
    rows
}

/// El frame de la vista previa en `t_ms` milisegundos, en loop con las duraciones del símbolo.
pub fn preview_frame(sprite: &Sprite, t_ms: u64) -> usize {
    let total: u64 = sprite.frames.iter().map(|f| f.duration_ms as u64).sum();
    if total == 0 {
        return 0;
    }
    let mut t = t_ms % total;
    for (i, frame) in sprite.frames.iter().enumerate() {
        let d = frame.duration_ms as u64;
        if t < d {
            return i;
        }
        t -= d;
    }
    0
}

/// La escala para mostrar `size` en `room`: el mayor zoom entero que cabe o, si no cabe ni al
/// 100 %, lo que haga falta para que quepa.
fn fit_scale(size: (u32, u32), room: egui::Vec2) -> f32 {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let fit = (room.x / w).min(room.y / h);
    if fit >= 1.0 { fit.floor() } else { fit }
}

/// El frame `frame` de `sprite` como imagen (con sus instancias resueltas).
fn render(doc: &Document, sprite: &Sprite, frame: usize) -> Bitmap {
    let shown = cs_render::materialize(doc, sprite, frame..frame + 1);
    Bitmap {
        width: sprite.width,
        height: sprite.height,
        pixels: cs_render::render_frame(&shown, frame),
    }
}

/// `texture` de `size` centrada en `area`, a la escala de [`fit_scale`].
fn paint_fitted(ui: &Ui, texture: &TextureHandle, size: (u32, u32), area: Rect) {
    let k = fit_scale(size, area.size());
    let shown = vec2(size.0 as f32, size.1 as f32) * k;
    let rect = Rect::from_min_size((area.center() - shown / 2.0).round(), shown);
    let full = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    ui.painter_at(area)
        .image(texture.id(), rect, full, Color32::WHITE);
}

/// Dibuja el panel; devuelve lo que el usuario pidió.
pub fn show(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut LibraryState,
) -> Option<LibraryAction> {
    let mut action = None;
    let rows = state.rows(editor);
    state.prune(&rows);
    // Lo elegido tiene que seguir en la biblioteca (lo pudo sacar un undo).
    if editor
        .library_selected
        .is_some_and(|id| !rows.iter().any(|(r, _, _)| *r == id))
    {
        editor.library_selected = None;
    }
    let count = match rows.len() {
        1 => "1 symbol".to_string(),
        n => format!("{n} symbols"),
    };
    label_with_hint(ui, "Library", &count);
    ui.add_space(4.0);
    preview(ui, editor, state);
    ui.add_space(6.0);
    if rows.is_empty() {
        ui.add(egui::Label::new("Select layers or pixels and press F8 to make a symbol.").wrap());
        return None;
    }
    // F2 renombra la fila elegida (si nadie más tiene el foco del teclado).
    if state.renaming.is_none()
        && let Some(id) = editor.library_selected
        && ui.memory(|m| m.focused().is_none())
        && ui.input(|i| i.key_pressed(egui::Key::F2))
    {
        start_rename(state, &rows, id);
    }
    for (id, name, uses) in &rows {
        if let Some(a) = row(ui, editor, skin, state, (*id, name, *uses)) {
            action = Some(a);
        }
    }
    if let Some(id) = state.confirm_delete
        && let Some(a) = confirm_delete(ui, editor, state, id)
    {
        action = Some(a);
    }
    action
}

/// Empieza a renombrar `id` con su nombre actual.
fn start_rename(state: &mut LibraryState, rows: &[Row], id: ArtboardId) {
    if let Some((_, name, _)) = rows.iter().find(|(r, _, _)| *r == id) {
        state.renaming = Some((id, name.clone()));
        state.focus_rename = true;
    }
}

/// La caja de la vista previa: el damero y el símbolo elegido, animado.
fn preview(ui: &mut Ui, editor: &Editor, state: &mut LibraryState) {
    let width = ui.max_rect().width();
    let (frame, _) = ui.allocate_exact_size(vec2(width, PREVIEW_HEIGHT), Sense::hover());
    ui.painter()
        .extend(notched_outline(frame, box_color(ui), 1.0));
    let well = frame.shrink(2.0);
    crate::panels::checker(&ui.painter_at(well), well);
    let doc = &editor.document;
    let Some(symbol) = editor.library_selected.and_then(|id| doc.artboard(id)) else {
        state.preview = None;
        return;
    };
    let sprite = &symbol.sprite;
    let t_ms = (ui.input(|i| i.time) * 1000.0) as u64;
    let f = preview_frame(sprite, t_ms);
    let key = (symbol.id, f, editor.doc_version);
    if !matches!(&state.preview, Some((k, _)) if *k == key) {
        let image = render(doc, sprite, f);
        let texture = crate::lumen::texture_of(ui, "library_preview", &image);
        state.preview = Some((key, texture));
    }
    if let Some((_, texture)) = &state.preview {
        paint_fitted(ui, texture, (sprite.width, sprite.height), well);
    }
    if sprite.frames.len() > 1 {
        ui.ctx().request_repaint();
    }
}

/// La miniatura de `id` (su frame 0), hecha de nuevo si cambió el documento.
fn thumb(
    ui: &Ui,
    editor: &Editor,
    state: &mut LibraryState,
    id: ArtboardId,
) -> Option<TextureHandle> {
    let version = editor.doc_version;
    if let Some((v, t)) = state.thumbs.get(&id)
        && *v == version
    {
        return Some(t.clone());
    }
    let sprite = &editor.document.artboard(id)?.sprite;
    let image = render(&editor.document, sprite, 0);
    let texture = crate::lumen::texture_of(ui, &format!("library_thumb_{}", id.0), &image);
    state.thumbs.insert(id, (version, texture.clone()));
    Some(texture)
}

/// Una fila de la lista: miniatura, nombre y usos. Devuelve lo que se pidió en ella.
fn row(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut LibraryState,
    (id, name, uses): (ArtboardId, &str, usize),
) -> Option<LibraryAction> {
    let mut action = None;
    let width = ui.max_rect().width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::click_and_drag());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    let selected = editor.library_selected == Some(id);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
    });
    let (text, dim, selection) = match skin {
        Some(skin) => {
            let c = skin.colors();
            (c.text, c.text_dim, c.selection)
        }
        None => {
            let v = ui.visuals();
            (v.text_color(), v.weak_text_color(), v.selection.bg_fill)
        }
    };
    if selected {
        ui.painter().rect_filled(rect, 0.0, selection);
    } else if response.hovered() {
        let hover = ui.visuals().widgets.hovered.weak_bg_fill;
        ui.painter().rect_filled(rect, 0.0, hover);
    }
    // La miniatura, sobre el damero.
    let thumb_rect = Rect::from_min_size(
        pos2(rect.left() + ROW_PAD, rect.center().y - THUMB / 2.0).round(),
        vec2(THUMB, THUMB),
    );
    crate::panels::checker(&ui.painter_at(thumb_rect), thumb_rect);
    if let Some(texture) = thumb(ui, editor, state, id)
        && let Some(a) = editor.document.artboard(id)
    {
        let size = (a.sprite.width, a.sprite.height);
        paint_fitted(ui, &texture, size, thumb_rect);
    }
    let text_left = thumb_rect.right() + ROW_PAD + 2.0;
    let font = egui::TextStyle::Body.resolve(ui.style());
    // Los usos, a la derecha.
    let mut text_right = rect.right() - ROW_PAD;
    if uses > 0 {
        let galley = ui
            .painter()
            .layout_no_wrap(format!("×{uses}"), font.clone(), dim);
        let at = pos2(
            text_right - galley.size().x,
            rect.center().y - galley.size().y / 2.0,
        );
        text_right = at.x - ROW_PAD;
        ui.painter().galley(at.round(), galley, dim);
    }
    let renaming = matches!(&state.renaming, Some((r, _)) if *r == id);
    if renaming {
        let field =
            Rect::from_min_max(pos2(text_left, rect.top()), pos2(text_right, rect.bottom()));
        let mut field_ui = child(ui, field, egui::Layout::left_to_right(egui::Align::Center));
        field_ui.spacing_mut().text_edit_width = field.width();
        let mut done = None;
        if let Some((_, buffer)) = &mut state.renaming {
            let edit = crate::widgets::text_line_named(&mut field_ui, buffer, "Symbol name");
            if state.focus_rename {
                edit.request_focus();
                state.focus_rename = false;
            } else if edit.lost_focus() {
                // Enter (o salir del campo) renombra; Esc cancela.
                let cancelled = ui.input(|i| i.key_pressed(egui::Key::Escape));
                let new = buffer.trim().to_string();
                done = Some((!cancelled && !new.is_empty() && new != name).then_some(new));
            }
        }
        if let Some(result) = done {
            state.renaming = None;
            if let Some(new) = result {
                action = Some(LibraryAction::Rename(id, new));
            }
        }
    } else {
        // En una línea: lo que no cabe termina en "…".
        let mut job = egui::text::LayoutJob::simple_singleline(name.to_string(), font, text);
        job.wrap = egui::text::TextWrapping::truncate_at_width((text_right - text_left).max(0.0));
        let galley = ui.painter().layout_job(job);
        let at = pos2(text_left, rect.center().y - galley.size().y / 2.0);
        ui.painter_at(rect).galley(at.round(), galley, text);
    }
    response.dnd_set_drag_payload(SymbolPayload(id));
    if response.clicked() || response.secondary_clicked() || response.drag_started() {
        editor.library_selected = Some(id);
    }
    if response.double_clicked() {
        action = Some(LibraryAction::Edit(id));
    }
    menus::context_menu(&response, |ui| {
        if ui.add(menus::plain("Edit Symbol")).clicked() {
            action = Some(LibraryAction::Edit(id));
            ui.close();
        }
        if ui.add(menus::plain("Rename")).clicked() {
            state.renaming = Some((id, name.to_string()));
            state.focus_rename = true;
            ui.close();
        }
        if ui.add(menus::plain("Duplicate")).clicked() {
            action = Some(LibraryAction::Duplicate(id));
            ui.close();
        }
        if ui.add(menus::plain("Place on Canvas")).clicked() {
            action = Some(LibraryAction::PlaceOnCanvas(id));
            ui.close();
        }
        if ui.add(menus::plain("Delete")).clicked() {
            if uses > 0 {
                state.confirm_delete = Some(id);
            } else {
                action = Some(LibraryAction::Delete(id));
            }
            ui.close();
        }
    });
    action
}

/// Abajo de la lista: borrar un símbolo con instancias pide confirmación.
fn confirm_delete(
    ui: &mut Ui,
    editor: &Editor,
    state: &mut LibraryState,
    id: ArtboardId,
) -> Option<LibraryAction> {
    if editor.document.artboard(id).is_none() {
        state.confirm_delete = None;
        return None;
    }
    let uses = editor.document.instance_uses(id);
    ui.add_space(6.0);
    let noun = if uses == 1 { "instance" } else { "instances" };
    ui.add(egui::Label::new(format!("Used by {uses} {noun}. Delete anyway?")).wrap());
    let mut action = None;
    ui.horizontal(|ui| {
        if pixel_ui::text_button(ui, "Delete").clicked() {
            action = Some(LibraryAction::Delete(id));
            state.confirm_delete = None;
        }
        if pixel_ui::text_button(ui, "Cancel").clicked() {
            state.confirm_delete = None;
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Frame};

    use super::*;

    #[test]
    fn rows_are_sorted_by_name_with_their_uses() {
        let mut doc = Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba));
        for (id, name) in [(2, "walk"), (3, "Coin")] {
            let mut a = doc.artboards[0].clone();
            a.id = ArtboardId(id);
            a.name = name.into();
            a.in_library = true;
            doc.artboards.push(a);
        }
        let cmd = cs_commands::ops::new_instance_layer(
            &doc.artboards[0].sprite,
            None,
            0,
            ArtboardId(3),
            "c",
            (0, 0),
        );
        cmd.apply(&mut doc.artboards[0].sprite).unwrap();
        assert_eq!(
            library_rows(&doc),
            vec![
                (ArtboardId(3), "Coin".to_string(), 1),
                (ArtboardId(2), "walk".to_string(), 0)
            ]
        );
    }

    #[test]
    fn preview_loops_with_the_symbol_durations() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        s.frames = vec![Frame { duration_ms: 100 }, Frame { duration_ms: 50 }];
        assert_eq!(preview_frame(&s, 0), 0);
        assert_eq!(preview_frame(&s, 99), 0);
        assert_eq!(preview_frame(&s, 100), 1);
        assert_eq!(preview_frame(&s, 150), 0);
    }

    #[test]
    fn a_symbol_without_durations_previews_its_first_frame() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        s.frames = vec![Frame { duration_ms: 0 }, Frame { duration_ms: 0 }];
        assert_eq!(preview_frame(&s, 1234), 0);
    }

    #[test]
    fn what_points_to_a_symbol_that_is_gone_is_forgotten() {
        let mut state = LibraryState {
            renaming: Some((ArtboardId(2), "walk".into())),
            confirm_delete: Some(ArtboardId(2)),
            ..Default::default()
        };
        state.prune(&[(ArtboardId(2), "walk".into(), 1)]);
        assert!(state.renaming.is_some() && state.confirm_delete.is_some());
        state.prune(&[(ArtboardId(3), "Coin".into(), 0)]);
        assert_eq!(state.renaming, None);
        assert_eq!(state.confirm_delete, None);
    }

    #[test]
    fn the_preview_zooms_by_whole_steps_and_shrinks_what_does_not_fit() {
        assert_eq!(fit_scale((8, 8), vec2(92.0, 92.0)), 11.0);
        assert_eq!(fit_scale((10, 4), vec2(92.0, 92.0)), 9.0);
        assert_eq!(fit_scale((32, 16), vec2(16.0, 16.0)), 0.5);
    }
}
