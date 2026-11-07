use super::*;
use crate::editor::ToolKind;
use crate::keymap::{Action, Keymap};

/// Un documento de sprite de un frame, con el lápiz, en la pestaña 0 y el artboard 1.
fn sprite() -> DocFacts {
    DocFacts {
        place: (0, 1),
        kind: DocKind::Sprite,
        frames: 1,
        onion: false,
        tool: ToolKind::Pencil,
        selection: false,
        symmetry: false,
        mirror_selection: false,
        artboards: 1,
    }
}

fn snap(doc: DocFacts) -> Snapshot {
    Snapshot {
        doc: Some(doc),
        ..Snapshot::default()
    }
}

/// Chimpy prendido, ya pasada la gracia del arranque (empezó en 0).
fn awake() -> Mascot {
    let mut m = Mascot::new(MascotSettings {
        enabled: true,
        discovered: true,
        ..MascotSettings::default()
    });
    m.observe(0.0, &snap(sprite()), &Keymap::default());
    m
}

fn shown_id(m: &Mascot) -> Option<&str> {
    m.tip().map(|t| t.id.as_str())
}

#[test]
fn off_by_default_and_settings_roundtrip() {
    let settings = MascotSettings::default();
    assert!(!settings.enabled && !settings.discovered);
    let mut seen = MascotSettings {
        enabled: true,
        discovered: true,
        ..MascotSettings::default()
    };
    seen.seen.insert("walk-cycle".into());
    let json = serde_json::to_string(&seen).unwrap();
    assert_eq!(serde_json::from_str::<MascotSettings>(&json).unwrap(), seen);
    // Lo guardado sin campos (o de una versión vieja) abre apagado.
    assert_eq!(
        serde_json::from_str::<MascotSettings>("{}").unwrap(),
        MascotSettings::default()
    );
}

#[test]
fn five_quick_pokes_wake_chimpy_and_he_says_hello() {
    let keys = Keymap::default();
    let mut m = Mascot::new(MascotSettings::default());
    for i in 0..4 {
        assert_eq!(m.poke(i as f64 * 0.3, &keys), Poke::Counting);
    }
    assert!(!m.settings.enabled);
    assert_eq!(m.poke(1.2, &keys), Poke::Woke);
    assert!(m.settings.enabled && m.settings.discovered);
    assert_eq!(shown_id(&m), Some(HELLO));
    // Otra vez apagado y despertado: ya no se presenta, vuelve.
    m.set_enabled(false);
    assert!(m.tip().is_none(), "turning him off hides the bubble");
    for i in 0..5 {
        m.poke(10.0 + i as f64 * 0.1, &keys);
    }
    assert_eq!(shown_id(&m), Some(BACK));
}

#[test]
fn slow_pokes_do_not_count() {
    let keys = Keymap::default();
    let mut m = Mascot::new(MascotSettings::default());
    for i in 0..10 {
        assert_eq!(m.poke(i as f64 * 1.0, &keys), Poke::Counting);
    }
    assert!(!m.settings.enabled);
}

#[test]
fn pokes_while_awake_ask_for_a_tip() {
    let keys = Keymap::default();
    let mut m = awake();
    for i in 0..4 {
        m.poke(1.0 + i as f64 * 0.1, &keys);
    }
    assert_eq!(m.poke(1.5, &keys), Poke::Asked);
    let id = shown_id(&m).unwrap();
    assert!(id.starts_with("dyk."), "{id}");
}

#[test]
fn nothing_happens_while_asleep() {
    let keys = Keymap::default();
    let mut m = Mascot::new(MascotSettings::default());
    m.observe(0.0, &snap(sprite()), &keys);
    let mut doc = sprite();
    doc.frames = 3;
    m.observe(500.0, &snap(doc), &keys);
    m.observe(2000.0, &snap(sprite()), &keys);
    assert!(m.tip().is_none());
}

