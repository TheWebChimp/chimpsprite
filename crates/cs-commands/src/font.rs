//! Los cambios de una fuente pixel: crearla, agregar o quitar glifos (un frame cada uno) y sus
//! anchos. Pintar un glifo es editar un cel, como siempre.

use std::sync::Arc;

use cs_core::{
    AccentSkip, Artboard, ArtboardId, Cel, ComposedPixels, Document, FontInfo, FontKerning,
    FontMode, Frame, GlyphBuild, GlyphComponent, GlyphInfo, Image, ImageId, ONE_BIT_INK,
    PixelValue, Rgba, Sprite, accent_chars, font_sprite, ink_width,
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
        name: font.full_name(),
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
            font.glyphs.insert(index, GlyphInfo::new(ch));
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

/// Quita el glifo de `ch` y su frame; `None` si no está o es el último. Los compuestos que lo
/// usaban se desvinculan antes (se quedan con lo que se veía), en el mismo paso.
pub fn remove_glyph(artboard: &Artboard, ch: char) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs.len() < 2 {
        return None;
    }
    let mut cmds = Vec::new();
    let mut next = artboard.sprite.next_image_id();
    for user in font.users_of(ch) {
        let i = font.index_of(user)?;
        let pixels = font.composed(&artboard.sprite, i);
        cmds.push(on(artboard, put_pixels(artboard, i, &pixels, &mut next)));
    }
    for g in font.glyphs.iter_mut() {
        if g.components.iter().any(|c| c.ch == ch) {
            g.components.clear();
        }
    }
    font.glyphs.remove(index);
    cmds.push(on(artboard, Command::RemoveFrame { index }));
    cmds.push(set_font(artboard, font));
    Some(DocCommand::Batch(cmds))
}

fn on(artboard: &Artboard, cmd: Command) -> DocCommand {
    DocCommand::OnArtboard {
        id: artboard.id,
        cmd,
    }
}

/// Los comandos que dejan `pixels` (por fila y columna) como el cel del glifo `frame`, con una
/// imagen nueva `next` (que avanza); sin pixeles, sin cel.
fn put_pixels(
    artboard: &Artboard,
    frame: usize,
    pixels: &ComposedPixels,
    next: &mut ImageId,
) -> Command {
    let sprite = &artboard.sprite;
    let layer = sprite.layers[0].id;
    if pixels.is_empty() {
        return Command::SetCel {
            layer,
            frame,
            cel: None,
        };
    }
    let mut image = Image::new(sprite.width, sprite.height, sprite.color_mode);
    for (&(y, x), &v) in pixels {
        let _ = image.set(x, y, v);
    }
    let id = *next;
    *next = ImageId(next.0 + 1);
    Command::Batch(vec![
        Command::SetImage {
            id,
            image: Some(Arc::new(image)),
        },
        Command::SetCel {
            layer,
            frame,
            cel: Some(Cel {
                position: (0, 0),
                opacity: 255,
                image: id,
            }),
        },
    ])
}

/// Los componentes del glifo de `ch`; `None` si no cambia nada, si no está o si haría un ciclo.
pub fn set_components(
    artboard: &Artboard,
    ch: char,
    components: Vec<GlyphComponent>,
) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if font.glyphs[index].components == components || font.would_cycle(ch, &components) {
        return None;
    }
    font.glyphs[index].components = components;
    Some(set_font(artboard, font))
}

/// Desvincula el compuesto de `ch`: lo que se ve pasa a su cel y deja de tener componentes.
/// `None` si no es compuesto.
pub fn unlink_glyph(artboard: &Artboard, ch: char) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    let index = font.index_of(ch)?;
    if !font.glyphs[index].is_composite() {
        return None;
    }
    let pixels = font.composed(&artboard.sprite, index);
    let mut next = artboard.sprite.next_image_id();
    let put = put_pixels(artboard, index, &pixels, &mut next);
    font.glyphs[index].components.clear();
    Some(DocCommand::Batch(vec![
        on(artboard, put),
        set_font(artboard, font),
    ]))
}

/// Si el glifo `index` está vacío: sin pixeles propios ni componentes.
fn is_blank(font: &FontInfo, sprite: &Sprite, index: usize) -> bool {
    !font.glyphs[index].is_composite() && ink_width(sprite, index) == 0
}

