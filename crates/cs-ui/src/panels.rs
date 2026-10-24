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
use crate::skin::Skin;
use crate::{pixel_ui, theme, tile_ops, tool_group, widgets};

/// Fila de un grupo en la barra con nombres: el botón con la herramienta que muestra el grupo y,
/// si hay más, un desplegable con las demás (también con clic derecho o manteniendo presionado),
/// como en Aseprite. Devuelve la respuesta del botón.
fn tool_group(ui: &mut egui::Ui, editor: &mut Editor, index: usize) -> egui::Response {
    let group = ToolKind::GROUPS[index];
    let more = group.len() > 1;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let width = ui.available_width() - if more { 22.0 } else { 0.0 };
        let response = tool_group::group_button(ui, editor, None, index, |ui, shown, state| {
            let label = format!("{} {}", tool_group::emoji(shown), shown.label());
            let button = egui::Button::selectable(state.selected, label)
                .shortcut_text("")
                .truncate();
            ui.add_sized([width, 24.0], button)
        });
        if more {
            let current = editor.tool;
            let mut pick = None;
            ui.menu_button("⏷", |ui| {
                pick = tool_group::variants_list(ui, None, current, group);
            })
            .response
            .on_hover_text("More tools (or right-click, or hold)");
            if let Some(kind) = pick {
                editor.set_tool(kind);
            }
        }
        response
    })
    .inner
}

/// Scroll del panel de herramientas: solo con la rueda y la barra. Arrastrar (también con el
/// dedo) no lo mueve, así mantener presionado un botón de grupo abre sus variantes.
pub fn tools_scroll_area() -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::vertical()
        .id_salt("tools_scroll")
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
}

/// La barra de íconos de Pixel UI: una columna con un botón por grupo, alineada a la izquierda.
/// Los marcos van pegados, uno debajo del otro. Cada botón lleva un id propio de su grupo, así su
/// lista de variantes (y la presión larga) no depende de lo que haya antes en la barra.
fn tool_column(ui: &mut egui::Ui, editor: &mut Editor, skin: &Skin) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for index in 0..ToolKind::GROUPS.len() {
            let id = ui.id().with(("tool_group", index));
            tool_group::group_button(ui, editor, Some(skin), index, |ui, shown, state| {
                pixel_ui::tool_button(
                    ui,
                    id,
                    skin,
                    tool_group::icon_piece(shown),
                    shown.label(),
                    state.selected,
                    state.more,
                )
            });
        }
    });
}

/// La barra de herramientas: solo los botones de grupo. Con `skin` (Pixel UI) es una columna de
/// íconos; sin él, filas con nombre bajo el título "Tools". Las opciones de la herramienta y
/// Symmetry van en la barra de opciones (`tool_options_bar`).
pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor, skin: Option<&Skin>) {
    match skin {
        Some(skin) => tool_column(ui, editor, skin),
        None => {
            ui.add_space(6.0);
            ui.heading("Tools");
            ui.separator();
            ui.spacing_mut().item_spacing.y = 3.0;
            for index in 0..ToolKind::GROUPS.len() {
                tool_group(ui, editor, index);
            }
        }
    }
}

/// Alto de la barra de opciones, con su margen: el mismo con todas las herramientas, así el lienzo
/// no se mueve al cambiar de herramienta.
pub const OPTIONS_BAR_HEIGHT: f32 = 32.0;

/// Barra de opciones, debajo de las pestañas: las opciones de la herramienta activa en una sola
/// fila y Symmetry a la derecha. Si las opciones no caben, hacen scroll horizontal con la rueda o
/// la barra (sin arrastre); no se parten en dos renglones.
pub fn tool_options_bar(ui: &mut egui::Ui, editor: &mut Editor) {
    // Los textos no se parten: la fila se alarga y hace scroll.
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
    // La rueda de siempre (vertical) mueve la fila de lado, sin Shift.
    ui.style_mut().always_scroll_the_only_direction = true;
    // De derecha a izquierda: Symmetry queda pegado al borde y las opciones usan lo que sobra.
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        symmetry_buttons(ui, editor);
        ui.separator();
        options_scroll_area().show(ui, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                tool_options(ui, editor)
            });
        });
    });
    text_window(ui.ctx(), editor);
}

