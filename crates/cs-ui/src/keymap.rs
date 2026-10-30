//! Los atajos de teclado, en un solo registro: cada acción con sus teclas de fábrica y, encima,
//! las que eligió el usuario en Edit → Keyboard Shortcuts (se guardan con la app). Las teclas se
//! comparan con sus modificadores exactos (Ctrl+N no es Ctrl+Alt+N). Los menús, los tooltips y
//! la lista de atajos leen de acá, así muestran lo elegido.
//!
//! Lo que no se reasigna (Cut, Copy y Paste, que egui manda como eventos; Delete, Esc, las
//! flechas al mover lo seleccionado y lo del mouse) va aparte, en [`FIXED`].

use std::collections::BTreeMap;
use std::sync::Arc;

use eframe::egui::{self, Key};
use serde::{Deserialize, Serialize};

use crate::editor::ToolKind;

/// Una tecla con sus modificadores (Ctrl es Cmd en macOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Combo {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Combo {
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    const fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    const fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    const fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Como se muestra: "Ctrl+Shift+S", "Alt+N", ".".
    pub fn label(&self) -> String {
        let mut s = String::new();
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                s.push_str(name);
            }
        }
        s.push_str(key_label(self.key));
        s
    }

    /// Como se guarda: los nombres de egui ("Ctrl+Shift+Comma").
    fn stored(&self) -> String {
        let mut s = String::new();
        for (on, name) in [
            (self.ctrl, "Ctrl+"),
            (self.alt, "Alt+"),
            (self.shift, "Shift+"),
        ] {
            if on {
                s.push_str(name);
            }
        }
        s.push_str(self.key.name());
        s
    }

    fn parse(text: &str) -> Option<Self> {
        let (mods, key) = text.rsplit_once('+').map_or(("", text), |(m, k)| (m, k));
        let mut combo = Combo::new(Key::from_name(key)?);
        for m in mods.split('+').filter(|m| !m.is_empty()) {
            match m {
                "Ctrl" => combo.ctrl = true,
                "Shift" => combo.shift = true,
                "Alt" => combo.alt = true,
                _ => return None,
            }
        }
        Some(combo)
    }

    /// Si la tecla `key` con `modifiers` es esta. Los símbolos que piden Shift en algunos
    /// teclados (+, =, -, ",", ".") no miran Shift.
    pub fn matches(&self, key: Key, modifiers: egui::Modifiers) -> bool {
        let symbol = matches!(
            self.key,
            Key::Plus | Key::Equals | Key::Minus | Key::Comma | Key::Period
        );
        key == self.key
            && modifiers.command == self.ctrl
            && modifiers.alt == self.alt
            && (symbol || modifiers.shift == self.shift)
    }
}

/// El nombre de una tecla como se muestra (los símbolos con su símbolo).
fn key_label(key: Key) -> &'static str {
    match key {
        Key::Comma => ",",
        Key::Period => ".",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::Minus => "-",
        Key::Escape => "Esc",
        Key::Delete => "Del",
        _ => key.name(),
    }
}

/// Algo que se hace con un atajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    NewFile,
    Open,
    Save,
    SaveAs,
    Export,
    CloseTab,
    NextTab,
    PreviousTab,
    Undo,
    Redo,
    Fill,
    Stroke,
    FlipHorizontal,
    FlipVertical,
    Transform,
    NewBrush,
    NewArtboardFromSelection,
    Invert,
    KeyboardShortcuts,
    Preferences,
    SelectAll,
    Deselect,
    Reselect,
    Inverse,
    Properties,
    SpriteSize,
    CanvasSize,
    NewLayer,
    ClippingMask,
    LayerAbove,
    LayerBelow,
    DuplicateLayer,
    MergeDown,
    Flatten,
    ConvertToSymbol,
    PlayPause,
    NextFrame,
    PreviousFrame,
    NewFrame,
    OnionSkin,
    Extras,
    Timeline,
    FullScreen,
    ZoomIn,
    ZoomOut,
    ActualSize,
    FitToScreen,
    SwapColors,
    Tool(ToolKind),
}

