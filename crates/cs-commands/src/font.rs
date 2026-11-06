//! Los cambios de una fuente pixel: crearla, agregar o quitar glifos (un frame cada uno) y sus
//! anchos. Pintar un glifo es editar un cel, como siempre.

use cs_core::{
    Artboard, ArtboardId, Document, FontInfo, FontMode, Frame, GlyphInfo, Sprite, font_sprite,
};

use crate::{Command, DocCommand};

/// El artboard de una fuente nueva en `position` (con los ASCII imprimibles vacíos) y el comando
/// que lo agrega al final.
pub fn new_font_artboard(
    doc: &Document,
    name: &str,
    cell: (u32, u32),
    mode: FontMode,
    position: (i32, i32),
) -> (ArtboardId, DocCommand) {
    let font = FontInfo::new(name, cell.1, mode);
    let sprite = font_sprite(cell, mode, font.glyphs.len());
    add_font_artboard(doc, font, sprite, position)
}

/// El artboard de la fuente `font` con su `sprite` (p. ej. una importada) en `position`, con el
/// nombre de la fuente, y el comando que lo agrega al final.
pub fn add_font_artboard(
    doc: &Document,
    font: FontInfo,
    sprite: Sprite,
    position: (i32, i32),
) -> (ArtboardId, DocCommand) {
    let id = doc.next_artboard_id();
    let artboard = Artboard {
        model: None,
        id,
        name: font.name.clone(),
        position,
        sprite,
        guides: Vec::new(),
        symmetry: Default::default(),
        in_library: false,
        font: Some(font),
    };
    let cmd = DocCommand::AddArtboard {
        index: doc.artboards.len(),
        artboard: Box::new(artboard),
    };
    (id, cmd)
}

fn set_font(artboard: &Artboard, font: FontInfo) -> DocCommand {
    DocCommand::SetFont {
        id: artboard.id,
        font: Some(Box::new(font)),
    }
}

/// Agrega los glifos de `chars` que no estén (vacíos, en su lugar); `None` si no falta ninguno.
pub fn add_glyphs(artboard: &Artboard, chars: &[char]) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let mut chars: Vec<char> = chars.to_vec();
    chars.sort_unstable();
    chars.dedup();
    let mut cmds = Vec::new();
    for ch in chars {
        if let Err(index) = font.insert_index(ch) {
            font.glyphs.insert(index, GlyphInfo { ch, advance: None });
            cmds.push(DocCommand::OnArtboard {
                id: artboard.id,
                cmd: Command::AddFrame {
                    index,
                    frame: Frame { duration_ms: 100 },
                },
            });
        }
    }
    if cmds.is_empty() {
        return None;
    }
    cmds.push(set_font(artboard, font));
    Some(DocCommand::Batch(cmds))
}

/// Quita el glifo de `ch` y su frame; `None` si no está o es el último.
pub fn remove_glyph(artboard: &Artboard, ch: char) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs.len() < 2 {
        return None;
    }
    font.glyphs.remove(index);
    Some(DocCommand::Batch(vec![
        DocCommand::OnArtboard {
            id: artboard.id,
            cmd: Command::RemoveFrame { index },
        },
        set_font(artboard, font),
    ]))
}

