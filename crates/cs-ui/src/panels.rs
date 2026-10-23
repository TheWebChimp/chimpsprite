use eframe::egui::{
    self, Color32, ColorImage, Rect, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};
use std::sync::Arc;

use cs_core::{ArtboardId, Palette, PixelValue, SymmetryMode, Tile};

use crate::canvas::CanvasView;
use cs_raster::RotationAlgorithm;
use cs_tools::text::FontChoice;
use cs_tools::{BrushShape, Dither, GradientShape};

use crate::editor::{Editor, MAX_BRUSH_SIZE, SelectionMode, ToolKind};
use crate::{theme, tile_ops, widgets};

fn icon(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "🔲",
        ToolKind::EllipseSelect => "🔘",
        ToolKind::Lasso => "➰",
        ToolKind::PolygonLasso => "🔷",
        ToolKind::MagicWand => "✨",
        ToolKind::Move => "✚",
        ToolKind::Hand => "✋",
        ToolKind::Contour => "➿",
        ToolKind::Blur => "☁",
        ToolKind::Pencil => "✏",
        ToolKind::Spray => "💨",
        ToolKind::Eraser => "🗑",
        ToolKind::Eyedropper => "💧",
        ToolKind::Bucket => "🍶",
        ToolKind::Gradient => "🌈",
        ToolKind::Line => "📏",
        ToolKind::Rectangle => "⬜",
        ToolKind::Ellipse => "⭕",
        ToolKind::Jumble => "🎲",
        ToolKind::Text => "🔤",
        ToolKind::Slice => "✂",
        ToolKind::Artboard => "🖼",
    }
}

/// Mantener presionado el botón de un grupo este tiempo (en segundos) abre la lista de variantes.
const LONG_PRESS_S: f64 = 0.4;

/// Qué pasa con el botón de un grupo mantenido presionado, en este frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    /// Nada que hacer.
    Idle,
    /// Se cumplió el tiempo: abrir la lista.
    Open,
    /// Se suelta el botón con la lista abierta por esta presión: el clic no elige ni cierra.
    Release,
}

/// Lleva la cuenta de una presión larga con el botón principal sobre `response`.
fn hold_state(ui: &egui::Ui, response: &egui::Response) -> Hold {
    let id = response.id.with("long_press");
    let opened = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let (down, held) = ui.input(|i| {
        let held = i.pointer.press_start_time().map(|t| i.time - t);
        (i.pointer.primary_down(), held.unwrap_or(0.0))
    });
    if down && response.is_pointer_button_down_on() {
        if opened {
            return Hold::Idle;
        }
        if held >= LONG_PRESS_S {
            ui.data_mut(|d| d.insert_temp(id, true));
            return Hold::Open;
        }
        // Sin eventos egui no vuelve a pintar: pide un frame para cuando se cumpla el tiempo.
        ui.ctx()
            .request_repaint_after_secs((LONG_PRESS_S - held) as f32);
        return Hold::Idle;
    }
    if opened {
        ui.data_mut(|d| d.remove::<bool>(id));
        return Hold::Release;
    }
    Hold::Idle
}

/// Lista de variantes de un grupo junto al puntero. Se abre con clic derecho o manteniendo
/// presionado, y un clic normal en el botón la cierra. Usa el mismo id que `context_menu`.
fn variants_popup(response: &egui::Response, hold: Hold, add_contents: impl FnOnce(&mut egui::Ui)) {
    use egui::{Popup, PopupCloseBehavior, SetOpenCommand};
    let open = if hold == Hold::Open || response.secondary_clicked() {
        Some(SetOpenCommand::Bool(true))
    } else if response.clicked() && hold != Hold::Release {
        Some(SetOpenCommand::Bool(false))
    } else {
        None
    };
    let mut popup = Popup::menu(response).open_memory(open).at_pointer_fixed();
    if hold == Hold::Release {
        // Soltar el botón cuenta como clic: no tiene que cerrar la lista recién abierta.
        popup = popup.close_behavior(PopupCloseBehavior::IgnoreClicks);
    }
    popup.show(add_contents);
}

/// Fila de un grupo: la herramienta actual del grupo y, si hay más, un desplegable con las demás
/// (también con clic derecho o manteniendo presionado), como en Aseprite. Devuelve la respuesta
/// del botón.
fn tool_group(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    index: usize,
    group: &[ToolKind],
) -> egui::Response {
    let memory = ui.id().with(("tool_group", index));
    if group.contains(&editor.tool) {
        ui.data_mut(|d| d.insert_temp(memory, editor.tool));
    }
    let shown = ui
        .data(|d| d.get_temp::<ToolKind>(memory))
        .unwrap_or(group[0]);
    let mut pick = None;
    let response = ui
        .horizontal(|ui| {
            let more = group.len() > 1;
            ui.spacing_mut().item_spacing.x = 2.0;
            let width = ui.available_width() - if more { 22.0 } else { 0.0 };
            let label = format!("{} {}", icon(shown), shown.label());
            let button = egui::Button::selectable(editor.tool == shown, label)
                .shortcut_text("")
                .truncate();
            let response = ui.add_sized([width, 24.0], button).on_hover_text(format!(
                "{} ({})",
                shown.label(),
                shown.shortcut()
            ));
            let hold = if more {
                hold_state(ui, &response)
            } else {
                Hold::Idle
            };
            if response.clicked() && hold != Hold::Release {
                pick = Some(shown);
            }
            if more {
                let current = editor.tool;
                variants_popup(&response, hold, |ui| {
                    group_menu(ui, current, group, &mut pick)
                });
                ui.menu_button("⏷", |ui| group_menu(ui, current, group, &mut pick))
                    .response
                    .on_hover_text("More tools (or right-click, or hold)");
            }
            response
        })
        .inner;
    if let Some(kind) = pick {
        editor.set_tool(kind);
    }
    response
}

fn group_menu(
    ui: &mut egui::Ui,
    current: ToolKind,
    group: &[ToolKind],
    pick: &mut Option<ToolKind>,
) {
    for &kind in group {
        let label = format!("{} {}", icon(kind), kind.label());
        let item = egui::Button::selectable(current == kind, label).shortcut_text(kind.shortcut());
        if ui.add(item).clicked() {
            *pick = Some(kind);
            ui.close();
        }
    }
}

pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.add_space(6.0);
    ui.heading("Tools");
    ui.separator();
    ui.spacing_mut().item_spacing.y = 3.0;
    for (index, group) in ToolKind::GROUPS.iter().enumerate() {
        tool_group(ui, editor, index, group);
    }

    ui.add_space(10.0);
    ui.heading("Options");
    ui.separator();
    tool_options(ui, editor);

    ui.add_space(10.0);
    ui.heading("Symmetry");
    ui.separator();
    symmetry_buttons(ui, editor);
    text_window(ui.ctx(), editor);
}