/// Lo que hizo Auto-build accents.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccentReport {
    /// Los que armó.
    pub built: Vec<char>,
    /// De esos, los que quedaron sin su aire.
    pub cramped: Vec<char>,
    /// Los que ya estaban dibujados o compuestos (no se tocan).
    pub kept: usize,
    /// Los que no se pudieron: sin base o sin marca.
    pub missing: Vec<char>,
}

/// Arma como compuestos los acentuados de la tabla (o solo los de `only`) que falten o estén
/// vacíos (con `replace`, también los dibujados o compuestos: pierden sus pixeles), agregando los
/// glifos que hagan falta. `None` si no arma ninguno (con lo que pasó).
pub fn auto_build_accents(
    artboard: &Artboard,
    only: Option<&[char]>,
    replace: bool,
) -> (Option<DocCommand>, AccentReport) {
    let mut report = AccentReport::default();
    let Some(original) = artboard.font.as_ref() else {
        return (None, report);
    };
    let sprite = &artboard.sprite;
    let mut chars: Vec<char> = match only {
        Some(only) => only.to_vec(),
        None => accent_chars().collect(),
    };
    chars.sort_unstable();
    chars.dedup();
    let mut font = original.clone();
    let mut cmds = Vec::new();
    let mut clears = Vec::new();
    for ch in chars {
        if let Some(i) = original.index_of(ch)
            && !replace
            && !is_blank(original, sprite, i)
        {
            report.kept += 1;
            continue;
        }
        let plan = match original.plan_accent(sprite, ch) {
            Ok(plan) => plan,
            Err(AccentSkip::Unknown) => continue,
            Err(_) => {
                report.missing.push(ch);
                continue;
            }
        };
        if original.would_cycle(ch, &plan.components) {
            report.missing.push(ch);
            continue;
        }
        if let Some(i) = original.index_of(ch)
            && sprite.cel(sprite.layers[0].id, i).is_some()
        {
            clears.push(ch);
        }
        let index = match font.insert_index(ch) {
            Ok(i) => i,
            Err(i) => {
                font.glyphs.insert(i, GlyphInfo::new(ch));
                cmds.push(on(
                    artboard,
                    Command::AddFrame {
                        index: i,
                        frame: Frame { duration_ms: 100 },
                    },
                ));
                i
            }
        };
        font.glyphs[index].components = plan.components;
        if plan.cramped {
            report.cramped.push(ch);
        }
        report.built.push(ch);
    }
    if report.built.is_empty() {
        return (None, report);
    }
    let layer = sprite.layers[0].id;
    for ch in clears {
        if let Some(frame) = font.index_of(ch) {
            cmds.push(on(
                artboard,
                Command::SetCel {
                    layer,
                    frame,
                    cel: None,
                },
            ));
        }
    }
    cmds.push(set_font(artboard, font));
    (Some(DocCommand::Batch(cmds)), report)
}

/// Pone los pixeles de `glyphs` (en la celda) en la fuente: agrega los que faltan y, de los que
/// están, solo los vacíos (o todos con `replace`, que también les quita los componentes). Los
/// glifos sin pixeles en la celda (y el espacio) no cuentan. En 1 bit todo es tinta; en color, un
/// pixel sin color es negro. Devuelve el comando y cuántos puso.
pub fn fill_glyphs(
    artboard: &Artboard,
    glyphs: &[GlyphBuild],
    replace: bool,
) -> Option<(DocCommand, usize)> {
    let original = artboard.font.as_ref()?;
    let sprite = &artboard.sprite;
    let (w, h) = (sprite.width, sprite.height);
    let mut font = original.clone();
    let mut adds = Vec::new();
    let mut puts: Vec<(char, ComposedPixels)> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for g in glyphs {
        if g.ch == ' ' || !seen.insert(g.ch) {
            continue;
        }
        let pixels: ComposedPixels = g
            .pixels
            .iter()
            .filter(|(x, y, _)| *x < w && *y < h)
            .map(|&(x, y, c)| {
                let v = match original.mode {
                    FontMode::OneBit => ONE_BIT_INK,
                    FontMode::Color => PixelValue::Rgba(c.unwrap_or(Rgba::new(0, 0, 0, 255))),
                };
                ((y, x), v)
            })
            .filter(|(_, v)| !matches!(v, PixelValue::Rgba(c) if c.a == 0))
            .collect();
        if pixels.is_empty() {
            continue;
        }
        match original.index_of(g.ch) {
            Some(i) if !replace && !is_blank(original, sprite, i) => continue,
            Some(_) => {}
            None => {
                let i = font.insert_index(g.ch).err()?;
                font.glyphs.insert(
                    i,
                    GlyphInfo {
                        advance: g.advance,
                        ..GlyphInfo::new(g.ch)
                    },
                );
                adds.push(on(
                    artboard,
                    Command::AddFrame {
                        index: i,
                        frame: Frame { duration_ms: 100 },
                    },
                ));
            }
        }
        puts.push((g.ch, pixels));
    }
    if puts.is_empty() {
        return None;
    }
    let count = puts.len();
    let mut cmds = adds;
    let mut next = sprite.next_image_id();
    for (ch, pixels) in &puts {
        let i = font.index_of(*ch)?;
        font.glyphs[i].components.clear();
        cmds.push(on(artboard, put_pixels(artboard, i, pixels, &mut next)));
    }
    cmds.push(set_font(artboard, font));
    Some((DocCommand::Batch(cmds), count))
}

