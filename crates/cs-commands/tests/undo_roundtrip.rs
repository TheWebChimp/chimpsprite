use cs_commands::ops::{self, LayerDrop};
use cs_commands::{Command, History, LayerProps};
use cs_core::*;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Draw {
        x: i32,
        y: i32,
        c: u8,
        layer: usize,
    },
    AddLayer {
        group: bool,
        above: usize,
    },
    RemoveLayer {
        layer: usize,
    },
    MoveLayer {
        layer: usize,
        target: usize,
        how: u8,
    },
    AddFrame,
    RemoveFrame {
        frame: usize,
    },
    MoveFrame {
        from: usize,
        to: usize,
    },
    Duplicate {
        frame: usize,
        linked: bool,
    },
    Duration {
        frame: usize,
        ms: u16,
    },
    Tag {
        from: usize,
        to: usize,
    },
    Hide {
        layer: usize,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let idx = 0..8usize;
    prop_oneof![
        4 => (-2..10i32, -2..10i32, any::<u8>(), idx.clone())
            .prop_map(|(x, y, c, layer)| Op::Draw { x, y, c, layer }),
        1 => (any::<bool>(), idx.clone()).prop_map(|(group, above)| Op::AddLayer { group, above }),
        1 => idx.clone().prop_map(|layer| Op::RemoveLayer { layer }),
        2 => (idx.clone(), idx.clone(), 0..3u8)
            .prop_map(|(layer, target, how)| Op::MoveLayer { layer, target, how }),
        1 => Just(Op::AddFrame),
        1 => idx.clone().prop_map(|frame| Op::RemoveFrame { frame }),
        1 => (idx.clone(), idx.clone()).prop_map(|(from, to)| Op::MoveFrame { from, to }),
        1 => (idx.clone(), any::<bool>()).prop_map(|(frame, linked)| Op::Duplicate { frame, linked }),
        1 => (idx.clone(), 1..500u16).prop_map(|(frame, ms)| Op::Duration { frame, ms }),
        1 => (idx.clone(), idx.clone()).prop_map(|(from, to)| Op::Tag { from, to }),
        1 => idx.prop_map(|layer| Op::Hide { layer }),
    ]
}

/// Traduce una operación abstracta a un comando válido (o casi) para el estado actual.
fn to_command(s: &Sprite, op: &Op) -> Option<Command> {
    let layer = |i: usize| s.layers[i % s.layers.len()].id;
    let frame = |i: usize| i % s.frames.len();
    Some(match *op {
        Op::Draw { x, y, c, layer: l } => Command::DrawPixels {
            layer: layer(l),
            frame: s.frames.len() - 1,
            pixels: vec![(x, y, PixelValue::Rgba(Rgba::rgb(c, c, c)))],
        },
        Op::AddLayer { group, above } => ops::new_layer(
            s,
            Some(layer(above)),
            if group {
                LayerKind::Group
            } else {
                LayerKind::Image
            },
        ),
        Op::RemoveLayer { layer: l } => ops::remove_layer_tree(s, layer(l))?,
        Op::MoveLayer {
            layer: l,
            target,
            how,
        } => {
            let t = layer(target);
            let drop = match how {
                0 => LayerDrop::Above(t),
                1 => LayerDrop::Below(t),
                _ => LayerDrop::Into(t),
            };
            ops::move_layer(s, layer(l), drop)?
        }
        Op::AddFrame => Command::AddFrame {
            index: 0,
            frame: Frame { duration_ms: 100 },
        },
        Op::RemoveFrame { frame: f } => Command::RemoveFrame { index: frame(f) },
        Op::MoveFrame { from, to } => Command::MoveFrame {
            from: frame(from),
            to: frame(to),
        },
        Op::Duplicate { frame: f, linked } => ops::duplicate_frame(s, frame(f), linked)?,
        Op::Duration { frame: f, ms } => Command::SetFrameDuration {
            index: frame(f),
            duration_ms: ms,
        },
        Op::Tag { from, to } => {
            let (a, b) = (frame(from), frame(to));
            Command::SetTags(vec![Tag {
                name: "t".into(),
                from: a.min(b),
                to: a.max(b),
                direction: AnimDirection::PingPong,
                color: Rgba::rgb(0, 0, 0),
            }])
        }
        Op::Hide { layer: l } => Command::SetLayerProps {
            id: layer(l),
            props: LayerProps {
                visible: Some(false),
                ..Default::default()
            },
        },
    })
}

proptest! {
    #[test]
    fn undo_all_restores_initial_and_redo_all_restores_final(ops in proptest::collection::vec(op(), 0..40)) {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let initial = s.clone();
        let mut h = History::new();
        for o in &ops {
            if s.layers.is_empty() {
                break;
            }
            if let Some(cmd) = to_command(&s, o) {
                let _ = h.execute(&mut s, cmd); // los errores no cambian nada
            }
            prop_assert!(s.is_valid_tree());
        }
        let fin = s.clone();
        while h.undo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &initial);
        while h.redo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &fin);
    }
}