/// Cuándo vale un atajo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Siempre, aun escribiendo en un campo (no choca con la edición de texto).
    Always,
    /// Si no se está escribiendo en un campo.
    NotTyping,
    /// Si no se escribe ni se está pintando.
    NotPainting,
}

impl Action {
    /// Las acciones que no son herramientas, en el orden de la lista.
    const COMMANDS: [Action; 48] = [
        Action::NewFile,
        Action::Open,
        Action::Save,
        Action::SaveAs,
        Action::Export,
        Action::CloseTab,
        Action::NextTab,
        Action::PreviousTab,
        Action::Undo,
        Action::Redo,
        Action::Fill,
        Action::Stroke,
        Action::FlipHorizontal,
        Action::FlipVertical,
        Action::Transform,
        Action::NewBrush,
        Action::NewArtboardFromSelection,
        Action::Invert,
        Action::KeyboardShortcuts,
        Action::Preferences,
        Action::SelectAll,
        Action::Deselect,
        Action::Reselect,
        Action::Inverse,
        Action::Properties,
        Action::SpriteSize,
        Action::CanvasSize,
        Action::NewLayer,
        Action::ClippingMask,
        Action::LayerAbove,
        Action::LayerBelow,
        Action::DuplicateLayer,
        Action::MergeDown,
        Action::Flatten,
        Action::ConvertToSymbol,
        Action::PlayPause,
        Action::NextFrame,
        Action::PreviousFrame,
        Action::NewFrame,
        Action::OnionSkin,
        Action::Extras,
        Action::Timeline,
        Action::FullScreen,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ActualSize,
        Action::FitToScreen,
        Action::SwapColors,
    ];

    /// Todas: las herramientas (en el orden de la barra) y después los comandos.
    pub fn all() -> Vec<Action> {
        ToolKind::GROUPS
            .iter()
            .flat_map(|g| g.iter())
            .map(|&t| Action::Tool(t))
            .chain(Action::COMMANDS)
            .collect()
    }

    /// La sección de la lista y el nombre.
    pub fn names(self) -> (&'static str, &'static str) {
        use Action::*;
        match self {
            NewFile => ("File", "New"),
            Open => ("File", "Open"),
            Save => ("File", "Save"),
            SaveAs => ("File", "Save As"),
            Export => ("File", "Export"),
            CloseTab => ("File", "Close Tab"),
            NextTab => ("File", "Next Tab"),
            PreviousTab => ("File", "Previous Tab"),
            Undo => ("Edit", "Undo"),
            Redo => ("Edit", "Redo"),
            Fill => ("Edit", "Fill"),
            Stroke => ("Edit", "Stroke"),
            FlipHorizontal => ("Edit", "Flip Horizontal"),
            FlipVertical => ("Edit", "Flip Vertical"),
            Transform => ("Edit", "Transform"),
            NewBrush => ("Edit", "New Brush"),
            NewArtboardFromSelection => ("Edit", "New Artboard From Selection"),
            Invert => ("Edit", "Invert"),
            KeyboardShortcuts => ("Edit", "Keyboard Shortcuts"),
            Preferences => ("Edit", "Preferences"),
            SelectAll => ("Select", "All"),
            Deselect => ("Select", "Deselect"),
            Reselect => ("Select", "Reselect"),
            Inverse => ("Select", "Inverse"),
            Properties => ("Sprite", "Properties"),
            SpriteSize => ("Sprite", "Sprite Size"),
            CanvasSize => ("Sprite", "Canvas Size"),
            NewLayer => ("Layer", "New Layer"),
            ClippingMask => ("Layer", "Clipping Mask"),
            LayerAbove => ("Layer", "Layer Above"),
            LayerBelow => ("Layer", "Layer Below"),
            DuplicateLayer => ("Layer", "Duplicate"),
            MergeDown => ("Layer", "Merge Down"),
            Flatten => ("Layer", "Flatten"),
            ConvertToSymbol => ("Layer", "Convert to Symbol"),
            PlayPause => ("Frame", "Play / Pause"),
            NextFrame => ("Frame", "Next Frame"),
            PreviousFrame => ("Frame", "Previous Frame"),
            NewFrame => ("Frame", "New Frame"),
            OnionSkin => ("Frame", "Onion Skin"),
            Extras => ("View", "Extras"),
            Timeline => ("View", "Timeline"),
            FullScreen => ("View", "Full Screen"),
            ZoomIn => ("View", "Zoom In"),
            ZoomOut => ("View", "Zoom Out"),
            ActualSize => ("View", "Actual Size"),
            FitToScreen => ("View", "Fit to Screen"),
            SwapColors => ("Canvas", "Swap Colors"),
            Tool(t) => ("Tools", t.label()),
        }
    }

