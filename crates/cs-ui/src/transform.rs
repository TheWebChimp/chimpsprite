//! Transformar la selección. Lo seleccionado se levanta una sola vez y flota (se mueve, se escala,
//! se rota) hasta que se confirma, en un solo paso de undo, o se cancela sin dejar rastro. Lo que
//! queda debajo mientras flota no se mezcla ni se lleva en los arrastres siguientes.

use std::borrow::Cow;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, LayerId, LayerKind, Mask, PixelValue, Sprite};
use cs_raster::{Block, MAX_BLOCK_SIDE, RotationAlgorithm};

use crate::editor::{DocView, Editor, ToolKind};
use crate::transform_box::{TransformBox, TransformHit};

/// Contenido levantado de una capa mientras se transforma.
#[derive(Clone, Debug)]
pub struct Floating {
    /// Píxeles levantados, sin transformar (`None` = fuera de la máscara).
    source: Block<PixelValue>,
    /// Esquina de la selección original (o de lo pegado).
    origin: (i32, i32),
    /// Destino: escala, volteos y ángulo.
    bbox: TransformBox,
    layer: LayerId,
    frame: usize,
    artboard: ArtboardId,
    /// Viene de pegar: no deja hueco en el origen.
    from_paste: bool,
    /// Selección de antes de levantar; cancelar vuelve a ella.
    before: Option<Mask>,
    /// Tamaño del lienzo al levantar; si cambia, se cancela.
    canvas: (u32, u32),
    /// Resultado transformado y su esquina en el lienzo (caché hasta el siguiente cambio).
    rendered: Block<PixelValue>,
    at: (i32, i32),
    /// Rotación con la que se calculó `rendered`.
    rendered_with: RotationAlgorithm,
}

impl Floating {
    fn new(
        source: Block<PixelValue>,
        origin: (i32, i32),
        target: (ArtboardId, LayerId, usize),
        from_paste: bool,
        before: Option<Mask>,
        canvas: (u32, u32),
    ) -> Self {
        let bbox = TransformBox::new(origin.0, origin.1, source.width, source.height);
        Self {
            rendered: source.clone(),
            at: origin,
            rendered_with: RotationAlgorithm::Fast,
            source,
            origin,
            bbox,
            artboard: target.0,
            layer: target.1,
            frame: target.2,
            from_paste,
            before,
            canvas,
        }
    }

    /// Escala (con volteos) y rota lo levantado según la caja.
    fn render(&mut self, algorithm: RotationAlgorithm) {
        let b = self.bbox;
        let scaled = cs_raster::scale_nearest(&self.source, b.width(), b.height());
        let (rotated, (dx, dy)) = algorithm.rotate(&scaled, b.angle);
        self.rendered = rotated;
        self.at = (b.left() + dx, b.top() + dy);
        self.rendered_with = algorithm;
    }

    /// Sigue igual que al levantar: confirmar no cambiaría nada.
    fn unchanged(&self) -> bool {
        let (w, h) = (self.source.width, self.source.height);
        !self.from_paste && self.bbox == TransformBox::new(self.origin.0, self.origin.1, w, h)
    }

    /// Máscara de lo transformado; lo que cae fuera del lienzo se pierde.
    fn mask(&self) -> Option<Mask> {
        let (w, h) = self.canvas;
        let (ax, ay) = self.at;
        let points = self.rendered.points().map(|(x, y, _)| (ax + x, ay + y));
        let mask = Mask::from_points(w, h, points);
        (!mask.is_empty()).then_some(mask)
    }

    /// Comando que deja lo flotante en la capa: el hueco en el origen (si no viene de pegar) y el
    /// estampado, sin borrar con lo transparente.
    fn command(&self, sprite: &Sprite) -> Command {
        let clear = sprite.color_mode.transparent();
        let mut pixels = Vec::new();
        if !self.from_paste {
            let (ox, oy) = self.origin;
            pixels.extend(
                self.source
                    .points()
                    .map(|(x, y, _)| (ox + x, oy + y, clear)),
            );
        }
        let (ax, ay) = self.at;
        pixels.extend(
            self.rendered
                .points()
                .filter(|&(_, _, v)| cs_render::resolve(sprite, v).a > 0)
                .map(|(x, y, v)| (ax + x, ay + y, v)),
        );
        Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        }
    }
}

