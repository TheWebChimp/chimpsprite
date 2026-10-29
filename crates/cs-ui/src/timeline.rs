//! Panel inferior: controles de reproducción, capas × frames, tags. Con Pixel UI, la barra y la
//! tabla del diseño (`pixel`).

mod pixel;

use cs_commands::ops::{self, LayerDrop};
use cs_core::{AnimDirection, BlendMode, LayerId, LayerKind, Tag};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use crate::editor::Editor;
use crate::mask_ops::NewMask;
use crate::pixel_ui::Tip as _;
use crate::skin::Skin;
use crate::{menus, theme, widgets};

pub use pixel::panel_frame as pixel_panel_frame;

/// Medidas de la grilla de capas × frames: las de siempre o, con Pixel UI, las del diseño.
#[derive(Clone, Copy)]
struct Grid<'a> {
    /// Con Pixel UI: el skin de las piezas y los colores.
    skin: Option<&'a Skin>,
    /// Alto de una fila de capa (con Pixel UI, con su línea de abajo).
    row_h: f32,
    header_h: f32,
    tag_h: f32,
    /// Ancho de la columna de un frame (con Pixel UI, con su línea de la derecha).
    cell_w: f32,
    /// Ancho de la parte de la izquierda: ojo, candado y nombre.
    left_w: f32,
    eye_w: f32,
    lock_w: f32,
    indent: f32,
    fold_w: f32,
    /// La miniatura de la máscara.
    chip: Vec2,
}

impl<'a> Grid<'a> {
    const CLASSIC: Grid<'static> = Grid {
        skin: None,
        row_h: 22.0,
        header_h: 18.0,
        tag_h: 16.0,
        cell_w: 22.0,
        left_w: 230.0,
        eye_w: 20.0,
        lock_w: 20.0,
        indent: 12.0,
        fold_w: 14.0,
        chip: vec2(16.0, 14.0),
    };

    fn pixel(skin: &'a Skin) -> Grid<'a> {
        Grid {
            skin: Some(skin),
            row_h: pixel::ROW_H,
            header_h: pixel::ROW_H,
            tag_h: pixel::ROW_H,
            cell_w: pixel::CELL_W,
            left_w: pixel::EYE_W + pixel::LOCK_W + pixel::NAME_W,
            eye_w: pixel::EYE_W,
            lock_w: pixel::LOCK_W,
            indent: 8.0,
            fold_w: pixel::FOLDER_W,
            chip: vec2(12.0, 9.0),
        }
    }

    /// El texto de la grilla.
    fn text(&self, ui: &egui::Ui) -> Color32 {
        self.skin
            .map_or(ui.visuals().text_color(), |s| s.colors().text)
    }

    /// Lo apagado: capas ocultas, cels vacíos, bordes.
    fn dim(&self) -> Color32 {
        self.skin.map_or(theme::LABEL, |s| s.colors().text_dim)
    }

    /// Lo elegido: con Pixel UI, el color de selección del skin.
    fn accent(&self) -> Color32 {
        self.skin.map_or(theme::ACCENT, |s| s.colors().selection)
    }

    /// Rectángulo de la columna del frame `f` dentro de una fila que empieza en `left`.
    fn cell_rect(&self, row: Rect, f: usize) -> Rect {
        Rect::from_min_size(
            pos2(row.left() + self.left_w + f as f32 * self.cell_w, row.top()),
            vec2(self.cell_w, row.height()),
        )
    }

    fn row_width(&self, editor: &Editor) -> f32 {
        self.left_w + editor.frame_count() as f32 * self.cell_w
    }
}

const BLEND_MODES: [BlendMode; 9] = [
    BlendMode::Normal,
    BlendMode::Multiply,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::Addition,
    BlendMode::Subtract,
    BlendMode::Difference,
];

fn blend_name(mode: BlendMode) -> String {
    match mode {
        BlendMode::Other(n) => format!("Other ({n})"),
        m => format!("{m:?}"),
    }
}

/// Estado temporal de la UI del timeline (ediciones en curso).
#[derive(Default)]
pub struct TimelineState {
    renaming: Option<(LayerId, String)>,
    /// Valores que se están arrastrando; se confirman al soltar (un paso de undo).
    opacity_edit: Option<u8>,
    duration_edit: Option<u16>,
    /// Frame donde empezó una selección con Shift.
    range_anchor: Option<usize>,
    /// El cel donde empezó el rango de cels (Shift+clic lo extiende desde ahí).
    cel_anchor: Option<(LayerId, usize)>,
    /// Un arrastre sobre los cels: eligiendo un rango o moviéndolo.
    cel_drag: Option<CelDrag>,
    /// Las filas de las capas dibujadas en este frame, en orden (para el contorno del rango).
    rows: Vec<(LayerId, Rect)>,
    /// Se pidió el diálogo "New Tilemap Layer" (lo abre la app).
    pub new_tilemap: bool,
}

/// Un arrastre sobre los cels, como en Aseprite.
#[derive(Clone, Copy, Debug, PartialEq)]
enum CelDrag {
    /// Eligiendo un rango desde este cel hasta el que tenga el puntero.
    Select((LayerId, usize)),
    /// Moviendo el rango (con Ctrl, copiándolo): de donde se agarró al cel con el puntero.
    Move {
        from: (LayerId, usize),
        to: (LayerId, usize),
    },
}

#[derive(Clone, Copy)]
struct FramePayload(usize);

#[derive(Clone, Copy)]
struct LayerPayload(LayerId);

fn to_color32(c: cs_core::Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// El timeline. Con Pixel UI (`skin`), la caja, la barra y la tabla del diseño.
pub fn show(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    state: &mut TimelineState,
    skin: Option<&Skin>,
) {
    if let Some(skin) = skin {
        pixel::show(ui, editor, state, skin);
        return;
    }
    controls(ui, editor, state);
    ui.separator();
    grid(ui, editor, state, Grid::CLASSIC);
}

/// Tags, encabezado y una fila por capa, con scroll en las dos direcciones.
fn grid(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState, grid: Grid) {
    crate::pixel_ui::scroll(
        ui,
        egui::ScrollArea::both().auto_shrink([false, false]),
        |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            if grid.skin.is_some() {
                // El scroll empieza una fila antes: ahí suben las líneas de las columnas.
                ui.add_space(1.0);
            }
            // Con Pixel UI, la fila de tags solo si hay tags (el diseño no la tiene).
            if grid.skin.is_none() || !editor.sprite().tags.is_empty() {
                tags_row(ui, editor, grid);
            }
            header_row(ui, editor, state, grid);
            state.rows.clear();
            for id in editor.timeline_layers() {
                layer_row(ui, editor, state, id, grid);
            }
            range_outline(ui, editor, state, grid);
            end_cel_drag(ui, editor, state);
        },
    );
}

