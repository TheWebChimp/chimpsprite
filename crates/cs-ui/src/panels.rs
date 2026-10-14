use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

use cs_core::{ArtboardId, SymmetryMode};

use crate::canvas::CanvasView;
use crate::editor::{Editor, ToolKind};
use crate::theme;

fn icon(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Select => "🔲",
        ToolKind::Move => "✋",
        ToolKind::Pencil => "✏",
        ToolKind::Eraser => "🗑",
        ToolKind::Bucket => "🍶",
        ToolKind::Line => "📏",
        ToolKind::Rectangle => "⬜",
        ToolKind::Ellipse => "⭕",
        ToolKind::Eyedropper => "💧",
        ToolKind::Artboard => "🖼",
    }
}

pub fn toolbar(ui: &mut egui::Ui, editor: &mut Editor) {
    ui.add_space(6.0);
    ui.heading("Tools");
    ui.separator();
    for kind in ToolKind::ALL {
        let selected = editor.tool == kind;
        let label = format!("{} {}", icon(kind), kind.label());
        let button = egui::Button::selectable(selected, label).shortcut_text(kind.shortcut());
        if ui.add_sized([ui.available_width(), 24.0], button).clicked() {
            editor.tool = kind;
        }
    }

    ui.add_space(10.0);
    ui.heading("Options");
    ui.separator();
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
    if tool == ToolKind::Bucket {
        ui.checkbox(&mut options.contiguous, "Contiguous");
    }
    if matches!(
        tool,
        ToolKind::Line | ToolKind::Rectangle | ToolKind::Ellipse | ToolKind::Select
    ) {
        ui.weak("Shift: constrain");
    }
    if tool.paints() {
        ui.weak("Alt+click: pick color");
    }

    ui.add_space(10.0);
    ui.heading("Symmetry");
    ui.separator();
    symmetry_buttons(ui, editor);
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

fn hex_of(c: cs_core::Rgba) -> String {
    if c.a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

/// `#rgb`, `#rrggbb` o `#rrggbbaa` (el `#` es opcional).
fn parse_hex(text: &str) -> Option<cs_core::Rgba> {
    let t = text.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(t.get(i..i + 2)?, 16).ok();
    match t.len() {
        3 => {
            let d = |i: usize| {
                u8::from_str_radix(t.get(i..i + 1)?, 16)
                    .ok()
                    .map(|v| v * 17)
            };
            Some(cs_core::Rgba::rgb(d(0)?, d(1)?, d(2)?))
        }
        6 => Some(cs_core::Rgba::rgb(byte(0)?, byte(2)?, byte(4)?)),
        8 => Some(cs_core::Rgba::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => None,
    }
}

/// Selector de color libre (RGBA): edita el color primario.
fn color_picker(ui: &mut egui::Ui, editor: &mut Editor) {
    let cs_core::PixelValue::Rgba(current) = editor.primary else {
        ui.weak("Indexed sprite: colors come from the palette.");
        return;
    };
    let mut c32 = to_color32(current);
    // El cuadro de saturación/brillo ocupa todo el ancho del panel.
    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(120.0);
    if egui::color_picker::color_picker_color32(ui, &mut c32, egui::color_picker::Alpha::OnlyBlend)
    {
        let [r, g, b, a] = c32.to_srgba_unmultiplied();
        editor.primary = cs_core::PixelValue::Rgba(cs_core::Rgba::new(r, g, b, a));
    }
    ui.horizontal(|ui| {
        let id = ui.id().with("hex");
        let mut text = ui
            .data_mut(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| hex_of(current));
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
            && let Some(c) = parse_hex(&text)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip_and_short_forms() {
        let c = cs_core::Rgba::new(0x90, 0x52, 0xbc, 255);
        assert_eq!(hex_of(c), "#9052bc");
        assert_eq!(parse_hex("#9052bc"), Some(c));
        assert_eq!(parse_hex("9052BC"), Some(c));
        assert_eq!(parse_hex("#f0a"), Some(cs_core::Rgba::rgb(255, 0, 170)));
        assert_eq!(
            parse_hex("#11223380"),
            Some(cs_core::Rgba::new(0x11, 0x22, 0x33, 0x80))
        );
        assert_eq!(hex_of(cs_core::Rgba::new(1, 2, 3, 4)), "#01020304");
        assert_eq!(parse_hex("#12"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
    }
}
