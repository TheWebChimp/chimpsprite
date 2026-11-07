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

mod mirror;
pub(crate) use mirror::MirrorFloat;

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
    /// El nombre del paso al confirmar: "Paste", "Move" o lo último que le hizo algo más que
    /// moverlo ("Transform", "Rotate", "Flip Horizontal"…).
    label: &'static str,
    /// Selección de antes de levantar; cancelar vuelve a ella.
    before: Option<Mask>,
    /// Tamaño del lienzo al levantar; si cambia, se cancela.
    canvas: (u32, u32),
    /// Resultado transformado y su esquina en el lienzo (caché hasta el siguiente cambio).
    rendered: Block<PixelValue>,
    at: (i32, i32),
    /// Rotación con la que se calculó `rendered`.
    rendered_with: RotationAlgorithm,
    /// Selección espejo: los reflejos de lo primario (`source`) y las piezas sobre los ejes, que
    /// se transforman en espejo con la caja.
    mirror: Option<Box<MirrorFloat>>,
    /// Vista previa (el documento con lo flotante aplicado; `None` si no se pudo aplicar) y el
    /// `doc_version` con el que se armó. Se descarta al cambiar la caja o lo transformado; la capa,
    /// el frame y el artboard no cambian mientras flota.
    preview: OnceCell<(u64, Option<Document>)>,
    /// Veces que se armó la vista previa (para los tests de la caché).
    #[cfg(test)]
    builds: std::cell::Cell<u32>,
}