/// Al soltar un rango que se arrastraba, se mueve ahí (con Ctrl, se copia).
fn end_cel_drag(ui: &egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    let (down, copy) = ui.input(|i| (i.pointer.primary_down(), i.modifiers.command));
    if down {
        return;
    }
    if let Some(CelDrag::Move { from, to }) = state.cel_drag.take() {
        let (layers, frames) = cel_offset(editor, from, to);
        editor.move_selected_cels(layers, frames, copy);
    }
}

/// Cuántas capas (hacia abajo en el timeline) y frames hay de `from` a `to`.
fn cel_offset(editor: &Editor, from: (LayerId, usize), to: (LayerId, usize)) -> (isize, isize) {
    let order = editor.timeline_layers();
    let at = |id: LayerId| order.iter().position(|l| *l == id).unwrap_or(0) as isize;
    (at(to.0) - at(from.0), to.1 as isize - from.1 as isize)
}

/// El contorno del rango de cels (si tiene más de uno); mientras se arrastra, donde caería.
/// Con Pixel UI, como en el diseño: una línea blanca sobre las líneas de las celdas del borde,
/// con una oscura a cada lado.
fn range_outline(ui: &egui::Ui, editor: &Editor, state: &TimelineState, grid: Grid) {
    if !editor.has_cel_range() {
        return;
    }
    let (layers, frames) = match state.cel_drag {
        Some(CelDrag::Move { from, to }) => cel_offset(editor, from, to),
        _ => (0, 0),
    };
    let range = editor.range_layers();
    let at = |id: LayerId| state.rows.iter().position(|(l, _)| *l == id);
    let (Some(top), Some(bottom)) = (at(range[0]), range.last().copied().and_then(at)) else {
        return;
    };
    let last_row = state.rows.len() as isize - 1;
    let row = |i: usize| state.rows[(i as isize + layers).clamp(0, last_row) as usize].1;
    let last_frame = editor.frame_count() as isize - 1;
    let frame = |f: usize| (f as isize + frames).clamp(0, last_frame) as usize;
    let (a, b) = editor.selected_frames();
    let first = grid.cell_rect(row(top), frame(a));
    let last = grid.cell_rect(row(bottom), frame(b));
    let painter = ui.painter();
    let Some(skin) = grid.skin else {
        let outline = Rect::from_min_max(first.min, last.max);
        painter.rect_stroke(
            outline,
            2.0,
            Stroke::new(2.0, theme::ACCENT),
            StrokeKind::Inside,
        );
        return;
    };
    // Las líneas de las celdas: la oscura de la de antes (2 antes del contenido) y la de cada
    // celda (al final de su contenido).
    let (first, last) = (pixel::content(first), pixel::content(last));
    let line = Rect::from_min_max(first.min - vec2(2.0, 2.0), last.max + vec2(1.0, 1.0));
    let dark = skin.colors().cel_range;
    let ppp = ui.ctx().pixels_per_point();
    let mut mesh = egui::Mesh::default();
    for (rect, color) in [
        (line.expand(1.0), dark),
        (line, Color32::WHITE),
        (line.shrink(1.0), dark),
    ] {
        for side in frame_sides(rect) {
            mesh.add_colored_rect(
                eframe::emath::GuiRounding::round_to_pixels(side, ppp),
                color,
            );
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Los cuatro lados de 1 px de `rect` (por dentro).
fn frame_sides(rect: Rect) -> [Rect; 4] {
    let (l, t, r, b) = (rect.left(), rect.top(), rect.right(), rect.bottom());
    [
        Rect::from_min_max(pos2(l, t), pos2(r, t + 1.0)),
        Rect::from_min_max(pos2(l, b - 1.0), pos2(r, b)),
        Rect::from_min_max(pos2(l, t + 1.0), pos2(l + 1.0, b - 1.0)),
        Rect::from_min_max(pos2(r - 1.0, t + 1.0), pos2(r, b - 1.0)),
    ]
}

/// La fila de controles: una sola línea que, si no cabe, hace scroll horizontal con la rueda o la
/// barra (como la barra de opciones). Alta como su contenido, no como el panel.
fn controls(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    ui.scope(|ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        ui.style_mut().always_scroll_the_only_direction = true;
        crate::panels::row_scroll_area("timeline_controls_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| controls_row(ui, editor, state));
    });
}

fn controls_row(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    ui.horizontal(|ui| {
        let last = editor.frame_count() - 1;
        if ui.button("⏮").tip("First frame").clicked() {
            editor.set_frame(0);
        }
        if ui.button("⏪").tip("Previous frame (,)").clicked() {
            editor.prev_frame();
        }
        let play = if editor.playing { "⏸" } else { "▶" };
        if ui.button(play).tip("Play / pause (Enter)").clicked() {
            editor.toggle_play();
        }
        if ui.button("⏩").tip("Next frame (.)").clicked() {
            editor.next_frame();
        }
        if ui.button("⏭").tip("Last frame").clicked() {
            editor.set_frame(last);
        }
        ui.label(format!("{}/{}", editor.frame + 1, last + 1));

        ui.separator();
        if ui
            .button("+ Frame")
            .tip("Duplicate frame (Alt+N)")
            .clicked()
        {
            editor.duplicate_frame();
        }
        if ui
            .add_enabled(last > 0, egui::Button::new("− Frame"))
            .clicked()
        {
            editor.remove_frame();
        }
        ui.separator();
        if ui.button("+ Layer").tip("New layer (Shift+N)").clicked() {
            editor.add_layer(LayerKind::Image);
        }
        if ui.button("+ Group").clicked() {
            editor.add_layer(LayerKind::Group);
        }
        if ui.button("+ Tilemap").tip("New tilemap layer").clicked() {
            state.new_tilemap = true;
        }
        let can_remove = editor
            .sprite()
            .subtree_range(editor.layer)
            .is_some_and(|r| r.len() < editor.sprite().layers.len());
        if ui
            .add_enabled(can_remove, egui::Button::new("− Layer"))
            .clicked()
        {
            editor.remove_layer();
        }

        ui.separator();
        ui.toggle_value(&mut editor.onion_enabled, "Onion")
            .tip("Onion skin (F3)");
        if editor.onion_enabled {
            let mut before = editor.onion.before as u32;
            let mut after = editor.onion.after as u32;
            widgets::number(ui, &mut before, 0..=8, " ◀");
            widgets::number(ui, &mut after, 0..=8, " ▶");
            editor.onion.before = before as usize;
            editor.onion.after = after as usize;
        }

        ui.separator();
        let layer = editor.layer;
        if let Some(l) = editor.sprite().layer(layer).cloned() {
            let mut opacity = state.opacity_edit.unwrap_or(l.opacity);
            let response = ui
                .add(egui::Slider::new(&mut opacity, 0..=255).text("Opacity"))
                .tip("Layer opacity");
            if response.changed() {
                state.opacity_edit = Some(opacity);
            }
            if (response.drag_stopped() || response.lost_focus() || !response.dragged())
                && let Some(v) = state.opacity_edit.take()
            {
                editor.set_opacity(layer, v);
            }
            egui::ComboBox::from_id_salt("blend")
                .selected_text(blend_name(l.blend))
                .show_ui(ui, |ui| {
                    for mode in BLEND_MODES {
                        if ui
                            .selectable_label(l.blend == mode, blend_name(mode))
                            .clicked()
                        {
                            editor.set_blend(layer, mode);
                        }
                    }
                });
        }

        ui.separator();
        let current = editor.sprite().frames[editor.frame].duration_ms;
        let mut ms = state.duration_edit.unwrap_or(current);
        let response =
            widgets::number(ui, &mut ms, 1..=60000, " ms").tip("Duration of the selected frames");
        if response.changed() {
            state.duration_edit = Some(ms);
        }
        if !response.dragged()
            && !response.has_focus()
            && let Some(v) = state.duration_edit.take()
        {
            editor.set_duration(v);
        }
    });
}

fn tags_row(ui: &mut egui::Ui, editor: &mut Editor, grid: Grid) {
    let tags = editor.sprite().tags.clone();
    // Con Pixel UI, como en Aseprite: cada tag con su nombre en una cajita y un corchete que baja
    // hasta sus frames; los que se enciman, en pisos distintos.
    let levels = tag_levels(&tags);
    let floors = levels.iter().max().map_or(1, |l| l + 1);
    let height = match grid.skin {
        Some(_) => floors as f32 * pixel::TAG_LEVEL + pixel::TAG_FOOT,
        None => grid.tag_h,
    };
    let (row, _) = ui.allocate_exact_size(vec2(grid.row_width(editor), height), Sense::hover());
    let mut action = None;
    for (i, tag) in tags.iter().enumerate() {
        let span = Rect::from_min_max(
            grid.cell_rect(row, tag.from).min,
            grid.cell_rect(row, tag.to).max,
        );
        let r = match grid.skin {
            Some(_) => pixel::tag_hit(span, levels[i]),
            None => Rect::from_min_max(span.min + vec2(1.0, 2.0), span.max - vec2(1.0, 1.0)),
        };
        let response = ui.interact(r, ui.id().with(("tag", i)), Sense::click());
        match grid.skin {
            Some(_) => pixel::tag(
                ui,
                span,
                row.bottom(),
                levels[i],
                to_color32(tag.color),
                &tag.name,
            ),
            None => {
                let painter = ui.painter();
                painter.rect_filled(r, 2.0, to_color32(tag.color).gamma_multiply(0.85));
                painter.text(
                    r.left_center() + vec2(4.0, 0.0),
                    Align2::LEFT_CENTER,
                    &tag.name,
                    FontId::proportional(11.0),
                    Color32::BLACK,
                );
            }
        }
        if response.clicked() {
            editor.set_frame(tag.from);
            editor.frame_range = Some((tag.from, tag.to));
        }
        let response = response.tip(format!("{} ({}-{})", tag.name, tag.from + 1, tag.to + 1));
        menus::context_menu(&response, |ui| {
            if let Some(a) = tag_menu(ui, editor, i, tag) {
                action = Some(a);
            }
        });
    }
    // Después del bucle: borrar un tag corre los índices de los que siguen.
    match action {
        Some(TagAction::Set(i, tag)) => editor.set_tag(i, tag),
        Some(TagAction::Remove(i)) => editor.remove_tag(i),
        None => {}
    }
}

/// El piso de cada tag (0, el de arriba): cada uno en el más alto donde no se encima con otro.
fn tag_levels(tags: &[Tag]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..tags.len()).collect();
    order.sort_by_key(|&i| (tags[i].from, std::cmp::Reverse(tags[i].to)));
    // El último frame ocupado de cada piso.
    let mut ends: Vec<usize> = Vec::new();
    let mut levels = vec![0; tags.len()];
    for i in order {
        let tag = &tags[i];
        let level = ends
            .iter()
            .position(|&end| end < tag.from)
            .unwrap_or(ends.len());
        if level == ends.len() {
            ends.push(tag.to);
        } else {
            ends[level] = tag.to;
        }
        levels[i] = level;
    }
    levels
}

/// Un cambio a la lista de tags pedido desde el menú de un tag.
enum TagAction {
    Set(usize, Tag),
    Remove(usize),
}

/// Los colores para elegir en el menú de un tag.
const TAG_COLORS: [cs_core::Rgba; 8] = [
    cs_core::Rgba::rgb(0, 0, 0),
    cs_core::Rgba::rgb(240, 128, 32),
    cs_core::Rgba::rgb(226, 114, 133),
    cs_core::Rgba::rgb(204, 51, 51),
    cs_core::Rgba::rgb(240, 200, 48),
    cs_core::Rgba::rgb(80, 160, 72),
    cs_core::Rgba::rgb(48, 128, 208),
    cs_core::Rgba::rgb(128, 80, 176),
];

/// El menú de un tag. Devuelve el cambio pedido, para aplicarlo fuera del bucle de los tags.
fn tag_menu(ui: &mut egui::Ui, editor: &Editor, index: usize, tag: &Tag) -> Option<TagAction> {
    let mut edited = tag.clone();
    ui.horizontal(|ui| {
        ui.label("Name");
        crate::widgets::text_line(ui, &mut edited.name);
    });
    ui.horizontal(|ui| {
        for (dir, label) in [
            (AnimDirection::Forward, "Forward"),
            (AnimDirection::Reverse, "Reverse"),
            (AnimDirection::PingPong, "Ping-pong"),
            (AnimDirection::PingPongReverse, "Ping-pong ←"),
        ] {
            crate::widgets::radio_value(ui, &mut edited.direction, dir, label);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Repeat");
        crate::widgets::number(ui, &mut edited.repeat, 0..=65535, "")
            .tip("Times the tag plays before stopping (0: forever)");
    });
    ui.horizontal(|ui| {
        ui.label("Color");
        // Los colores de los tags, en cuadritos como los de la paleta.
        let dark = crate::pixel_ui::control_border(ui.ctx());
        for color in TAG_COLORS {
            let (rect, response) = ui.allocate_exact_size(vec2(11.0, 11.0), Sense::click());
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, to_color32(color));
            let chosen = edited.color == color;
            let border = if chosen { Color32::WHITE } else { dark };
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
            if chosen {
                painter.rect_stroke(
                    rect.expand(1.0),
                    0.0,
                    Stroke::new(1.0, dark),
                    StrokeKind::Inside,
                );
            }
            if response.clicked() {
                edited.color = color;
            }
        }
    });
    let (a, b) = editor.selected_frames();
    if ui
        .add(menus::plain(&format!(
            "Set range to selection ({}-{})",
            a + 1,
            b + 1
        )))
        .clicked()
    {
        (edited.from, edited.to) = (a, b);
    }
    let mut action = (edited != *tag).then_some(TagAction::Set(index, edited));
    menus::separator(ui);
    if ui.add(menus::plain("Delete tag")).clicked() {
        action = Some(TagAction::Remove(index));
        ui.close();
    }
    action
}

fn header_row(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState, grid: Grid) {
    let (row, _) =
        ui.allocate_exact_size(vec2(grid.row_width(editor), grid.header_h), Sense::hover());
    match grid.skin {
        // Con Pixel UI: las líneas, el ojo abierto y el candado cerrado de las columnas.
        Some(skin) => pixel::header_left(ui, skin, row, editor.frame_count()),
        None => {
            ui.painter().text(
                row.left_center() + vec2(6.0, 0.0),
                Align2::LEFT_CENTER,
                "Layers",
                FontId::proportional(11.0),
                theme::LABEL,
            );
        }
    }
    let (sel_a, sel_b) = editor.selected_frames();
    // Lo que cambia la lista de frames (borrar, insertar, mover) se aplica después del bucle:
    // si no, al borrar un frame las vueltas que quedan leen más allá del final.
    let mut action = None;
    for f in 0..editor.frame_count() {
        let r = grid.cell_rect(row, f);
        let response = ui.interact(r, ui.id().with(("frame", f)), Sense::click_and_drag());
        let in_range = editor.frame_range.is_some() && (sel_a..=sel_b).contains(&f);
        let painter = ui.painter();
        match grid.skin {
            Some(skin) => {
                pixel::frame_number(ui, skin, r, f, f == editor.frame, in_range);
            }
            None => {
                if f == editor.frame {
                    painter.rect_filled(r.shrink(1.0), 2.0, theme::ACCENT.gamma_multiply(0.7));
                } else if in_range {
                    painter.rect_filled(r.shrink(1.0), 2.0, theme::ACCENT.gamma_multiply(0.3));
                }
                if response.hovered() {
                    painter.rect_stroke(
                        r.shrink(1.0),
                        2.0,
                        Stroke::new(1.0, theme::LABEL),
                        StrokeKind::Inside,
                    );
                }
                painter.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    (f + 1).to_string(),
                    FontId::monospace(10.0),
                    if f == editor.frame {
                        Color32::BLACK
                    } else {
                        ui.visuals().text_color()
                    },
                );
            }
        }
        let painter = ui.painter();

        // Arrastrar un encabezado reordena frames.
        if response.drag_started() {
            response.dnd_set_drag_payload(FramePayload(f));
        }
        if response.dnd_hover_payload::<FramePayload>().is_some() {
            painter.vline(r.left(), r.y_range(), Stroke::new(2.0, grid.accent()));
        }
        if let Some(p) = response.dnd_release_payload::<FramePayload>() {
            action = Some(FrameAction::Move(p.0, f));
        }

        if response.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            // Elegidos en el encabezado: copiar y pegar es de frames enteros.
            editor.frames_picked = true;
            if shift {
                let anchor = state.range_anchor.unwrap_or(editor.frame);
                editor.frame_range = Some((anchor, f));
            } else {
                state.range_anchor = Some(f);
                editor.frame_range = None;
                editor.stop();
            }
            editor.set_frame(f);
        }
        let duration = editor.sprite().frames[f].duration_ms;
        let response = response.tip(format!("Frame {} · {duration} ms", f + 1));
        menus::context_menu(&response, |ui| {
            if ui.add(menus::plain("New tag from selection")).clicked() {
                editor.add_tag();
                ui.close();
            }
            if ui.add(menus::plain("Duplicate frame")).clicked() {
                action = Some(FrameAction::Duplicate(f));
                ui.close();
            }
            if ui.add(menus::plain("Insert empty frame")).clicked() {
                action = Some(FrameAction::InsertEmpty(f));
                ui.close();
            }
            if ui
                .add_enabled(editor.frame_count() > 1, menus::plain("Delete frame"))
                .clicked()
            {
                action = Some(FrameAction::Remove(f));
                ui.close();
            }
        });
    }
    match action {
        Some(FrameAction::Move(from, to)) => editor.move_frame(from, to),
        Some(FrameAction::Duplicate(f)) => {
            editor.set_frame(f);
            editor.duplicate_frame();
        }
        Some(FrameAction::InsertEmpty(f)) => {
            editor.set_frame(f);
            editor.insert_empty_frame();
        }
        Some(FrameAction::Remove(f)) => {
            editor.set_frame(f);
            editor.remove_frame();
        }
        None => {}
    }
}

