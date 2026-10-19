//! Codificar y decodificar documentos según la extensión del nombre de archivo (sin E/S).

use std::path::Path;

use crate::{IoError, LoadReport, aseprite, chimp, import};
use cs_core::{Document, LayerKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileKind {
    Chimp,
    Aseprite,
    Png,
}

pub fn kind_of(name: &str) -> Option<FileKind> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "chimp" => Some(FileKind::Chimp),
        "aseprite" | "ase" => Some(FileKind::Aseprite),
        "png" => Some(FileKind::Png),
        _ => None,
    }
}

/// ¿Guardar en este formato aplica (pierde como capas editables) máscaras o recortes?
pub fn bakes_masks(doc: &Document, file_name: &str) -> bool {
    kind_of(file_name) == Some(FileKind::Aseprite)
        && doc
            .artboards
            .iter()
            .any(|a| a.sprite.layers.iter().any(|l| l.mask.is_some() || l.clip))
}

/// ¿Guardar en este formato convierte capas tilemap en capas de imagen? Pasa en `.aseprite` con
/// las tilemap que tienen máscara o recorte (o están en un grupo con máscara): para aplicar la
/// máscara se guardan como píxeles.
pub fn converts_tilemaps(doc: &Document, file_name: &str) -> bool {
    kind_of(file_name) == Some(FileKind::Aseprite)
        && doc
            .artboards
            .iter()
            .any(|a| !cs_render::masked_tilemap_layers(&a.sprite).is_empty())
}

/// Tilesets a los que les faltan tiles que usan sus grillas (pasa con los tilesets externos, que
/// se abren solo con el tile 0): al guardar como `.aseprite` o `.chimp` el archivo conserva los
/// números de tile, pero no esos tiles ni el vínculo al archivo externo. Nombres, sin repetir.
pub fn tilesets_missing_tiles(doc: &Document, file_name: &str) -> Vec<String> {
    if !matches!(
        kind_of(file_name),
        Some(FileKind::Aseprite | FileKind::Chimp)
    ) {
        return Vec::new();
    }
    let mut names = Vec::new();
    for sprite in doc.artboards.iter().map(|a| &a.sprite) {
        for (index, tileset) in sprite.tilesets.iter().enumerate() {
            let count = tileset.tiles.len() as u32;
            let missing = sprite
                .layers
                .iter()
                .filter(|l| l.kind == LayerKind::Tilemap && l.tileset == Some(index as u32))
                .flat_map(|l| sprite.cels.iter().filter(move |((id, _), _)| *id == l.id))
                .filter_map(|(_, cel)| sprite.tilemap(cel.image))
                .any(|grid| grid.tiles.iter().any(|t| t.index() >= count));
            if missing && !names.contains(&tileset.name) {
                names.push(tileset.name.clone());
            }
        }
    }
    names
}

/// Aviso para el usuario sobre `tilesets_missing_tiles` (en inglés, como toda la UI).
pub fn missing_tiles_note(names: &[String]) -> String {
    let list = names
        .iter()
        .map(|n| format!("'{n}'"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Tileset {list} is missing tiles that its tilemaps use (probably an external tileset \
         file, which can't be loaded yet). The saved file keeps the tile numbers but not those \
         tiles nor a link to the external file."
    )
}

/// ¿Guardar en este formato convierte instancias en píxeles?
pub fn bakes_instances(doc: &Document, file_name: &str) -> bool {
    kind_of(file_name) == Some(FileKind::Aseprite)
        && doc
            .artboards
            .iter()
            .any(|a| a.sprite.layers.iter().any(|l| l.instance.is_some()))
}

/// Nombre sin extensión.
pub fn stem(name: &str) -> &str {
    Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
}

#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("unknown file type: {0}")]
    UnknownType(String),
    #[error("this document has {0} artboards and .aseprite files hold only one; save it as .chimp")]
    MultipleArtboards(usize),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("{0}")]
    Platform(String),
}

/// Abre los bytes de un `.chimp`, `.aseprite`/`.ase` o `.png` como documento.
pub fn decode(name: &str, bytes: &[u8]) -> Result<(Document, LoadReport), FileError> {
    let kind = kind_of(name).ok_or_else(|| FileError::UnknownType(name.to_string()))?;
    Ok(match kind {
        FileKind::Chimp => chimp::read(bytes)?,
        FileKind::Aseprite => {
            let (sprite, report) = aseprite::read(bytes)?;
            (Document::with_sprite(stem(name), sprite), report)
        }
        FileKind::Png => (
            Document::with_sprite(stem(name), import::import_png(bytes)?),
            LoadReport::default(),
        ),
    })
}

