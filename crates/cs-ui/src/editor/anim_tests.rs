use cs_core::{Axis3, ClipMotion, ColorMode, Curve, Sprite};

use super::anim_ops::{ClipEdge, NewClip, row_clips};
use super::{CadMode, Editor, Row};

/// Un modelo nuevo (un cubo, elegido) en su workspace, en modo Anim.
fn animating() -> Editor {
    let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
    assert!(e.new_model("Box", (64, 64)));
    e.set_cad_mode(CadMode::Anim);
    e
}

fn clips(e: &Editor, row: Row) -> Vec<cs_core::Clip> {
    row_clips(e.edited_model().unwrap(), row).unwrap().clone()
}

#[test]
fn anim_mode_shows_the_time_and_the_other_modes_the_rest_pose() {
    let mut e = animating();
    assert_eq!(e.cad_mode(), CadMode::Anim);
    assert!(e.is_model(), "the model stays active");
    e.set_anim_time(700);
    assert_eq!(e.anim_time(), Some(700));
    // Más allá de la duración (2 s por defecto), se recorta.
    e.set_anim_time(9000);
    assert_eq!(e.anim_time(), Some(2000));
    e.toggle_anim_play();
    e.set_cad_mode(CadMode::Model);
    assert_eq!(e.anim_time(), None);
    assert!(!e.anim().playing, "leaving Anim pauses");
}

#[test]
fn adding_blocks_goes_to_the_active_track_at_the_current_time() {
    let mut e = animating();
    e.set_anim_time(520);
    assert!(e.add_clip(NewClip::Rotate));
    let c = clips(&e, Row::Object(0));
    assert_eq!(c.len(), 1);
    // Ajustado a 50 ms.
    assert_eq!((c[0].start_ms, c[0].duration_ms), (500, 1000));
    assert_eq!(
        c[0].motion,
        ClipMotion::Rotate {
            axis: Axis3::Y,
            turns: 250
        }
    );
    assert_eq!(
        e.selected_clip().map(|(r, i, _)| (r, i)),
        Some((Row::Object(0), 0))
    );
    // Cerca del final no cabe: dura lo que queda.
    e.set_anim_time(1500);
    assert!(e.add_clip(NewClip::Move));
    assert_eq!(clips(&e, Row::Object(0))[1].duration_ms, 500);
    // Spin dura toda la animación; Bob va y vuelve.
    assert!(e.add_clip(NewClip::Spin));
    let spin = clips(&e, Row::Object(0))[2];
    assert_eq!((spin.start_ms, spin.duration_ms), (0, 2000));
    assert!(e.add_clip(NewClip::Bob));
    let bob = clips(&e, Row::Object(0))[3];
    assert!(bob.ping_pong && bob.curve == Curve::EaseInOut);
    // Cada uno es un paso de undo.
    e.undo();
    assert_eq!(clips(&e, Row::Object(0)).len(), 3);
    // Sin pista activa no se agrega nada.
    e.set_active_row(None, false);
    assert!(!e.add_clip(NewClip::Scale));
}

#[test]
fn editing_and_deleting_the_selected_block() {
    let mut e = animating();
    e.add_clip(NewClip::Rotate);
    let (row, i, mut clip) = e.selected_clip().unwrap();
    clip.curve = Curve::Bounce;
    clip.repeat = 0;
    assert!(e.set_clip(row, i, clip));
    let saved = clips(&e, row)[0];
    assert_eq!(saved.curve, Curve::Bounce);
    assert_eq!(saved.repeat, 1, "at least once");
    assert!(e.delete_clip());
    assert!(clips(&e, row).is_empty());
    assert!(e.selected_clip().is_none());
    e.undo();
    assert_eq!(clips(&e, row).len(), 1);
    // El bloque elegido ya no existe después de deshacer el agregado.
    e.select_clip(Some((row, 0)));
    e.undo();
    e.undo();
    assert!(e.selected_clip().is_none());
    assert!(!e.delete_clip());
}