/// El kerning entero de la fuente; `None` si no cambia nada.
pub fn set_kerning(artboard: &Artboard, kerning: FontKerning) -> Option<DocCommand> {
    let mut font = artboard.font.clone()?;
    if font.kerning == kerning {
        return None;
    }
    font.kerning = kerning;
    Some(set_font(artboard, font))
}

/// Reemplaza la fuente y su sprite (Rebuild de una variante: la celda puede cambiar), en un paso.
/// `None` si el artboard no es una fuente o la fuente nueva no va con el sprite.
pub fn replace_font(artboard: &Artboard, font: FontInfo, sprite: Sprite) -> Option<DocCommand> {
    artboard.font.as_ref()?;
    font.check(&sprite).ok()?;
    Some(DocCommand::Batch(vec![
        DocCommand::SetSprite {
            id: artboard.id,
            sprite: Box::new(sprite),
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
            kerning: Vec::new(),
            style: Default::default(),
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
    fn set_kerning_and_replace_font_undo_even_when_the_cell_grows() {
        let (mut doc, id) = doc_with_font();
        let mut history = History::new();
        let before = doc.clone();
        let mut kerning = FontKerning::default();
        kerning.set(
            cs_core::KernSide::Char('A'),
            cs_core::KernSide::Char('V'),
            -1,
        );
        let a = doc.artboard(id).unwrap();
        assert!(
            set_kerning(a, FontKerning::default()).is_none(),
            "no change"
        );
        let cmd = set_kerning(a, kerning.clone()).unwrap();
        StructureLock::execute(&mut history, &mut doc, cmd, false).unwrap();
        let font = doc.artboard(id).unwrap().font.clone().unwrap();
        assert_eq!(font.kerning, kerning);
        // Una fuente nueva de otra celda (como Rebuild): pasa el candado, en un paso.
        let mut bigger = font.clone();
        bigger.name = "Rebuilt".into();
        let sprite = cs_core::font_sprite((20, 18), FontMode::OneBit, font.glyphs.len());
        let a = doc.artboard(id).unwrap();
        assert!(
            replace_font(
                a,
                bigger.clone(),
                cs_core::font_sprite((4, 4), FontMode::Color, 1)
            )
            .is_none()
        );
        let cmd = replace_font(a, bigger, sprite).unwrap();
        StructureLock::execute(&mut history, &mut doc, cmd, false).unwrap();
        let a = doc.artboard(id).unwrap();
        assert_eq!((a.sprite.width, a.sprite.height), (20, 18));
        assert_eq!(a.font.as_ref().unwrap().name, "Rebuilt");
        // Cambiar la celda sin la fuente sigue sin poder.
        let resize = DocCommand::SetSprite {
            id,
            sprite: Box::new(cs_core::font_sprite(
                (8, 8),
                FontMode::OneBit,
                a.sprite.frames.len(),
            )),
        };
        assert_eq!(
            StructureLock::execute(&mut history, &mut doc, resize, false),
            Err(CmdError::StructureLocked(id))
        );
        history.undo(&mut doc).unwrap();
        history.undo(&mut doc).unwrap();
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

#[cfg(test)]
#[path = "font_composite_tests.rs"]
mod composite_tests;
