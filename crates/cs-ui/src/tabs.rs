//! Pestañas de documentos. La app tiene el documento activo en `ChimpApp::doc`; las demás pestañas
//! viven aquí y cambiar de pestaña intercambia `DocState`s.

use std::path::Path;

use cs_core::ArtboardId;

use crate::canvas::CanvasView;
use crate::editor::Editor;
use crate::file_actions::DocLocation;
use crate::timeline::TimelineState;

/// Todo lo que pertenece a un documento abierto.
pub(crate) struct DocState {
    pub editor: Editor,
    pub canvas: CanvasView,
    pub timeline: TimelineState,
    pub location: DocLocation,
    /// `Editor::doc_version` en el último guardado.
    pub saved_version: u64,
    /// Texto del nombre del artboard activo mientras se edita.
    pub artboard_name: (ArtboardId, String),
    /// Miniaturas del panel Tileset.
    pub tile_thumbs: crate::panels::TileThumbs,
}

impl DocState {
    pub fn new(editor: Editor, location: DocLocation) -> Self {
        Self {
            saved_version: editor.doc_version,
            editor,
            canvas: CanvasView::default(),
            timeline: TimelineState::default(),
            location,
            artboard_name: (ArtboardId(0), String::new()),
            tile_thumbs: Default::default(),
        }
    }

    pub fn untitled(name: impl Into<String>) -> Self {
        Self::untitled_with_palette(name, cs_core::Palette::db32())
    }

    /// Documento nuevo con la paleta por defecto del usuario.
    pub fn untitled_with_palette(name: impl Into<String>, palette: cs_core::Palette) -> Self {
        Self::new(
            crate::new_editor(palette),
            DocLocation {
                name: name.into(),
                target: None,
            },
        )
    }

    /// Nombre de la pestaña sin extensión (para sugerir nombres de archivo).
    pub fn tab_name_stem(&self) -> String {
        crate::files::stem(&self.location.name).to_string()
    }

    /// Hay cambios sin guardar. Lo flotante cuenta: todavía no está en el documento.
    pub fn is_dirty(&self) -> bool {
        self.editor.doc_version != self.saved_version || self.editor.is_floating()
    }

    /// Recién creado y sin tocar: se puede reemplazar al abrir otro archivo.
    pub fn is_pristine(&self) -> bool {
        !self.is_dirty() && self.location.target.is_none() && !self.editor.history.can_undo()
    }

    pub fn is_at(&self, path: &Path) -> bool {
        self.location
            .target
            .as_ref()
            .and_then(|t| t.path())
            .is_some_and(|p| p == path)
    }
}

/// Nombre y estado de una pestaña, para dibujar la barra.
pub(crate) struct TabInfo {
    pub name: String,
    pub dirty: bool,
}

#[derive(Default)]
pub(crate) struct Tabs {
    /// Una entrada por pestaña; la activa queda en `None` (su `DocState` está en la app).
    slots: Vec<Option<DocState>>,
    /// `None` = Home.
    active: Option<usize>,
}

impl Tabs {
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    fn get<'a>(&'a self, current: &'a DocState, index: usize) -> &'a DocState {
        if Some(index) == self.active {
            current
        } else {
            self.slots[index].as_ref().expect("inactive tab is stored")
        }
    }

    /// Todos los documentos en orden de pestaña.
    pub fn docs<'a>(&'a self, current: &'a DocState) -> Vec<&'a DocState> {
        (0..self.len()).map(|i| self.get(current, i)).collect()
    }

    pub fn infos(&self, current: &DocState) -> Vec<TabInfo> {
        (0..self.len())
            .map(|i| {
                let doc = self.get(current, i);
                TabInfo {
                    name: doc.location.name.clone(),
                    dirty: doc.is_dirty(),
                }
            })
            .collect()
    }

    /// Índices de las pestañas con cambios sin guardar.
    pub fn dirty(&self, current: &DocState) -> Vec<usize> {
        (0..self.len())
            .filter(|&i| self.get(current, i).is_dirty())
            .collect()
    }

