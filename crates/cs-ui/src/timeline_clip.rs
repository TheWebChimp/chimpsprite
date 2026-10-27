//! Copiar, cortar y pegar en el timeline, como en Aseprite: frames enteros (los elegidos en el
//! encabezado) o un rango de cels (capas × frames). Lo copiado guarda el contenido de cada cel,
//! así se pega aunque después cambie o se borre lo de origen, y las copias llevan su propia
//! imagen (no quedan enlazadas).

use std::sync::Arc;

use cs_commands::Command;
use cs_core::{Cel, Frame, Image, ImageId, LayerId, LayerKind, Sprite, TileGrid};

use crate::editor::Editor;

/// Lo de adentro de un cel copiado.
#[derive(Clone, Debug, PartialEq)]
enum Content {
    Image(Arc<Image>),
    Tilemap(Arc<TileGrid>),
}

/// Un cel copiado: su posición, su opacidad y su contenido.
#[derive(Clone, Debug, PartialEq)]
pub struct CelCopy {
    cel: Cel,
    content: Content,
}

impl CelCopy {
    fn of(sprite: &Sprite, layer: LayerId, frame: usize) -> Option<Self> {
        let cel = *sprite.cel(layer, frame)?;
        let content = match sprite.tilemap(cel.image) {
            Some(grid) => Content::Tilemap(grid.clone()),
            None => Content::Image(sprite.image(cel.image)?.clone()),
        };
        Some(Self { cel, content })
    }

    /// La capa donde puede ir: las imágenes en capas de imagen y los tilemaps en tilemaps.
    fn fits(&self, kind: LayerKind) -> bool {
        matches!(
            (&self.content, kind),
            (Content::Image(_), LayerKind::Image) | (Content::Tilemap(_), LayerKind::Tilemap)
        )
    }

    /// Los comandos para ponerlo en `layer` y `frame` con la imagen nueva `id`.
    fn place(&self, layer: LayerId, frame: usize, id: ImageId) -> [Command; 2] {
        let content = match &self.content {
            Content::Image(image) => Command::SetImage {
                id,
                image: Some(image.clone()),
            },
            Content::Tilemap(grid) => Command::SetTilemap {
                id,
                grid: Some(grid.clone()),
            },
        };
        let cel = Cel {
            image: id,
            ..self.cel
        };
        [
            content,
            Command::SetCel {
                layer,
                frame,
                cel: Some(cel),
            },
        ]
    }
}

/// Lo copiado del timeline.
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineClip {
    /// Un rango de cels de `rows` capas × `frames` frames: cada uno con su fila y su frame desde
    /// la esquina (los vacíos también: al pegar dejan vacío).
    Cels {
        rows: usize,
        frames: usize,
        cels: Vec<(usize, usize, Option<CelCopy>)>,
    },
    /// Frames enteros: cada uno con su duración y el cel de cada capa.
    Frames(Vec<(Frame, Vec<(LayerId, CelCopy)>)>),
}

impl Editor {
    /// Copia del timeline: los frames elegidos en el encabezado o el rango de cels (sin rango,
    /// el cel actual).
    pub fn copy_timeline(&mut self) -> Option<TimelineClip> {
        self.commit_floating();
        let sprite = self.sprite();
        let (a, b) = self.selected_frames();
        if self.frames_picked {
            let frames = (a..=b)
                .map(|f| {
                    let cels = sprite
                        .layers
                        .iter()
                        .filter_map(|l| Some((l.id, CelCopy::of(sprite, l.id, f)?)))
                        .collect();
                    (sprite.frames[f], cels)
                })
                .collect();
            return Some(TimelineClip::Frames(frames));
        }
        let layers = self.range_layers();
        let cels: Vec<_> = layers
            .iter()
            .enumerate()
            .flat_map(|(row, &layer)| {
                (a..=b).map(move |f| (row, f - a, CelCopy::of(sprite, layer, f)))
            })
            .collect();
        cels.iter()
            .any(|c| c.2.is_some())
            .then_some(TimelineClip::Cels {
                rows: layers.len(),
                frames: b - a + 1,
                cels,
            })
    }

