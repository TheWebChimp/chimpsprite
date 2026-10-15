//! Panel inferior: controles de reproducción, capas × frames, tags.

use cs_commands::ops::{self, LayerDrop};
use cs_core::{AnimDirection, BlendMode, LayerId, LayerKind, Tag};
use eframe::egui::{
    self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2, pos2, vec2,
};

use crate::editor::Editor;
use crate::mask_ops::NewMask;
use crate::{theme, widgets};

const ROW_H: f32 = 22.0;
const CELL_W: f32 = 22.0;
const LEFT_W: f32 = 230.0;
const TAG_H: f32 = 16.0;
const HEADER_H: f32 = 18.0;
const ICON_W: f32 = 20.0;
const INDENT: f32 = 12.0;

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
    /// Capa en la que se está arrastrando una selección de cels.
    cel_drag: Option<LayerId>,
}

#[derive(Clone, Copy)]
struct FramePayload(usize);

#[derive(Clone, Copy)]
struct LayerPayload(LayerId);

fn to_color32(c: cs_core::Rgba) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

pub fn show(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    controls(ui, editor, state);
    ui.separator();
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            tags_row(ui, editor);
            header_row(ui, editor, state);
            for id in editor.timeline_layers() {
                layer_row(ui, editor, state, id);
            }
        });
}

