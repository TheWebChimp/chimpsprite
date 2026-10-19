//! Ver (render a PNG), leer píxeles como texto y exportar.

use cs_core::{ColorMode, PixelValue, Rgba};
use cs_io::engines;
use cs_io::export::{self, Bitmap, JsonFormat, SheetLayout};
use cs_render::{OnionSkin, blend_over, render_frame, render_onion, resolve};

use crate::error::{Result, invalid};
use crate::params::{Export, ExportKind, LayoutArg, ReadPixels, Render};
use crate::target;
use crate::text::{Cell, grid};
use crate::{Reply, Session};

/// Lado mayor (en píxeles) que busca la escala automática de `render`.
const RENDER_TARGET: u32 = 512;
/// Celdas máximas por lado en `read_pixels`.
const READ_MAX: u32 = 128;

impl Session {
    pub fn render(&self, p: Render) -> Result<Reply> {
        let (id, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let frame = target::frame(sprite, p.frame)?;
        let scale = match p.scale {
            Some(s @ 1..=16) => s,
            Some(s) => return Err(invalid(format!("scale {s} must be 1-16"))),
            None => (RENDER_TARGET / sprite.width.max(sprite.height)).clamp(1, 16),
        };
        let background = p.background.as_deref().map(target::hex).transpose()?;
        // Las instancias se materializan antes de componer.
        let d = &self.doc(&p.doc)?.document;
        let composed = cs_render::materialize(d, sprite, 0..sprite.frames.len());
        let mut pixels = if p.onion {
            render_onion(&composed, frame, &OnionSkin::default())
        } else {
            render_frame(&composed, frame)
        };
        if let Some(bg) = background {
            for px in &mut pixels {
                *px = blend_over(bg, *px, px.a);
            }
        }
        let (w, h) = (sprite.width * scale, sprite.height * scale);
        let bitmap = Bitmap {
            width: w,
            height: h,
            pixels: (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .map(|(x, y)| pixels[((y / scale) * sprite.width + x / scale) as usize])
                .collect(),
        };
        let name = &self
            .doc(&p.doc)?
            .document
            .artboard(id)
            .expect("resolved")
            .name;
        Ok(Reply {
            text: format!(
                "Artboard {} '{name}', frame {frame} of {}, {}×{} px shown at {scale}×.",
                id.0,
                sprite.frames.len(),
                sprite.width,
                sprite.height
            ),
            image: Some(bitmap.to_png()?),
        })
    }

    pub fn read_pixels(&self, p: ReadPixels) -> Result<Reply> {
        let (_, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let frame = target::frame(sprite, p.frame)?;
        let layer = p
            .layer
            .as_ref()
            .map(|r| target::image_layer(sprite, Some(r)))
            .transpose()?;
        let (x0, y0, x1, y1) = target::region(sprite, p.rect)?;
        if (x1 - x0) as u32 > READ_MAX || (y1 - y0) as u32 > READ_MAX {
            return Err(invalid(format!(
                "region is {}×{}; read at most {READ_MAX}×{READ_MAX} at a time using rect",
                x1 - x0,
                y1 - y0
            )));
        }
        let d = &self.doc(&p.doc)?.document;
        let composed = cs_render::materialize(d, sprite, frame..frame + 1);
        let indexed = matches!(sprite.color_mode, ColorMode::Indexed { .. });
        let palette_index = |c: Rgba| {
            sprite
                .palette
                .colors()
                .iter()
                .position(|&p| p == c)
                .map(|i| i as u8)
        };
        let rows: Vec<Vec<Cell>> = (y0..y1)
            .map(|y| {
                (x0..x1)
                    .map(|x| match layer {
                        Some(l) => {
                            let v = sprite.pixel(l, frame, x, y);
                            let index = match v {
                                PixelValue::Index(i) => Some(i),
                                PixelValue::Rgba(_) => None,
                            };
                            Cell {
                                color: resolve(sprite, v),
                                index,
                            }
                        }
                        None => {
                            let color = cs_render::composite_pixel(&composed, frame, x, y);
                            let index = if indexed { palette_index(color) } else { None };
                            Cell { color, index }
                        }
                    })
                    .collect()
            })
            .collect();
        let source = match layer {
            Some(l) => format!("layer {}", l.0),
            None => "visible composite".into(),
        };
        Ok(Reply::text(format!(
            "frame {frame}, {source}, rect x={x0} y={y0} w={} h={} (row 0 is y={y0})\n{}",
            x1 - x0,
            y1 - y0,
            grid(&rows)
        )))
    }

    pub fn export(&self, p: Export) -> Result<Reply> {
        let (id, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        // Las instancias se hornean en capas de imagen antes de exportar.
        let baked = cs_render::bake_instances(&self.doc(&p.doc)?.document, sprite);
        let sprite = baked.as_ref().unwrap_or(sprite);
        let scale = match p.scale.unwrap_or(1) {
            s @ 1..=64 => s,
            s => return Err(invalid(format!("scale {s} must be 1-64"))),
        };
        let path = self.path(&p.path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("export")
            .to_string();
        let image_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("export.png")
            .to_string();
        let format = if p.json_array {
            JsonFormat::Array
        } else {
            JsonFormat::Hash
        };
        let layout = match p.layout {
            LayoutArg::Horizontal => SheetLayout::Horizontal,
            LayoutArg::Vertical => SheetLayout::Vertical,
            LayoutArg::Grid => SheetLayout::Grid {
                columns: p.columns.unwrap_or(4),
            },
        };
        let mut written = Vec::new();
        let mut write = |path: std::path::PathBuf, bytes: &[u8]| -> Result<()> {
            std::fs::write(&path, bytes)?;
            written.push(path.display().to_string());
            Ok(())
        };
        match p.kind {
            ExportKind::Png => {
                let frame = target::frame(sprite, p.frame)?;
                write(path.clone(), &export::export_png(sprite, frame, scale)?)?;
            }
            ExportKind::Gif => {
                let last = sprite.frames.len() - 1;
                write(path.clone(), &export::export_gif(sprite, 0, last, scale)?)?;
            }
            ExportKind::Sequence => {
                for (i, png) in export::export_png_sequence(sprite, scale)?
                    .into_iter()
                    .enumerate()
                {
                    write(
                        path.with_file_name(format!("{stem}_{:03}.png", i + 1)),
                        &png,
                    )?;
                }
            }
            ExportKind::Sheet => {
                let name = &self
                    .doc(&p.doc)?
                    .document
                    .artboard(id)
                    .expect("resolved")
                    .name;
                let sheet = export::export_sheet(sprite, name, &image_name, layout, scale, format)?;
                write(path.clone(), &sheet.png)?;
                write(path.with_extension("json"), sheet.json.as_bytes())?;
            }
            ExportKind::Atlas => {
                let doc = &self.doc(&p.doc)?.document;
                let atlas = export::export_atlas(doc, &image_name, scale, format)?;
                write(path.clone(), &atlas.png)?;
                write(path.with_extension("json"), atlas.json.as_bytes())?;
            }
            ExportKind::Godot | ExportKind::Unity | ExportKind::Bevy => {
                let name = &self
                    .doc(&p.doc)?
                    .document
                    .artboard(id)
                    .expect("resolved")
                    .name;
                let sheet = export::export_sheet(sprite, name, &image_name, layout, scale, format)?;
                write(path.clone(), &sheet.png)?;
                match p.kind {
                    ExportKind::Godot => {
                        let folder = p.godot_folder.as_deref().unwrap_or("res://");
                        let res = format!("{}/{image_name}", folder.trim_end_matches('/'));
                        let tres = engines::godot_sprite_frames(sprite, &res, layout, scale);
                        write(path.with_extension("tres"), tres.as_bytes())?;
                    }
                    ExportKind::Unity => {
                        let meta = engines::unity_meta(sprite, &image_name, layout, scale);
                        write(
                            path.with_file_name(format!("{image_name}.meta")),
                            meta.as_bytes(),
                        )?;
                    }
                    _ => {
                        let ron = engines::bevy_atlas(sprite, &image_name, layout, scale);
                        write(path.with_extension("ron"), ron.as_bytes())?;
                    }
                }
            }
        }
        if p.normal_map {
            let normals = match p.kind {
                ExportKind::Png => {
                    export::export_normal_png(sprite, target::frame(sprite, p.frame)?, scale)?
                }
                ExportKind::Sheet | ExportKind::Godot | ExportKind::Unity | ExportKind::Bevy => {
                    export::export_normal_sheet(sprite, layout, scale)?
                }
                _ => {
                    return Err(invalid(
                        "normal_map works with kind png, sheet, godot, unity or bevy",
                    ));
                }
            };
            write(path.with_file_name(format!("{stem}_n.png")), &normals)?;
        }
        Ok(Reply::text(format!("Wrote:\n{}", written.join("\n"))))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{p, session};

    #[test]
    fn render_scales_to_about_512() {
        let mut s = session(32, 16);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}]}),
        ))
        .unwrap();
        let r = s.render(p(json!({"doc": "d1"}))).unwrap();
        let png = cs_io::import::decode_png(r.image.as_ref().unwrap()).unwrap();
        assert_eq!((png.width, png.height), (512, 256));
        assert_eq!(png.pixels[0], Rgba::rgb(255, 0, 0));
        assert_eq!(png.pixels[(512 * 255 + 511) as usize], Rgba::TRANSPARENT);
        let r = s
            .render(p(
                json!({"doc": "d1", "scale": 2, "background": "#fff", "onion": true}),
            ))
            .unwrap();
        let png = cs_io::import::decode_png(r.image.as_ref().unwrap()).unwrap();
        assert_eq!(png.width, 64);
        assert_eq!(png.pixels[31], Rgba::rgb(255, 255, 255));
        assert!(s.render(p(json!({"doc": "d1", "scale": 0}))).is_err());
        assert!(s.render(p(json!({"doc": "d1", "frame": 3}))).is_err());
    }

