use cs_commands::{Command, History};
use cs_core::{ColorMode, LayerId, Mask, PixelValue, Rgba, Sprite};
use cs_tools::{
    Bucket, Eraser, Eyedropper, MoveTool, Pencil, Point, RectSelect, ShapeKind, ShapeTool, Tool,
    ToolContext, ToolOutput,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    Pencil,
    Eraser,
    Bucket,
    Line,
    Rectangle,
    Ellipse,
    Select,
    Move,
    Eyedropper,
}

impl ToolKind {
    pub const ALL: [ToolKind; 9] = [
        ToolKind::Select,
        ToolKind::Move,
        ToolKind::Pencil,
        ToolKind::Eraser,
        ToolKind::Bucket,
        ToolKind::Line,
        ToolKind::Rectangle,
        ToolKind::Ellipse,
        ToolKind::Eyedropper,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToolKind::Pencil => "Pencil",
            ToolKind::Eraser => "Eraser",
            ToolKind::Bucket => "Bucket",
            ToolKind::Line => "Line",
            ToolKind::Rectangle => "Rectangle",
            ToolKind::Ellipse => "Ellipse",
            ToolKind::Select => "Select",
            ToolKind::Move => "Move",
            ToolKind::Eyedropper => "Eyedropper",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            ToolKind::Pencil => "B",
            ToolKind::Eraser => "E",
            ToolKind::Bucket => "G",
            ToolKind::Line => "L",
            ToolKind::Rectangle | ToolKind::Ellipse => "U",
            ToolKind::Select => "M",
            ToolKind::Move => "V",
            ToolKind::Eyedropper => "I",
        }
    }

    /// Herramientas que pintan y por lo tanto se recortan a la selección.
    pub fn paints(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Eraser
                | ToolKind::Bucket
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
        )
    }

    /// Herramientas que usan el tamaño de pincel.
    pub fn uses_brush(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
        )
    }
}

/// Opciones compartidas por las herramientas.
#[derive(Clone, Copy, Debug)]
pub struct ToolOptions {
    pub brush_size: u32,
    pub pixel_perfect: bool,
    pub filled: bool,
    pub contiguous: bool,
    /// Shift presionado (lo actualiza la UI cada frame).
    pub constrain: bool,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            brush_size: 1,
            pixel_perfect: true,
            filled: false,
            contiguous: true,
            constrain: false,
        }
    }
}

struct Tools {
    pencil: Pencil,
    eraser: Eraser,
    bucket: Bucket,
    line: ShapeTool,
    rectangle: ShapeTool,
    ellipse: ShapeTool,
    select: RectSelect,
    mover: MoveTool,
    eyedropper: Eyedropper,
}

impl Tools {
    fn new() -> Self {
        Self {
            pencil: Pencil::new(),
            eraser: Eraser::new(),
            bucket: Bucket::new(),
            line: ShapeTool::new(ShapeKind::Line),
            rectangle: ShapeTool::new(ShapeKind::Rectangle),
            ellipse: ShapeTool::new(ShapeKind::Ellipse),
            select: RectSelect::new(),
            mover: MoveTool::new(),
            eyedropper: Eyedropper::new(),
        }
    }

    fn get(&self, kind: ToolKind) -> &dyn Tool {
        match kind {
            ToolKind::Pencil => &self.pencil,
            ToolKind::Eraser => &self.eraser,
            ToolKind::Bucket => &self.bucket,
            ToolKind::Line => &self.line,
            ToolKind::Rectangle => &self.rectangle,
            ToolKind::Ellipse => &self.ellipse,
            ToolKind::Select => &self.select,
            ToolKind::Move => &self.mover,
            ToolKind::Eyedropper => &self.eyedropper,
        }
    }

    fn get_mut(&mut self, kind: ToolKind) -> &mut dyn Tool {
        match kind {
            ToolKind::Pencil => &mut self.pencil,
            ToolKind::Eraser => &mut self.eraser,
            ToolKind::Bucket => &mut self.bucket,
            ToolKind::Line => &mut self.line,
            ToolKind::Rectangle => &mut self.rectangle,
            ToolKind::Ellipse => &mut self.ellipse,
            ToolKind::Select => &mut self.select,
            ToolKind::Move => &mut self.mover,
            ToolKind::Eyedropper => &mut self.eyedropper,
        }
    }
}

/// Recorta un `DrawPixels` a la máscara; otros comandos pasan igual.
fn clip(cmd: Command, mask: &Mask) -> Command {
    match cmd {
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        } => Command::DrawPixels {
            layer,
            frame,
            pixels: pixels
                .into_iter()
                .filter(|&(x, y, _)| mask.contains(x, y))
                .collect(),
        },
        other => other,
    }
}

/// Lo que el lienzo debe mostrar en este frame.
pub struct FrameView {
    pub pixels: Vec<Rgba>,
    pub selection: Option<Mask>,
}

