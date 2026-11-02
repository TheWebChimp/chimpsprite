//! El workspace: el modelo o la fuente que se edita en su propia pestaña, fuera del lienzo. Es
//! estado de la UI (no se guarda ni pasa por undo); el historial es el del documento.

use cs_3d::Selection;
use cs_core::ArtboardId;

use super::Editor;
use super::model_edit::ModelEdit;

impl Editor {
    /// El modelo o la fuente que se edita en su pestaña (`None`: el lienzo).
    pub fn workspace(&self) -> Option<ArtboardId> {
        self.workspace
    }

    /// Si `id` puede tener workspace: un modelo o una fuente.
    pub(crate) fn has_workspace(&self, id: ArtboardId) -> bool {
        self.document
            .artboard(id)
            .is_some_and(|a| a.model.is_some() || a.font.is_some())
    }

    /// Entra al workspace de `id` (un modelo o una fuente): recuerda el artboard del lienzo activo
    /// y activa `id`. `false` si `id` no es modelo ni fuente.
    pub fn enter_workspace(&mut self, id: ArtboardId) -> bool {
        if !self.has_workspace(id) {
            return false;
        }
        // Sale del modo símbolo (`ensure_valid` regresa a su artboard del lienzo).
        if !self.symbol_stack.is_empty() {
            self.exit_symbol_to(0);
        }
        // De un workspace a otro se conserva el artboard del lienzo de antes del primero.
        if self.on_canvas(self.active) {
            self.workspace_return = Some(self.active);
        }
        self.workspace = Some(id);
        self.activate(id);
        if self.is_model() {
            self.reset_model_edit(id);
        }
        self.revision += 1;
        true
    }

    /// Regresa al lienzo, al artboard que estaba activo (o al primero del lienzo). Un gesto del
    /// modelo a medias (p. ej. una G de teclado) se cancela.
    pub fn exit_workspace(&mut self) {
        if self.workspace.take().is_none() {
            return;
        }
        self.cancel_model_op();
        let back = self
            .workspace_return
            .take()
            .filter(|&id| self.on_canvas(id))
            .or_else(|| self.document.canvas_artboards().next().map(|a| a.id));
        match back {
            Some(id) => self.activate(id),
            None => self.ensure_valid(),
        }
        self.revision += 1;
    }

    /// Al entrar al workspace de un modelo: en otro modelo el modo Edit empieza de cero (con el
    /// mismo modo de selección); siempre se encuadra el modelo cuando el lienzo sepa el área.
    fn reset_model_edit(&mut self, id: ArtboardId) {
        if self.model_edit.artboard != Some(id) {
            let mode = self.model_edit.selection.mode;
            self.model_edit = ModelEdit {
                artboard: Some(id),
                selection: Selection {
                    mode,
                    items: Default::default(),
                },
                ..Default::default()
            };
        }
        let edit = &mut self.model_edit;
        edit.op = None;
        edit.preview = None;
        edit.key_request = None;
        edit.fit = true;
        if let Some(b) = self.model().and_then(cs_3d::model_bounds) {
            self.model_edit.center = cs_3d::center(b);
        }
    }
}