fn controls(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    ui.horizontal(|ui| {
        let last = editor.frame_count() - 1;
        if ui.button("⏮").on_hover_text("First frame").clicked() {
            editor.set_frame(0);
        }
        if ui
            .button("⏪")
            .on_hover_text("Previous frame (,)")
            .clicked()
        {
            editor.prev_frame();
        }
        let play = if editor.playing { "⏸" } else { "▶" };
        if ui
            .button(play)
            .on_hover_text("Play / pause (Enter)")
            .clicked()
        {
            editor.toggle_play();
        }
        if ui.button("⏩").on_hover_text("Next frame (.)").clicked() {
            editor.next_frame();
        }
        if ui.button("⏭").on_hover_text("Last frame").clicked() {
            editor.set_frame(last);
        }
        ui.label(format!("{}/{}", editor.frame + 1, last + 1));

        ui.separator();
        if ui
            .button("+ Frame")
            .on_hover_text("Duplicate frame (Alt+N)")
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
        if ui
            .button("+ Layer")
            .on_hover_text("New layer (Shift+N)")
            .clicked()
        {
            editor.add_layer(LayerKind::Image);
        }
        if ui.button("+ Group").clicked() {
            editor.add_layer(LayerKind::Group);
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
            .on_hover_text("Onion skin (F3)");
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
                .on_hover_text("Layer opacity");
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
        let response = widgets::number(ui, &mut ms, 1..=60000, " ms")
            .on_hover_text("Duration of the selected frames");
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

/// Rectángulo de la columna del frame `f` dentro de una fila que empieza en `left`.
fn cell_rect(row: Rect, f: usize) -> Rect {
    Rect::from_min_size(
        pos2(row.left() + LEFT_W + f as f32 * CELL_W, row.top()),
        vec2(CELL_W, row.height()),
    )
}

fn row_width(editor: &Editor) -> f32 {
    LEFT_W + editor.frame_count() as f32 * CELL_W
}

fn tags_row(ui: &mut egui::Ui, editor: &mut Editor) {
    let (row, _) = ui.allocate_exact_size(vec2(row_width(editor), TAG_H), Sense::hover());
    let tags = editor.sprite().tags.clone();
    for (i, tag) in tags.iter().enumerate() {
        let r = Rect::from_min_max(
            cell_rect(row, tag.from).min + vec2(1.0, 2.0),
            cell_rect(row, tag.to).max - vec2(1.0, 1.0),
        );
        let response = ui.interact(r, ui.id().with(("tag", i)), Sense::click());
        let painter = ui.painter();
        painter.rect_filled(r, 2.0, to_color32(tag.color).gamma_multiply(0.85));
        painter.text(
            r.left_center() + vec2(4.0, 0.0),
            Align2::LEFT_CENTER,
            &tag.name,
            FontId::proportional(11.0),
            Color32::BLACK,
        );
        if response.clicked() {
            editor.set_frame(tag.from);
            editor.frame_range = Some((tag.from, tag.to));
        }
        response
            .on_hover_text(format!("{} ({}–{})", tag.name, tag.from + 1, tag.to + 1))
            .context_menu(|ui| tag_menu(ui, editor, i, tag));
    }
}

fn tag_menu(ui: &mut egui::Ui, editor: &mut Editor, index: usize, tag: &Tag) {
    let mut edited = tag.clone();
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut edited.name);
    });
    ui.horizontal(|ui| {
        for (dir, label) in [
            (AnimDirection::Forward, "Forward"),
            (AnimDirection::Reverse, "Reverse"),
            (AnimDirection::PingPong, "Ping-pong"),
            (AnimDirection::PingPongReverse, "Ping-pong ←"),
        ] {
            ui.radio_value(&mut edited.direction, dir, label);
        }
    });
    ui.horizontal(|ui| {
        ui.label("Color");
        let mut c = to_color32(edited.color);
        if egui::color_picker::color_edit_button_srgba(
            ui,
            &mut c,
            egui::color_picker::Alpha::Opaque,
        )
        .changed()
        {
            edited.color = cs_core::Rgba::rgb(c.r(), c.g(), c.b());
        }
    });
    let (a, b) = editor.selected_frames();
    if ui
        .button(format!("Set range to selection ({}–{})", a + 1, b + 1))
        .clicked()
    {
        (edited.from, edited.to) = (a, b);
    }
    if edited != *tag {
        editor.set_tag(index, edited);
    }
    ui.separator();
    if ui.button("Delete tag").clicked() {
        editor.remove_tag(index);
        ui.close();
    }
}

fn header_row(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState) {
    let (row, _) = ui.allocate_exact_size(vec2(row_width(editor), HEADER_H), Sense::hover());
    ui.painter().text(
        row.left_center() + vec2(6.0, 0.0),
        Align2::LEFT_CENTER,
        "Layers",
        FontId::proportional(11.0),
        theme::LABEL,
    );
    let (sel_a, sel_b) = editor.selected_frames();
    for f in 0..editor.frame_count() {
        let r = cell_rect(row, f);
        let response = ui.interact(r, ui.id().with(("frame", f)), Sense::click_and_drag());
        let in_range = editor.frame_range.is_some() && (sel_a..=sel_b).contains(&f);
        let painter = ui.painter();
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

        // Arrastrar un encabezado reordena frames.
        if response.drag_started() {
            response.dnd_set_drag_payload(FramePayload(f));
        }
        if response.dnd_hover_payload::<FramePayload>().is_some() {
            painter.vline(r.left(), r.y_range(), Stroke::new(2.0, theme::ACCENT));
        }
        if let Some(p) = response.dnd_release_payload::<FramePayload>() {
            editor.move_frame(p.0, f);
        }

        if response.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
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
        response
            .on_hover_text(format!("Frame {} · {duration} ms", f + 1))
            .context_menu(|ui| {
                if ui.button("New tag from selection").clicked() {
                    editor.add_tag();
                    ui.close();
                }
                if ui.button("Duplicate frame").clicked() {
                    editor.set_frame(f);
                    editor.duplicate_frame();
                    ui.close();
                }
                if ui.button("Insert empty frame").clicked() {
                    editor.set_frame(f);
                    editor.insert_empty_frame();
                    ui.close();
                }
                if ui
                    .add_enabled(editor.frame_count() > 1, egui::Button::new("Delete frame"))
                    .clicked()
                {
                    editor.set_frame(f);
                    editor.remove_frame();
                    ui.close();
                }
            });
    }
}

fn layer_row(ui: &mut egui::Ui, editor: &mut Editor, state: &mut TimelineState, id: LayerId) {
    let (row, _) = ui.allocate_exact_size(vec2(row_width(editor), ROW_H), Sense::hover());
    let Some(layer) = editor.sprite().layer(id).cloned() else {
        return;
    };
    let active = editor.layer == id;
    let depth = editor.sprite().depth(id) as f32;
    let collapsed = editor.collapsed.contains(&(editor.active, id));
    let visible_chain = editor.sprite().is_visible(id);

    let painter = ui.painter().clone();
    let left = Rect::from_min_size(row.min, vec2(LEFT_W, ROW_H));
    if active {
        painter.rect_filled(left, 0.0, theme::ACCENT.gamma_multiply(0.35));
    }
    painter.hline(
        row.x_range(),
        row.bottom() - 0.5,
        Stroke::new(1.0, theme::CANVAS_BG),
    );

    // Ojo y candado.
    let eye = Rect::from_min_size(row.min, vec2(ICON_W, ROW_H));
    let lock = eye.translate(vec2(ICON_W, 0.0));
    let icon_color = |on: bool| {
        if on {
            ui.visuals().text_color()
        } else {
            theme::LABEL.gamma_multiply(0.5)
        }
    };
    if ui
        .interact(eye, ui.id().with(("eye", id)), Sense::click())
        .on_hover_text("Show / hide")
        .clicked()
    {
        editor.toggle_visible(id);
    }
    painter.text(
        eye.center(),
        Align2::CENTER_CENTER,
        if layer.visible { "👁" } else { "–" },
        FontId::proportional(12.0),
        icon_color(layer.visible),
    );
    if ui
        .interact(lock, ui.id().with(("lock", id)), Sense::click())
        .on_hover_text("Lock / unlock")
        .clicked()
    {
        editor.toggle_locked(id);
    }
    painter.text(
        lock.center(),
        Align2::CENTER_CENTER,
        if layer.locked { "🔒" } else { "🔓" },
        FontId::proportional(11.0),
        icon_color(layer.locked),
    );

    // Flecha de colapsar (grupos) y nombre.
    let mut x = lock.right() + 4.0 + depth * INDENT;
    if layer.kind == LayerKind::Group {
        let arrow = Rect::from_min_size(pos2(x, row.top()), vec2(14.0, ROW_H));
        if ui
            .interact(arrow, ui.id().with(("fold", id)), Sense::click())
            .clicked()
        {
            editor.toggle_collapsed(id);
        }
        draw_fold_arrow(
            &painter,
            arrow.center(),
            collapsed,
            ui.visuals().text_color(),
        );
        x += 14.0;
    }
    // Recortadas: sangría con flecha hacia la capa base.
    if layer.clip {
        let stroke = Stroke::new(1.5, theme::ACCENT);
        let (top, mid) = (row.top() + 4.0, row.center().y + 1.0);
        let (x0, x1) = (x + 3.0, x + 10.0);
        painter.line_segment([pos2(x0, top), pos2(x0, mid)], stroke);
        painter.line_segment([pos2(x0, mid), pos2(x1, mid)], stroke);
        painter.line_segment([pos2(x1 - 3.0, mid - 3.0), pos2(x1, mid)], stroke);
        painter.line_segment([pos2(x1 - 3.0, mid + 3.0), pos2(x1, mid)], stroke);
        x += 14.0;
    }
    // Miniatura de la máscara: clic para pintarla (o volver a la capa).
    let mut right = left.right();
    if let Some(mask) = &layer.mask {
        let chip =
            Rect::from_center_size(pos2(left.right() - 12.0, row.center().y), vec2(16.0, 14.0));
        right = chip.left() - 2.0;
        let editing = active && editor.edit_mask;
        let response = ui
            .interact(chip, ui.id().with(("mask", id)), Sense::click())
            .on_hover_text(if editing {
                "Editing the mask (click to edit the layer)"
            } else {
                "Layer mask (click to paint it)"
            });
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
                if editing { theme::ACCENT } else { theme::LABEL },
            ),
            StrokeKind::Outside,
        );
        if response.clicked() {
            let editing_now = editor.layer == id && editor.edit_mask;
            editor.select_layer(id);
            editor.edit_mask = !editing_now;
        }
        response.context_menu(|ui| mask_menu(ui, editor, id));
    }
    let name_rect = Rect::from_min_max(pos2(x, row.top()), pos2(right, row.bottom()));
    let name_response = ui.interact(
        name_rect,
        ui.id().with(("name", id)),
        Sense::click_and_drag(),
    );

    let renaming = matches!(&state.renaming, Some((rid, _)) if *rid == id);
    if renaming {
        let (_, text) = state.renaming.as_mut().expect("renaming");
        let edit = ui.put(
            name_rect.shrink2(vec2(0.0, 2.0)),
            egui::TextEdit::singleline(text).font(FontId::proportional(12.0)),
        );
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        if edit.lost_focus() {
            let (_, name) = state.renaming.take().expect("renaming");
            editor.rename_layer(id, name.trim().to_string());
        }
    } else {
        let label = if layer.kind == LayerKind::Group {
            format!("📁 {}", layer.name)
        } else {
            layer.name.clone()
        };
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
        draw_drop_hint(&painter, left, drop);
    }
    if let Some(payload) = name_response.dnd_release_payload::<LayerPayload>()
        && let Some(pointer) = ui.ctx().pointer_interact_pos()
        && payload.0 != id
    {
        let drop = drop_kind(row, pointer, &layer);
        editor.move_layer(payload.0, drop);
    }
    name_response.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            state.renaming = Some((id, layer.name.clone()));
            ui.close();
        }
        if ui.button("New layer above").clicked() {
            editor.select_layer(id);
            editor.add_layer(LayerKind::Image);
            ui.close();
        }
        if ui.button("New group above").clicked() {
            editor.select_layer(id);
            editor.add_layer(LayerKind::Group);
            ui.close();
        }
        if ui.button("Delete").clicked() {
            editor.select_layer(id);
            editor.remove_layer();
            ui.close();
        }
        ui.separator();
        mask_menu(ui, editor, id);
    });

    // Cels.
    if layer.kind == LayerKind::Group {
        return;
    }
    for f in 0..editor.frame_count() {
        let r = cell_rect(row, f);
        let response = ui.interact(r, ui.id().with(("cel", id, f)), Sense::click_and_drag());
        let (sel_a, sel_b) = editor.selected_frames();
        let in_range = active && editor.frame_range.is_some() && (sel_a..=sel_b).contains(&f);
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
        let sprite = editor.sprite();
        let cel = sprite.cel(id, f).copied();
        let center = r.center();
        if let Some(cel) = cel {
            let linked_prev = f > 0 && sprite.cel(id, f - 1).is_some_and(|p| p.image == cel.image);
            if linked_prev {
                painter.hline(
                    (center.x - CELL_W)..=center.x,
                    center.y,
                    Stroke::new(2.0, ui.visuals().text_color()),
                );
            }
            painter.circle_filled(center, 4.0, ui.visuals().text_color());
        } else {
            painter.circle_stroke(
                center,
                3.5,
                Stroke::new(1.0, theme::LABEL.gamma_multiply(0.6)),
            );
        }
        // Arrastrar sobre los cels de una capa selecciona un rango de frames.
        if response.drag_started() {
            editor.stop();
            editor.select_layer(id);
            state.range_anchor = Some(f);
            state.cel_drag = Some(id);
            editor.frame_range = Some((f, f));
            editor.set_frame(f);
        }
        if state.cel_drag == Some(id) && response.contains_pointer() {
            if ui.input(|i| i.pointer.primary_down()) {
                let anchor = state.range_anchor.unwrap_or(f);
                editor.frame_range = Some((anchor, f));
            } else {
                state.cel_drag = None;
            }
        }
        if response.clicked() {
            editor.stop();
            editor.select_layer(id);
            if ui.input(|i| i.modifiers.shift) {
                let anchor = state.range_anchor.unwrap_or(editor.frame);
                editor.frame_range = Some((anchor, f));
            } else {
                state.range_anchor = Some(f);
                editor.frame_range = None;
            }
            editor.set_frame(f);
        }
        response.context_menu(|ui| {
            let (a, b) = editor.selected_frames();
            if editor.frame_range.is_some() && a != b && (a..=b).contains(&f) {
                if ui.button(format!("Link {} cels", b - a + 1)).clicked() {
                    editor.link_range(id, a, b);
                    ui.close();
                }
                if ui.button("Unlink cels").clicked() {
                    editor.unlink_range(id, a, b);
                    ui.close();
                }
                ui.separator();
            }
            let sprite = editor.sprite();
            let has_cel = sprite.cel(id, f).is_some();
            let can_link = f > 0 && sprite.cel(id, f - 1).is_some();
            let linked = ops::is_linked(sprite, id, f);
            if ui
                .add_enabled(can_link, egui::Button::new("Link to previous cel"))
                .clicked()
            {
                editor.link_to_previous(id, f);
                ui.close();
            }
            if ui
                .add_enabled(linked, egui::Button::new("Unlink"))
                .clicked()
            {
                editor.unlink(id, f);
                ui.close();
            }
            if ui
                .add_enabled(has_cel, egui::Button::new("Clear cel"))
                .clicked()
            {
                editor.clear_cel(id, f);
                ui.close();
            }
        });
    }
}

