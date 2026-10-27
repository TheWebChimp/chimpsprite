//! Select → Color Range… y Select → Modify (Border, Expand, Contract), como en Aseprite: la
//! selección cambia en el lienzo mientras se mueven los valores; OK la deja y Cancel, Escape o
//! la × vuelven a la de antes.

use cs_core::{Mask, Rgba};
use eframe::egui::{self, Ui};

use crate::editor::Editor;
use crate::skin::Skin;
use crate::tilemap_dialog::DialogResult;
use crate::{pixel_ui, widgets};

/// El ancho de lo de adentro (el del selector de color).
const WIDTH: f32 = 200.0;

/// Qué hace Select → Modify con la selección.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modify {
    Border,
    Expand,
    Contract,
}

impl Modify {
    pub const ALL: [Modify; 3] = [Modify::Border, Modify::Expand, Modify::Contract];

    pub fn label(self) -> &'static str {
        match self {
            Modify::Border => "Border",
            Modify::Expand => "Expand",
            Modify::Contract => "Contract",
        }
    }

    fn apply(self, mask: &Mask, px: u32) -> Mask {
        match self {
            Modify::Border => mask.border(px),
            Modify::Expand => mask.expanded(px),
            Modify::Contract => mask.contracted(px),
        }
    }
}

enum Kind {
    /// Los píxeles de la capa actual a menos de `tolerance` (en cada canal) de `color`.
    ColorRange {
        color: Rgba,
        tolerance: u8,
    },
    Modify {
        how: Modify,
        px: u32,
    },
}

pub struct SelectDialog {
    kind: Kind,
    /// La selección de antes de abrir: Cancel vuelve a ella y Modify parte de ella.
    before: Option<Mask>,
}

impl SelectDialog {
    /// Color Range con el color primario.
    pub fn color_range(editor: &Editor) -> Self {
        Self {
            kind: Kind::ColorRange {
                color: editor.resolve(editor.primary),
                tolerance: 0,
            },
            before: editor.selection.clone(),
        }
    }

    /// Modify, sobre la selección de ahora (sin selección no se abre).
    pub fn modify(editor: &Editor, how: Modify) -> Option<Self> {
        editor.selection.as_ref()?;
        Some(Self {
            kind: Kind::Modify { how, px: 1 },
            before: editor.selection.clone(),
        })
    }

    fn title(&self) -> &'static str {
        match self.kind {
            Kind::ColorRange { .. } => "Color Range",
            Kind::Modify { how, .. } => how.label(),
        }
    }

    /// La selección con los valores de ahora.
    fn selection(&self, editor: &Editor) -> Option<Mask> {
        match self.kind {
            Kind::ColorRange { color, tolerance } => Some(editor.color_range(color, tolerance)),
            Kind::Modify { how, px } => self.before.as_ref().map(|m| how.apply(m, px)),
        }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        editor: &mut Editor,
        skin: Option<&Skin>,
    ) -> DialogResult {
        let mut result = DialogResult::Open;
        let (_, closed) = pixel_ui::app_window(
            ctx,
            skin,
            "select_dialog",
            self.title(),
            true,
            (egui::Align2::CENTER_CENTER, egui::Vec2::ZERO),
            |ui| {
                ui.set_width(WIDTH);
                self.contents(ui, skin);
                ui.add_space(6.0);
                match pixel_ui::dialog_buttons(ui, skin, &["OK", "Cancel"]) {
                    Some(0) => result = DialogResult::Accept,
                    Some(_) => result = DialogResult::Cancel,
                    None => {}
                }
            },
        );
        let (enter, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        let result = if closed || escape {
            DialogResult::Cancel
        } else if enter {
            DialogResult::Accept
        } else {
            result
        };
        let selection = match result {
            DialogResult::Cancel => self.before.clone(),
            _ => self.selection(editor).filter(|m| !m.is_empty()),
        };
        if editor.selection != selection {
            editor.set_selection(selection);
        }
        result
    }

    fn contents(&mut self, ui: &mut Ui, skin: Option<&Skin>) {
        match &mut self.kind {
            Kind::ColorRange { color, tolerance } => {
                crate::filter_dialog::color_picker(ui, skin, color);
                ui.horizontal(|ui| {
                    ui.label("Tolerance");
                    widgets::number(ui, tolerance, 0..=255, "");
                });
            }
            Kind::Modify { px, .. } => {
                ui.horizontal(|ui| {
                    ui.label("Width");
                    widgets::number(ui, px, 1..=100, " px");
                });
            }
        }
    }
}