    #[test]
    fn read_pixels_composite_and_layer() {
        let mut s = session(3, 2);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 1, "y": 0, "color": "#f00"}]}),
        ))
        .unwrap();
        s.add_layer(p(json!({"doc": "d1", "name": "top"}))).unwrap();
        s.draw_pixels(p(
            json!({"doc": "d1", "layer": "top", "pixels": [{"x": 2, "y": 1, "color": "#00f"}]}),
        ))
        .unwrap();
        let all = s.read_pixels(p(json!({"doc": "d1"}))).unwrap().text;
        assert!(all.ends_with("rows:\n.a.\n..b\n"), "{all}");
        let one = s
            .read_pixels(p(json!({"doc": "d1", "layer": 1})))
            .unwrap()
            .text;
        assert!(one.ends_with("rows:\n.a.\n...\n"), "{one}");
        let part = s
            .read_pixels(p(
                json!({"doc": "d1", "rect": {"x": 1, "y": 0, "w": 1, "h": 1}}),
            ))
            .unwrap()
            .text;
        assert!(part.ends_with("rows:\na\n"), "{part}");
        let big = session(200, 2).read_pixels(p(json!({"doc": "d1"})));
        assert!(big.is_err());
    }

    #[test]
    fn read_pixels_shows_palette_indices() {
        let mut s = Session::new(None);
        s.new_document(p(json!({"width": 2, "height": 1, "color_mode": "indexed"})))
            .unwrap();
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": 2}]}),
        ))
        .unwrap();
        let text = s.read_pixels(p(json!({"doc": "d1"}))).unwrap().text;
        assert!(text.contains("(index 2)"), "{text}");
    }

    #[test]
    fn export_every_kind() {
        let dir = std::env::temp_dir().join(format!("cs-agent-export-{}", std::process::id()));
        let mut s = Session::new(Some(dir.clone()));
        s.new_document(p(json!({"width": 4, "height": 4, "name": "hero"})))
            .unwrap();
        s.add_frame(p(json!({"doc": "d1"}))).unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "a.png", "kind": "png", "scale": 2}),
        ))
        .unwrap();
        s.export(p(json!({"doc": "d1", "path": "a.gif", "kind": "gif"})))
            .unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "seq/a.png", "kind": "sequence"}),
        ))
        .unwrap();
        s.export(p(json!({"doc": "d1", "path": "sheet.png", "kind": "sheet", "layout": "grid", "columns": 1})))
            .unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "atlas.png", "kind": "atlas", "json_array": true}),
        ))
        .unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "game/hero.png", "kind": "godot", "normal_map": true,
            "godot_folder": "res://art/"}),
        ))
        .unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "u/hero.png", "kind": "unity"}),
        ))
        .unwrap();
        s.export(p(
            json!({"doc": "d1", "path": "b/hero.png", "kind": "bevy"}),
        ))
        .unwrap();
        assert!(
            s.export(p(
                json!({"doc": "d1", "path": "x.gif", "kind": "gif", "normal_map": true})
            ))
            .is_err()
        );
        let tres = std::fs::read_to_string(dir.join("game/hero.tres")).unwrap();
        assert!(tres.contains("path=\"res://art/hero.png\""));
        for f in [
            "game/hero_n.png",
            "u/hero.png.meta",
            "b/hero.ron",
            "a.png",
            "a.gif",
            "seq/a_001.png",
            "seq/a_002.png",
            "sheet.png",
            "sheet.json",
            "atlas.json",
        ] {
            assert!(dir.join(f).exists(), "{f}");
        }
        let png = cs_io::import::decode_png(&std::fs::read(dir.join("a.png")).unwrap()).unwrap();
        assert_eq!(png.width, 8);
        assert!(
            s.export(p(json!({"doc": "d1", "path": "../x.png", "kind": "png"})))
                .is_err()
        );
        std::fs::remove_dir_all(dir).ok();
    }
}
