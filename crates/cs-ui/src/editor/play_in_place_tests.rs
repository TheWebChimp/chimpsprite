use cs_core::{AnimDirection, ColorMode, Frame, Rgba, Sprite, Tag};

use super::Editor;

/// Un sprite de 4 frames de 100, 200, 100 y 100 ms.
fn animated() -> Sprite {
    let mut s = Sprite::new(4, 4, ColorMode::Rgba);
    s.frames = [100, 200, 100, 100]
        .into_iter()
        .map(|duration_ms| Frame { duration_ms })
        .collect();
    s
}

/// Dos artboards animados: "A" (el 1) y "B" (el nuevo, activo).
fn two() -> (Editor, cs_core::ArtboardId, cs_core::ArtboardId) {
    let mut e = Editor::with_sprite(animated());
    let a = e.active;
    assert!(e.add_artboard_with_sprite("B".into(), animated()));
    let b = e.active;
    assert_ne!(a, b);
    (e, a, b)
}

#[test]
fn a_non_active_artboard_plays_in_place_with_its_durations_and_the_active_one_keeps_its_frame() {
    let (mut e, a, b) = two();
    e.set_frame(2);
    let (revision, version, undo) = (e.revision, e.doc_version, e.history.can_undo());
    assert!(e.toggle_play_in_place(a), "turned on without activating it");
    assert_eq!(e.active, b);
    assert!(e.is_playing_in_place(a));
    assert!(e.any_playing_in_place());
    // Frame 0 dura 100: a los 99 ms sigue ahí; faltaba 1 ms.
    assert_eq!(e.tick_play_in_place(99.0), (Some(1.0), false));
    assert_eq!(e.frame_of(a), 0);
    let (next, changed) = e.tick_play_in_place(1.0);
    assert!(changed);
    assert_eq!(e.frame_of(a), 1);
    assert_eq!(next, Some(200.0), "frame 1 lasts 200 ms");
    e.tick_play_in_place(200.0 + 100.0 + 100.0);
    assert_eq!(e.frame_of(a), 0, "and it loops");
    // El activo no se movió y nada pasó por el documento.
    assert_eq!((e.active, e.frame, e.frame_of(b)), (b, 2, 2));
    assert_eq!(e.doc_version, version);
    assert_eq!(e.history.can_undo(), undo);
    assert_eq!(
        e.revision, revision,
        "the canvas re-renders by frame, not by revision"
    );
}

#[test]
fn the_active_artboard_pauses_its_play_in_place_and_keeps_its_edit_frame() {
    let (mut e, a, b) = two();
    e.toggle_play_in_place(a);
    e.tick_play_in_place(100.0);
    assert_eq!(e.frame_of(a), 1);
    // Activarlo: se edita su frame de edición (el 0 recordado), no el de la reproducción.
    e.activate(a);
    assert_eq!(e.frame, 0);
    assert!(e.is_playing_in_place(a), "it stays on, paused");
    assert!(!e.any_playing_in_place());
    assert_eq!(e.tick_play_in_place(500.0), (None, false));
    // Al volver a B, sigue desde donde iba.
    e.activate(b);
    assert_eq!(e.frame_of(a), 1);
    assert_eq!(e.edit_frame_of(a), 0);
    assert!(!e.toggle_play_in_place(a), "turned off");
    assert_eq!(e.frame_of(a), 0, "back to its edit frame");
}

#[test]
fn it_plays_the_tag_of_the_edit_frame_in_its_direction_and_always_loops() {
    let mut sprite = animated();
    sprite.tags.push(Tag {
        name: "walk".into(),
        from: 1,
        to: 3,
        direction: AnimDirection::PingPong,
        color: Rgba::rgb(0, 0, 0),
        repeat: 1,
    });
    let mut e = Editor::with_sprite(sprite);
    let a = e.active;
    e.set_frame(1);
    e.add_artboard_with_sprite("B".into(), animated());
    e.toggle_play_in_place(a);
    let mut seen = vec![e.frame_of(a)];
    for _ in 0..6 {
        // Lo que dura el frame de ahora.
        let ms = [100.0, 200.0, 100.0, 100.0][e.frame_of(a)];
        e.tick_play_in_place(ms);
        seen.push(e.frame_of(a));
    }
    assert_eq!(
        seen,
        vec![1, 2, 3, 2, 1, 2, 3],
        "ping-pong, past its one repeat"
    );
}

#[test]
fn fonts_models_and_single_frames_do_not_play_in_place() {
    let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
    let single = e.active;
    assert!(!e.can_play_in_place(single));
    assert!(!e.toggle_play_in_place(single));
    assert!(e.new_model("Box", (32, 32)));
    let model = e.active;
    assert!(!e.can_play_in_place(model));
}

#[test]
fn deleting_the_artboard_or_its_frames_is_handled_on_the_next_tick() {
    let (mut e, a, b) = two();
    e.toggle_play_in_place(a);
    e.tick_play_in_place(100.0 + 200.0 + 100.0);
    assert_eq!(e.frame_of(a), 3);
    // Le quitan dos frames (desde A, que queda activo un momento) y se vuelve a B.
    e.activate(a);
    e.set_frame(3);
    e.remove_frame();
    e.set_frame(2);
    e.remove_frame();
    e.activate(b);
    assert_eq!(e.frame_of(a), 1, "clamped right away");
    e.tick_play_in_place(0.0);
    e.tick_play_in_place(200.0);
    assert_eq!(e.frame_of(a), 0, "the range is now the two frames left");
    // Con un solo frame ya no se anima.
    e.activate(a);
    e.remove_frame();
    e.activate(b);
    e.tick_play_in_place(0.0);
    assert!(!e.is_playing_in_place(a));
    // Borrado: se descarta.
    let (mut e, a, _) = two();
    e.toggle_play_in_place(a);
    e.activate(a);
    e.remove_active_artboard();
    e.tick_play_in_place(0.0);
    assert!(!e.is_playing_in_place(a));
    assert_eq!(e.tick_play_in_place(100.0), (None, false));
}

#[test]
fn a_frame_longer_than_a_second_still_advances_in_small_steps() {
    let mut sprite = animated();
    sprite.frames[0].duration_ms = 2500;
    let mut e = Editor::with_sprite(sprite);
    let a = e.active;
    e.add_artboard_with_sprite("B".into(), animated());
    e.toggle_play_in_place(a);
    // 156 pasos de 16 ms = 2496 ms: todavía en el 0; faltan 4.
    for _ in 0..156 {
        e.tick_play_in_place(16.0);
    }
    assert_eq!(e.frame_of(a), 0);
    assert_eq!(e.tick_play_in_place(0.0).0, Some(4.0));
    e.tick_play_in_place(16.0);
    assert_eq!(e.frame_of(a), 1);
    // Una pausa enorme no gira sin fin: queda en algún frame, empezándolo.
    let (next, _) = e.tick_play_in_place(1.0e9);
    let ms = [2500.0, 200.0, 100.0, 100.0][e.frame_of(a)];
    assert_eq!(next, Some(ms));
}