    /// Cómo se guarda (estable aunque cambie el orden de la lista).
    fn id(self) -> String {
        let (section, name) = self.names();
        format!("{section}/{name}")
    }

    pub fn gate(self) -> Gate {
        use Action::*;
        match self {
            NewFile
            | Open
            | Save
            | SaveAs
            | Export
            | CloseTab
            | NextTab
            | PreviousTab
            | Properties
            | SpriteSize
            | ActualSize
            | FitToScreen
            | KeyboardShortcuts
            | Preferences
            | NewArtboardFromSelection => Gate::Always,
            Extras | Undo | Redo | SelectAll | Deselect | Reselect | Inverse | ClippingMask
            | Invert | Transform | NewBrush | ZoomIn | ZoomOut | Timeline | FullScreen => {
                Gate::NotTyping
            }
            _ => Gate::NotPainting,
        }
    }

    /// Si necesita un documento abierto.
    pub fn needs_document(self) -> bool {
        !matches!(
            self,
            Action::NewFile
                | Action::Open
                | Action::NextTab
                | Action::PreviousTab
                | Action::KeyboardShortcuts
                | Action::Preferences
        )
    }

    /// Las teclas de fábrica.
    pub fn defaults(self) -> Vec<Combo> {
        use Action::*;
        let k = Combo::new;
        match self {
            NewFile => vec![k(Key::N).ctrl()],
            Open => vec![k(Key::O).ctrl()],
            Save => vec![k(Key::S).ctrl()],
            SaveAs => vec![k(Key::S).ctrl().shift()],
            Export => vec![k(Key::E).ctrl()],
            CloseTab => vec![k(Key::W).ctrl()],
            NextTab => vec![k(Key::Tab).ctrl()],
            PreviousTab => vec![k(Key::Tab).ctrl().shift()],
            Undo => vec![k(Key::Z).ctrl()],
            Redo => vec![k(Key::Y).ctrl(), k(Key::Z).ctrl().shift()],
            Fill => vec![k(Key::F)],
            Stroke => vec![k(Key::S)],
            FlipHorizontal => vec![k(Key::H).shift()],
            FlipVertical => vec![k(Key::V).shift()],
            Transform => vec![k(Key::T).ctrl()],
            NewBrush => vec![k(Key::B).ctrl()],
            NewArtboardFromSelection => vec![k(Key::N).ctrl().alt()],
            Invert => vec![k(Key::I).ctrl()],
            KeyboardShortcuts => vec![k(Key::K).ctrl().alt().shift()],
            Preferences => vec![k(Key::K).ctrl()],
            SelectAll => vec![k(Key::A).ctrl()],
            Deselect => vec![k(Key::D).ctrl()],
            Reselect => vec![k(Key::D).ctrl().shift()],
            Inverse => vec![k(Key::I).ctrl().shift()],
            Properties => vec![k(Key::P).ctrl()],
            SpriteSize => vec![k(Key::I).ctrl().alt()],
            CanvasSize => vec![k(Key::C)],
            NewLayer => vec![k(Key::N).shift()],
            ClippingMask => vec![k(Key::G).ctrl().alt()],
            LayerAbove => vec![k(Key::ArrowUp)],
            LayerBelow => vec![k(Key::ArrowDown)],
            // Sin atajo de fábrica (el Ctrl+E de Aseprite es Export aquí); se asignan en
            // Keyboard Shortcuts.
            DuplicateLayer | MergeDown | Flatten => vec![],
            ConvertToSymbol => vec![k(Key::F8)],
            PlayPause => vec![k(Key::Enter)],
            NextFrame => vec![k(Key::Period), k(Key::ArrowRight)],
            PreviousFrame => vec![k(Key::Comma), k(Key::ArrowLeft)],
            NewFrame => vec![k(Key::N).alt()],
            OnionSkin => vec![k(Key::F3)],
            Extras => vec![k(Key::H).ctrl()],
            Timeline => vec![k(Key::Tab)],
            FullScreen => vec![k(Key::F11)],
            ZoomIn => vec![k(Key::Plus), k(Key::Equals)],
            ZoomOut => vec![k(Key::Minus)],
            ActualSize => vec![k(Key::Num0).ctrl()],
            FitToScreen => vec![k(Key::Num0).ctrl().shift()],
            SwapColors => vec![k(Key::X)],
            Tool(t) => Combo::parse(t.shortcut()).into_iter().collect(),
        }
    }
}

