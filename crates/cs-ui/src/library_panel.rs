//! El panel Library del dock: los símbolos, los modelos 3D y las fuentes del documento. Arriba, la
//! vista previa animada de lo elegido; abajo, tres secciones (Symbols, Models, Fonts) con su
//! cuenta y, en cada fila, su miniatura, su nombre y cuántas instancias la usan (las fuentes de
//! una familia van juntas bajo su nombre, ver [`font_entries`]). Cada fila se
//! renombra con F2, tiene su menú (Edit Symbol u Open, Rename, Duplicate, Place on Canvas,
//! Delete) y los símbolos y modelos se arrastran al lienzo (ver [`SymbolPayload`]). El doble clic
//! entra al símbolo o abre la pestaña del modelo o de la fuente.

use std::collections::HashMap;

use cs_core::{Artboard, ArtboardId, Document, Rgba, Sprite};
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
/// Alto del encabezado de una familia de fuentes y cuánto se corren a la derecha sus filas.
const FAMILY_HEIGHT: f32 = 14.0;
const FAMILY_INDENT: f32 = 8.0;

/// Lo que el usuario pidió en el panel (lo resuelve la app).
#[derive(Clone, Debug, PartialEq)]
pub enum LibraryAction {
    Edit(ArtboardId),
    /// Abrir la pestaña de vista de un modelo o una fuente.
    Open(ArtboardId),
    PlaceOnCanvas(ArtboardId),
    Delete(ArtboardId),
    Duplicate(ArtboardId),
    Rename(ArtboardId, String),
    /// Library → Fonts → Import…: elegir un TTF, OTF o JSON de BitFontMaker2.
    ImportFont,
}

/// Lo que se arrastra desde una fila: el símbolo o el modelo (lo suelta el lienzo).
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
    preview: Option<(PreviewKey, TextureHandle, (u32, u32))>,
    /// Las miniaturas, con la versión del documento de la que salieron.
    thumbs: HashMap<ArtboardId, (u64, TextureHandle, (u32, u32))>,
    /// Las filas de las secciones y la versión del documento de la que salieron (no se
    /// recalculan mientras solo se anima la vista previa).
    rows: Option<(u64, Sections)>,
}

/// Una fila de la lista: id, nombre y usos.
type Row = (ArtboardId, String, usize);

/// Qué es lo que muestra una fila.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Symbol,
    Model,
    Font,
}

/// Una entrada de la sección Fonts: una fuente suelta (la única de su familia) o una familia con
/// sus fuentes, cada una con el nombre de su estilo (lo que se ve en su fila).
#[derive(Clone, Debug, PartialEq)]
enum FontEntry {
    Font(Row),
    Family(String, Vec<(Row, String)>),
}

/// Las filas de las tres secciones.
#[derive(Clone, Default)]
struct Sections {
    symbols: Vec<Row>,
    models: Vec<Row>,
    fonts: Vec<FontEntry>,
}

impl Sections {
    fn of(doc: &Document) -> Sections {
        Sections {
            symbols: library_rows(doc),
            models: rows_sorted(doc, doc.models()),
            fonts: font_entries(doc),
        }
    }

    /// Las filas de las fuentes, de arriba a abajo.
    fn font_rows(&self) -> impl Iterator<Item = &Row> {
        self.fonts.iter().flat_map(|e| {
            let (alone, members) = match e {
                FontEntry::Font(row) => (Some(row), &[][..]),
                FontEntry::Family(_, members) => (None, &members[..]),
            };
            alone.into_iter().chain(members.iter().map(|(row, _)| row))
        })
    }

    /// Cuántas fuentes hay (la cuenta del título Fonts).
    fn font_count(&self) -> usize {
        self.font_rows().count()
    }

    /// Todas las filas, de arriba a abajo.
    fn all(&self) -> Vec<Row> {
        self.symbols
            .iter()
            .chain(&self.models)
            .chain(self.font_rows())
            .cloned()
            .collect()
    }
}

