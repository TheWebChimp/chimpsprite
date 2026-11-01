//! Los cambios de una fuente pixel: crearla, agregar o quitar glifos (un frame cada uno) y sus
//! anchos. Pintar un glifo es editar un cel, como siempre.

use cs_core::{Artboard, ArtboardId, Document, FontInfo, FontMode, Frame, GlyphInfo, font_sprite};

use crate::{CmdError, Command, DocCommand, History};

/// El artboard de una fuente nueva en `position` (con los ASCII imprimibles vacíos) y el comando
/// que lo agrega al final.
pub fn new_font_artboard(
    doc: &Document,
    name: &str,
    cell: (u32, u32),
    mode: FontMode,
    position: (i32, i32),
) -> (ArtboardId, DocCommand) {
    let id = doc.next_artboard_id();
    let font = FontInfo::new(name, cell.1, mode);
    let artboard = Artboard {
        id,
        name: name.to_owned(),
        position,
        sprite: font_sprite(cell, mode, font.glyphs.len()),
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

/// Lo que no puede cambiar de las fuentes de un documento con los comandos de siempre: sus
/// frames y capas (solo con los de glifos, que traen su `SetFont`), su modo de color y su celda.
/// Se toma antes de un comando y se comprueba después ([`FontLock::execute`]).
#[derive(Clone, Debug, Default)]
pub struct FontLock(Vec<(ArtboardId, (u32, u32))>);

impl FontLock {
    /// Las fuentes bien armadas de `doc` con su celda (una mal armada no se traba).
    pub fn of(doc: &Document) -> FontLock {
        FontLock(
            doc.artboards
                .iter()
                .filter(|a| a.font.as_ref().is_some_and(|f| f.check(&a.sprite).is_ok()))
                .map(|a| (a.id, (a.sprite.width, a.sprite.height)))
                .collect(),
        )
    }

    fn locked(&self, id: ArtboardId) -> bool {
        self.0.iter().any(|(a, _)| *a == id)
    }

    /// Antes de aplicar: `cmd` no agrega, quita ni mueve frames o capas de una fuente, salvo
    /// que traiga también la fuente nueva de ese artboard (las operaciones de glifos).
    pub fn allows(&self, cmd: &DocCommand) -> Result<(), CmdError> {
        let mut with_font = Vec::new();
        sets_font(cmd, &mut with_font);
        self.allows_in(cmd, &with_font)
    }

    fn allows_in(&self, cmd: &DocCommand, with_font: &[ArtboardId]) -> Result<(), CmdError> {
        match cmd {
            DocCommand::OnArtboard { id, cmd }
                if self.locked(*id) && !with_font.contains(id) && moves_structure(cmd) =>
            {
                Err(CmdError::FontStructure(*id))
            }
            DocCommand::Batch(cmds) => cmds.iter().try_for_each(|c| self.allows_in(c, with_font)),
            _ => Ok(()),
        }
    }

    /// Después de aplicar: cada fuente que sigue siéndolo está bien armada y con su celda.
    pub fn holds(&self, doc: &Document) -> Result<(), CmdError> {
        for &(id, cell) in &self.0 {
            let Some(a) = doc.artboard(id) else {
                continue;
            };
            let Some(font) = &a.font else {
                continue;
            };
            if font.check(&a.sprite).is_err() || (a.sprite.width, a.sprite.height) != cell {
                return Err(CmdError::FontStructure(id));
            }
        }
        Ok(())
    }

    /// Ejecuta `cmd` en `history` (como continuación del paso anterior con `merged`) solo si
    /// deja las fuentes de `doc` como estaban armadas; si no, nada cambia.
    pub fn execute(
        history: &mut History<DocCommand>,
        doc: &mut Document,
        cmd: DocCommand,
        merged: bool,
    ) -> Result<(), CmdError> {
        let lock = FontLock::of(doc);
        lock.allows(&cmd)?;
        history.execute_checked(doc, cmd, merged, |d| lock.holds(d))
    }
}

/// Los artboards a los que `cmd` les pone fuente.
fn sets_font(cmd: &DocCommand, out: &mut Vec<ArtboardId>) {
    match cmd {
        DocCommand::SetFont { id, font: Some(_) } => out.push(*id),
        DocCommand::Batch(cmds) => cmds.iter().for_each(|c| sets_font(c, out)),
        _ => {}
    }
}

/// Si `cmd` agrega, quita o mueve frames o capas.
fn moves_structure(cmd: &Command) -> bool {
    match cmd {
        Command::AddFrame { .. }
        | Command::RemoveFrame { .. }
        | Command::MoveFrame { .. }
        | Command::AddLayer { .. }
        | Command::RemoveLayer { .. }
        | Command::MoveLayer { .. } => true,
        Command::Batch(cmds) => cmds.iter().any(moves_structure),
        _ => false,
    }
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
                FontLock::execute(&mut history, &mut doc, cmd, false),
                Err(CmdError::FontStructure(id))
            );
        }
        assert_eq!(doc, before);
        assert!(!history.can_undo());
        let add = add_glyphs(doc.artboard(id).unwrap(), &['á']).unwrap();
        assert!(FontLock::execute(&mut history, &mut doc, add, false).is_ok());
        // Las de un artboard que no es fuente pasan.
        let plain = DocCommand::OnArtboard {
            id: ArtboardId(1),
            cmd: Command::AddFrame {
                index: 0,
                frame: Frame { duration_ms: 100 },
            },
        };
        assert!(FontLock::execute(&mut history, &mut doc, plain, false).is_ok());
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
