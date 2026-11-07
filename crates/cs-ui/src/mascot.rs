//! Chimpy, el asistente escondido (un easter egg como Clippy, el clip de Office): se despierta
//! con cinco clics al chango que corre arriba a la derecha y, de vez en cuando, baja colgado al
//! lienzo con un consejo según lo que se está haciendo. La lógica (cuándo sale, qué dice, cómo se
//! mueve) vive acá sin egui; la dibuja `mascot_view.rs`.
//!
//! No molesta: nunca con alguien ocupado (pintando, escribiendo, en un diálogo), nunca en los
//! primeros `START_GRACE` segundos, nunca dos automáticos a menos de `MIN_GAP`, cada contextual
//! una sola vez en la vida, y se va solo a los `SHOW_FOR` segundos.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::editor::ToolKind;
use crate::keymap::{Action, Keymap};

/// Su nombre (en el menú Help y en Preferences).
pub const NAME: &str = "Chimpy";
/// Clics seguidos al chango que corre que lo despiertan (o le piden un consejo), dentro de
/// `POKE_WINDOW` segundos del primero al último.
pub const POKES: usize = 5;
pub const POKE_WINDOW: f64 = 2.5;
/// Segundos desde que abre la app sin consejos automáticos.
pub const START_GRACE: f64 = 60.0;
/// Segundos mínimos entre dos consejos automáticos.
pub const MIN_GAP: f64 = 240.0;
/// Segundos desde el último consejo para un "¿Sabías que…?", y los que el usuario tiene que
/// llevar sin tocar nada.
pub const TRIVIA_GAP: f64 = 480.0;
pub const TRIVIA_IDLE: f64 = 5.0;
/// Lo que un consejo contextual espera a que el usuario se desocupe; después se pierde.
pub const PENDING_FOR: f64 = 10.0;
/// Segundos en pantalla (sin contar los que tiene el puntero encima).
pub const SHOW_FOR: f64 = 14.0;
/// Lo que tarda en bajar y en subir, y lo que habla al llegar.
pub const ENTER: f64 = 0.3;
pub const LEAVE: f64 = 0.25;
pub const TALK_FOR: f64 = 1.5;

/// El saludo de la primera vez y el de cuando se lo vuelve a despertar.
pub const HELLO: &str = "hello";
pub const BACK: &str = "back";

/// Lo que se guarda entre sesiones (`chimpsprite.mascot`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MascotSettings {
    /// Prendido: da consejos.
    pub enabled: bool,
    /// Ya se lo encontró alguna vez: Help y Preferences muestran su interruptor.
    pub discovered: bool,
    /// Los consejos que ya dio (por id): los automáticos no se repiten.
    pub seen: BTreeSet<String>,
}

/// Qué se edita en la pestaña activa.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocKind {
    Sprite,
    Font,
    Model,
}

/// Lo que se ve del documento activo en un frame.
#[derive(Clone, Debug, PartialEq)]
pub struct DocFacts {
    /// La pestaña y el artboard activo: si cambian, lo demás no se compara con lo de antes.
    pub place: (usize, u64),
    pub kind: DocKind,
    /// Frames del artboard activo.
    pub frames: usize,
    pub onion: bool,
    pub tool: ToolKind,
    pub selection: bool,
    /// Algún eje de simetría prendido.
    pub symmetry: bool,
    pub mirror_selection: bool,
    /// Artboards en el lienzo.
    pub artboards: usize,
}

/// Lo que la app le cuenta en cada frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// `None` en Home (o en una página).
    pub doc: Option<DocFacts>,
    /// Pintando o arrastrando (fuera de Chimpy), escribiendo o con un diálogo modal.
    pub busy: bool,
    /// Hubo input en este frame (para saber si el usuario está quieto).
    pub input: bool,
    /// Se apretó una tecla en este frame (la herramienta se eligió con su atajo).
    pub key_pressed: bool,
    /// El puntero está sobre Chimpy o su globo.
    pub hovered: bool,
}

