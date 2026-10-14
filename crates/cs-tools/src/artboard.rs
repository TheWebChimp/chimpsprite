//! Herramienta Artboard: crea, mueve y redimensiona artboards en coordenadas de mundo.

use cs_commands::{Command, DocCommand};
use cs_core::{Artboard, ArtboardId, ColorMode, Document, Sprite};

/// Bordes tomados al redimensionar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Edges {
    pub fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

/// Qué hay bajo el puntero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtboardHit {
    Edge(ArtboardId, Edges),
    Inside(ArtboardId),
}

/// Busca de arriba hacia abajo; los bordes (con tolerancia `slop`) ganan al interior.
pub fn hit_test(doc: &Document, p: (f32, f32), slop: f32) -> Option<ArtboardHit> {
    for artboard in doc.artboards.iter().rev() {
        let ((x0, y0), (x1, y1)) = artboard.rect();
        let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
        let in_x = p.0 >= x0 - slop && p.0 <= x1 + slop;
        let in_y = p.1 >= y0 - slop && p.1 <= y1 + slop;
        if !(in_x && in_y) {
            continue;
        }
        let edges = Edges {
            left: (p.0 - x0).abs() <= slop,
            right: (p.0 - x1).abs() <= slop,
            top: (p.1 - y0).abs() <= slop,
            bottom: (p.1 - y1).abs() <= slop,
        };
        if edges.any() {
            return Some(ArtboardHit::Edge(artboard.id, edges));
        }
        if artboard.contains(p) {
            return Some(ArtboardHit::Inside(artboard.id));
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
enum Mode {
    /// `template`: artboard del que se copian modo de color y paleta.
    Create {
        template: Option<ArtboardId>,
    },
    Move(ArtboardId),
    Resize(ArtboardId, Edges),
}

#[derive(Debug, Default)]
pub struct ArtboardTool {
    mode: Option<Mode>,
    start: (f32, f32),
    current: (f32, f32),
}

impl ArtboardTool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Empieza un gesto. Devuelve el artboard que debe quedar activo, si se tocó uno.
    pub fn press(
        &mut self,
        doc: &Document,
        p: (f32, f32),
        slop: f32,
        active: Option<ArtboardId>,
    ) -> Option<ArtboardId> {
        self.start = p;
        self.current = p;
        let (mode, activate) = match hit_test(doc, p, slop) {
            Some(ArtboardHit::Edge(id, edges)) => (Mode::Resize(id, edges), Some(id)),
            Some(ArtboardHit::Inside(id)) => (Mode::Move(id), Some(id)),
            None => (Mode::Create { template: active }, None),
        };
        self.mode = Some(mode);
        activate
    }

    /// Empieza a mover `id` directamente (p. ej. al arrastrar su etiqueta con otra herramienta).
    pub fn start_move(&mut self, id: ArtboardId, p: (f32, f32)) {
        self.start = p;
        self.current = p;
        self.mode = Some(Mode::Move(id));
    }

    pub fn drag(&mut self, p: (f32, f32)) {
        if self.mode.is_some() {
            self.current = p;
        }
    }

    pub fn is_active(&self) -> bool {
        self.mode.is_some()
    }

    pub fn release(&mut self, doc: &Document) -> Option<DocCommand> {
        let cmd = self.preview(doc);
        self.mode = None;
        cmd
    }

    /// Comando que produciría el gesto si terminara ahora.
    pub fn preview(&self, doc: &Document) -> Option<DocCommand> {
        let delta = (
            (self.current.0 - self.start.0).round() as i32,
            (self.current.1 - self.start.1).round() as i32,
        );
        match self.mode? {
            Mode::Create { template } => self.create(doc, template),
            Mode::Move(id) => {
                let artboard = doc.artboard(id)?;
                (delta != (0, 0)).then(|| DocCommand::SetArtboardProps {
                    id,
                    name: None,
                    position: Some((artboard.position.0 + delta.0, artboard.position.1 + delta.1)),
                })
            }
            Mode::Resize(id, edges) => resize(doc.artboard(id)?, edges, delta),
        }
    }

    fn create(&self, doc: &Document, template: Option<ArtboardId>) -> Option<DocCommand> {
        let a = (self.start.0.floor() as i32, self.start.1.floor() as i32);
        let b = (self.current.0.floor() as i32, self.current.1.floor() as i32);
        if a == b {
            return None;
        }
        let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
        let (w, h) = (
            (a.0 - b.0).unsigned_abs() + 1,
            (a.1 - b.1).unsigned_abs() + 1,
        );
        let template = template.and_then(|t| doc.artboard(t));
        let mode = template.map_or(ColorMode::Rgba, |t| t.sprite.color_mode);
        let mut sprite = Sprite::new(w, h, mode);
        if let Some(t) = template {
            sprite.palette = t.sprite.palette.clone();
        }
        let id = doc.next_artboard_id();
        Some(DocCommand::AddArtboard {
            index: doc.artboards.len(),
            artboard: Box::new(Artboard {
                id,
                name: format!("Artboard {}", id.0),
                position: (x0, y0),
                sprite,
            }),
        })
    }
}

/// Redimensiona moviendo los bordes indicados; el contenido queda fijo en el mundo.
fn resize(artboard: &Artboard, edges: Edges, delta: (i32, i32)) -> Option<DocCommand> {
    let ((x0, y0), (x1, y1)) = artboard.rect();
    let nx0 = if edges.left {
        (x0 + delta.0).min(x1 - 1)
    } else {
        x0
    };
    let ny0 = if edges.top {
        (y0 + delta.1).min(y1 - 1)
    } else {
        y0
    };
    let nx1 = if edges.right {
        (x1 + delta.0).max(nx0 + 1)
    } else {
        x1
    };
    let ny1 = if edges.bottom {
        (y1 + delta.1).max(ny0 + 1)
    } else {
        y1
    };
    if (nx0, ny0, nx1, ny1) == (x0, y0, x1, y1) {
        return None;
    }
    Some(DocCommand::Batch(vec![
        DocCommand::OnArtboard {
            id: artboard.id,
            cmd: Command::ResizeCanvas {
                width: (nx1 - nx0) as u32,
                height: (ny1 - ny0) as u32,
                offset: (x0 - nx0, y0 - ny0),
            },
        },
        DocCommand::SetArtboardProps {
            id: artboard.id,
            name: None,
            position: Some((nx0, ny0)),
        },
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cs_core::{LayerId, PixelValue, Rgba};

    fn doc() -> Document {
        let mut d = Document::with_sprite("hero", Sprite::new(10, 10, ColorMode::Rgba));
        d.artboards[0].position = (100, 100);
        d
    }

    fn gesture(d: &Document, from: (f32, f32), to: (f32, f32)) -> Option<DocCommand> {
        let mut t = ArtboardTool::new();
        t.press(d, from, 0.5, Some(ArtboardId(1)));
        t.drag(to);
        t.release(d)
    }

    #[test]
    fn drag_on_empty_space_creates() {
        let mut d = doc();
        let cmd = gesture(&d, (5.5, 7.2), (2.1, 3.9)).unwrap();
        cmd.apply(&mut d).unwrap();
        let ab = &d.artboards[1];
        assert_eq!(ab.position, (2, 3));
        assert_eq!((ab.sprite.width, ab.sprite.height), (4, 5));
        assert_eq!(ab.name, "Artboard 2");
    }

    #[test]
    fn drag_inside_moves() {
        let mut d = doc();
        gesture(&d, (105.0, 105.0), (108.2, 101.0))
            .unwrap()
            .apply(&mut d)
            .unwrap();
        assert_eq!(d.artboards[0].position, (103, 96));
    }

    #[test]
    fn left_edge_resize_keeps_world_content() {
        let mut d = doc();
        let red = PixelValue::Rgba(Rgba::rgb(255, 0, 0));
        DocCommand::OnArtboard {
            id: ArtboardId(1),
            cmd: Command::DrawPixels {
                layer: LayerId(1),
                frame: 0,
                pixels: vec![(0, 0, red)],
            },
        }
        .apply(&mut d)
        .unwrap();
        let before = d.clone();
        let inv = gesture(&d, (100.1, 105.0), (96.0, 105.0))
            .unwrap()
            .apply(&mut d)
            .unwrap();
        let ab = &d.artboards[0];
        assert_eq!(ab.position, (96, 100));
        assert_eq!((ab.sprite.width, ab.sprite.height), (14, 10));
        // El píxel que estaba en el mundo (100, 100) sigue ahí: local (4, 0).
        assert_eq!(ab.sprite.pixel(LayerId(1), 0, 4, 0), red);
        inv.apply(&mut d).unwrap();
        assert_eq!(d, before);
    }

    #[test]
    fn start_move_moves_from_anywhere() {
        let mut d = doc();
        let mut t = ArtboardTool::new();
        // Punto fuera del artboard (sobre su etiqueta), igual mueve.
        t.start_move(ArtboardId(1), (101.0, 98.0));
        t.drag((111.4, 90.0));
        t.release(&d).unwrap().apply(&mut d).unwrap();
        assert_eq!(d.artboards[0].position, (110, 92));
    }

    #[test]
    fn resize_never_goes_below_one_pixel() {
        let mut d = doc();
        gesture(&d, (110.0, 105.0), (50.0, 105.0))
            .unwrap()
            .apply(&mut d)
            .unwrap();
        assert_eq!(d.artboards[0].sprite.width, 1);
    }

    #[test]
    fn click_without_drag_does_nothing() {
        let d = doc();
        assert_eq!(gesture(&d, (5.0, 5.0), (5.2, 5.1)), None);
        assert_eq!(gesture(&d, (105.0, 105.0), (105.0, 105.0)), None);
    }
}