fn tool_options(ui: &mut egui::Ui, editor: &mut Editor) {
    let tool = editor.tool;
    let options = &mut editor.options;
    if tool.uses_brush() {
        ui.horizontal(|ui| {
            ui.label("Brush size");
            brush_shape_buttons(ui, &mut options.brush_shape);
        });
        ui.add(egui::Slider::new(
            &mut options.brush_size,
            1..=MAX_BRUSH_SIZE,
        ))
        .on_hover_text("Ctrl+wheel over the canvas changes it");
    }
    if matches!(tool, ToolKind::Pencil | ToolKind::Eraser) {
        ui.checkbox(&mut options.pixel_perfect, "Pixel-perfect");
    }
    if matches!(tool, ToolKind::Rectangle | ToolKind::Ellipse) {
        ui.checkbox(&mut options.filled, "Filled");
    }
    if tool == ToolKind::Rectangle {
        ui.label("Corner radius");
        ui.add(egui::Slider::new(&mut options.corner_radius, 0..=32))
            .on_hover_text("0 = square corners; large values make a pill or a circle");
    }
    if tool.is_selection() {
        ui.label("Mode");
        for mode in SelectionMode::ALL {
            ui.radio_value(&mut options.selection_mode, mode, mode.label());
        }
    }
    match tool {
        ToolKind::Hand => {
            ui.weak("Drag to move the view. Space+drag or the middle button do it with any tool.");
        }
        ToolKind::MagicWand => {
            ui.label("Tolerance");
            ui.add(egui::Slider::new(&mut options.tolerance, 0..=255));
            ui.checkbox(&mut options.contiguous, "Contiguous");
            ui.horizontal(|ui| {
                ui.label("Sample:");
                ui.radio_value(&mut options.sample_all, false, "Layer");
                ui.radio_value(&mut options.sample_all, true, "All layers");
            });
        }
        ToolKind::Lasso => {
            ui.weak("Drag around the area; it closes when you release.");
        }
        ToolKind::PolygonLasso => {
            ui.weak("Click to add points. Click the first point, double-click or press Enter to close; Esc cancels.");
        }
        ToolKind::Bucket => {
            ui.checkbox(&mut options.contiguous, "Contiguous");
        }
        ToolKind::Spray => {
            ui.label("Spray width");
            ui.add(egui::Slider::new(&mut options.spray_width, 1..=64));
            ui.label("Spray speed");
            ui.add(egui::Slider::new(&mut options.spray_speed, 1..=100));
        }
        ToolKind::Gradient => {
            ui.horizontal(|ui| {
                ui.radio_value(&mut options.gradient_shape, GradientShape::Linear, "Linear");
                ui.radio_value(&mut options.gradient_shape, GradientShape::Radial, "Radial");
            });
            egui::ComboBox::from_id_salt("dither")
                .selected_text(options.dither.label())
                .show_ui(ui, |ui| {
                    for d in Dither::ALL {
                        ui.selectable_value(&mut options.dither, d, d.label());
                    }
                });
            ui.weak("Drag from the primary to the secondary color. Right-drag swaps them.");
        }
        ToolKind::Jumble => {
            ui.weak("Scrambles pixels within the brush radius.");
        }
        ToolKind::Contour => {
            ui.weak("Draw the outline: it closes and fills when you release.");
        }
        ToolKind::Blur => {
            ui.weak("Softens the pixels under the brush. Go over an area again to blur it more.");
        }
        ToolKind::Text => {
            text_style_options(ui, editor);
            ui.weak("Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            ui.weak("Drag to create a slice (hitbox, pivot…). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.");
        }
        _ => {}
    }
    if tool.is_selection() || tool == ToolKind::Move {
        let before = editor.options.rotation;
        ui.horizontal(|ui| {
            ui.label("Rotation:");
            for algorithm in RotationAlgorithm::ALL {
                ui.radio_value(&mut editor.options.rotation, algorithm, algorithm.label());
            }
        });
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        ui.weak("Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.");
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Gradient
    ) || matches!(tool, ToolKind::RectSelect | ToolKind::EllipseSelect)
    {
        ui.weak("Shift: constrain");
    }
    if tool.paints() {
        ui.weak("Alt+click: pick color");
    }
}

/// Botones Round y Square de la forma del pincel. El círculo y el cuadrado se pintan en vez de
/// usar glifos: `●` solo está en Hack, que no entra en la familia proporcional de egui.
fn brush_shape_buttons(ui: &mut egui::Ui, shape: &mut BrushShape) -> [egui::Response; 2] {
    ui.horizontal(|ui| {
        [(BrushShape::Round, "Round"), (BrushShape::Square, "Square")].map(|(value, name)| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            if response.clicked() {
                *shape = value;
            }
            let selected = *shape == value;
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, name)
            });
            let visuals = ui.style().interact_selectable(&response, selected);
            let painter = ui.painter();
            painter.rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                StrokeKind::Inside,
            );
            let icon = rect.shrink(5.0);
            let color = visuals.fg_stroke.color;
            if value == BrushShape::Round {
                painter.circle_filled(icon.center(), icon.width() / 2.0, color);
            } else {
                painter.rect_filled(icon, 0.0, color);
            }
            response.on_hover_text(name)
        })
    })
    .inner
}

fn text_style_options(ui: &mut egui::Ui, editor: &mut Editor) {
    let style = &mut editor.text_style;
    let mut fonts = vec![FontChoice::Pixel, FontChoice::Hack, FontChoice::Ubuntu];
    if let FontChoice::Custom { .. } = style.font {
        fonts.push(style.font.clone());
    }
    egui::ComboBox::from_label("Font")
        .selected_text(style.font.name())
        .show_ui(ui, |ui| {
            for font in fonts {
                let name = font.name();
                let selected = style.font == font;
                if ui.selectable_label(selected, name).clicked() && !selected {
                    style.size = if font == FontChoice::Pixel { 1 } else { 16 };
                    style.font = font;
                }
            }
        });
    #[cfg(not(target_arch = "wasm32"))]
    if ui.button("Load font…").clicked()
        && let Some(path) = rfd::FileDialog::new()
            .add_filter("Fonts", &["ttf", "otf"])
            .pick_file()
    {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("Font")
            .to_string();
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| FontChoice::load(name, &bytes))
        {
            Ok(font) => {
                style.font = font;
                style.size = 16;
            }
            Err(e) => log::warn!("could not load font {}: {e}", path.display()),
        }
    }
    if style.font == FontChoice::Pixel {
        ui.add(egui::Slider::new(&mut style.size, 1..=8).text("Scale"));
    } else {
        ui.add(egui::Slider::new(&mut style.size, 6..=96).text("Size"));
        ui.checkbox(&mut style.antialias, "Antialias");
    }
}

