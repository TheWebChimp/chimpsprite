//! Edit → Keyboard Shortcuts…: la lista de los atajos de la app, por sección y con un buscador.
//! Las herramientas salen de [`ToolKind`] (su nombre y su atajo), así no se desfasan.

use eframe::egui::{self, Align2, RichText, Ui, vec2};

use crate::editor::ToolKind;
use crate::skin::Skin;

/// Los atajos que no son de herramientas: sección, acción y teclas.
const SHORTCUTS: &[(&str, &str, &str)] = &[
    ("File", "New", "Ctrl+N"),
    ("File", "Open", "Ctrl+O"),
    ("File", "Save", "Ctrl+S"),
    ("File", "Save As", "Ctrl+Shift+S"),
    ("File", "Export", "Ctrl+E"),
    ("File", "Close Tab", "Ctrl+W"),
    ("File", "Next Tab", "Ctrl+Tab"),
    ("File", "Previous Tab", "Ctrl+Shift+Tab"),
    ("Edit", "Undo", "Ctrl+Z"),
    ("Edit", "Redo", "Ctrl+Y"),
    ("Edit", "Redo", "Ctrl+Shift+Z"),
    ("Edit", "Cut", "Ctrl+X"),
    ("Edit", "Copy", "Ctrl+C"),
    ("Edit", "Copy Merged", "Ctrl+Shift+C"),
    ("Edit", "Paste", "Ctrl+V"),
    ("Edit", "Paste as New Artboard", "Ctrl+Shift+V"),
    ("Edit", "Delete", "Del"),
    ("Edit", "Fill", "F"),
    ("Edit", "Stroke", "S"),
    ("Edit", "Flip Horizontal", "Shift+H"),
    ("Edit", "Flip Vertical", "Shift+V"),
    ("Edit", "Transform", "Ctrl+T"),
    ("Edit", "New Brush", "Ctrl+B"),
    ("Edit", "New Artboard From Selection", "Ctrl+Alt+N"),
    ("Edit", "Invert", "Ctrl+I"),
    ("Select", "All", "Ctrl+A"),
    ("Select", "Deselect", "Ctrl+D"),
    ("Select", "Reselect", "Ctrl+Shift+D"),
    ("Select", "Inverse", "Ctrl+Shift+I"),
    ("Select", "Move the selection 1 px", "Arrows"),
    ("Sprite", "Properties", "Ctrl+P"),
    ("Sprite", "Sprite Size", "Ctrl+Alt+I"),
    ("Sprite", "Canvas Size", "C"),
    ("Layer", "New Layer", "Shift+N"),
    ("Layer", "Clipping Mask", "Ctrl+Alt+G"),
    ("Layer", "Layer Above", "Up"),
    ("Layer", "Layer Below", "Down"),
    ("Frame", "Play / Pause", "Enter"),
    ("Frame", "Next Frame", "."),
    ("Frame", "Previous Frame", ","),
    ("Frame", "Duplicate Frame", "Alt+N"),
    ("Frame", "Onion Skin", "F3"),
    ("View", "Extras", "Ctrl+H"),
    ("View", "Timeline", "Tab"),
    ("View", "Full Screen", "F11"),
    ("View", "Zoom In", "+"),
    ("View", "Zoom Out", "-"),
    ("View", "Actual Size", "Ctrl+0"),
    ("View", "Fit to Screen", "Ctrl+Shift+0"),
    ("Canvas", "Swap Colors", "X"),
    ("Canvas", "Cancel", "Esc"),
    ("Canvas", "Brush Size", "Ctrl+Wheel"),
    ("Canvas", "Pick Color", "Alt+Click"),
    ("Canvas", "Pan", "Space+Drag"),
];

const WIDTH: f32 = 280.0;
const LIST_HEIGHT: f32 = 320.0;

/// Todas las filas: (sección, acción, teclas), las herramientas primero.
fn rows() -> Vec<(&'static str, &'static str, &'static str)> {
    let tools = ToolKind::GROUPS
        .iter()
        .flat_map(|g| g.iter())
        .filter(|t| !t.shortcut().is_empty())
        .map(|t| ("Tools", t.label(), t.shortcut()));
    tools.chain(SHORTCUTS.iter().copied()).collect()
}

/// Las filas que coinciden con `filter` (en la acción, las teclas o la sección).
fn matching(filter: &str) -> Vec<(&'static str, &'static str, &'static str)> {
    let filter = filter.trim().to_lowercase();
    rows()
        .into_iter()
        .filter(|(section, action, keys)| {
            filter.is_empty()
                || [section, action, keys]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&filter))
        })
        .collect()
}

/// La ventana; `filter` es lo escrito en el buscador. Devuelve si se cerró.
pub fn show(ctx: &egui::Context, filter: &mut String, skin: Option<&Skin>) -> bool {
    let anchor = (Align2::CENTER_CENTER, egui::Vec2::ZERO);
    let (_, closed) = crate::pixel_ui::app_window(
        ctx,
        skin,
        "keyboard_shortcuts",
        "Keyboard Shortcuts",
        true,
        anchor,
        |ui| contents(ui, filter, skin),
    );
    closed || ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

fn contents(ui: &mut Ui, filter: &mut String, skin: Option<&Skin>) {
    ui.set_width(WIDTH);
    ui.horizontal(|ui| {
        ui.label("Search");
        crate::widgets::text_line(ui, filter);
    });
    ui.add_space(4.0);
    let dim = skin.map_or(ui.visuals().weak_text_color(), |s| s.colors().text_dim);
    let rows = matching(filter);
    crate::pixel_ui::scroll(
        ui,
        egui::ScrollArea::vertical()
            .max_height(LIST_HEIGHT)
            .auto_shrink([false, true]),
        |ui| {
            let mut section = "";
            egui::Grid::new("shortcuts")
                .num_columns(2)
                .spacing(vec2(12.0, 2.0))
                .show(ui, |ui| {
                    for (s, action, keys) in rows {
                        if s != section {
                            section = s;
                            ui.label(RichText::new(s).color(dim));
                            ui.end_row();
                        }
                        ui.label(action);
                        ui.label(keys);
                        ui.end_row();
                    }
                });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_with_a_shortcut_is_listed_and_search_filters() {
        let all = rows();
        assert!(all.contains(&("Tools", "Pencil", "B")));
        assert!(all.contains(&("Tools", "Polygon", "Shift+D")));
        assert!(all.contains(&("Edit", "Stroke", "S")));
        let found = matching("stroke");
        assert_eq!(found, vec![("Edit", "Stroke", "S")]);
        // También por las teclas.
        assert!(
            matching("ctrl+alt+n")
                .iter()
                .any(|r| r.1 == "New Artboard From Selection")
        );
        assert_eq!(matching("").len(), all.len());
    }
}