#[test]
fn a_walk_cycle_offers_onion_skin() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.frames = 2;
    m.observe(100.0, &snap(doc.clone()), &keys);
    assert!(m.tip().is_none());
    doc.frames = 3;
    m.observe(101.0, &snap(doc.clone()), &keys);
    let tip = m.tip().expect("the walk cycle tip");
    assert_eq!(tip.id, "walk-cycle");
    assert_eq!(tip.buttons(), vec!["Yes", "No, thanks"]);
    assert_eq!(m.answer(0, 102.0), Some(Suggestion::Run(Action::OnionSkin)));
    assert!(m.settings.seen.contains("walk-cycle"));
    // Se va (sube) y no vuelve a salir.
    m.observe(110.0, &snap(doc.clone()), &keys);
    assert!(m.tip().is_none());
    doc.frames = 1;
    m.observe(500.0, &snap(doc.clone()), &keys);
    doc.frames = 4;
    m.observe(501.0, &snap(doc), &keys);
    assert_ne!(shown_id(&m), Some("walk-cycle"));
}

#[test]
fn no_onion_tip_if_onion_skin_is_already_on_or_frames_came_from_another_tab() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.onion = true;
    m.observe(100.0, &snap(doc.clone()), &keys);
    doc.frames = 5;
    m.observe(101.0, &snap(doc), &keys);
    assert!(m.tip().is_none());
    // Otra pestaña con más frames: no es que se agregaron.
    let mut other = sprite();
    other.place = (1, 1);
    other.frames = 8;
    m.observe(102.0, &snap(other), &keys);
    assert!(m.tip().is_none());
}

#[test]
fn no_tips_during_the_startup_grace() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.frames = 3;
    m.observe(10.0, &snap(doc), &keys);
    assert!(m.tip().is_none());
}

#[test]
fn automatic_tips_keep_their_distance() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.frames = 3;
    m.observe(100.0, &snap(doc.clone()), &keys);
    assert_eq!(shown_id(&m), Some("walk-cycle"));
    m.answer(1, 101.0);
    // Un minuto después, una selección con simetría: todavía no.
    doc.symmetry = true;
    m.observe(160.0, &snap(doc.clone()), &keys);
    doc.selection = true;
    m.observe(161.0, &snap(doc.clone()), &keys);
    assert!(m.tip().is_none());
    // Pasado el intervalo, la próxima vez sí.
    doc.selection = false;
    m.observe(100.0 + MIN_GAP + 1.0, &snap(doc.clone()), &keys);
    doc.selection = true;
    m.observe(100.0 + MIN_GAP + 2.0, &snap(doc), &keys);
    assert_eq!(shown_id(&m), Some("mirror"));
    assert_eq!(
        m.tip().unwrap().suggestion(0),
        Some(Suggestion::MirrorSelection)
    );
}

#[test]
fn a_busy_user_is_not_interrupted_but_the_tip_waits_a_bit() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.frames = 3;
    let busy = |doc: DocFacts| Snapshot {
        busy: true,
        ..snap(doc)
    };
    m.observe(100.0, &busy(doc.clone()), &keys);
    assert!(m.tip().is_none());
    m.observe(105.0, &snap(doc.clone()), &keys);
    assert_eq!(shown_id(&m), Some("walk-cycle"));

    // Si sigue ocupado más de `PENDING_FOR`, se pierde (y puede volver a salir otra vez).
    let mut m = awake();
    let mut doc = sprite();
    m.observe(100.0, &busy(doc.clone()), &keys);
    doc.frames = 3;
    m.observe(101.0, &busy(doc.clone()), &keys);
    m.observe(101.0 + PENDING_FOR + 1.0, &snap(doc), &keys);
    assert!(m.tip().is_none());
    assert!(!m.settings.seen.contains("walk-cycle"));
}

#[test]
fn picking_a_tool_with_the_mouse_teaches_its_shortcut() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    m.observe(100.0, &snap(doc.clone()), &keys);
    doc.tool = ToolKind::Eyedropper;
    m.observe(101.0, &snap(doc), &keys);
    let tip = m.tip().expect("a tool tip");
    assert_eq!(tip.id, "tool.eyedropper");
    assert!(tip.text.contains("press I"), "{}", tip.text);
    assert!(tip.text.contains("Alt+Click"), "{}", tip.text);
    assert_eq!(tip.buttons(), vec!["OK", "Shortcuts…"]);
    assert_eq!(
        tip.suggestion(1),
        Some(Suggestion::Run(Action::KeyboardShortcuts))
    );
    assert_eq!(tip.suggestion(0), None);
}