/// Ventana flotante para escribir el texto que se está colocando.
fn text_window(ctx: &egui::Context, editor: &mut Editor) {
    if editor.text.is_none() {
        return;
    }
    let mut commit = false;
    let mut cancel = false;
    egui::Window::new("Text")
        .collapsible(false)
        .resizable(false)
        .default_width(260.0)
        .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 90.0))
        .show(ctx, |ui| {
            text_style_options(ui, editor);
            let Some(draft) = editor.text.as_mut() else {
                return;
            };
            let edit = egui::TextEdit::multiline(&mut draft.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Type here…");
            let response = ui.add(edit);
            if draft.text.is_empty() && !response.has_focus() {
                response.request_focus();
            }
            if response.changed() {
                editor.revision += 1;
            }
            let (enter, esc) = ui.input(|i| {
                (
                    i.modifiers.command && i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            ui.weak("Click the canvas to move it. Ctrl+Enter applies.");
            ui.horizontal(|ui| {
                commit = ui.button("OK").clicked() || enter;
                cancel = ui.button("Cancel").clicked() || esc;
            });
        });
    if commit {
        editor.commit_text();
    } else if cancel {
        editor.cancel_gesture();
    }
}

const SYMMETRY_MODES: [(SymmetryMode, &str); 7] = [
    (SymmetryMode::None, "Off"),
    (SymmetryMode::Horizontal, "Horizontal (left ↔ right)"),
    (SymmetryMode::Vertical, "Vertical (top ↔ bottom)"),
    (SymmetryMode::Both, "Horizontal + vertical"),
    (SymmetryMode::DiagonalDown, "Diagonal ╲"),
    (SymmetryMode::DiagonalUp, "Diagonal ╱"),
    (SymmetryMode::All, "All 8 directions"),
];

fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor) {
    let current = editor.symmetry();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        for (mode, tip) in SYMMETRY_MODES {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let selected = current.mode == mode;
            let visuals = ui.style().interact_selectable(&response, selected);
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, visuals.weak_bg_fill);
            let color = if selected {
                theme::ACCENT
            } else {
                visuals.fg_stroke.color
            };
            draw_symmetry_icon(painter, rect.shrink(5.0), mode, color);
            if response.on_hover_text(tip).clicked() {
                editor.set_symmetry(cs_core::Symmetry { mode, ..current });
            }
        }
    });
    if current.mode != SymmetryMode::None {
        ui.weak("Ctrl+drag the dashed axes to move them");
        if current.center2.is_some() && ui.small_button("Center axes").clicked() {
            editor.set_symmetry(cs_core::Symmetry {
                center2: None,
                ..current
            });
        }
    }
}

fn draw_symmetry_icon(painter: &egui::Painter, r: Rect, mode: SymmetryMode, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    painter.rect_stroke(
        r,
        1.0,
        Stroke::new(1.0, color.gamma_multiply(0.5)),
        StrokeKind::Inside,
    );
    let (c, tl, br) = (r.center(), r.left_top(), r.right_bottom());
    let (tr, bl) = (r.right_top(), r.left_bottom());
    let vertical = || painter.vline(c.x, r.y_range(), stroke);
    let horizontal = || painter.hline(r.x_range(), c.y, stroke);
    let down = || painter.line_segment([tl, br], stroke);
    let up = || painter.line_segment([bl, tr], stroke);
    match mode {
        SymmetryMode::None => {
            painter.circle_filled(c, 1.5, color);
        }
        SymmetryMode::Horizontal => {
            vertical();
        }
        SymmetryMode::Vertical => {
            horizontal();
        }
        SymmetryMode::Both => {
            vertical();
            horizontal();
        }
        SymmetryMode::DiagonalDown => {
            down();
        }
        SymmetryMode::DiagonalUp => {
            up();
        }
        SymmetryMode::All => {
            vertical();
            horizontal();
            down();
            up();
        }
    }
}

/// Damero mínimo para que se note la transparencia.
fn checker(painter: &egui::Painter, rect: Rect) {
    let half = rect.size() / 2.0;
    painter.rect_filled(rect, 0.0, theme::CHECKER_LIGHT);
    painter.rect_filled(
        Rect::from_min_size(rect.min, half),
        0.0,
        theme::CHECKER_DARK,
    );
    painter.rect_filled(
        Rect::from_min_size(rect.min + half, half),
        0.0,
        theme::CHECKER_DARK,
    );
}

fn swatch(ui: &mut egui::Ui, color: Color32, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    if color.a() < 255 {
        checker(painter, rect);
    }
    painter.rect_filled(rect, 0.0, color);
    response
}

/// Marco blanco si `value` es el color primario; del color de acento si es el secundario.
fn mark_selected(ui: &egui::Ui, editor: &Editor, value: PixelValue, rect: Rect) {
    let color = if value == editor.primary {
        Color32::WHITE
    } else if value == editor.secondary {
        theme::ACCENT
    } else {
        return;
    };
    ui.painter()
        .rect_stroke(rect, 0.0, Stroke::new(2.0, color), StrokeKind::Inside);
}

/// Swatch del color Mask (transparente): tablero con una diagonal roja.
fn mask_swatch(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    paint_mask(ui.painter(), rect);
    response
}

fn paint_mask(painter: &egui::Painter, rect: Rect) {
    checker(painter, rect);
    painter.line_segment(
        [rect.left_bottom(), rect.right_top()],
        Stroke::new(2.0, Color32::from_rgb(220, 40, 40)),
    );
}

/// Swatch del color primario o secundario; el Mask se dibuja como tal.
fn color_swatch(
    ui: &mut egui::Ui,
    editor: &Editor,
    value: PixelValue,
    size: f32,
) -> egui::Response {
    if editor.is_mask(value) {
        mask_swatch(ui, size)
    } else {
        swatch(ui, to_color32(editor.resolve(value)), size)
    }
}

fn to_color32(c: cs_core::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

/// Lo que se pidió desde el encabezado del panel de paleta (lo resuelve `ChimpApp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteAction {
    OpenPresets,
    Load,
    Save,
    SaveAsPreset,
    SetAsDefault,
    LoadDefault,
    /// Solo escritorio: en web no hay carpeta que abrir.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    OpenFolder,
}

/// Tira de colores de una paleta: como mucho `max` y termina en "…" si hay más.
pub fn palette_strip(ui: &mut egui::Ui, palette: &Palette, max: usize, size: f32) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::ZERO;
        for &c in palette.colors().iter().take(max) {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
            ui.painter().rect_filled(rect, 0.0, to_color32(c));
        }
        if palette.len() > max {
            ui.weak("…");
        }
    });
}

