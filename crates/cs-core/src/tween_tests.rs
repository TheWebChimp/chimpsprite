use super::*;
use crate::ArtboardId;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

fn key(frame: usize, position: (i32, i32)) -> InstanceKey {
    InstanceKey::new(frame, ArtboardId(2), position)
}

fn tweened(frame: usize, position: (i32, i32)) -> InstanceKey {
    InstanceKey {
        tween: Some(Tween::default()),
        ..key(frame, position)
    }
}

fn track(keys: Vec<InstanceKey>) -> InstanceTrack {
    InstanceTrack { keys }
}

#[test]
fn classic_ease_is_flash_quadratic() {
    for t in [0.0, 0.25, 0.5, 0.9] {
        assert!(close(Ease::Classic(0).apply(t), t));
        // Ease out (frena): 2t − t².
        assert!(close(Ease::Classic(100).apply(t), 2.0 * t - t * t));
        // Ease in (acelera): t².
        assert!(close(Ease::Classic(-100).apply(t), t * t));
    }
    assert!(close(Ease::Classic(50).apply(0.5), 0.625));
    assert!(close(Ease::Classic(100).apply(1.0), 1.0));
    assert!(close(Ease::Classic(-100).apply(0.0), 0.0));
    // Fuera de rango se recorta.
    assert!(close(Ease::Classic(127).apply(0.5), 0.75));
    assert!(close(Ease::Classic(0).apply(2.0), 1.0));
}

#[test]
fn curve_ease_mixes_with_linear_by_amount() {
    let full = Ease::Curve {
        curve: Curve::EaseInOut,
        amount: 100,
    };
    let half = Ease::Curve {
        curve: Curve::EaseIn,
        amount: 50,
    };
    let none = Ease::Curve {
        curve: Curve::Bounce,
        amount: 0,
    };
    assert!(close(full.apply(0.25), Curve::EaseInOut.value(0.25)));
    assert!(close(half.apply(0.5), 0.5 + (0.125 - 0.5) * 0.5));
    assert!(close(none.apply(0.3), 0.3));
    for e in [full, half, none] {
        assert!(
            close(e.apply(0.0), 0.0) && close(e.apply(1.0), 1.0),
            "{e:?}"
        );
    }
}

#[test]
fn without_tween_the_pose_is_the_key() {
    let mut k = key(0, (3, 4));
    k.scale = (2000, 500);
    k.rotation = 90;
    k.opacity = 100;
    let t = track(vec![k, key(4, (20, 20))]);
    let pose = t.pose_at(2).unwrap();
    assert_eq!(pose.key, k);
    assert_eq!(pose.position, (3, 4));
    assert_eq!(pose.scale, (2.0, 0.5));
    assert_eq!((pose.rotation, pose.opacity), (90.0, 100));
    assert!(t.pose_at(4).unwrap().is_plain());
    assert!(track(vec![key(2, (0, 0))]).pose_at(1).is_none());
}

#[test]
fn a_tween_interpolates_every_property() {
    let mut a = tweened(0, (0, 0));
    a.opacity = 0;
    a.tint = Some(Tint {
        color: Rgba::rgb(255, 0, 0),
        amount: 200,
    });
    let mut b = key(4, (10, -6));
    b.scale = (3000, 1000);
    b.rotation = 360;
    b.opacity = 255;
    let t = track(vec![a, b]);
    let mid = t.pose_at(2).unwrap();
    assert_eq!(
        mid.key, a,
        "source, flips and timing come from the start key"
    );
    assert_eq!(mid.position, (5, -3));
    assert!(close(mid.scale.0, 2.0) && close(mid.scale.1, 1.0));
    assert!(close(mid.rotation, 180.0));
    assert_eq!(mid.opacity, 128, "127.5 rounds away from zero");
    // b sin tinte: el mismo rojo yendo a cantidad 0.
    assert_eq!(
        mid.tint,
        Some(Tint {
            color: Rgba::rgb(255, 0, 0),
            amount: 100
        })
    );
    // Un cuarto: la posición se redondea al píxel.
    let quarter = t.pose_at(1).unwrap();
    assert_eq!(quarter.position, (3, -2), "2.5 → 3 and -1.5 → -2");
    // El frame de la key final es la key final; el de inicio, la de inicio.
    assert_eq!(t.pose_at(4).unwrap(), InstancePose::of(b));
    assert_eq!(t.pose_at(0).unwrap(), InstancePose::of(a));
    assert!(t.pose_at(4).unwrap().tint.is_none());
}