/// Un cambio a la lista de frames pedido desde el encabezado; se aplica al terminar el bucle.
enum FrameAction {
    /// Soltar el frame `.0` sobre el `.1`.
    Move(usize, usize),
    Duplicate(usize),
    InsertEmpty(usize),
    Remove(usize),
}

fn layer_row(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    state: &mut TimelineState,
    id: LayerId,
    grid: Grid,
) {
    let (row, _) = ui.allocate_exact_size(vec2(grid.row_width(editor), grid.row_h), Sense::hover());
    state.rows.push((id, row));
    let Some(layer) = editor.sprite().layer(id).cloned() else {
        return;
    };
    let active = editor.layer == id;
    let depth = editor.sprite().depth(id) as f32;
    let collapsed = editor.collapsed.contains(&(editor.active, id));
    let visible_chain = editor.sprite().is_visible(id);

    let painter = ui.painter().clone();
    let left = Rect::from_min_size(row.min, vec2(grid.left_w, grid.row_h));
    // Con Pixel UI, sin la línea de abajo (y de la derecha): donde va el contenido.
    let inner = |r: Rect| match grid.skin {
        Some(_) => pixel::content(r),
        None => r,
    };
    match grid.skin {
        Some(skin) => pixel::layer_left(ui, skin, row, editor.frame_count(), active),
        None => {
            if active {
                painter.rect_filled(left, 0.0, theme::ACCENT.gamma_multiply(0.35));
            }
            painter.hline(
                row.x_range(),
                row.bottom() - 0.5,
                Stroke::new(1.0, theme::CANVAS_BG),
            );
        }
    }

    // Ojo y candado.
    let eye = Rect::from_min_size(row.min, vec2(grid.eye_w, grid.row_h));
    let lock = Rect::from_min_size(eye.right_top(), vec2(grid.lock_w, grid.row_h));
    let icon_color = |on: bool| {
        if on {
            ui.visuals().text_color()
        } else {
            theme::LABEL.gamma_multiply(0.5)
        }
    };
    let eye_response = ui
        .interact(inner(eye), ui.id().with(("eye", id)), Sense::click())
        .tip("Show / hide");
    let lock_response = ui
        .interact(inner(lock), ui.id().with(("lock", id)), Sense::click())
        .tip("Lock / unlock");
    match grid.skin {
        Some(skin) => {
            // Con Pixel UI, con nombre accesible (y estado) para encontrarlos.
            let enabled = ui.is_enabled();
            eye_response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Checkbox,
                    enabled,
                    layer.visible,
                    format!("Visible: {}", layer.name),
                )
            });
            lock_response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Checkbox,
                    enabled,
                    layer.locked,
                    format!("Locked: {}", layer.name),
                )
            });
            pixel::eye_and_lock(ui, skin, eye, lock, layer.visible, layer.locked, active);
        }
        None => {
            painter.text(
                eye.center(),
                Align2::CENTER_CENTER,
                if layer.visible { "👁" } else { "–" },
                FontId::proportional(12.0),
                icon_color(layer.visible),
            );
            painter.text(
                lock.center(),
                Align2::CENTER_CENTER,
                if layer.locked { "🔒" } else { "🔓" },
                FontId::proportional(11.0),
                icon_color(layer.locked),
            );
        }
    }
    if eye_response.clicked() {
        editor.toggle_visible(id);
    }
    if lock_response.clicked() {
        editor.toggle_locked(id);
    }

    // Flecha de colapsar (grupos) y nombre.
    let name_left = lock.right()
        + if grid.skin.is_some() {
            pixel::NAME_PAD
        } else {
            4.0
        };
    let mut x = name_left + depth * grid.indent;
    let content = inner(row);
    if layer.kind == LayerKind::Group {
        // Con Pixel UI, la carpetita lleva su flechita (▾/▸) a un lado.
        let fold_w = grid.fold_w
            + if grid.skin.is_some() {
                pixel::FOLD_ARROW_W
            } else {
                0.0
            };
        let arrow = Rect::from_min_max(pos2(x, content.top()), pos2(x + fold_w, content.bottom()));
        if ui
            .interact(arrow, ui.id().with(("fold", id)), Sense::click())
            .clicked()
        {
            editor.toggle_collapsed(id);
        }
        match grid.skin {
            // Con Pixel UI, la carpetita del diseño: cerrada si el grupo está colapsado.
            Some(skin) => pixel::folder(ui, skin, arrow, collapsed, active),
            None => draw_fold_arrow(&painter, arrow.center(), collapsed, grid.text(ui), 4.0),
        }
        x += fold_w;
    }
    // Con Pixel UI, las capas tilemap llevan su ícono donde los grupos su carpetita.
    if layer.kind == LayerKind::Tilemap
        && let Some(skin) = grid.skin
    {
        let at = Rect::from_min_max(
            pos2(x, content.top()),
            pos2(x + grid.fold_w, content.bottom()),
        );
        pixel::tilemap_icon(ui, skin, at, active);
        x += grid.fold_w;
    }
    // Recortadas: sangría con flecha hacia la capa base.
    if layer.clip
        && let Some(skin) = grid.skin
    {
        // Con Pixel UI, la flecha del diseño, 3 px después de lo anterior.
        pixel::clip_arrow(ui, skin, pos2(x + 3.0, content.top()), active);
        x += 14.0;
    } else if layer.clip {
        let stroke = Stroke::new(1.5, grid.accent());
        let (top, mid) = (content.top() + 4.0, content.center().y + 1.0);
        let (x0, x1) = (x + 3.0, x + 10.0);
        painter.line_segment([pos2(x0, top), pos2(x0, mid)], stroke);
        painter.line_segment([pos2(x0, mid), pos2(x1, mid)], stroke);
        painter.line_segment([pos2(x1 - 3.0, mid - 3.0), pos2(x1, mid)], stroke);
        painter.line_segment([pos2(x1 - 3.0, mid + 3.0), pos2(x1, mid)], stroke);
        x += 14.0;
    }
    // Miniatura de la máscara: clic para pintarla (o volver a la capa).
    let left_content = inner(left);
    let mut right = left_content.right();
    // Con Pixel UI: la cara del diseño (y, en la capa activa de imagen sin máscara, el botón
    // para agregarle una) contra la derecha, centrada en la fila.
    let pixel_chip = |size: Vec2| {
        Rect::from_min_size(
            pos2(
                left_content.right() - 4.0 - size.x,
                (content.center().y - size.y / 2.0).floor(),
            ),
            size,
        )
    };
    if layer.mask.is_none()
        && active
        && layer.kind == LayerKind::Image
        && let Some(skin) = grid.skin
    {
        let add = pixel_chip(pixel::MASK_ADD_SIZE);
        right = add.left() - 2.0;
        pixel::mask_add(ui, skin, add.min);
        let response = ui
            .interact(add, ui.id().with(("mask_add", id)), Sense::click())
            .tip("Add Layer Mask");
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Add Layer Mask")
        });
        if response.clicked() {
            editor.add_mask(id, NewMask::RevealAll);
        }
    }
    if let Some(mask) = &layer.mask {
        let chip = match grid.skin {
            Some(_) => pixel_chip(pixel::MASK_SIZE),
            None => Rect::from_center_size(
                pos2(
                    left_content.right() - 4.0 - grid.chip.x / 2.0,
                    content.center().y,
                ),
                grid.chip,
            ),
        };
        right = chip.left() - 2.0;
        let editing = active && editor.edit_mask;
        let response = ui
            .interact(chip, ui.id().with(("mask", id)), Sense::click())
            .tip(if editing {
                "Editing the mask (click to edit the layer)"
            } else {
                "Layer mask (click to paint it)"
            });
        if let Some(skin) = grid.skin {
            pixel::mask_chip(ui, skin, chip.min, editing, mask.enabled);
        } else {
            painter.rect_filled(chip, 2.0, Color32::from_gray(40));
            painter.rect_filled(
                Rect::from_min_max(chip.min, pos2(chip.center().x, chip.max.y)).shrink(2.0),
                0.0,
                Color32::WHITE,
            );
            painter.rect_filled(
                Rect::from_min_max(pos2(chip.center().x, chip.min.y), chip.max).shrink(2.0),
                0.0,
                Color32::BLACK,
            );
            if !mask.enabled {
                painter.line_segment(
                    [chip.left_bottom(), chip.right_top()],
                    Stroke::new(1.5, Color32::from_rgb(230, 40, 60)),
                );
            }
            painter.rect_stroke(
                chip,
                2.0,
                Stroke::new(
                    if editing { 2.0 } else { 1.0 },
                    if editing { grid.accent() } else { grid.dim() },
                ),
                StrokeKind::Outside,
            );
        }
        if response.clicked() {
            let editing_now = editor.layer == id && editor.edit_mask;
            editor.select_layer(id);
            editor.set_edit_mask(!editing_now);
        }
        menus::context_menu(&response, |ui| mask_menu(ui, editor, id));
    }
    let name_rect = Rect::from_min_max(pos2(x, content.top()), pos2(right, content.bottom()));
    let name_response = ui.interact(
        name_rect,
        ui.id().with(("name", id)),
        Sense::click_and_drag(),
    );

    let renaming = matches!(&state.renaming, Some((rid, _)) if *rid == id);
    if renaming {
        let (_, text) = state.renaming.as_mut().expect("renaming");
        let edit = match grid.skin {
            // Con Pixel UI, la fila es baja: el campo ocupa todo el nombre, con la fuente pixel.
            Some(_) => ui.put(
                name_rect,
                egui::TextEdit::singleline(text).margin(Vec2::ZERO),
            ),
            None => ui.put(
                name_rect.shrink2(vec2(0.0, 2.0)),
                egui::TextEdit::singleline(text).font(FontId::proportional(12.0)),
            ),
        };
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        if edit.lost_focus() {
            let (_, name) = state.renaming.take().expect("renaming");
            editor.rename_layer(id, name.trim().to_string());
        }
    } else {
        let label = match layer.kind {
            // Con Pixel UI, los grupos y los tilemaps ya tienen su ícono.
            LayerKind::Group | LayerKind::Tilemap if grid.skin.is_some() => layer.name.clone(),
            LayerKind::Group => format!("📁 {}", layer.name),
            // El mismo ícono que la herramienta Artboard: la capa muestra otro artboard.
            LayerKind::Instance => format!("🖼 {}", layer.name),
            // Grilla de tiles.
            LayerKind::Tilemap => format!("⊞ {}", layer.name),
            // La de referencia (no se exporta) lo dice.
            LayerKind::Image if layer.reference => format!("{} (Ref)", layer.name),
            LayerKind::Image => layer.name.clone(),
        };
        match grid.skin {
            Some(skin) => {
                // Como en el diseño, el nombre de una capa oculta va en negro (lo dice el ojo);
                // apagado solo si la esconde un grupo de arriba.
                let colors = skin.colors();
                let color = if visible_chain || !layer.visible {
                    colors.text
                } else {
                    colors.text_dim
                };
                pixel::name(ui, name_rect, &label, color);
            }
            None => {
                let color = if visible_chain {
                    ui.visuals().text_color()
                } else {
                    theme::LABEL.gamma_multiply(0.6)
                };
                painter.text(
                    name_rect.left_center(),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(12.0),
                    color,
                );
            }
        }
    }

    if name_response.clicked() {
        editor.select_layer(id);
    }
    if name_response.double_clicked() {
        state.renaming = Some((id, layer.name.clone()));
    }
    if name_response.drag_started() {
        name_response.dnd_set_drag_payload(LayerPayload(id));
    }
    // Soltar otra capa sobre esta fila.
    if let Some(pointer) = ui.ctx().pointer_hover_pos()
        && name_response.dnd_hover_payload::<LayerPayload>().is_some()
    {
        let drop = drop_kind(row, pointer, &layer);
        draw_drop_hint(&painter, left, drop, grid.accent());
    }
    if let Some(payload) = name_response.dnd_release_payload::<LayerPayload>()
        && let Some(pointer) = ui.ctx().pointer_interact_pos()
        && payload.0 != id
    {
        let drop = drop_kind(row, pointer, &layer);
        editor.move_layer(payload.0, drop);
    }
    menus::context_menu(&name_response, |ui| {
        if ui.add(menus::plain("Rename")).clicked() {
            state.renaming = Some((id, layer.name.clone()));
            ui.close();
        }
        if ui.add(menus::plain("New layer above")).clicked() {
            editor.select_layer(id);
            editor.add_layer(LayerKind::Image);
            ui.close();
        }
        if ui.add(menus::plain("New group above")).clicked() {
            editor.select_layer(id);
            editor.add_layer(LayerKind::Group);
            ui.close();
        }
        if ui.add(menus::plain("Delete")).clicked() {
            editor.select_layer(id);
            editor.remove_layer();
            ui.close();
        }
        menus::separator(ui);
        instance_menu(ui, editor);
        menus::separator(ui);
        mask_menu(ui, editor, id);
    });

    // Cels.
    if layer.kind == LayerKind::Group {
        return;
    }
    let in_layers = editor.frame_range.is_some() && editor.range_layers().contains(&id);
    let cel_range = editor.has_cel_range();
    for f in 0..editor.frame_count() {
        let r = grid.cell_rect(row, f);
        let response = ui.interact(r, ui.id().with(("cel", id, f)), Sense::click_and_drag());
        let (sel_a, sel_b) = editor.selected_frames();
        let in_range = in_layers && (sel_a..=sel_b).contains(&f);
        let selected = in_range || (active && f == editor.frame);
        if let Some(skin) = grid.skin {
            // Con Pixel UI, el cel elegido y el rango, como en el diseño.
            if selected {
                pixel::select(ui, skin, r, row.left(), false);
            }
        } else {
            if in_range {
                painter.rect_filled(r.shrink(1.0), 2.0, theme::ACCENT.gamma_multiply(0.35));
            } else if f == editor.frame {
                painter.rect_filled(r, 0.0, theme::ACCENT.gamma_multiply(0.15));
            }
            if active && f == editor.frame {
                painter.rect_stroke(
                    r.shrink(1.0),
                    2.0,
                    Stroke::new(1.0, theme::ACCENT),
                    StrokeKind::Inside,
                );
            }
        }
        let sprite = editor.sprite();
        let r = inner(r);
        let center = r.center();
        // Con Pixel UI, todo más chico: la fila mide 11 de alto.
        let (thick, d) = if grid.skin.is_some() {
            (1.0, 3.0)
        } else {
            (2.0, 4.5)
        };
        if let Some(track) = sprite.layer(id).and_then(|l| l.instance.as_ref()) {
            // Instancia: ◆ en cada keyframe y una barra mientras dura.
            let color = grid.text(ui);
            if let Some(key) = track.key_at(f) {
                let stroke = Stroke::new(thick, color);
                // Barra de "hold": viene del frame anterior y sigue al próximo.
                if f > key.frame {
                    painter.hline(r.left()..=center.x, center.y, stroke);
                }
                if f + 1 < editor.frame_count() {
                    painter.hline(center.x..=r.right(), center.y, stroke);
                }
                if key.frame == f {
                    let points = vec![
                        pos2(center.x, center.y - d),
                        pos2(center.x + d, center.y),
                        pos2(center.x, center.y + d),
                        pos2(center.x - d, center.y),
                    ];
                    let fill = if key.source.is_some() {
                        color
                    } else {
                        grid.dim()
                    };
                    painter.add(egui::Shape::convex_polygon(points, fill, Stroke::NONE));
                }
            }
        } else if let Some(cel) = sprite.cel(id, f).copied() {
            let linked_prev = f > 0 && sprite.cel(id, f - 1).is_some_and(|p| p.image == cel.image);
            match grid.skin {
                Some(skin) => pixel::cel(ui, skin, r, true, linked_prev, selected),
                None => {
                    if linked_prev {
                        painter.hline(
                            (center.x - grid.cell_w)..=center.x,
                            center.y,
                            Stroke::new(2.0, ui.visuals().text_color()),
                        );
                    }
                    painter.circle_filled(center, 4.0, ui.visuals().text_color());
                }
            }
        } else {
            match grid.skin {
                Some(skin) => pixel::cel(ui, skin, r, false, false, selected),
                None => {
                    painter.circle_stroke(
                        center,
                        3.5,
                        Stroke::new(1.0, theme::LABEL.gamma_multiply(0.6)),
                    );
                }
            }
        }
        // Como en Aseprite: arrastrar sobre los cels elige un rango de capas × frames; arrastrar
        // el rango lo mueve (con Ctrl, lo copia) al soltar.
        let cel = (id, f);
        if response.drag_started() {
            editor.stop();
            if cel_range && in_range {
                state.cel_drag = Some(CelDrag::Move { from: cel, to: cel });
            } else {
                editor.select_layer(id);
                state.range_anchor = Some(f);
                state.cel_anchor = Some(cel);
                state.cel_drag = Some(CelDrag::Select(cel));
                editor.select_cels(cel, cel);
                editor.set_frame(f);
            }
        }
        if response.contains_pointer() && ui.input(|i| i.pointer.primary_down()) {
            match &mut state.cel_drag {
                Some(CelDrag::Select(anchor)) => {
                    let anchor = *anchor;
                    editor.select_cels(anchor, cel);
                    editor.set_frame(f);
                }
                Some(CelDrag::Move { to, .. }) => *to = cel,
                None => {}
            }
        }
        if response.clicked() {
            editor.stop();
            if ui.input(|i| i.modifiers.shift) {
                let anchor = state.cel_anchor.unwrap_or((editor.layer, editor.frame));
                editor.select_cels(anchor, cel);
                state.range_anchor = Some(anchor.1);
            } else {
                editor.select_layer(id);
                state.range_anchor = Some(f);
                state.cel_anchor = Some(cel);
                editor.frame_range = None;
                editor.frames_picked = false;
            }
            editor.set_frame(f);
        }
        menus::context_menu(&response, |ui| {
            if layer.kind == LayerKind::Instance {
                let is_key = editor.is_instance_keyframe(id, f);
                // Antes de la primera key no hay nada que copiar.
                let has_key = editor
                    .sprite()
                    .layer(id)
                    .and_then(|l| l.instance.as_ref())
                    .is_some_and(|t| t.key_at(f).is_some());
                if ui
                    .add_enabled(has_key && !is_key, menus::plain("Insert Keyframe"))
                    .clicked()
                {
                    editor.insert_instance_key(id, f);
                    ui.close();
                }
                if ui
                    .add_enabled(is_key, menus::plain("Delete Keyframe"))
                    .clicked()
                {
                    editor.remove_instance_key(id, f);
                    ui.close();
                }
                return;
            }
            let (a, b) = editor.selected_frames();
            if editor.frame_range.is_some() && a != b && (a..=b).contains(&f) {
                if ui
                    .add(menus::plain(&format!("Link {} cels", b - a + 1)))
                    .clicked()
                {
                    editor.link_range(id, a, b);
                    ui.close();
                }
                if ui.add(menus::plain("Unlink cels")).clicked() {
                    editor.unlink_range(id, a, b);
                    ui.close();
                }
                if ui.add(menus::plain("Clear cels")).clicked() {
                    editor.clear_selected_cels();
                    ui.close();
                }
                menus::separator(ui);
            }
            let sprite = editor.sprite();
            let has_cel = sprite.cel(id, f).is_some();
            let can_link = f > 0 && sprite.cel(id, f - 1).is_some();
            let linked = ops::is_linked(sprite, id, f);
            if ui
                .add_enabled(can_link, menus::plain("Link to previous cel"))
                .clicked()
            {
                editor.link_to_previous(id, f);
                ui.close();
            }
            if ui.add_enabled(linked, menus::plain("Unlink")).clicked() {
                editor.unlink(id, f);
                ui.close();
            }
            if ui.add_enabled(has_cel, menus::plain("Clear cel")).clicked() {
                editor.clear_cel(id, f);
                ui.close();
            }
            menus::separator(ui);
            // Dentro del rango elegido, la de todos sus cels.
            let cels = if editor.frame_range.is_some() && (a..=b).contains(&f) {
                editor.selected_cels()
            } else {
                vec![(id, f)]
            };
            cel_opacity_row(ui, editor, &cels);
        });
    }
}