/// `menus`: con un diálogo abierto, Presets ⏷ y … quedan desactivados.
pub fn palette(ui: &mut egui::Ui, editor: &mut Editor, menus: bool) -> Option<PaletteAction> {
    ui.add_space(6.0);
    ui.heading("Color");
    ui.separator();
    ui.horizontal(|ui| {
        let name = |v| {
            if editor.is_mask(v) {
                " · Mask (transparent)"
            } else {
                ""
            }
        };
        color_swatch(ui, editor, editor.primary, 36.0)
            .on_hover_text(format!("Primary (left click){}", name(editor.primary)));
        color_swatch(ui, editor, editor.secondary, 24.0)
            .on_hover_text(format!("Secondary (right click){}", name(editor.secondary)));
        if ui.small_button("🔄").on_hover_text("Swap (X)").clicked() {
            editor.swap_colors();
        }
    });
    ui.add_space(8.0);
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label("Palette");
        ui.add_enabled_ui(menus, |ui| {
            if ui.small_button("Presets ⏷").clicked() {
                action = Some(PaletteAction::OpenPresets);
            }
            ui.menu_button("…", |ui| {
                let mut item = |ui: &mut egui::Ui, label: &str, a: PaletteAction| {
                    if ui.button(label).clicked() {
                        action = Some(a);
                        ui.close();
                    }
                };
                item(ui, "Load Palette…", PaletteAction::Load);
                item(ui, "Save Palette…", PaletteAction::Save);
                item(ui, "Save as Preset…", PaletteAction::SaveAsPreset);
                ui.separator();
                item(ui, "Set as Default", PaletteAction::SetAsDefault);
                item(ui, "Load Default", PaletteAction::LoadDefault);
                #[cfg(not(target_arch = "wasm32"))]
                {
                    ui.separator();
                    item(ui, "Open Folder", PaletteAction::OpenFolder);
                }
            });
        });
    });
    let size = 20.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        let mask = editor.mask_value();
        let response =
            mask_swatch(ui, size).on_hover_text("Mask (transparent): painting with it erases");
        mark_selected(ui, editor, mask, response.rect);
        if response.clicked() {
            editor.primary = mask;
        }
        if response.secondary_clicked() {
            editor.secondary = mask;
        }
        let count = editor.sprite().palette.len().min(256);
        for i in 0..count {
            let index = i as u8;
            let value = editor.palette_value(index);
            let color = to_color32(editor.sprite().palette.colors()[i]);
            let response = swatch(ui, color, size).on_hover_text(format!("#{i}"));
            mark_selected(ui, editor, value, response.rect);
            if response.clicked() {
                editor.primary = value;
            }
            if response.secondary_clicked() {
                editor.secondary = value;
            }
        }
    });
    ui.add_space(10.0);
    color_picker(ui, editor);
    action
}

/// Lado máximo del cuadro del selector de color, en puntos.
const COLOR_PICKER_MAX: f32 = 240.0;

/// Selector de color libre (RGBA): edita el color primario.
pub fn color_picker(ui: &mut egui::Ui, editor: &mut Editor) {
    let cs_core::PixelValue::Rgba(current) = editor.primary else {
        ui.weak("Indexed sprite: colors come from the palette.");
        return;
    };
    let mut c32 = to_color32(current);
    // El cuadro de saturación/brillo ocupa el ancho del panel hasta `COLOR_PICKER_MAX` (es
    // cuadrado: con un sidebar ancho se haría enorme). Se restaura después: si no, los sliders de
    // los paneles siguientes heredan ese ancho y agrandan el panel en cada frame.
    let slider_width = ui.spacing().slider_width;
    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).clamp(120.0, COLOR_PICKER_MAX);
    let picked = egui::color_picker::color_picker_color32(
        ui,
        &mut c32,
        egui::color_picker::Alpha::OnlyBlend,
    );
    ui.spacing_mut().slider_width = slider_width;
    if picked {
        let [r, g, b, a] = c32.to_srgba_unmultiplied();
        editor.primary = cs_core::PixelValue::Rgba(cs_core::Rgba::new(r, g, b, a));
    }
    ui.horizontal(|ui| {
        let id = ui.id().with("hex");
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| current.to_hex());
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(90.0)
                .font(egui::TextStyle::Monospace),
        );
        if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if response.lost_focus()
            && let Some(c) = cs_core::Rgba::parse_hex(&text)
        {
            editor.primary = cs_core::PixelValue::Rgba(c);
        }
        let in_palette = editor.sprite().palette.colors().contains(&current);
        if ui
            .add_enabled(!in_palette, egui::Button::new("+").small())
            .on_hover_text("Add to palette")
            .clicked()
        {
            editor.add_to_palette(current);
        }
    });
}

/// Lado de las miniaturas del panel Tileset.
const THUMB: f32 = 32.0;

/// Identidad de las miniaturas: artboard, tileset, tamaño de tile, el `Arc` de cada tile (pintar
/// celdas no los cambia; reemplazar la imagen de un tile sí) y la paleta (los indexados se
/// resuelven con ella).
#[derive(PartialEq)]
struct ThumbKey {
    artboard: ArtboardId,
    index: u32,
    tile_size: (u32, u32),
    tiles: Vec<usize>,
    palette: cs_core::Palette,
}

/// Miniaturas de los tiles del tileset activo, cacheadas por la identidad del tileset (no por la
/// versión del documento: cada trazo de tiles la cambia).
#[derive(Default)]
pub struct TileThumbs {
    key: Option<ThumbKey>,
    /// Una por tile (la 0 es el vacío y no se dibuja).
    textures: Vec<TextureHandle>,
}

impl TileThumbs {
    fn refresh(&mut self, ctx: &egui::Context, editor: &Editor) {
        let Some((index, ts)) = editor.active_tileset() else {
            self.key = None;
            self.textures.clear();
            return;
        };
        let sprite = editor.sprite();
        let key = ThumbKey {
            artboard: editor.active,
            index,
            tile_size: ts.tile_size,
            tiles: ts.tiles.iter().map(|t| Arc::as_ptr(t) as usize).collect(),
            palette: sprite.palette.clone(),
        };
        if self.key.as_ref() == Some(&key) {
            return;
        }
        self.textures = ts
            .tiles
            .iter()
            .enumerate()
            .map(|(i, tile)| {
                let bytes: Vec<u8> = (0..tile.height())
                    .flat_map(|y| (0..tile.width()).map(move |x| (x, y)))
                    .flat_map(|(x, y)| {
                        let c = tile.get(x, y).map_or(cs_core::Rgba::TRANSPARENT, |v| {
                            cs_render::resolve(sprite, v)
                        });
                        [c.r, c.g, c.b, c.a]
                    })
                    .collect();
                let size = [tile.width() as usize, tile.height() as usize];
                let image = ColorImage::from_rgba_unmultiplied(size, &bytes);
                let name = format!("tile-{}-{index}-{i}", editor.active.0);
                ctx.load_texture(name, image, TextureOptions::NEAREST)
            })
            .collect();
        self.key = Some(key);
    }
}

