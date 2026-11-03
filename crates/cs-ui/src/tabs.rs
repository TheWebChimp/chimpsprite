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
    /// El panel Library de este documento: lo que se renombra, lo que se borra y las texturas.
    pub library: crate::library_panel::LibraryState,
    /// Las pestañas de vista abiertas (modelos y fuentes de este documento), en orden.
    pub views: Vec<ArtboardId>,
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
            library: Default::default(),
            views: Vec::new(),
        }
    }

    pub fn untitled(name: impl Into<String>) -> Self {
        Self::untitled_with_palette(name, cs_core::Palette::splendor128())
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
    pub icon: TabIcon,
}

/// El ícono que va antes del nombre de una pestaña de vista.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabIcon {
    None,
    Model,
    Font,
    /// Una pieza del skin con ese nombre (los objetos del panel 3D: `icon.object.cube`…).
    Piece(&'static str),
}

/// Una pestaña que no es un documento. Va después de las de documentos y, mientras se ve, no hay
/// documento activo (como en Home).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    /// El UI Catalog: los componentes del kit en todos sus estados.
    Catalog,
}

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Catalog => crate::kit::catalog::TITLE,
        }
    }
}

/// Una pestaña de la barra: un documento (su índice), la vista de un modelo o una fuente de ese
/// documento, o una página.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabRef {
    Doc(usize),
    View(usize, ArtboardId),
    Page(Page),
}

#[derive(Default)]
pub(crate) struct Tabs {
    /// Una entrada por pestaña; la activa queda en `None` (su `DocState` está en la app).
    slots: Vec<Option<DocState>>,
    /// `None` = Home (o la página `page`).
    active: Option<usize>,
    /// Las páginas abiertas, en orden de pestaña (después de los documentos).
    pages: Vec<Page>,
    /// La página a la vista (solo sin documento activo).
    page: Option<Page>,
}

impl Tabs {
    /// El documento activo (`None`: Home o una página).
    pub fn active(&self) -> Option<usize> {
        self.active
    }

    /// La página a la vista (`None`: un documento o Home).
    pub fn page(&self) -> Option<Page> {
        self.page
    }

    /// Muestra la página `page` (y la abre si no estaba): guarda el documento activo.
    pub fn show_page(&mut self, current: &mut DocState, page: Page) {
        self.stash(current);
        self.active = None;
        if !self.pages.contains(&page) {
            self.pages.push(page);
        }
        self.page = Some(page);
    }

    /// Cierra la página `page`; si se veía, queda Home.
    pub fn close_page(&mut self, page: Page) {
        self.pages.retain(|p| *p != page);
        if self.page == Some(page) {
            self.page = None;
        }
    }

    /// Las pestañas de la barra en orden: cada documento seguido de sus vistas y, al final, las
    /// páginas.
    pub fn bar(&self, current: &DocState) -> Vec<TabRef> {
        let mut bar = self.doc_bar(current);
        bar.extend(self.pages.iter().map(|&p| TabRef::Page(p)));
        bar
    }

    /// Los documentos y sus vistas (la barra sin las páginas).
    fn doc_bar(&self, current: &DocState) -> Vec<TabRef> {
        let mut bar = Vec::new();
        for i in 0..self.len() {
            bar.push(TabRef::Doc(i));
            bar.extend(
                self.get(current, i)
                    .views
                    .iter()
                    .map(|&id| TabRef::View(i, id)),
            );
        }
        bar
    }

    /// La pestaña del documento activo que se ve: su vista si está en el workspace; si no, él.
    fn shown_doc_tab(&self, current: &DocState) -> Option<TabRef> {
        let a = self.active?;
        Some(match current.editor.workspace() {
            Some(id) if current.views.contains(&id) => TabRef::View(a, id),
            _ => TabRef::Doc(a),
        })
    }

    /// Cuál de [`Tabs::bar`] es la que se ve (`None`: Home).
    pub fn bar_active(&self, current: &DocState) -> Option<usize> {
        let page = self.page.map(TabRef::Page);
        let shown = self.shown_doc_tab(current).or(page)?;
        self.bar(current).iter().position(|t| *t == shown)
    }