/// Estado del documento abierto y de la edición. No sabe nada de egui.
pub struct Editor {
    pub sprite: Sprite,
    pub history: History,
    pub layer: LayerId,
    pub frame: usize,
    pub primary: PixelValue,
    pub secondary: PixelValue,
    pub tool: ToolKind,
    pub options: ToolOptions,
    pub selection: Option<Mask>,
    /// Aumenta cada vez que el documento o la selección cambian.
    pub revision: u64,
    tools: Tools,
    painting_secondary: bool,
    /// Alt al presionar: cuentagotas temporal durante el gesto.
    temp_eyedropper: bool,
}

impl Editor {
    pub fn new(sprite: Sprite) -> Self {
        let layer = sprite.layers.last().map(|l| l.id).unwrap_or(LayerId(1));
        let mut editor = Self {
            primary: PixelValue::Index(0),
            secondary: PixelValue::Index(0),
            sprite,
            history: History::new(),
            layer,
            frame: 0,
            tool: ToolKind::Pencil,
            options: ToolOptions::default(),
            selection: None,
            revision: 0,
            tools: Tools::new(),
            painting_secondary: false,
            temp_eyedropper: false,
        };
        editor.primary = editor.palette_value(0);
        editor.secondary = editor.palette_value(21);
        editor
    }

    /// Valor de píxel para la entrada `index` de la paleta según el modo de color.
    pub fn palette_value(&self, index: u8) -> PixelValue {
        match self.sprite.color_mode {
            ColorMode::Indexed { .. } => PixelValue::Index(index),
            ColorMode::Rgba => PixelValue::Rgba(self.sprite.palette.get(index).unwrap_or_default()),
        }
    }

    pub fn resolve(&self, value: PixelValue) -> Rgba {
        cs_render::resolve(&self.sprite, value)
    }

    /// Ejecuta un comando; devuelve si se aplicó.
    pub fn execute(&mut self, cmd: Command) -> bool {
        match self.history.execute(&mut self.sprite, cmd) {
            Ok(()) => {
                self.revision += 1;
                true
            }
            Err(e) => {
                log::warn!("command rejected: {e}");
                false
            }
        }
    }

    pub fn undo(&mut self) {
        match self.history.undo(&mut self.sprite) {
            Ok(true) => self.revision += 1,
            Ok(false) => {}
            Err(e) => log::error!("undo failed: {e}"),
        }
    }

    pub fn redo(&mut self) {
        match self.history.redo(&mut self.sprite) {
            Ok(true) => self.revision += 1,
            Ok(false) => {}
            Err(e) => log::error!("redo failed: {e}"),
        }
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
    }

    pub fn set_selection(&mut self, selection: Option<Mask>) {
        self.selection = selection.filter(|m| !m.is_empty());
        self.revision += 1;
    }

    pub fn select_all(&mut self) {
        self.set_selection(Some(Mask::all(self.sprite.width, self.sprite.height)));
    }

    pub fn deselect(&mut self) {
        self.set_selection(None);
    }

    /// Borra (deja transparente) el contenido seleccionado de la capa actual.
    pub fn clear_selection_content(&mut self) {
        let Some(mask) = &self.selection else {
            return;
        };
        let t = self.sprite.color_mode.transparent();
        let pixels = mask.points().map(|(x, y)| (x, y, t)).collect();
        self.execute(Command::DrawPixels {
            layer: self.layer,
            frame: self.frame,
            pixels,
        });
    }

    /// Herramienta efectiva (considera el cuentagotas temporal).
    pub fn active_tool(&self) -> ToolKind {
        if self.temp_eyedropper {
            ToolKind::Eyedropper
        } else {
            self.tool
        }
    }

    pub fn is_painting(&self) -> bool {
        self.tools.get(self.active_tool()).is_active()
    }

    fn color(&self) -> PixelValue {
        if self.painting_secondary {
            self.secondary
        } else {
            self.primary
        }
    }