/// Panel "Tileset" (reemplaza a la paleta con una capa tilemap activa): elegir el tile del pincel
/// y sus flips.
pub fn tileset(ui: &mut egui::Ui, editor: &mut Editor, thumbs: &mut TileThumbs) {
    thumbs.refresh(ui.ctx(), editor);
    let Some((_, ts)) = editor.active_tileset() else {
        return;
    };
    let (name, (tw, th), count, base) =
        (ts.name.clone(), ts.tile_size, ts.tiles.len(), ts.base_index);
    ui.add_space(6.0);
    ui.heading("Tileset");
    ui.separator();
    ui.label(format!(
        "{name}  {tw}×{th}  {} tiles",
        count.saturating_sub(1)
    ));
    let brush = editor.brush_tile();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        for i in 0..count {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(THUMB + 4.0), Sense::click());
            let inner = rect.shrink(2.0);
            let painter = ui.painter();
            checker(painter, inner);
            if i == 0 {
                // El vacío: el dibujo de "transparente".
                painter.line_segment(
                    [inner.left_bottom(), inner.right_top()],
                    Stroke::new(1.5, Color32::from_rgb(220, 60, 60)),
                );
            } else if let Some(tex) = thumbs.textures.get(i) {
                let scale = THUMB / tw.max(th) as f32;
                let size = Vec2::new(tw as f32 * scale, th as f32 * scale);
                let uv = Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0));
                painter.image(
                    tex.id(),
                    Rect::from_center_size(inner.center(), size),
                    uv,
                    Color32::WHITE,
                );
            }
            let number = tile_ops::tile_number(i as u32, base);
            widgets::outlined_text(
                painter,
                inner.left_top() + Vec2::new(1.0, 0.0),
                egui::Align2::LEFT_TOP,
                &number.to_string(),
                egui::FontId::monospace(9.0),
            );
            if brush.index() as usize == i {
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(2.0, Color32::WHITE),
                    StrokeKind::Inside,
                );
            }
            if response.on_hover_text(format!("Tile {number}")).clicked() {
                editor.set_brush_tile(Tile::new(
                    i as u32,
                    brush.flip_x(),
                    brush.flip_y(),
                    brush.flip_d(),
                ));
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Flip:");
        let brush = editor.brush_tile();
        if ui
            .selectable_label(brush.flip_x(), "X")
            .on_hover_text("Flip horizontally (X)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_X);
        }
        if ui
            .selectable_label(brush.flip_y(), "Y")
            .on_hover_text("Flip vertically (Y)")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_Y);
        }
        if ui
            .add_enabled(tw == th, egui::Button::selectable(brush.flip_d(), "D"))
            .on_hover_text("Flip diagonally (swap X and Y)")
            .on_disabled_hover_text("Needs square tiles")
            .clicked()
        {
            editor.toggle_brush_flip(Tile::FLIP_D);
        }
    });
}

/// `notice`: aviso breve al final de la barra (p. ej. autosave desactivado).
pub fn status_bar(ui: &mut egui::Ui, editor: &Editor, canvas: &CanvasView, notice: Option<&str>) {
    ui.horizontal(|ui| {
        let s = editor.sprite();
        ui.label(format!(
            "{}  {}×{}",
            editor.artboard().name,
            s.width,
            s.height
        ));
        ui.separator();
        match canvas.hover {
            Some((x, y)) if x >= 0 && y >= 0 && x < s.width as i32 && y < s.height as i32 => {
                ui.label(format!("{x}, {y}"));
            }
            _ => {
                ui.label("—");
            }
        }
        ui.separator();
        ui.label(format!("{}%", (canvas.zoom * 100.0) as u32));
        ui.separator();
        ui.label(format!("Frame {}/{}", editor.frame + 1, s.frames.len()));
        if editor.editing_mask() {
            ui.separator();
            ui.colored_label(
                theme::ACCENT,
                "Editing layer mask: white reveals, black hides",
            );
        }
        let on_instance = s
            .layer(editor.layer)
            .is_some_and(|l| l.kind == cs_core::LayerKind::Instance);
        if on_instance && editor.active_tool().paints() {
            ui.separator();
            ui.colored_label(theme::ACCENT, "Instance layer: edit the source artboard");
        }
        if let Some(text) = editor.tiles_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
        if let Some(text) = editor.floating_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
        }
        if let Some(hint) = canvas.guide_hint {
            ui.separator();
            ui.colored_label(theme::GUIDE_LINE, hint);
        }
        if let Some(notice) = notice {
            ui.separator();
            ui.colored_label(theme::ACCENT, notice);
        }
    });
}

/// Lista de artboards: activar, crear, renombrar y eliminar.
/// Devuelve `true` si se pidió abrir el diálogo de tamaño.
pub fn artboards(
    ui: &mut egui::Ui,
    editor: &mut Editor,
    name_buffer: &mut (ArtboardId, String),
) -> bool {
    let mut resize = false;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.heading("Artboards");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_remove = editor.document.artboards.len() > 1;
            if ui
                .add_enabled(can_remove, egui::Button::new("🗑").small())
                .on_hover_text("Delete artboard")
                .clicked()
            {
                editor.remove_active_artboard();
            }
            if ui
                .small_button("➕")
                .on_hover_text("New 64×64 artboard")
                .clicked()
            {
                editor.add_artboard(64, 64);
            }
        });
    });
    ui.separator();

    let ids: Vec<(ArtboardId, String)> = editor
        .document
        .artboards
        .iter()
        .rev()
        .map(|a| (a.id, a.name.clone()))
        .collect();
    for (id, name) in ids {
        if ui.selectable_label(id == editor.active, name).clicked() {
            editor.activate(id);
        }
    }

    ui.add_space(4.0);
    if name_buffer.0 != editor.active {
        *name_buffer = (editor.active, editor.artboard().name.clone());
    }
    let response = ui.add(egui::TextEdit::singleline(&mut name_buffer.1).hint_text("Name"));
    if response.lost_focus() {
        let name = name_buffer.1.trim().to_string();
        if name.is_empty() {
            name_buffer.1 = editor.artboard().name.clone();
        } else {
            editor.rename_active_artboard(name);
        }
    } else if !response.has_focus() {
        name_buffer.1 = editor.artboard().name.clone();
    }
    if ui.button("Canvas Size…").clicked() {
        resize = true;
    }
    resize
}

