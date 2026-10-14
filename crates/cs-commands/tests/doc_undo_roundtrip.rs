use cs_commands::{Command, DocCommand, History};
use cs_core::*;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Op {
    Draw {
        board: usize,
        x: i32,
        y: i32,
    },
    AddArtboard {
        w: u32,
        h: u32,
    },
    RemoveArtboard {
        board: usize,
    },
    Move {
        board: usize,
        x: i32,
        y: i32,
    },
    Replace {
        board: usize,
        w: u32,
    },
    Resize {
        board: usize,
        w: u32,
        h: u32,
        dx: i32,
        dy: i32,
    },
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (0..4usize, -1..9i32, -1..9i32).prop_map(|(board, x, y)| Op::Draw { board, x, y }),
        1 => (1..9u32, 1..9u32).prop_map(|(w, h)| Op::AddArtboard { w, h }),
        1 => (0..4usize).prop_map(|board| Op::RemoveArtboard { board }),
        1 => (0..4usize, -20..20i32, -20..20i32).prop_map(|(board, x, y)| Op::Move { board, x, y }),
        1 => (0..4usize, 1..6u32).prop_map(|(board, w)| Op::Replace { board, w }),
        1 => (0..4usize, 0..9u32, 0..9u32, -3..3i32, -3..3i32)
            .prop_map(|(board, w, h, dx, dy)| Op::Resize { board, w, h, dx, dy }),
    ]
}

fn to_command(d: &Document, op: &Op) -> DocCommand {
    let pick = |i: usize| d.artboards[i % d.artboards.len()].id;
    match *op {
        Op::Draw { board, x, y } => DocCommand::OnArtboard {
            id: pick(board),
            cmd: Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(x, y, PixelValue::Rgba(Rgba::rgb(7, 7, 7)))],
            },
        },
        Op::AddArtboard { w, h } => DocCommand::AddArtboard {
            index: d.artboards.len(),
            artboard: Box::new(Artboard {
                guides: Vec::new(),
                symmetry: Default::default(),
                id: d.next_artboard_id(),
                name: "a".into(),
                position: (0, 0),
                sprite: Sprite::new(w, h, ColorMode::Rgba),
            }),
        },
        Op::RemoveArtboard { board } => DocCommand::RemoveArtboard { id: pick(board) },
        Op::Replace { board, w } => DocCommand::SetSprite {
            id: pick(board),
            sprite: Box::new(Sprite::new(
                w,
                2,
                ColorMode::Indexed {
                    transparent_index: 0,
                },
            )),
        },
        Op::Move { board, x, y } => DocCommand::SetArtboardProps {
            id: pick(board),
            name: None,
            position: Some((x, y)),
        },
        Op::Resize {
            board,
            w,
            h,
            dx,
            dy,
        } => DocCommand::OnArtboard {
            id: pick(board),
            cmd: Command::ResizeCanvas {
                width: w,
                height: h,
                offset: (dx, dy),
            },
        },
    }
}

proptest! {
    #[test]
    fn undo_all_restores_initial_document(ops in proptest::collection::vec(op(), 0..30)) {
        let mut d = Document::with_sprite("start", Sprite::new(8, 8, ColorMode::Rgba));
        let initial = d.clone();
        let mut h = History::new();
        for o in &ops {
            let cmd = to_command(&d, o);
            let _ = h.execute(&mut d, cmd);
        }
        let fin = d.clone();
        while h.undo(&mut d).unwrap() {}
        prop_assert_eq!(&d, &initial);
        while h.redo(&mut d).unwrap() {}
        prop_assert_eq!(&d, &fin);
    }
}