/// Lo que la app hace cuando se acepta un consejo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suggestion {
    /// Lo de un atajo (Keyboard Shortcuts…).
    Run(Action),
    /// Prender Onion Skin (no es el atajo: ese lo apaga si ya estaba prendido).
    OnionSkin,
    /// Prender Mirror Selection.
    MirrorSelection,
    /// Play in Place del artboard activo.
    PlayInPlace,
}

/// Un consejo: su texto y sus botones (con lo que hace cada uno).
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub id: String,
    pub text: String,
    actions: Vec<(&'static str, Option<Suggestion>)>,
}

impl Tip {
    /// "Yes" / "No, thanks": acepta `suggestion` o no.
    fn offer(id: &str, text: impl Into<String>, suggestion: Suggestion) -> Tip {
        Tip {
            id: id.to_owned(),
            text: text.into(),
            actions: vec![("Yes", Some(suggestion)), ("No, thanks", None)],
        }
    }

    /// Solo información: "OK" (y quizá un botón que lleva a más).
    fn info(id: &str, text: impl Into<String>, more: Option<(&'static str, Suggestion)>) -> Tip {
        let mut actions = vec![("OK", None)];
        actions.extend(more.map(|(label, s)| (label, Some(s))));
        Tip {
            id: id.to_owned(),
            text: text.into(),
            actions,
        }
    }

    pub fn buttons(&self) -> Vec<&'static str> {
        self.actions.iter().map(|(label, _)| *label).collect()
    }

    /// Lo que hace el botón `index`.
    pub fn suggestion(&self, index: usize) -> Option<Suggestion> {
        self.actions.get(index).and_then(|(_, s)| *s)
    }
}

/// Un "¿Sabías que…?": `{key}` en el texto es el atajo de `key` (si la acción quedó sin atajo, el
/// consejo no sale).
pub struct Trivia {
    pub id: &'static str,
    text: &'static str,
    key: Option<Action>,
    more: Option<(&'static str, Suggestion)>,
}

const SHORTCUTS: (&str, Suggestion) = ("Shortcuts…", Suggestion::Run(Action::KeyboardShortcuts));

const fn trivia(id: &'static str, text: &'static str) -> Trivia {
    Trivia {
        id,
        text,
        key: None,
        more: None,
    }
}

/// Los "¿Sabías que…?", en el orden en que salen.
pub const TRIVIA: &[Trivia] = &[
    trivia(
        "dyk.mirror",
        "Did you know? Mirror Selection, the last button of Symmetry in the options bar, selects \
         and moves things mirrored across the axes.",
    ),
    trivia(
        "dyk.tweens",
        "Did you know? Put the same symbol on two keyframes and Create Tween (right-click in the \
         timeline): it animates position, scale, rotation and tint, like Flash.",
    ),
    trivia(
        "dyk.play-in-place",
        "Did you know? Play in Place, in an artboard's menu, plays its animation right on the \
         canvas.",
    ),
    trivia(
        "dyk.fontforge",
        "Did you know? File → New Font… opens FontForge: draw a pixel font glyph by glyph, kern \
         it and export it as a TTF.",
    ),
    trivia(
        "dyk.chimpcad",
        "Did you know? File → New 3D Model… opens ChimpCAD: model low-poly things and bake them \
         into sprites.",
    ),
    Trivia {
        id: "dyk.notes",
        text: "Did you know? Press {key} for the Note tool and stick post-its anywhere on the \
               canvas.",
        key: Some(Action::Tool(ToolKind::Note)),
        more: None,
    },
    trivia(
        "dyk.coolors",
        "Did you know? Copy a Coolors palette link and press Ctrl+V: it opens in Palette \
         Presets.",
    ),
    trivia(
        "dyk.tiled",
        "Did you know? View → Tiled Mode repeats the artboard around itself, for seamless \
         textures.",
    ),
    Trivia {
        id: "dyk.extras",
        text: "Did you know? {key} hides the grid, guides and other extras in one go.",
        key: Some(Action::Extras),
        more: None,
    },
    trivia(
        "dyk.expand",
        "Did you know? An artboard's menu can Expand to Spritesheet or Slice into Frames.",
    ),
    trivia(
        "dyk.groom",
        "Did you know? Groom, in the dock, turns blurry fake pixel art (like AI images) into \
         real pixel art.",
    ),
    trivia(
        "dyk.lumen",
        "Did you know? Lumen, in the dock, makes normal maps for your sprite and tries them \
         with a light.",
    ),
    trivia(
        "dyk.pan",
        "Did you know? Hold Space and drag to pan, whatever the tool.",
    ),
    trivia(
        "dyk.brush",
        "Did you know? Ctrl+Wheel over the canvas changes the brush size.",
    ),
    Trivia {
        id: "dyk.shortcuts",
        text: "Did you know? Every shortcut can be changed in Edit → Keyboard Shortcuts…",
        key: None,
        more: Some(SHORTCUTS),
    },
];

impl Trivia {
    fn tip(&self, keys: &Keymap) -> Option<Tip> {
        let text = match self.key {
            Some(action) => {
                let key = keys.label(action);
                if key.is_empty() {
                    return None;
                }
                self.text.replace("{key}", &key)
            }
            None => self.text.to_owned(),
        };
        Some(Tip::info(self.id, text, self.more))
    }
}

/// El id del consejo de `tool`: "tool." y su nombre en minúsculas con guiones.
fn tool_id(tool: ToolKind) -> String {
    format!("tool.{}", tool.label().to_lowercase().replace(' ', "-"))
}

/// El truco de una herramienta (de los atajos fijos de la app), si tiene.
fn tool_trick(tool: ToolKind) -> Option<&'static str> {
    use ToolKind::*;
    Some(match tool {
        RectSelect | EllipseSelect | Lasso | PolygonLasso | MagicWand => {
            "The arrows move the selection 1 px."
        }
        Hand => "With any tool, hold Space and drag to pan.",
        Eyedropper => "With any other tool, Alt+Click picks a color too.",
        Pencil | Spray | Eraser => "Ctrl+Wheel over the canvas changes the brush size.",
        Note => "Edit → New Note puts one in the middle of the view.",
        _ => return None,
    })
}