/// La sección Fonts: las fuentes de una misma familia (las que comparten su nombre de familia),
/// juntas bajo ella, la Regular primero y luego por peso e itálica; las solas de su familia,
/// sueltas. Familias y sueltas van por nombre (sin distinguir mayúsculas).
fn font_entries(doc: &Document) -> Vec<FontEntry> {
    let mut families: Vec<(String, Vec<&Artboard>)> = Vec::new();
    for a in doc.fonts() {
        let Some(font) = &a.font else { continue };
        match families.iter_mut().find(|(f, _)| *f == font.name) {
            Some((_, members)) => members.push(a),
            None => families.push((font.name.clone(), vec![a])),
        }
    }
    let row = |a: &Artboard| (a.id, a.name.clone(), doc.instance_uses(a.id));
    let mut entries: Vec<(String, FontEntry)> = families
        .into_iter()
        .map(|(family, mut members)| {
            if let [a] = members[..] {
                return (a.name.to_lowercase(), FontEntry::Font(row(a)));
            }
            members.sort_by_cached_key(|a| {
                let s = &a.font.as_ref().expect("a font").style;
                let regular = s.name.trim().eq_ignore_ascii_case("Regular");
                (!regular, s.weight, s.italic, a.name.to_lowercase())
            });
            let members = members
                .into_iter()
                .map(|a| {
                    let style = a.font.as_ref().expect("a font").style.name.trim();
                    let shown = if style.is_empty() { &a.name } else { style };
                    (row(a), shown.to_owned())
                })
                .collect();
            (family.to_lowercase(), FontEntry::Family(family, members))
        })
        .collect();
    entries.sort_by(|(a, _), (b, _)| a.cmp(b));
    entries.into_iter().map(|(_, e)| e).collect()
}