/// Gesto sobre campos numéricos o sliders: un solo paso de undo por arrastre o edición.
/// Llamar antes de aplicar el cambio; devuelve el grupo (id del widget que cambió) si cambió.
/// El gesto anterior del mismo widget ya terminó en `gesture_end` (al soltar o perder el foco),
/// así que acá solo se corta al tomar foco; cortar en `drag_started` partiría en dos el
/// "clic (el slider salta) + arrastre".
fn gesture_group(editor: &mut Editor, responses: &[&egui::Response]) -> Option<u64> {
    if responses.iter().any(|r| r.gained_focus()) {
        editor.end_group();
    }
    responses.iter().find(|r| r.changed()).map(|r| r.id.value())
}

/// Llamar después de aplicar: cierra el gesto si terminó (al soltar el botón, al perder el foco,
/// o si el cambio no vino de un botón apretado ni de un campo con foco).
fn gesture_end(editor: &mut Editor, responses: &[&egui::Response]) {
    let ended = responses.iter().any(|r| {
        r.drag_stopped()
            || r.clicked()
            || r.lost_focus()
            || (r.changed() && !r.is_pointer_button_down_on() && !r.has_focus())
    });
    if ended {
        editor.end_group();
    }
}

/// Rango de los campos de posición de la instancia.
const POSITION_RANGE: std::ops::RangeInclusive<i32> = -16384..=16384;

/// Propiedades de la instancia de la capa actual en el frame actual. Devuelve `true` si se
/// entró al símbolo (el lienzo debe centrarlo).
pub fn instance(ui: &mut egui::Ui, editor: &mut Editor) -> bool {
    let Some(key) = editor.instance_key() else {
        return false;
    };
    ui.heading("Instance");
    let is_key = editor.is_instance_keyframe(editor.layer, editor.frame);
    if !is_key {
        ui.weak("Changes create a keyframe on this frame.");
    }
    let names: Vec<(cs_core::ArtboardId, String)> = editor.instance_candidates();
    let current = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or("(missing)".to_string(), |a| a.name.clone());
    egui::ComboBox::from_label("Source")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (id, name) in names {
                if ui.selectable_label(key.source == Some(id), name).clicked() {
                    editor.update_instance_key(|k| k.source = Some(id));
                }
            }
        });
    let (mut x, mut y) = key.position;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("Position");
            let rx = widgets::number(ui, &mut x, POSITION_RANGE, "");
            let ry = widgets::number(ui, &mut y, POSITION_RANGE, "");
            (rx, ry)
        })
        .inner;
    if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
        editor.update_instance_key_grouped(Some(group), |k| k.position = (x, y));
    }
    gesture_end(editor, &[&rx, &ry]);
    let (mut fh, mut fv) = (key.flip_h, key.flip_v);
    ui.horizontal(|ui| {
        if ui.checkbox(&mut fh, "Flip H").changed() {
            editor.update_instance_key(|k| k.flip_h = fh);
        }
        if ui.checkbox(&mut fv, "Flip V").changed() {
            editor.update_instance_key(|k| k.flip_v = fv);
        }
    });
    let mut opacity = key.opacity;
    let r = ui.add(egui::Slider::new(&mut opacity, 0..=255).text("Opacity"));
    if let Some(group) = gesture_group(editor, &[&r]) {
        editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
    }
    gesture_end(editor, &[&r]);
    use cs_core::{GraphicPlay, Timing};
    let (mut first, mut mode) = match key.timing {
        Timing::Graphic { first_frame, play } => (first_frame, Some(play)),
        Timing::MovieClip { first_frame } => (first_frame, None),
    };
    let before = (first, mode);
    ui.horizontal(|ui| {
        ui.selectable_value(&mut mode, Some(GraphicPlay::Loop), "Loop")
            .on_hover_text("Graphic: follows this artboard's frames, looping");
        ui.selectable_value(&mut mode, Some(GraphicPlay::Once), "Once")
            .on_hover_text("Graphic: plays once and holds the last frame");
        ui.selectable_value(&mut mode, Some(GraphicPlay::Single), "Single")
            .on_hover_text("Graphic: always the same frame");
        ui.selectable_value(&mut mode, None, "Movie clip")
            .on_hover_text("Plays on its own clock with its own frame durations");
    });
    let frames = key
        .source
        .and_then(|s| editor.document.artboard(s))
        .map_or(1, |a| a.sprite.frames.len());
    let first_response = ui
        .horizontal(|ui| {
            ui.label("First frame");
            let mut shown = first + 1;
            let r = widgets::number(ui, &mut shown, 1..=frames, "");
            if r.changed() {
                first = shown - 1;
            }
            r
        })
        .inner;
    // El modo es un clic (paso propio); el primer frame se arrastra (un paso por gesto).
    let group = gesture_group(editor, &[&first_response]).filter(|_| mode == before.1);
    if (first, mode) != before {
        editor.update_instance_key_grouped(group, |k| {
            k.timing = match mode {
                Some(play) => Timing::Graphic {
                    first_frame: first,
                    play,
                },
                None => Timing::MovieClip { first_frame: first },
            }
        });
    }
    gesture_end(editor, &[&first_response]);
    let entered = ui
        .button("Edit Symbol")
        .on_hover_text("Go to the source artboard (double-click with Move)")
        .clicked()
        && editor.enter_instance();
    ui.separator();
    entered
}

/// Panel de slices: lista, nombre, color, datos y la clave del frame actual (rect y pivote).
pub fn slices(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.horizontal(|ui| {
        ui.heading("Slices");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(editor.slice.is_some(), egui::Button::new("🗑").small())
                .on_hover_text("Delete slice")
                .clicked()
            {
                editor.delete_slice();
            }
        });
    });
    ui.separator();
    let names: Vec<(String, cs_core::Rgba)> = editor
        .sprite()
        .slices
        .iter()
        .map(|s| (s.name.clone(), s.color))
        .collect();
    if names.is_empty() {
        ui.weak("No slices. Use the Slice tool (Shift+C) to mark hitboxes and pivots.");
        return;
    }
    for (i, (name, color)) in names.iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
            ui.painter()
                .rect_filled(r, 2.0, Color32::from_rgb(color.r, color.g, color.b));
            if ui.selectable_label(editor.slice == Some(i), name).clicked() {
                editor.end_group();
                editor.slice = Some(i);
                editor.set_tool(ToolKind::Slice);
            }
        });
    }
    if editor.slice.is_some() {
        ui.separator();
        slice_properties(ui, editor);
    }
}