/// El consejo de haber elegido `tool` con el mouse: su atajo (el del keymap) y su truco.
/// `None` si la herramienta no tiene atajo.
pub fn tool_tip(tool: ToolKind, keys: &Keymap) -> Option<Tip> {
    let key = keys.label(Action::Tool(tool));
    if key.is_empty() {
        return None;
    }
    let mut text = format!("Tip: press {key} to pick the {} tool.", tool.label());
    if let Some(trick) = tool_trick(tool) {
        text.push(' ');
        text.push_str(trick);
    }
    Some(Tip::info(&tool_id(tool), text, Some(SHORTCUTS)))
}

/// El consejo `id` (cualquiera: contextual, de herramienta, "¿Sabías que…?" o saludo).
pub fn tip_by_id(id: &str, keys: &Keymap) -> Option<Tip> {
    let tip = match id {
        HELLO => Tip::info(
            HELLO,
            "Hi! I'm Chimpy, your assistant. Looks like you found me! I'll drop a tip now and \
             then. Click me for another one; turn me off in Help → Chimpy.",
            None,
        ),
        BACK => Tip::info(BACK, "I'm back! Click me any time for a tip.", None),
        "walk-cycle" => Tip::offer(
            id,
            "It looks like you're drawing a walk cycle! Want Onion Skin, to see the frames \
             around this one?",
            Suggestion::OnionSkin,
        ),
        "mirror" => Tip::offer(
            id,
            "Symmetry is on. Want Mirror Selection? What you select and move is mirrored across \
             the axes.",
            Suggestion::MirrorSelection,
        ),
        "play-in-place" => Tip::offer(
            id,
            "Two artboards! Want this one to play its animation right on the canvas? (Play in \
             Place)",
            Suggestion::PlayInPlace,
        ),
        "font" => Tip::info(
            id,
            "A pixel font! In FontForge you can kern pairs and make Bold, Italic, Outline and \
             Shadow versions with New Variant…",
            None,
        ),
        "model" => Tip::info(
            id,
            "Building in ChimpCAD? In Model mode, Ctrl+E extrudes and Ctrl+D subdivides.",
            None,
        ),
        _ => {
            if let Some(trivia) = TRIVIA.iter().find(|t| t.id == id) {
                return trivia.tip(keys);
            }
            let tool = ToolKind::GROUPS
                .iter()
                .flat_map(|g| g.iter())
                .find(|&&t| tool_id(t) == id)?;
            return tool_tip(*tool, keys);
        }
    };
    Some(tip)
}