/// Cel Properties, en línea: la opacidad de `cels` (la del primero que existe); arrastrar el
/// número es un solo paso de undo.
pub(crate) fn cel_opacity_row(ui: &mut egui::Ui, editor: &mut Editor, cels: &[(LayerId, usize)]) {
    let sprite = editor.sprite();
    let Some(mut opacity) = cels
        .iter()
        .find_map(|&(l, f)| sprite.cel(l, f))
        .map(|c| c.opacity)
    else {
        return;
    };
    ui.horizontal(|ui| {
        ui.label("Cel Opacity");
        let response = crate::widgets::number(ui, &mut opacity, 0..=255, "");
        let group = crate::panels::gesture_group(editor, &[&response]);
        if response.changed() {
            editor.set_cels_opacity(cels, opacity, group);
        }
        crate::panels::gesture_end(editor, &[&response]);
    });
}

/// Submenú para colocar otro artboard como instancia (menú Layer y menú de capa).
pub(crate) fn instance_menu(ui: &mut egui::Ui, editor: &mut Editor) {
    let candidates = editor.instance_candidates();
    ui.add_enabled_ui(!candidates.is_empty(), |ui| {
        menus::submenu(ui, "New Instance", |ui| {
            for (id, name) in candidates {
                if ui.add(menus::plain(&name)).clicked() {
                    editor.add_instance_layer(id);
                    ui.close();
                }
            }
        })
        .response
        .on_disabled_hover_text("Needs another artboard, and an RGBA artboard to hold it");
    });
}

