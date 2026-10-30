use cs_core::*;

use super::*;
use crate::CmdError;

/// Documento con un artboard de 8×8 en el lienzo (id 1).
fn doc() -> Document {
    Document::with_sprite("host", Sprite::new(8, 8, ColorMode::Rgba))
}

fn add(doc: &mut Document, id: u32, position: (i32, i32), in_library: bool) {
    doc.artboards.push(Artboard {
        id: ArtboardId(id),
        name: format!("a{id}"),
        position,
        sprite: Sprite::new(4, 4, ColorMode::Rgba),
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library,
    });
}

#[test]
fn move_to_library_and_back_with_undo() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), false);
    let cmd = move_to_library(&d, ArtboardId(2)).unwrap();
    let undo = cmd.apply(&mut d).unwrap();
    assert!(d.artboard(ArtboardId(2)).unwrap().in_library);
    undo.apply(&mut d).unwrap();
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}

#[test]
fn the_last_canvas_artboard_cannot_leave() {
    let mut d = doc();
    add(&mut d, 2, (20, 0), true);
    assert_eq!(move_to_library(&d, ArtboardId(1)), None);
    let forced = DocCommand::SetArtboardInLibrary {
        id: ArtboardId(1),
        in_library: true,
    };
    assert_eq!(forced.apply(&mut d), Err(CmdError::LastArtboard));
    let remove = DocCommand::RemoveArtboard { id: ArtboardId(1) };
    assert_eq!(remove.apply(&mut d), Err(CmdError::LastArtboard));
    // Un símbolo sí se puede borrar.
    DocCommand::RemoveArtboard { id: ArtboardId(2) }
        .apply(&mut d)
        .unwrap();
}

#[test]
fn place_on_canvas_keeps_its_spot_or_goes_to_the_right() {
    let mut d = doc();
    add(&mut d, 2, (40, 0), true);
    let cmd = place_on_canvas(&d, ArtboardId(2)).unwrap();
    cmd.apply(&mut d).unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (40, 0));

    let mut d = doc();
    add(&mut d, 2, (2, 2), true); // choca con el host (0,0)-(8,8)
    place_on_canvas(&d, ArtboardId(2))
        .unwrap()
        .apply(&mut d)
        .unwrap();
    assert_eq!(d.artboard(ArtboardId(2)).unwrap().position, (8 + 16, 0));
    assert!(!d.artboard(ArtboardId(2)).unwrap().in_library);
}