/// Lo que no se reasigna: (sección, acción, teclas).
pub const FIXED: &[(&str, &str, &str)] = &[
    ("Edit", "Cut", "Ctrl+X"),
    ("Edit", "Copy", "Ctrl+C"),
    ("Edit", "Copy Merged", "Ctrl+Shift+C"),
    ("Edit", "Paste", "Ctrl+V"),
    ("Edit", "Paste as New Artboard", "Ctrl+Shift+V"),
    ("Edit", "Delete", "Del"),
    ("Select", "Move the selection 1 px", "Arrows"),
    ("Canvas", "Cancel", "Esc"),
    ("Canvas", "Brush Size", "Ctrl+Wheel"),
    ("Canvas", "Pick Color", "Alt+Click"),
    ("Canvas", "Pick Color (Pencil)", "Right Click"),
    ("Canvas", "Pan", "Space+Drag"),
];

/// Las teclas elegidas por el usuario (encima de las de fábrica), como se guardan.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeymapSettings {
    /// Por acción, sus teclas (vacío: sin atajo).
    pub overrides: BTreeMap<String, Vec<String>>,
}

/// El registro: las teclas de cada acción.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Keymap {
    overrides: BTreeMap<Action, Vec<Combo>>,
}

impl Keymap {
    pub fn from_settings(settings: &KeymapSettings) -> Self {
        let by_id: BTreeMap<String, Action> =
            Action::all().into_iter().map(|a| (a.id(), a)).collect();
        let overrides = settings
            .overrides
            .iter()
            .filter_map(|(id, combos)| {
                let action = *by_id.get(id)?;
                Some((
                    action,
                    combos.iter().filter_map(|c| Combo::parse(c)).collect(),
                ))
            })
            .collect();
        Self { overrides }
    }

    pub fn settings(&self) -> KeymapSettings {
        KeymapSettings {
            overrides: self
                .overrides
                .iter()
                .map(|(a, combos)| (a.id(), combos.iter().map(Combo::stored).collect()))
                .collect(),
        }
    }

    /// Las teclas de `action`.
    pub fn combos(&self, action: Action) -> Vec<Combo> {
        self.overrides
            .get(&action)
            .cloned()
            .unwrap_or_else(|| action.defaults())
    }

    /// Las teclas de `action` como se muestran (la primera), o "" si no tiene.
    pub fn label(&self, action: Action) -> String {
        self.combos(action)
            .first()
            .map(Combo::label)
            .unwrap_or_default()
    }

    /// Si es distinta de la de fábrica.
    pub fn is_custom(&self, action: Action) -> bool {
        self.overrides
            .get(&action)
            .is_some_and(|c| *c != action.defaults())
    }

    /// Le pone `combo` a `action` (solo esa tecla) y se la quita a cualquier otra que la tuviera.
    pub fn assign(&mut self, action: Action, combo: Combo) {
        for other in Action::all() {
            if other != action && self.combos(other).contains(&combo) {
                let rest: Vec<Combo> = self
                    .combos(other)
                    .into_iter()
                    .filter(|c| *c != combo)
                    .collect();
                self.overrides.insert(other, rest);
            }
        }
        self.overrides.insert(action, vec![combo]);
    }

