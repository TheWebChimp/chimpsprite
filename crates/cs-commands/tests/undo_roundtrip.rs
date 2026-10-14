use cs_commands::{Command, History, LayerProps};
use cs_core::*;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Draw { x: i32, y: i32, c: u8 },
    AddLayer,
    AddFrame,
    RemoveFrame,
    Hide,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (-2..10i32, -2..10i32, any::<u8>()).prop_map(|(x, y, c)| Op::Draw { x, y, c }),
        1 => Just(Op::AddLayer),
        1 => Just(Op::AddFrame),
        1 => Just(Op::RemoveFrame),
        1 => Just(Op::Hide),
    ]
}

fn to_command(s: &Sprite, op: &Op) -> Command {
    let top = s.layers.last().unwrap().id;
    let last_frame = s.frames.len() - 1;
    match *op {
        Op::Draw { x, y, c } => Command::DrawPixels {
            layer: top,
            frame: last_frame,
            pixels: vec![(x, y, PixelValue::Rgba(Rgba::rgb(c, c, c)))],
        },
        Op::AddLayer => Command::AddLayer {
            index: s.layers.len(),
            layer: Layer::new_image(s.next_layer_id(), "L"),
        },
        Op::AddFrame => Command::AddFrame {
            index: 0,
            frame: Frame { duration_ms: 100 },
        },
        Op::RemoveFrame => Command::RemoveFrame { index: 0 },
        Op::Hide => Command::SetLayerProps {
            id: top,
            props: LayerProps {
                visible: Some(false),
                ..Default::default()
            },
        },
    }
}

proptest! {
    #[test]
    fn undo_all_restores_initial_and_redo_all_restores_final(ops in proptest::collection::vec(op(), 0..30)) {
        let mut s = Sprite::new(8, 8, ColorMode::Rgba);
        let initial = s.clone();
        let mut h = History::new();
        for o in &ops {
            let cmd = to_command(&s, o);
            let _ = h.execute(&mut s, cmd); // los errores (p. ej. LastFrame) no cambian nada
        }
        let fin = s.clone();
        while h.undo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &initial);
        while h.redo(&mut s).unwrap() {}
        prop_assert_eq!(&s, &fin);
    }
}
