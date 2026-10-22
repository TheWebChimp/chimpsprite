use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use cs_commands::{Command, DocCommand, History};
use cs_core::{
    AnimDirection, Artboard, ArtboardId, ColorMode, Document, LayerId, Mask, PixelValue, Playback,
    Rgba, Sprite, Tile,
};
use cs_render::OnionSkin;
use cs_tools::text::TextStyle;
use cs_tools::{
    ArtboardTool, Bucket, Dither, Eraser, Eyedropper, GradientShape, GradientTool, Jumble, Lasso,
    MagicWand, Marquee, MarqueeShape, MoveTool, Pencil, Point, PolygonLasso, ShapeKind, ShapeTool,
    SliceTool, Spray, Tool, ToolContext, ToolOutput,
};

use crate::tile_ops::{self, TileProxy};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolKind {
    RectSelect,
    EllipseSelect,
    Lasso,
    PolygonLasso,
    MagicWand,
    Move,
    Pencil,
    Spray,
    Eraser,
    Eyedropper,
    Bucket,
    Gradient,
    Line,
    Rectangle,
    Ellipse,
    Jumble,
    Text,
    Slice,
    Artboard,
}

impl ToolKind {
    /// Grupos de la barra (como en Aseprite); el primero de cada grupo es el predeterminado.
    pub const GROUPS: [&'static [ToolKind]; 12] = [
        &[
            ToolKind::RectSelect,
            ToolKind::EllipseSelect,
            ToolKind::Lasso,
            ToolKind::PolygonLasso,
            ToolKind::MagicWand,
        ],
        &[ToolKind::Move],
        &[ToolKind::Pencil, ToolKind::Spray],
        &[ToolKind::Eraser],
        &[ToolKind::Eyedropper],
        &[ToolKind::Bucket, ToolKind::Gradient],
        &[ToolKind::Line],
        &[ToolKind::Rectangle, ToolKind::Ellipse],
        &[ToolKind::Jumble],
        &[ToolKind::Text],
        &[ToolKind::Slice],
        &[ToolKind::Artboard],
    ];

    pub fn label(self) -> &'static str {
        match self {
            ToolKind::RectSelect => "Rectangular Marquee",
            ToolKind::EllipseSelect => "Elliptical Marquee",
            ToolKind::Lasso => "Lasso",
            ToolKind::PolygonLasso => "Polygonal Lasso",
            ToolKind::MagicWand => "Magic Wand",
            ToolKind::Move => "Move",
            ToolKind::Pencil => "Pencil",
            ToolKind::Spray => "Spray",
            ToolKind::Eraser => "Eraser",
            ToolKind::Eyedropper => "Eyedropper",
            ToolKind::Bucket => "Paint Bucket",
            ToolKind::Gradient => "Gradient",
            ToolKind::Line => "Line",
            ToolKind::Rectangle => "Rectangle",
            ToolKind::Ellipse => "Ellipse",
            ToolKind::Jumble => "Jumble",
            ToolKind::Text => "Text",
            ToolKind::Slice => "Slice",
            ToolKind::Artboard => "Artboard",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            ToolKind::RectSelect => "M",
            ToolKind::EllipseSelect => "Shift+M",
            ToolKind::Lasso => "Q",
            ToolKind::PolygonLasso => "Shift+Q",
            ToolKind::MagicWand => "W",
            ToolKind::Move => "V",
            ToolKind::Pencil => "B",
            ToolKind::Spray => "Shift+S",
            ToolKind::Eraser => "E",
            ToolKind::Eyedropper => "I",
            ToolKind::Bucket => "G",
            ToolKind::Gradient => "Shift+G",
            ToolKind::Line => "L",
            ToolKind::Rectangle => "U",
            ToolKind::Ellipse => "Shift+U",
            ToolKind::Jumble => "R",
            ToolKind::Text => "T",
            ToolKind::Slice => "Shift+C",
            ToolKind::Artboard => "Shift+O",
        }
    }

    /// Herramienta de un atajo de teclado (sin Ctrl/Alt).
    pub fn from_key(key: char, shift: bool) -> Option<ToolKind> {
        Some(match (key, shift) {
            ('M', false) => ToolKind::RectSelect,
            ('M', true) => ToolKind::EllipseSelect,
            ('Q', false) => ToolKind::Lasso,
            ('Q', true) => ToolKind::PolygonLasso,
            ('W', false) => ToolKind::MagicWand,
            ('V', false) => ToolKind::Move,
            ('B', false) => ToolKind::Pencil,
            ('S', true) => ToolKind::Spray,
            ('E', false) => ToolKind::Eraser,
            ('I', false) => ToolKind::Eyedropper,
            ('G', false) => ToolKind::Bucket,
            ('G', true) => ToolKind::Gradient,
            ('L', false) => ToolKind::Line,
            ('U', false) => ToolKind::Rectangle,
            ('U', true) => ToolKind::Ellipse,
            ('R', false) => ToolKind::Jumble,
            ('T', false) => ToolKind::Text,
            ('C', true) => ToolKind::Slice,
            ('O', true) => ToolKind::Artboard,
            _ => return None,
        })
    }

    pub fn is_selection(self) -> bool {
        matches!(
            self,
            ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::Lasso
                | ToolKind::PolygonLasso
                | ToolKind::MagicWand
        )
    }

    /// Herramientas que pintan y por lo tanto se recortan a la selección.
    pub fn paints(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Spray
                | ToolKind::Eraser
                | ToolKind::Bucket
                | ToolKind::Gradient
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Jumble
                | ToolKind::Text
        )
    }

    /// Herramientas que usan el tamaño de pincel.
    pub fn uses_brush(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Spray
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Jumble
        )
    }

    /// Herramientas que no actúan sobre una capa tilemap en modo Tiles (2A).
    pub fn blocked_in_tiles(self) -> bool {
        matches!(
            self,
            ToolKind::Spray
                | ToolKind::Jumble
                | ToolKind::Gradient
                | ToolKind::Text
                | ToolKind::Lasso
                | ToolKind::PolygonLasso
        )
    }

    /// Herramientas que en modo Tiles trabajan sobre el proxy de celdas (Move, Slice y Artboard
    /// siguen en píxeles).
    pub fn uses_cells(self) -> bool {
        matches!(
            self,
            ToolKind::Pencil
                | ToolKind::Eraser
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
                | ToolKind::Bucket
                | ToolKind::Eyedropper
                | ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::MagicWand
        )
    }
}

/// Cómo se combina una selección nueva con la existente.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    #[default]
    Replace,
    Add,
    Subtract,
    Intersect,
}

impl SelectionMode {
    pub const ALL: [SelectionMode; 4] = [
        SelectionMode::Replace,
        SelectionMode::Add,
        SelectionMode::Subtract,
        SelectionMode::Intersect,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SelectionMode::Replace => "Replace",
            SelectionMode::Add => "Add (Shift)",
            SelectionMode::Subtract => "Subtract (Alt)",
            SelectionMode::Intersect => "Intersect (Shift+Alt)",
        }
    }

    /// Combina la forma `shape` de un gesto con la selección `base`. `shape = None` es un clic
    /// sin forma: en Replace deselecciona; en los demás modos no cambia nada.
    pub fn combine(self, base: Option<&Mask>, shape: Option<Mask>) -> Option<Mask> {
        match (self, shape, base) {
            (SelectionMode::Replace, shape, _) => shape,
            (_, None, base) => base.cloned(),
            (SelectionMode::Add, Some(s), Some(b)) => Some(b.union(&s)),
            (SelectionMode::Add, Some(s), None) => Some(s),
            (SelectionMode::Subtract, Some(s), Some(b)) => Some(b.subtract(&s)),
            (SelectionMode::Intersect, Some(s), Some(b)) => Some(b.intersect(&s)),
            (SelectionMode::Subtract | SelectionMode::Intersect, Some(_), None) => None,
        }
    }
}