/// Arrastre en curso sobre lo seleccionado o sobre la caja de transformación.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TransformDrag {
    hit: TransformHit,
    /// Píxel local donde se presionó.
    start_pixel: (i32, i32),
    /// Caja al presionar: cada movimiento se calcula desde acá.
    start_box: TransformBox,
    /// Se presionó dentro de lo seleccionado.
    inside: bool,
    /// El arrastre ya cambió la caja (y lo seleccionado ya flota).
    moved: bool,
}

impl Editor {
    pub fn floating(&self) -> Option<&Floating> {
        self.floating.as_ref()
    }

    pub fn is_floating(&self) -> bool {
        self.floating.is_some()
    }

    /// Se puede transformar: herramienta de selección o Move sobre una capa de imagen editable,
    /// fuera del modo Tiles y sin editar la máscara (como el portapapeles), con algo seleccionado
    /// o flotando.
    pub fn can_transform(&self) -> bool {
        (self.tool.is_selection() || self.tool == ToolKind::Move)
            && self.clip_layer_ok(true)
            && (self.floating.is_some() || self.selection.is_some())
    }

    /// Caja de lo flotante o, si no flota nada, el rectángulo que contiene la selección.
    pub fn transform_box(&self) -> Option<TransformBox> {
        if let Some(f) = &self.floating {
            return Some(f.bbox);
        }
        let ((x0, y0), (x1, y1)) = self.selection.as_ref()?.bounds()?;
        Some(TransformBox::new(
            x0,
            y0,
            (x1 - x0 + 1) as u32,
            (y1 - y0 + 1) as u32,
        ))
    }

    /// Levanta lo seleccionado de la capa actual. El documento no cambia hasta confirmar.
    pub fn lift(&mut self) -> bool {
        if self.floating.is_some() {
            return true;
        }
        if !self.clip_layer_ok(true) {
            return false;
        }
        let Some(mask) = self.selection.clone() else {
            return false;
        };
        let Some(((x0, y0), (x1, y1))) = mask.bounds() else {
            return false;
        };
        let sprite = self.sprite();
        let mut source = Block::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        for (x, y) in mask.points() {
            source.set(
                x - x0,
                y - y0,
                Some(sprite.pixel(self.layer, self.frame, x, y)),
            );
        }
        let canvas = (sprite.width, sprite.height);
        let target = (self.active, self.layer, self.frame);
        self.floating = Some(Floating::new(
            source,
            (x0, y0),
            target,
            false,
            Some(mask),
            canvas,
        ));
        self.revision += 1;
        true
    }

    /// Empieza a mover, escalar o rotar lo seleccionado. Se levanta recién al cambiar algo, así
    /// un clic no levanta nada.
    pub fn press_transform(&mut self, hit: TransformHit, world: (f32, f32)) {
        let Some(start_box) = self.transform_box() else {
            return;
        };
        self.stop();
        let start_pixel = self.to_local(world);
        let inside = self
            .selection
            .as_ref()
            .is_some_and(|m| m.contains(start_pixel.0, start_pixel.1));
        self.transform_drag = Some(TransformDrag {
            hit,
            start_pixel,
            start_box,
            inside,
            moved: false,
        });
        self.revision += 1;
    }

    pub(crate) fn drag_transform(&mut self, world: (f32, f32)) {
        let Some(mut drag) = self.transform_drag else {
            return;
        };
        let pixel = self.to_local(world);
        let bbox = match drag.hit {
            TransformHit::Move => drag
                .start_box
                .translated(pixel.0 - drag.start_pixel.0, pixel.1 - drag.start_pixel.1),
        };
        if Some(bbox) == self.transform_box() {
            return;
        }
        if !drag.moved {
            // Se levanta una sola vez, al primer cambio.
            if !self.lift() {
                self.transform_drag = None;
                return;
            }
            drag.moved = true;
            self.transform_drag = Some(drag);
        }
        self.set_floating_box(bbox, false);
    }

    /// Termina el arrastre. Devuelve el píxel presionado si fue un clic sin arrastrar dentro de la
    /// selección con una herramienta de selección y sin nada flotando: es un clic normal de esa
    /// herramienta (en Replace, deselecciona).
    pub(crate) fn release_transform(&mut self) -> Option<(i32, i32)> {
        let drag = self.transform_drag.take()?;
        self.revision += 1;
        if drag.moved {
            // Durante el arrastre la rotación usa Fast; al soltar se calcula la elegida una vez.
            if self
                .floating
                .as_ref()
                .is_some_and(|f| f.rendered_with != self.options.rotation)
            {
                self.refresh_floating(false);
            }
            return None;
        }
        if self.floating.is_some() {
            // Con Move, un clic fuera de lo flotante lo confirma.
            if self.tool == ToolKind::Move && !drag.inside {
                self.commit_floating();
            }
            return None;
        }
        (self.tool.is_selection() && drag.hit == TransformHit::Move).then_some(drag.start_pixel)
    }