    pub fn find_path(&self, current: &DocState, path: &Path) -> Option<usize> {
        (0..self.len()).find(|&i| self.get(current, i).is_at(path))
    }

    /// Guarda el documento activo en su pestaña y deja en `current` un marcador.
    /// Lo flotante se confirma antes: cambiar de pestaña, ir a Home o abrir otra lo aplica.
    fn stash(&mut self, current: &mut DocState) {
        if let Some(a) = self.active {
            current.editor.commit_floating();
            self.slots[a] = Some(std::mem::replace(current, DocState::untitled("")));
        }
    }

    pub fn switch(&mut self, current: &mut DocState, index: usize) {
        if index >= self.len() || Some(index) == self.active {
            return;
        }
        self.stash(current);
        *current = self.slots[index].take().expect("inactive tab is stored");
        self.active = Some(index);
    }

    pub fn go_home(&mut self, current: &mut DocState) {
        self.stash(current);
        self.active = None;
    }

    /// Abre `doc` en una pestaña nueva (o en lugar de un Untitled intacto) y la activa.
    pub fn open(&mut self, current: &mut DocState, doc: DocState) {
        if self.active.is_some() && current.is_pristine() {
            *current = doc;
            return;
        }
        self.stash(current);
        self.slots.push(None);
        self.active = Some(self.len() - 1);
        *current = doc;
    }

    /// Cierra la pestaña `index` sin preguntar. Activa la vecina o, si no queda ninguna, Home.
    pub fn close(&mut self, current: &mut DocState, index: usize) {
        if index >= self.len() {
            return;
        }
        let was_active = Some(index) == self.active;
        if was_active {
            *current = DocState::untitled("");
        }
        self.slots.remove(index);
        self.active = match self.active {
            _ if self.slots.is_empty() => None,
            Some(a) if was_active => {
                let next = a.min(self.len() - 1);
                *current = self.slots[next].take().expect("inactive tab is stored");
                Some(next)
            }
            Some(a) if a > index => Some(a - 1),
            other => other,
        };
    }

    /// Siguiente (o anterior) pestaña, pasando por Home.
    pub fn cycle(&mut self, current: &mut DocState, forward: bool) {
        let n = self.len() as isize + 1; // Home + pestañas
        let pos = self.active.map_or(0, |a| a as isize + 1);
        let next = (pos + if forward { 1 } else { -1 }).rem_euclid(n);
        if next == 0 {
            self.go_home(current);
        } else {
            self.switch(current, next as usize - 1);
        }
    }