#[test]
fn the_ease_shapes_the_tween() {
    let mut a = tweened(0, (0, 0));
    a.tween = Some(Tween {
        ease: Ease::Classic(100),
    });
    let t = track(vec![a, key(4, (16, 0))]);
    // p(0.5) = 0.75 con ease out completo.
    assert_eq!(t.pose_at(2).unwrap().position, (12, 0));
    let mut a = tweened(0, (0, 0));
    a.tween = Some(Tween {
        ease: Ease::Classic(-100),
    });
    let t = track(vec![a, key(4, (16, 0))]);
    assert_eq!(t.pose_at(2).unwrap().position, (4, 0));
}

#[test]
fn broken_tweens_hold() {
    // Sin key siguiente.
    let alone = track(vec![tweened(0, (0, 0))]);
    assert_eq!(alone.pose_at(3).unwrap().position, (0, 0));
    assert_eq!(
        alone.tween_span(3),
        Some(TweenSpan {
            start: 0,
            end: None,
            broken: true
        })
    );
    // La siguiente muestra otro símbolo.
    let mut other = key(4, (8, 8));
    other.source = Some(ArtboardId(3));
    let t = track(vec![tweened(0, (0, 0)), other]);
    assert_eq!(t.pose_at(2).unwrap().position, (0, 0));
    assert!(t.tween_span(2).unwrap().broken);
    // Bien armado.
    let t = track(vec![tweened(1, (0, 0)), key(5, (8, 8))]);
    assert_eq!(
        t.tween_span(4),
        Some(TweenSpan {
            start: 1,
            end: Some(5),
            broken: false
        })
    );
    assert_eq!(t.tween_span(5), None, "the end key has no tween");
    assert_eq!(t.tween_span(0), None);
}

#[test]
fn keyframe_at_splits_a_tween_through_the_same_values() {
    let mut a = tweened(0, (0, 0));
    a.rotation = 0;
    let mut b = key(6, (12, 6));
    b.rotation = 720;
    b.scale = (1600, 1000);
    let t = track(vec![a, b]);
    let before: Vec<_> = (0..=6).map(|f| t.pose_at(f).unwrap().position).collect();
    let k = t.keyframe_at(3).unwrap();
    assert_eq!(k.frame, 3);
    assert_eq!(k.position, (6, 3));
    assert_eq!((k.rotation, k.scale), (360, (1300, 1000)));
    assert_eq!(k.tween, a.tween, "the second half is still tweened");
    let mut split = t.clone();
    split.set_key(k);
    let after: Vec<_> = (0..=6)
        .map(|f| split.pose_at(f).unwrap().position)
        .collect();
    assert_eq!(before, after);
    // Sin tween: la vigente movida al frame (como siempre).
    let plain = track(vec![key(0, (1, 2))]);
    assert_eq!(plain.keyframe_at(3), Some(key(3, (1, 2))));
    // En una key, ella misma.
    assert_eq!(t.keyframe_at(6), Some(b));
    assert_eq!(track(vec![key(2, (0, 0))]).keyframe_at(1), None);
}

#[test]
fn new_fields_are_optional_in_json() {
    let plain = key(0, (1, 2));
    let json = serde_json::to_string(&plain).unwrap();
    for field in ["scale", "rotation", "tint", "tween"] {
        assert!(!json.contains(field), "{field} not written: {json}");
    }
    let old = r#"{"frame":0,"source":2,"position":[1,2],"flip_h":false,"flip_v":false,
        "opacity":255,"timing":{"Graphic":{"first_frame":0,"play":"Loop"}}}"#;
    assert_eq!(serde_json::from_str::<InstanceKey>(old).unwrap(), plain);
    let mut full = tweened(3, (4, 5));
    full.scale = (500, 2000);
    full.rotation = -45;
    full.tint = Some(Tint {
        color: Rgba::rgb(1, 2, 3),
        amount: 9,
    });
    full.tween = Some(Tween {
        ease: Ease::Curve {
            curve: Curve::Back,
            amount: 70,
        },
    });
    let json = serde_json::to_string(&full).unwrap();
    assert_eq!(serde_json::from_str::<InstanceKey>(&json).unwrap(), full);
}

#[test]
fn plain_keys_and_poses() {
    let mut k = key(0, (0, 0));
    assert!(k.is_plain());
    k.rotation = 720;
    assert!(k.is_plain(), "whole turns look the same");
    k.rotation = 90;
    assert!(!k.is_plain());
    k.rotation = 0;
    k.flip_h = true;
    assert!(!k.is_plain());
    k.flip_h = false;
    k.scale = (1000, 999);
    assert!(!k.is_plain());
    assert!(InstancePose::of(k).is_transformed());
    k.scale = (1000, 1000);
    k.tint = Some(Tint {
        color: Rgba::rgb(0, 0, 0),
        amount: 255,
    });
    assert!(k.is_plain() && InstancePose::of(k).is_plain());
}
