//! El diálogo New Variant… del panel FontForge: genera otro estilo de la familia desde la fuente
//! activa (Bold, Italic, Bold Italic, Outline, Shadow o una copia para dibujarla a mano), con su
//! nombre y su peso y una vista previa del texto de prueba con la variante.

use cs_core::{Artboard, ArtboardId, FontMode, FontStyle, VariantKind, VariantRecipe, WEIGHTS};
use eframe::egui;

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::{DialogResult, dialog_window_with, finish};
use crate::widgets;

/// New Variant: de qué fuente sale, la receta y el estilo que tendrá.
pub struct NewVariantDialog {
    /// La fuente de origen (si deja de estar activa, el diálogo no hace nada).
    pub source: ArtboardId,
    from: String,
    mode: FontMode,
    pub recipe: VariantRecipe,
    pub name: String,
    pub weight: u16,
    /// El texto de la vista previa.
    text: String,
    /// La vista previa y la receta con que se hizo.
    preview: Option<(VariantRecipe, Artboard)>,
}

impl NewVariantDialog {
    /// Para la fuente activa, con Bold elegido; `None` si el activo no es una fuente.
    pub fn new(editor: &Editor, text: &str) -> Option<Self> {
        let font = editor.font()?;
        let recipe = VariantRecipe::new(VariantKind::Bold);
        Some(NewVariantDialog {
            source: editor.active,
            from: font.full_name(),
            mode: font.mode,
            recipe,
            name: recipe.kind.style_name().to_owned(),
            weight: recipe.kind.weight(),
            text: if text.trim().is_empty() {
                "AaBbGg 0123".to_owned()
            } else {
                text.to_owned()
            },
            preview: None,
        })
    }

    /// Cambia el tipo; el nombre y el peso siguen al tipo si no se cambiaron a mano.
    pub fn set_kind(&mut self, kind: VariantKind) {
        let old = self.recipe.kind;
        if self.name == old.style_name() {
            self.name = kind.style_name().to_owned();
        }
        if self.weight == old.weight() {
            self.weight = kind.weight();
        }
        self.recipe.kind = kind;
    }

    /// El estilo que tendrá la variante.
    pub fn style(&self) -> FontStyle {
        let kind = self.recipe.kind;
        FontStyle {
            name: self.name.trim().to_owned(),
            weight: self.weight,
            italic: kind.italic(),
            slant: if kind.italic() { self.recipe.slant } else { 0 },
            source: None,
        }
    }

    /// La vista previa con la receta de ahora (se rehace solo si cambió).
    fn preview(&mut self, editor: &Editor) -> Option<&Artboard> {
        if editor.active != self.source {
            return None;
        }
        if self.preview.as_ref().is_none_or(|(r, _)| *r != self.recipe) {
            let (font, sprite) = editor.variant_of_active(&self.recipe, self.style())?;
            let artboard = Artboard {
                model: None,
                id: ArtboardId(0),
                name: font.full_name(),
                position: (0, 0),
                sprite,
                guides: Vec::new(),
                symmetry: Default::default(),
                in_library: false,
                font: Some(font),
            };
            self.preview = Some((self.recipe, artboard));
        }
        self.preview.as_ref().map(|(_, a)| a)
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        skin: Option<&Skin>,
        editor: &Editor,
    ) -> DialogResult {
        let buttons = ["Create", "Cancel"];
        let taken = editor.family_has_style(&self.name);
        let (result, open) =
            dialog_window_with(ctx, skin, "new_variant", "New Variant", buttons, |ui| {
                ui.label(format!("From {}", self.from));
                ui.add_space(4.0);
                egui::Grid::new("new_variant_grid")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Style:");
                        ui.vertical(|ui| {
                            for kind in VariantKind::ALL {
                                let mut current = self.recipe.kind;
                                if widgets::radio_value(ui, &mut current, kind, kind.label())
                                    .clicked()
                                {
                                    self.set_kind(kind);
                                }
                            }
                        });
                        ui.end_row();
                        let kind = self.recipe.kind;
                        if kind.italic() {
                            ui.label("Slant:");
                            widgets::number(ui, &mut self.recipe.slant, 1..=8, " rows per px");
                            ui.end_row();
                        }
                        if kind == VariantKind::Outline {
                            ui.label("Outline:");
                            ui.horizontal(|ui| {
                                widgets::radio_value(ui, &mut self.recipe.square, false, "Circle");
                                widgets::radio_value(ui, &mut self.recipe.square, true, "Square");
                            });
                            ui.end_row();
                        }
                        let colored = matches!(kind, VariantKind::Outline | VariantKind::Shadow);
                        if colored && self.mode == FontMode::Color {
                            ui.label("Color:");
                            widgets::color_edit(ui, &mut self.recipe.color, true, "Variant color");
                            ui.end_row();
                        }
                        ui.label("Name:");
                        widgets::text_line_named(ui, &mut self.name, "Style name");
                        ui.end_row();
                        ui.label("Weight:");
                        let selected =
                            format!("{} {}", self.weight, cs_core::weight_name(self.weight));
                        let weight = &mut self.weight;
                        widgets::combo(ui, "new_variant_weight", &selected, |ui| {
                            for (w, name) in WEIGHTS {
                                widgets::choice(ui, weight, w, &format!("{w} {name}"));
                            }
                        });
                        ui.end_row();
                    });
                if taken {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!("The family has a \"{}\" style already.", self.name.trim()),
                    );
                }
                ui.add_space(6.0);
                let text = self.text.clone();
                if let Some(artboard) = self.preview(editor) {
                    crate::font_io::paint_preview(ui, artboard, &text);
                }
            });
        finish(ctx, open, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_weight_follow_the_kind_until_edited() {
        let mut e = Editor::with_sprite(cs_core::Sprite::new(8, 8, cs_core::ColorMode::Rgba));
        assert!(e.new_font("Mine", (8, 8), FontMode::OneBit));
        let mut d = NewVariantDialog::new(&e, "").unwrap();
        assert_eq!((d.name.as_str(), d.weight), ("Bold", 700));
        d.set_kind(VariantKind::Italic);
        assert_eq!((d.name.as_str(), d.weight), ("Italic", 400));
        assert!(d.style().italic && d.style().slant == 3);
        d.name = "Oblique".into();
        d.set_kind(VariantKind::BoldItalic);
        assert_eq!((d.name.as_str(), d.weight), ("Oblique", 700));
        assert_eq!(d.text, "AaBbGg 0123");
        assert!(d.preview(&e).is_some());
    }
}