/// El ancho del glifo de `ch` (`None`: automático); `None` si no cambia nada.
pub fn set_advance(artboard: &Artboard, ch: char, advance: Option<u32>) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs[index].advance == advance {
        return None;
    }
    font.glyphs[index].advance = advance;
    Some(set_font(artboard, font))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CmdError, History, StructureLock};
    use cs_core::{ColorMode, Document, Sprite};

    fn doc_with_font() -> (Document, ArtboardId) {
        let mut doc = Document::with_sprite("one", Sprite::new(4, 4, ColorMode::Rgba));
        let (id, cmd) = new_font_artboard(&doc, "Mine", (16, 16), FontMode::OneBit, (40, 0));
        cmd.apply(&mut doc).unwrap();
        (doc, id)
    }

    #[test]
    fn a_new_font_artboard_has_one_empty_frame_per_ascii_glyph() {
        let (doc, id) = doc_with_font();
        let a = doc.artboard(id).unwrap();
        let font = a.font.as_ref().unwrap();
        assert_eq!(a.name, "Mine");
        assert_eq!(a.position, (40, 0));
        assert_eq!(a.sprite.frames.len(), font.glyphs.len());
        assert!(a.sprite.cels.is_empty());
    }

    #[test]
    fn adding_glyphs_inserts_frames_in_order_and_undoes() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let cmd = add_glyphs(doc.artboard(id).unwrap(), &['é', 'á', 'A']).unwrap();
        let undo = cmd.apply(&mut doc).unwrap();
        let a = doc.artboard(id).unwrap();
        let font = a.font.as_ref().unwrap();
        assert_eq!(font.glyphs.len(), 96, "'A' was already there");
        assert_eq!(font.glyphs[94].ch, 'á');
        assert_eq!(font.glyphs[95].ch, 'é');
        assert_eq!(a.sprite.frames.len(), 96);
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        assert!(add_glyphs(doc.artboard(id).unwrap(), &['A']).is_none());
    }

    #[test]
    fn removing_a_glyph_removes_its_frame_and_undoes() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let undo = remove_glyph(doc.artboard(id).unwrap(), 'A')
            .unwrap()
            .apply(&mut doc)
            .unwrap();
        let a = doc.artboard(id).unwrap();
        assert_eq!(a.font.as_ref().unwrap().index_of('A'), None);
        assert_eq!(a.sprite.frames.len(), 93);
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        assert!(remove_glyph(doc.artboard(id).unwrap(), 'á').is_none());
    }

    #[test]
    fn the_font_lock_refuses_frame_and_layer_changes_but_not_glyph_ops() {
        let (mut doc, id) = doc_with_font();
        let mut history = History::new();
        let before = doc.clone();
        let on = |cmd| DocCommand::OnArtboard { id, cmd };
        for cmd in [
            on(Command::AddFrame {
                index: 0,
                frame: Frame { duration_ms: 100 },
            }),
            on(Command::MoveFrame { from: 0, to: 3 }),
            on(Command::ResizeCanvas {
                width: 4,
                height: 4,
                offset: (0, 0),
            }),
        ] {
            assert_eq!(
                StructureLock::execute(&mut history, &mut doc, cmd, false),
                Err(CmdError::StructureLocked(id))
            );
        }
        assert_eq!(doc, before);
        assert!(!history.can_undo());
        let add = add_glyphs(doc.artboard(id).unwrap(), &['á']).unwrap();
        assert!(StructureLock::execute(&mut history, &mut doc, add, false).is_ok());
        // Las de un artboard que no es fuente pasan.
        let plain = DocCommand::OnArtboard {
            id: ArtboardId(1),
            cmd: Command::AddFrame {
                index: 0,
                frame: Frame { duration_ms: 100 },
            },
        };
        assert!(StructureLock::execute(&mut history, &mut doc, plain, false).is_ok());
    }

    #[test]
    fn a_built_font_is_added_as_an_artboard_and_undoes() {
        let (mut doc, _) = doc_with_font();
        let before = doc.clone();
        let (font, sprite) = cs_core::build_font(cs_core::FontBuild {
            name: "Imported".into(),
            mode: FontMode::OneBit,
            cell: (6, 8),
            metrics: cs_core::FontMetrics::for_cell(8),
            letter_spacing: 1,
            space_width: 3,
            monospace: false,
            glyphs: vec![cs_core::GlyphBuild {
                ch: 'x',
                advance: None,
                pixels: vec![(1, 1, None)],
            }],
        });
        let (id, cmd) = add_font_artboard(&doc, font, sprite, (100, 0));
        let undo = cmd.apply(&mut doc).unwrap();
        let a = doc.artboard(id).unwrap();
        assert_eq!(a.name, "Imported");
        assert_eq!(a.font.as_ref().unwrap().glyphs.len(), 1);
        assert_eq!((a.sprite.width, a.sprite.height), (6, 8));
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
    }

    #[test]
    fn set_advance_and_set_font_roundtrip() {
        let (mut doc, id) = doc_with_font();
        let before = doc.clone();
        let undo = set_advance(doc.artboard(id).unwrap(), 'A', Some(7))
            .unwrap()
            .apply(&mut doc)
            .unwrap();
        let font = doc.artboard(id).unwrap().font.clone().unwrap();
        assert_eq!(font.glyphs[font.index_of('A').unwrap()].advance, Some(7));
        undo.apply(&mut doc).unwrap();
        assert_eq!(doc, before);
        let json = serde_json::to_string(&DocCommand::SetFont { id, font: None }).unwrap();
        assert!(serde_json::from_str::<DocCommand>(&json).is_ok());
    }
}