/// Los consejos que pide el cambio de `prev` a `now` (del más importante al menos).
fn triggers(prev: Option<&DocFacts>, now: &DocFacts, key_pressed: bool, keys: &Keymap) -> Vec<Tip> {
    let same = prev.filter(|p| p.place == now.place);
    let mut ids: Vec<String> = Vec::new();
    match same {
        // Otro documento o artboard: si es una fuente o un modelo, lo suyo.
        None => match now.kind {
            DocKind::Font => ids.push("font".into()),
            DocKind::Model => ids.push("model".into()),
            DocKind::Sprite => {}
        },
        Some(prev) => {
            if prev.frames <= 2 && now.frames >= 3 && !now.onion {
                ids.push("walk-cycle".into());
            }
            if !prev.selection && now.selection && now.symmetry && !now.mirror_selection {
                ids.push("mirror".into());
            }
            if prev.artboards < 2 && now.artboards >= 2 && now.frames > 1 {
                ids.push("play-in-place".into());
            }
            if prev.tool != now.tool && !key_pressed {
                ids.push(tool_id(now.tool));
            }
        }
    }
    ids.iter().filter_map(|id| tip_by_id(id, keys)).collect()
}

/// El consejo en pantalla.
#[derive(Clone, Debug)]
struct Shown {
    tip: Tip,
    /// Cuándo empezó a bajar.
    since: f64,
    /// Segundos que lleva a la vista sin el puntero encima, y hasta cuándo se contaron.
    elapsed: f64,
    counted: f64,
    /// Cuándo empezó a irse.
    leaving: Option<f64>,
    /// Del demo: no se va solo.
    pinned: bool,
}

/// Qué animación hace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anim {
    Talk,
    Idle,
}

/// Cómo se ve en este momento.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Cuánto bajó: 0 escondido arriba, 1 colgando a la vista.
    pub drop: f32,
    /// El globo a la vista (no mientras baja ni mientras se va).
    pub bubble: bool,
    pub anim: Anim,
    /// Segundos desde que llegó (para los frames de la animación).
    pub clock: f64,
}

/// Lo que pasó con un clic al chango que corre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Poke {
    /// Todavía no son cinco seguidos.
    Counting,
    /// Estaba dormido y se despertó.
    Woke,
    /// Estaba despierto: dio un consejo.
    Asked,
}

/// Chimpy: lo guardado y lo de esta sesión.
#[derive(Clone, Debug, Default)]
pub struct Mascot {
    pub settings: MascotSettings,
    shown: Option<Shown>,
    /// El primer frame visto (la gracia del arranque cuenta desde acá).
    started: Option<f64>,
    /// Cuándo salió el último consejo (automático o pedido).
    last_tip: Option<f64>,
    last_input: f64,
    prev: Option<DocFacts>,
    /// El contextual que espera a que el usuario se desocupe, y desde cuándo.
    pending: Option<(Tip, f64)>,
    pokes: Vec<f64>,
    /// Por dónde van los pedidos cuando ya se vieron todos los "¿Sabías que…?".
    cycle: usize,
}

impl Mascot {
    pub fn new(settings: MascotSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }

    /// El consejo en pantalla (también mientras se va).
    pub fn tip(&self) -> Option<&Tip> {
        self.shown.as_ref().map(|s| &s.tip)
    }

    /// Prende o apaga (Help → Chimpy, Preferences). Apagado se va al momento.
    pub fn set_enabled(&mut self, on: bool) {
        self.settings.enabled = on;
        if on {
            self.settings.discovered = true;
        } else {
            self.shown = None;
            self.pending = None;
        }
    }