#[test]
fn picking_a_tool_with_its_key_says_nothing() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    m.observe(100.0, &snap(doc.clone()), &keys);
    doc.tool = ToolKind::Eraser;
    m.observe(
        101.0,
        &Snapshot {
            key_pressed: true,
            ..snap(doc)
        },
        &keys,
    );
    assert!(m.tip().is_none());
}

#[test]
fn tool_tips_follow_the_keymap() {
    let mut keys = Keymap::default();
    keys.assign(
        Action::Tool(ToolKind::Eraser),
        crate::keymap::Combo {
            shift: true,
            ..crate::keymap::Combo::new(eframe::egui::Key::E)
        },
    );
    let tip = tool_tip(ToolKind::Eraser, &keys).unwrap();
    assert!(tip.text.contains("press Shift+E"), "{}", tip.text);
    // Sin atajo, no hay consejo de esa herramienta.
    keys.clear(Action::Tool(ToolKind::Eraser));
    assert!(tool_tip(ToolKind::Eraser, &keys).is_none());
}

#[test]
fn every_tool_with_a_shortcut_has_a_tip() {
    let keys = Keymap::default();
    for tool in ToolKind::GROUPS.iter().flat_map(|g| g.iter()) {
        let tip = tool_tip(*tool, &keys).expect("a tip");
        assert!(tip.text.contains(tool.label()), "{}", tip.text);
    }
}

#[test]
fn fonts_and_models_get_their_own_tip() {
    let keys = Keymap::default();
    let mut m = awake();
    m.observe(100.0, &snap(sprite()), &keys);
    let mut font = sprite();
    font.place = (1, 4);
    font.kind = DocKind::Font;
    m.observe(101.0, &snap(font), &keys);
    assert_eq!(shown_id(&m), Some("font"));

    let mut m = awake();
    m.observe(100.0, &snap(sprite()), &keys);
    let mut model = sprite();
    model.kind = DocKind::Model;
    model.place = (0, 2);
    m.observe(101.0, &snap(model), &keys);
    assert_eq!(shown_id(&m), Some("model"));
}

#[test]
fn a_second_animated_artboard_offers_play_in_place() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut doc = sprite();
    doc.frames = 4;
    doc.onion = true;
    m.observe(100.0, &snap(doc.clone()), &keys);
    doc.artboards = 2;
    m.observe(101.0, &snap(doc), &keys);
    assert_eq!(shown_id(&m), Some("play-in-place"));
    assert_eq!(
        m.tip().unwrap().suggestion(0),
        Some(Suggestion::PlayInPlace)
    );
}

#[test]
fn did_you_know_waits_for_a_quiet_moment() {
    let keys = Keymap::default();
    let mut m = awake();
    let input = Snapshot {
        input: true,
        ..snap(sprite())
    };
    // Usándose sin parar: nada.
    let mut t = 0.0;
    while t < TRIVIA_GAP + 30.0 {
        m.observe(t, &input, &keys);
        t += 1.0;
    }
    assert!(m.tip().is_none());
    // Unos segundos quieto: el primero de la lista.
    m.observe(t + TRIVIA_IDLE + 0.5, &snap(sprite()), &keys);
    assert_eq!(shown_id(&m), Some(TRIVIA[0].id));
}

#[test]
fn at_home_nothing_shows() {
    let keys = Keymap::default();
    let mut m = awake();
    m.observe(TRIVIA_GAP * 2.0, &Snapshot::default(), &keys);
    assert!(m.tip().is_none());
}

#[test]
fn asking_goes_through_unseen_trivia_and_then_cycles() {
    let keys = Keymap::default();
    let mut m = awake();
    let mut ids = Vec::new();
    for i in 0..TRIVIA.len() + 2 {
        m.ask(10.0 + i as f64, &keys);
        ids.push(m.tip().unwrap().id.clone());
    }
    let first: Vec<&str> = TRIVIA.iter().map(|t| t.id).collect();
    assert_eq!(&ids[..TRIVIA.len()], &first[..]);
    assert_eq!(ids[TRIVIA.len()], first[0]);
    assert_eq!(ids[TRIVIA.len() + 1], first[1]);
}

