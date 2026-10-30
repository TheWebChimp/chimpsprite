//! Los gestos de las herramientas: el puntero, el contexto que reciben y lo que devuelven.

use super::*;

impl Editor {
    /// Herramienta efectiva (considera el cuentagotas temporal).
    pub fn active_tool(&self) -> ToolKind {
        if self.temp_eyedropper {
            ToolKind::Eyedropper
        } else {
            self.tool
        }
    }

    /// Gesto de la herramienta Artboard en curso (crear, mover o redimensionar).
    pub fn artboard_gesture(&self) -> &ArtboardTool {
        &self.artboard_tool
    }

    pub fn is_painting(&self) -> bool {
        self.transform_drag.is_some()
            || self.artboard_tool.is_active()
            || self
                .tools
                .get(self.active_tool())
                .is_some_and(|t| t.is_active())
    }

    /// Gesto de varios clics a medias (lazo poligonal) o texto en edición.
    pub fn is_pending(&self) -> bool {
        self.text.is_some()
            || self
                .tools
                .get(self.active_tool())
                .is_some_and(|t| t.is_pending())
    }

    /// Movimiento del puntero sin botones.
    pub fn pointer_hover(&mut self, world: (f32, f32)) {
        if self
            .tools
            .get(self.active_tool())
            .is_some_and(|t| t.is_pending())
        {
            let p = self.tool_point(world);
            self.with_tool(|tool, ctx| tool.hover(ctx, p));
        }
    }

    /// Tiempo con el botón presionado (spray).
    pub fn tool_tick(&mut self, dt_ms: f32) {
        if self.is_painting() {
            self.with_tool(|tool, ctx| tool.tick(ctx, dt_ms));
        }
    }

    /// Termina un gesto de varios clics (Enter / doble clic) o aplica el texto en edición.
    pub fn finish_gesture(&mut self) {
        if self.text.is_some() {
            self.commit_text();
            return;
        }
        let kind = self.active_tool();
        if self.tools.get(kind).is_some_and(|t| t.is_pending()) {
            let out = self.with_tool(|tool, ctx| tool.finish(ctx));
            self.apply_output(kind, out);
        }
    }

    /// Descarta el gesto en curso (Esc).
    pub fn cancel_gesture(&mut self) {
        self.text = None;
        if let Some(tool) = self.tools.get_mut(self.active_tool()) {
            tool.cancel();
        }
        self.revision += 1;
    }

    /// A qué pinta la herramienta ahora: `(editando la máscara, modo Tiles)`.
    pub(crate) fn paint_target(&self) -> (bool, bool) {
        (self.editing_mask(), self.tiles_mode())
    }

    /// Si a mitad de un trazo cambió a qué pinta la herramienta (`before` es el
    /// [`paint_target`](Self::paint_target) de antes del cambio), lo corta sin aplicarlo.
    pub(crate) fn cancel_if_target_changed(&mut self, before: (bool, bool)) {
        if self.is_painting() && self.paint_target() != before {
            self.abort_stroke();
        }
    }

    /// Con un trazo de la herramienta en curso, el clic del otro botón (`secondary`: el derecho)
    /// lo corta sin aplicarlo. Devuelve si lo cortó (entonces ese clic no empieza otro).
    pub fn cancel_with_other_button(&mut self, secondary: bool) -> bool {
        let stroking = self
            .tools
            .get(self.active_tool())
            .is_some_and(|t| t.is_active());
        if !stroking || secondary == self.painting_secondary {
            return false;
        }
        self.abort_stroke();
        true
    }

    /// Corta el trazo en curso sin aplicarlo (`cancel_gesture` y la herramienta como recién
    /// creada). Si no, el trazo quedaría colgado: al soltar, una herramienta bloqueada no se
    /// suelta.
    fn abort_stroke(&mut self) {
        self.object_copy = false;
        self.cancel_gesture();
        self.tools.reset(self.active_tool());
        self.temp_eyedropper = false;
    }

    /// Aplica el texto en edición con el color primario (un paso de undo).
    pub fn commit_text(&mut self) {
        let Some(cmd) = self.text_command() else {
            self.text = None;
            return;
        };
        self.text = None;
        let cmd = self.shape_paint(ToolKind::Text, cmd);
        self.execute_named(cmd, ToolKind::Text.label());
    }