    /// Copia y quita lo copiado: los cels quedan vacíos; los frames se borran (si no son todos).
    pub fn cut_timeline(&mut self) -> Option<TimelineClip> {
        self.named("Cut", Self::cut_timeline_content)
    }

    fn cut_timeline_content(&mut self) -> Option<TimelineClip> {
        let clip = self.copy_timeline()?;
        match &clip {
            TimelineClip::Frames(frames) => {
                if frames.len() >= self.frame_count() {
                    return Some(clip);
                }
                let (a, b) = self.selected_frames();
                let cmds = (a..=b)
                    .rev()
                    .map(|index| Command::RemoveFrame { index })
                    .collect();
                if self.execute(Command::Batch(cmds)) {
                    self.frame_range = None;
                    self.set_frame(a.min(self.frame_count() - 1));
                }
            }
            TimelineClip::Cels { .. } => self.clear_selected_cels(),
        }
        Some(clip)
    }

    /// Pega lo copiado del timeline (un paso de undo). Los frames van después del actual y
    /// quedan elegidos; los cels, desde la capa y el frame actuales (si no entran, se agregan
    /// frames al final) y quedan como el rango elegido.
    pub fn paste_timeline(&mut self, clip: &TimelineClip) {
        self.commit_floating();
        let sprite = self.sprite();
        let mut next = sprite.next_image_id().0;
        let mut id = || {
            next += 1;
            ImageId(next - 1)
        };
        let mut cmds = Vec::new();
        match clip {
            TimelineClip::Frames(frames) => {
                let at = self.frame + 1;
                for (i, (frame, cels)) in frames.iter().enumerate() {
                    cmds.push(Command::AddFrame {
                        index: at + i,
                        frame: *frame,
                    });
                    for (layer, copy) in cels {
                        let fits = sprite.layer(*layer).is_some_and(|l| copy.fits(l.kind));
                        if fits {
                            cmds.extend(copy.place(*layer, at + i, id()));
                        }
                    }
                }
                if self.execute_named(Command::Batch(cmds), "Paste") {
                    self.set_frame(at);
                    self.frame_range = Some((at, at + frames.len() - 1));
                    self.frames_picked = true;
                }
            }
            TimelineClip::Cels { rows, frames, cels } => {
                let order = self.timeline_layers();
                let Some(top) = order.iter().position(|l| *l == self.layer) else {
                    return;
                };
                let from = self.frame;
                // Los frames que falten, al final, con la duración del último.
                let count = sprite.frames.len();
                let last = sprite.frames[count - 1];
                for index in count..from + frames {
                    cmds.push(Command::AddFrame { index, frame: last });
                }
                for (row, df, copy) in cels {
                    let Some(&layer) = order.get(top + row) else {
                        continue;
                    };
                    let Some(kind) = sprite.layer(layer).map(|l| l.kind) else {
                        continue;
                    };
                    let frame = from + df;
                    match copy {
                        Some(copy) if copy.fits(kind) => {
                            cmds.extend(copy.place(layer, frame, id()))
                        }
                        // Lo vacío deja vacío (solo en capas donde iría un cel).
                        None if matches!(kind, LayerKind::Image | LayerKind::Tilemap)
                            && sprite.cel(layer, frame).is_some() =>
                        {
                            cmds.push(Command::SetCel {
                                layer,
                                frame,
                                cel: None,
                            })
                        }
                        _ => {}
                    }
                }
                if cmds.is_empty() || !self.execute_named(Command::Batch(cmds), "Paste") {
                    return;
                }
                let bottom = order[(top + rows - 1).min(order.len() - 1)];
                self.select_cels((self.layer, from), (bottom, from + frames - 1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, PixelValue, Rgba, Sprite};

    use super::*;

    /// Dos capas de imagen (arriba la 2) y 3 frames, con un punto rojo en la capa de abajo,
    /// frame 0, y uno verde en la de arriba, frame 1.
    fn editor() -> (Editor, LayerId, LayerId) {
        let mut e = Editor::with_sprite(Sprite::new(4, 4, ColorMode::Rgba));
        e.insert_empty_frame();
        e.insert_empty_frame();
        e.add_layer(LayerKind::Image);
        let order = e.timeline_layers();
        let (top, bottom) = (order[0], order[1]);
        for (layer, frame, c) in [
            (bottom, 0, Rgba::rgb(255, 0, 0)),
            (top, 1, Rgba::rgb(0, 255, 0)),
        ] {
            e.execute(Command::DrawPixels {
                layer,
                frame,
                pixels: vec![(0, 0, PixelValue::Rgba(c))],
            });
        }
        e.set_frame(0);
        (e, top, bottom)
    }

    fn pixel(e: &Editor, layer: LayerId, frame: usize) -> PixelValue {
        e.sprite().pixel(layer, frame, 0, 0)
    }

    #[test]
    fn copied_cels_paste_from_the_current_cel_and_add_the_frames_they_need() {
        let (mut e, top, bottom) = editor();
        e.select_layer(top);
        e.select_cels((top, 0), (bottom, 1));
        let clip = e.copy_timeline().expect("cels");
        // Desde la capa de arriba, frame 2: el segundo frame del rango cae en uno nuevo.
        e.frame_range = None;
        e.select_layer(top);
        e.set_frame(2);
        e.paste_timeline(&clip);
        assert_eq!(e.frame_count(), 4, "one frame added");
        assert_eq!(pixel(&e, bottom, 2), pixel(&e, bottom, 0), "the red one");
        assert_eq!(pixel(&e, top, 3), pixel(&e, top, 1), "the green one");
        assert_ne!(
            e.sprite().cel(bottom, 2).unwrap().image,
            e.sprite().cel(bottom, 0).unwrap().image,
            "its own image"
        );
        assert_eq!(e.selected_cels().len(), 4, "the pasted range is selected");
        // Un solo paso de undo.
        e.undo();
        assert_eq!(e.frame_count(), 3);
        assert!(e.sprite().cel(bottom, 2).is_none());
    }

    #[test]
    fn copied_frames_paste_after_the_current_one() {
        let (mut e, top, bottom) = editor();
        e.frames_picked = true;
        e.frame_range = Some((0, 1));
        let clip = e.copy_timeline().expect("frames");
        e.frame_range = None;
        e.set_frame(2);
        e.paste_timeline(&clip);
        assert_eq!(e.frame_count(), 5);
        assert_eq!(pixel(&e, bottom, 3), pixel(&e, bottom, 0));
        assert_eq!(pixel(&e, top, 4), pixel(&e, top, 1));
        assert_eq!(
            e.selected_frames(),
            (3, 4),
            "the pasted frames are selected"
        );
    }

    #[test]
    fn cutting_cels_empties_them_and_cutting_frames_removes_them() {
        let (mut e, top, bottom) = editor();
        e.select_layer(bottom);
        let clip = e.cut_timeline().expect("the current cel");
        assert!(e.sprite().cel(bottom, 0).is_none());
        e.paste_timeline(&clip);
        assert!(e.sprite().cel(bottom, 0).is_some());
        e.frames_picked = true;
        e.frame_range = Some((1, 1));
        e.set_frame(1);
        e.cut_timeline().expect("a frame");
        assert_eq!(e.frame_count(), 2);
        assert!(e.sprite().cel(top, 1).is_none(), "the green frame is gone");
    }

    #[test]
    fn nothing_to_copy_in_empty_cels() {
        let (mut e, top, _) = editor();
        e.select_layer(top);
        assert!(e.copy_timeline().is_none());
    }
}