    /// Deja `action` sin atajo.
    pub fn clear(&mut self, action: Action) {
        self.overrides.insert(action, Vec::new());
    }

    /// Vuelve `action` a sus teclas de fábrica.
    pub fn reset(&mut self, action: Action) {
        self.overrides.remove(&action);
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    /// La acción de la tecla `key` con `modifiers`, si hay una.
    pub fn action_for(&self, key: Key, modifiers: egui::Modifiers) -> Option<Action> {
        Action::all()
            .into_iter()
            .find(|&a| self.combos(a).iter().any(|c| c.matches(key, modifiers)))
    }
}

fn keymap_id() -> egui::Id {
    egui::Id::new("chimpsprite_keymap")
}

/// Deja el registro de este frame en la memoria de egui (para los tooltips y los menús).
pub fn publish(ctx: &egui::Context, keymap: &Keymap) {
    ctx.data_mut(|d| d.insert_temp(keymap_id(), Arc::new(keymap.clone())));
}

/// Las teclas de `action` como se muestran, con el registro de este frame (o las de fábrica).
pub fn label(ctx: &egui::Context, action: Action) -> String {
    ctx.data(|d| d.get_temp::<Arc<Keymap>>(keymap_id()))
        .map_or_else(|| Keymap::default().label(action), |k| k.label(action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combos_round_trip_and_match_exactly() {
        for text in [
            "Ctrl+Shift+S",
            "Alt+N",
            "Ctrl+Alt+Shift+K",
            "Comma",
            "F3",
            "Ctrl+0",
        ] {
            let combo = Combo::parse(text).expect(text);
            assert_eq!(Combo::parse(&combo.stored()), Some(combo));
        }
        let ctrl_n = Combo::new(Key::N).ctrl();
        let mods = |command, alt| egui::Modifiers {
            command,
            ctrl: command,
            alt,
            ..Default::default()
        };
        assert!(ctrl_n.matches(Key::N, mods(true, false)));
        assert!(
            !ctrl_n.matches(Key::N, mods(true, true)),
            "Ctrl+Alt+N is not Ctrl+N"
        );
        assert_eq!(Combo::new(Key::Period).label(), ".");
        assert_eq!(Combo::new(Key::S).ctrl().shift().label(), "Ctrl+Shift+S");
    }

    #[test]
    fn every_action_has_its_defaults_and_no_two_share_one() {
        let keymap = Keymap::default();
        assert_eq!(keymap.label(Action::Tool(ToolKind::Pencil)), "B");
        assert_eq!(keymap.label(Action::Tool(ToolKind::Polygon)), "Shift+D");
        let mut seen: Vec<(Combo, Action)> = Vec::new();
        for action in Action::all() {
            for combo in keymap.combos(action) {
                if let Some((_, other)) = seen.iter().find(|(c, _)| *c == combo) {
                    panic!("{combo:?} is both {other:?} and {action:?}");
                }
                seen.push((combo, action));
            }
        }
    }

    #[test]
    fn assigning_takes_the_key_from_the_other_action_and_is_saved() {
        let mut keymap = Keymap::default();
        let p = Combo::new(Key::P);
        keymap.assign(Action::PlayPause, p);
        assert_eq!(
            keymap.action_for(Key::P, Default::default()),
            Some(Action::PlayPause)
        );
        assert_eq!(
            keymap.action_for(Key::Enter, Default::default()),
            None,
            "only P now"
        );
        // B era el lápiz: se lo quita.
        let b = Combo::new(Key::B);
        keymap.assign(Action::Fill, b);
        assert!(keymap.combos(Action::Tool(ToolKind::Pencil)).is_empty());
        let saved = keymap.settings();
        assert_eq!(Keymap::from_settings(&saved), keymap);
        keymap.reset_all();
        assert_eq!(keymap, Keymap::default());
    }
}