#[test]
fn dragging_a_block_snaps_stays_inside_and_is_one_undo_step() {
    let mut e = animating();
    e.set_anim_time(500);
    e.add_clip(NewClip::Rotate); // 500..1500
    let row = Row::Object(0);
    let steps = e.history.done_count();
    assert!(e.start_clip_drag(row, 0, ClipEdge::Body, 600.0));
    e.drag_clip_to(830.0, 50);
    // Vista previa: se ve movido, el documento no.
    let shown = row_clips(e.edit_model_shown().unwrap(), row).unwrap()[0];
    assert_eq!(shown.start_ms, 750);
    assert_eq!(clips(&e, row)[0].start_ms, 500);
    // No se sale de la animación.
    e.drag_clip_to(5000.0, 50);
    let shown = row_clips(e.edit_model_shown().unwrap(), row).unwrap()[0];
    assert_eq!((shown.start_ms, shown.end_ms()), (1000, 2000));
    assert!(e.end_clip_drag());
    assert_eq!(clips(&e, row)[0].start_ms, 1000);
    assert_eq!(e.history.done_count(), steps + 1);
    // Estirar el final, con el mínimo de 50 ms.
    e.start_clip_drag(row, 0, ClipEdge::End, 2000.0);
    e.drag_clip_to(0.0, 50);
    e.end_clip_drag();
    assert_eq!(clips(&e, row)[0].duration_ms, 50);
    // El inicio: el final se queda.
    e.start_clip_drag(row, 0, ClipEdge::Start, 1000.0);
    e.drag_clip_to(-400.0, 10);
    e.end_clip_drag();
    let c = clips(&e, row)[0];
    assert_eq!((c.start_ms, c.end_ms()), (0, 1050));
    // Esc cancela sin tocar nada.
    e.start_clip_drag(row, 0, ClipEdge::Body, 0.0);
    e.drag_clip_to(300.0, 50);
    assert!(e.anim_key(eframe::egui::Key::Escape, false));
    assert!(e.anim().drag.is_none());
    assert_eq!(clips(&e, row)[0].start_ms, 0);
    assert_eq!(e.edit_model_shown(), e.edited_model());
}

#[test]
fn playback_advances_loops_or_stops_at_the_end() {
    let mut e = animating();
    e.toggle_anim_play();
    assert!(e.tick_anim(10.0), "the first tick anchors the clock");
    assert_eq!(e.anim().time_ms, 0);
    assert!(e.tick_anim(10.5));
    assert_eq!(e.anim().time_ms, 500);
    // Con loop vuelve a empezar.
    assert!(e.tick_anim(12.25));
    assert_eq!(e.anim().time_ms, 250);
    // Sin loop se detiene al final.
    e.set_anim_looping(false);
    assert!(!e.tick_anim(14.0));
    assert_eq!(e.anim().time_ms, 2000);
    assert!(!e.anim().playing);
    // Reproducir al final vuelve al principio.
    e.toggle_anim_play();
    assert_eq!(e.anim().time_ms, 0);
    assert!(e.anim().playing);
    e.toggle_anim_play();
    assert!(!e.tick_anim(20.0));
}

#[test]
fn the_length_is_an_undo_step_and_clamps_the_time() {
    let mut e = animating();
    e.set_anim_time(1800);
    assert!(e.set_anim_length(1000));
    assert_eq!(e.anim_length(), 1000);
    assert_eq!(e.anim().time_ms, 1000);
    assert!(e.set_anim_length(1));
    assert_eq!(e.anim_length(), 100);
    e.undo();
    e.undo();
    assert_eq!(e.anim_length(), 2000);
}

#[test]
fn the_keys_play_delete_and_move_the_time() {
    use eframe::egui::Key;
    let mut e = animating();
    e.add_clip(NewClip::Scale);
    assert!(e.anim_key(Key::ArrowRight, false));
    assert_eq!(e.anim().time_ms, 100);
    assert!(e.anim_key(Key::ArrowLeft, true));
    assert_eq!(e.anim().time_ms, 90);
    assert!(e.anim_key(Key::Enter, false));
    assert!(e.anim().playing);
    assert!(e.anim_key(Key::Delete, false));
    assert!(clips(&e, Row::Object(0)).is_empty());
    assert!(!e.anim_key(Key::A, false));
}
