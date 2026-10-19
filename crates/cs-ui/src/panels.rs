use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

use cs_core::{ArtboardId, SymmetryMode};

use crate::canvas::CanvasView;
use cs_tools::text::FontChoice;
use cs_tools::{Dither, GradientShape};

use crate::editor::{Editor, SelectionMode, ToolKind};
use crate::{theme, widgets};

fn icon(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::RectSelect => "🔲",
        ToolKind::EllipseSelect => "🔘",
        ToolKind::Lasso => "➰",
        ToolKind::PolygonLasso => "🔷",
        ToolKind::MagicWand => "✨",
        ToolKind::Move => "✋",
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

/// Fila de un grupo: la herramienta actual del grupo y, si hay más, un desplegable (también con
/// clic derecho) con las demás, como en Aseprite.
fn tool_group(ui: &mut egui::Ui, editor: &mut Editor, index: usize, group: &[ToolKind]) {
    let memory = ui.id().with(("tool_group", index));
    if group.contains(&editor.tool) {
        ui.data_mut(|d| d.insert_temp(memory, editor.tool));
    }
    let shown = ui
        .data(|d| d.get_temp::<ToolKind>(memory))
        .unwrap_or(group[0]);
    let mut pick = None;
    ui.horizontal(|ui| {
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
        if response.clicked() {
            pick = Some(shown);
        }
        if more {
            let current = editor.tool;
            response.context_menu(|ui| group_menu(ui, current, group, &mut pick));
            ui.menu_button("⏷", |ui| group_menu(ui, current, group, &mut pick))
                .response
                .on_hover_text("More tools (or right-click)");
        }
    });
    if let Some(kind) = pick {
        editor.set_tool(kind);
    }
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
        ui.label("Brush size");
        ui.add(egui::Slider::new(&mut options.brush_size, 1..=32));
    }
    if matches!(tool, ToolKind::Pencil | ToolKind::Eraser) {
        ui.checkbox(&mut options.pixel_perfect, "Pixel-perfect");
    }
    if matches!(tool, ToolKind::Rectangle | ToolKind::Ellipse) {
        ui.checkbox(&mut options.filled, "Filled");
    }
    if tool.is_selection() {
        ui.label("Mode");
        for mode in SelectionMode::ALL {
            ui.radio_value(&mut options.selection_mode, mode, mode.label());
        }
    }
    match tool {
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
        ToolKind::Text => {
            text_style_options(ui, editor);
            ui.weak("Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            ui.weak("Drag to create a slice (hitbox, pivot…). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.");
        }
        _ => {}
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

fn swatch(ui: &mut egui::Ui, color: Color32, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    if color.a() < 255 {
        // Damero mínimo para que se note la transparencia.
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
    painter.rect_filled(rect, 0.0, color);
    response
}

fn to_color32(c: cs_core::Rgba) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

pub fn palette(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.add_space(6.0);
    ui.heading("Color");
    ui.separator();
    ui.horizontal(|ui| {
        let primary = to_color32(editor.resolve(editor.primary));
        let secondary = to_color32(editor.resolve(editor.secondary));
        swatch(ui, primary, 36.0).on_hover_text("Primary (left click)");
        swatch(ui, secondary, 24.0).on_hover_text("Secondary (right click)");
        if ui.small_button("🔄").on_hover_text("Swap (X)").clicked() {
            editor.swap_colors();
        }
    });
    ui.add_space(8.0);
    ui.label("Palette");
    let size = 20.0;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        let count = editor.sprite().palette.len().min(256);
        for i in 0..count {
            let index = i as u8;
            let value = editor.palette_value(index);
            let color = to_color32(editor.sprite().palette.colors()[i]);
            let response = swatch(ui, color, size).on_hover_text(format!("#{i}"));
            if value == editor.primary || value == editor.secondary {
                let stroke_color = if value == editor.primary {
                    Color32::WHITE
                } else {
                    theme::ACCENT
                };
                ui.painter().rect_stroke(
                    response.rect,
                    0.0,
                    Stroke::new(2.0, stroke_color),
                    StrokeKind::Inside,
                );
            }
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
}

/// Selector de color libre (RGBA): edita el color primario.
fn color_picker(ui: &mut egui::Ui, editor: &mut Editor) {
    let cs_core::PixelValue::Rgba(current) = editor.primary else {
        ui.weak("Indexed sprite: colors come from the palette.");
        return;
    };
    let mut c32 = to_color32(current);
    // El cuadro de saturación/brillo ocupa todo el ancho del panel. Se restaura después: si no,
    // los sliders de los paneles siguientes heredan ese ancho y agrandan el panel en cada frame.
    let slider_width = ui.spacing().slider_width;
    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(120.0);
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
fn gesture_group(editor: &mut Editor, responses: &[&egui::Response]) -> Option<u64> {
    if responses
        .iter()
        .any(|r| r.drag_started() || r.gained_focus())
    {
        editor.end_group();
    }
    responses.iter().find(|r| r.changed()).map(|r| r.id.value())
}

/// Llamar después de aplicar: cierra el gesto si terminó (o si el cambio no era parte de uno).
fn gesture_end(editor: &mut Editor, responses: &[&egui::Response]) {
    let ended = responses.iter().any(|r| {
        r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged() && !r.has_focus())
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
    let Some(slice) = editor.selected_slice().cloned() else {
        return;
    };
    ui.separator();
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
