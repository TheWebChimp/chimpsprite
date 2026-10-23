//! Máscaras de capa y de recorte desde el editor.

use cs_commands::{Command, LayerProps, ops};
use cs_core::LayerId;

use crate::editor::Editor;

/// Con qué empieza una máscara nueva.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewMask {
    RevealAll,
    HideAll,
    /// Visible solo lo seleccionado.
    FromSelection,
}

impl Editor {
    /// ¿Las herramientas pintan en la máscara de la capa actual?
    pub fn editing_mask(&self) -> bool {
        self.edit_mask && self.layer_has_mask(self.layer)
    }

    /// Entra o sale de la edición de la máscara de la capa actual. Si eso cambia a qué pinta la
    /// herramienta a mitad de un trazo, el trazo se corta.
    pub fn set_edit_mask(&mut self, on: bool) {
        let before = self.paint_target();
        self.edit_mask = on;
        self.cancel_if_target_changed(before);
    }

    /// La herramienta activa no actúa ahora: bloqueada en modo Tiles o mientras se edita una
    /// máscara. Tampoco muestra el contorno del pincel.
    pub fn tool_blocked(&self) -> bool {
        let kind = self.active_tool();
        (self.tiles_mode() && kind.blocked_in_tiles())
            || (self.editing_mask() && kind.blocked_on_mask())
    }

    /// Aviso de la barra de estado cuando la herramienta no actúa sobre la máscara.
    pub fn mask_status(&self) -> Option<&'static str> {
        (self.editing_mask() && self.active_tool().blocked_on_mask())
            .then_some("Not available while editing a mask")
    }

    pub fn layer_has_mask(&self, id: LayerId) -> bool {
        self.sprite().layer(id).is_some_and(|l| l.mask.is_some())
    }

    /// Agrega una máscara a la capa y la deja lista para pintarla.
    pub fn add_mask(&mut self, id: LayerId, how: NewMask) {
        let selection = match how {
            NewMask::FromSelection => self.selection.clone(),
            _ => None,
        };
        let mask = ops::new_mask(self.sprite(), how == NewMask::HideAll, selection.as_ref());
        if self.execute(Command::SetLayerMask {
            id,
            mask: Some(mask),
        }) {
            let before = self.paint_target();
            self.layer = id;
            self.edit_mask = true;
            self.cancel_if_target_changed(before);
            if how == NewMask::FromSelection {
                self.deselect();
            }
        }
    }

    pub fn delete_mask(&mut self, id: LayerId) {
        if self.layer_has_mask(id) {
            self.execute(Command::SetLayerMask { id, mask: None });
        }
    }

    pub fn apply_mask(&mut self, id: LayerId) {
        // Confirma lo flotante antes de armar el comando: lo arma con el contenido de ahora.
        self.commit_floating();
        if let Some(cmd) = ops::apply_mask(self.sprite(), id) {
            self.execute(cmd);
        }
    }

    pub fn toggle_mask_enabled(&mut self, id: LayerId) {
        let Some(mut mask) = self.sprite().layer(id).and_then(|l| l.mask.clone()) else {
            return;
        };
        mask.enabled = !mask.enabled;
        self.execute(Command::SetLayerMask {
            id,
            mask: Some(mask),
        });
    }

    /// Selecciona lo que deja ver la máscara.
    pub fn mask_to_selection(&mut self, id: LayerId) {
        if let Some(selection) = ops::mask_to_selection(self.sprite(), id) {
            self.set_selection(Some(selection));
        }
    }

    /// Activa o quita la máscara de recorte de la capa.
    pub fn toggle_clip(&mut self, id: LayerId) {
        let Some(clip) = self.sprite().layer(id).map(|l| l.clip) else {
            return;
        };
        self.execute(Command::SetLayerProps {
            id,
            props: LayerProps {
                clip: Some(!clip),
                ..Default::default()
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Mask, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::editor::ToolKind;

    fn editor() -> Editor {
        Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba))
    }

    fn click(e: &mut Editor, x: i32, y: i32) {
        e.pointer_press((x as f32 + 0.5, y as f32 + 0.5), false, false, 0.1);
        e.pointer_release();
    }

    #[test]
    fn painting_goes_to_the_mask_while_editing_it() {
        let mut e = editor();
        e.primary = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        click(&mut e, 0, 0);
        e.add_mask(LayerId(1), NewMask::RevealAll);
        assert!(e.editing_mask());
        // El lápiz con negro oculta; el borrador también.
        e.primary = PixelValue::Rgba(Rgba::rgb(0, 0, 0));
        click(&mut e, 0, 0);
        let mask = e.sprite().layers[0].mask.clone().unwrap();
        assert_eq!(mask.get(0, 0), 0);
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 0, 0),
            PixelValue::Rgba(Rgba::rgb(255, 0, 0)),
            "the layer pixels are untouched"
        );
        e.set_tool(ToolKind::Eraser);
        click(&mut e, 1, 0);
        assert_eq!(e.sprite().layers[0].mask.as_ref().unwrap().get(1, 0), 0);
        // Seleccionar la capa vuelve a editar sus píxeles.
        e.select_layer(LayerId(1));
        assert!(!e.editing_mask());
    }

    #[test]
    fn mask_from_selection_toggle_apply_and_clip() {
        let mut e = editor();
        e.set_selection(Some(Mask::from_rect(4, 4, (0, 0), (1, 3))));
        e.add_mask(LayerId(1), NewMask::FromSelection);
        assert!(e.selection.is_none());
        let mask = e.sprite().layers[0].mask.clone().unwrap();
        assert_eq!((mask.get(1, 1), mask.get(2, 1)), (255, 0));
        e.toggle_mask_enabled(LayerId(1));
        assert!(!e.sprite().layers[0].mask.as_ref().unwrap().enabled);
        e.mask_to_selection(LayerId(1));
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 8);
        e.apply_mask(LayerId(1));
        assert!(!e.layer_has_mask(LayerId(1)));
        e.undo();
        assert!(e.layer_has_mask(LayerId(1)));
        e.delete_mask(LayerId(1));
        assert!(!e.layer_has_mask(LayerId(1)));
        e.toggle_clip(LayerId(1));
        assert!(e.sprite().layers[0].clip);
        e.undo();
        assert!(!e.sprite().layers[0].clip);
    }
}
