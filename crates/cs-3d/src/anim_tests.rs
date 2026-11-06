use cs_core::{
    Axis3, Clip, ClipMotion, Curve, Group3D, Model3D, Node, Object3D, Transform3D, Vec3i,
};

use super::*;

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn close3(a: P3, b: P3) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-2)
}

#[test]
fn every_curve_starts_at_0_and_ends_at_1() {
    for c in Curve::ALL {
        assert!(close(curve_value(c, 0.0), 0.0), "{c:?}");
        assert!(close(curve_value(c, 1.0), 1.0), "{c:?}");
        // Fuera de 0..1 se recorta.
        assert!(close(curve_value(c, -1.0), 0.0), "{c:?}");
        assert!(close(curve_value(c, 2.0), 1.0), "{c:?}");
    }
}

#[test]
fn the_eases_are_monotonic_and_shaped() {
    for c in [
        Curve::Linear,
        Curve::EaseIn,
        Curve::EaseOut,
        Curve::EaseInOut,
        Curve::Bounce,
    ] {
        let mut last = 0.0;
        for i in 0..=100 {
            let v = curve_value(c, i as f32 / 100.0);
            if c != Curve::Bounce {
                assert!(v + 1e-6 >= last, "{c:?} at {i}");
            }
            assert!((0.0..=1.0 + 1e-6).contains(&v), "{c:?} at {i}: {v}");
            last = v;
        }
    }
    assert!(curve_value(Curve::EaseIn, 0.5) < 0.5);
    assert!(curve_value(Curve::EaseOut, 0.5) > 0.5);
    assert!(close(curve_value(Curve::EaseInOut, 0.5), 0.5));
    assert!(close(curve_value(Curve::Linear, 0.3), 0.3));
    assert!(close(curve_value(Curve::Step, 0.99), 0.0));
}

#[test]
fn bounce_touches_1_and_comes_back_down() {
    // El primer golpe llega a 1 en 1/2.75 y luego rebota hacia abajo.
    assert!(close(curve_value(Curve::Bounce, 1.0 / 2.75), 1.0));
    assert!(curve_value(Curve::Bounce, 0.55) < 0.9);
    // Elastic y back se pasan de 1.
    let over = |c| (1..100).any(|i| curve_value(c, i as f32 / 100.0) > 1.0);
    assert!(over(Curve::Elastic));
    assert!(over(Curve::Back));
}

fn rotate(start: u32, duration: u32, turns: i32) -> Clip {
    Clip::new(
        ClipMotion::Rotate {
            axis: Axis3::Z,
            turns,
        },
        start,
        duration,
    )
}

#[test]
fn a_clip_is_0_before_and_holds_its_last_value_after() {
    let c = rotate(100, 200, 1000);
    assert!(close(clip_progress(&c, 0.0), 0.0));
    assert!(close(clip_progress(&c, 100.0), 0.0));
    assert!(close(clip_progress(&c, 200.0), 0.5));
    assert!(close(clip_progress(&c, 300.0), 1.0));
    assert!(close(clip_progress(&c, 5000.0), 1.0));
    // Sin duración salta al empezar.
    let instant = rotate(100, 0, 1000);
    assert!(close(clip_progress(&instant, 99.0), 0.0));
    assert!(close(clip_progress(&instant, 100.0), 1.0));
}

#[test]
fn ping_pong_goes_and_comes_back_each_repeat() {
    let c = Clip {
        ping_pong: true,
        repeat: 2,
        ..rotate(0, 400, 1000)
    };
    // Dos ciclos de 200 ms: arriba en 100 y 300, abajo en 0, 200 y 400.
    assert!(close(clip_progress(&c, 50.0), 0.5));
    assert!(close(clip_progress(&c, 100.0), 1.0));
    assert!(close(clip_progress(&c, 150.0), 0.5));
    assert!(close(clip_progress(&c, 200.0), 0.0));
    assert!(close(clip_progress(&c, 300.0), 1.0));
    assert!(close(clip_progress(&c, 400.0), 0.0));
    assert!(close(clip_progress(&c, 900.0), 0.0));
    // Sin ping pong, cada repetición vuelve a empezar y el final se queda en 1.
    let r = Clip {
        repeat: 2,
        ..rotate(0, 400, 1000)
    };
    assert!(close(clip_progress(&r, 100.0), 0.5));
    assert!(close(clip_progress(&r, 300.0), 0.5));
    assert!(close(clip_progress(&r, 400.0), 1.0));
    // La curva va en cada ida.
    let s = Clip {
        curve: Curve::EaseIn,
        ..rotate(0, 100, 1000)
    };
    assert!(close(clip_progress(&s, 50.0), 0.125));
}

