use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Vec2};

use cs_core::ArtboardId;

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
}

pub fn status_bar(ui: &mut egui::Ui, editor: &Editor, canvas: &CanvasView) {
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
    });
}

/// Lista de artboards: activar, crear, renombrar y eliminar.
pub fn artboards(ui: &mut egui::Ui, editor: &mut Editor, name_buffer: &mut (ArtboardId, String)) {
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
}