/// Nombre, color, datos y clave del frame actual del slice seleccionado (panel y diálogo).
pub fn slice_properties(ui: &mut egui::Ui, editor: &mut Editor) {
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    let mut name = slice.name.clone();
    ui.horizontal(|ui| {
        ui.label("Name");
        if ui.text_edit_singleline(&mut name).lost_focus() && name != slice.name {
            editor.update_slice(|s| s.name = name.clone());
        }
    });
    let mut c32 = Color32::from_rgb(slice.color.r, slice.color.g, slice.color.b);
    ui.horizontal(|ui| {
        ui.label("Color");
        if ui.color_edit_button_srgba(&mut c32).changed() {
            let [r, g, b, _] = c32.to_array();
            editor.update_slice(|s| s.color = cs_core::Rgba::rgb(r, g, b));
        }
    });
    let mut text = slice.text.clone();
    ui.horizontal(|ui| {
        ui.label("Data");
        if ui
            .text_edit_singleline(&mut text)
            .on_hover_text("Free text exported with the slice (e.g. damage=3)")
            .lost_focus()
            && text != slice.text
        {
            editor.update_slice(|s| s.text = text.clone());
        }
    });
    let frame = editor.frame;
    let Some(key) = slice.key_at(frame).copied() else {
        ui.weak(format!("Hidden on frame {}", frame + 1));
        return;
    };
    let own_key = key.frame == frame;
    ui.label(if own_key {
        format!("Frame {} (own key)", frame + 1)
    } else {
        format!("Frame {} (from frame {})", frame + 1, key.frame + 1)
    });
    let (mut x, mut y, mut w, mut h) = key.bounds;
    let (rx, ry) = ui
        .horizontal(|ui| {
            ui.label("X");
            let rx = widgets::number(ui, &mut x, -4096..=4096, "");
            ui.label("Y");
            let ry = widgets::number(ui, &mut y, -4096..=4096, "");
            (rx, ry)
        })
        .inner;
    let (rw, rh) = ui
        .horizontal(|ui| {
            ui.label("W");
            let rw = widgets::number(ui, &mut w, 1..=4096, "");
            ui.label("H");
            let rh = widgets::number(ui, &mut h, 1..=4096, "");
            (rw, rh)
        })
        .inner;
    let rect_fields = [&rx, &ry, &rw, &rh];
    if let Some(group) = gesture_group(editor, &rect_fields) {
        editor.update_slice_key_grouped(Some(group), |k| k.bounds = (x, y, w, h));
    }
    gesture_end(editor, &rect_fields);
    let mut has_pivot = key.pivot.is_some();
    if ui.checkbox(&mut has_pivot, "Pivot").changed() {
        let default = (w as i32 / 2, h as i32);
        editor.update_slice_key(|k| k.pivot = has_pivot.then_some(default));
    }
    if let Some((mut px, mut py)) = key.pivot {
        let (rx, ry) = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut px, -4096..=4096, "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut py, -4096..=4096, "");
                (rx, ry)
            })
            .inner;
        if let Some(group) = gesture_group(editor, &[&rx, &ry]) {
            editor.update_slice_key_grouped(Some(group), |k| k.pivot = Some((px, py)));
        }
        gesture_end(editor, &[&rx, &ry]);
    }
    // 9-slice: el rect central (relativo al slice) se estira; las esquinas quedan fijas.
    let mut nine = key.center.is_some();
    if ui
        .checkbox(&mut nine, "9-Slice")
        .on_hover_text("Center area that stretches; corners keep their size")
        .changed()
    {
        editor
            .update_slice_key(|k| k.center = nine.then(|| crate::slice_ops::default_center(w, h)));
    }
    if let Some((mut cx, mut cy, mut cw, mut ch)) = key.center {
        let fields = ui
            .horizontal(|ui| {
                ui.label("X");
                let rx = widgets::number(ui, &mut cx, 0..=(w as i32 - 1).max(0), "");
                ui.label("Y");
                let ry = widgets::number(ui, &mut cy, 0..=(h as i32 - 1).max(0), "");
                [rx, ry]
            })
            .inner;
        let sizes = ui
            .horizontal(|ui| {
                ui.label("W");
                let rw = widgets::number(ui, &mut cw, 1..=w.max(1), "");
                ui.label("H");
                let rh = widgets::number(ui, &mut ch, 1..=h.max(1), "");
                [rw, rh]
            })
            .inner;
        let all = [&fields[0], &fields[1], &sizes[0], &sizes[1]];
        if let Some(group) = gesture_group(editor, &all) {
            // El centro no puede salirse del slice.
            let cw = cw.min(w.saturating_sub(cx as u32)).max(1);
            let ch = ch.min(h.saturating_sub(cy as u32)).max(1);
            editor.update_slice_key_grouped(Some(group), |k| k.center = Some((cx, cy, cw, ch)));
        }
        gesture_end(editor, &all);
    }
    if own_key
        && slice.keys.first().is_some_and(|k| k.frame != frame)
        && ui.button("Remove key on this frame").clicked()
    {
        editor.remove_slice_key();
    }
    let mut hide = false;
    if ui
        .small_button("Hide from this frame")
        .on_hover_text("Adds an empty key: the slice disappears from here on")
        .clicked()
    {
        hide = true;
    }
    if hide {
        editor.update_slice(|s| {
            s.set_key(cs_core::SliceKey {
                frame,
                bounds: (x, y, 0, 0),
                center: None,
                pivot: None,
            })
        });
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Sprite};
    use eframe::egui::{Event, PointerButton, Pos2, RawInput, pos2};

    use super::*;

    #[test]
    fn tile_thumbnails_are_cached_until_a_tile_image_changes() {
        let ctx = egui::Context::default();
        let mut e = crate::tile_ops::test_util::tile_editor();
        let mut thumbs = TileThumbs::default();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures.len(), 3, "the empty tile and tiles 1 and 2");
        let first = thumbs.textures[1].id();
        thumbs.refresh(&ctx, &e);
        assert_eq!(thumbs.textures[1].id(), first, "cached");
        let cmd = cs_commands::ops::set_tiles(
            e.sprite(),
            e.layer,
            0,
            &[(0, 0, cs_core::Tile::new(1, false, false, false))],
        )
        .unwrap();
        e.execute(cmd);
        thumbs.refresh(&ctx, &e);
        assert_eq!(
            thumbs.textures[1].id(),
            first,
            "a set_tiles stroke keeps the thumbnails"
        );
        let mut img = cs_core::Image::new(4, 4, ColorMode::Rgba);
        img.set(1, 1, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(9, 9, 9)))
            .unwrap();
        let cmd = cs_commands::ops::set_tile_image(e.sprite(), 0, 1, img).unwrap();
        e.execute(cmd);
        thumbs.refresh(&ctx, &e);
        assert_ne!(
            thumbs.textures[1].id(),
            first,
            "rebuilt after replacing a tile image"
        );
        e.select_layer(cs_core::LayerId(1));
        thumbs.refresh(&ctx, &e);
        assert!(thumbs.textures.is_empty(), "no tileset on an image layer");
    }

    /// Corre un frame con un slider de opacidad conectado como en el panel de instancia;
    /// devuelve el rect del slider.
    fn frame(ctx: &egui::Context, editor: &mut Editor, events: Vec<Event>) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
            events,
            ..Default::default()
        };
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                let mut opacity = editor.instance_key().unwrap().opacity;
                let r = ui.add(egui::Slider::new(&mut opacity, 0..=255));
                if let Some(group) = gesture_group(editor, &[&r]) {
                    editor.update_instance_key_grouped(Some(group), |k| k.opacity = opacity);
                }
                gesture_end(editor, &[&r]);
                rect = r.rect;
            });
        });
        output.textures_delta.clear();
        rect
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn slider_click_then_drag_is_one_undo_step() {
        let mut e = Editor::with_sprite(Sprite::new(16, 16, ColorMode::Rgba));
        e.add_artboard(4, 4);
        e.activate(cs_core::ArtboardId(1));
        e.add_instance_layer(cs_core::ArtboardId(2));
        let original = e.instance_key().unwrap();
        let ctx = egui::Context::default();
        let r = frame(&ctx, &mut e, vec![]);
        let y = r.center().y;
        let at = |t: f32| pos2(r.left() + r.width() * t, y);
        // Clic en el 20 % (el valor salta), arrastre hasta el 30 % y suelta.
        frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.2))]);
        frame(&ctx, &mut e, vec![button(at(0.2), true)]);
        for t in [0.22, 0.25, 0.3] {
            frame(&ctx, &mut e, vec![Event::PointerMoved(at(t))]);
        }
        frame(&ctx, &mut e, vec![button(at(0.3), false)]);
        frame(&ctx, &mut e, vec![]);
        let dragged = e.instance_key().unwrap().opacity;
        assert_ne!(dragged, original.opacity, "the slider moved");
        // Un segundo clic es otro paso.
        frame(&ctx, &mut e, vec![Event::PointerMoved(at(0.6))]);
        frame(&ctx, &mut e, vec![button(at(0.6), true)]);
        frame(&ctx, &mut e, vec![button(at(0.6), false)]);
        frame(&ctx, &mut e, vec![]);
        assert_ne!(e.instance_key().unwrap().opacity, dragged);
        e.undo();
        assert_eq!(e.instance_key().unwrap().opacity, dragged);
        e.undo();
        assert_eq!(e.instance_key().unwrap(), original);
    }

    /// Corre un frame con el grupo `group` de la barra en el instante `time`; devuelve el rect
    /// de su botón.
    fn group_frame(
        ctx: &egui::Context,
        editor: &mut Editor,
        group: usize,
        time: f64,
        events: Vec<Event>,
    ) -> Rect {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut rect = Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                rect = tool_group(ui, editor, group, ToolKind::GROUPS[group]).rect;
            });
        });
        output.textures_delta.clear();
        rect
    }

    fn mouse(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        }
    }

    fn escape() -> Event {
        Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn right_click_or_holding_a_group_button_opens_its_variants() {
        use PointerButton::{Primary, Secondary};
        use egui::Popup;
        let ctx = egui::Context::default();
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let bucket = ToolKind::GROUPS
            .iter()
            .position(|g| g.contains(&ToolKind::Bucket))
            .unwrap();
        let c = group_frame(&ctx, &mut e, bucket, 0.0, vec![]).center();
        group_frame(&ctx, &mut e, bucket, 0.05, vec![Event::PointerMoved(c)]);

        // Clic derecho: abre la lista y no cambia de herramienta.
        group_frame(&ctx, &mut e, bucket, 0.1, vec![mouse(c, Secondary, true)]);
        group_frame(&ctx, &mut e, bucket, 0.15, vec![mouse(c, Secondary, false)]);
        group_frame(&ctx, &mut e, bucket, 0.2, vec![]);
        assert!(Popup::is_any_open(&ctx), "right-click opens the list");
        assert_eq!(e.tool, ToolKind::Pencil);
        group_frame(&ctx, &mut e, bucket, 0.25, vec![escape()]);
        group_frame(&ctx, &mut e, bucket, 0.3, vec![]);
        assert!(!Popup::is_any_open(&ctx));

        // Mantener presionado: a los 400 ms se abre; al soltar sigue abierta y no elige.
        group_frame(&ctx, &mut e, bucket, 1.0, vec![mouse(c, Primary, true)]);
        group_frame(&ctx, &mut e, bucket, 1.2, vec![]);
        assert!(!Popup::is_any_open(&ctx), "not yet");
        group_frame(&ctx, &mut e, bucket, 1.45, vec![]);
        group_frame(&ctx, &mut e, bucket, 1.5, vec![]);
        assert!(Popup::is_any_open(&ctx), "held for 400 ms");
        group_frame(&ctx, &mut e, bucket, 1.55, vec![mouse(c, Primary, false)]);
        group_frame(&ctx, &mut e, bucket, 1.6, vec![]);
        assert!(Popup::is_any_open(&ctx), "releasing keeps it open");
        assert_eq!(e.tool, ToolKind::Pencil, "and does not pick the tool");
        group_frame(&ctx, &mut e, bucket, 1.65, vec![escape()]);
        group_frame(&ctx, &mut e, bucket, 1.7, vec![]);
        assert!(!Popup::is_any_open(&ctx));

        // Un clic corto elige la herramienta del grupo.
        group_frame(&ctx, &mut e, bucket, 2.0, vec![mouse(c, Primary, true)]);
        group_frame(&ctx, &mut e, bucket, 2.1, vec![mouse(c, Primary, false)]);
        group_frame(&ctx, &mut e, bucket, 2.15, vec![]);
        assert_eq!(e.tool, ToolKind::Bucket);
        assert!(!Popup::is_any_open(&ctx));
    }

    #[test]
    fn the_shape_buttons_pick_round_or_square() {
        use cs_tools::BrushShape;
        let ctx = egui::Context::default();
        let run = |shape: &mut BrushShape, events: Vec<Event>| -> [Rect; 2] {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 200.0))),
                events,
                ..Default::default()
            };
            let mut rects = [Rect::NOTHING; 2];
            let mut output = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    rects = brush_shape_buttons(ui, shape).map(|r| r.rect);
                });
            });
            output.textures_delta.clear();
            rects
        };
        let click = |shape: &mut BrushShape, p: Pos2| {
            run(shape, vec![Event::PointerMoved(p)]);
            run(shape, vec![mouse(p, PointerButton::Primary, true)]);
            run(shape, vec![mouse(p, PointerButton::Primary, false)]);
        };
        let mut shape = BrushShape::Round;
        let [round, square] = run(&mut shape, vec![]);
        assert!(
            round.width() > 0.0 && square.left() >= round.right(),
            "side by side"
        );
        click(&mut shape, square.center());
        assert_eq!(shape, BrushShape::Square);
        click(&mut shape, round.center());
        assert_eq!(shape, BrushShape::Round);
    }
}