#[test]
fn trivia_texts_use_the_keymap() {
    let keys = Keymap::default();
    let tip = tip_by_id("dyk.notes", &keys).unwrap();
    assert!(tip.text.contains("Press N "), "{}", tip.text);
    let tip = tip_by_id("dyk.extras", &keys).unwrap();
    assert!(tip.text.contains("Ctrl+H"), "{}", tip.text);
    let mut cleared = Keymap::default();
    cleared.clear(Action::Extras);
    assert!(tip_by_id("dyk.extras", &cleared).is_none());
}

#[test]
fn every_tip_id_can_be_forced() {
    let keys = Keymap::default();
    for t in TRIVIA {
        assert!(tip_by_id(t.id, &keys).is_some(), "{}", t.id);
    }
    for id in [
        HELLO,
        BACK,
        "walk-cycle",
        "mirror",
        "play-in-place",
        "font",
        "model",
        "tool.pencil",
        "tool.rectangular-marquee",
    ] {
        assert_eq!(tip_by_id(id, &keys).map(|t| t.id), Some(id.to_owned()));
    }
    assert!(tip_by_id("nope", &keys).is_none());
}

#[test]
fn the_bubble_leaves_on_its_own_unless_hovered() {
    let keys = Keymap::default();
    let mut m = awake();
    m.ask(100.0, &keys);
    m.observe(100.0 + SHOW_FOR - 1.0, &snap(sprite()), &keys);
    assert!(m.pose(100.0 + SHOW_FOR - 1.0).unwrap().bubble);
    // Con el puntero encima, el tiempo no corre.
    let hovered = Snapshot {
        hovered: true,
        ..snap(sprite())
    };
    m.observe(100.0 + SHOW_FOR + 30.0, &hovered, &keys);
    assert!(m.pose(100.0 + SHOW_FOR + 30.0).unwrap().bubble);
    m.observe(100.0 + SHOW_FOR + 32.0, &snap(sprite()), &keys);
    let pose = m.pose(100.0 + SHOW_FOR + 32.0).expect("leaving");
    assert!(!pose.bubble, "the bubble goes first");
    m.observe(100.0 + SHOW_FOR + 33.0, &snap(sprite()), &keys);
    assert!(m.pose(100.0 + SHOW_FOR + 33.0).is_none());
    assert!(m.tip().is_none());
}

#[test]
fn painting_sends_him_away() {
    let keys = Keymap::default();
    let mut m = awake();
    m.ask(100.0, &keys);
    m.observe(101.0, &snap(sprite()), &keys);
    let busy = Snapshot {
        busy: true,
        ..snap(sprite())
    };
    m.observe(102.0, &busy, &keys);
    assert!(!m.pose(102.0).unwrap().bubble);
}

#[test]
fn he_drops_in_talks_and_then_idles() {
    let keys = Keymap::default();
    let mut m = awake();
    m.ask(100.0, &keys);
    let start = m.pose(100.0).unwrap();
    assert_eq!(start.drop, 0.0);
    assert!(!start.bubble);
    let half = m.pose(100.0 + ENTER / 2.0).unwrap();
    assert!(half.drop > 0.3 && half.drop < 0.7, "{}", half.drop);
    let down = m.pose(100.0 + ENTER + 0.1).unwrap();
    assert_eq!(down.drop, 1.0);
    assert!(down.bubble);
    assert_eq!(down.anim, Anim::Talk);
    let later = m.pose(100.0 + ENTER + TALK_FOR + 0.5).unwrap();
    assert_eq!(later.anim, Anim::Idle);
    // Al irse sube.
    m.answer(0, 105.0);
    let up = m.pose(105.0 + LEAVE / 2.0).unwrap();
    assert!(up.drop > 0.3 && up.drop < 0.7, "{}", up.drop);
}

#[test]
fn the_demo_forces_a_bubble_without_touching_the_settings() {
    let keys = Keymap::default();
    let mut m = Mascot::new(MascotSettings::default());
    assert!(m.force("walk-cycle", 0.0, &keys));
    assert_eq!(shown_id(&m), Some("walk-cycle"));
    assert!(!m.settings.enabled);
    assert!(m.settings.seen.is_empty());
    // Y se queda: no se va solo (es para las capturas).
    m.observe(SHOW_FOR * 3.0, &snap(sprite()), &keys);
    assert_eq!(shown_id(&m), Some("walk-cycle"));
    assert!(!m.force("nope", 0.0, &keys));
}
