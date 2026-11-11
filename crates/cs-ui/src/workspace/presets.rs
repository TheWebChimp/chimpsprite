//! Los workspaces con nombre (fase D, tarea 7), como en Photoshop: los de fábrica ([`FACTORY`],
//! ver [`factory`]) y los del usuario (Window → Workspace → New Workspace…). Se usa uno a la vez
//! ([`Workspaces::current`]); su acomodo es el de la app (`app.workspace`).
//!
//! Cada uno recuerda su último acomodo, en la sesión y al cerrar la app: el que se deja se queda
//! como estaba y, al volver a él, vuelve así; Reset lo regresa a su versión de fábrica (o, uno del
//! usuario, a como se creó). Con cada acomodo va lo que se ve ([`Shown`]: el timeline y el
//! Preview): Animation prende el Preview (acoplado a la derecha del timeline) y, al volver a
//! Essentials, se apaga si ahí estaba apagado.
//!
//! Se guarda todo junto con el acomodo de ahora (`SavedWorkspace`, versión 7): lo de antes (sin
//! nombres) es el acomodo de Essentials, cambiado por el usuario.

use egui_dock::Node;

use super::model::{SAVED_VERSION, SavedLayout, SavedWorkspace};
use super::{DockTarget, Split, Workspace};
use crate::panel_registry::PanelId;

/// El de fábrica de siempre (el acomodo de hoy).
pub(crate) const ESSENTIALS: &str = "Essentials";

/// Los workspaces de fábrica, en el orden del menú.
pub(crate) const FACTORY: [&str; 4] = [ESSENTIALS, "Animation", "Tiles", "Fonts"];

/// Qué parte del lado del lienzo (a lo alto) toma de entrada la franja de abajo de Animation: el
/// timeline con el Preview.
const ANIMATION_BAND: f32 = 0.4;
/// Qué parte de esa franja es el timeline (lo demás, el Preview).
const ANIMATION_TIMELINE: f32 = 0.75;
/// Qué parte de la columna derecha es Color en Tiles y en Fonts (lo demás, los paneles de abajo;
/// los mínimos de cada uno ganan, ver `fit::fit_splits`).
const COLUMN_COLOR: f32 = 0.5;

/// Lo que se ve en un acomodo: el timeline y el Preview (`view.timeline` y `view.preview`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    pub timeline: bool,
    pub preview: bool,
}

impl Default for Shown {
    fn default() -> Self {
        Shown {
            timeline: true,
            preview: false,
        }
    }
}

/// El acomodo de fábrica `name` (`None` si no es uno de [`FACTORY`]):
/// - **Essentials**: el de hoy ([`Workspace::essentials`]).
/// - **Animation**: el timeline alto (de entrada el 40 % del lado del lienzo) con el Preview
///   acoplado a su derecha, los dos debajo del lienzo (`V[Canvas / H[Timeline | Preview]]`).
/// - **Tiles**: Artboards y Slices en pestañas, en la columna derecha debajo de Color.
/// - **Fonts**: FontForge en la columna derecha, debajo de Color.
///
/// Lo demás, como Essentials (la toolbar a la izquierda, lo del dock que no se usa en la tira).
pub(crate) fn factory(name: &str) -> Option<Workspace> {
    let mut w = Workspace::essentials();
    match name {
        ESSENTIALS => return Some(w),
        "Animation" => {
            let (s, n) = w.leaf_of(PanelId::Timeline)?;
            w.dock(PanelId::Preview, DockTarget::Split(s, n, Split::Right));
            set_fraction(&mut w, PanelId::Preview, ANIMATION_TIMELINE);
            // El alto de la franja lo pone `fit::pin_sides` (es un lado del lienzo).
            w.sides
                .shares
                .push((vec![PanelId::Timeline, PanelId::Preview], ANIMATION_BAND));
        }
        "Tiles" => {
            let (s, n) = w.leaf_of(PanelId::Color)?;
            w.dock(PanelId::Artboards, DockTarget::Split(s, n, Split::Below));
            let (s, n) = w.leaf_of(PanelId::Artboards)?;
            w.dock(PanelId::Slices, DockTarget::Leaf(s, n));
            set_fraction(&mut w, PanelId::Artboards, COLUMN_COLOR);
            w.focus(PanelId::Artboards);
        }
        "Fonts" => {
            let (s, n) = w.leaf_of(PanelId::Color)?;
            w.dock(PanelId::Font, DockTarget::Split(s, n, Split::Below));
            set_fraction(&mut w, PanelId::Font, COLUMN_COLOR);
        }
        _ => return None,
    }
    // El foco, en el grupo de Color (ahí se acopla lo de la tira), como en Essentials.
    w.refocus(Some(PanelId::Color));
    Some(w)
}