    /// Cambia la caja de lo flotante si el resultado no pasa de `MAX_BLOCK_SIDE`; si pasa, el
    /// handle se queda en el último tamaño válido.
    fn set_floating_box(&mut self, bbox: TransformBox, fast: bool) -> bool {
        let (w, h) = cs_raster::rotated_size(
            bbox.width().unsigned_abs(),
            bbox.height().unsigned_abs(),
            bbox.angle,
        );
        if w > MAX_BLOCK_SIDE || h > MAX_BLOCK_SIDE {
            return false;
        }
        let Some(f) = self.floating.as_mut() else {
            return false;
        };
        f.bbox = bbox;
        self.refresh_floating(fast);
        true
    }

    /// Recalcula lo transformado y la selección. `fast`: rotación Fast (durante un arrastre de
    /// rotación); si no, la elegida en las opciones.
    pub fn refresh_floating(&mut self, fast: bool) {
        let algorithm = if fast {
            RotationAlgorithm::Fast
        } else {
            self.options.rotation
        };
        let Some(f) = self.floating.as_mut() else {
            return;
        };
        f.render(algorithm);
        self.selection = f.mask();
        self.revision += 1;
    }

    /// Deja lo flotante en su capa (un solo paso de undo) y selecciona lo transformado.
    pub fn commit_floating(&mut self) -> bool {
        let Some(f) = self.floating.take() else {
            return false;
        };
        self.transform_drag = None;
        let cmd = match self.document.artboard(f.artboard) {
            Some(artboard) if !f.unchanged() => Some(f.command(&artboard.sprite)),
            _ => None,
        };
        let applied = match cmd {
            Some(Command::DrawPixels { ref pixels, .. }) if pixels.is_empty() => true,
            Some(cmd) => self.execute_doc(DocCommand::OnArtboard {
                id: f.artboard,
                cmd,
            }),
            None => true,
        };
        self.selection = if applied { f.mask() } else { f.before };
        self.revision += 1;
        true
    }

    /// Descarta lo flotante: todo vuelve a como estaba antes de levantar, sin pasos en el
    /// historial. Si venía de pegar, vuelve la selección de antes del pegado.
    pub fn cancel_floating(&mut self) -> bool {
        let Some(f) = self.floating.take() else {
            return false;
        };
        self.transform_drag = None;
        let s = self.sprite();
        let size = (s.width, s.height);
        self.selection = f.before.filter(|m| (m.width(), m.height()) == size);
        self.revision += 1;
        true
    }

    /// Una vez por frame: confirma si cambió la capa, el frame, el artboard o la herramienta por
    /// un camino que no confirma solo (p. ej. ↑/↓), y cancela si la capa ya no se puede editar o
    /// cambió el tamaño del lienzo.
    pub fn sync_floating(&mut self) {
        let Some(f) = &self.floating else {
            return;
        };
        let valid = self.document.artboard(f.artboard).is_some_and(|a| {
            let s = &a.sprite;
            (s.width, s.height) == f.canvas
                && f.frame < s.frames.len()
                && s.layer(f.layer)
                    .is_some_and(|l| l.kind == LayerKind::Image && !l.locked)
        });
        let same_target = (f.artboard, f.layer, f.frame) == (self.active, self.layer, self.frame);
        let tool_ok = self.tool.is_selection() || self.tool == ToolKind::Move;
        if !valid {
            self.cancel_floating();
        } else if !same_target || !tool_ok || self.editing_mask() {
            self.commit_floating();
        }
    }

    /// Vista previa: el documento con lo flotante aplicado (hueco y estampado) en un clon.
    pub(crate) fn floating_view(&self) -> Option<DocView<'_>> {
        let f = self.floating.as_ref()?;
        let sprite = &self.document.artboard(f.artboard)?.sprite;
        let cmd = DocCommand::OnArtboard {
            id: f.artboard,
            cmd: f.command(sprite),
        };
        let mut doc = self.document.clone();
        let document = match cmd.apply(&mut doc) {
            Ok(_) => Cow::Owned(doc),
            Err(_) => Cow::Borrowed(&self.document),
        };
        Some(DocView {
            document,
            selection: self.selection.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::editor::ToolKind;
    use crate::transform_box::TransformBox;

    fn red() -> PixelValue {
        PixelValue::Rgba(Rgba::rgb(255, 0, 0))
    }

    /// 8×8 con un punto rojo en (1, 1), la selección (0, 0)–(2, 2) y una herramienta de selección.
    fn editor() -> Editor {
        let mut e = Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red())],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.tool = ToolKind::RectSelect;
        e
    }