    /// Nombre y estado de cada pestaña de [`Tabs::bar`].
    pub fn bar_infos(&self, current: &DocState) -> Vec<TabInfo> {
        let mut infos: Vec<TabInfo> = self
            .doc_bar(current)
            .into_iter()
            .map(|tab| match tab {
                TabRef::Doc(i) => {
                    let doc = self.get(current, i);
                    TabInfo {
                        name: doc.location.name.clone(),
                        dirty: doc.is_dirty(),
                        icon: TabIcon::None,
                    }
                }
                TabRef::View(i, id) => {
                    let doc = self.get(current, i);
                    let artboard = doc.editor.document.artboard(id);
                    TabInfo {
                        name: artboard.map(|a| a.name.clone()).unwrap_or_default(),
                        dirty: doc.is_dirty(),
                        icon: if artboard.is_some_and(|a| a.model.is_some()) {
                            TabIcon::Model
                        } else {
                            TabIcon::Font
                        },
                    }
                }
                TabRef::Page(_) => unreachable!("las páginas no están en `doc_bar`"),
            })
            .collect();
        infos.extend(self.pages.iter().map(|p| TabInfo {
            name: p.title().to_owned(),
            dirty: false,
            icon: TabIcon::None,
        }));
        infos
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

    /// Todos los documentos, el activo (`current`) y los guardados en sus pestañas.
    pub fn docs_mut<'a>(
        &'a mut self,
        current: &'a mut DocState,
    ) -> impl Iterator<Item = &'a mut DocState> {
        std::iter::once(current).chain(self.slots.iter_mut().flatten())
    }

    pub fn infos(&self, current: &DocState) -> Vec<TabInfo> {
        (0..self.len())
            .map(|i| {
                let doc = self.get(current, i);
                TabInfo {
                    name: doc.location.name.clone(),
                    dirty: doc.is_dirty(),
                    icon: TabIcon::None,
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

    /// Muestra el lienzo del documento `index`: sale del workspace de su vista, si estaba en ella.
    pub fn switch(&mut self, current: &mut DocState, index: usize) {
        if index >= self.len() {
            return;
        }
        if Some(index) == self.active {
            current.editor.exit_workspace();
            return;
        }
        self.page = None;
        self.stash(current);
        *current = self.slots[index].take().expect("inactive tab is stored");
        self.active = Some(index);
        current.editor.exit_workspace();
    }

    /// Muestra el lienzo del documento activo (sale del workspace de su vista).
    pub fn show_doc(&mut self, current: &mut DocState) {
        if let Some(a) = self.active {
            self.switch(current, a);
        }
    }

    /// Abre (o activa) la pestaña de vista de `artboard` en el documento activo.
    pub fn open_view(&mut self, current: &mut DocState, artboard: ArtboardId) {
        if self.active.is_none() || !current.editor.has_workspace(artboard) {
            return;
        }
        if !current.views.contains(&artboard) {
            current.views.push(artboard);
        }
        current.editor.enter_workspace(artboard);
    }

    /// Activa la pestaña de vista de `artboard` del documento `doc`: cambia a él y entra al
    /// workspace.
    pub fn switch_view(&mut self, current: &mut DocState, doc: usize, artboard: ArtboardId) {
        if Some(doc) != self.active {
            self.switch(current, doc);
        }
        if Some(doc) == self.active {
            current.editor.enter_workspace(artboard);
        }
    }

    /// Cierra la pestaña de vista de `artboard` del documento `doc` (sin preguntar: no hay nada
    /// que guardar). Si se estaba viendo, vuelve al lienzo de ese documento.
    pub fn close_view(&mut self, current: &mut DocState, doc: usize, artboard: ArtboardId) {
        let target = if Some(doc) == self.active {
            Some(current)
        } else {
            self.slots.get_mut(doc).and_then(Option::as_mut)
        };
        if let Some(target) = target {
            target.views.retain(|&id| id != artboard);
            if target.editor.workspace() == Some(artboard) {
                target.editor.exit_workspace();
            }
        }
    }

    /// Quita de las vistas de todos los documentos los artboards que ya no existen o ya no son
    /// modelo ni fuente (se borraron, o se deshizo su creación).
    pub fn sync_views(&mut self, current: &mut DocState) {
        for doc in self.docs_mut(current) {
            let editor = &doc.editor;
            doc.views.retain(|&id| editor.has_workspace(id));
        }
    }

    pub fn go_home(&mut self, current: &mut DocState) {
        self.stash(current);
        self.active = None;
        self.page = None;
    }

    /// Abre `doc` en una pestaña nueva (o en lugar de un Untitled intacto) y la activa.
    pub fn open(&mut self, current: &mut DocState, doc: DocState) {
        if self.active.is_some() && current.is_pristine() {
            *current = doc;
            return;
        }
        self.page = None;
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

    /// Siguiente (o anterior) pestaña, pasando por Home. Recorre los documentos y sus vistas.
    pub fn cycle(&mut self, current: &mut DocState, forward: bool) {
        let list = self.doc_bar(current);
        let n = list.len() as isize + 1; // Home + pestañas
        let shown = self.shown_doc_tab(current);
        let pos = shown
            .and_then(|t| list.iter().position(|x| *x == t))
            .map_or(0, |p| p as isize + 1);
        let next = (pos + if forward { 1 } else { -1 }).rem_euclid(n);
        if next == 0 {
            self.go_home(current);
            return;
        }
        match list[next as usize - 1] {
            TabRef::Doc(i) => self.switch(current, i),
            TabRef::View(i, id) => self.switch_view(current, i, id),
            TabRef::Page(_) => {}
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
    fn a_page_is_a_tab_after_the_documents_and_hides_the_active_one() {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, named("a"));
        touch(&mut current);
        tabs.show_page(&mut current, Page::Catalog);
        assert_eq!((tabs.active(), tabs.page()), (None, Some(Page::Catalog)));
        assert_eq!(
            tabs.bar(&current),
            vec![TabRef::Doc(0), TabRef::Page(Page::Catalog)]
        );
        assert_eq!(tabs.bar_active(&current), Some(1));
        let infos = tabs.bar_infos(&current);
        assert_eq!(infos[1].name, "UI Catalog");
        // Otra vez no la duplica; un documento la tapa y sigue abierta.
        tabs.show_page(&mut current, Page::Catalog);
        tabs.switch(&mut current, 0);
        assert_eq!(current.location.name, "a");
        assert_eq!((tabs.page(), tabs.bar(&current).len()), (None, 2));
        tabs.show_page(&mut current, Page::Catalog);
        tabs.go_home(&mut current);
        assert_eq!((tabs.page(), tabs.bar_active(&current)), (None, None));
        // Cerrada a la vista, queda Home.
        tabs.show_page(&mut current, Page::Catalog);
        tabs.close_page(Page::Catalog);
        assert_eq!(
            (tabs.page(), tabs.bar(&current)),
            (None, vec![TabRef::Doc(0)])
        );
    }

    /// Dos documentos; el primero con un modelo "Ship" (con su vista abierta). Queda activo el 0.
    fn with_model() -> (Tabs, DocState, ArtboardId) {
        let mut tabs = Tabs::default();
        let mut current = named("");
        tabs.open(&mut current, named("a"));
        touch(&mut current);
        assert!(current.editor.new_model("Ship", (32, 32)));
        let id = current.editor.workspace().expect("enters the workspace");
        tabs.open_view(&mut current, id);
        tabs.open(&mut current, named("b"));
        touch(&mut current);
        tabs.switch(&mut current, 0);
        (tabs, current, id)
    }

    #[test]
    fn a_view_tab_follows_its_document() {
        let (tabs, current, id) = with_model();
        assert_eq!(
            tabs.bar(&current),
            vec![TabRef::Doc(0), TabRef::View(0, id), TabRef::Doc(1)]
        );
        let infos = tabs.bar_infos(&current);
        assert_eq!(infos[1].name, "Ship");
        assert_eq!(infos[1].icon, TabIcon::Model);
        assert_eq!(infos[1].dirty, infos[0].dirty);
    }

    #[test]
    fn opening_switching_and_closing_a_view() {
        let (mut tabs, mut current, id) = with_model();
        // `switch` al documento deja el lienzo.
        assert_eq!(tabs.bar_active(&current), Some(0));
        tabs.switch_view(&mut current, 0, id);
        assert_eq!(tabs.bar_active(&current), Some(1));
        // Otro documento y de vuelta a la vista.
        tabs.switch(&mut current, 1);
        assert_eq!(tabs.bar_active(&current), Some(2));
        tabs.switch_view(&mut current, 0, id);
        assert_eq!(current.editor.workspace(), Some(id));
        assert_eq!(tabs.bar_active(&current), Some(1));
        // Cerrar la activa regresa al lienzo de su documento.
        tabs.close_view(&mut current, 0, id);
        assert_eq!(current.editor.workspace(), None);
        assert_eq!(tabs.bar_active(&current), Some(0));
        assert_eq!(tabs.bar(&current), vec![TabRef::Doc(0), TabRef::Doc(1)]);
    }

    #[test]
    fn ctrl_tab_walks_through_the_views() {
        let (mut tabs, mut current, id) = with_model();
        tabs.cycle(&mut current, true); // Doc(0) → View
        assert_eq!(current.editor.workspace(), Some(id));
        tabs.cycle(&mut current, true); // View → Doc(1)
        assert_eq!(tabs.active(), Some(1));
        tabs.cycle(&mut current, false); // Doc(1) → View
        assert_eq!(tabs.bar_active(&current), Some(1));
    }

    #[test]
    fn the_face_selection_does_not_follow_to_the_canvas_from_its_own_texture() {
        use crate::editor::CadMode;
        use cs_3d::{Item, SelectMode, Selection};
        // Se entra a la vista del modelo desde su textura: al volver, la textura sigue activa.
        for close in [false, true] {
            let (mut tabs, mut current, id) = with_model();
            let texture = current.editor.document.artboard(id).unwrap();
            let texture = texture.model.as_ref().unwrap().texture.unwrap();
            current.editor.activate(texture);
            tabs.switch_view(&mut current, 0, id);
            current.editor.set_cad_mode(CadMode::Texture);
            current.editor.set_model_selection(Selection {
                mode: SelectMode::Face,
                items: [Item::Face(0, 0)].into(),
            });
            assert!(current.editor.selection_is_face_mask());
            if close {
                tabs.close_view(&mut current, 0, id);
            } else {
                tabs.switch(&mut current, 0);
            }
            let e = &current.editor;
            assert_eq!((e.workspace(), e.active), (None, texture), "close: {close}");
            assert!(e.selection.is_none(), "close: {close}");
            assert!(!e.selection_is_face_mask(), "close: {close}");
        }
    }

    #[test]
    fn closing_a_document_closes_its_views() {
        let (mut tabs, mut current, _) = with_model();
        tabs.close(&mut current, 0);
        assert_eq!(tabs.bar(&current), vec![TabRef::Doc(0)]);
        assert_eq!(current.location.name, "b");
    }

    #[test]
    fn views_of_deleted_artboards_are_dropped() {
        let (mut tabs, mut current, id) = with_model();
        tabs.switch_view(&mut current, 0, id);
        current.editor.undo(); // deshace la creación del modelo (o lo último)
        while current.editor.document.artboard(id).is_some() {
            assert!(current.editor.history.can_undo());
            current.editor.undo();
        }
        current.editor.ensure_valid();
        tabs.sync_views(&mut current);
        assert!(current.views.is_empty());
        assert_eq!(tabs.bar(&current), vec![TabRef::Doc(0), TabRef::Doc(1)]);
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