    pub(super) fn text_command(&self) -> Option<Command> {
        let draft = self.text.as_ref().filter(|d| !d.text.trim().is_empty())?;
        let pixels = cs_tools::text::text_pixels(
            self.sprite(),
            (self.layer, self.frame),
            draft.origin,
            self.primary,
            &draft.text,
            &self.text_style,
        );
        Some(Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        })
    }

    pub fn to_local(&self, world: (f32, f32)) -> Point {
        let (x, y) = self.artboard().position;
        (
            (world.0 - x as f32).floor() as i32,
            (world.1 - y as f32).floor() as i32,
        )
    }

    pub(super) fn color(&self) -> PixelValue {
        if self.painting_secondary {
            self.secondary
        } else {
            self.primary
        }
    }

    /// Shift restringe la forma, salvo en selección si al empezar eligió el modo Add.
    pub(super) fn constrain(&self) -> bool {
        self.options.constrain && !(self.active_tool().is_selection() && self.shift_at_press)
    }

    /// Los puntos del pincel personalizado con sus colores en este sprite, si pinta con ellos.
    /// Se arman una vez por pincel, artboard y versión del documento (el modo de color y la
    /// paleta deciden los valores), no en cada movimiento del puntero.
    fn brush_image(&self) -> Option<BrushImage> {
        if self.options.brush_shape != BrushShape::Custom || !self.options.brush_colors {
            return None;
        }
        let key = (
            cs_raster::custom_brush_version(),
            self.active,
            self.doc_version,
        );
        let mut cache = self.brush_image.borrow_mut();
        if let Some((k, image)) = cache.as_ref()
            && *k == key
        {
            return (!image.is_empty()).then(|| image.clone());
        }
        let image: BrushImage = cs_raster::custom_brush()
            .into_iter()
            .map(|(p, c)| (p, self.value_of(c)))
            .collect();
        *cache = Some((key, image.clone()));
        (!image.is_empty()).then_some(image)
    }

    pub(super) fn context(&self) -> ToolContext<'_> {
        let secondary = if self.painting_secondary {
            self.primary
        } else {
            self.secondary
        };
        ToolContext {
            sprite: self.sprite(),
            layer: self.layer,
            frame: self.frame,
            color: self.color(),
            secondary,
            brush_size: self.options.brush_size,
            brush_shape: self.options.brush_shape,
            brush_image: self.brush_image(),
            pixel_perfect: self.options.pixel_perfect,
            filled: self.options.filled,
            corner_radius: self.options.corner_radius,
            contiguous: self.options.contiguous,
            constrain: self.constrain(),
            selection: self.selection.as_ref(),
            tolerance: self.options.tolerance,
            sample_all: self.options.sample_all,
            spray_width: self.options.spray_width,
            spray_speed: self.options.spray_speed,
            gradient_shape: self.options.gradient_shape,
            dither: self.options.dither,
            selected_slice: self.slice,
            tiles: false,
            objects: self.selected_objects(),
            copy_objects: self.object_copy,
        }
    }

    /// Contexto sobre el proxy de celdas: los dos colores son el tile del pincel codificado, el
    /// pincel se mide en celdas y la selección es la del proxy. Pixel-perfect no aplica.
    pub(super) fn tiles_context<'a>(&'a self, proxy: &'a TileProxy) -> ToolContext<'a> {
        let brush = PixelValue::Rgba(tile_ops::encode(self.brush_tile()));
        ToolContext {
            sprite: &proxy.sprite,
            layer: tile_ops::PROXY_LAYER,
            frame: 0,
            color: brush,
            secondary: brush,
            brush_size: self.options.brush_size,
            // En celdas el pincel sigue cuadrado: el contorno del lienzo dibuja celdas enteras.
            brush_shape: BrushShape::Square,
            brush_image: None,
            pixel_perfect: false,
            filled: self.options.filled,
            corner_radius: self.options.corner_radius,
            contiguous: self.options.contiguous,
            constrain: self.constrain(),
            selection: proxy.selection.as_ref(),
            tolerance: 0,
            sample_all: false,
            spray_width: self.options.spray_width,
            spray_speed: self.options.spray_speed,
            gradient_shape: self.options.gradient_shape,
            dither: self.options.dither,
            selected_slice: None,
            tiles: true,
            objects: &[],
            copy_objects: false,
        }
    }

    /// Proxy de celdas para `kind` si la capa activa está en modo Tiles y la herramienta trabaja
    /// en celdas.
    pub(super) fn cell_proxy(&self, kind: ToolKind) -> Option<TileProxy> {
        if !kind.uses_cells() || !self.tiles_mode() {
            return None;
        }
        TileProxy::build(
            self.sprite(),
            self.layer,
            self.frame,
            self.selection.as_ref(),
        )
    }

    /// Punto de mundo → punto de la herramienta: píxel local o, en modo Tiles, el píxel del proxy
    /// de la celda que contiene al puntero.
    fn tool_point(&self, world: (f32, f32)) -> Point {
        let p = self.to_local(self.snapped(world));
        match self
            .tile_space()
            .filter(|_| self.active_tool().uses_cells())
        {
            Some(space) => space.proxy_pixel(space.cell_at(p)),
            None => p,
        }
    }

    /// Corre `f` sobre la herramienta de sprite activa con un contexto que toma prestado el
    /// documento (o el proxy de celdas en modo Tiles).
    fn with_tool<R: Default>(&mut self, f: impl FnOnce(&mut dyn Tool, &ToolContext) -> R) -> R {
        let kind = self.active_tool();
        let proxy = self.cell_proxy(kind);
        let mut tools = std::mem::replace(&mut self.tools, SpriteTools::new());
        let result = tools
            .get_mut(kind)
            .map(|tool| match &proxy {
                Some(proxy) => f(tool, &self.tiles_context(proxy)),
                None => f(tool, &self.context()),
            })
            .unwrap_or_default();
        self.tools = tools;
        result
    }

    /// Arrastrar la etiqueta de un artboard: lo activa y lo mueve, con cualquier herramienta.
    pub fn press_artboard_label(&mut self, id: ArtboardId, world: (f32, f32)) {
        self.activate(id);
        self.artboard_tool.start_move(id, world);
    }

    /// `slop`: tolerancia en píxeles de mundo para tomar bordes de artboards.
    pub fn pointer_press(&mut self, world: (f32, f32), secondary: bool, alt: bool, slop: f32) {
        self.stop();
        self.tile_limit = false;
        self.object_copy = false;
        // Con el lápiz, el clic derecho es el cuentagotas: el color de debajo pasa a ser el
        // primario (para seguir pintando con él). Con Shading no: ahí el derecho retrocede en la
        // rampa.
        let pencil_pick =
            secondary && self.tool == ToolKind::Pencil && self.options.ink != Ink::Shading;
        self.painting_secondary = secondary && !pencil_pick;
        // Alt es cuentagotas temporal solo en las herramientas que pintan.
        self.temp_eyedropper = (alt && self.tool.paints()) || pencil_pick;
        let shift = self.options.constrain;
        // Arrastrar lo seleccionado (o cualquier punto con Move) lo levanta y lo deja flotando.
        // Solo con el botón principal: con el derecho, Move no hace nada y las herramientas de
        // selección siguen con su gesto.
        if self.moves_selection_at(world, shift, alt) {
            if !secondary {
                self.press_transform(TransformHit::Move, world);
                return;
            }
            if self.tool == ToolKind::Move {
                return;
            }
        }
        // Un clic fuera de lo flotante lo confirma antes de seguir con la herramienta.
        self.commit_floating();
        if self.tool.is_selection() && !self.tools.get(self.tool).is_some_and(|t| t.is_pending()) {
            self.shift_at_press = shift;
            self.gesture_mode = match (shift, alt) {
                (true, true) => SelectionMode::Intersect,
                (true, false) => SelectionMode::Add,
                (false, true) => SelectionMode::Subtract,
                (false, false) => self.options.selection_mode,
            };
        }
        if self.active_tool() == ToolKind::Artboard {
            // En el modo símbolo la herramienta no toca los artboards del lienzo (no se ven).
            if self.in_symbol_mode() {
                return;
            }
            if let Some(id) =
                self.artboard_tool
                    .press(&self.document, world, slop, Some(self.active))
            {
                self.activate(id);
            }
            return;
        }
        // En el modo símbolo solo se ve el símbolo: un clic no activa otro artboard.
        if !self.in_symbol_mode()
            && let Some(id) = self.document.artboard_at(world)
        {
            self.activate(id);
        }
        let p = self.tool_point(world);
        // En modo Tiles o editando una máscara algunas herramientas no actúan (la barra de
        // estado lo avisa).
        if self.tool_blocked() {
            return;
        }
        if self.active_tool() == ToolKind::Slice {
            self.select_slice_at(p);
        }
        if self.active_tool() == ToolKind::Text {
            // Un clic empieza un texto o mueve el que se está escribiendo.
            match &mut self.text {
                Some(draft) => draft.origin = p,
                None => {
                    self.text = Some(TextDraft {
                        origin: p,
                        text: String::new(),
                    })
                }
            }
            self.revision += 1;
            return;
        }
        // Mover sin selección: un objeto bajo el puntero queda seleccionado (con Shift se suma o
        // se quita) y activa su capa; con Alt, el arrastre mueve copias. Sin objeto, cambia a la
        // capa que tiene algo dibujado bajo el puntero. En modo Tiles Move se queda en la capa
        // tilemap.
        if self.active_tool() == ToolKind::Move && self.selection.is_none() && !self.tiles_mode() {
            match self.object_at(world) {
                Some((layer, object)) => {
                    // Un clic sobre uno ya seleccionado conserva la selección (para arrastrarla
                    // entera).
                    let selected = layer == self.layer && self.selected_objects().contains(&object);
                    if shift || !selected {
                        self.select_object(layer, object, shift);
                    }
                    self.object_copy = alt;
                }
                None => {
                    self.clear_object_selection();
                    if let Some(layer) = self.layer_at(p) {
                        self.layer = layer;
                    }
                }
            }
        }
        self.with_tool(|tool, ctx| tool.press(ctx, p));
    }

    /// Presionar en `world` arrastraría lo seleccionado (levantándolo): con Move, desde cualquier
    /// punto que no sea otro artboard; con una herramienta de selección, dentro de lo
    /// seleccionado y sin modificadores que cambien el modo de selección.
    pub fn moves_selection_at(&self, world: (f32, f32), shift: bool, alt: bool) -> bool {
        let artboard = self.shown_artboard_at(world);
        if !self.can_transform() || artboard.is_some_and(|id| id != self.active) {
            return false;
        }
        if self.tool == ToolKind::Move {
            return true;
        }
        let p = self.to_local(world);
        !shift
            && !alt
            && !self.tools.get(self.tool).is_some_and(|t| t.is_pending())
            && artboard == Some(self.active)
            && self
                .selection
                .as_ref()
                .is_some_and(|m| m.contains(p.0, p.1))
    }

    /// Capa de imagen más alta con un píxel visible en `p` (local), ignorando ocultas y bloqueadas.
    pub fn layer_at(&self, p: Point) -> Option<LayerId> {
        let sprite = self.sprite();
        sprite
            .layers
            .iter()
            .rev()
            .filter(|l| l.kind == cs_core::LayerKind::Image && !l.locked && l.opacity > 0)
            .filter(|l| sprite.is_visible(l.id))
            .find(|l| {
                let visible = cs_render::resolve(sprite, sprite.pixel(l.id, self.frame, p.0, p.1));
                let cel_opacity = sprite.cel(l.id, self.frame).map_or(0, |c| c.opacity);
                visible.a > 0 && cel_opacity > 0
            })
            .map(|l| l.id)
    }

    pub fn pointer_drag(&mut self, world: (f32, f32)) {
        if self.artboard_tool.is_active() {
            self.artboard_tool.drag(world);
            return;
        }
        if self.transform_drag.is_some() {
            self.drag_transform(world);
            return;
        }
        // Si la herramienta quedó bloqueada a mitad del trazo (entró al modo máscara o al modo
        // Tiles sin pasar por `set_edit_mask`/`select_layer`), el trazo se corta.
        if self.tool_blocked() {
            if self.is_painting() {
                self.abort_stroke();
            }
            return;
        }
        let p = self.tool_point(world);
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
    }

    /// Pega el punto a guías/grid si la herramienta activa lo usa (no las de trazo libre).
    fn snapped(&self, world: (f32, f32)) -> (f32, f32) {
        let snaps = matches!(
            self.active_tool(),
            ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::PolygonLasso
                | ToolKind::Polygon
                | ToolKind::Gradient
                | ToolKind::Text
                | ToolKind::Move
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
        );
        if snaps { self.snap_world(world) } else { world }
    }

    /// Pega el punto a guías/grid, sin importar la herramienta (mover lo seleccionado pega
    /// siempre, como Move).
    pub(crate) fn snap_world(&self, world: (f32, f32)) -> (f32, f32) {
        let Some(ctx) = self.snap.as_ref() else {
            return world;
        };
        let (ox, oy) = self.artboard().position;
        let (lx, ly) = ctx.snap((world.0 - ox as f32, world.1 - oy as f32));
        (lx + ox as f32, ly + oy as f32)
    }

    pub fn pointer_release(&mut self) {
        if self.artboard_tool.is_active() {
            if let Some(cmd) = self.artboard_tool.release(&self.document) {
                let created = match &cmd {
                    DocCommand::AddArtboard { artboard, .. } => Some(artboard.id),
                    _ => None,
                };
                if self.execute_doc(cmd)
                    && let Some(id) = created
                {
                    self.activate(id);
                }
            }
            return;
        }
        if self.transform_drag.is_some() {
            if let Some(from) = self.release_transform() {
                // Un clic sin arrastrar es un clic normal de la herramienta de selección.
                let kind = self.tool;
                self.gesture_mode = self.options.selection_mode;
                self.shift_at_press = false;
                self.with_tool(|tool, ctx| tool.press(ctx, from));
                let out = self.with_tool(|tool, ctx| tool.release(ctx));
                self.apply_output(kind, out);
            }
            return;
        }
        let kind = self.active_tool();
        if self.tool_blocked() {
            if self.is_painting() {
                self.abort_stroke();
            }
            self.temp_eyedropper = false;
            return;
        }
        let out = self.with_tool(|tool, ctx| tool.release(ctx));
        self.temp_eyedropper = false;
        if kind == ToolKind::Move && self.object_copy {
            self.object_copy = false;
            self.apply_object_copy(out);
            return;
        }
        self.apply_output(kind, out);
    }

    /// Suelta Alt+arrastrar con Move sobre objetos: las copias movidas quedan seleccionadas (un
    /// paso de undo).
    fn apply_object_copy(&mut self, out: ToolOutput) {
        let Some(cmd) = out.command else {
            return;
        };
        let ids = |e: &Self| -> Vec<cs_core::ObjectId> {
            e.sprite()
                .layer(e.layer)
                .map(|l| l.objects.iter().map(|o| o.id).collect())
                .unwrap_or_default()
        };
        let before = ids(self);
        if self.named("Duplicate Objects", |e| e.execute(cmd)) {
            let copies = ids(self)
                .into_iter()
                .filter(|o| !before.contains(o))
                .collect();
            self.set_selected_objects(copies);
            self.revision += 1;
        }
    }

    /// Simetría y recorte a la selección para las herramientas que pintan.
    pub(super) fn shape_paint(&self, kind: ToolKind, cmd: Command) -> Command {
        if !kind.paints() {
            return cmd;
        }
        let artboard = self.artboard();
        let cmd = if kind.mirrors() {
            mirror(cmd, artboard.symmetry, artboard.symmetry_center2())
        } else {
            cmd
        };
        let cmd = self.wrap_tiled(cmd);
        let cmd = match &self.selection {
            Some(mask) => clip(cmd, mask),
            None => cmd,
        };
        if self.editing_mask() {
            self.to_mask_command(cmd)
        } else if kind.uses_ink() {
            self.apply_ink(cmd)
        } else {
            cmd
        }
    }

    /// Con Tiled Mode, lo pintado fuera del sprite cae dentro, del otro lado (en los ejes que se
    /// repiten).
    fn wrap_tiled(&self, cmd: Command) -> Command {
        let tiled = self.tiled;
        let Command::DrawPixels {
            layer,
            frame,
            pixels,
        } = cmd
        else {
            return cmd;
        };
        if tiled == TiledMode::None {
            return Command::DrawPixels {
                layer,
                frame,
                pixels,
            };
        }
        let sprite = self.sprite();
        let (w, h) = (sprite.width as i32, sprite.height as i32);
        let pixels = pixels
            .into_iter()
            .map(|(x, y, v)| {
                let x = if tiled.x() { x.rem_euclid(w) } else { x };
                let y = if tiled.y() { y.rem_euclid(h) } else { y };
                (x, y, v)
            })
            .collect();
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        }
    }

    /// El ink elegido sobre un trazo: Lock Alpha deja solo lo que cae sobre algo pintado (con su
    /// transparencia) y Shading cambia cada píxel por el siguiente de la rampa.
    fn apply_ink(&self, cmd: Command) -> Command {
        let Command::DrawPixels {
            layer,
            frame,
            pixels,
        } = cmd
        else {
            return cmd;
        };
        let sprite = self.sprite();
        let under = |x, y| sprite.pixel(layer, frame, x, y);
        let pixels = match self.options.ink {
            Ink::Simple => pixels,
            Ink::LockAlpha => pixels
                .into_iter()
                .filter_map(|(x, y, v)| {
                    let old = self.resolve(under(x, y));
                    (old.a > 0).then_some((
                        x,
                        y,
                        match v {
                            PixelValue::Rgba(c) => PixelValue::Rgba(Rgba { a: old.a, ..c }),
                            index => index,
                        },
                    ))
                })
                .collect(),
            Ink::Shading => {
                let ramp = self.shading_ramp();
                pixels
                    .into_iter()
                    .filter_map(|(x, y, _)| {
                        let i = ramp.iter().position(|&v| v == under(x, y))?;
                        let j = if self.painting_secondary {
                            i.checked_sub(1)?
                        } else {
                            i + 1
                        };
                        ramp.get(j).map(|&v| (x, y, v))
                    })
                    .collect()
            }
        };
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        }
    }

    /// Los colores por los que avanza Shading: el rango elegido de la paleta o la paleta entera.
    fn shading_ramp(&self) -> Vec<PixelValue> {
        let len = self.sprite().palette.len();
        let (a, b) = match self.palette_selection {
            Some((a, b)) => (a.min(b), a.max(b)),
            None => (0, len.saturating_sub(1)),
        };
        (a..=b.min(len.saturating_sub(1)))
            .map(|i| self.palette_value(i as u8))
            .collect()
    }

    /// Editando la máscara, un trazo pinta niveles: blanco muestra, negro o transparente oculta.
    fn to_mask_command(&self, cmd: Command) -> Command {
        match cmd {
            Command::DrawPixels { layer, pixels, .. } => Command::DrawMask {
                layer,
                pixels: pixels
                    .into_iter()
                    .map(|(x, y, v)| (x, y, cs_render::mask_level(self.resolve(v))))
                    .collect(),
            },
            other => other,
        }
    }

    fn apply_output(&mut self, kind: ToolKind, out: ToolOutput) {
        if let Some(proxy) = self.cell_proxy(kind) {
            self.apply_tiles_output(kind, out, &proxy);
            return;
        }
        let slices_before = self.sprite().slices.len();
        let applied = match out.command {
            Some(cmd) => {
                let cmd = self.shape_paint(kind, cmd);
                self.execute_named(cmd, kind.label())
            }
            None => true,
        };
        if let Some(v) = out.pick {
            // Como un clic en un cuadrito: el editor de color pasa al color elegido. No salió de
            // un cuadrito, así que no hay entrada de la paleta que editar (en indexado, la del
            // índice).
            self.pick_color(v, self.painting_secondary, None);
        }
        if let Some(selection) = out.selection
            && applied
        {
            let combined = self.combine_selection(kind, selection);
            self.set_selection(combined);
        }
        // Un slice recién creado queda seleccionado.
        let slices_after = self.sprite().slices.len();
        if kind == ToolKind::Slice && slices_after > slices_before {
            self.slice = Some(slices_after - 1);
        }
    }

    /// Resultado de una herramienta sobre el proxy → tiles y selección del sprite real.
    fn apply_tiles_output(&mut self, kind: ToolKind, out: ToolOutput, proxy: &TileProxy) {
        // `set_tiles` reserva el próximo id de imagen: lo flotante se confirma antes de armarlo.
        self.commit_floating();
        let mut applied = true;
        if let Some(cmd) = &out.command {
            match self.tiles_edit(proxy, cmd) {
                TilesEdit::Apply(cmd) => {
                    applied = self.execute_named(cmd, kind.label());
                    self.tile_limit = false;
                }
                TilesEdit::Nothing => {}
                TilesEdit::TooLarge => {
                    self.tile_limit = true;
                    applied = false;
                }
            }
        }
        if let Some(PixelValue::Rgba(code)) = out.pick {
            self.set_brush_tile(tile_ops::decode(code));
        }
        if let Some(shape) = out.selection
            && applied
        {
            let s = self.sprite();
            let canvas = (s.width, s.height);
            let shape = shape.map(|m| tile_ops::cells_to_pixels(&m, &proxy.space, canvas));
            let combined = self.combine_selection(kind, shape);
            self.set_selection(combined);
        }
    }

    /// Traduce un `DrawPixels` del proxy a `ops::set_tiles` sobre la capa real.
    fn tiles_edit(&self, proxy: &TileProxy, cmd: &Command) -> TilesEdit {
        let locked = self.sprite().layer(proxy.layer).is_none_or(|l| l.locked);
        let Some(cells) = proxy
            .cells(cmd)
            .filter(|cells| !locked && !proxy.unchanged(cells))
        else {
            return TilesEdit::Nothing;
        };
        match cs_commands::ops::set_tiles(self.sprite(), proxy.layer, proxy.frame, &cells) {
            Some(cmd) => TilesEdit::Apply(cmd),
            None => TilesEdit::TooLarge,
        }
    }

    /// Vista previa de un gesto en modo Tiles: el trazo pasa por la misma traducción que al soltar.
    pub(super) fn tiles_view(
        &self,
        kind: ToolKind,
        out: ToolOutput,
        proxy: &TileProxy,
    ) -> DocView<'_> {
        let s = self.sprite();
        let canvas = (s.width, s.height);
        let selection = match out.selection {
            Some(shape) => self.combine_selection(
                kind,
                shape.map(|m| tile_ops::cells_to_pixels(&m, &proxy.space, canvas)),
            ),
            None => self.selection.clone(),
        };
        let edit = out.command.as_ref().map(|cmd| self.tiles_edit(proxy, cmd));
        let Some(TilesEdit::Apply(cmd)) = edit else {
            return DocView {
                document: Cow::Borrowed(&self.document),
                selection,
            };
        };
        let mut doc = self.document.clone();
        let applied = DocCommand::OnArtboard {
            id: self.active,
            cmd,
        }
        .apply(&mut doc);
        DocView {
            document: if applied.is_ok() {
                Cow::Owned(doc)
            } else {
                Cow::Borrowed(&self.document)
            },
            selection,
        }
    }

    /// Combina la forma de un gesto de selección con la selección actual según el modo.
    pub(super) fn combine_selection(&self, kind: ToolKind, shape: Option<Mask>) -> Option<Mask> {
        if kind.is_selection() {
            let shape = shape.map(|m| self.mirrored_selection(m));
            self.gesture_mode.combine(self.selection.as_ref(), shape)
        } else {
            shape
        }
    }

    /// Con la selección espejo y algún eje de simetría, `mask` junto con sus reflejos.
    fn mirrored_selection(&self, mask: Mask) -> Mask {
        let symmetry = self.symmetry();
        if !self.options.mirror_selection || !symmetry.axes.any() {
            return mask;
        }
        let center2 = self.artboard().symmetry_center2();
        let points: Vec<_> = mask
            .points()
            .flat_map(|p| cs_raster::symmetric_points(p, symmetry.axes, center2))
            .collect();
        Mask::from_points(mask.width(), mask.height(), points)
    }

    /// El espejo para mover lo levantado: los ejes vertical y horizontal (las diagonales solo
    /// reflejan al seleccionar) y el centro, si la selección espejo está prendida.
    pub(crate) fn move_mirror(&self) -> Option<(cs_core::SymmetryAxes, (i32, i32))> {
        let axes = self.symmetry().axes;
        (self.options.mirror_selection && (axes.vertical || axes.horizontal)).then(|| {
            let axes = cs_core::SymmetryAxes {
                vertical: axes.vertical,
                horizontal: axes.horizontal,
                ..Default::default()
            };
            (axes, self.artboard().symmetry_center2())
        })
    }
}