/// Bytes a guardar con el nombre `file_name`, según su extensión.
pub fn encode(doc: &Document, file_name: &str) -> Result<Vec<u8>, FileError> {
    match kind_of(file_name) {
        Some(FileKind::Chimp) => Ok(chimp::write(doc)?),
        Some(FileKind::Aseprite) => match doc.artboards.as_slice() {
            // `.aseprite` no tiene instancias, máscaras ni recortes: se aplican a los píxeles. Los
            // tilemaps se guardan como tilemaps.
            [only] => {
                let baked =
                    cs_render::bake_layers(doc, &only.sprite, cs_render::Bake::KeepTilemaps);
                let sprite = baked.as_ref().unwrap_or(&only.sprite);
                let masked = cs_render::bake_masks(sprite);
                Ok(aseprite::write(masked.as_ref().unwrap_or(sprite)))
            }
            many => Err(FileError::MultipleArtboards(many.len())),
        },
        _ => Err(FileError::UnknownType(file_name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{Artboard, ArtboardId, ColorMode, Sprite};

    use super::*;

    #[test]
    fn kinds_by_extension() {
        assert_eq!(kind_of("a/b.CHIMP"), Some(FileKind::Chimp));
        assert_eq!(kind_of("x.ase"), Some(FileKind::Aseprite));
        assert_eq!(kind_of("x.png"), Some(FileKind::Png));
        assert_eq!(kind_of("x.txt"), None);
        assert_eq!(stem("dir/hero.aseprite"), "hero");
    }

    #[test]
    fn encode_and_decode_roundtrip() {
        let mut doc = Document::with_sprite("hero", Sprite::new(5, 4, ColorMode::Rgba));
        let bytes = encode(&doc, "hero.aseprite").unwrap();
        let (back, _) = decode("hero.aseprite", &bytes).unwrap();
        assert_eq!(back.artboards[0].name, "hero");
        assert_eq!(back.artboards[0].sprite.width, 5);

        doc.artboards.push(Artboard {
            guides: Vec::new(),
            symmetry: Default::default(),
            id: ArtboardId(2),
            name: "coin".into(),
            position: (20, 0),
            sprite: Sprite::new(2, 2, ColorMode::Rgba),
        });
        assert!(matches!(
            encode(&doc, "game.aseprite"),
            Err(FileError::MultipleArtboards(2))
        ));
        let (back, _) = decode("game.chimp", &encode(&doc, "game.chimp").unwrap()).unwrap();
        assert_eq!(back.artboards.len(), 2);
        assert!(matches!(
            decode("x.txt", b""),
            Err(FileError::UnknownType(_))
        ));
    }

    #[test]
    fn saving_aseprite_bakes_instances() {
        let mut d = Document::with_sprite("a", Sprite::new(4, 4, ColorMode::Rgba));
        let mut symbol = Sprite::new(2, 2, ColorMode::Rgba);
        cs_commands::Command::DrawPixels {
            layer: cs_core::LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(5, 6, 7)))],
        }
        .apply(&mut symbol)
        .unwrap();
        d.artboards.push(Artboard {
            id: ArtboardId(2),
            name: "s".into(),
            position: (9, 0),
            sprite: symbol,
            guides: Vec::new(),
            symmetry: Default::default(),
        });
        cs_commands::ops::new_instance_layer(
            &d.artboards[0].sprite.clone(),
            None,
            0,
            ArtboardId(2),
            "i",
            (2, 2),
        )
        .apply(&mut d.artboards[0].sprite)
        .unwrap();
        assert!(bakes_instances(&d, "x.aseprite"));
        assert!(!bakes_instances(&d, "x.chimp"));
        d.artboards.truncate(1);
        // Un solo artboard con la fuente ausente: igual guarda (la instancia no dibuja nada).
        let bytes = encode(&d, "x.aseprite").unwrap();
        let (back, _) = aseprite::read(&bytes).unwrap();
        assert_eq!(back.layers[1].kind, cs_core::LayerKind::Image);
    }

    /// 8×8 con una capa de imagen (1), una tilemap (2) con tiles rojos en las celdas (0, 0) y
    /// (1, 1) de 4×4, y otra tilemap (3) sin máscaras con un tile en (1, 0).
    fn masked_tilemap_sprite() -> Sprite {
        use cs_core::{Image, LayerId, PixelValue, Rgba, Tile, Tileset};
        let mut sprite = Sprite::new(8, 8, ColorMode::Rgba);
        let mut ts = Tileset::new("t", (4, 4), ColorMode::Rgba);
        let mut tile = Image::new(4, 4, ColorMode::Rgba);
        for y in 0..4 {
            for x in 0..4 {
                tile.set(x, y, PixelValue::Rgba(Rgba::rgb(200, 10 * x as u8, 30)))
                    .unwrap();
            }
        }
        ts.tiles.push(std::sync::Arc::new(tile));
        cs_commands::Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut sprite)
        .unwrap();
        let t = Tile::new(1, false, false, false);
        for (above, cells) in [(1, [(0, 0), (1, 1)]), (2, [(1, 0), (1, 0)])] {
            cs_commands::ops::new_tilemap_layer(&sprite.clone(), Some(LayerId(above)), 0)
                .apply(&mut sprite)
                .unwrap();
            let id = LayerId(above + 1);
            let cells: Vec<_> = cells.iter().map(|&(x, y)| (x, y, t)).collect();
            cs_commands::ops::set_tiles(&sprite, id, 0, &cells)
                .unwrap()
                .apply(&mut sprite)
                .unwrap();
        }
        sprite
    }

    fn rendered(sprite: &Sprite) -> Vec<cs_core::Rgba> {
        cs_render::render_frame(&cs_render::materialize_tilemaps(sprite, 0..1), 0)
    }

    /// Guarda como `.aseprite`, vuelve a abrir y compara lo que se ve.
    fn aseprite_roundtrip(sprite: Sprite, converts: bool) -> Sprite {
        let d = Document::with_sprite("m", sprite.clone());
        assert_eq!(converts_tilemaps(&d, "m.aseprite"), converts);
        assert!(!converts_tilemaps(&d, "m.chimp"));
        let (back, _) = decode("m.aseprite", &encode(&d, "m.aseprite").unwrap()).unwrap();
        let back = back.artboards[0].sprite.clone();
        assert_eq!(rendered(&back), rendered(&sprite));
        // La tilemap sin máscaras sigue siendo tilemap.
        assert_eq!(kind(&back, "Tilemap 3"), cs_core::LayerKind::Tilemap);
        back
    }

    /// Tipo de la capa con ese nombre (los ids cambian al releer el `.aseprite`).
    fn kind(sprite: &Sprite, name: &str) -> cs_core::LayerKind {
        sprite.layers.iter().find(|l| l.name == name).unwrap().kind
    }

    #[test]
    fn saving_aseprite_keeps_masks_on_tilemap_layers() {
        use cs_core::{LayerId, LayerKind, LayerMask};
        // (a) Una máscara que oculta la mitad izquierda de la tilemap 2.
        let mut s = masked_tilemap_sprite();
        let mut mask = LayerMask::filled(8, 8, 255);
        mask.data = std::sync::Arc::new((0..64).map(|i| if i % 8 < 4 { 0 } else { 255 }).collect());
        s.layer_mut(LayerId(2)).unwrap().mask = Some(mask);
        let back = aseprite_roundtrip(s, true);
        assert_eq!(kind(&back, "Tilemap 2"), LayerKind::Image);
    }

    #[test]
    fn saving_aseprite_keeps_layers_clipped_onto_tilemaps() {
        use cs_core::{Layer, LayerId, LayerKind, PixelValue, Rgba};
        // (b) Una capa de imagen azul recortada sobre la tilemap 2 (la base).
        let mut s = masked_tilemap_sprite();
        let mut shade = Layer::new_image(LayerId(4), "shade");
        shade.clip = true;
        let index = s.layer_index(LayerId(2)).unwrap() + 1;
        cs_commands::Command::AddLayer {
            index,
            layer: shade,
        }
        .apply(&mut s)
        .unwrap();
        let blue = PixelValue::Rgba(Rgba::new(0, 0, 255, 200));
        cs_commands::Command::DrawPixels {
            layer: LayerId(4),
            frame: 0,
            pixels: (0..8)
                .flat_map(|y| (0..8).map(move |x| (x, y, blue)))
                .collect(),
        }
        .apply(&mut s)
        .unwrap();
        let before = rendered(&s);
        assert!(before[0].b > 100, "shade over the base");
        assert_eq!(before[7].b, 30, "no base, no shade: only tilemap 3 there");
        let back = aseprite_roundtrip(s, false);
        // La base no tiene máscara ni recorte propio: sigue siendo tilemap.
        assert_eq!(kind(&back, "Tilemap 2"), LayerKind::Tilemap);
    }

    #[test]
    fn saving_aseprite_keeps_group_masks_over_tilemaps() {
        use cs_core::{Layer, LayerId, LayerKind, LayerMask};
        // (c) Un grupo con máscara que contiene a la tilemap 2.
        let mut s = masked_tilemap_sprite();
        let mut group = Layer::new_image(LayerId(5), "g");
        group.kind = LayerKind::Group;
        let mut mask = LayerMask::filled(8, 8, 255);
        mask.data = std::sync::Arc::new((0..64).map(|i| if i < 32 { 0 } else { 255 }).collect());
        group.mask = Some(mask);
        // Los grupos van justo debajo (antes en la lista) de sus hijas.
        let at = s.layer_index(LayerId(2)).unwrap();
        s.layers.insert(at, group);
        s.layer_mut(LayerId(2)).unwrap().parent = Some(LayerId(5));
        assert!(s.is_valid_tree());
        let back = aseprite_roundtrip(s, true);
        assert_eq!(kind(&back, "Tilemap 2"), LayerKind::Image);
    }

    #[test]
    fn saving_tilesets_with_missing_tiles_warns() {
        use cs_core::Tile;
        let mut s = masked_tilemap_sprite();
        assert!(
            tilesets_missing_tiles(&Document::with_sprite("m", s.clone()), "m.aseprite").is_empty()
        );
        // Como un tileset externo: solo queda el tile 0, pero las grillas usan el 1.
        s.tilesets[0].tiles.truncate(1);
        let d = Document::with_sprite("m", s.clone());
        assert_eq!(
            tilesets_missing_tiles(&d, "m.aseprite"),
            vec!["t".to_string()]
        );
        assert_eq!(tilesets_missing_tiles(&d, "m.chimp"), vec!["t".to_string()]);
        assert!(tilesets_missing_tiles(&d, "m.png").is_empty());
        let note = missing_tiles_note(&["t".to_string()]);
        assert!(note.contains("'t' is missing tiles"), "{note}");
        assert!(!note.contains("  "), "no runs of spaces: {note}");
        // Solo cuentan los tiles que alguna grilla usa.
        for (_, cel) in s.cels.clone() {
            if let Some(grid) = s.tilemaps.get_mut(&cel.image) {
                let grid = std::sync::Arc::make_mut(grid);
                grid.tiles.fill(Tile::EMPTY);
            }
        }
        assert!(tilesets_missing_tiles(&Document::with_sprite("m", s), "m.aseprite").is_empty());
    }

    #[test]
    fn saving_aseprite_and_chimp_keeps_tilemaps() {
        use cs_core::{LayerId, LayerKind, Tile};
        let mut sprite = Sprite::new(8, 8, ColorMode::Rgba);
        let mut ts = cs_core::Tileset::new("t", (4, 4), ColorMode::Rgba);
        ts.tiles.push(std::sync::Arc::new(cs_core::Image::new(
            4,
            4,
            ColorMode::Rgba,
        )));
        cs_commands::Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        }
        .apply(&mut sprite)
        .unwrap();
        cs_commands::ops::new_tilemap_layer(&sprite.clone(), None, 0)
            .apply(&mut sprite)
            .unwrap();
        let flipped = Tile::new(1, true, false, false);
        cs_commands::ops::set_tiles(&sprite, LayerId(2), 0, &[(1, 1, flipped)])
            .unwrap()
            .apply(&mut sprite)
            .unwrap();
        let d = Document::with_sprite("map", sprite);
        for name in ["x.aseprite", "x.chimp"] {
            let (back, report) = decode(name, &encode(&d, name).unwrap()).unwrap();
            assert!(report.warnings.is_empty(), "{name}: {:?}", report.warnings);
            let s = &back.artboards[0].sprite;
            assert_eq!(s.layers[1].kind, LayerKind::Tilemap, "{name}");
            assert_eq!(s.tilesets, d.artboards[0].sprite.tilesets, "{name}");
            let cel = s.cel(LayerId(2), 0).unwrap();
            assert_eq!(
                s.tilemap(cel.image).unwrap().get(1, 1),
                Some(flipped),
                "{name}"
            );
        }
    }
}