/// Scroll de las opciones de la barra: horizontal, solo con la rueda y la barra.
fn options_scroll_area() -> egui::ScrollArea {
    use egui::containers::scroll_area::ScrollSource;
    egui::ScrollArea::horizontal()
        .id_salt("tool_options_scroll")
        .auto_shrink([false, false])
        .scroll_source(ScrollSource::SCROLL_BAR | ScrollSource::MOUSE_WHEEL)
}

/// Separadores entre los grupos de la barra de opciones: una línea antes de cada grupo, menos el
/// primero.
#[derive(Default)]
struct Groups {
    started: bool,
}

impl Groups {
    fn next(&mut self, ui: &mut egui::Ui) {
        if self.started {
            ui.separator();
        }
        self.started = true;
    }
}

/// Las opciones de la herramienta activa, en fila: cada etiqueta al lado de su control y un
/// separador entre grupos. La ayuda de la herramienta va en el mismo orden que antes.
fn tool_options(ui: &mut egui::Ui, editor: &mut Editor) {
    let tool = editor.tool;
    let mut groups = Groups::default();
    let options = &mut editor.options;
    if tool.uses_brush() {
        groups.next(ui);
        ui.label("Brush size");
        ui.add(egui::Slider::new(
            &mut options.brush_size,
            1..=MAX_BRUSH_SIZE,
        ))
        .on_hover_text("Ctrl+wheel over the canvas changes it");
        brush_shape_buttons(ui, &mut options.brush_shape);
    }
    if matches!(tool, ToolKind::Pencil | ToolKind::Eraser) {
        groups.next(ui);
        ui.checkbox(&mut options.pixel_perfect, "Pixel-perfect");
    }
    if matches!(tool, ToolKind::Rectangle | ToolKind::Ellipse) {
        groups.next(ui);
        ui.checkbox(&mut options.filled, "Filled");
    }
    if tool == ToolKind::Rectangle {
        groups.next(ui);
        ui.label("Corner radius");
        ui.add(egui::Slider::new(&mut options.corner_radius, 0..=32))
            .on_hover_text("0 = square corners; large values make a pill or a circle");
    }
    if tool.is_selection() {
        groups.next(ui);
        ui.label("Mode");
        for mode in SelectionMode::ALL {
            ui.radio_value(&mut options.selection_mode, mode, mode.label());
        }
    }
    match tool {
        ToolKind::Hand => {
            groups.next(ui);
            ui.weak("Drag to move the view. Space+drag or the middle button do it with any tool.");
        }
        ToolKind::MagicWand => {
            groups.next(ui);
            ui.label("Tolerance");
            ui.add(egui::Slider::new(&mut options.tolerance, 0..=255));
            ui.checkbox(&mut options.contiguous, "Contiguous");
            ui.label("Sample:");
            ui.radio_value(&mut options.sample_all, false, "Layer");
            ui.radio_value(&mut options.sample_all, true, "All layers");
        }
        ToolKind::Lasso => {
            groups.next(ui);
            ui.weak("Drag around the area; it closes when you release.");
        }
        ToolKind::PolygonLasso => {
            groups.next(ui);
            ui.weak("Click to add points. Click the first point, double-click or press Enter to close; Esc cancels.");
        }
        ToolKind::Bucket => {
            groups.next(ui);
            ui.checkbox(&mut options.contiguous, "Contiguous");
        }
        ToolKind::Spray => {
            groups.next(ui);
            ui.label("Spray width");
            ui.add(egui::Slider::new(&mut options.spray_width, 1..=64));
            ui.label("Spray speed");
            ui.add(egui::Slider::new(&mut options.spray_speed, 1..=100));
        }
        ToolKind::Gradient => {
            groups.next(ui);
            ui.radio_value(&mut options.gradient_shape, GradientShape::Linear, "Linear");
            ui.radio_value(&mut options.gradient_shape, GradientShape::Radial, "Radial");
            egui::ComboBox::from_id_salt("dither")
                .selected_text(options.dither.label())
                .show_ui(ui, |ui| {
                    for d in Dither::ALL {
                        ui.selectable_value(&mut options.dither, d, d.label());
                    }
                });
            groups.next(ui);
            ui.weak("Drag from the primary to the secondary color. Right-drag swaps them.");
        }
        ToolKind::Jumble => {
            groups.next(ui);
            ui.weak("Scrambles pixels within the brush radius.");
        }
        ToolKind::Contour => {
            groups.next(ui);
            ui.weak("Draw the outline: it closes and fills when you release.");
        }
        ToolKind::Blur => {
            groups.next(ui);
            ui.weak("Softens the pixels under the brush. Go over an area again to blur it more.");
        }
        ToolKind::Text => {
            groups.next(ui);
            text_style_options(ui, editor);
            groups.next(ui);
            ui.weak("Click on the canvas to place text.");
        }
        ToolKind::Slice => {
            groups.next(ui);
            ui.weak("Drag to create a slice (hitbox, pivot…). Drag the selected one to move it or its edges to resize it: changes apply from the current frame on.");
        }
        _ => {}
    }
    if tool.is_selection() || tool == ToolKind::Move {
        groups.next(ui);
        let before = editor.options.rotation;
        ui.label("Rotation:");
        for algorithm in RotationAlgorithm::ALL {
            ui.radio_value(&mut editor.options.rotation, algorithm, algorithm.label());
        }
        if editor.options.rotation != before {
            editor.refresh_floating(false);
        }
        groups.next(ui);
        ui.weak("Drag the handles to scale, or just outside a corner to rotate. Shift keeps the proportions or snaps to 15°.");
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Gradient
    ) || matches!(tool, ToolKind::RectSelect | ToolKind::EllipseSelect)
    {
        groups.next(ui);
        ui.weak("Shift: constrain");
    }
    if tool.paints() {
        groups.next(ui);
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

/// Ayuda de los ejes, en el tooltip de los botones de simetría cuando hay una activa.
const AXES_HINT: &str = "Ctrl+drag the dashed axes to move them";

/// Symmetry, al final de la barra de opciones. Se arma de derecha a izquierda (la barra lo pone
/// pegado al borde): "Center axes" si los ejes se movieron, los siete modos en orden inverso y el
/// título. En pantalla se leen en el orden de siempre.
fn symmetry_buttons(ui: &mut egui::Ui, editor: &mut Editor) {
    let current = editor.symmetry();
    if current.mode != SymmetryMode::None
        && current.center2.is_some()
        && ui.small_button("Center axes").clicked()
    {
        editor.set_symmetry(cs_core::Symmetry {
            center2: None,
            ..current
        });
    }
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (mode, tip) in SYMMETRY_MODES.into_iter().rev() {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
            let selected = current.mode == mode;
            let enabled = ui.is_enabled();
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    enabled,
                    selected,
                    tip,
                )
            });
            let visuals = ui.style().interact_selectable(&response, selected);
            let painter = ui.painter();
            painter.rect_filled(rect, 3.0, visuals.weak_bg_fill);
            let color = if selected {
                theme::ACCENT
            } else {
                visuals.fg_stroke.color
            };
            draw_symmetry_icon(painter, rect.shrink(5.0), mode, color);
            let response = if current.mode == SymmetryMode::None {
                response.on_hover_text(tip)
            } else {
                response.on_hover_text(format!("{tip}\n{AXES_HINT}"))
            };
            if response.clicked() {
                editor.set_symmetry(cs_core::Symmetry { mode, ..current });
            }
        }
    });
    ui.label("Symmetry");
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
        if let Some(text) = editor.mask_status() {
            ui.separator();
            ui.colored_label(theme::ACCENT, text);
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
                rect = tool_group(ui, editor, group).rect;
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

    /// La columna de íconos con el skin embebido dentro del scroll del panel, manejada con
    /// eventos crudos (el reloj avanza 50 ms por frame). Guarda los botones del árbol de
    /// accesibilidad (nombre y rect), las texturas dibujadas y el desplazamiento del scroll.
    struct ColumnHarness {
        ctx: egui::Context,
        editor: Editor,
        skin: Skin,
        time: f64,
        /// Alto de la pantalla: con 150 la columna (13 × 23) no entra y hay scroll.
        height: f32,
        buttons: Vec<(String, Rect)>,
        textures: Vec<egui::TextureId>,
        offset: f32,
    }

    impl ColumnHarness {
        fn new(height: f32) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                skin,
                time: 0.0,
                height,
                buttons: Vec::new(),
                textures: Vec::new(),
                offset: 0.0,
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.frame_after(0.05, events);
        }

        fn frame_after(&mut self, dt: f64, events: Vec<Event>) {
            self.time += dt;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(200.0, self.height),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let (editor, skin) = (&mut self.editor, &self.skin);
            let mut offset = 0.0;
            let mut output = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    offset = tools_scroll_area()
                        .show(ui, |ui| tool_column(ui, editor, skin))
                        .state
                        .offset
                        .y;
                });
            });
            output.textures_delta.clear();
            self.offset = offset;
            self.textures.clear();
            for clipped in &output.shapes {
                collect_textures(&clipped.shape, &mut self.textures);
            }
            self.buttons = output
                .platform_output
                .accesskit_update
                .map(|update| {
                    update
                        .nodes
                        .iter()
                        .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
                        .filter_map(|(_, n)| {
                            let b = n.bounds()?;
                            let rect = Rect::from_min_max(
                                pos2(b.x0 as f32, b.y0 as f32),
                                pos2(b.x1 as f32, b.y1 as f32),
                            );
                            Some((n.label()?.to_string(), rect))
                        })
                        .collect()
                })
                .unwrap_or_default();
        }

        fn at(&self, label: &str) -> Pos2 {
            self.buttons
                .iter()
                .find(|(l, _)| l == label)
                .map(|(_, r)| r.center())
                .unwrap_or_else(|| panic!("no button {label:?} in {:?}", self.buttons))
        }

        fn labels(&self) -> Vec<&str> {
            self.buttons.iter().map(|(l, _)| l.as_str()).collect()
        }

        fn press(&mut self, label: &str, button: PointerButton) -> Pos2 {
            let pos = self.at(label);
            self.frame(vec![Event::PointerMoved(pos)]);
            self.frame(vec![mouse(pos, button, true)]);
            pos
        }

        fn click(&mut self, label: &str, button: PointerButton) {
            let pos = self.press(label, button);
            self.frame(vec![mouse(pos, button, false)]);
            self.frame(vec![]);
        }

        fn texture(&self, piece: &str) -> egui::TextureId {
            self.skin
                .piece(piece, crate::skin::WidgetState::Normal)
                .unwrap()
                .texture
                .id()
        }
    }

    fn collect_textures(shape: &egui::Shape, out: &mut Vec<egui::TextureId>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_textures(s, out)),
            egui::Shape::Mesh(mesh) => out.push(mesh.texture_id),
            _ => {}
        }
    }

    /// Nombre accesible de una fila de la lista de variantes: el nombre y el atajo.
    fn item(kind: ToolKind) -> String {
        format!("{} {}", kind.label(), kind.shortcut())
    }

    #[test]
    fn the_icon_column_has_a_named_button_per_group_and_a_click_picks() {
        let mut h = ColumnHarness::new(600.0);
        let mut column = h.buttons.clone();
        column.sort_by(|a, b| a.1.min.y.total_cmp(&b.1.min.y));
        let names: Vec<&str> = column.iter().map(|(l, _)| l.as_str()).collect();
        let expected: Vec<&str> = ToolKind::GROUPS.iter().map(|g| g[0].label()).collect();
        assert_eq!(names, expected);
        assert!(
            column.iter().all(|(_, r)| r.size() == Vec2::splat(23.0)),
            "{column:?}"
        );
        h.click("Paint Bucket", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Bucket);
    }

    #[test]
    fn right_click_lists_the_five_marquee_tools_and_holding_opens_the_list_too() {
        use egui::Popup;
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        assert!(Popup::is_any_open(&h.ctx));
        for &kind in ToolKind::GROUPS[0] {
            assert!(h.labels().contains(&item(kind).as_str()), "{kind:?}");
        }
        assert_eq!(h.editor.tool, ToolKind::Pencil, "right-click does not pick");
        h.frame(vec![escape()]);
        h.frame(vec![]);
        assert!(!Popup::is_any_open(&h.ctx));

        // Mantener presionado 400 ms abre la lista.
        h.press("Rectangular Marquee", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(Popup::is_any_open(&h.ctx), "held for 400 ms");
    }

    #[test]
    fn the_group_shows_and_picks_the_variant_chosen_in_the_list() {
        let mut h = ColumnHarness::new(600.0);
        h.click("Rectangular Marquee", PointerButton::Secondary);
        h.click(&item(ToolKind::Lasso), PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
        h.click("Pencil", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        assert!(h.textures.contains(&h.texture("icon.lasso")));
        assert!(!h.textures.contains(&h.texture("icon.marquee")));
        // El botón del grupo ahora es Lasso: un clic vuelve a Lasso.
        h.click("Lasso", PointerButton::Primary);
        assert_eq!(h.editor.tool, ToolKind::Lasso);
    }

    /// Ids de los botones de la columna (por nombre), con o sin un widget antes de la columna.
    fn column_ids(prefix: bool) -> Vec<(String, egui::accesskit::NodeId)> {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if prefix {
                    let _ = ui.button("Before");
                }
                tool_column(ui, &mut editor, &skin);
            });
        });
        output.textures_delta.clear();
        let mut ids: Vec<_> = output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .iter()
            .filter(|(_, n)| n.role() == egui::accesskit::Role::Button)
            .filter_map(|(id, n)| Some((n.label()?.to_string(), *id)))
            .filter(|(l, _)| l != "Before")
            .collect();
        ids.sort();
        ids
    }

    #[test]
    fn each_group_button_has_its_own_id_that_does_not_depend_on_what_comes_before() {
        let plain = column_ids(false);
        assert_eq!(plain.len(), ToolKind::GROUPS.len());
        let mut unique: Vec<_> = plain.iter().map(|(_, id)| *id).collect();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), plain.len());
        assert_eq!(plain, column_ids(true));
    }

    #[test]
    fn dragging_over_a_tool_button_does_not_scroll_the_panel() {
        let mut h = ColumnHarness::new(150.0);
        // Con una pantalla táctil, egui arrastra el contenido de un ScrollArea por defecto.
        let touch = |phase| Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(1),
            phase,
            pos: pos2(199.0, 1.0),
            force: None,
        };
        h.frame(vec![
            touch(egui::TouchPhase::Start),
            touch(egui::TouchPhase::End),
        ]);
        assert!(h.ctx.input(|i| i.has_touch_screen()));
        let from = h.press("Pencil", PointerButton::Primary);
        for dy in [10.0, 25.0, 40.0] {
            h.frame(vec![Event::PointerMoved(from - Vec2::new(0.0, dy))]);
        }
        h.frame(vec![mouse(
            from - Vec2::new(0.0, 40.0),
            PointerButton::Primary,
            false,
        )]);
        h.frame(vec![]);
        assert_eq!(h.offset, 0.0, "the panel did not scroll");
    }

    #[test]
    fn holding_a_tool_button_in_the_scrolling_panel_opens_its_variants() {
        let mut h = ColumnHarness::new(150.0);
        h.press("Pencil", PointerButton::Primary);
        h.frame_after(0.45, vec![]);
        h.frame(vec![]);
        assert!(egui::Popup::is_any_open(&h.ctx));
        assert!(h.labels().contains(&item(ToolKind::Spray).as_str()));
        assert_eq!(h.offset, 0.0);
    }

    /// Nombre (o texto, en las etiquetas) y rect de cada nodo del árbol de accesibilidad.
    fn nodes(output: &egui::FullOutput) -> Vec<(String, Rect)> {
        let Some(update) = &output.platform_output.accesskit_update else {
            return Vec::new();
        };
        update
            .nodes
            .iter()
            .filter_map(|(_, n)| {
                let b = n.bounds()?;
                let text = n.label().or_else(|| n.value())?.to_string();
                let rect = Rect::from_min_max(
                    pos2(b.x0 as f32, b.y0 as f32),
                    pos2(b.x1 as f32, b.y1 as f32),
                );
                Some((text, rect))
            })
            .collect()
    }

    /// La barra de opciones en su panel de arriba, como en la app, manejada con eventos crudos (el
    /// reloj avanza 50 ms por frame). Guarda el rect del panel y los nodos de accesibilidad.
    struct BarHarness {
        ctx: egui::Context,
        editor: Editor,
        time: f64,
        width: f32,
        bar: Rect,
        nodes: Vec<(String, Rect)>,
    }

    impl BarHarness {
        fn new(width: f32) -> Self {
            let ctx = egui::Context::default();
            ctx.enable_accesskit();
            let mut h = Self {
                ctx,
                editor: Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba)),
                time: 0.0,
                width,
                bar: Rect::NOTHING,
                nodes: Vec::new(),
            };
            h.frame(vec![]);
            h
        }

        fn frame(&mut self, events: Vec<Event>) {
            self.time += 0.05;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(self.width, 300.0),
                )),
                time: Some(self.time),
                events,
                ..Default::default()
            };
            let editor = &mut self.editor;
            let mut bar = Rect::NOTHING;
            let mut output = self.ctx.run_ui(input, |ui| {
                bar = egui::Panel::top("tool_options")
                    .exact_size(OPTIONS_BAR_HEIGHT)
                    .show(ui, |ui| tool_options_bar(ui, editor))
                    .response
                    .rect;
            });
            output.textures_delta.clear();
            self.bar = bar;
            self.nodes = nodes(&output);
        }

        fn has(&self, text: &str) -> bool {
            self.nodes.iter().any(|(t, _)| t == text)
        }

        fn rect(&self, text: &str) -> Rect {
            self.nodes
                .iter()
                .find(|(t, _)| t == text)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| panic!("no {text:?} in {:?}", self.nodes))
        }
    }

    #[test]
    fn the_options_bar_has_the_brush_with_the_pencil_but_not_with_the_hand() {
        let mut h = BarHarness::new(1200.0);
        assert_eq!(h.editor.tool, ToolKind::Pencil);
        for text in ["Brush size", "Round", "Square", "Pixel-perfect"] {
            assert!(h.has(text), "{text}");
        }
        // Etiqueta y control uno al lado del otro: la forma va a la derecha del texto.
        assert!(h.rect("Round").left() > h.rect("Brush size").right());
        assert!(h.has("Off"), "Symmetry");
        h.editor.set_tool(ToolKind::Hand);
        h.frame(vec![]);
        for text in ["Brush size", "Round", "Square"] {
            assert!(!h.has(text), "{text}");
        }
        assert!(h.has("Off"), "Symmetry is always there");
        assert!(h.has("Symmetry"));
    }

    #[test]
    fn symmetry_is_at_the_right_end_of_the_bar_in_the_usual_order() {
        let h = BarHarness::new(1200.0);
        let names: Vec<&str> = SYMMETRY_MODES.iter().map(|(_, tip)| *tip).collect();
        let rects: Vec<Rect> = names.iter().map(|n| h.rect(n)).collect();
        for pair in rects.windows(2) {
            assert!(pair[0].right() <= pair[1].left(), "{names:?}: {rects:?}");
        }
        let last = rects[rects.len() - 1];
        assert!(
            h.bar.right() - last.right() < 12.0,
            "{last:?} in {:?}",
            h.bar
        );
        assert!(h.rect("Symmetry").right() <= rects[0].left());
    }

    #[test]
    fn the_options_bar_is_one_row_of_the_same_height_with_every_tool() {
        let mut h = BarHarness::new(800.0);
        for &kind in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
            h.editor.set_tool(kind);
            h.frame(vec![]);
            assert_eq!(h.bar.height(), OPTIONS_BAR_HEIGHT, "{kind:?}");
            for (text, rect) in &h.nodes {
                assert!(
                    rect.top() >= h.bar.top() && rect.bottom() <= h.bar.bottom(),
                    "{kind:?}: {text:?} {rect:?} out of {:?}",
                    h.bar
                );
            }
        }
    }

    #[test]
    fn the_options_scroll_sideways_with_the_wheel_when_they_do_not_fit() {
        let mut h = BarHarness::new(320.0);
        let before = h.rect("Round");
        let over = pos2(40.0, h.bar.center().y);
        h.frame(vec![Event::PointerMoved(over)]);
        h.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -60.0),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
        }]);
        for _ in 0..20 {
            h.frame(vec![]);
        }
        let after = h.rect("Round");
        assert!(after.left() < before.left(), "{before:?} -> {after:?}");
        assert_eq!(after.top(), before.top(), "still one row");
        assert!(h.rect("Off").right() <= 320.0, "Symmetry stays in view");
    }

    #[test]
    fn the_toolbar_has_only_the_tool_buttons() {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        let skin = Skin::load(&ctx, crate::skin::EMBEDDED).unwrap();
        let mut editor = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        for skin in [None, Some(&skin)] {
            let mut output = ctx.run_ui(RawInput::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| toolbar(ui, &mut editor, skin));
            });
            output.textures_delta.clear();
            let texts: Vec<String> = nodes(&output).into_iter().map(|(t, _)| t).collect();
            for gone in ["Options", "Symmetry", "Brush size", "Off"] {
                assert!(!texts.iter().any(|t| t == gone), "{gone} in {texts:?}");
            }
            assert!(texts.iter().any(|t| t.contains("Pencil")), "{texts:?}");
        }
    }
}