    /// Un clic al chango que corre.
    pub fn poke(&mut self, now: f64, keys: &Keymap) -> Poke {
        self.pokes.retain(|&t| now - t <= POKE_WINDOW);
        self.pokes.push(now);
        if self.pokes.len() < POKES {
            return Poke::Counting;
        }
        self.pokes.clear();
        if self.settings.enabled {
            self.ask(now, keys);
            return Poke::Asked;
        }
        let first = !self.settings.discovered;
        self.set_enabled(true);
        let id = if first { HELLO } else { BACK };
        if let Some(tip) = tip_by_id(id, keys) {
            self.show(tip, now, false);
        }
        Poke::Woke
    }

    /// Un consejo ya (cinco clics o clic en Chimpy): el próximo "¿Sabías que…?" que no se vio o,
    /// si ya se vieron todos, uno tras otro.
    pub fn ask(&mut self, now: f64, keys: &Keymap) {
        let tips: Vec<Tip> = TRIVIA.iter().filter_map(|t| t.tip(keys)).collect();
        let tip = match tips.iter().find(|t| !self.settings.seen.contains(&t.id)) {
            Some(tip) => tip.clone(),
            None if tips.is_empty() => return,
            None => {
                let tip = tips[self.cycle % tips.len()].clone();
                self.cycle += 1;
                tip
            }
        };
        self.show(tip, now, false);
    }

    /// Desarrollo: muestra el consejo `id` y lo deja (no se va solo ni toca lo guardado).
    /// `false` si no existe.
    pub fn force(&mut self, id: &str, now: f64, keys: &Keymap) -> bool {
        let Some(tip) = tip_by_id(id, keys) else {
            return false;
        };
        self.show(tip, now, true);
        true
    }

    fn show(&mut self, tip: Tip, now: f64, pinned: bool) {
        if !pinned {
            self.settings.seen.insert(tip.id.clone());
            self.last_tip = Some(now);
        }
        // Si ya estaba colgado, cambia de consejo sin volver a bajar (y vuelve a hablar).
        let down = self
            .shown
            .as_ref()
            .is_some_and(|s| s.leaving.is_none() && now - s.since >= ENTER);
        self.shown = Some(Shown {
            tip,
            since: if down { now - ENTER } else { now },
            elapsed: 0.0,
            counted: now,
            leaving: None,
            pinned,
        });
    }

    /// Se tocó el botón `index` del globo: se va y devuelve lo que hay que hacer.
    pub fn answer(&mut self, index: usize, now: f64) -> Option<Suggestion> {
        let shown = self.shown.as_mut()?;
        if shown.leaving.is_some() {
            return None;
        }
        shown.leaving = Some(now);
        shown.tip.suggestion(index)
    }

    /// Lo de este frame: envejece el globo y, si toca, saca un consejo.
    pub fn observe(&mut self, now: f64, snap: &Snapshot, keys: &Keymap) {
        let started = *self.started.get_or_insert(now);
        if snap.input {
            self.last_input = now;
        }
        self.age(now, snap);
        let prev = std::mem::replace(&mut self.prev, snap.doc.clone());
        if !self.settings.enabled {
            self.pending = None;
            return;
        }
        if let Some(doc) = &snap.doc
            && let Some(tip) = triggers(prev.as_ref(), doc, snap.key_pressed, keys)
                .into_iter()
                .find(|t| !self.settings.seen.contains(&t.id))
        {
            self.pending = Some((tip, now));
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, at)| now - at > PENDING_FOR)
        {
            self.pending = None;
        }
        let free = self.shown.is_none() && snap.doc.is_some() && !snap.busy;
        let rested =
            now - started >= START_GRACE && self.last_tip.is_none_or(|t| now - t >= MIN_GAP);
        if !free || !rested {
            return;
        }
        if let Some((tip, _)) = self.pending.take() {
            self.show(tip, now, false);
            return;
        }
        let since = self.last_tip.unwrap_or(started);
        if now - since >= TRIVIA_GAP
            && now - self.last_input >= TRIVIA_IDLE
            && let Some(tip) = TRIVIA
                .iter()
                .filter_map(|t| t.tip(keys))
                .find(|t| !self.settings.seen.contains(&t.id))
        {
            self.show(tip, now, false);
        }
    }

    /// El tiempo del globo: se va a los `SHOW_FOR` segundos a la vista (sin el puntero encima)
    /// o cuando el usuario se pone a pintar; termina de irse a los `LEAVE`.
    fn age(&mut self, now: f64, snap: &Snapshot) {
        let Some(shown) = &mut self.shown else {
            return;
        };
        match shown.leaving {
            Some(at) => {
                if now - at >= LEAVE {
                    self.shown = None;
                }
            }
            None if shown.pinned => {}
            None => {
                if !snap.hovered {
                    shown.elapsed += (now - shown.counted).max(0.0);
                }
                shown.counted = now;
                if shown.elapsed >= SHOW_FOR || snap.busy {
                    shown.leaving = Some(now);
                }
            }
        }
    }

    /// Cómo se ve en `now` (`None` escondido).
    pub fn pose(&self, now: f64) -> Option<Pose> {
        let shown = self.shown.as_ref()?;
        let clock = (now - shown.since - ENTER).max(0.0);
        let anim = if clock < TALK_FOR {
            Anim::Talk
        } else {
            Anim::Idle
        };
        let (drop, bubble) = match shown.leaving {
            Some(at) => (1.0 - ((now - at) / LEAVE).clamp(0.0, 1.0), false),
            None => {
                let t = ((now - shown.since) / ENTER).clamp(0.0, 1.0);
                (t, t >= 1.0)
            }
        };
        Some(Pose {
            drop: drop as f32,
            bubble,
            anim,
            clock,
        })
    }
}