/// Lo que se ve en el de fábrica `name`: el timeline siempre; el Preview, en Animation.
pub(crate) fn factory_shown(name: &str) -> Shown {
    Shown {
        timeline: true,
        preview: name == "Animation",
    }
}

/// La fracción del split del que cuelga el grupo de `id`: lo que toma su lado de arriba (o de la
/// izquierda).
fn set_fraction(w: &mut Workspace, id: PanelId, fraction: f32) {
    if let Some((surface, node)) = w.leaf_of(id)
        && let Some(parent) = node.parent()
        && let Node::Horizontal(split) | Node::Vertical(split) = &mut w.state[surface][parent]
    {
        split.fraction = fraction;
    }
}

/// Los workspaces con nombre: cuál se usa, los del usuario como se crearon y el último acomodo de
/// cada uno de los demás que se dejó (ver el módulo).
#[derive(Clone, Debug)]
pub(crate) struct Workspaces {
    current: String,
    user: Vec<(String, SavedLayout)>,
    edited: Vec<(String, SavedLayout)>,
}

impl Default for Workspaces {
    fn default() -> Self {
        Workspaces {
            current: ESSENTIALS.to_owned(),
            user: Vec::new(),
            edited: Vec::new(),
        }
    }
}

impl Workspaces {
    /// El que se usa.
    pub(crate) fn current(&self) -> &str {
        &self.current
    }