/// Dónde cae una capa soltada sobre `row` según la altura del puntero.
/// Acciones de máscara y recorte de una capa (menú contextual y menú Layer).
pub(crate) fn mask_menu(ui: &mut egui::Ui, editor: &mut Editor, id: LayerId) {
    let Some(layer) = editor.sprite().layer(id).cloned() else {
        return;
    };
    match &layer.mask {
        None => {
            ui.menu_button("Add Layer Mask", |ui| {
                if ui.button("Reveal All").clicked() {
                    editor.add_mask(id, NewMask::RevealAll);
                    ui.close();
                }
                if ui.button("Hide All").clicked() {
                    editor.add_mask(id, NewMask::HideAll);
                    ui.close();
                }
                let has_selection = editor.selection.is_some();
                if ui
                    .add_enabled(has_selection, egui::Button::new("From Selection"))
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
                .button(if editing {
                    "Edit Layer Pixels"
                } else {
                    "Edit Mask"
                })
                .clicked()
            {
                editor.select_layer(id);
                editor.edit_mask = !editing;
                ui.close();
            }
            if ui
                .button(if mask.enabled {
                    "Disable Mask"
                } else {
                    "Enable Mask"
                })
                .clicked()
            {
                editor.toggle_mask_enabled(id);
                ui.close();
            }
            if ui.button("Mask to Selection").clicked() {
                editor.mask_to_selection(id);
                ui.close();
            }
            if ui
                .add_enabled(
                    layer.kind == LayerKind::Image,
                    egui::Button::new("Apply Mask"),
                )
                .clicked()
            {
                editor.apply_mask(id);
                ui.close();
            }
            if ui.button("Delete Mask").clicked() {
                editor.delete_mask(id);
                ui.close();
            }
        }
    }
    let mut clip = layer.clip;
    let toggle = egui::Checkbox::new(&mut clip, "Clipping Mask");
    if ui
        .add(toggle)
        .on_hover_text("Show this layer only where the layer below has pixels (Ctrl+Alt+G)")
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

fn draw_drop_hint(painter: &egui::Painter, left: Rect, drop: LayerDrop) {
    let stroke = Stroke::new(2.0, theme::ACCENT);
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

fn draw_fold_arrow(painter: &egui::Painter, center: Pos2, collapsed: bool, color: Color32) {
    let s = 4.0;
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