    fn context(&self) -> ToolContext<'_> {
        ToolContext {
            sprite: &self.sprite,
            layer: self.layer,
            frame: self.frame,
            color: self.color(),
            brush_size: self.options.brush_size,
            pixel_perfect: self.options.pixel_perfect,
            filled: self.options.filled,
            contiguous: self.options.contiguous,
            constrain: self.options.constrain,
            selection: self.selection.as_ref(),
        }
    }

    /// Corre `f` sobre la herramienta activa con un contexto que toma prestado el documento.
    fn with_tool<R>(&mut self, f: impl FnOnce(&mut dyn Tool, &ToolContext) -> R) -> R {
        let kind = self.active_tool();
        let mut tools = std::mem::replace(&mut self.tools, Tools::new());
        let result = f(tools.get_mut(kind), &self.context());
        self.tools = tools;
        result
    }

    pub fn pointer_press(&mut self, p: Point, secondary: bool, alt: bool) {
        self.painting_secondary = secondary;
        self.temp_eyedropper = alt && self.tool != ToolKind::Eyedropper;
        self.with_tool(|tool, ctx| tool.press(ctx, p));
    }

    pub fn pointer_drag(&mut self, p: Point) {
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
    }

    pub fn pointer_release(&mut self) {
        let kind = self.active_tool();
        let out = self.with_tool(|tool, ctx| tool.release(ctx));
        self.temp_eyedropper = false;
        self.apply_output(kind, out);
    }

    fn apply_output(&mut self, kind: ToolKind, out: ToolOutput) {
        let applied = match out.command {
            Some(cmd) => {
                let cmd = match (&self.selection, kind.paints()) {
                    (Some(mask), true) => clip(cmd, mask),
                    _ => cmd,
                };
                self.execute(cmd)
            }
            None => true,
        };
        if let Some(v) = out.pick {
            if self.painting_secondary {
                self.secondary = v;
            } else {
                self.primary = v;
            }
        }
        if let Some(selection) = out.selection
            && applied
        {
            self.set_selection(selection);
        }
    }

    /// Composición a mostrar: el documento con el gesto en curso aplicado sobre un clon.
    pub fn frame_view(&self) -> FrameView {
        let kind = self.active_tool();
        let tool = self.tools.get(kind);
        if !tool.is_active() {
            return FrameView {
                pixels: cs_render::render_frame(&self.sprite, self.frame),
                selection: self.selection.clone(),
            };
        }
        let out = tool.preview(&self.context());
        let pixels = match out.command {
            Some(cmd) => {
                let cmd = match (&self.selection, kind.paints()) {
                    (Some(mask), true) => clip(cmd, mask),
                    _ => cmd,
                };
                let mut preview = self.sprite.clone();
                match cmd.apply(&mut preview) {
                    Ok(_) => cs_render::render_frame(&preview, self.frame),
                    Err(_) => cs_render::render_frame(&self.sprite, self.frame),
                }
            }
            None => cs_render::render_frame(&self.sprite, self.frame),
        };
        let selection = match out.selection {
            Some(sel) => sel,
            None => self.selection.clone(),
        };
        FrameView { pixels, selection }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> Editor {
        Editor::new(Sprite::new(8, 8, ColorMode::Rgba))
    }

    fn click(e: &mut Editor, p: Point, secondary: bool, alt: bool) {
        e.pointer_press(p, secondary, alt);
        e.pointer_release();
    }

    #[test]
    fn execute_undo_redo_bump_revision() {
        let mut e = editor();
        let r0 = e.revision;
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, e.primary)],
        });
        assert!(e.revision > r0 && e.history.can_undo());
        e.undo();
        assert!(e.sprite.cels.is_empty());
        e.redo();
        assert!(!e.sprite.cels.is_empty());
    }

    #[test]
    fn secondary_button_paints_secondary_color() {
        let mut e = editor();
        click(&mut e, (1, 1), true, false);
        assert_eq!(e.sprite.pixel(LayerId(1), 0, 1, 1), e.secondary);
    }

    #[test]
    fn painting_is_clipped_to_selection() {
        let mut e = editor();
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        e.tool = ToolKind::Bucket;
        e.options.contiguous = false;
        click(&mut e, (0, 0), false, false);
        assert_eq!(e.sprite.pixel(LayerId(1), 0, 1, 1), e.primary);
        assert_eq!(
            e.sprite.pixel(LayerId(1), 0, 2, 2),
            e.sprite.color_mode.transparent()
        );
    }

    #[test]
    fn alt_click_picks_color_and_restores_tool() {
        let mut e = editor();
        let green = PixelValue::Rgba(Rgba::rgb(0, 255, 0));
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(3, 3, green)],
        });
        click(&mut e, (3, 3), false, true);
        assert_eq!(e.primary, green);
        assert_eq!(e.active_tool(), ToolKind::Pencil);
        e.tool = ToolKind::Eyedropper;
        click(&mut e, (0, 0), true, false);
        assert_eq!(e.secondary, e.sprite.color_mode.transparent());
    }

    #[test]
    fn delete_clears_selected_content() {
        let mut e = editor();
        e.tool = ToolKind::Bucket;
        click(&mut e, (0, 0), false, false);
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (3, 3))));
        e.clear_selection_content();
        let t = e.sprite.color_mode.transparent();
        assert_eq!(e.sprite.pixel(LayerId(1), 0, 2, 2), t);
        assert_eq!(e.sprite.pixel(LayerId(1), 0, 0, 0), e.primary);
    }

    #[test]
    fn select_all_and_deselect() {
        let mut e = editor();
        e.select_all();
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 64);
        e.deselect();
        assert!(e.selection.is_none());
    }

    #[test]
    fn preview_shows_stroke_without_touching_document() {
        let mut e = editor();
        e.pointer_press((0, 0), false, false);
        let view = e.frame_view();
        assert_eq!(view.pixels[0], e.resolve(e.primary));
        assert!(e.sprite.cels.is_empty());
        e.pointer_release();
        assert!(!e.sprite.cels.is_empty());
    }

    #[test]
    fn indexed_sprite_uses_palette_indices() {
        let e = Editor::new(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        assert_eq!(e.primary, PixelValue::Index(0));
        assert_eq!(e.secondary, PixelValue::Index(21));
    }
}