/// Dónde cae una capa soltada sobre `row` según la altura del puntero.
/// Acciones de máscara y recorte de una capa (menú contextual y menú Layer).
pub(crate) fn mask_menu(ui: &mut egui::Ui, editor: &mut Editor, id: LayerId) {
    let Some(layer) = editor.sprite().layer(id).cloned() else {
        return;
    };
    match &layer.mask {
        None => {
            menus::submenu(ui, "Add Layer Mask", |ui| {
                if ui.add(menus::plain("Reveal All")).clicked() {
                    editor.add_mask(id, NewMask::RevealAll);
                    ui.close();
                }
                if ui.add(menus::plain("Hide All")).clicked() {
                    editor.add_mask(id, NewMask::HideAll);
                    ui.close();
                }
                let has_selection = editor.selection.is_some();
                if ui
                    .add_enabled(has_selection, menus::plain("From Selection"))
                    .clicked()
                {
                    editor.add_mask(id, NewMask::FromSelection);
                    ui.close();
                }
            });
        }
        Some(mask) => {
            let editing = editor.layer == id && editor.edit_mask;
            if ui
                .add(menus::plain(if editing {
                    "Edit Layer Pixels"
                } else {
                    "Edit Mask"
                }))
                .clicked()
            {
                editor.select_layer(id);
                editor.set_edit_mask(!editing);
                ui.close();
            }
            if ui
                .add(menus::plain(if mask.enabled {
                    "Disable Mask"
                } else {
                    "Enable Mask"
                }))
                .clicked()
            {
                editor.toggle_mask_enabled(id);
                ui.close();
            }
            if ui.add(menus::plain("Mask to Selection")).clicked() {
                editor.mask_to_selection(id);
                ui.close();
            }
            if ui
                .add_enabled(layer.kind == LayerKind::Image, menus::plain("Apply Mask"))
                .clicked()
            {
                editor.apply_mask(id);
                ui.close();
            }
            if ui.add(menus::plain("Delete Mask")).clicked() {
                editor.delete_mask(id);
                ui.close();
            }
        }
    }
    let mut clip = layer.clip;
    let toggle = menus::check(&mut clip, "Clipping Mask");
    if ui
        .add(toggle)
        .tip("Show this layer only where the layer below has pixels (Ctrl+Alt+G)")
        .clicked()
    {
        editor.toggle_clip(id);
    }
}