impl LibraryState {
    /// Las filas de `editor`, de la memoria si el documento no cambió.
    fn rows(&mut self, editor: &Editor) -> Sections {
        let version = editor.doc_version;
        if let Some((v, rows)) = &self.rows
            && *v == version
        {
            return rows.clone();
        }
        let rows = Sections::of(&editor.document);
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

/// De qué es la textura de la vista previa: artboard, frame y versión del documento.
type PreviewKey = (ArtboardId, usize, u64);

/// Las filas de la lista: (id, nombre, usos), ordenadas por nombre (sin distinguir mayúsculas).
pub fn library_rows(doc: &Document) -> Vec<Row> {
    rows_sorted(doc, doc.library())
}

/// Las filas de `boards`, ordenadas por nombre (sin distinguir mayúsculas).
fn rows_sorted<'a>(doc: &Document, boards: impl Iterator<Item = &'a Artboard>) -> Vec<Row> {
    let mut rows: Vec<_> = boards
        .map(|a| (a.id, a.name.clone(), doc.instance_uses(a.id)))
        .collect();
    rows.sort_by_cached_key(|(_, name, _)| name.to_lowercase());
    rows
}

/// El frame de la vista previa en `t_ms` milisegundos, en loop con las duraciones del símbolo, y
/// los milisegundos que le quedan (`None` si no anima).
fn preview_frame(sprite: &Sprite, t_ms: u64) -> (usize, Option<u64>) {
    let total: u64 = sprite.frames.iter().map(|f| f.duration_ms as u64).sum();
    if total == 0 || sprite.frames.len() < 2 {
        return (0, None);
    }
    let mut t = t_ms % total;
    for (i, frame) in sprite.frames.iter().enumerate() {
        let d = frame.duration_ms as u64;
        if t < d {
            return (i, Some(d - t));
        }
        t -= d;
    }
    (0, None)
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

/// El texto de la miniatura de una fuente.
const FONT_THUMB_SAMPLE: &str = "Ab";

/// La tinta de las fuentes de 1 bit.
const FONT_INK: Rgba = Rgba {
    r: 5,
    g: 4,
    b: 3,
    a: 255,
};

/// La imagen de `a` en su frame `frame`: un símbolo, su sprite; un modelo, su render; una fuente,
/// `sample` escrito con ella. `None` si no hay nada que mostrar (una fuente sin tinta).
fn image_of(doc: &Document, a: &Artboard, frame: usize, sample: &str) -> Option<Bitmap> {
    if a.model.is_some() {
        let sprite = cs_render::model_sprite(doc, a, frame)?;
        return Some(render(doc, &sprite, 0));
    }
    if let Some(view) = a.font_view() {
        let text = cs_tools::text::font_text(&view, sample, 1);
        let xs = || text.iter().map(|&(x, _, _)| x);
        let ys = || text.iter().map(|&(_, y, _)| y);
        let (x0, x1, y0, y1) = (xs().min()?, xs().max()?, ys().min()?, ys().max()?);
        let mut image = Bitmap::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        for (x, y, c) in text {
            let at = ((y - y0) as u32 * image.width + (x - x0) as u32) as usize;
            image.pixels[at] = c.unwrap_or(FONT_INK);
        }
        return Some(image);
    }
    Some(render(doc, &a.sprite, frame))
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
    let sections = state.rows(editor);
    let rows = sections.all();
    state.prune(&rows);
    // Lo elegido tiene que seguir en la biblioteca (lo pudo sacar un undo).
    if editor
        .library_selected
        .is_some_and(|id| !rows.iter().any(|(r, _, _)| *r == id))
    {
        editor.library_selected = None;
    }
    label_with_hint(ui, "Library", "");
    ui.add_space(4.0);
    preview(ui, editor, state);
    ui.add_space(6.0);
    // F2 renombra la fila elegida (si nadie más tiene el foco del teclado).
    if state.renaming.is_none()
        && let Some(id) = editor.library_selected
        && ui.memory(|m| m.focused().is_none())
        && ui.input(|i| i.key_pressed(egui::Key::F2))
    {
        start_rename(state, &rows, id);
    }
    let parts = [
        (
            "Symbols",
            Kind::Symbol,
            sections.symbols.len(),
            "F8 makes a symbol",
        ),
        (
            "Models",
            Kind::Model,
            sections.models.len(),
            "File → New 3D Model…",
        ),
        (
            "Fonts",
            Kind::Font,
            sections.font_count(),
            "File → New Font…",
        ),
    ];
    for (title, kind, count, help) in parts {
        label_with_hint(ui, title, &count.to_string());
        ui.add_space(2.0);
        if count == 0 {
            ui.add(egui::Label::new(help).wrap());
        }
        let mut show_row = |ui: &mut Ui, (id, name, uses): &Row, shown: &str, indent: f32| {
            let look = (shown, indent);
            if let Some(a) = row(ui, editor, skin, state, kind, (*id, name, *uses), look) {
                action = Some(a);
            }
        };
        match kind {
            Kind::Symbol => sections
                .symbols
                .iter()
                .for_each(|r| show_row(ui, r, &r.1, 0.0)),
            Kind::Model => sections
                .models
                .iter()
                .for_each(|r| show_row(ui, r, &r.1, 0.0)),
            // Las fuentes de una familia, bajo su nombre y con el de su estilo.
            Kind::Font => {
                for entry in &sections.fonts {
                    match entry {
                        FontEntry::Font(r) => show_row(ui, r, &r.1, 0.0),
                        FontEntry::Family(family, members) => {
                            family_header(ui, skin, family);
                            for (r, style) in members {
                                show_row(ui, r, style, FAMILY_INDENT);
                            }
                        }
                    }
                }
            }
        }
        // Las fuentes también entran de afuera: TTF, OTF o el JSON de BitFontMaker2.
        if kind == Kind::Font {
            ui.add_space(2.0);
            let import = match skin {
                Some(skin) => crate::kit::button::button(ui, skin, "Import…"),
                None => pixel_ui::text_button(ui, "Import…"),
            };
            if import
                .on_hover_text("A TTF, OTF or BitFontMaker2 JSON font")
                .clicked()
            {
                action = Some(LibraryAction::ImportFont);
            }
        }
        ui.add_space(6.0);
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
    let (f, rest) = preview_frame(sprite, t_ms);
    let key = (symbol.id, f, editor.doc_version);
    if !matches!(&state.preview, Some((k, ..)) if *k == key) {
        state.preview =
            image_of(doc, symbol, f, crate::artboard_window::CARD_SAMPLE).map(|image| {
                let texture = crate::lumen::texture_of(ui, "library_preview", &image);
                (key, texture, (image.width, image.height))
            });
    }
    if let Some((_, texture, size)) = &state.preview {
        paint_fitted(ui, texture, *size, well);
    }
    // Se vuelve a pintar cuando cambia el frame, no en cada frame de la pantalla.
    if let Some(rest) = rest {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(rest));
    }
}

/// La miniatura de `id` (su frame 0) y su tamaño, hecha de nuevo si cambió el documento.
fn thumb(
    ui: &Ui,
    editor: &Editor,
    state: &mut LibraryState,
    id: ArtboardId,
) -> Option<(TextureHandle, (u32, u32))> {
    let version = editor.doc_version;
    if let Some((v, t, size)) = state.thumbs.get(&id)
        && *v == version
    {
        return Some((t.clone(), *size));
    }
    let a = editor.document.artboard(id)?;
    let image = image_of(&editor.document, a, 0, FONT_THUMB_SAMPLE)?;
    let size = (image.width, image.height);
    let texture = crate::lumen::texture_of(ui, &format!("library_thumb_{}", id.0), &image);
    state.thumbs.insert(id, (version, texture.clone(), size));
    Some((texture, size))
}

/// El encabezado de una familia de fuentes: su nombre, en el gris de las pistas, sobre sus
/// filas (corridas a la derecha).
fn family_header(ui: &mut Ui, skin: Option<&Skin>, family: &str) {
    let width = ui.max_rect().width();
    let (rect, response) = ui.allocate_exact_size(vec2(width, FAMILY_HEIGHT), Sense::hover());
    let rect = Rect::from_min_size(rect.min.round(), rect.size());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, family));
    let dim = match skin {
        Some(skin) => skin.colors().text_dim,
        None => ui.visuals().weak_text_color(),
    };
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::simple_singleline(family.to_string(), font, dim);
    job.wrap = egui::text::TextWrapping::truncate_at_width((rect.width() - 2.0 * ROW_PAD).max(0.0));
    let galley = ui.painter().layout_job(job);
    let at = pos2(
        rect.left() + ROW_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter_at(rect).galley(at.round(), galley, dim);
}

/// Una fila de la lista: miniatura, nombre y usos. `shown` es lo que dice (el nombre, o el
/// estilo bajo su familia) e `indent` cuánto se corre. Devuelve lo que se pidió en ella.
fn row(
    ui: &mut Ui,
    editor: &mut Editor,
    skin: Option<&Skin>,
    state: &mut LibraryState,
    kind: Kind,
    (id, name, uses): (ArtboardId, &str, usize),
    (shown, indent): (&str, f32),
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
        pos2(
            rect.left() + ROW_PAD + indent,
            rect.center().y - THUMB / 2.0,
        )
        .round(),
        vec2(THUMB, THUMB),
    );
    crate::panels::checker(&ui.painter_at(thumb_rect), thumb_rect);
    if let Some((texture, size)) = thumb(ui, editor, state, id) {
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
            let edit = crate::widgets::text_line_named(
                &mut field_ui,
                buffer,
                match kind {
                    Kind::Symbol => "Symbol name",
                    Kind::Model => "Model name",
                    Kind::Font => "Font name",
                },
            );
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
        let mut job = egui::text::LayoutJob::simple_singleline(shown.to_string(), font, text);
        job.wrap = egui::text::TextWrapping::truncate_at_width((text_right - text_left).max(0.0));
        let galley = ui.painter().layout_job(job);
        let at = pos2(text_left, rect.center().y - galley.size().y / 2.0);
        ui.painter_at(rect).galley(at.round(), galley, text);
    }
    if kind != Kind::Font {
        response.dnd_set_drag_payload(SymbolPayload(id));
    }
    if response.clicked() || response.secondary_clicked() || response.drag_started() {
        editor.library_selected = Some(id);
    }
    if response.double_clicked() {
        action = Some(match kind {
            Kind::Symbol => LibraryAction::Edit(id),
            _ => LibraryAction::Open(id),
        });
    }
    menus::context_menu(&response, |ui| {
        if kind == Kind::Symbol {
            if ui.add(menus::plain("Edit Symbol")).clicked() {
                action = Some(LibraryAction::Edit(id));
                ui.close();
            }
        } else if ui.add(menus::plain("Open")).clicked() {
            action = Some(LibraryAction::Open(id));
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
        if kind != Kind::Font && ui.add(menus::plain("Place on Canvas")).clicked() {
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
    let noun = if uses == 1 { "object" } else { "objects" };
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
        let cmd = cs_commands::ops::new_object_layer(
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

    /// Un documento con fuentes de (artboard, familia, estilo, peso, itálica).
    fn with_fonts(fonts: &[(u32, &str, &str, u16, bool)]) -> Document {
        let mut doc = Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba));
        for &(id, family, style, weight, italic) in fonts {
            let mut a = doc.artboards[0].clone();
            a.id = ArtboardId(id);
            let font = cs_core::FontInfo {
                style: cs_core::FontStyle {
                    name: style.into(),
                    weight,
                    italic,
                    ..Default::default()
                },
                ..cs_core::FontInfo::new(family, 8, cs_core::FontMode::OneBit)
            };
            a.name = font.full_name();
            a.font = Some(font);
            doc.artboards.push(a);
        }
        doc
    }

    #[test]
    fn fonts_of_a_family_go_under_it_regular_first_and_lone_ones_stay_loose() {
        let doc = with_fonts(&[
            (2, "Mine", "Bold Italic", 700, true),
            (3, "zed", "Regular", 400, false),
            (4, "Mine", "Light", 300, false),
            (5, "Mine", "Bold", 700, false),
            (6, "Mine", "Regular", 400, false),
            (7, "Ace", "Bold", 700, false),
            (8, "Mine", "Italic", 400, true),
        ]);
        let row = |id: u32, name: &str| (ArtboardId(id), name.to_string(), 0);
        assert_eq!(
            font_entries(&doc),
            vec![
                // Una sola fuente de su familia (aunque sea Bold) va suelta, con su nombre.
                FontEntry::Font(row(7, "Ace Bold")),
                FontEntry::Family(
                    "Mine".into(),
                    vec![
                        (row(6, "Mine"), "Regular".into()),
                        (row(4, "Mine Light"), "Light".into()),
                        (row(8, "Mine Italic"), "Italic".into()),
                        (row(5, "Mine Bold"), "Bold".into()),
                        (row(2, "Mine Bold Italic"), "Bold Italic".into()),
                    ]
                ),
                FontEntry::Font(row(3, "zed")),
            ]
        );
        let sections = Sections::of(&doc);
        assert_eq!(sections.font_count(), 7);
        assert_eq!(sections.all().len(), 7, "every font is a row");
    }

    #[test]
    fn preview_loops_with_the_symbol_durations() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        s.frames = vec![Frame { duration_ms: 100 }, Frame { duration_ms: 50 }];
        assert_eq!(preview_frame(&s, 0).0, 0);
        assert_eq!(preview_frame(&s, 99).0, 0);
        assert_eq!(preview_frame(&s, 100).0, 1);
        assert_eq!(preview_frame(&s, 150).0, 0);
    }

    #[test]
    fn the_preview_repaints_when_its_frame_ends() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        s.frames = vec![Frame { duration_ms: 100 }, Frame { duration_ms: 50 }];
        assert_eq!(preview_frame(&s, 30), (0, Some(70)));
        assert_eq!(preview_frame(&s, 120), (1, Some(30)));
        assert_eq!(preview_frame(&s, 150), (0, Some(100)));
        s.frames.truncate(1);
        assert_eq!(preview_frame(&s, 30), (0, None), "one frame");
    }

    #[test]
    fn a_symbol_without_durations_previews_its_first_frame() {
        let mut s = Sprite::new(2, 2, ColorMode::Rgba);
        s.frames = vec![Frame { duration_ms: 0 }, Frame { duration_ms: 0 }];
        assert_eq!(preview_frame(&s, 1234).0, 0);
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