impl Editor {
    /// Lo de la capa actual (en el frame actual) cuyo color está a menos de `tolerance` de
    /// `color` en cada canal.
    pub fn color_range(&self, color: Rgba, tolerance: u8) -> Mask {
        let sprite = self.sprite();
        let near = |a: u8, b: u8| a.abs_diff(b) <= tolerance;
        let points = (0..sprite.height as i32)
            .flat_map(|y| (0..sprite.width as i32).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let c = self.resolve(sprite.pixel(self.layer, self.frame, x, y));
                near(c.r, color.r) && near(c.g, color.g) && near(c.b, color.b) && near(c.a, color.a)
            });
        Mask::from_points(sprite.width, sprite.height, points)
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, PixelValue, Sprite};

    use super::*;

    #[test]
    fn color_range_selects_every_pixel_of_that_color() {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        let red = Rgba::rgb(255, 0, 0);
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![
                (0, 0, PixelValue::Rgba(red)),
                (3, 3, PixelValue::Rgba(red)),
                (1, 1, PixelValue::Rgba(Rgba::rgb(250, 0, 0))),
            ],
        });
        let exact: Vec<_> = e.color_range(red, 0).points().collect();
        assert_eq!(exact, vec![(0, 0), (3, 3)], "not contiguous");
        assert_eq!(e.color_range(red, 5).points().count(), 3, "with tolerance");
    }

    #[test]
    fn the_dialog_previews_on_the_canvas_and_cancel_brings_back_the_selection() {
        use eframe::egui::{Event, Key, RawInput};
        let mut e = Editor::with_sprite(Sprite::new(10, 10, ColorMode::Rgba));
        let square = Mask::from_rect(10, 10, (3, 3), (6, 6));
        e.set_selection(Some(square.clone()));
        let mut d = SelectDialog::modify(&e, Modify::Expand).expect("with a selection");
        let ctx = egui::Context::default();
        let frame = |d: &mut SelectDialog, e: &mut Editor, key: Option<Key>| {
            let events = key
                .map(|key| {
                    vec![Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Default::default(),
                    }]
                })
                .unwrap_or_default();
            let mut result = DialogResult::Open;
            let mut out = ctx.run_ui(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ui| result = d.show(ui.ctx(), e, None),
            );
            out.textures_delta.clear();
            result
        };
        frame(&mut d, &mut e, None);
        assert_eq!(e.selection, Some(square.expanded(1)), "previewed");
        assert!(matches!(
            frame(&mut d, &mut e, Some(Key::Escape)),
            DialogResult::Cancel
        ));
        assert_eq!(e.selection, Some(square), "back");
    }

    #[test]
    fn modify_expands_contracts_and_borders_the_selection() {
        let square = Mask::from_rect(10, 10, (3, 3), (6, 6));
        assert!(square.expanded(1).contains(2, 4) && !square.expanded(1).contains(2, 2));
        let inner: Vec<_> = square.contracted(1).points().collect();
        assert_eq!(inner, vec![(4, 4), (5, 4), (4, 5), (5, 5)]);
        let ring = square.border(1);
        assert_eq!(ring.points().count(), 16 - 4);
        assert!(!ring.contains(4, 4) && ring.contains(3, 3));
    }
}