/// Texto que se está escribiendo (todavía no aplicado).
#[derive(Clone, Debug, PartialEq)]
pub struct TextDraft {
    /// Esquina superior izquierda, en píxeles del artboard activo.
    pub origin: Point,
    pub text: String,
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
    pub selection_mode: SelectionMode,
    /// Varita: diferencia máxima por canal.
    pub tolerance: u8,
    /// Varita: tomar los colores de todas las capas visibles.
    pub sample_all: bool,
    pub spray_width: u32,
    pub spray_speed: u32,
    pub gradient_shape: GradientShape,
    pub dither: Dither,
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            brush_size: 1,
            pixel_perfect: true,
            filled: false,
            contiguous: true,
            constrain: false,
            selection_mode: SelectionMode::Replace,
            tolerance: 0,
            sample_all: false,
            spray_width: 16,
            spray_speed: 32,
            gradient_shape: GradientShape::Linear,
            dither: Dither::None,
        }
    }
}

/// Herramientas que trabajan dentro de un sprite.
struct SpriteTools {
    rect_select: Marquee,
    ellipse_select: Marquee,
    lasso: Lasso,
    polygon_lasso: PolygonLasso,
    magic_wand: MagicWand,
    pencil: Pencil,
    spray: Spray,
    eraser: Eraser,
    bucket: Bucket,
    gradient: GradientTool,
    line: ShapeTool,
    rectangle: ShapeTool,
    ellipse: ShapeTool,
    jumble: Jumble,
    slice: SliceTool,
    mover: MoveTool,
    eyedropper: Eyedropper,
}

impl SpriteTools {
    fn new() -> Self {
        Self {
            rect_select: Marquee::new(MarqueeShape::Rectangle),
            ellipse_select: Marquee::new(MarqueeShape::Ellipse),
            lasso: Lasso::new(),
            polygon_lasso: PolygonLasso::new(),
            magic_wand: MagicWand::new(),
            pencil: Pencil::new(),
            spray: Spray::new(),
            eraser: Eraser::new(),
            bucket: Bucket::new(),
            gradient: GradientTool::new(),
            line: ShapeTool::new(ShapeKind::Line),
            rectangle: ShapeTool::new(ShapeKind::Rectangle),
            ellipse: ShapeTool::new(ShapeKind::Ellipse),
            jumble: Jumble::new(),
            slice: SliceTool::new(),
            mover: MoveTool::new(),
            eyedropper: Eyedropper::new(),
        }
    }

    fn get(&self, kind: ToolKind) -> Option<&dyn Tool> {
        Some(match kind {
            ToolKind::RectSelect => &self.rect_select,
            ToolKind::EllipseSelect => &self.ellipse_select,
            ToolKind::Lasso => &self.lasso,
            ToolKind::PolygonLasso => &self.polygon_lasso,
            ToolKind::MagicWand => &self.magic_wand,
            ToolKind::Pencil => &self.pencil,
            ToolKind::Spray => &self.spray,
            ToolKind::Eraser => &self.eraser,
            ToolKind::Bucket => &self.bucket,
            ToolKind::Gradient => &self.gradient,
            ToolKind::Line => &self.line,
            ToolKind::Rectangle => &self.rectangle,
            ToolKind::Ellipse => &self.ellipse,
            ToolKind::Jumble => &self.jumble,
            ToolKind::Slice => &self.slice,
            ToolKind::Move => &self.mover,
            ToolKind::Eyedropper => &self.eyedropper,
            ToolKind::Text | ToolKind::Artboard => return None,
        })
    }

    fn get_mut(&mut self, kind: ToolKind) -> Option<&mut dyn Tool> {
        Some(match kind {
            ToolKind::RectSelect => &mut self.rect_select,
            ToolKind::EllipseSelect => &mut self.ellipse_select,
            ToolKind::Lasso => &mut self.lasso,
            ToolKind::PolygonLasso => &mut self.polygon_lasso,
            ToolKind::MagicWand => &mut self.magic_wand,
            ToolKind::Pencil => &mut self.pencil,
            ToolKind::Spray => &mut self.spray,
            ToolKind::Eraser => &mut self.eraser,
            ToolKind::Bucket => &mut self.bucket,
            ToolKind::Gradient => &mut self.gradient,
            ToolKind::Line => &mut self.line,
            ToolKind::Rectangle => &mut self.rectangle,
            ToolKind::Ellipse => &mut self.ellipse,
            ToolKind::Jumble => &mut self.jumble,
            ToolKind::Slice => &mut self.slice,
            ToolKind::Move => &mut self.mover,
            ToolKind::Eyedropper => &mut self.eyedropper,
            ToolKind::Text | ToolKind::Artboard => return None,
        })
    }
}