    /// Los del usuario, en el orden en que se crearon.
    pub(crate) fn user_names(&self) -> Vec<&str> {
        self.user.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Los que se pueden borrar: los del usuario menos el que se usa (como en Photoshop).
    pub(crate) fn deletable(&self) -> Vec<&str> {
        self.user_names()
            .into_iter()
            .filter(|n| *n != self.current)
            .collect()
    }

    /// Si hay uno que se llama `name` (de fábrica o del usuario).
    fn exists(&self, name: &str) -> bool {
        FACTORY.contains(&name) || self.user.iter().any(|(n, _)| n == name)
    }

    /// Por qué `name` (sin los espacios de las orillas) no sirve para uno nuevo: vacío o el de uno
    /// que ya existe (sin distinguir mayúsculas, también los de fábrica).
    pub(crate) fn name_problem(&self, name: &str) -> Option<&'static str> {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return Some("Type a name for the workspace");
        }
        let taken = FACTORY
            .into_iter()
            .chain(self.user.iter().map(|(n, _)| n.as_str()))
            .any(|n| n.to_lowercase() == name);
        taken.then_some("A workspace with that name already exists")
    }

    /// El acomodo de `name` sin lo cambiado: el de fábrica o el de cuando se creó, con lo que se ve
    /// en él.
    fn base(&self, name: &str) -> Option<(Workspace, Shown)> {
        if let Some(w) = factory(name) {
            return Some((w, factory_shown(name)));
        }
        let (_, layout) = self.user.iter().find(|(n, _)| n == name)?;
        Some(Workspace::from_layout(SAVED_VERSION, layout.clone()))
    }

    /// `layout` es el último acomodo de `name` (el que se dejó).
    fn remember(&mut self, name: String, layout: SavedLayout) {
        self.edited.retain(|(n, _)| *n != name);
        self.edited.push((name, layout));
    }

    /// Pasa a `name`: el de ahora (`w`, con lo que se ve, `shown`) se queda como está y `w` es el
    /// de `name` como se dejó (o, si no se cambió, su [`Workspaces::base`]). Devuelve lo que se ve
    /// en él; `None` (sin cambiar nada) si es el de ahora o no existe.
    pub(crate) fn switch(&mut self, name: &str, w: &mut Workspace, shown: Shown) -> Option<Shown> {
        if name == self.current || !self.exists(name) {
            return None;
        }
        let edited = self.edited.iter().position(|(n, _)| n == name);
        let (next, shown_next) = match edited {
            Some(i) => {
                let (_, layout) = self.edited.remove(i);
                Workspace::from_layout(SAVED_VERSION, layout)
            }
            None => self.base(name)?,
        };
        let leaving = std::mem::replace(&mut self.current, name.to_owned());
        self.remember(leaving, w.to_layout(shown));
        *w = next;
        Some(shown_next)
    }

    /// Reset: el de ahora vuelve a su [`Workspaces::base`] (lo cambiado se olvida). Devuelve lo que
    /// se ve en él.
    pub(crate) fn reset(&mut self, w: &mut Workspace) -> Shown {
        let current = self.current.clone();
        self.edited.retain(|(n, _)| *n != current);
        let (next, shown) = self
            .base(&current)
            .unwrap_or_else(|| (Workspace::essentials(), Shown::default()));
        *w = next;
        shown
    }

    /// New Workspace…: uno del usuario llamado `name` (sin los espacios de las orillas) con el
    /// acomodo de ahora (`w`, con lo que se ve, `shown`), que pasa a ser el que se usa. El que se
    /// deja se queda como estaba (como al cambiar de uno a otro). Si el nombre no sirve, por qué
    /// (ver [`Workspaces::name_problem`]).
    pub(crate) fn create(
        &mut self,
        name: &str,
        w: &Workspace,
        shown: Shown,
    ) -> Result<(), &'static str> {
        if let Some(problem) = self.name_problem(name) {
            return Err(problem);
        }
        let name = name.trim().to_owned();
        let layout = w.to_layout(shown);
        let leaving = std::mem::replace(&mut self.current, name.clone());
        self.remember(leaving, layout.clone());
        self.user.push((name, layout));
        Ok(())
    }

    /// Delete Workspace…: borra el del usuario `name` (no el que se usa ni uno de fábrica).
    /// Devuelve si lo borró.
    pub(crate) fn delete(&mut self, name: &str) -> bool {
        if name == self.current {
            return false;
        }
        let before = self.user.len();
        self.user.retain(|(n, _)| n != name);
        self.edited.retain(|(n, _)| n != name);
        self.user.len() != before
    }

    /// Lo que se guarda: el acomodo de ahora (`w`) con los nombres.
    pub(crate) fn to_saved(&self, w: &Workspace) -> SavedWorkspace {
        SavedWorkspace {
            current: self.current.clone(),
            user: self.user.clone(),
            edited: self.edited.clone(),
            ..w.to_saved()
        }
    }

    /// Lo guardado, sin confiar en ello: el acomodo de ahora como siempre (ver
    /// [`Workspace::from_saved`]); los del usuario con un nombre que no sirve (vacío, repetido o de
    /// fábrica), fuera; los acomodos dejados de uno que no existe (o repetidos, o del de ahora),
    /// fuera; y si el de ahora no existe (o no viene: lo de una versión 6 o anterior), Essentials,
    /// con ese acomodo. Cada acomodo con nombre se lee como uno guardado (uno roto es Essentials)
    /// y queda en la versión de ahora. Lo que se descarta se anota en el log.
    pub(crate) fn from_saved(saved: SavedWorkspace) -> (Workspaces, Workspace) {
        let version = saved.version;
        let current = saved.current.trim().to_owned();
        let user = saved.user.clone();
        let edited = saved.edited.clone();
        // Leído y escrito: ya en la versión de ahora (y en buen estado).
        let read = |layout: SavedLayout| {
            let (w, shown) = Workspace::from_layout(version, layout);
            w.to_layout(shown)
        };
        let mut ws = Workspaces::default();
        for (name, layout) in user {
            match ws.name_problem(&name) {
                Some(why) => log::warn!("discarding the saved workspace {name:?}: {why}"),
                None => ws.user.push((name.trim().to_owned(), read(layout))),
            }
        }
        if !current.is_empty() {
            if ws.exists(&current) {
                ws.current = current;
            } else {
                log::warn!("the saved workspace {current:?} does not exist: using {ESSENTIALS}");
            }
        }
        for (name, layout) in edited {
            let known = ws.exists(&name) && name != ws.current;
            if !known || ws.edited.iter().any(|(n, _)| *n == name) {
                log::warn!("discarding the saved layout of the workspace {name:?}");
                continue;
            }
            ws.edited.push((name, read(layout)));
        }
        (ws, Workspace::from_saved(saved))
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui;
    use egui_dock::{DockState, NodePath};

    use super::*;
    use crate::dock::Edge;
    use crate::panel_registry::PanelId;
    use crate::workspace::model::{SAVED_VERSION, sound};
    use crate::workspace::test_util::{frames, test_app};
    use crate::workspace::{DockTarget, NodeIndex, Split};

    /// Cada panel una vez (el Preview, solo si se ve), el lienzo solo en su grupo de la superficie
    /// principal y un árbol que egui_dock puede dibujar.
    fn assert_invariants(name: &str, w: &Workspace, shown: Shown) {
        for &id in PanelId::ALL {
            let want = usize::from(id != PanelId::Preview || shown.preview);
            assert_eq!(w.places(id), want, "{name}: {id:?}");
        }
        let (surface, node) = w.leaf_of(PanelId::Canvas).expect(name);
        assert!(surface.is_main(), "{name}");
        let leaf = w.state.leaf(NodePath { surface, node }).unwrap();
        assert_eq!(leaf.tabs, [PanelId::Canvas], "{name}");
        assert!(sound(&w.state), "{name}");
    }

    /// `w` dibujado unos frames en una app con Pixel UI, con lo que se ve en `shown`.
    fn drawn(w: Workspace, shown: Shown) -> crate::ChimpApp {
        let mut app = test_app();
        app.view.timeline = shown.timeline;
        app.view.preview = shown.preview;
        app.workspace = w;
        frames(&egui::Context::default(), &mut app);
        app
    }

    #[test]
    fn each_factory_preset_keeps_the_invariants_and_draws() {
        for name in FACTORY {
            let w = factory(name).unwrap_or_else(|| panic!("{name}"));
            let shown = factory_shown(name);
            assert_invariants(name, &w, shown);
            let app = drawn(w, shown);
            let w = &app.workspace;
            assert_invariants(name, w, shown);
            // Lo acoplado, con lugar y en puntos enteros.
            for &id in PanelId::ALL.iter().filter(|&&p| w.docked(p)) {
                let rect = w.leaf_rect(id).unwrap();
                assert!(
                    rect.is_positive() && rect.is_finite(),
                    "{name}: {id:?} {rect:?}"
                );
                assert_eq!(rect.min, rect.min.round(), "{name}: {id:?} {rect:?}");
                assert_eq!(rect.max, rect.max.round(), "{name}: {id:?} {rect:?}");
            }
        }
        assert!(factory("Nope").is_none());
    }

    /// Los rects de lo que hay en Essentials (y la columna derecha) de la app `app` ya dibujada.
    fn essentials_rects(app: &crate::ChimpApp) -> Vec<Option<egui::Rect>> {
        let w = &app.workspace;
        let mut rects: Vec<_> = [
            PanelId::Tools,
            PanelId::Canvas,
            PanelId::Timeline,
            PanelId::Color,
        ]
        .into_iter()
        .map(|id| w.leaf_rect(id))
        .collect();
        rects.push(w.column_rect());
        rects
    }

    #[test]
    fn essentials_is_todays_layout() {
        let w = factory(ESSENTIALS).unwrap();
        let today = Workspace::essentials();
        assert_eq!(w.state_tabs(), today.state_tabs());
        for edge in Edge::ALL {
            assert_eq!(w.strip_at(edge), today.strip_at(edge), "{edge:?}");
        }
        assert_eq!(
            factory_shown(ESSENTIALS),
            Shown {
                timeline: true,
                preview: false
            }
        );
        let want = essentials_rects(&drawn(today, Shown::default()));
        assert!(want.iter().all(Option::is_some), "{want:?}");
        assert_eq!(essentials_rects(&drawn(w, Shown::default())), want);
        // Dejado y vuelto a elegir (sin cambios): se ve igual.
        let mut app = drawn(Workspace::essentials(), Shown::default());
        let mut ws = Workspaces::default();
        let shown = ws.switch("Tiles", &mut app.workspace, Shown::default());
        let shown = ws.switch(ESSENTIALS, &mut app.workspace, shown.expect("Tiles"));
        assert_eq!(shown, Some(Shown::default()));
        frames(&egui::Context::default(), &mut app);
        assert_eq!(essentials_rects(&app), want);
    }

    #[test]
    fn the_factory_presets_put_their_panels_where_they_go() {
        // Animation: el Preview acoplado a la derecha del timeline, los dos debajo del lienzo; la
        // franja, ~40 % de alto del lado del lienzo.
        assert_eq!(
            factory_shown("Animation"),
            Shown {
                timeline: true,
                preview: true
            }
        );
        let app = drawn(factory("Animation").unwrap(), factory_shown("Animation"));
        let w = &app.workspace;
        assert!(w.docked(PanelId::Preview) && w.docked(PanelId::Timeline));
        let timeline = w.leaf_rect(PanelId::Timeline).unwrap();
        let preview = w.leaf_rect(PanelId::Preview).unwrap();
        let canvas = w.leaf_rect(PanelId::Canvas).unwrap();
        assert!(
            preview.left() > timeline.right(),
            "{preview:?} {timeline:?}"
        );
        assert_eq!(preview.top(), timeline.top());
        assert!(timeline.width() > preview.width());
        assert!(timeline.top() > canvas.bottom(), "{timeline:?} {canvas:?}");
        let share = timeline.height() / (timeline.bottom() - canvas.top());
        assert!((share - 0.4).abs() < 0.03, "{share}");
        assert_eq!(w.column_panels(), [PanelId::Color]);
        // Tiles: Artboards y Slices en pestañas, en la columna debajo de Color (Artboards a la
        // vista).
        let w = factory("Tiles").unwrap();
        assert_eq!(w.leaf_of(PanelId::Artboards), w.leaf_of(PanelId::Slices));
        assert_ne!(w.leaf_of(PanelId::Artboards), w.leaf_of(PanelId::Color));
        assert_eq!(
            w.column_panels(),
            [PanelId::Color, PanelId::Artboards, PanelId::Slices]
        );
        let (surface, node) = w.leaf_of(PanelId::Slices).unwrap();
        let leaf = w.state.leaf(NodePath { surface, node }).unwrap();
        assert_eq!(leaf.tabs[leaf.active.0], PanelId::Artboards);
        assert!(!w.strip().contains(&PanelId::Slices));
        // Fonts: FontForge en la columna, debajo de Color.
        let w = factory("Fonts").unwrap();
        assert_eq!(w.column_panels(), [PanelId::Color, PanelId::Font]);
        assert!(!w.strip().contains(&PanelId::Font));
        for name in ["Tiles", "Fonts"] {
            assert_eq!(factory_shown(name), Shown::default(), "{name}");
        }
    }

    /// Essentials con Groom debajo de Color y Lumen flotando.
    fn mine() -> Workspace {
        let mut w = Workspace::essentials();
        let (s, n) = w.leaf_of(PanelId::Color).unwrap();
        w.dock(PanelId::Groom, DockTarget::Split(s, n, Split::Below));
        w.float(PanelId::Lumen, egui::pos2(320.0, 120.0));
        w
    }

    #[test]
    fn a_new_workspace_comes_back_after_switching_to_another_one() {
        let mut ws = Workspaces::default();
        let mut w = mine();
        let tabs = w.state_tabs();
        let mine_shown = Shown {
            timeline: false,
            preview: false,
        };
        ws.create("Mine", &w, mine_shown).unwrap();
        assert_eq!(ws.current(), "Mine");
        assert_eq!(ws.user_names(), ["Mine"]);
        let shown = ws
            .switch("Animation", &mut w, mine_shown)
            .expect("Animation");
        assert_eq!(ws.current(), "Animation");
        assert_eq!(shown, factory_shown("Animation"));
        assert!(w.docked(PanelId::Preview));
        assert!(!w.docked(PanelId::Groom) && !w.floating(PanelId::Lumen));
        let back = ws.switch("Mine", &mut w, shown).expect("Mine");
        assert_eq!(back, mine_shown);
        assert_eq!(w.state_tabs(), tabs);
        assert!(w.docked(PanelId::Groom) && w.floating(PanelId::Lumen));
        assert_ne!(w.leaf_of(PanelId::Groom), w.leaf_of(PanelId::Color));
        // El mismo, o uno que no existe: nada.
        assert_eq!(ws.switch("Mine", &mut w, back), None);
        assert_eq!(ws.switch("Nope", &mut w, back), None);
        assert_eq!(ws.current(), "Mine");
    }

    #[test]
    fn switching_keeps_the_edits_of_the_one_left_and_reset_discards_them() {
        let mut ws = Workspaces::default();
        let mut w = Workspace::essentials();
        w.dock(PanelId::Library, DockTarget::Focused);
        let tiles = ws.switch("Tiles", &mut w, Shown::default()).unwrap();
        assert!(w.docked(PanelId::Slices) && !w.docked(PanelId::Library));
        w.to_strip(PanelId::Slices);
        let essentials = ws.switch(ESSENTIALS, &mut w, tiles).unwrap();
        assert!(w.docked(PanelId::Library), "Essentials keeps its edit");
        ws.switch("Tiles", &mut w, essentials).unwrap();
        assert!(!w.docked(PanelId::Slices), "Tiles keeps its edit");
        // Reset: el de fábrica.
        assert_eq!(ws.reset(&mut w), factory_shown("Tiles"));
        assert!(w.docked(PanelId::Slices) && w.docked(PanelId::Artboards));
        ws.switch(ESSENTIALS, &mut w, tiles).unwrap();
        assert!(w.docked(PanelId::Library));
        assert_eq!(ws.reset(&mut w), Shown::default());
        assert!(!w.docked(PanelId::Library));
        assert_eq!(w.state_tabs(), Workspace::essentials().state_tabs());
        // Uno del usuario vuelve a como se creó.
        ws.create("Mine", &mine(), Shown::default()).unwrap();
        let mut w = mine();
        w.dock(PanelId::Slices, DockTarget::Focused);
        w.to_strip(PanelId::Groom);
        ws.reset(&mut w);
        assert!(w.docked(PanelId::Groom) && !w.docked(PanelId::Slices));
        assert!(w.floating(PanelId::Lumen));
    }

    #[test]
    fn delete_removes_a_user_workspace_but_not_the_current_one_or_a_factory_one() {
        let mut ws = Workspaces::default();
        let mut w = Workspace::essentials();
        ws.create("Mine", &w, Shown::default()).unwrap();
        ws.create("Other", &w, Shown::default()).unwrap();
        assert_eq!(ws.deletable(), ["Mine"], "not the current one");
        assert!(!ws.delete("Other"), "the current one");
        ws.switch("Animation", &mut w, Shown::default()).unwrap();
        assert_eq!(ws.deletable(), ["Mine", "Other"]);
        assert!(!ws.delete("Animation"));
        assert!(!ws.delete("Nope"));
        assert!(ws.delete("Mine"));
        assert_eq!(ws.user_names(), ["Other"]);
        assert_eq!(ws.switch("Mine", &mut w, Shown::default()), None);
        assert_eq!(ws.current(), "Animation");
    }

    #[test]
    fn a_new_name_must_be_new_and_not_empty() {
        let mut ws = Workspaces::default();
        let w = Workspace::essentials();
        assert!(ws.name_problem("").is_some());
        assert!(ws.name_problem("   ").is_some());
        for name in FACTORY {
            assert!(ws.name_problem(name).is_some(), "{name}");
            assert!(ws.name_problem(&name.to_lowercase()).is_some(), "{name}");
        }
        assert_eq!(ws.name_problem("Mine"), None);
        ws.create(" Mine ", &w, Shown::default()).unwrap();
        assert_eq!(ws.current(), "Mine", "trimmed");
        assert!(ws.name_problem("mine").is_some());
        assert!(ws.create("Mine", &w, Shown::default()).is_err());
        assert!(ws.create("", &w, Shown::default()).is_err());
        assert_eq!(ws.user_names(), ["Mine"]);
    }

    #[test]
    fn the_named_workspaces_are_saved_and_read_back() {
        let mut ws = Workspaces::default();
        let mut w = mine();
        ws.create("Mine", &w, Shown::default()).unwrap();
        // Mine, cambiado y dejado; Animation, cambiado y a la vista.
        w.dock(PanelId::Slices, DockTarget::Focused);
        let shown = ws.switch("Animation", &mut w, Shown::default()).unwrap();
        w.float(PanelId::Library, egui::pos2(200.0, 200.0));
        let saved = ws.to_saved(&w);
        assert_eq!(saved.version, SAVED_VERSION);
        assert_eq!(SAVED_VERSION, 7);
        let ron = ron::to_string(&saved).unwrap();
        let (mut back_ws, mut back) = Workspaces::from_saved(ron::from_str(&ron).unwrap());
        assert_eq!(back_ws.current(), "Animation");
        assert!(back.floating(PanelId::Library) && back.docked(PanelId::Preview));
        assert_eq!(back_ws.user_names(), ["Mine"]);
        // Mine vuelve con lo cambiado; con Reset, a como se creó.
        back_ws.switch("Mine", &mut back, shown).unwrap();
        assert!(back.docked(PanelId::Slices) && back.docked(PanelId::Groom));
        back_ws.reset(&mut back);
        assert!(!back.docked(PanelId::Slices) && back.docked(PanelId::Groom));
        assert!(back.floating(PanelId::Lumen));
    }

    #[test]
    fn a_version_6_workspace_is_the_current_layout_of_essentials() {
        // Lo que guardaba la tarea 6: solo el acomodo (Groom debajo de Color), sin nombres.
        let mut saved = mine().to_saved();
        saved.version = 6;
        let ron = ron::to_string(&saved).unwrap();
        let old = ron.replacen(",current:\"\",user:[],edited:[]", "", 1);
        assert_ne!(old, ron);
        let (mut ws, mut w) = Workspaces::from_saved(ron::from_str(&old).unwrap());
        assert_eq!(ws.current(), ESSENTIALS);
        assert!(ws.user_names().is_empty());
        assert!(w.docked(PanelId::Groom) && w.floating(PanelId::Lumen));
        // Reset Essentials: el de fábrica.
        ws.reset(&mut w);
        assert!(!w.docked(PanelId::Groom) && !w.floating(PanelId::Lumen));
        assert_eq!(w.state_tabs(), Workspace::essentials().state_tabs());
        // Una más vieja (la 3: sin el timeline en el árbol): migrada y también de Essentials.
        let mut state = DockState::new(vec!["canvas".to_string()]);
        let [canvas, _] =
            state
                .main_surface_mut()
                .split_left(NodeIndex::root(), 0.03, vec!["tools".to_string()]);
        state
            .main_surface_mut()
            .split_right(canvas, 0.7, vec!["color".to_string()]);
        let old = SavedWorkspace {
            version: 3,
            state,
            strip: Vec::new(),
            left: Vec::new(),
            top: Vec::new(),
            bottom: Vec::new(),
            ..SavedWorkspace::default()
        };
        let ron = ron::to_string(&old).unwrap();
        let ron = ron.replacen(",current:\"\",user:[],edited:[]", "", 1);
        let (ws, w) = Workspaces::from_saved(ron::from_str(&ron).unwrap());
        assert_eq!(ws.current(), ESSENTIALS);
        assert!(w.timeline_below());
        assert_eq!(w.places(PanelId::Timeline), 1);
    }

    #[test]
    fn unknown_or_clashing_names_in_saved_workspaces_are_dropped() {
        let layout = || mine().to_layout(Shown::default());
        let mut saved = mine().to_saved();
        saved.current = "Gone".into();
        saved.user = vec![
            ("Essentials".into(), layout()),
            ("".into(), layout()),
            ("Mine".into(), layout()),
            ("mine".into(), layout()),
        ];
        saved.edited = vec![("Gone".into(), layout()), ("Tiles".into(), layout())];
        // Un acomodo roto: Essentials al elegirlo (sin tronar).
        let mut broken = layout();
        broken.state.main_surface_mut()[NodeIndex(2)] = egui_dock::Node::Empty;
        saved.edited.push(("Animation".into(), broken));
        let (mut ws, mut w) = Workspaces::from_saved(saved);
        assert_eq!(ws.current(), ESSENTIALS);
        assert!(w.docked(PanelId::Groom), "the layout stays");
        assert_eq!(ws.user_names(), ["Mine"]);
        ws.switch("Tiles", &mut w, Shown::default()).unwrap();
        assert!(w.docked(PanelId::Groom), "Tiles as it was left");
        ws.switch("Animation", &mut w, Shown::default()).unwrap();
        assert_eq!(w.state_tabs(), Workspace::essentials().state_tabs());
    }
}
