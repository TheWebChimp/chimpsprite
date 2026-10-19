//! Artboards, capas, frames, tags, cels e inbetween.

use cs_commands::{Command, DocCommand, LayerProps, ops};
use cs_core::{
    AnimDirection, Artboard, BlendMode, Cel, Frame, GraphicPlay, LayerKind, Rgba, Tag, Timing,
};

use crate::error::{Result, invalid};
use crate::ops::document::{blank_sprite, check_size};
use crate::params::*;
use crate::target;
use crate::{Reply, Session};

/// Color por defecto de los tags nuevos (el mismo gris azulado de Aseprite).
const TAG_COLOR: Rgba = Rgba::rgb(0x5a, 0x6e, 0x9c);

fn blend(b: BlendArg) -> BlendMode {
    match b {
        BlendArg::Normal => BlendMode::Normal,
        BlendArg::Multiply => BlendMode::Multiply,
        BlendArg::Screen => BlendMode::Screen,
        BlendArg::Overlay => BlendMode::Overlay,
        BlendArg::Darken => BlendMode::Darken,
        BlendArg::Lighten => BlendMode::Lighten,
        BlendArg::Addition => BlendMode::Addition,
        BlendArg::Subtract => BlendMode::Subtract,
        BlendArg::Difference => BlendMode::Difference,
    }
}

fn direction(d: DirectionArg) -> AnimDirection {
    match d {
        DirectionArg::Forward => AnimDirection::Forward,
        DirectionArg::Reverse => AnimDirection::Reverse,
        DirectionArg::PingPong => AnimDirection::PingPong,
        DirectionArg::PingPongReverse => AnimDirection::PingPongReverse,
    }
}

impl Session {
    // --- Artboards ---

    pub fn add_artboard(&mut self, p: AddArtboard) -> Result<Reply> {
        check_size(p.width, p.height)?;
        let doc = &self.doc(&p.doc)?.document;
        let mode = p
            .color_mode
            .unwrap_or(match doc.artboards[0].sprite.color_mode {
                cs_core::ColorMode::Rgba => ColorModeArg::Rgba,
                cs_core::ColorMode::Indexed { .. } => ColorModeArg::Indexed,
            });
        let position = match p.position {
            Some([x, y]) => (x, y),
            None => doc
                .bounds()
                .map_or((0, 0), |((_, y0), (x1, _))| (x1 + 8, y0)),
        };
        let id = doc.next_artboard_id();
        let artboard = Artboard {
            id,
            name: p.name.clone(),
            position,
            sprite: blank_sprite(p.width, p.height, mode),
            guides: Vec::new(),
            symmetry: Default::default(),
        };
        let index = doc.artboards.len();
        self.run(
            &p.doc,
            vec![DocCommand::AddArtboard {
                index,
                artboard: Box::new(artboard),
            }],
        )?;
        Ok(Reply::text(format!(
            "Added artboard {} '{}' ({}×{}) at ({}, {}).",
            id.0, p.name, p.width, p.height, position.0, position.1
        )))
    }