/// El arte de Chimpy (provisional, para redibujar): `assets/ui/mascot.aseprite`, colgando de la
/// mano derecha, con los tags `idle` y `talk`.
pub const ART: &[u8] = include_bytes!("../../../assets/ui/mascot.aseprite");

/// Los frames del arte ya compuestos y los de cada animación (sin egui).
#[derive(Clone, Debug, PartialEq)]
pub struct Art {
    pub size: [usize; 2],
    pub frames: Vec<Vec<cs_core::Rgba>>,
    pub durations_ms: Vec<u32>,
    pub idle: std::ops::Range<usize>,
    pub talk: std::ops::Range<usize>,
}

impl Art {
    /// Lee el arte de `bytes` (un `.aseprite`).
    pub fn decode(bytes: &[u8]) -> Option<Art> {
        let (sprite, _) = cs_io::aseprite::read(bytes).ok()?;
        let frames = (0..sprite.frames.len())
            .map(|i| cs_render::render_frame(&sprite, i))
            .collect();
        let durations = sprite
            .frames
            .iter()
            .map(|f| u32::from(f.duration_ms).max(1))
            .collect();
        let tags: Vec<(&str, std::ops::Range<usize>)> = sprite
            .tags
            .iter()
            .map(|t| (t.name.as_str(), t.from..t.to + 1))
            .collect();
        Some(Self::from_parts(
            [sprite.width as usize, sprite.height as usize],
            frames,
            durations,
            &tags,
        ))
    }

    /// Con los tags `idle` y `talk` (por nombre); el que falte (o se salga) usa todos los frames.
    pub fn from_parts(
        size: [usize; 2],
        frames: Vec<Vec<cs_core::Rgba>>,
        durations_ms: Vec<u32>,
        tags: &[(&str, std::ops::Range<usize>)],
    ) -> Art {
        let all = 0..frames.len();
        let tag = |name: &str| {
            tags.iter()
                .find(|(n, r)| *n == name && !r.is_empty() && r.end <= frames.len())
                .map_or(all.clone(), |(_, r)| r.clone())
        };
        let (idle, talk) = (tag("idle"), tag("talk"));
        Art {
            size,
            frames,
            durations_ms,
            idle,
            talk,
        }
    }

    /// El frame de `anim` a los `clock` segundos (en bucle, con las duraciones del arte).
    pub fn frame(&self, anim: Anim, clock: f64) -> usize {
        let range = match anim {
            Anim::Idle => self.idle.clone(),
            Anim::Talk => self.talk.clone(),
        };
        crate::tab_bar::animation_frame(&self.durations_ms[range.clone()], clock)
            .map_or(range.start, |(i, _)| range.start + i)
    }
}

#[cfg(test)]
#[path = "mascot_tests.rs"]
mod tests;
