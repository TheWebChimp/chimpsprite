//! Transformar la selección. Lo seleccionado se levanta una sola vez y flota (se mueve, se escala,
//! se rota) hasta que se confirma, en un solo paso de undo, o se cancela sin dejar rastro. Lo que
//! queda debajo mientras flota no se mezcla ni se lleva en los arrastres siguientes.

use std::borrow::Cow;
use std::cell::OnceCell;

use cs_commands::{Command, DocCommand};
use cs_core::{ArtboardId, Document, LayerId, LayerKind, Mask, PixelValue, Sprite};
use cs_raster::{Block, MAX_BLOCK_SIDE, RotationAlgorithm};

use crate::editor::{DocView, Editor, ToolKind};
use crate::transform_box::{self, TransformBox, TransformHit};

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
    /// Selección espejo: los ejes (vertical y horizontal) y el centro (en medios píxeles). Al
    /// trasladar, lo de cada lado del eje se mueve al revés que lo del otro.
    mirror: Option<(cs_core::SymmetryAxes, (i32, i32))>,
    /// Vista previa (el documento con lo flotante aplicado; `None` si no se pudo aplicar) y el
    /// `doc_version` con el que se armó. Se descarta al cambiar la caja o lo transformado; la capa,
    /// el frame y el artboard no cambian mientras flota.
    preview: OnceCell<(u64, Option<Document>)>,
    /// Veces que se armó la vista previa (para los tests de la caché).
    #[cfg(test)]
    builds: std::cell::Cell<u32>,
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
            mirror: None,
            source,
            origin,
            bbox,
            artboard: target.0,
            layer: target.1,
            frame: target.2,
            from_paste,
            before,
            canvas,
            preview: OnceCell::new(),
            #[cfg(test)]
            builds: Default::default(),
        }
    }

    /// Con la selección espejo y la caja solo trasladada `(dx, dy)`: cada píxel se mueve hacia su
    /// lado del eje (los de un lado `+dx`, los del otro `-dx`; igual en vertical con el eje
    /// horizontal). Lo que está justo sobre el eje no se mueve en ese sentido.
    fn render_mirrored(&mut self) -> bool {
        let Some((axes, center2)) = self.mirror else {
            return false;
        };
        let b = self.bbox;
        let (w, h) = (self.source.width as i32, self.source.height as i32);
        if b.angle != 0.0 || b.width() != w || b.height() != h {
            return false;
        }
        let (dx, dy) = (b.left() - self.origin.0, b.top() - self.origin.1);
        let shift = |c: i32, axis: bool, center: i32, d: i32| {
            // `center` está en medios píxeles: el centro del píxel `c` es `2c + 1`.
            match (axis, (2 * c + 1).cmp(&center)) {
                (false, _) => c + d,
                (true, std::cmp::Ordering::Less) => c + d,
                (true, std::cmp::Ordering::Greater) => c - d,
                (true, std::cmp::Ordering::Equal) => c,
            }
        };
        let moved: Vec<(i32, i32, PixelValue)> = self
            .source
            .points()
            .map(|(x, y, v)| {
                let (cx, cy) = (self.origin.0 + x, self.origin.1 + y);
                (
                    shift(cx, axes.vertical, center2.0, dx),
                    shift(cy, axes.horizontal, center2.1, dy),
                    v,
                )
            })
            .collect();
        let Some(x0) = moved.iter().map(|p| p.0).min() else {
            return false;
        };
        let y0 = moved.iter().map(|p| p.1).min().unwrap_or(0);
        let x1 = moved.iter().map(|p| p.0).max().unwrap_or(x0);
        let y1 = moved.iter().map(|p| p.1).max().unwrap_or(y0);
        let mut block = Block::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
        for (x, y, v) in moved {
            block.set(x - x0, y - y0, Some(v));
        }
        self.rendered = block;
        self.at = (x0, y0);
        self.preview.take();
        true
    }

    /// Escala (con volteos) y rota lo levantado según la caja.
    fn render(&mut self, algorithm: RotationAlgorithm) {
        if self.render_mirrored() {
            self.rendered_with = algorithm;
            return;
        }
        let b = self.bbox;
        let scaled = cs_raster::scale_nearest(&self.source, b.width(), b.height());
        let (rotated, (dx, dy)) = algorithm.rotate(&scaled, b.angle);
        self.rendered = rotated;
        self.at = (b.left() + dx, b.top() + dy);
        self.rendered_with = algorithm;
        self.preview.take();
    }

    /// Documento con lo flotante aplicado (hueco y estampado); `None` si el comando no se aplica.
    fn build_preview(&self, document: &Document) -> Option<Document> {
        #[cfg(test)]
        self.builds.set(self.builds.get() + 1);
        let sprite = &document.artboard(self.artboard)?.sprite;
        let cmd = DocCommand::OnArtboard {
            id: self.artboard,
            cmd: self.command(sprite),
        };
        let mut doc = document.clone();
        cmd.apply(&mut doc).ok().map(|_| doc)
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
    /// Punto local (con decimales) donde se presionó: escalar y rotar se miden desde acá.
    start: (f32, f32),
    /// Píxel local donde se presionó.
    start_pixel: (i32, i32),
    /// El mismo punto pegado a guías/grid: el desplazamiento se mide desde acá.
    start_snapped: (i32, i32),
    /// Caja al presionar: cada movimiento se calcula desde acá.
    start_box: TransformBox,
    /// Se presionó dentro de lo seleccionado.
    inside: bool,
    /// El arrastre ya cambió la caja (y lo seleccionado ya flota).
    moved: bool,
    /// El puntero (sin pegar) ya salió del píxel presionado: no es un clic aunque el ajuste haya
    /// dejado la caja quieta.
    dragged: bool,
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
        let mut floating = Floating::new(source, (x0, y0), target, false, Some(mask), canvas);
        // Con la selección espejo, al trasladar cada lado se mueve como en un espejo.
        floating.mirror = self.move_mirror();
        self.floating = Some(floating);
        self.revision += 1;
        true
    }

    /// Deja `block` flotando con su esquina en `at` (pegar). No deja hueco; si se cancela, vuelve
    /// la selección de antes del pegado.
    pub(crate) fn float_pasted(&mut self, block: Block<PixelValue>, at: (i32, i32)) {
        self.commit_floating();
        let s = self.sprite();
        let canvas = (s.width, s.height);
        let target = (self.active, self.layer, self.frame);
        let before = self.selection.clone();
        self.floating = Some(Floating::new(block, at, target, true, before, canvas));
        self.refresh_floating(false);
    }

    /// Empieza a mover, escalar o rotar lo seleccionado. Se levanta recién al cambiar algo, así
    /// un clic no levanta nada.
    pub fn press_transform(&mut self, hit: TransformHit, world: (f32, f32)) {
        let Some(start_box) = self.transform_box() else {
            return;
        };
        self.stop();
        let start_pixel = self.to_local(world);
        let start_snapped = self.to_local(self.snap_world(world));
        let inside = self
            .selection
            .as_ref()
            .is_some_and(|m| m.contains(start_pixel.0, start_pixel.1));
        self.transform_drag = Some(TransformDrag {
            hit,
            start: self.local_point(world),
            start_pixel,
            start_snapped,
            start_box,
            inside,
            moved: false,
            dragged: false,
        });
        self.revision += 1;
    }

    pub(crate) fn drag_transform(&mut self, world: (f32, f32)) {
        let Some(mut drag) = self.transform_drag else {
            return;
        };
        if !drag.dragged && self.to_local(world) != drag.start_pixel {
            drag.dragged = true;
            self.transform_drag = Some(drag);
        }
        let p = self.local_point(world);
        let shift = self.options.constrain;
        let bbox = match drag.hit {
            TransformHit::Move => {
                // Pega a guías/grid como Move: el desplazamiento entre los dos puntos pegados.
                let pixel = self.to_local(self.snap_world(world));
                drag.start_box.translated(
                    pixel.0 - drag.start_snapped.0,
                    pixel.1 - drag.start_snapped.1,
                )
            }
            TransformHit::Handle(handle) => {
                transform_box::resize(&drag.start_box, handle, drag.start, p, shift)
            }
            TransformHit::Rotate => transform_box::rotate(&drag.start_box, drag.start, p, shift),
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
        // Escalando o rotando se usa Fast (RotSprite es lento para cada movimiento); al soltar se
        // calcula la rotación elegida una vez.
        self.set_floating_box(bbox, drag.hit != TransformHit::Move);
    }

    /// Punto de mundo → coordenadas locales del artboard activo, con decimales.
    fn local_point(&self, world: (f32, f32)) -> (f32, f32) {
        let (x, y) = self.artboard().position;
        (world.0 - x as f32, world.1 - y as f32)
    }

    /// El lienzo muestra los 8 handles (y la franja de rotación).
    pub fn shows_handles(&self) -> bool {
        self.can_transform() && !self.is_pending()
    }

    /// Qué se está arrastrando (para el cursor).
    pub fn transform_drag_hit(&self) -> Option<TransformHit> {
        self.transform_drag.map(|d| d.hit)
    }

    /// Hay un arrastre de transformación en curso.
    pub fn is_transforming(&self) -> bool {
        self.transform_drag.is_some()
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
        if drag.dragged {
            // Arrastre retenido por el ajuste: no cambió nada, pero tampoco es un clic.
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
        // Ni lo escalado ni lo rotado pueden pasar del límite.
        let (sw, sh) = (bbox.width().unsigned_abs(), bbox.height().unsigned_abs());
        let (w, h) = cs_raster::rotated_size(sw, sh, bbox.angle);
        if [sw, sh, w, h].iter().any(|&side| side > MAX_BLOCK_SIDE) {
            return false;
        }
        let Some(f) = self.floating.as_mut() else {
            return false;
        };
        let (dx, dy) = (bbox.x0 - f.bbox.x0, bbox.y0 - f.bbox.y0);
        // (Con el espejo, trasladar cambia lo transformado: no sirve correrlo.)
        if f.mirror.is_none() && bbox == f.bbox.translated(dx, dy) {
            // Solo se mueve: lo transformado (la caché) sirve igual, corrido.
            f.bbox = bbox;
            f.at = (f.at.0 + dx, f.at.1 + dy);
            f.preview.take();
            self.selection = f.mask();
            self.revision += 1;
            return true;
        }
        f.bbox = bbox;
        self.refresh_floating(fast);
        true
    }

    /// Voltea en horizontal (`horizontal`) o en vertical, como Edit → Flip de Aseprite. Con algo
    /// seleccionado o flotando, lo levanta y lo deja volteado dentro de su caja (sigue flotando con
    /// las herramientas que transforman; con las demás se confirma enseguida). Sin selección,
    /// voltea la capa entera en el frame actual, en un paso de undo.
    pub fn flip(&mut self, horizontal: bool) -> bool {
        if !self.clip_layer_ok(true) {
            return false;
        }
        if self.floating.is_none() && self.selection.is_none() {
            return self.flip_layer(horizontal);
        }
        if !self.lift() {
            return false;
        }
        self.stop();
        self.transform_drag = None;
        let Some(f) = self.floating.as_mut() else {
            return false;
        };
        let b = &mut f.bbox;
        if horizontal {
            std::mem::swap(&mut b.x0, &mut b.x1);
        } else {
            std::mem::swap(&mut b.y0, &mut b.y1);
        }
        // Espejar una caja rotada en pantalla = espejarla sin rotar y rotar al revés.
        b.angle = -b.angle;
        self.refresh_floating(false);
        if !self.can_transform() {
            self.commit_floating();
        }
        true
    }

    /// Voltea la capa actual entera (todo el lienzo) en el frame actual.
    fn flip_layer(&mut self, horizontal: bool) -> bool {
        let sprite = self.sprite();
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for y in 0..h {
            for x in 0..w {
                let (sx, sy) = if horizontal {
                    (w - 1 - x, y)
                } else {
                    (x, h - 1 - y)
                };
                pixels.push((x, y, sprite.pixel(self.layer, self.frame, sx, sy)));
            }
        }
        self.execute(Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        })
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

    /// Texto para la barra de estado mientras flota: tamaño, ángulo y teclas.
    pub fn floating_status(&self) -> Option<String> {
        let b = self.floating.as_ref()?.bbox;
        let degrees = (b.angle.to_degrees().round() as i32).rem_euclid(360);
        Some(format!(
            "{}×{}  {}°  Enter to apply · Esc to cancel",
            b.width().unsigned_abs(),
            b.height().unsigned_abs(),
            degrees
        ))
    }

    /// Deja lo flotante en su capa (un solo paso de undo) y selecciona lo transformado. Si el
    /// destino ya no sirve (otro tamaño de lienzo, capa bloqueada o borrada…), cancela.
    pub fn commit_floating(&mut self) -> bool {
        match &self.floating {
            None => return false,
            Some(f) if !self.floating_valid(f) => return self.cancel_floating(),
            Some(_) => {}
        }
        let Some(f) = self.floating.take() else {
            return false;
        };
        self.transform_drag = None;
        let cmd = match self.document.artboard(f.artboard) {
            Some(artboard) if !f.unchanged() => Some(f.command(&artboard.sprite)),
            _ => None,
        };
        // `apply_doc` y no `execute_doc`: este es el comando que confirma, no hay nada antes.
        let applied = match cmd {
            Some(Command::DrawPixels { ref pixels, .. }) if pixels.is_empty() => true,
            Some(cmd) => self.apply_doc(DocCommand::OnArtboard {
                id: f.artboard,
                cmd,
            }),
            None => true,
        };
        self.selection = if applied {
            f.mask()
        } else {
            self.selection_before(f.before)
        };
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
        self.selection = self.selection_before(f.before);
        self.revision += 1;
        true
    }

    /// La selección de antes de levantar, si todavía tiene el tamaño del lienzo.
    fn selection_before(&self, before: Option<Mask>) -> Option<Mask> {
        let s = self.sprite();
        let size = (s.width, s.height);
        before.filter(|m| (m.width(), m.height()) == size)
    }

    /// Delete mientras flota. Lo pegado se descarta (no borra lo de debajo); lo levantado de la
    /// capa deja solo el hueco en el origen, en un paso de undo, y la selección queda en el hueco.
    /// `false` si no flotaba nada.
    pub(crate) fn delete_floating(&mut self) -> bool {
        let valid = match &self.floating {
            None => return false,
            Some(f) => self.floating_valid(f),
        };
        if !valid || self.floating.as_ref().is_some_and(|f| f.from_paste) {
            return self.cancel_floating();
        }
        let Some(f) = self.floating.take() else {
            return false;
        };
        self.transform_drag = None;
        let clear = self
            .document
            .artboard(f.artboard)
            .map(|a| a.sprite.color_mode.transparent());
        if let Some(clear) = clear {
            let (ox, oy) = f.origin;
            let pixels = f
                .source
                .points()
                .map(|(x, y, _)| (ox + x, oy + y, clear))
                .collect();
            self.apply_doc(DocCommand::OnArtboard {
                id: f.artboard,
                cmd: Command::DrawPixels {
                    layer: f.layer,
                    frame: f.frame,
                    pixels,
                },
            });
        }
        self.selection = self.selection_before(f.before);
        self.revision += 1;
        true
    }

    /// El destino de lo flotante sigue igual: mismo tamaño de lienzo, el frame existe y la capa
    /// es de imagen y no está bloqueada.
    fn floating_valid(&self, f: &Floating) -> bool {
        self.document.artboard(f.artboard).is_some_and(|a| {
            let s = &a.sprite;
            (s.width, s.height) == f.canvas
                && f.frame < s.frames.len()
                && s.layer(f.layer)
                    .is_some_and(|l| l.kind == LayerKind::Image && !l.locked)
        })
    }

    /// Esc hace una sola cosa por pulsación. Primero descarta el gesto o el texto a medio hacer,
    /// después cancela lo flotante (sin deseleccionar) y, si no hay nada de eso, deselecciona.
    /// Esc: corta el gesto a medias; si flota algo pegado, lo descarta; si flota lo seleccionado,
    /// lo deja donde quedó (lo confirma, la selección sigue ahí y Ctrl+Z lo deshace); si no,
    /// deselecciona.
    pub fn escape(&mut self) {
        if self.is_pending() {
            self.cancel_gesture();
        } else if self.floating.as_ref().is_some_and(|f| f.from_paste) {
            self.cancel_floating();
        } else if self.is_floating() {
            self.commit_floating();
        } else {
            self.deselect();
        }
    }

    /// Las flechas con algo seleccionado: mueven lo seleccionado `(dx, dy)` píxeles (lo levantan
    /// si hace falta). `false` si no se puede transformar.
    pub fn nudge(&mut self, dx: i32, dy: i32) -> bool {
        if !self.can_transform() || !self.lift() {
            return false;
        }
        let Some(bbox) = self.floating.as_ref().map(|f| f.bbox.translated(dx, dy)) else {
            return false;
        };
        self.set_floating_box(bbox, false)
    }

    /// Una vez por frame: confirma si cambió la capa, el frame, el artboard o la herramienta por
    /// un camino que no confirma solo (p. ej. ↑/↓), y cancela si la capa ya no se puede editar o
    /// cambió el tamaño del lienzo.
    pub fn sync_floating(&mut self) {
        let version = self.doc_version;
        let Some(f) = &mut self.floating else {
            return;
        };
        // Una vista previa de otra versión del documento ya no sirve.
        if f.preview.get().is_some_and(|(v, _)| *v != version) {
            f.preview.take();
        }
        let Some(f) = &self.floating else {
            return;
        };
        let valid = self.floating_valid(f);
        let same_target = (f.artboard, f.layer, f.frame) == (self.active, self.layer, self.frame);
        let tool_ok = self.tool.is_selection() || self.tool == ToolKind::Move;
        if !valid {
            self.cancel_floating();
        } else if !same_target || !tool_ok || self.editing_mask() {
            self.commit_floating();
        }
    }

    /// Vista previa: el documento con lo flotante aplicado (hueco y estampado) en un clon. Se
    /// arma una vez y se reusa en los frames siguientes hasta que algo cambia.
    pub(crate) fn floating_view(&self) -> Option<DocView<'_>> {
        let f = self.floating.as_ref()?;
        self.document.artboard(f.artboard)?;
        let version = self.doc_version;
        let cached = f
            .preview
            .get_or_init(|| (version, f.build_preview(&self.document)));
        let document = match cached {
            (v, Some(doc)) if *v == version => Cow::Borrowed(doc),
            (v, None) if *v == version => Cow::Borrowed(&self.document),
            // Otra versión del documento: se arma sin guardar (`sync_floating` la descarta).
            _ => match f.build_preview(&self.document) {
                Some(doc) => Cow::Owned(doc),
                None => Cow::Borrowed(&self.document),
            },
        };
        Some(DocView {
            document,
            selection: self.selection.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use cs_commands::Command;
    use cs_core::{ColorMode, LayerId, LayerKind, Mask, PixelValue, Rgba, Sprite};

    use super::*;
    use crate::editor::ToolKind;
    use crate::transform_box::{Handle, TransformBox, TransformHit};

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

    /// `editor()` con el punto rojo en `at` (dentro de la selección de 3×3) en vez de en (1, 1).
    fn editor_with_dot(at: (i32, i32)) -> Editor {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, t), (at.0, at.1, red())],
        });
        e
    }

    #[test]
    fn flipping_a_selection_leaves_it_floating_and_flipped_in_its_box() {
        let mut e = editor_with_dot((0, 1));
        let t = e.sprite().color_mode.transparent();
        assert!(e.flip(true));
        assert!(e.is_floating(), "with a selection tool it keeps floating");
        assert_eq!(e.transform_box().map(|b| (b.left(), b.top())), Some((0, 0)));
        assert!(e.commit_floating());
        assert_eq!(px(&e, 2, 1), red());
        assert_eq!(px(&e, 0, 1), t);
        // Un solo paso de undo.
        e.undo();
        assert_eq!(px(&e, 0, 1), red());
        assert_eq!(px(&e, 2, 1), t);
    }

    #[test]
    fn flipping_vertically_mirrors_top_and_bottom() {
        let mut e = editor_with_dot((1, 0));
        assert!(e.flip(false));
        e.commit_floating();
        assert_eq!(px(&e, 1, 2), red());
        assert_eq!(px(&e, 1, 0), e.sprite().color_mode.transparent());
    }

    #[test]
    fn flipping_twice_gets_back_where_it_was() {
        let mut e = editor_with_dot((0, 1));
        e.flip(true);
        e.flip(true);
        e.commit_floating();
        assert_eq!(px(&e, 0, 1), red());
    }

    #[test]
    fn with_a_painting_tool_the_flip_applies_right_away() {
        let mut e = editor_with_dot((0, 1));
        e.tool = ToolKind::Pencil;
        assert!(e.flip(true));
        assert!(!e.is_floating());
        assert_eq!(px(&e, 2, 1), red());
        assert!(e.selection.is_some(), "the selection stays");
    }

    #[test]
    fn without_a_selection_the_whole_layer_flips() {
        let mut e = editor();
        e.set_selection(None);
        assert!(e.flip(true));
        assert_eq!(px(&e, 6, 1), red());
        assert_eq!(px(&e, 1, 1), e.sprite().color_mode.transparent());
        e.undo();
        assert_eq!(px(&e, 1, 1), red());
        assert!(e.flip(false));
        assert_eq!(px(&e, 1, 6), red());
    }

    #[test]
    fn a_locked_layer_does_not_flip() {
        let mut e = editor();
        e.set_selection(None);
        e.toggle_locked(LayerId(1));
        assert!(!e.flip(true));
        assert_eq!(px(&e, 1, 1), red());
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
    fn escape_cancels_the_gesture_then_applies_the_floating_content_then_deselects() {
        let mut e = editor();
        // Un lazo poligonal a medias: Esc lo descarta y la selección sigue.
        e.set_tool(ToolKind::PolygonLasso);
        e.pointer_press((6.5, 6.5), false, false, 0.1);
        e.pointer_release();
        assert!(e.is_pending());
        e.escape();
        assert!(!e.is_pending());
        assert!(e.selection.is_some());
        // Flotando: Esc lo deja donde se movió, sin deseleccionar.
        e.set_tool(ToolKind::RectSelect);
        assert!(e.lift());
        moved(&mut e, 2, 2);
        e.escape();
        assert!(!e.is_floating());
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((2, 2), (4, 4))));
        assert_eq!(px(&e, 3, 3), red());
        // Sin nada más: Esc deselecciona, como Deselect.
        e.escape();
        assert!(e.selection.is_none());
        assert!(e.can_reselect());
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
        // Bloquear con el comando (un cambio de propiedades) confirma antes.
        e.toggle_locked(LayerId(1));
        assert!(!e.is_floating());
        assert_eq!(px(&e, 5, 4), red(), "committed before locking");
        e.toggle_locked(LayerId(1));

        // Bloqueada por un camino que no confirma: el frame siguiente cancela.
        e.set_selection(Some(Mask::from_rect(8, 8, (4, 3), (6, 5))));
        assert!(e.lift());
        moved(&mut e, 0, -3);
        let lock = cs_commands::DocCommand::OnArtboard {
            id: e.active,
            cmd: Command::SetLayerProps {
                id: LayerId(1),
                props: cs_commands::LayerProps {
                    locked: Some(true),
                    ..Default::default()
                },
            },
        };
        e.history.execute(&mut e.document, lock).unwrap();
        e.sync_floating();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 5, 4), red(), "nothing moved");
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

    #[test]
    fn dragging_a_corner_handle_scales_and_shift_keeps_the_proportion() {
        let mut e = editor();
        e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
        e.pointer_drag((6.0, 4.0));
        e.pointer_release();
        assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 4)));
        assert!(e.cancel_floating());
        e.options.constrain = true;
        e.press_transform(TransformHit::Handle(Handle::SE), (3.0, 3.0));
        e.pointer_drag((6.0, 4.0));
        e.pointer_release();
        assert_eq!(e.transform_box(), Some(TransformBox::new(0, 0, 6, 6)));
    }

    #[test]
    fn dragging_around_the_center_rotates_and_shift_snaps_to_15_degrees() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        // 4×4 con el rojo en su esquina superior izquierda; centro (3, 3).
        e.set_selection(Some(Mask::from_rect(8, 8, (1, 1), (4, 4))));
        e.press_transform(TransformHit::Rotate, (6.0, 3.0));
        e.pointer_drag((3.0, 6.0)); // de la derecha a abajo: +90°
        assert_eq!(
            e.floating().unwrap().rendered_with,
            RotationAlgorithm::Fast,
            "Fast while dragging"
        );
        e.pointer_release();
        assert_eq!(
            e.floating().unwrap().rendered_with,
            RotationAlgorithm::RotSprite,
            "the chosen rotation on release"
        );
        let b = e.transform_box().unwrap();
        assert!((b.angle - FRAC_PI_2).abs() < 1e-4, "{}", b.angle);
        assert!(e.commit_floating());
        assert_eq!(
            px(&e, 4, 1),
            red(),
            "the top-left corner went to the top-right"
        );
        assert_eq!(px(&e, 1, 1), t);
        e.options.constrain = true;
        e.press_transform(TransformHit::Rotate, (6.0, 3.0));
        e.pointer_drag((3.0 + 3.0 * 0.9, 3.0 + 3.0 * 0.44)); // unos 26°
        e.pointer_release();
        let b = e.transform_box().unwrap();
        assert!((b.angle - PI / 6.0).abs() < 1e-4, "{}", b.angle);
    }

    #[test]
    fn moving_a_rotated_block_shifts_the_same_result() {
        let mut e = editor();
        e.press_transform(TransformHit::Rotate, (4.0, 1.5));
        e.pointer_drag((1.5 + 2.5 * 0.8, 1.5 + 2.5 * 0.6)); // unos 37°
        e.pointer_release();
        let before = e.floating().unwrap().clone();
        assert_eq!(before.rendered_with, RotationAlgorithm::RotSprite);
        e.press_transform(TransformHit::Move, (1.5, 1.5));
        e.pointer_drag((3.5, 2.5));
        let after = e.floating().unwrap();
        assert_eq!(
            after.rendered_with,
            RotationAlgorithm::RotSprite,
            "not Fast"
        );
        assert_eq!(after.rendered, before.rendered, "the same pixels");
        assert_eq!(after.at, (before.at.0 + 2, before.at.1 + 1));
        e.pointer_release();
    }

    #[test]
    fn status_shows_size_angle_and_keys_while_floating() {
        let mut e = editor();
        assert_eq!(e.floating_status(), None);
        assert!(e.lift());
        place(
            &mut e,
            TransformBox {
                x0: 6,
                y0: 0,
                x1: 0,
                y1: 3,
                angle: FRAC_PI_2,
            },
        );
        assert_eq!(
            e.floating_status().as_deref(),
            Some("6×3  90°  Enter to apply · Esc to cancel")
        );
    }

    #[test]
    fn changing_the_rotation_option_recomputes_the_floating_content() {
        let mut e = editor();
        // Lift, rotate about 30 degrees by dragging and release: renders with RotSprite.
        e.press_transform(TransformHit::Rotate, (4.0, 1.5));
        e.pointer_drag((1.5 + 2.5 * 0.866, 1.5 + 2.5 * 0.5));
        e.pointer_release();
        assert_eq!(
            e.floating().unwrap().rendered_with,
            RotationAlgorithm::RotSprite
        );
        // Switching the option re-renders the floating content with the new algorithm.
        e.options.rotation = RotationAlgorithm::Fast;
        e.refresh_floating(false);
        assert_eq!(e.floating().unwrap().rendered_with, RotationAlgorithm::Fast);
    }

    #[test]
    fn a_handle_stops_at_the_size_limit() {
        let mut e = editor();
        e.press_transform(TransformHit::Handle(Handle::E), (3.0, 1.5));
        e.pointer_drag((5000.0, 1.5));
        assert_eq!(
            e.transform_box(),
            Some(TransformBox::new(0, 0, 3, 3)),
            "too wide: keeps the last valid size"
        );
        e.pointer_drag((100.0, 1.5));
        assert_eq!(e.transform_box().unwrap().width(), 100);
        e.pointer_release();
    }

    fn pxf(e: &Editor, frame: usize, x: i32, y: i32) -> PixelValue {
        e.sprite().pixel(LayerId(1), frame, x, y)
    }

    /// Tres frames con el punto rojo en (1, 1) (copias independientes) y el frame 0 activo.
    fn three_frames() -> Editor {
        let mut e = editor();
        e.duplicate_frame();
        e.duplicate_frame();
        e.set_frame(0);
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e
    }

    #[test]
    fn removing_the_frame_while_floating_commits_on_the_lifted_frame_first() {
        let mut e = three_frames();
        let t = e.sprite().color_mode.transparent();
        e.set_frame(1);
        assert!(e.lift());
        moved(&mut e, 3, 3);
        e.remove_frame();
        e.sync_floating();
        e.commit_floating();
        assert!(!e.is_floating());
        assert_eq!(e.frame_count(), 2);
        for f in 0..2 {
            assert_eq!(pxf(&e, f, 1, 1), red(), "frame {f} untouched");
            assert_eq!(pxf(&e, f, 4, 4), t, "frame {f} untouched");
        }
        e.undo();
        assert_eq!(e.frame_count(), 3);
        assert_eq!(pxf(&e, 1, 4, 4), red(), "the move was applied on its frame");
        assert_eq!(pxf(&e, 1, 1, 1), t);
    }

    #[test]
    fn moving_the_frame_while_floating_keeps_the_content_on_the_lifted_frame() {
        let mut e = three_frames();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 3, 3);
        e.move_frame(0, 2);
        e.sync_floating();
        e.commit_floating();
        assert!(!e.is_floating());
        // El frame levantado ahora es el 2.
        assert_eq!(pxf(&e, 2, 4, 4), red());
        assert_eq!(pxf(&e, 2, 1, 1), t);
        for f in 0..2 {
            assert_eq!(pxf(&e, f, 1, 1), red(), "frame {f} untouched");
            assert_eq!(pxf(&e, f, 4, 4), t, "frame {f} untouched");
        }
    }

    #[test]
    fn a_palette_change_while_floating_commits_first() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 3, 3);
        let mut colors = e.sprite().palette.colors().to_vec();
        colors.reverse();
        assert!(e.apply_palette(cs_core::Palette::new(colors)));
        assert!(!e.is_floating());
        assert_eq!(px(&e, 4, 4), red());
        assert_eq!(px(&e, 1, 1), t);
        e.undo(); // la paleta
        e.undo(); // el movimiento
        assert_eq!(px(&e, 1, 1), red());
        assert_eq!(px(&e, 4, 4), t);
    }

    #[test]
    fn the_preview_is_cached_until_the_box_or_the_algorithm_changes() {
        let mut e = editor();
        assert!(e.lift());
        moved(&mut e, 1, 0);
        let builds = |e: &Editor| e.floating().unwrap().builds.get();
        let start = builds(&e);
        e.view();
        e.view();
        assert_eq!(builds(&e), start + 1, "an idle frame reuses the preview");
        let b = e.transform_box().unwrap().translated(1, 1);
        assert!(e.set_floating_box(b, false));
        e.view();
        e.view();
        assert_eq!(builds(&e), start + 2, "moving rebuilds it once");
        assert_eq!(shown(&e, 3, 2), red());
        e.options.rotation = RotationAlgorithm::Fast;
        e.refresh_floating(false);
        e.view();
        assert_eq!(builds(&e), start + 3, "another algorithm rebuilds it");
    }

    #[test]
    fn commit_after_a_canvas_resize_cancels_without_a_wrong_size_selection() {
        let mut e = editor();
        assert!(e.lift());
        moved(&mut e, 1, 1);
        let version = e.doc_version;
        // Un cambio de tamaño que no pasa por `execute_doc` (no confirma antes).
        let resize = cs_commands::DocCommand::OnArtboard {
            id: e.active,
            cmd: Command::ResizeCanvas {
                width: 16,
                height: 16,
                offset: (0, 0),
            },
        };
        e.history.execute(&mut e.document, resize).unwrap();
        assert!(e.commit_floating());
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version, "nothing applied");
        assert!(
            e.selection
                .as_ref()
                .is_none_or(|m| (m.width(), m.height()) == (16, 16)),
            "no selection of the old size"
        );
        assert_eq!(px(&e, 1, 1), red());
    }

    #[test]
    fn delete_while_floating_a_paste_discards_it_without_erasing_below() {
        let mut e = editor();
        let version = e.doc_version;
        // Un pegado transparente encima del punto rojo.
        let clip = crate::clipboard::Clip::from_rgba(3, 3, &[0; 3 * 3 * 4]).unwrap();
        e.paste(&clip);
        assert!(e.is_floating());
        e.clear_selection_content();
        assert!(!e.is_floating());
        assert_eq!(px(&e, 1, 1), red(), "the artwork under the paste stays");
        assert_eq!(e.doc_version, version, "no undo step");
    }

    #[test]
    fn delete_while_floating_lifted_content_only_leaves_the_hole_in_one_step() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        assert!(e.lift());
        moved(&mut e, 3, 3);
        let version = e.doc_version;
        e.clear_selection_content();
        assert!(!e.is_floating());
        assert_eq!(e.doc_version, version + 1, "one undo step");
        assert_eq!(px(&e, 1, 1), t, "the lifted pixels are gone");
        assert_eq!(px(&e, 4, 4), t, "nothing stamped");
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((0, 0), (2, 2))), "the selection is the hole");
        e.undo();
        assert_eq!(px(&e, 1, 1), red());
    }

    #[test]
    fn indexed_lift_move_and_commit_keep_the_indices() {
        let mut e = Editor::with_sprite(Sprite::new(
            8,
            8,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, PixelValue::Index(5)), (2, 1, PixelValue::Index(9))],
        });
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.tool = ToolKind::RectSelect;
        assert!(e.lift());
        moved(&mut e, 3, 2);
        assert!(e.commit_floating());
        assert_eq!(px(&e, 4, 3), PixelValue::Index(5));
        assert_eq!(px(&e, 5, 3), PixelValue::Index(9));
        assert_eq!(px(&e, 1, 1), PixelValue::Index(0));
    }

    #[test]
    fn undo_of_a_move_whose_hole_and_stamp_overlap_restores_everything() {
        let mut e = editor();
        let t = e.sprite().color_mode.transparent();
        let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, blue), (2, 2, blue)],
        });
        let before: Vec<PixelValue> = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .map(|(x, y)| px(&e, x, y))
            .collect();
        assert!(e.lift());
        moved(&mut e, 1, 0); // la mitad del bloque cae sobre su propio hueco
        assert!(e.commit_floating());
        assert_eq!(px(&e, 0, 0), t);
        assert_eq!(px(&e, 1, 0), blue);
        assert_eq!(px(&e, 2, 1), red());
        assert_eq!(px(&e, 3, 2), blue);
        e.undo();
        let after: Vec<PixelValue> = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .map(|(x, y)| px(&e, x, y))
            .collect();
        assert_eq!(after, before);
    }

    #[test]
    fn copy_while_floating_applies_it_first() {
        let mut e = editor();
        assert!(e.lift());
        moved(&mut e, 3, 3);
        let clip = e.copy().expect("something selected");
        assert!(!e.is_floating(), "copy confirms first");
        assert_eq!(px(&e, 4, 4), red());
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (2, 2))));
        e.paste(&clip);
        assert_eq!(
            e.transform_box(),
            Some(TransformBox::new(3, 3, 3, 3)),
            "copied from where it was moved"
        );
    }

    #[test]
    fn handles_show_only_where_the_clipboard_works() {
        let mut e = editor();
        assert!(e.shows_handles());
        e.set_tool(ToolKind::Pencil);
        assert!(!e.shows_handles(), "only selection tools and Move");
        e.set_tool(ToolKind::Move);
        assert!(e.shows_handles());
        e.toggle_locked(LayerId(1));
        assert!(!e.shows_handles(), "not on a locked layer");
    }
}