#[test]
fn rotations_compose_in_start_order_and_moves_add() {
    // Un cuarto de vuelta en z: x → y.
    let q = node_pose(&[rotate(0, 100, 250)], 100.0).unwrap();
    assert!(close3(q.apply([0.0; 3], [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    // Igual que la rotación del transform en z (900 décimas).
    let t = Transform3D {
        rotation: Vec3i { x: 0, y: 0, z: 900 },
        ..Transform3D::default()
    };
    assert!(close3(to_world(&t, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
    // Primero x un cuarto (empieza antes) y luego z un cuarto: y → z → z.
    let x = Clip::new(
        ClipMotion::Rotate {
            axis: Axis3::X,
            turns: 250,
        },
        0,
        100,
    );
    let both = node_pose(&[rotate(50, 100, 250), x], 1000.0).unwrap();
    assert!(close3(
        both.apply([0.0; 3], [0.0, 1.0, 0.0]),
        [0.0, 0.0, 1.0]
    ));
    // Los Move se suman con su progreso.
    let mv = |y| {
        Clip::new(
            ClipMotion::Move {
                offset: Vec3i { x: 0, y, z: 0 },
            },
            0,
            100,
        )
    };
    let p = node_pose(&[mv(16), mv(32)], 50.0).unwrap();
    assert!(close3(p.offset, [0.0, 24.0, 0.0]));
    // Scale desde el origen del nodo.
    let s = Clip::new(
        ClipMotion::Scale {
            factor: Vec3i {
                x: 3000,
                y: 1000,
                z: 1000,
            },
        },
        0,
        100,
    );
    let p = node_pose(&[s], 50.0).unwrap();
    assert!(close3(
        p.apply([10.0, 0.0, 0.0], [11.0, 1.0, 0.0]),
        [12.0, 1.0, 0.0]
    ));
    assert_eq!(node_pose(&[], 10.0), None);
}

/// Una caja en el origen y una tapa (un cubo plano) metida en el grupo "hinge" cuyo origen es la
/// bisagra, arriba atrás de la caja: (0, 8, -8). La tapa queda en su lugar con la posición
/// (0, 1, 8) dentro del grupo.
fn box_with_lid() -> Model3D {
    let obj = |name: &str, position: Vec3i| Object3D {
        name: name.into(),
        mesh: crate::primitive_mesh(&crate::Primitive::Cube),
        transform: Transform3D {
            position,
            ..Transform3D::default()
        },
        double_sided: false,
        kind: None,
        hidden: false,
        locked: false,
        anim: Vec::new(),
    };
    let mut m = Model3D::default();
    m.objects.push(obj("box", Vec3i::default()));
    m.objects.push(obj("lid", Vec3i { x: 0, y: 1, z: 8 }));
    m.groups.push(Group3D {
        name: "hinge".into(),
        transform: Transform3D {
            position: Vec3i { x: 0, y: 8, z: -8 },
            ..Transform3D::default()
        },
        hidden: false,
        locked: false,
        open: true,
        anim: vec![Clip::new(
            ClipMotion::Rotate {
                axis: Axis3::X,
                turns: -250,
            },
            0,
            1000,
        )],
    });
    m.tree = vec![
        Node::Object(0),
        Node::Group {
            group: 0,
            children: vec![Node::Object(1)],
        },
    ];
    m
}

#[test]
fn a_lid_in_a_hinge_group_turns_around_the_hinge() {
    let m = box_with_lid();
    assert_eq!(pose_at(&Model3D::default(), 0.0), None);
    let rest = pose_at(&m, 0.0).unwrap();
    // El centro de la tapa: en reposo arriba de la caja, en (0, 9, 0).
    let c = posed_to_world(&m, Some(&rest), 1, [0.0; 3]);
    assert!(close3(c, [0.0, 9.0, 0.0]), "{c:?}");
    // Al final, un cuarto de vuelta en −x alrededor de la bisagra: (0,1,8) → (0,8,−1)
    // relativo a ella.
    let open = pose_at(&m, 1000.0).unwrap();
    let c = posed_to_world(&m, Some(&open), 1, [0.0; 3]);
    assert!(close3(c, [0.0, 16.0, -9.0]), "{c:?}");
    // La bisagra no se mueve: el punto de la tapa sobre ella se queda.
    let hinge_local = [0.0, -1.0, -8.0];
    assert!(close3(
        posed_to_world(&m, Some(&open), 1, hinge_local),
        [0.0, 8.0, -8.0]
    ));
    // La caja (sin bloques) no se mueve.
    assert!(close3(
        posed_to_world(&m, Some(&open), 0, [1.0, 2.0, 3.0]),
        [1.0, 2.0, 3.0]
    ));
    // Sin pose, como siempre.
    assert!(close3(
        posed_to_world(&m, None, 1, [0.0; 3]),
        crate::object_to_world(&m, 1, [0.0; 3])
    ));
}

#[test]
fn overlapping_clips_get_their_own_lanes() {
    let clips = [rotate(0, 100, 1), rotate(50, 100, 1), rotate(100, 50, 1)];
    let (lanes, n) = clip_lanes(&clips);
    assert_eq!((lanes, n), (vec![0, 1, 0], 2));
    assert_eq!(clip_lanes(&[]), (vec![], 1));
}

#[test]
fn snapping_rounds_to_the_nearest_step() {
    assert_eq!(snap_ms(124, 50), 100);
    assert_eq!(snap_ms(125, 50), 150);
    assert_eq!(snap_ms(-30, 50), -50);
    assert_eq!(snap_ms(-20, 50), 0);
    assert_eq!(snap_ms(7, 0), 7);
}