    /// Siguiente nombre "Untitled N" libre.
    pub fn untitled_name(&self, current: &DocState) -> String {
        let names: Vec<String> = self.infos(current).into_iter().map(|t| t.name).collect();
        (1..)
            .map(|n| {
                if n == 1 {
                    "Untitled".to_string()
                } else {
                    format!("Untitled {n}")
                }
            })
            .find(|name| !names.contains(name))
            .expect("infinite names")
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{LayerId, PixelValue, Rgba};

    use super::*;
    use crate::platform::SaveTarget;

    fn named(name: &str) -> DocState {
        DocState::untitled(name)
    }

    fn touch(doc: &mut DocState) {
        doc.editor.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(1, 1, 1)))],
        });
    }

    fn names(tabs: &Tabs, current: &DocState) -> Vec<String> {
        tabs.infos(current).into_iter().map(|t| t.name).collect()
    }

    #[test]
    fn open_switch_and_home() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, named("a"));
        touch(&mut current); // "a" ya no está intacto
        tabs.open(&mut current, named("b"));
        assert_eq!(names(&tabs, &current), vec!["a", "b"]);
        assert_eq!(
            (tabs.active(), current.location.name.as_str()),
            (Some(1), "b")
        );
        tabs.switch(&mut current, 0);
        assert_eq!(current.location.name, "a");
        assert!(current.is_dirty(), "keeps its own history");
        tabs.go_home(&mut current);
        assert_eq!(tabs.active(), None);
        assert_eq!(names(&tabs, &current), vec!["a", "b"]);
        tabs.switch(&mut current, 1);
        assert_eq!(current.location.name, "b");
    }

    #[test]
    fn pristine_untitled_is_replaced() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, named("Untitled"));
        tabs.open(&mut current, named("hero.chimp"));
        assert_eq!(names(&tabs, &current), vec!["hero.chimp"]);
    }

    #[test]
    fn close_picks_neighbor_and_falls_back_to_home() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        for n in ["a", "b", "c"] {
            tabs.open(&mut current, named(n));
            touch(&mut current);
        }
        tabs.switch(&mut current, 1);
        tabs.close(&mut current, 1);
        assert_eq!(names(&tabs, &current), vec!["a", "c"]);
        assert_eq!(current.location.name, "c");
        tabs.close(&mut current, 0);
        assert_eq!(
            (tabs.active(), current.location.name.as_str()),
            (Some(0), "c")
        );
        tabs.close(&mut current, 0);
        assert_eq!(tabs.active(), None);
        assert_eq!(tabs.len(), 0);
    }

    #[test]
    fn closing_an_inactive_tab_keeps_the_active_one() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        for n in ["a", "b", "c"] {
            tabs.open(&mut current, named(n));
            touch(&mut current);
        }
        tabs.close(&mut current, 0);
        assert_eq!(
            (tabs.active(), current.location.name.as_str()),
            (Some(1), "c")
        );
    }

    #[test]
    fn dirty_find_cycle_and_names() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        let mut saved = named("hero.chimp");
        saved.location.target = SaveTarget::from_path(Path::new("/art/hero.chimp"));
        tabs.open(&mut current, saved);
        tabs.open(&mut current, named("Untitled"));
        touch(&mut current);
        assert_eq!(tabs.dirty(&current), vec![1]);
        assert_eq!(
            tabs.find_path(&current, Path::new("/art/hero.chimp")),
            Some(0)
        );
        assert_eq!(tabs.untitled_name(&current), "Untitled 2");
        tabs.cycle(&mut current, true); // 1 → Home
        assert_eq!(tabs.active(), None);
        tabs.cycle(&mut current, true); // Home → 0
        assert_eq!(tabs.active(), Some(0));
        tabs.cycle(&mut current, false); // 0 → Home
        assert_eq!(tabs.active(), None);
    }
    /// Documento con un punto levantado y movido (flotando, sin cambios en el documento).
    fn floating(name: &str) -> DocState {
        let mut doc = named(name);
        touch(&mut doc);
        doc.saved_version = doc.editor.doc_version;
        let e = &mut doc.editor;
        e.tool = crate::editor::ToolKind::RectSelect;
        let (w, h) = (e.sprite().width, e.sprite().height);
        e.set_selection(Some(cs_core::Mask::from_rect(w, h, (0, 0), (0, 0))));
        let (x, y) = e.artboard().position;
        let world = |p: i32| ((x + p) as f32 + 0.5, (y + p) as f32 + 0.5);
        e.pointer_press(world(0), false, false, 0.1);
        e.pointer_drag(world(2));
        e.pointer_release();
        assert!(e.is_floating());
        doc
    }

    #[test]
    fn floating_content_counts_as_unsaved() {
        let doc = floating("a");
        assert_eq!(doc.editor.doc_version, doc.saved_version);
        assert!(doc.is_dirty());
        assert!(!doc.is_pristine());
    }

    #[test]
    fn switching_tabs_applies_the_floating_content() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, floating("a"));
        tabs.open(&mut current, named("b"));
        tabs.switch(&mut current, 0);
        assert!(!current.editor.is_floating(), "applied when it was stashed");
        let white = PixelValue::Rgba(Rgba::rgb(1, 1, 1));
        assert_eq!(current.editor.sprite().pixel(LayerId(1), 0, 2, 2), white);
        assert!(current.is_dirty());
        tabs.go_home(&mut current);
        assert_eq!(tabs.dirty(&current), vec![0]);
    }

    #[test]
    fn closing_a_tab_with_floating_content_asks_first() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, named("a"));
        touch(&mut current);
        tabs.open(&mut current, floating("b"));
        // `request_close_tab` pregunta por las pestañas de esta lista.
        assert_eq!(tabs.dirty(&current), vec![0, 1]);
    }
}