/// Refleja un `DrawPixels` según la simetría; otros comandos pasan igual.
fn mirror(cmd: Command, symmetry: cs_core::Symmetry, center2: (i32, i32)) -> Command {
    match cmd {
        Command::DrawPixels {
            layer,
            frame,
            pixels,
        } if symmetry.mode != cs_core::SymmetryMode::None => Command::DrawPixels {
            layer,
            frame,
            pixels: cs_raster::symmetrize(&pixels, symmetry.mode, center2),
        },
        other => other,
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
pub struct DocView<'a> {
    /// El documento, con el gesto en curso aplicado si lo hay.
    pub document: Cow<'a, Document>,
    pub selection: Option<Mask>,
}

/// Qué hace un trazo en modo Tiles.
enum TilesEdit {
    Apply(Command),
    /// Nada cambia (o la capa está bloqueada).
    Nothing,
    /// La grilla pasaría el límite de tamaño de `ops::set_tiles`.
    TooLarge,
}

/// Lo que edita un gesto agrupado: artboard, capa, frame y slice seleccionados.
type MergeTarget = (ArtboardId, LayerId, usize, Option<usize>);

/// Estado del documento abierto y de la edición. No sabe nada de egui.
pub struct Editor {
    pub document: Document,
    pub history: History<DocCommand>,
    pub active: ArtboardId,
    pub layer: LayerId,
    pub frame: usize,
    pub primary: PixelValue,
    pub secondary: PixelValue,
    pub tool: ToolKind,
    pub options: ToolOptions,
    /// Selección dentro del artboard activo.
    pub selection: Option<Mask>,
    /// Aumenta cada vez que el documento o la selección cambian.
    pub revision: u64,
    /// Capa y frame recordados por artboard.
    views: HashMap<ArtboardId, (LayerId, usize)>,
    tools: SpriteTools,
    artboard_tool: ArtboardTool,
    painting_secondary: bool,
    /// Alt al presionar: cuentagotas temporal durante el gesto.
    temp_eyedropper: bool,
    /// Arrastre dentro de la selección con una herramienta de selección: mueve su contenido
    /// (como Move). Guarda el punto de partida y si el puntero ya se desplazó.
    temp_move: Option<(Point, bool)>,
    /// Reproducción en curso.
    pub playing: bool,
    /// Estado de la reproducción: posición, rango (inclusivo) y dirección.
    pub(crate) playback: Option<(Playback, (usize, usize), AnimDirection)>,
    /// Milisegundos acumulados en el frame actual durante la reproducción.
    pub(crate) play_clock: f64,
    /// Rango de frames seleccionado en el timeline (para tags, duraciones…).
    pub frame_range: Option<(usize, usize)>,
    /// Grupos colapsados en el timeline (estado de UI, por artboard).
    pub collapsed: HashSet<(ArtboardId, LayerId)>,
    pub onion_enabled: bool,
    pub onion: OnionSkin,
    /// Cambia con cada modificación del documento (ejecutar, deshacer, rehacer); sirve para
    /// saber si hay cambios sin guardar.
    pub doc_version: u64,
    /// A qué se pegan las herramientas de forma (lo actualiza el lienzo cada frame).
    pub snap: Option<crate::guides::SnapContext>,
    /// Última selección descartada (para Reselect).
    last_selection: Option<Mask>,
    /// Modo de combinación del gesto de selección en curso.
    gesture_mode: SelectionMode,
    /// Shift ya estaba presionado al empezar el gesto (se usó para el modo, no para restringir).
    shift_at_press: bool,
    /// Texto en edición con la herramienta Text.
    pub text: Option<TextDraft>,
    /// Las herramientas pintan en la máscara de la capa actual (si tiene).
    pub edit_mask: bool,
    /// Slice seleccionado del artboard activo (índice en `sprite.slices`).
    pub slice: Option<usize>,
    pub text_style: TextStyle,
    /// Gesto en curso (p. ej. arrastrar un campo del panel) cuyos cambios se juntan en un solo
    /// paso de undo: id del widget y lo que se estaba editando (artboard, capa, frame, slice).
    /// El mismo widget sirve para todas las capas, así que el objetivo también tiene que coincidir.
    merge: Option<(u64, MergeTarget)>,
    /// Tile con el que pintan las herramientas en modo Tiles (índice y flips). Ver `brush_tile()`.
    pub(crate) brush_tile: Tile,
    /// Artboard y tileset para los que se eligió `brush_tile`; en otro tileset el pincel es el 1.
    pub(crate) brush_tileset: Option<(ArtboardId, u32)>,
    /// El último trazo en modo Tiles no se aplicó porque la grilla pasaría el límite de tamaño.
    pub(crate) tile_limit: bool,
}

impl Editor {
    pub fn new(document: Document) -> Self {
        assert!(
            !document.artboards.is_empty(),
            "a document needs at least one artboard"
        );
        let active = document.artboards[0].id;
        let mut editor = Self {
            document,
            history: History::new(),
            active,
            layer: LayerId(1),
            frame: 0,
            primary: PixelValue::Index(0),
            secondary: PixelValue::Index(0),
            tool: ToolKind::Pencil,
            options: ToolOptions::default(),
            selection: None,
            revision: 0,
            views: HashMap::new(),
            tools: SpriteTools::new(),
            artboard_tool: ArtboardTool::new(),
            painting_secondary: false,
            temp_eyedropper: false,
            temp_move: None,
            playing: false,
            playback: None,
            play_clock: 0.0,
            frame_range: None,
            collapsed: HashSet::new(),
            onion_enabled: false,
            onion: OnionSkin::default(),
            doc_version: 0,
            snap: None,
            last_selection: None,
            gesture_mode: SelectionMode::Replace,
            shift_at_press: false,
            text: None,
            text_style: TextStyle::default(),
            merge: None,
            edit_mask: false,
            slice: None,
            brush_tile: Tile::new(1, false, false, false),
            brush_tileset: None,
            tile_limit: false,
        };
        editor.ensure_valid();
        editor.primary = editor.palette_value(0);
        editor.secondary = editor.palette_value(21);
        editor
    }

    pub fn with_sprite(sprite: Sprite) -> Self {
        Self::new(Document::with_sprite("Artboard 1", sprite))
    }

    pub fn artboard(&self) -> &Artboard {
        self.document
            .artboard(self.active)
            .expect("active artboard exists")
    }

    pub fn sprite(&self) -> &Sprite {
        &self.artboard().sprite
    }

    /// Mantiene invariantes tras cambios en el documento (p. ej. undo de un artboard).
    fn ensure_valid(&mut self) {
        if self.document.artboard(self.active).is_none() {
            self.active = self.document.artboards[0].id;
            self.selection = None;
        }
        let sprite = &self
            .document
            .artboard(self.active)
            .expect("active artboard exists")
            .sprite;
        if sprite.layer(self.layer).is_none() {
            self.layer = sprite.layers.last().map_or(LayerId(1), |l| l.id);
        }
        self.frame = self.frame.min(sprite.frames.len().saturating_sub(1));
        let slices = sprite.slices.len();
        self.slice = self.slice.filter(|&i| i < slices);
        let last = sprite.frames.len().saturating_sub(1);
        self.frame_range = self.frame_range.map(|(a, b)| (a.min(last), b.min(last)));
        let (w, h) = (sprite.width, sprite.height);
        if self
            .selection
            .as_ref()
            .is_some_and(|m| (m.width(), m.height()) != (w, h))
        {
            self.selection = None;
        }
    }

    /// Activa otro artboard: recuerda capa/frame del actual y descarta la selección.
    pub fn activate(&mut self, id: ArtboardId) {
        if id == self.active || self.document.artboard(id).is_none() {
            return;
        }
        self.stop();
        self.end_group();
        self.frame_range = None;
        self.views.insert(self.active, (self.layer, self.frame));
        self.active = id;
        (self.layer, self.frame) = self.views.get(&id).copied().unwrap_or((LayerId(0), 0));
        self.selection = None;
        self.slice = None;
        self.ensure_valid();
        self.revision += 1;
    }

    /// Valor de píxel para la entrada `index` de la paleta según el modo de color.
    pub fn palette_value(&self, index: u8) -> PixelValue {
        let sprite = self.sprite();
        match sprite.color_mode {
            ColorMode::Indexed { .. } => PixelValue::Index(index),
            ColorMode::Rgba => PixelValue::Rgba(sprite.palette.get(index).unwrap_or_default()),
        }
    }

    pub fn resolve(&self, value: PixelValue) -> Rgba {
        cs_render::resolve(self.sprite(), value)
    }

    /// Ejecuta un comando sobre el artboard activo; devuelve si se aplicó.
    pub fn execute(&mut self, cmd: Command) -> bool {
        self.execute_doc(DocCommand::OnArtboard {
            id: self.active,
            cmd,
        })
    }

    pub fn execute_doc(&mut self, cmd: DocCommand) -> bool {
        self.merge = None;
        match self.history.execute(&mut self.document, cmd) {
            Ok(()) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
                true
            }
            Err(e) => {
                log::warn!("command rejected: {e}");
                false
            }
        }
    }

    /// Como `execute`, pero los comandos seguidos del mismo `group` forman un solo paso de undo.
    /// El comando debe reemplazar un estado completo (como `SetInstanceTrack` o `SetSlices`) para
    /// que el inverso del primero deshaga todo el gesto.
    pub fn execute_grouped(&mut self, cmd: Command, group: u64) -> bool {
        let cmd = DocCommand::OnArtboard {
            id: self.active,
            cmd,
        };
        let target = (self.active, self.layer, self.frame, self.slice);
        let result = if self.merge == Some((group, target)) {
            self.history.execute_merged(&mut self.document, cmd)
        } else {
            self.history.execute(&mut self.document, cmd)
        };
        match result {
            Ok(()) => {
                self.merge = Some((group, target));
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
                true
            }
            Err(e) => {
                log::warn!("command rejected: {e}");
                false
            }
        }
    }

    /// Termina el gesto agrupado en curso: el próximo cambio es un paso de undo nuevo.
    pub fn end_group(&mut self) {
        self.merge = None;
    }

    pub fn undo(&mut self) {
        self.merge = None;
        match self.history.undo(&mut self.document) {
            Ok(true) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
            }
            Ok(false) => {}
            Err(e) => log::error!("undo failed: {e}"),
        }
    }

    pub fn redo(&mut self) {
        self.merge = None;
        match self.history.redo(&mut self.document) {
            Ok(true) => {
                self.revision += 1;
                self.doc_version += 1;
                self.ensure_valid();
            }
            Ok(false) => {}
            Err(e) => log::error!("redo failed: {e}"),
        }
    }

    /// Crea un artboard vacío del tamaño dado a la derecha de los existentes y lo activa.
    pub fn add_artboard(&mut self, width: u32, height: u32) {
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let template = self.sprite();
        let mut sprite = Sprite::new(width, height, template.color_mode);
        sprite.palette = template.palette.clone();
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            guides: Vec::new(),
            symmetry: Default::default(),
            id,
            name: format!("Artboard {}", id.0),
            position: (x1 + 16, y0),
            sprite,
        };
        if self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        }) {
            self.activate(id);
        }
    }

    pub fn remove_active_artboard(&mut self) {
        if self.document.artboards.len() > 1 {
            self.execute_doc(DocCommand::RemoveArtboard { id: self.active });
        }
    }

    pub fn rename_active_artboard(&mut self, name: String) {
        if name != self.artboard().name {
            self.execute_doc(DocCommand::SetArtboardProps {
                id: self.active,
                name: Some(name),
                position: None,
            });
        }
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
    }

    pub fn set_selection(&mut self, selection: Option<Mask>) {
        let selection = selection.filter(|m| !m.is_empty());
        if selection.is_none()
            && let Some(old) = self.selection.take()
        {
            self.last_selection = Some(old);
        }
        self.selection = selection;
        self.revision += 1;
    }

    /// Vuelve a la última selección descartada (si sigue cabiendo en el lienzo).
    pub fn reselect(&mut self) {
        let s = self.sprite();
        let (w, h) = (s.width, s.height);
        if let Some(last) = self.last_selection.clone()
            && (last.width(), last.height()) == (w, h)
        {
            self.set_selection(Some(last));
        }
    }

    /// Invierte la selección; sin selección, selecciona todo.
    pub fn invert_selection(&mut self) {
        let s = self.sprite();
        let inverted = match &self.selection {
            Some(mask) => mask.inverted(),
            None => Mask::all(s.width, s.height),
        };
        self.set_selection(Some(inverted));
    }

    pub fn can_reselect(&self) -> bool {
        self.last_selection.is_some()
    }

    /// Cambia de herramienta descartando gestos de varios clics a medias.
    pub fn set_tool(&mut self, kind: ToolKind) {
        if kind != self.tool {
            self.cancel_gesture();
            self.tool = kind;
        }
    }

    pub fn select_all(&mut self) {
        let s = self.sprite();
        self.set_selection(Some(Mask::all(s.width, s.height)));
    }

    pub fn deselect(&mut self) {
        self.set_selection(None);
    }

    /// Borra (deja transparente) el contenido seleccionado de la capa actual.
    pub fn clear_selection_content(&mut self) {
        let Some(mask) = &self.selection else {
            return;
        };
        if self.editing_mask() {
            let pixels = mask.points().map(|(x, y)| (x, y, 0)).collect();
            self.execute(Command::DrawMask {
                layer: self.layer,
                pixels,
            });
            return;
        }
        if self.tiles_mode() {
            self.clear_selected_cells();
            return;
        }
        let t = self.sprite().color_mode.transparent();
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
        } else if self.temp_move.is_some() {
            ToolKind::Move
        } else {
            self.tool
        }
    }

    /// Gesto de la herramienta Artboard en curso (crear, mover o redimensionar).
    pub fn artboard_gesture(&self) -> &ArtboardTool {
        &self.artboard_tool
    }

    pub fn is_painting(&self) -> bool {
        self.artboard_tool.is_active()
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

    /// Aplica el texto en edición con el color primario (un paso de undo).
    pub fn commit_text(&mut self) {
        let Some(cmd) = self.text_command() else {
            self.text = None;
            return;
        };
        self.text = None;
        let cmd = self.shape_paint(ToolKind::Text, cmd);
        self.execute(cmd);
    }

    fn text_command(&self) -> Option<Command> {
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

    /// Punto de mundo → píxel local del artboard activo.
    /// Frame a mostrar de un artboard: el actual si es el activo; si no, el recordado.
    pub fn frame_of(&self, id: ArtboardId) -> usize {
        if id == self.active {
            self.frame
        } else {
            self.views.get(&id).map_or(0, |v| v.1)
        }
    }

    pub fn to_local(&self, world: (f32, f32)) -> Point {
        let (x, y) = self.artboard().position;
        (
            (world.0 - x as f32).floor() as i32,
            (world.1 - y as f32).floor() as i32,
        )
    }

    fn color(&self) -> PixelValue {
        if self.painting_secondary {
            self.secondary
        } else {
            self.primary
        }
    }

    /// Shift restringe la forma, salvo en selección si al empezar eligió el modo Add.
    fn constrain(&self) -> bool {
        self.options.constrain && !(self.active_tool().is_selection() && self.shift_at_press)
    }

    fn context(&self) -> ToolContext<'_> {
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
            pixel_perfect: self.options.pixel_perfect,
            filled: self.options.filled,
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
        }
    }

    /// Contexto sobre el proxy de celdas: los dos colores son el tile del pincel codificado, el
    /// pincel se mide en celdas y la selección es la del proxy. Pixel-perfect no aplica.
    fn tiles_context<'a>(&'a self, proxy: &'a TileProxy) -> ToolContext<'a> {
        let brush = PixelValue::Rgba(tile_ops::encode(self.brush_tile()));
        ToolContext {
            sprite: &proxy.sprite,
            layer: tile_ops::PROXY_LAYER,
            frame: 0,
            color: brush,
            secondary: brush,
            brush_size: self.options.brush_size,
            pixel_perfect: false,
            filled: self.options.filled,
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
        }
    }

    /// Proxy de celdas para `kind` si la capa activa está en modo Tiles y la herramienta trabaja
    /// en celdas.
    fn cell_proxy(&self, kind: ToolKind) -> Option<TileProxy> {
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
        self.painting_secondary = secondary;
        // Alt es cuentagotas temporal solo en las herramientas que pintan.
        self.temp_eyedropper = alt && self.tool.paints();
        let shift = self.options.constrain;
        self.temp_move = self
            .moves_selection_at(world, shift, alt)
            .then(|| (self.to_local(world), false));
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
            if let Some(id) =
                self.artboard_tool
                    .press(&self.document, world, slop, Some(self.active))
            {
                self.activate(id);
            }
            return;
        }
        if let Some(id) = self.document.artboard_at(world) {
            self.activate(id);
        }
        let p = self.tool_point(world);
        // En modo Tiles algunas herramientas no actúan (la barra de estado lo avisa).
        if self.tiles_mode() && self.active_tool().blocked_in_tiles() {
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
        // Mover sin selección: cambia a la capa que tiene algo dibujado bajo el puntero.
        // En modo Tiles Move se queda en la capa tilemap.
        if self.active_tool() == ToolKind::Move
            && self.selection.is_none()
            && !self.tiles_mode()
            && let Some(layer) = self.layer_at(p)
        {
            self.layer = layer;
        }
        self.with_tool(|tool, ctx| tool.press(ctx, p));
    }

    /// Presionar en `world` con una herramienta de selección movería lo seleccionado: el punto
    /// cae dentro de la selección y no hay modificadores que cambien el modo de selección.
    pub fn moves_selection_at(&self, world: (f32, f32), shift: bool, alt: bool) -> bool {
        let p = self.to_local(world);
        self.tool.is_selection()
            && !shift
            && !alt
            && !self.tiles_mode()
            && !self.editing_mask()
            && !self.tools.get(self.tool).is_some_and(|t| t.is_pending())
            && self.document.artboard_at(world) == Some(self.active)
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
        let p = self.tool_point(world);
        let local = self.to_local(world);
        if let Some((from, moved)) = &mut self.temp_move {
            *moved |= local != *from;
        }
        self.with_tool(|tool, ctx| tool.drag(ctx, p));
    }

    /// Pega el punto a guías/grid si la herramienta activa lo usa (no las de trazo libre).
    fn snapped(&self, world: (f32, f32)) -> (f32, f32) {
        let snaps = matches!(
            self.active_tool(),
            ToolKind::RectSelect
                | ToolKind::EllipseSelect
                | ToolKind::PolygonLasso
                | ToolKind::Gradient
                | ToolKind::Text
                | ToolKind::Move
                | ToolKind::Line
                | ToolKind::Rectangle
                | ToolKind::Ellipse
        );
        let Some(ctx) = self.snap.as_ref().filter(|_| snaps) else {
            return world;
        };
        let (ox, oy) = self.artboard().position;
        let (lx, ly) = ctx.snap((world.0 - ox as f32, world.1 - oy as f32));
        (lx + ox as f32, ly + oy as f32)
    }

    pub fn guides(&self) -> &[cs_core::Guide] {
        &self.artboard().guides
    }

    /// Reemplaza las guías del artboard activo (un paso de undo).
    pub fn set_guides(&mut self, guides: Vec<cs_core::Guide>) {
        if guides.as_slice() != self.guides() {
            self.execute_doc(DocCommand::SetGuides {
                id: self.active,
                guides,
            });
        }
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
        let kind = self.active_tool();
        if self.tiles_mode() && kind.blocked_in_tiles() {
            self.temp_eyedropper = false;
            return;
        }
        let out = self.with_tool(|tool, ctx| tool.release(ctx));
        self.temp_eyedropper = false;
        if let Some((from, moved)) = self.temp_move.take() {
            if moved {
                self.apply_output(kind, out);
            } else {
                // Un clic sin arrastrar es un clic normal de la herramienta de selección.
                let kind = self.tool;
                self.with_tool(|tool, ctx| tool.press(ctx, from));
                let out = self.with_tool(|tool, ctx| tool.release(ctx));
                self.apply_output(kind, out);
            }
            return;
        }
        self.apply_output(kind, out);
    }

    /// Simetría y recorte a la selección para las herramientas que pintan.
    fn shape_paint(&self, kind: ToolKind, cmd: Command) -> Command {
        if !kind.paints() {
            return cmd;
        }
        let artboard = self.artboard();
        let cmd = if kind == ToolKind::Text {
            cmd
        } else {
            mirror(cmd, artboard.symmetry, artboard.symmetry_center2())
        };
        let cmd = match &self.selection {
            Some(mask) => clip(cmd, mask),
            None => cmd,
        };
        if self.editing_mask() {
            self.to_mask_command(cmd)
        } else {
            cmd
        }
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

    pub fn symmetry(&self) -> cs_core::Symmetry {
        self.artboard().symmetry
    }

    /// Cambia la simetría del artboard activo (un paso de undo).
    pub fn set_symmetry(&mut self, symmetry: cs_core::Symmetry) {
        if symmetry != self.symmetry() {
            self.execute_doc(DocCommand::SetSymmetry {
                id: self.active,
                symmetry,
            });
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
        let mut applied = true;
        if let Some(cmd) = &out.command {
            match self.tiles_edit(proxy, cmd) {
                TilesEdit::Apply(cmd) => {
                    applied = self.execute(cmd);
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
    fn tiles_view(&self, kind: ToolKind, out: ToolOutput, proxy: &TileProxy) -> DocView<'_> {
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
    fn combine_selection(&self, kind: ToolKind, shape: Option<Mask>) -> Option<Mask> {
        if kind.is_selection() {
            self.gesture_mode.combine(self.selection.as_ref(), shape)
        } else {
            shape
        }
    }

    /// Documento a mostrar: el actual o un clon con el gesto en curso aplicado.
    pub fn view(&self) -> DocView<'_> {
        let current = || DocView {
            document: Cow::Borrowed(&self.document),
            selection: self.selection.clone(),
        };
        if self.artboard_tool.is_active() {
            let Some(cmd) = self.artboard_tool.preview(&self.document) else {
                return current();
            };
            let mut doc = self.document.clone();
            return match cmd.apply(&mut doc) {
                Ok(_) => DocView {
                    document: Cow::Owned(doc),
                    // La selección no aplica si el tamaño cambia durante el gesto.
                    selection: None,
                },
                Err(_) => current(),
            };
        }
        if let Some(cmd) = self.text_command() {
            let cmd = self.shape_paint(ToolKind::Text, cmd);
            let mut doc = self.document.clone();
            let applied = DocCommand::OnArtboard {
                id: self.active,
                cmd,
            }
            .apply(&mut doc);
            return DocView {
                document: if applied.is_ok() {
                    Cow::Owned(doc)
                } else {
                    Cow::Borrowed(&self.document)
                },
                selection: self.selection.clone(),
            };
        }
        let kind = self.active_tool();
        let Some(tool) = self
            .tools
            .get(kind)
            .filter(|t| t.is_active() || t.is_pending())
        else {
            return current();
        };
        if let Some(proxy) = self.cell_proxy(kind) {
            let out = tool.preview(&self.tiles_context(&proxy));
            return self.tiles_view(kind, out, &proxy);
        }
        let out = tool.preview(&self.context());
        let selection = match out.selection {
            Some(sel) => self.combine_selection(kind, sel),
            None => self.selection.clone(),
        };
        let Some(cmd) = out.command else {
            return DocView {
                document: Cow::Borrowed(&self.document),
                selection,
            };
        };
        let cmd = self.shape_paint(kind, cmd);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile_ops::test_util::{t, tile_at, tile_editor};
    use cs_commands::ops;

    fn editor() -> Editor {
        Editor::with_sprite(Sprite::new(8, 8, ColorMode::Rgba))
    }

    /// Clic en coordenadas locales del artboard activo.
    fn click(e: &mut Editor, p: Point, secondary: bool, alt: bool) {
        let (x, y) = e.artboard().position;
        let world = ((x + p.0) as f32 + 0.5, (y + p.1) as f32 + 0.5);
        e.pointer_press(world, secondary, alt, 0.1);
        e.pointer_release();
    }

    /// Arrastre en coordenadas locales (centro de píxel) con Shift/Alt al presionar.
    fn drag_local(e: &mut Editor, a: Point, b: Point, shift: bool, alt: bool) {
        let at = |p: Point| (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
        e.options.constrain = shift;
        e.pointer_press(at(a), false, alt, 0.1);
        e.pointer_drag(at(b));
        e.pointer_release();
        e.options.constrain = false;
    }

    #[test]
    fn selection_modes_from_modifiers() {
        let mut e = editor();
        e.tool = ToolKind::RectSelect;
        drag_local(&mut e, (0, 0), (3, 3), false, false);
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 16);
        drag_local(&mut e, (4, 0), (5, 0), true, false); // Shift: add
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 18);
        drag_local(&mut e, (0, 0), (1, 1), false, true); // Alt: subtract (no cuentagotas)
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 14);
        drag_local(&mut e, (2, 2), (7, 7), true, true); // Shift+Alt: intersect
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
        // Modo elegido en las opciones.
        e.options.selection_mode = SelectionMode::Add;
        drag_local(&mut e, (7, 7), (7, 7), false, false); // clic: no cambia en Add
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
        e.options.selection_mode = SelectionMode::Replace;
        drag_local(&mut e, (7, 7), (7, 7), false, false);
        assert!(e.selection.is_none(), "click deselects in Replace");
        e.reselect();
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 4);
        e.invert_selection();
        assert_eq!(e.selection.as_ref().unwrap().points().count(), 60);
        e.deselect();
        e.invert_selection();
        assert_eq!(
            e.selection.as_ref().unwrap().points().count(),
            64,
            "inverse of nothing = all"
        );
    }

    #[test]
    fn polygon_lasso_by_clicks_enter_and_escape() {
        let mut e = editor();
        e.set_tool(ToolKind::PolygonLasso);
        for p in [(0, 0), (6, 0), (6, 6)] {
            click(&mut e, p, false, false);
        }
        assert!(e.is_pending() && !e.is_painting());
        e.pointer_hover((0.5, 6.5));
        assert!(e.view().selection.is_some(), "preview while pending");
        e.finish_gesture();
        assert!(!e.is_pending());
        assert!(e.selection.as_ref().unwrap().contains(5, 1));
        click(&mut e, (1, 1), false, false);
        e.cancel_gesture();
        assert!(!e.is_pending());
        // Cambiar de herramienta descarta el gesto a medias.
        click(&mut e, (1, 1), false, false);
        e.set_tool(ToolKind::Pencil);
        e.set_tool(ToolKind::PolygonLasso);
        assert!(!e.is_pending());
    }

    #[test]
    fn text_draft_previews_commits_and_cancels() {
        let mut e = editor();
        e.set_tool(ToolKind::Text);
        click(&mut e, (1, 0), false, false);
        e.text.as_mut().unwrap().text = "I".into();
        assert!(!e.history.can_undo());
        let preview = e.view().document.artboards[0].sprite.clone();
        assert!(preview.cels.contains_key(&(LayerId(1), 0)), "preview draws");
        click(&mut e, (2, 0), false, false); // mueve el texto
        e.commit_text();
        assert!(e.text.is_none());
        let px = e.sprite().pixel(LayerId(1), 0, 4, 3);
        assert_eq!(px, e.primary, "the I stem at x = 2 + 2");
        e.undo();
        assert!(e.sprite().cels.is_empty());
        click(&mut e, (0, 0), false, false);
        e.cancel_gesture();
        assert!(e.text.is_none());
    }

    #[test]
    fn spray_keeps_painting_while_held_and_gradient_fills() {
        let mut e = editor();
        e.set_tool(ToolKind::Spray);
        e.options.spray_width = 4;
        e.pointer_press((4.5, 4.5), false, false, 0.1);
        e.tool_tick(1000.0);
        e.pointer_release();
        let painted = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| e.sprite().pixel(LayerId(1), 0, x, y) == e.primary)
            .count();
        assert!(painted > 3, "{painted}");

        let mut e = editor();
        e.set_tool(ToolKind::Gradient);
        e.options.dither = Dither::Bayer2;
        drag_local(&mut e, (0, 0), (7, 0), false, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 0, 5), e.primary);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 7, 5), e.secondary);
    }

    fn two_boards() -> Editor {
        let mut e = editor();
        e.add_artboard(4, 4);
        e
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
        assert!(e.sprite().cels.is_empty());
        e.redo();
        assert!(!e.sprite().cels.is_empty());
    }

    #[test]
    fn secondary_button_paints_secondary_color() {
        let mut e = editor();
        click(&mut e, (1, 1), true, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), e.secondary);
    }

    #[test]
    fn painting_is_clipped_to_selection() {
        let mut e = editor();
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (1, 1))));
        e.tool = ToolKind::Bucket;
        e.options.contiguous = false;
        click(&mut e, (0, 0), false, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), e.primary);
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 2, 2),
            e.sprite().color_mode.transparent()
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
        assert_eq!(e.secondary, e.sprite().color_mode.transparent());
    }

    #[test]
    fn delete_clears_selected_content() {
        let mut e = editor();
        e.tool = ToolKind::Bucket;
        click(&mut e, (0, 0), false, false);
        e.set_selection(Some(Mask::from_rect(8, 8, (2, 2), (3, 3))));
        e.clear_selection_content();
        let t = e.sprite().color_mode.transparent();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), t);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 0, 0), e.primary);
    }

    #[test]
    fn delete_in_tiles_mode_clears_the_selected_cells_in_one_undo_step() {
        use crate::tile_ops::test_util::{t, tile_at, tile_editor};
        let mut e = tile_editor();
        let cmd = cs_commands::ops::set_tiles(
            e.sprite(),
            e.layer,
            0,
            &[(0, 0, t(1)), (1, 0, t(2)), (3, 3, t(1))],
        )
        .unwrap();
        e.execute(cmd);
        // Toca las celdas (0, 0) y (1, 0).
        e.set_selection(Some(Mask::from_rect(16, 16, (3, 1), (5, 1))));
        e.clear_selection_content();
        assert_eq!(tile_at(&e, (0, 0)), cs_core::Tile::EMPTY);
        assert_eq!(tile_at(&e, (1, 0)), cs_core::Tile::EMPTY);
        assert_eq!(tile_at(&e, (3, 3)), t(1), "outside the selection");
        e.undo();
        assert_eq!(tile_at(&e, (0, 0)), t(1));
        assert_eq!(tile_at(&e, (1, 0)), t(2));
    }

    #[test]
    fn delete_does_nothing_on_a_locked_tilemap_layer() {
        use crate::tile_ops::test_util::{t, tile_at, tile_editor};
        let mut e = tile_editor();
        let cmd = cs_commands::ops::set_tiles(e.sprite(), e.layer, 0, &[(0, 0, t(1))]).unwrap();
        e.execute(cmd);
        e.execute(Command::SetLayerProps {
            id: e.layer,
            props: cs_commands::LayerProps {
                locked: Some(true),
                ..Default::default()
            },
        });
        e.set_selection(Some(Mask::all(16, 16)));
        let version = e.doc_version;
        e.clear_selection_content();
        assert_eq!(tile_at(&e, (0, 0)), t(1), "the locked layer is untouched");
        assert_eq!(e.doc_version, version, "no command ran");
        e.undo();
        assert!(
            !e.sprite().layer(e.layer).unwrap().locked,
            "the last undo step was the lock, not a clear"
        );
    }

    #[test]
    fn preview_shows_stroke_without_touching_document() {
        let mut e = editor();
        e.pointer_press((0.5, 0.5), false, false, 0.1);
        let view = e.view();
        let preview = &view.document.artboards[0].sprite;
        assert_eq!(preview.pixel(LayerId(1), 0, 0, 0), e.primary);
        drop(view);
        assert!(e.sprite().cels.is_empty());
        e.pointer_release();
        assert!(!e.sprite().cels.is_empty());
    }

    #[test]
    fn drawing_only_touches_active_artboard_and_click_activates() {
        let mut e = two_boards();
        let second = e.active;
        click(&mut e, (0, 0), false, false);
        assert!(e.document.artboards[0].sprite.cels.is_empty());
        assert!(!e.document.artboards[1].sprite.cels.is_empty());
        // Clic sobre el primer artboard: lo activa y pinta ahí.
        e.pointer_press((0.5, 0.5), false, false, 0.1);
        e.pointer_release();
        assert_ne!(e.active, second);
        assert!(!e.document.artboards[0].sprite.cels.is_empty());
    }

    #[test]
    fn undo_is_chronological_across_artboards() {
        let mut e = two_boards();
        click(&mut e, (0, 0), false, false);
        e.activate(e.document.artboards[0].id);
        click(&mut e, (0, 0), false, false);
        e.undo();
        assert!(e.document.artboards[0].sprite.cels.is_empty());
        assert!(!e.document.artboards[1].sprite.cels.is_empty());
        e.undo();
        assert!(e.document.artboards[1].sprite.cels.is_empty());
        e.undo();
        assert_eq!(e.document.artboards.len(), 1);
        assert_eq!(e.active, e.document.artboards[0].id);
    }

    #[test]
    fn activating_drops_selection_and_remembers_layer() {
        let mut e = two_boards();
        let (first, second) = (e.document.artboards[0].id, e.document.artboards[1].id);
        e.execute(Command::AddLayer {
            index: 1,
            layer: cs_core::Layer::new_image(LayerId(2), "top"),
        });
        e.layer = LayerId(2);
        e.select_all();
        e.activate(first);
        assert!(e.selection.is_none());
        assert_eq!(e.layer, LayerId(1));
        e.activate(second);
        assert_eq!(e.layer, LayerId(2));
    }

    #[test]
    fn cannot_remove_last_artboard() {
        let mut e = editor();
        e.remove_active_artboard();
        assert_eq!(e.document.artboards.len(), 1);
    }

    #[test]
    fn artboard_tool_creates_and_activates() {
        let mut e = editor();
        e.tool = ToolKind::Artboard;
        e.pointer_press((20.0, 0.0), false, false, 0.1);
        e.pointer_drag((29.0, 9.0));
        e.pointer_release();
        assert_eq!(e.document.artboards.len(), 2);
        assert_eq!(e.active, e.document.artboards[1].id);
        assert_eq!((e.sprite().width, e.sprite().height), (10, 10));
    }

    #[test]
    fn dragging_label_moves_artboard_with_any_tool() {
        let mut e = two_boards();
        let first = e.document.artboards[0].id;
        e.tool = ToolKind::Pencil;
        e.press_artboard_label(first, (1.0, -2.0));
        assert!(e.is_painting());
        e.pointer_drag((6.0, -2.0));
        e.pointer_release();
        assert_eq!(e.active, first);
        assert_eq!(e.artboard().position, (5, 0));
        assert!(e.sprite().cels.is_empty(), "no debe pintar");
        e.undo();
        assert_eq!(e.artboard().position, (0, 0));
    }

    #[test]
    fn shape_tools_snap_to_guides_but_pencil_does_not() {
        use cs_core::{Axis, Guide};
        let mut e = editor();
        e.set_guides(vec![Guide {
            axis: Axis::Vertical,
            position: 4,
        }]);
        e.snap = Some(crate::guides::SnapContext {
            guides: e.guides().to_vec(),
            grid: None,
            tolerance: 1.0,
        });
        e.tool = ToolKind::RectSelect;
        e.pointer_press((0.5, 0.5), false, false, 0.1);
        e.pointer_drag((4.3, 2.5)); // cerca de x = 4 → píxel 4 (lado del puntero)
        e.pointer_release();
        assert_eq!(
            e.selection.as_ref().unwrap().bounds(),
            Some(((0, 0), (4, 2)))
        );
        e.deselect();
        e.tool = ToolKind::Pencil;
        e.pointer_press((3.6, 1.5), false, false, 0.1);
        e.pointer_release();
        assert_eq!(
            e.sprite().pixel(LayerId(1), 0, 3, 1),
            e.primary,
            "pencil ignores snap"
        );
        e.undo();
        e.undo();
        assert!(e.guides().is_empty(), "guides are undoable");
    }

    #[test]
    fn symmetric_strokes_are_one_undo_step_and_respect_selection() {
        use cs_core::{Symmetry, SymmetryMode};
        let mut e = editor(); // 8×8
        e.set_symmetry(Symmetry {
            mode: SymmetryMode::Horizontal,
            center2: None,
        });
        click(&mut e, (1, 1), false, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 6, 1), e.primary);
        e.undo();
        assert!(e.sprite().cels.is_empty(), "both sides undone together");
        // Con selección solo en la mitad izquierda, el reflejo queda fuera.
        e.set_selection(Some(Mask::from_rect(8, 8, (0, 0), (3, 7))));
        click(&mut e, (2, 2), false, false);
        let t = e.sprite().color_mode.transparent();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 2, 2), e.primary);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 5, 2), t);
    }

    #[test]
    fn move_tool_picks_the_layer_under_the_pointer() {
        let mut e = editor();
        let bottom = LayerId(1);
        e.execute(Command::DrawPixels {
            layer: bottom,
            frame: 0,
            pixels: vec![(1, 1, e.primary)],
        });
        e.execute(Command::AddLayer {
            index: 1,
            layer: cs_core::Layer::new_image(LayerId(2), "top"),
        });
        e.execute(Command::DrawPixels {
            layer: LayerId(2),
            frame: 0,
            pixels: vec![(5, 5, e.primary)],
        });
        e.layer = bottom;
        e.tool = ToolKind::Move;
        // Clic sobre lo de la capa de arriba: cambia a ella y la mueve.
        e.pointer_press((5.5, 5.5), false, false, 0.1);
        assert_eq!(e.layer, LayerId(2));
        e.pointer_drag((6.5, 5.5));
        e.pointer_release();
        assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (1, 0));
        assert_eq!(e.sprite().cel(bottom, 0).unwrap().position, (0, 0));
        // Clic en vacío: se queda en la capa actual.
        e.pointer_press((7.5, 0.5), false, false, 0.1);
        e.pointer_release();
        assert_eq!(e.layer, LayerId(2));
        // Las capas bloqueadas u ocultas no se eligen.
        e.toggle_locked(LayerId(2));
        e.pointer_press((1.5, 1.5), false, false, 0.1);
        e.pointer_release();
        assert_eq!(e.layer, bottom);
        e.toggle_visible(bottom);
        e.layer = LayerId(2);
        assert_eq!(e.layer_at((1, 1)), None);
    }

    #[test]
    fn dragging_inside_the_selection_with_a_selection_tool_moves_its_content() {
        let mut e = editor();
        let red = e.primary;
        let t = e.sprite().color_mode.transparent();
        e.execute(Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(1, 1, red)],
        });
        e.set_tool(ToolKind::RectSelect);
        drag_local(&mut e, (0, 0), (2, 2), false, false);
        // Arrastrar desde dentro: mueve píxeles y selección, sin cambiar de herramienta.
        drag_local(&mut e, (1, 1), (4, 3), false, false);
        assert_eq!(e.tool, ToolKind::RectSelect);
        assert_eq!(e.active_tool(), ToolKind::RectSelect);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), t);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 4, 3), red);
        let bounds = e.selection.as_ref().and_then(|m| m.bounds());
        assert_eq!(bounds, Some(((3, 2), (5, 4))));
        e.undo();
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
        // Con Shift (Add) dentro de la selección se selecciona, no se mueve.
        drag_local(&mut e, (4, 3), (6, 6), true, false);
        assert_eq!(e.sprite().pixel(LayerId(1), 0, 1, 1), red);
        assert!(e.selection.as_ref().unwrap().contains(6, 6));
        // Un clic sin arrastrar dentro de la selección deselecciona, como siempre.
        click(&mut e, (4, 4), false, false);
        assert!(e.selection.is_none());
    }

    #[test]
    fn indexed_sprite_uses_palette_indices() {
        let e = Editor::with_sprite(Sprite::new(
            4,
            4,
            ColorMode::Indexed {
                transparent_index: 0,
            },
        ));
        assert_eq!(e.primary, PixelValue::Index(0));
        assert_eq!(e.secondary, PixelValue::Index(21));
    }

    #[test]
    fn tiles_mode_pencil_paints_the_brush_tile_with_flips_and_previews_it() {
        let mut e = tile_editor();
        assert!(e.tiles_mode());
        assert_eq!(e.brush_tile(), t(1), "the brush starts on tile 1");
        let flipped = Tile::new(2, true, false, false);
        e.set_brush_tile(flipped);
        e.pointer_press((5.5, 9.5), false, false, 0.1); // celda (1, 2)
        let preview = e.view().document.artboards[0].sprite.clone();
        let shown = TileProxy::build(&preview, LayerId(2), 0, None).unwrap();
        assert_eq!(shown.tile_at((1, 2)), flipped, "the preview shows the tile");
        assert!(e.sprite().cel(LayerId(2), 0).is_none(), "preview only");
        e.pointer_release();
        assert_eq!(tile_at(&e, (1, 2)), flipped);
        assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
        e.undo();
        assert!(e.sprite().cel(LayerId(2), 0).is_none());
    }

    #[test]
    fn tiles_mode_shapes_are_exact_in_cells() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::Line);
        drag_local(&mut e, (1, 1), (13, 1), false, false);
        for cx in 0..4 {
            assert_eq!(tile_at(&e, (cx, 0)), t(1));
        }
        assert_eq!(tile_at(&e, (0, 1)), Tile::EMPTY);
        e.undo();
        e.set_tool(ToolKind::Rectangle);
        e.options.filled = true;
        drag_local(&mut e, (1, 1), (9, 9), false, false);
        let proxy = TileProxy::build(e.sprite(), LayerId(2), 0, None).unwrap();
        let filled: Vec<Point> = (0..4)
            .flat_map(|y| (0..4).map(move |x| (x, y)))
            .filter(|&c| proxy.tile_at(c) == t(1))
            .collect();
        assert_eq!(
            filled,
            vec![
                (0, 0),
                (1, 0),
                (2, 0),
                (0, 1),
                (1, 1),
                (2, 1),
                (0, 2),
                (1, 2),
                (2, 2)
            ]
        );
    }

    #[test]
    fn tiles_mode_bucket_fills_equal_tiles_contiguous_or_not() {
        let mut e = tile_editor();
        let flipped = Tile::new(1, true, false, false);
        let cells = [
            (0, 0, t(1)),
            (1, 0, t(1)),
            (2, 0, t(2)),
            (3, 0, t(1)),
            (0, 1, flipped),
        ];
        let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &cells).unwrap();
        e.execute(cmd);
        let row = |e: &Editor| (0..4).map(|x| tile_at(e, (x, 0))).collect::<Vec<_>>();
        e.set_tool(ToolKind::Bucket);
        e.set_brush_tile(t(2));
        click(&mut e, (5, 1), false, false); // celda (1, 0)
        assert_eq!(row(&e), vec![t(2), t(2), t(2), t(1)]);
        assert_eq!(
            tile_at(&e, (0, 1)),
            flipped,
            "flips count: not the same tile"
        );
        e.undo();
        e.options.contiguous = false;
        click(&mut e, (5, 1), false, false);
        assert_eq!(row(&e), vec![t(2); 4]);
        assert_eq!(tile_at(&e, (0, 1)), flipped);
        assert_eq!(tile_at(&e, (1, 1)), Tile::EMPTY);
    }

    #[test]
    fn tiles_mode_eyedropper_picks_the_tile_and_its_flips() {
        let mut e = tile_editor();
        let flipped = Tile::new(2, false, true, false);
        let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &[(2, 0, flipped)]).unwrap();
        e.execute(cmd);
        e.set_tool(ToolKind::Eyedropper);
        click(&mut e, (9, 1), false, false);
        assert_eq!(e.brush_tile(), flipped);
        click(&mut e, (1, 13), false, false);
        assert_eq!(
            e.brush_tile(),
            Tile::EMPTY,
            "clicking an empty cell picks the empty tile"
        );
    }

    #[test]
    fn tiles_mode_eraser_leaves_the_empty_tile() {
        let mut e = tile_editor();
        click(&mut e, (1, 1), false, false);
        assert_eq!(tile_at(&e, (0, 0)), t(1));
        e.set_tool(ToolKind::Eraser);
        click(&mut e, (2, 2), false, false);
        assert_eq!(tile_at(&e, (0, 0)), Tile::EMPTY);
    }

    #[test]
    fn tiles_mode_marquee_selects_whole_cells_and_limits_painting() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::RectSelect);
        drag_local(&mut e, (1, 1), (5, 5), false, false);
        let sel = e.selection.clone().unwrap();
        assert_eq!(sel.bounds(), Some(((0, 0), (7, 7))));
        assert_eq!(sel.points().count(), 64);
        e.set_tool(ToolKind::Pencil);
        drag_local(&mut e, (1, 1), (13, 1), false, false);
        assert_eq!(tile_at(&e, (1, 0)), t(1));
        assert_eq!(tile_at(&e, (2, 0)), Tile::EMPTY, "outside the selection");
    }

    #[test]
    fn tiles_mode_move_steps_by_whole_tiles() {
        let mut e = tile_editor();
        click(&mut e, (1, 1), false, false); // tile 1 en la celda (0, 0)
        e.set_tool(ToolKind::Move);
        drag_local(&mut e, (1, 1), (2, 1), false, false); // 1 px: menos de medio tile
        assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (0, 0));
        drag_local(&mut e, (1, 1), (4, 2), false, false); // 3 px → 1 tile; 1 px → 0
        assert_eq!(e.sprite().cel(LayerId(2), 0).unwrap().position, (4, 0));
        assert_eq!(tile_at(&e, (1, 0)), t(1));
        assert_eq!(e.layer, LayerId(2), "Move keeps the tilemap layer");
    }

    #[test]
    fn tiles_mode_blocks_spray_and_text() {
        let mut e = tile_editor();
        e.set_tool(ToolKind::Spray);
        e.pointer_press((4.5, 4.5), false, false, 0.1);
        e.tool_tick(1000.0);
        e.pointer_release();
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Not available in Tiles mode")
        );
        e.set_tool(ToolKind::Text);
        click(&mut e, (1, 1), false, false);
        assert!(e.text.is_none());
        assert!(!e.history.can_undo(), "no commands");
        e.set_tool(ToolKind::Pencil);
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: painting with tile 1")
        );
        // Numeración del usuario: índice + base_index - 1.
        let mut ts = e.sprite().tilesets[0].clone();
        ts.base_index = 5;
        e.execute(Command::SetTileset {
            index: 0,
            tileset: Some(ts),
        });
        e.set_brush_tile(t(2));
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: painting with tile 6")
        );
    }

    #[test]
    fn tiles_mode_reports_the_grid_limit() {
        use std::sync::Arc;

        use cs_core::{Image, Tileset};
        let mut s = Sprite::new(5000, 1, ColorMode::Rgba);
        let mut ts = Tileset::new("t", (1, 1), ColorMode::Rgba);
        ts.tiles.push(Arc::new(Image::new(1, 1, ColorMode::Rgba)));
        s.tilesets.push(ts);
        let add = ops::new_tilemap_layer(&s, Some(LayerId(1)), 0);
        add.apply(&mut s).unwrap();
        let mut e = Editor::with_sprite(s);
        e.select_layer(LayerId(2));
        e.set_tool(ToolKind::Line);
        // 5000 celdas de ancho: más que MAX_TILEMAP_SIDE (4096).
        drag_local(&mut e, (0, 0), (4999, 0), false, false);
        assert!(e.sprite().cel(LayerId(2), 0).is_none());
        assert_eq!(
            e.tiles_status().as_deref(),
            Some("Tiles mode: the tilemap would exceed its size limit")
        );
    }

    #[test]
    fn tiles_mode_paints_the_partial_edge_cell_of_an_unaligned_cel_with_a_selection() {
        let mut e = tile_editor();
        // Un tile en la celda (1, 1) y el cel corrido 2 px: offset (2, 2), la celda -1 se ve en
        // los píxeles [0, 2) del borde.
        let cmd = ops::set_tiles(e.sprite(), LayerId(2), 0, &[(1, 1, t(2))]).unwrap();
        e.execute(cmd);
        let mut cel = *e.sprite().cel(LayerId(2), 0).unwrap();
        cel.position = (cel.position.0 + 2, cel.position.1 + 2);
        e.execute(Command::SetCel {
            layer: LayerId(2),
            frame: 0,
            cel: Some(cel),
        });
        e.select_all();
        click(&mut e, (1, 1), false, false);
        assert_eq!(tile_at(&e, (-1, -1)), t(1), "the edge cell is painted");
        assert_eq!(tile_at(&e, (1, 1)), t(2));
        // El puntero sobre un tile corrido cae en su celda.
        click(&mut e, (6, 6), false, false);
        assert_eq!(tile_at(&e, (1, 1)), t(1));
    }
}