    fn px(e: &Editor, x: i32, y: i32) -> PixelValue {
        e.sprite().pixel(LayerId(1), 0, x, y)
    }

    /// Lo que muestra el lienzo (con lo flotante aplicado).
    fn shown(e: &Editor, x: i32, y: i32) -> PixelValue {
        e.view().document.artboards[0]
            .sprite
            .pixel(LayerId(1), 0, x, y)
    }

    fn place(e: &mut Editor, bbox: TransformBox) {
        e.floating.as_mut().expect("floating").bbox = bbox;
        e.refresh_floating(false);
    }

    fn moved(e: &mut Editor, dx: i32, dy: i32) {
        let b = e.transform_box().unwrap().translated(dx, dy);
        place(e, b);
    }

    #[test]
    fn lifting_and_moving_only_changes_the_view_until_commit() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 3, 2);
        assert_eq!(px(&e, 1, 1), red(), "the layer is untouched");
        assert_eq!(shown(&e, 1, 1), t);
        assert_eq!(shown(&e, 4, 3), red());
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((3, 2), (5, 4))));
        let version = e.doc_version;
        assert!(e.commit_floating());
        assert_eq!(e.doc_version, version + 1, "a single command");
        assert_eq!(px(&e, 4, 3), red());
        assert_eq!(px(&e, 1, 1), t);
        e.undo();
        assert_eq!(px(&e, 1, 1), red());
        assert_eq!(px(&e, 4, 3), t);
    }

    #[test]
    fn cancel_leaves_no_trace_and_restores_the_selection() {
        let mut e = editor();
        let before = e.selection.clone();
        let version = e.doc_version;
        assert!(e.lift());
        moved(&mut e, 1, 1);
        assert!(e.cancel_floating());
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version);
        assert_eq!(e.selection, before);
        assert_eq!(shown(&e, 1, 1), red());
        assert!(!e.cancel_floating(), "nothing left to cancel");
    }

    #[test]
    fn scaling_doubles_pixels_and_commit_selects_the_result() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        place(&mut e, TransformBox::new(0, 0, 6, 6));
        assert!(e.commit_floating());
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            assert_eq!(px(&e, x, y), red(), "({x}, {y})");
        }
        assert_eq!(px(&e, 1, 1), t);
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((0, 0), (5, 5))));
    }

    #[test]
    fn an_untouched_lift_commits_nothing() {
        let mut e = editor();
        let version = e.doc_version;
        assert!(e.lift());
        assert!(e.commit_floating());
        assert_eq!(e.doc_version, version, "no empty undo step");
    }

    #[test]
    fn changing_layer_commits_and_locking_the_layer_cancels() {
        let mut e = editor();
        e.add_layer(LayerKind::Image);
        let top = e.layer;
        e.select_layer(LayerId(1));
        assert!(e.lift());
        moved(&mut e, 4, 0);
        e.select_layer(top);
        assert!(!e.is_floating());
        assert_eq!(
            px(&e, 5, 1),
            red(),
            "committed on the layer it was lifted from"
        );

        e.select_layer(LayerId(1));
        e.set_selection(Some(Mask::from_rect(8, 8, (4, 0), (6, 2))));
        assert!(e.lift());
        moved(&mut e, 0, 3);
        e.toggle_locked(LayerId(1));
        e.sync_floating();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 5, 1), red(), "nothing moved");
        assert!(!e.lift(), "a locked layer cannot be lifted");
    }

    #[test]
    fn changing_tool_commits_and_undo_while_floating_only_cancels() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 2, 0);
        e.undo();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 1, 1), red(), "the drawing is still there");
        assert!(e.lift());
        moved(&mut e, 2, 0);
        e.set_tool(ToolKind::Pencil);
        assert!(!e.is_floating());
        assert_eq!(px(&e, 3, 1), red());
        e.undo();
        e.undo();
        assert_eq!(
            px(&e, 1, 1),
            t,
            "the next undos undo the move and the drawing"
        );
    }
}