fn drop_kind(row: Rect, pointer: Pos2, target: &cs_core::Layer) -> LayerDrop {
    let t = (pointer.y - row.top()) / row.height();
    if target.kind == LayerKind::Group && (0.3..0.7).contains(&t) {
        LayerDrop::Into(target.id)
    } else if t < 0.5 {
        LayerDrop::Above(target.id)
    } else {
        LayerDrop::Below(target.id)
    }
}

fn draw_drop_hint(painter: &egui::Painter, left: Rect, drop: LayerDrop, color: Color32) {
    let stroke = Stroke::new(2.0, color);
    match drop {
        LayerDrop::Above(_) => {
            painter.hline(left.x_range(), left.top() + 1.0, stroke);
        }
        LayerDrop::Below(_) => {
            painter.hline(left.x_range(), left.bottom() - 1.0, stroke);
        }
        LayerDrop::Into(_) => {
            painter.rect_stroke(left.shrink(1.0), 2.0, stroke, StrokeKind::Inside);
        }
    }
}

/// El triángulo de abrir y cerrar un grupo, de `s` de radio.
fn draw_fold_arrow(painter: &egui::Painter, center: Pos2, collapsed: bool, color: Color32, s: f32) {
    let points = if collapsed {
        vec![
            center + vec2(-s * 0.6, -s),
            center + vec2(s * 0.8, 0.0),
            center + vec2(-s * 0.6, s),
        ]
    } else {
        vec![
            center + vec2(-s, -s * 0.6),
            center + vec2(s, -s * 0.6),
            center + vec2(0.0, s * 0.8),
        ]
    };
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

#[cfg(test)]
mod tests;