/// El nombre de Edit → Flip en el historial.
fn flip_label(horizontal: bool) -> &'static str {
    if horizontal {
        "Flip Horizontal"
    } else {
        "Flip Vertical"
    }
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
            label: if from_paste { "Paste" } else { "Move" },
            before,
            canvas,
            preview: OnceCell::new(),
            #[cfg(test)]
            builds: Default::default(),
        }
    }

    /// Escala (con volteos) y rota lo levantado según la caja.
    fn render(&mut self, algorithm: RotationAlgorithm) {
        if let Some(mirror) = &self.mirror {
            let (w, h) = (self.source.width, self.source.height);
            let start = TransformBox::new(self.origin.0, self.origin.1, w, h);
            (self.rendered, self.at) = mirror.render(&self.source, start, self.bbox, algorithm);
            self.rendered_with = algorithm;
            self.preview.take();
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

    /// Los píxeles levantados (el hueco): lo seleccionado entero, también con el espejo.
    fn hole(&self) -> Vec<(i32, i32)> {
        match &self.mirror {
            Some(mirror) => mirror.hole.clone(),
            None => {
                let (ox, oy) = self.origin;
                self.source
                    .points()
                    .map(|(x, y, _)| (ox + x, oy + y))
                    .collect()
            }
        }
    }

    /// Comando que deja lo flotante en la capa: el hueco en el origen (si no viene de pegar) y el
    /// estampado, sin borrar con lo transparente.
    fn command(&self, sprite: &Sprite) -> Command {
        let clear = sprite.color_mode.transparent();
        let mut pixels = Vec::new();
        if !self.from_paste {
            pixels.extend(self.hole().into_iter().map(|(x, y)| (x, y, clear)));
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
    /// Con el espejo, el reflejo que se agarró: el arrastre se lleva con su inversa, así lo
    /// agarrado sigue al mouse y lo primario se mueve en espejo.
    mirror_op: cs_raster::MirrorOp,
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
    /// o flotando. La selección de las caras del modo Texture no se transforma: con Move se
    /// mueven sus UVs.
    pub fn can_transform(&self) -> bool {
        (self.tool.is_selection() || self.tool == ToolKind::Move)
            && self.clip_layer_ok(true)
            && (self.floating.is_some()
                || (self.selection.is_some() && !self.selection_is_face_mask()))
    }

    /// Caja de lo flotante o, si no flota nada, el rectángulo que contiene la selección (con la
    /// selección espejo, el de lo primario).
    pub fn transform_box(&self) -> Option<TransformBox> {
        if let Some(f) = &self.floating {
            return Some(f.bbox);
        }
        let bounds = match self.mirror_plan() {
            Some(plan) => plan.bounds(),
            None => self.selection.as_ref()?.bounds(),
        };
        let ((x0, y0), (x1, y1)) = bounds?;
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
        let plan = self.mirror_plan();
        let sprite = self.sprite();
        let canvas = (sprite.width, sprite.height);
        let target = (self.active, self.layer, self.frame);
        let (layer, frame) = (self.layer, self.frame);
        let sample = |(x, y): (i32, i32)| sprite.pixel(layer, frame, x, y);
        // Con la selección espejo, lo primario flota con la caja y lo demás en espejo.
        let mirrored = plan.and_then(|plan| {
            let ops = cs_raster::mirror_ops(self.symmetry().axes);
            let clear = sprite.color_mode.transparent();
            MirrorFloat::lift(&plan, mask.points().collect(), clear, ops, sample)
        });
        let floating = match mirrored {
            Some((source, origin, mirror)) => {
                let mut f = Floating::new(source, origin, target, false, Some(mask), canvas);
                f.mirror = Some(Box::new(mirror));
                f.render(RotationAlgorithm::Fast);
                f
            }
            None => {
                let mut source = Block::new((x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32);
                for (x, y) in mask.points() {
                    source.set(x - x0, y - y0, Some(sample((x, y))));
                }
                Floating::new(source, (x0, y0), target, false, Some(mask), canvas)
            }
        };
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
        let start_pixel = self.to_local(world);
        let inside = self
            .selection
            .as_ref()
            .is_some_and(|m| m.contains(start_pixel.0, start_pixel.1));
        // Selección espejo: sin levantar, lo agarrado pasa a ser lo primario; flotando, el
        // arrastre sobre un reflejo se lleva con su inversa.
        let mut mirror_op = cs_raster::MirrorOp::IDENTITY;
        if hit == TransformHit::Move && inside {
            match self.floating.as_ref() {
                None => self.mirror_anchor = Some((2 * start_pixel.0 + 1, 2 * start_pixel.1 + 1)),
                Some(f) => {
                    if let Some(mirror) = &f.mirror {
                        let b = f.bbox;
                        mirror_op = mirror.op_at(start_pixel, (b.x0 + b.x1, b.y0 + b.y1));
                    }
                }
            }
        }
        let Some(start_box) = self.transform_box() else {
            return;
        };
        self.stop();
        let start_snapped = self.to_local(self.snap_world(world));
        self.transform_drag = Some(TransformDrag {
            hit,
            start: self.local_point(world),
            start_pixel,
            start_snapped,
            start_box,
            inside,
            moved: false,
            dragged: false,
            mirror_op,
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
                let delta = (
                    pixel.0 - drag.start_snapped.0,
                    pixel.1 - drag.start_snapped.1,
                );
                let (dx, dy) = drag.mirror_op.inverse().linear(delta);
                drag.start_box.translated(dx, dy)
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
        if drag.hit != TransformHit::Move {
            self.name_floating("Transform");
        }
    }

    /// El nombre del paso que dejará lo flotante al confirmarse (lo pegado sigue siendo "Paste").
    fn name_floating(&mut self, label: &'static str) {
        if let Some(f) = self.floating.as_mut().filter(|f| !f.from_paste) {
            f.label = label;
        }
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
        self.name_floating(flip_label(horizontal));
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

    /// Edit → Rotate: gira lo seleccionado (sin selección, la capa entera) y lo deja flotando
    /// como Flip.
    pub fn rotate_content(&mut self, turn: crate::edit_ops::Turn) -> bool {
        if !self.clip_layer_ok(true) {
            return false;
        }
        if self.floating.is_none() && self.selection.is_none() {
            self.select_all();
        }
        if !self.lift() {
            return false;
        }
        self.stop();
        self.transform_drag = None;
        self.name_floating("Rotate");
        let Some(f) = self.floating.as_mut() else {
            return false;
        };
        f.bbox.angle += turn.angle();
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
        self.execute_named(
            Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            },
            flip_label(horizontal),
        )
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
            Some(cmd) => {
                let applied = self.apply_doc(DocCommand::OnArtboard {
                    id: f.artboard,
                    cmd,
                });
                if applied {
                    self.history.rename_last(f.label);
                }
                applied
            }
            None => true,
        };
        // Lo confirmado deja su propia selección (ya no la de las caras del modo Texture).
        if applied {
            self.face_mask_replaced();
        }
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
            let pixels = f.hole().into_iter().map(|(x, y)| (x, y, clear)).collect();
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
        } else if self.tool == ToolKind::Move && !self.selected_objects().is_empty() {
            self.clear_object_selection();
        } else if self.selection.is_none() && self.in_symbol_mode() {
            // Sin nada que cancelar, sale un nivel del modo símbolo (como en Flash).
            self.exit_symbol_to(self.symbol_stack.len() - 1);
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
mod mirror_tests;
#[cfg(test)]
mod tests;