    pub fn set_artboard(&mut self, p: SetArtboard) -> Result<Reply> {
        let (id, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let mut cmds = Vec::new();
        if p.name.is_some() || p.position.is_some() {
            cmds.push(DocCommand::SetArtboardProps {
                id,
                name: p.name.clone(),
                position: p.position.map(|[x, y]| (x, y)),
            });
        }
        if p.width.is_some() || p.height.is_some() {
            let (w, h) = (
                p.width.unwrap_or(sprite.width),
                p.height.unwrap_or(sprite.height),
            );
            check_size(w, h)?;
            cmds.push(DocCommand::OnArtboard {
                id,
                cmd: Command::ResizeCanvas {
                    width: w,
                    height: h,
                    offset: (0, 0),
                },
            });
        }
        if cmds.is_empty() {
            return Err(invalid(
                "nothing to change: pass name, position, width or height",
            ));
        }
        self.run(&p.doc, cmds)?;
        Ok(Reply::text(format!("Updated artboard {}.", id.0)))
    }

    pub fn remove_artboard(&mut self, p: ArtboardOnly) -> Result<Reply> {
        let (id, _) = self.sprite(&p.doc, p.artboard.as_ref())?;
        self.run(&p.doc, vec![DocCommand::RemoveArtboard { id }])?;
        Ok(Reply::text(format!("Removed artboard {}.", id.0)))
    }

    // --- Capas ---

    pub fn add_layer(&mut self, p: AddLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let kind = if p.group {
            LayerKind::Group
        } else {
            LayerKind::Image
        };
        let above = p
            .above
            .as_ref()
            .map(|r| target::layer(sprite, r))
            .transpose()?;
        let mut cmd = ops::new_layer(sprite, above, kind);
        if let Some(into) = &p.into {
            let group = target::layer(sprite, into)?;
            if sprite.layer(group).expect("resolved").kind != LayerKind::Group {
                return Err(invalid(format!("layer {into:?} is not a group")));
            }
            if let Command::AddLayer { index, layer } = &mut cmd {
                *index = sprite.subtree_range(group).expect("exists").end;
                layer.parent = Some(group);
            }
        }
        let Command::AddLayer { layer, .. } = &mut cmd else {
            unreachable!("new_layer returns AddLayer")
        };
        if let Some(name) = &p.name {
            layer.name = name.clone();
        }
        let (id, name) = (layer.id, layer.name.clone());
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!("Added layer {} '{name}'.", id.0)))
    }

    pub fn set_layer(&mut self, p: SetLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let props = LayerProps {
            name: p.name,
            visible: p.visible,
            locked: p.locked,
            opacity: p.opacity,
            blend: p.blend.map(blend),
            clip: p.clip,
        };
        let mut cmds = vec![Command::SetLayerProps { id, props }];
        if let Some(enabled) = p.mask_enabled {
            let Some(mut mask) = sprite.layer(id).and_then(|l| l.mask.clone()) else {
                return Err(invalid("the layer has no mask; add one with add_mask"));
            };
            mask.enabled = enabled;
            cmds.push(Command::SetLayerMask {
                id,
                mask: Some(mask),
            });
        }
        self.run_on(&p.doc, ab, cmds)?;
        Ok(Reply::text(format!("Updated layer {}.", id.0)))
    }

    pub fn add_mask(&mut self, p: AddMask) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let reveal = match p.reveal {
            Some(r) => {
                let (x0, y0, x1, y1) = target::region(sprite, Some(r))?;
                Some(cs_core::Mask::from_rect(
                    sprite.width,
                    sprite.height,
                    (x0, y0),
                    (x1 - 1, y1 - 1),
                ))
            }
            None => None,
        };
        let mask = ops::new_mask(sprite, p.hide_all, reveal.as_ref());
        self.run_on(
            &p.doc,
            ab,
            vec![Command::SetLayerMask {
                id,
                mask: Some(mask),
            }],
        )?;
        Ok(Reply::text(format!(
            "Added a mask to layer {}. Paint on it with on_mask: true (white reveals, black hides).",
            id.0
        )))
    }

    pub fn remove_mask(&mut self, p: LayerOnly) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        if sprite.layer(id).is_none_or(|l| l.mask.is_none()) {
            return Err(invalid("the layer has no mask"));
        }
        self.run_on(&p.doc, ab, vec![Command::SetLayerMask { id, mask: None }])?;
        Ok(Reply::text(format!("Removed the mask of layer {}.", id.0)))
    }

    pub fn apply_mask(&mut self, p: LayerOnly) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let cmd = ops::apply_mask(sprite, id)
            .ok_or_else(|| invalid("only image layers with a mask can have it applied"))?;
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Applied the mask of layer {} to its pixels in every frame.",
            id.0
        )))
    }

    pub fn remove_layer(&mut self, p: LayerOnly) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let range = sprite.subtree_range(id).expect("exists");
        if range.len() == sprite.layers.len() {
            return Err(invalid("cannot remove every layer of the sprite"));
        }
        let cmd = ops::remove_layer_tree(sprite, id).expect("exists");
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!("Removed layer {}.", id.0)))
    }

    pub fn move_layer(&mut self, p: MoveLayer) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let id = target::layer(sprite, &p.layer)?;
        let drop = match (&p.above, &p.below, &p.into) {
            (Some(r), None, None) => ops::LayerDrop::Above(target::layer(sprite, r)?),
            (None, Some(r), None) => ops::LayerDrop::Below(target::layer(sprite, r)?),
            (None, None, Some(r)) => ops::LayerDrop::Into(target::layer(sprite, r)?),
            _ => return Err(invalid("pass exactly one of above, below or into")),
        };
        let cmd = ops::move_layer(sprite, id, drop).ok_or_else(|| {
            invalid("invalid destination (a layer can't go inside itself, and into needs a group)")
        })?;
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!("Moved layer {}.", id.0)))
    }

    // --- Frames ---

    pub fn add_frame(&mut self, p: AddFrame) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let (index, mut cmds) = match p.duplicate {
            Some(src) => {
                target::frame(sprite, src)?;
                (
                    src + 1,
                    vec![ops::duplicate_frame(sprite, src, p.linked).expect("valid frame")],
                )
            }
            None => {
                let index = p.at.unwrap_or(sprite.frames.len());
                if index > sprite.frames.len() {
                    return Err(invalid(format!(
                        "at={index} out of range; frames can be inserted at 0..={}",
                        sprite.frames.len()
                    )));
                }
                let frame = Frame {
                    duration_ms: p.duration_ms.unwrap_or(100),
                };
                (index, vec![Command::AddFrame { index, frame }])
            }
        };
        if let (Some(_), Some(ms)) = (p.duplicate, p.duration_ms) {
            cmds.push(Command::SetFrameDuration {
                index,
                duration_ms: ms,
            });
        }
        self.run_on(&p.doc, ab, cmds)?;
        Ok(Reply::text(format!("Added frame {index}.")))
    }

    pub fn remove_frame(&mut self, p: FrameOnly) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let index = target::frame(sprite, p.frame)?;
        self.run_on(&p.doc, ab, vec![Command::RemoveFrame { index }])?;
        Ok(Reply::text(format!("Removed frame {index}.")))
    }

    pub fn move_frame(&mut self, p: MoveFrame) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        target::frame(sprite, p.from)?;
        target::frame(sprite, p.to)?;
        self.run_on(
            &p.doc,
            ab,
            vec![Command::MoveFrame {
                from: p.from,
                to: p.to,
            }],
        )?;
        Ok(Reply::text(format!("Moved frame {} to {}.", p.from, p.to)))
    }

    pub fn set_frame_duration(&mut self, p: SetFrameDuration) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let range = match p.frames {
            Some(r) => target::frames(sprite, r.from, r.to)?,
            None => 0..=sprite.frames.len() - 1,
        };
        let cmds = range
            .map(|index| Command::SetFrameDuration {
                index,
                duration_ms: p.duration_ms,
            })
            .collect();
        self.run_on(&p.doc, ab, cmds)?;
        Ok(Reply::text(format!(
            "Set duration to {} ms.",
            p.duration_ms
        )))
    }

    // --- Tags ---

    pub fn set_tag(&mut self, p: SetTag) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        target::frames(sprite, p.from, p.to)?;
        let color = match &p.color {
            Some(c) => target::hex(c)?,
            None => TAG_COLOR,
        };
        let tag = Tag {
            name: p.name.clone(),
            from: p.from,
            to: p.to,
            direction: direction(p.direction),
            color,
            repeat: p.repeat,
        };
        let mut tags = sprite.tags.clone();
        match tags.iter_mut().find(|t| t.name == p.name) {
            Some(t) => *t = tag,
            None => tags.push(tag),
        }
        self.run_on(&p.doc, ab, vec![Command::SetTags(tags)])?;
        Ok(Reply::text(format!(
            "Tag '{}' covers frames {}..={}.",
            p.name, p.from, p.to
        )))
    }

    pub fn remove_tag(&mut self, p: RemoveTag) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        if !sprite.tags.iter().any(|t| t.name == p.name) {
            return Err(invalid(format!("no tag named '{}'", p.name)));
        }
        let tags = sprite
            .tags
            .iter()
            .filter(|t| t.name != p.name)
            .cloned()
            .collect();
        self.run_on(&p.doc, ab, vec![Command::SetTags(tags)])?;
        Ok(Reply::text(format!("Removed tag '{}'.", p.name)))
    }

    // --- Slices ---

    pub fn set_slice(&mut self, p: SetSlice) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        target::frame(sprite, p.frame)?;
        if p.w == 0 || p.h == 0 {
            return Err(invalid("w and h must be at least 1"));
        }
        let mut slices = sprite.slices.clone();
        let index = match slices.iter().position(|s| s.name == p.name) {
            Some(i) => i,
            None => {
                slices.push(cs_core::Slice {
                    name: p.name.clone(),
                    color: cs_tools::SLICE_COLOR,
                    text: String::new(),
                    keys: Vec::new(),
                });
                slices.len() - 1
            }
        };
        let slice = &mut slices[index];
        if let Some(c) = &p.color {
            slice.color = target::hex(c)?;
        }
        if let Some(t) = &p.text {
            slice.text = t.clone();
        }
        slice.set_key(cs_core::SliceKey {
            frame: p.frame,
            bounds: (p.x, p.y, p.w, p.h),
            center: None,
            pivot: p.pivot.map(|[x, y]| (x, y)),
        });
        self.run_on(&p.doc, ab, vec![Command::SetSlices(slices)])?;
        Ok(Reply::text(format!(
            "Slice '{}' set from frame {}.",
            p.name, p.frame
        )))
    }

    pub fn remove_slice(&mut self, p: RemoveSlice) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        if !sprite.slices.iter().any(|s| s.name == p.name) {
            return Err(invalid(format!("no slice named '{}'", p.name)));
        }
        let slices = sprite
            .slices
            .iter()
            .filter(|s| s.name != p.name)
            .cloned()
            .collect();
        self.run_on(&p.doc, ab, vec![Command::SetSlices(slices)])?;
        Ok(Reply::text(format!("Removed slice '{}'.", p.name)))
    }

    // --- Instancias ---

    pub fn add_instance_layer(&mut self, p: AddInstanceLayer) -> Result<Reply> {
        let d = &self.doc(&p.doc)?.document;
        let host = target::artboard(d, p.artboard.as_ref())?;
        let source = target::artboard(d, Some(&p.source))?;
        if d.would_cycle(host, source) {
            return Err(invalid(
                "the source artboard contains this one; that would be a cycle",
            ));
        }
        let sprite = &d.artboard(host).expect("resolved").sprite;
        if sprite.color_mode != cs_core::ColorMode::Rgba {
            return Err(invalid("instances can only be placed in RGBA artboards"));
        }
        target::frame(sprite, p.frame)?;
        let src = d.artboard(source).expect("resolved");
        let x =
            p.x.unwrap_or((sprite.width as i32 - src.sprite.width as i32) / 2);
        let y =
            p.y.unwrap_or((sprite.height as i32 - src.sprite.height as i32) / 2);
        let name = p.name.clone().unwrap_or_else(|| src.name.clone());
        // Arriba de todo, en el nivel raíz (no dentro del grupo de la última capa).
        let cmd = ops::new_instance_layer(sprite, None, p.frame, source, &name, (x, y));
        let reply = format!("Placed '{}' as layer '{name}'.", src.name);
        self.run_on(&p.doc, host, vec![cmd])?;
        Ok(Reply::text(reply))
    }

    pub fn set_instance_key(&mut self, p: SetInstanceKey) -> Result<Reply> {
        let d = &self.doc(&p.doc)?.document;
        let host = target::artboard(d, p.artboard.as_ref())?;
        let source = p
            .source
            .as_ref()
            .map(|r| target::artboard(d, Some(r)))
            .transpose()?;
        let sprite = &d.artboard(host).expect("resolved").sprite;
        let layer = target::layer(sprite, &p.layer)?;
        target::frame(sprite, p.frame)?;
        let cmd = ops::update_instance_key(sprite, layer, p.frame, |k| {
            if let Some(s) = source {
                k.source = Some(s);
            }
            k.position = (p.x.unwrap_or(k.position.0), p.y.unwrap_or(k.position.1));
            k.flip_h = p.flip_h.unwrap_or(k.flip_h);
            k.flip_v = p.flip_v.unwrap_or(k.flip_v);
            k.opacity = p.opacity.unwrap_or(k.opacity);
            let first = p.first_frame.unwrap_or(match k.timing {
                Timing::Graphic { first_frame, .. } | Timing::MovieClip { first_frame } => {
                    first_frame
                }
            });
            k.timing = match (&p.timing, k.timing) {
                (Some(TimingArg::MovieClip), _) => Timing::MovieClip { first_frame: first },
                (Some(t), _) => Timing::Graphic {
                    first_frame: first,
                    play: match t {
                        TimingArg::Once => GraphicPlay::Once,
                        TimingArg::Single => GraphicPlay::Single,
                        _ => GraphicPlay::Loop,
                    },
                },
                (None, Timing::Graphic { play, .. }) => Timing::Graphic {
                    first_frame: first,
                    play,
                },
                (None, Timing::MovieClip { .. }) => Timing::MovieClip { first_frame: first },
            };
        })
        .ok_or_else(|| {
            invalid(
                "not an instance layer, no keyframe at or before that frame, or nothing changed",
            )
        })?;
        self.run_on(&p.doc, host, vec![cmd])?;
        Ok(Reply::text(format!(
            "Instance key set at frame {}.",
            p.frame
        )))
    }

    pub fn remove_instance_key(&mut self, p: RemoveInstanceKey) -> Result<Reply> {
        let (host, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::layer(sprite, &p.layer)?;
        let cmd = ops::remove_instance_key(sprite, layer, p.frame)
            .ok_or_else(|| invalid(format!("no instance keyframe at frame {}", p.frame)))?;
        self.run_on(&p.doc, host, vec![cmd])?;
        Ok(Reply::text(format!(
            "Removed the keyframe at frame {}.",
            p.frame
        )))
    }

    // --- Cels ---

    pub fn link_cels(&mut self, p: CelRange) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::image_layer(sprite, p.layer.as_ref())?;
        target::frames(sprite, p.from, p.to)?;
        let Some(cmd) = ops::link_range(sprite, layer, p.from, p.to) else {
            return Ok(Reply::text("Nothing to link (no cels, or already linked)."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Linked frames {}..={} of layer {}.",
            p.from, p.to, layer.0
        )))
    }

    pub fn unlink_cels(&mut self, p: CelRange) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::image_layer(sprite, p.layer.as_ref())?;
        target::frames(sprite, p.from, p.to)?;
        let Some(cmd) = ops::unlink_range(sprite, layer, p.from, p.to) else {
            return Ok(Reply::text("Nothing to unlink."));
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Unlinked frames {}..={} of layer {}.",
            p.from, p.to, layer.0
        )))
    }

    pub fn clear_cel(&mut self, p: CelOnly) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::image_layer(sprite, p.layer.as_ref())?;
        let frame = target::frame(sprite, p.frame)?;
        if sprite.cel(layer, frame).is_none() {
            return Ok(Reply::text("The cel is already empty."));
        }
        let cmd = Command::SetCel {
            layer,
            frame,
            cel: None,
        };
        self.run_on(&p.doc, ab, vec![cmd])?;
        Ok(Reply::text(format!(
            "Cleared layer {} frame {frame}.",
            layer.0
        )))
    }

    pub fn move_cel(&mut self, p: MoveCel) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        let layer = target::image_layer(sprite, p.layer.as_ref())?;
        let frames = match p.frames {
            Some(r) => target::frames(sprite, r.from, r.to)?,
            None => {
                let f = target::frame(sprite, p.frame)?;
                f..=f
            }
        };
        let cmds: Vec<Command> = frames
            .filter_map(|frame| {
                let cel = sprite.cel(layer, frame)?;
                let (x, y) = cel.position;
                Some(Command::SetCel {
                    layer,
                    frame,
                    cel: Some(Cel {
                        position: (x + p.dx, y + p.dy),
                        ..*cel
                    }),
                })
            })
            .collect();
        if cmds.is_empty() {
            return Ok(Reply::text("No cels in range; nothing moved."));
        }
        let n = cmds.len();
        self.run_on(&p.doc, ab, cmds)?;
        Ok(Reply::text(format!(
            "Moved {n} cel(s) of layer {} by ({}, {}).",
            layer.0, p.dx, p.dy
        )))
    }

    // --- Animación ---

    pub fn inbetween(&mut self, p: Inbetween) -> Result<Reply> {
        let (ab, sprite) = self.sprite(&p.doc, p.artboard.as_ref())?;
        target::frames(sprite, p.from, p.to)?;
        if p.from == p.to {
            return Err(invalid("from and to must be different frames (from < to)"));
        }
        if !(1..=64).contains(&p.count) {
            return Err(invalid("count must be 1-64"));
        }
        let duration_ms = sprite.frames[p.from].duration_ms;
        let mut cmds: Vec<Command> = (0..p.count)
            .map(|_| Command::AddFrame {
                index: p.to,
                frame: Frame { duration_ms },
            })
            .collect();
        for layer in &sprite.layers {
            let Some(start) = sprite.cel(layer.id, p.from) else {
                continue;
            };
            let end = sprite
                .cel(layer.id, p.to)
                .filter(|c| c.image == start.image);
            for i in 1..=p.count {
                let position = match end {
                    Some(end) => {
                        let t = i as f32 / (p.count + 1) as f32;
                        let lerp = |a: i32, b: i32| (a as f32 + (b - a) as f32 * t).round() as i32;
                        (
                            lerp(start.position.0, end.position.0),
                            lerp(start.position.1, end.position.1),
                        )
                    }
                    None => start.position,
                };
                cmds.push(Command::SetCel {
                    layer: layer.id,
                    frame: p.to + i - 1,
                    cel: Some(Cel { position, ..*start }),
                });
            }
        }
        self.run_on(&p.doc, ab, cmds)?;
        Ok(Reply::text(format!(
            "Inserted {} frame(s) at {}..={}; frame {} is now {}.",
            p.count,
            p.to,
            p.to + p.count - 1,
            p.to,
            p.to + p.count
        )))
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{LayerId, PixelValue};
    use serde_json::json;

    use super::*;
    use crate::ops::test_util::{p, session};

    #[test]
    fn artboards_add_set_remove() {
        let mut s = session(8, 8);
        s.add_artboard(p(
            json!({"doc": "d1", "name": "coin", "width": 4, "height": 4}),
        ))
        .unwrap();
        let doc = &s.doc("d1").unwrap().document;
        assert_eq!(doc.artboards[1].position, (16, 0));
        s.set_artboard(p(
            json!({"doc": "d1", "artboard": "coin", "width": 6, "position": [0, 20]}),
        ))
        .unwrap();
        let (_, coin) = s.sprite("d1", Some(&Ref::Name("coin".into()))).unwrap();
        assert_eq!((coin.width, coin.height), (6, 4));
        assert!(s.set_artboard(p(json!({"doc": "d1"}))).is_err());
        s.remove_artboard(p(json!({"doc": "d1", "artboard": 2})))
            .unwrap();
        assert!(
            s.remove_artboard(p(json!({"doc": "d1"}))).is_err(),
            "last artboard"
        );
    }

    #[test]
    fn layers_add_into_group_set_move_remove() {
        let mut s = session(4, 4);
        s.add_layer(p(json!({"doc": "d1", "name": "body"})))
            .unwrap();
        s.add_layer(p(json!({"doc": "d1", "name": "fx", "group": true})))
            .unwrap();
        s.add_layer(p(json!({"doc": "d1", "name": "spark", "into": "fx"})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        let names: Vec<_> = sp.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Layer 1", "body", "fx", "spark"]);
        assert_eq!(sp.layers[3].parent, Some(sp.layers[2].id));
        s.set_layer(p(
            json!({"doc": "d1", "layer": "body", "opacity": 128, "blend": "multiply"}),
        ))
        .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.layers[1].opacity, 128);
        assert_eq!(sp.layers[1].blend, BlendMode::Multiply);
        s.move_layer(p(json!({"doc": "d1", "layer": "body", "below": "Layer 1"})))
            .unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1.layers[0].name, "body");
        assert!(
            s.move_layer(p(json!({"doc": "d1", "layer": "body"})))
                .is_err()
        );
        assert!(
            s.move_layer(p(json!({"doc": "d1", "layer": "body", "into": "Layer 1"})))
                .is_err()
        );
        s.remove_layer(p(json!({"doc": "d1", "layer": "fx"})))
            .unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1.layers.len(), 2);
        assert!(
            s.add_layer(p(json!({"doc": "d1", "into": "body"})))
                .is_err()
        );
    }

    #[test]
    fn slices_create_update_per_frame_and_remove() {
        let mut s = session(8, 8);
        s.add_frame(p(json!({"doc": "d1"}))).unwrap();
        s.set_slice(p(
            json!({"doc": "d1", "name": "hit", "x": 1, "y": 1, "w": 4, "h": 4,
            "pivot": [2, 4], "text": "damage=3"}),
        ))
        .unwrap();
        s.set_slice(p(
            json!({"doc": "d1", "name": "hit", "frame": 1, "x": 3, "y": 1, "w": 4, "h": 4}),
        ))
        .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        let slice = &sp.slices[0];
        assert_eq!(slice.text, "damage=3");
        assert_eq!(slice.key_at(0).unwrap().pivot, Some((2, 4)));
        assert_eq!(slice.key_at(1).unwrap().bounds.0, 3);
        let described = crate::ops::test_util::json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        assert_eq!(described["artboards"][0]["slices"][0]["name"], "hit");
        assert!(
            s.set_slice(p(
                json!({"doc": "d1", "name": "x", "x": 0, "y": 0, "w": 0, "h": 1})
            ))
            .is_err()
        );
        s.remove_slice(p(json!({"doc": "d1", "name": "hit"})))
            .unwrap();
        assert!(
            s.remove_slice(p(json!({"doc": "d1", "name": "hit"})))
                .is_err()
        );
    }

    #[test]
    fn instance_layers_go_on_top_at_root_level() {
        let mut s = session(8, 8);
        s.add_artboard(p(
            json!({"doc": "d1", "name": "coin", "width": 2, "height": 2}),
        ))
        .unwrap();
        s.add_layer(p(json!({"doc": "d1", "name": "G", "group": true})))
            .unwrap();
        s.add_layer(p(json!({"doc": "d1", "name": "inner", "into": "G"})))
            .unwrap();
        s.add_instance_layer(p(json!({"doc": "d1", "source": "coin", "name": "Coin"})))
            .unwrap();
        let sprite = s.sprite("d1", None).unwrap().1;
        let top = sprite.layers.last().unwrap();
        assert_eq!(top.name, "Coin");
        assert_eq!(top.parent, None, "not inside the last layer's group");
    }

    #[test]
    fn instances_place_key_render_and_reject_cycles() {
        let mut s = session(8, 8);
        s.add_artboard(p(
            json!({"doc": "d1", "name": "coin", "width": 2, "height": 2}),
        ))
        .unwrap();
        s.draw_pixels(p(json!({"doc": "d1", "artboard": "coin",
            "pixels": [{"x": 0, "y": 0, "color": "#ff0000"}]})))
            .unwrap();
        s.add_frame(p(json!({"doc": "d1"}))).unwrap();
        s.add_instance_layer(p(json!({"doc": "d1", "source": "coin", "x": 3, "y": 4,
            "name": "Coin"})))
            .unwrap();
        let red = |s: &Session, x: usize, y: usize| {
            cs_render::render_frame(
                &cs_render::materialize(
                    &s.doc("d1").unwrap().document,
                    s.sprite("d1", None).unwrap().1,
                    0..2,
                ),
                1,
            )[y * 8 + x]
                .r
        };
        assert_eq!(red(&s, 3, 4), 255);
        s.set_instance_key(p(json!({"doc": "d1", "layer": "Coin", "frame": 1,
            "x": 5, "flip_h": true, "timing": "movie_clip"})))
            .unwrap();
        assert_eq!(red(&s, 6, 4), 255, "x=5 and flipped: the dot lands on 6");
        let described = crate::ops::test_util::json(&s.describe(p(json!({"doc": "d1"}))).unwrap());
        let layer = &described["artboards"][0]["layers_bottom_to_top"][1];
        assert_eq!(layer["kind"], "instance");
        assert_eq!(layer["instance"]["keys"].as_array().unwrap().len(), 2);
        let image = &described["artboards"][0]["layers_bottom_to_top"][0];
        assert!(
            image.get("instance").is_none(),
            "only instance layers say so"
        );
        s.remove_instance_key(p(json!({"doc": "d1", "layer": "Coin", "frame": 1})))
            .unwrap();
        let err = s
            .add_instance_layer(p(json!({"doc": "d1", "artboard": "coin", "source": 1})))
            .expect_err("coin cannot contain the artboard that contains it");
        assert!(
            err.to_string().contains("cycle"),
            "error message should mention cycle: {}",
            err
        );

        // Test that set_instance_key also rejects cycles
        s.add_artboard(p(
            json!({"doc": "d1", "name": "gem", "width": 2, "height": 2}),
        ))
        .unwrap();
        s.add_instance_layer(p(
            json!({"doc": "d1", "artboard": "coin", "source": "gem", "x": 0, "y": 0,
            "name": "GemInstance"}),
        ))
        .unwrap();
        let (_, coin_sprite) = s.sprite("d1", Some(&Ref::Name("coin".into()))).unwrap();
        let gem_layer = target::layer(coin_sprite, &Ref::Name("GemInstance".into())).unwrap();
        let err = s
            .set_instance_key(p(
                json!({"doc": "d1", "artboard": "coin", "layer": "GemInstance",
            "frame": 0, "source": 1}),
            ))
            .expect_err("coin cannot re-point its instance to the artboard that contains it");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("contain") || err_msg.contains("cycle"),
            "error message should mention containing or cycle: {}",
            err_msg
        );
        // Verify the layer's source is unchanged
        let (_, coin_sprite_after) = s.sprite("d1", Some(&Ref::Name("coin".into()))).unwrap();
        let layer = coin_sprite_after.layer(gem_layer).expect("layer exists");
        let instance = layer.instance.as_ref().expect("is instance layer");
        let key = instance.key_at(0).expect("key at frame 0");
        let gem_id = s
            .doc("d1")
            .unwrap()
            .document
            .artboards
            .iter()
            .find(|a| a.name == "gem")
            .map(|a| a.id)
            .expect("gem artboard exists");
        assert_eq!(
            key.source,
            Some(gem_id),
            "source should still be gem, not the root artboard"
        );
    }

    #[test]
    fn masks_and_clipping() {
        let mut s = session(4, 1);
        s.draw_line(p(
            json!({"doc": "d1", "points": [[0, 0], [3, 0]], "color": "#f00"}),
        ))
        .unwrap();
        assert!(
            s.draw_pixels(p(json!({"doc": "d1", "on_mask": true,
                "pixels": [{"x": 0, "y": 0, "color": "#000"}]})))
                .is_err(),
            "no mask yet"
        );
        s.add_mask(p(
            json!({"doc": "d1", "layer": 1, "reveal": {"x": 1, "y": 0, "w": 2, "h": 1}}),
        ))
        .unwrap();
        let alpha =
            |s: &Session, x| cs_render::composite_pixel(s.sprite("d1", None).unwrap().1, 0, x, 0).a;
        assert_eq!((alpha(&s, 0), alpha(&s, 1)), (0, 255));
        s.draw_pixels(p(json!({"doc": "d1", "on_mask": true,
            "pixels": [{"x": 0, "y": 0, "color": "#fff"}, {"x": 1, "y": 0, "color": "transparent"}]})))
            .unwrap();
        assert_eq!((alpha(&s, 0), alpha(&s, 1)), (255, 0));
        s.set_layer(p(json!({"doc": "d1", "layer": 1, "mask_enabled": false})))
            .unwrap();
        assert_eq!(alpha(&s, 1), 255);
        s.set_layer(p(json!({"doc": "d1", "layer": 1, "mask_enabled": true})))
            .unwrap();
        s.apply_mask(p(json!({"doc": "d1", "layer": 1}))).unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert!(sp.layers[0].mask.is_none());
        assert_eq!(alpha(&s, 1), 0, "applied to the pixels");
        assert!(s.remove_mask(p(json!({"doc": "d1", "layer": 1}))).is_err());

        s.add_layer(p(json!({"doc": "d1", "name": "shade"})))
            .unwrap();
        s.draw_line(p(
            json!({"doc": "d1", "layer": "shade", "points": [[0, 0], [3, 0]], "color": "#00f"}),
        ))
        .unwrap();
        s.set_layer(p(json!({"doc": "d1", "layer": "shade", "clip": true})))
            .unwrap();
        let px =
            |s: &Session, x| cs_render::composite_pixel(s.sprite("d1", None).unwrap().1, 0, x, 0);
        assert_eq!(px(&s, 0), cs_core::Rgba::rgb(0, 0, 255));
        assert_eq!(px(&s, 1).a, 0, "clipped where the base is empty");
    }

    #[test]
    fn frames_tags_and_durations() {
        let mut s = session(4, 4);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}]}),
        ))
        .unwrap();
        s.add_frame(p(
            json!({"doc": "d1", "duplicate": 0, "linked": true, "duration_ms": 50}),
        ))
        .unwrap();
        s.add_frame(p(json!({"doc": "d1"}))).unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.frames.len(), 3);
        assert_eq!(sp.frames[1].duration_ms, 50);
        assert_eq!(
            sp.cel(LayerId(1), 1).unwrap().image,
            sp.cel(LayerId(1), 0).unwrap().image
        );
        assert!(s.add_frame(p(json!({"doc": "d1", "at": 9}))).is_err());

        s.set_tag(p(
            json!({"doc": "d1", "name": "walk", "from": 0, "to": 1, "direction": "ping_pong"}),
        ))
        .unwrap();
        s.set_tag(p(json!({"doc": "d1", "name": "walk", "from": 1, "to": 2})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.tags.len(), 1);
        assert_eq!((sp.tags[0].from, sp.tags[0].to), (1, 2));
        assert!(
            s.set_tag(p(json!({"doc": "d1", "name": "x", "from": 0, "to": 3})))
                .is_err()
        );
        s.remove_tag(p(json!({"doc": "d1", "name": "walk"})))
            .unwrap();
        assert!(
            s.remove_tag(p(json!({"doc": "d1", "name": "walk"})))
                .is_err()
        );

        s.set_frame_duration(p(json!({"doc": "d1", "duration_ms": 70})))
            .unwrap();
        assert!(
            s.sprite("d1", None)
                .unwrap()
                .1
                .frames
                .iter()
                .all(|f| f.duration_ms == 70)
        );
        s.move_frame(p(json!({"doc": "d1", "from": 0, "to": 2})))
            .unwrap();
        s.remove_frame(p(json!({"doc": "d1", "frame": 2}))).unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1.frames.len(), 2);
    }

    #[test]
    fn link_unlink_clear() {
        let mut s = session(4, 4);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 1, "y": 1, "color": "#0f0"}]}),
        ))
        .unwrap();
        s.add_frame(p(json!({"doc": "d1"}))).unwrap();
        s.link_cels(p(json!({"doc": "d1", "from": 0, "to": 1})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(
            sp.cel(LayerId(1), 1).unwrap().image,
            sp.cel(LayerId(1), 0).unwrap().image
        );
        s.unlink_cels(p(json!({"doc": "d1", "from": 0, "to": 1})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_ne!(
            sp.cel(LayerId(1), 1).unwrap().image,
            sp.cel(LayerId(1), 0).unwrap().image
        );
        s.clear_cel(p(json!({"doc": "d1", "frame": 1}))).unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert!(sp.cel(LayerId(1), 1).is_none());
        assert_eq!(
            sp.pixel(LayerId(1), 0, 1, 1),
            PixelValue::Rgba(Rgba::rgb(0, 255, 0))
        );
    }

    #[test]
    fn move_cel_keeps_links() {
        let mut s = session(8, 8);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}]}),
        ))
        .unwrap();
        s.add_frame(p(json!({"doc": "d1", "duplicate": 0, "linked": true})))
            .unwrap();
        s.move_cel(p(json!({"doc": "d1", "frame": 1, "dx": 3, "dy": 2})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        let (a, b) = (
            sp.cel(LayerId(1), 0).unwrap(),
            sp.cel(LayerId(1), 1).unwrap(),
        );
        assert_eq!((a.position, b.position), ((0, 0), (3, 2)));
        assert_eq!(a.image, b.image);
        assert_eq!(
            sp.pixel(LayerId(1), 1, 3, 2),
            PixelValue::Rgba(Rgba::rgb(255, 0, 0))
        );
    }

    #[test]
    fn inbetween_interpolates_linked_positions() {
        let mut s = session(8, 8);
        s.draw_pixels(p(
            json!({"doc": "d1", "pixels": [{"x": 0, "y": 0, "color": "#f00"}]}),
        ))
        .unwrap();
        s.add_frame(p(json!({"doc": "d1", "duplicate": 0, "linked": true})))
            .unwrap();
        // Mueve el cel del frame 1 a (6, 3) sin desvincular.
        let (_, sp) = s.sprite("d1", None).unwrap();
        let cel = *sp.cel(LayerId(1), 1).unwrap();
        let moved = json!({"OnArtboard": {"id": 1, "cmd": {"SetCel": {"layer": 1, "frame": 1,
            "cel": {"position": [6, 3], "opacity": 255, "image": cel.image.0}}}}});
        s.apply_commands(p(json!({"doc": "d1", "commands": [moved]})))
            .unwrap();
        s.set_tag(p(json!({"doc": "d1", "name": "go", "from": 0, "to": 1})))
            .unwrap();

        s.inbetween(p(json!({"doc": "d1", "from": 0, "to": 1, "count": 2})))
            .unwrap();
        let (_, sp) = s.sprite("d1", None).unwrap();
        assert_eq!(sp.frames.len(), 4);
        let positions: Vec<_> = (0..4)
            .map(|f| sp.cel(LayerId(1), f).unwrap().position)
            .collect();
        assert_eq!(positions, [(0, 0), (2, 1), (4, 2), (6, 3)]);
        assert_eq!((sp.tags[0].from, sp.tags[0].to), (0, 3));
        assert!(
            s.inbetween(p(json!({"doc": "d1", "from": 2, "to": 1, "count": 1})))
                .is_err()
        );
        assert!(
            s.inbetween(p(json!({"doc": "d1", "from": 0, "to": 1, "count": 0})))
                .is_err()
        );
        s.undo(p(json!({"doc": "d1"}))).unwrap();
        assert_eq!(s.sprite("d1", None).unwrap().1.frames.len(), 2);
    }
}
