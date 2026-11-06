//! El workspace: el modelo o la fuente que se edita en su propia pestaña, fuera del lienzo. Es
//! estado de la UI (no se guarda ni pasa por undo); el historial es el del documento.

use cs_3d::{SelectMode, Selection};
use cs_core::ArtboardId;

use super::Editor;
use super::model_edit::{CadMode, ModelEdit};

impl Editor {
    /// El modelo o la fuente que se edita en su pestaña (`None`: el lienzo).
    pub fn workspace(&self) -> Option<ArtboardId> {
        self.workspace
    }

    /// El workspace al que se entró sin pasar por su pestaña, una sola vez: la app le abre la
    /// pestaña de vista y su panel.
    pub fn take_view_request(&mut self) -> Option<ArtboardId> {
        self.view_request.take()
    }

    /// Si el lienzo muestra un solo artboard, el activo: en el modo símbolo, en el workspace de
    /// una fuente o en la textura del modo Texture. Los demás no se ven y tampoco se tocan
    /// (clics, la herramienta Artboard, Fit).
    pub fn single_artboard(&self) -> bool {
        self.in_symbol_mode()
            || (self.workspace == Some(self.active) && self.is_font())
            || self.is_workspace_texture(self.active)
    }

    /// Si `id` puede tener workspace: un modelo o una fuente.
    pub(crate) fn has_workspace(&self, id: ArtboardId) -> bool {
        self.document
            .artboard(id)
            .is_some_and(|a| a.model.is_some() || a.font.is_some())
    }

    /// Entra al workspace de `id` (un modelo o una fuente): recuerda el artboard del lienzo activo
    /// y activa `id`. `false` si `id` no es modelo ni fuente. Entrar a otro deja el modo en
    /// Model; volver a entrar al mismo (su pestaña otra vez) lo conserva.
    pub fn enter_workspace(&mut self, id: ArtboardId) -> bool {
        if !self.has_workspace(id) {
            return false;
        }
        let same = self.workspace == Some(id);
        if !same {
            self.cancel_uv_op();
            self.model_edit.mode = CadMode::Model;
        }
        // Sale del modo símbolo (`ensure_valid` regresa a su artboard del lienzo).
        if !self.symbol_stack.is_empty() {
            self.exit_symbol_to(0);
        }
        // De un workspace a otro se conserva el artboard del lienzo de antes del primero (la
        // textura del modo Texture está en el lienzo, pero no es de donde se vino).
        if self.workspace.is_none() && self.on_canvas(self.active) {
            self.workspace_return = Some(self.active);
        }
        self.workspace = Some(id);
        // En modo Texture del mismo modelo, el activo sigue siendo su textura.
        if !(same && self.is_workspace_texture(self.active)) {
            self.activate(id);
        }
        if self.workspace_model().is_some() {
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
        self.cancel_uv_op();
        self.model_edit.mode = CadMode::Model;
        // Si se regresa a la textura (ya activa), `activate` no hace nada: la selección de las
        // caras se quita aquí para que no se quede en el lienzo.
        self.commit_floating();
        self.sync_face_mask();
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

    /// Al entrar al workspace de un modelo: en otro modelo el modo Edit empieza de cero (en modo
    /// Edge y sin selección) y se encuadra el modelo cuando el lienzo sepa el área; al volver al
    /// mismo se conservan el zoom y el centro de las vistas.
    fn reset_model_edit(&mut self, id: ArtboardId) {
        if self.model_edit.artboard != Some(id) {
            let center = self
                .workspace_model()
                .and_then(cs_3d::model_bounds)
                .map_or([0.0; 3], cs_3d::center);
            self.model_edit = ModelEdit {
                artboard: Some(id),
                selection: Selection {
                    mode: SelectMode::Edge,
                    items: Default::default(),
                },
                center,
                fit: true,
                // El gizmo es de la UI, no del modelo.
                gizmo: self.model_edit.gizmo,
                ..Default::default()
            };
        }
        let edit = &mut self.model_edit;
        edit.op = None;
        edit.uv_op = None;
        edit.preview = None;
        edit.key_request = None;
        if let Some(x) = edit.extrusion.take() {
            edit.selection = x.before;
        }
    }
}
