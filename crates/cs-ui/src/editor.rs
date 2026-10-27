use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use cs_commands::{Command, DocCommand, History, sprite_ops};
use cs_core::{
    AnimDirection, Artboard, ArtboardId, ColorMode, Document, LayerId, Mask, PixelValue, Playback,
    Rgba, Sprite, Tile,
};
use cs_render::OnionSkin;
use cs_tools::text::TextStyle;
use cs_tools::{
    ArtboardTool, Blur, BrushShape, Bucket, Contour, Dither, Eraser, Eyedropper, GradientShape,
    GradientTool, Jumble, Lasso, MagicWand, Marquee, MarqueeShape, MoveTool, Pencil, Point,
    Polygon, PolygonLasso, ShapeKind, ShapeTool, SliceTool, Spray, Tool, ToolContext, ToolOutput,
};

use crate::tile_ops::{self, TileProxy};
use crate::transform::{Floating, TransformDrag};
use crate::transform_box::TransformHit;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ToolKind {
    RectSelect,
    EllipseSelect,
    Lasso,
    PolygonLasso,
    MagicWand,
    Hand,
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
    Polygon,
    Jumble,
    Text,
    Contour,
    Blur,
    Slice,
    Artboard,
}

impl ToolKind {
    /// Grupos de la barra (como en Aseprite); el primero de cada grupo es el predeterminado.
    pub const GROUPS: [&'static [ToolKind]; 13] = [
        &[
            ToolKind::RectSelect,
            ToolKind::EllipseSelect,
            ToolKind::Lasso,
            ToolKind::PolygonLasso,
            ToolKind::MagicWand,
        ],
        &[ToolKind::Hand],
        &[ToolKind::Pencil, ToolKind::Spray],
        &[ToolKind::Eraser],
        &[ToolKind::Eyedropper],
        &[ToolKind::Bucket, ToolKind::Gradient, ToolKind::Jumble],
        &[ToolKind::Line],
        &[ToolKind::Rectangle, ToolKind::Ellipse, ToolKind::Polygon],
        &[ToolKind::Text],
        &[ToolKind::Contour],
        &[ToolKind::Blur],
        &[ToolKind::Move, ToolKind::Slice],
        &[ToolKind::Artboard],
    ];

    /// Índice en `GROUPS` del grupo que tiene a `self`.
    pub fn group(self) -> usize {
        ToolKind::GROUPS
            .iter()
            .position(|g| g.contains(&self))
            .expect("every tool is in a group")
    }

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
            ToolKind::Polygon => "Polygon",
            ToolKind::Jumble => "Jumble",
            ToolKind::Text => "Text",
            ToolKind::Slice => "Slice",
            ToolKind::Artboard => "Artboard",
            ToolKind::Hand => "Hand",
            ToolKind::Contour => "Contour",
            ToolKind::Blur => "Blur",
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
            ToolKind::Polygon => "Shift+D",
            ToolKind::Jumble => "Shift+R",
            ToolKind::Text => "T",
            ToolKind::Slice => "Shift+C",
            ToolKind::Artboard => "Shift+O",
            ToolKind::Hand => "H",
            ToolKind::Contour => "D",
            ToolKind::Blur => "R",
        }
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
                | ToolKind::Polygon
                | ToolKind::Jumble
                | ToolKind::Text
                | ToolKind::Contour
                | ToolKind::Blur
        )
    }

    /// Herramientas que pintan con la simetría del artboard. Text no (el texto se leería al
    /// revés) y Blur tampoco (como en Aseprite).
    pub fn mirrors(self) -> bool {
        self.paints() && !matches!(self, ToolKind::Text | ToolKind::Blur)
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
                | ToolKind::Polygon
                | ToolKind::Jumble
                | ToolKind::Blur
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
                | ToolKind::Contour
                | ToolKind::Polygon
                | ToolKind::Blur
        )
    }

    /// Herramientas que no actúan mientras se edita una máscara de capa: promedian o mezclan los
    /// colores de la capa y los escribirían como niveles de la máscara.
    pub fn blocked_on_mask(self) -> bool {
        matches!(self, ToolKind::Jumble | ToolKind::Blur)
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// La herramienta que muestra cada grupo de la barra: la última elegida en él.
///
/// Se guarda con las opciones como una lista de nombres de variante (`"Lasso"`), uno por grupo en
/// el orden de `GROUPS`. Si falta un nombre, no se conoce o es de otro grupo, ese grupo vuelve a su
/// primera herramienta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroupChoice([ToolKind; ToolKind::GROUPS.len()]);

impl Default for GroupChoice {
    fn default() -> Self {
        Self(ToolKind::GROUPS.map(|g| g[0]))
    }
}

impl GroupChoice {
    /// La herramienta recordada del grupo `group`.
    pub fn get(&self, group: usize) -> ToolKind {
        self.0[group]
    }

    /// Recuerda `kind` como la elegida de su grupo.
    pub fn remember(&mut self, kind: ToolKind) {
        self.0[kind.group()] = kind;
    }
}

impl serde::Serialize for GroupChoice {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|k| format!("{k:?}")))
    }
}

impl<'de> serde::Deserialize<'de> for GroupChoice {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let names = <Vec<String> as serde::Deserialize>::deserialize(d)?;
        let mut choice = GroupChoice::default();
        for (slot, (group, name)) in choice.0.iter_mut().zip(ToolKind::GROUPS.iter().zip(&names)) {
            if let Some(&kind) = group.iter().find(|k| format!("{k:?}") == *name) {
                *slot = kind;
            }
        }
        Ok(choice)
    }
}

/// Tamaño máximo del pincel (slider de Options y Ctrl+rueda).
pub const MAX_BRUSH_SIZE: u32 = 32;

/// Opciones compartidas por las herramientas. Se guardan al salir; lo que falte en un ajuste
/// guardado (por ejemplo `brush_shape`, de antes de que existiera) toma el valor por defecto.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ToolOptions {
    /// Un ajuste guardado fuera de `1..=MAX_BRUSH_SIZE` se lleva al rango al leerlo.
    #[serde(deserialize_with = "clamped_brush_size")]
    pub brush_size: u32,
    pub brush_shape: BrushShape,
    /// El pincel personalizado pinta con los colores del dibujo del que salió (si no, con el
    /// color primario), como el de Aseprite.
    pub brush_colors: bool,
    pub pixel_perfect: bool,
    pub filled: bool,
    /// Radio de las esquinas del rectángulo (0 = rectas).
    pub corner_radius: u32,
    pub contiguous: bool,
    /// Shift presionado (lo actualiza la UI cada frame). No se guarda.
    #[serde(skip)]
    pub constrain: bool,
    /// No se guarda: cada sesión empieza en Replace.
    #[serde(skip)]
    pub selection_mode: SelectionMode,
    /// Varita: diferencia máxima por canal.
    pub tolerance: u8,
    /// Varita: tomar los colores de todas las capas visibles.
    pub sample_all: bool,
    pub spray_width: u32,
    pub spray_speed: u32,
    pub gradient_shape: GradientShape,
    pub dither: Dither,
    /// Cómo se rota la selección (herramientas de selección y Move).
    pub rotation: cs_raster::RotationAlgorithm,
    /// La variante que muestra cada grupo de la barra.
    pub group_choice: GroupChoice,
    /// Selección espejo: con simetría, lo seleccionado se refleja en los ejes, y al moverlo la
    /// parte reflejada se mueve como en un espejo.
    pub mirror_selection: bool,
}

fn clamped_brush_size<'de, D: serde::Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let size = <u32 as serde::Deserialize>::deserialize(d)?;
    Ok(size.clamp(1, MAX_BRUSH_SIZE))
}

impl Default for ToolOptions {
    fn default() -> Self {
        Self {
            brush_size: 1,
            brush_shape: BrushShape::Round,
            brush_colors: true,
            pixel_perfect: true,
            filled: false,
            corner_radius: 0,
            contiguous: true,
            constrain: false,
            selection_mode: SelectionMode::Replace,
            tolerance: 0,
            sample_all: false,
            spray_width: 16,
            spray_speed: 32,
            gradient_shape: GradientShape::Linear,
            dither: Dither::None,
            rotation: cs_raster::RotationAlgorithm::RotSprite,
            group_choice: GroupChoice::default(),
            mirror_selection: false,
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
    polygon: Polygon,
    jumble: Jumble,
    contour: Contour,
    blur: Blur,
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
            polygon: Polygon::new(),
            jumble: Jumble::new(),
            contour: Contour::new(),
            blur: Blur::new(),
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
            ToolKind::Polygon => &self.polygon,
            ToolKind::Jumble => &self.jumble,
            ToolKind::Slice => &self.slice,
            ToolKind::Move => &self.mover,
            ToolKind::Eyedropper => &self.eyedropper,
            ToolKind::Contour => &self.contour,
            ToolKind::Blur => &self.blur,
            // Text y Artboard no son herramientas de sprite; Hand la maneja el lienzo.
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand => return None,
        })
    }

    /// Deja la herramienta `kind` como recién creada: descarta un trazo a medias aunque la
    /// herramienta no sepa cancelarse (`Tool::cancel` no hace nada en las de pincel).
    fn reset(&mut self, kind: ToolKind) {
        match kind {
            ToolKind::RectSelect => self.rect_select = Marquee::new(MarqueeShape::Rectangle),
            ToolKind::EllipseSelect => self.ellipse_select = Marquee::new(MarqueeShape::Ellipse),
            ToolKind::Lasso => self.lasso = Lasso::new(),
            ToolKind::PolygonLasso => self.polygon_lasso = PolygonLasso::new(),
            ToolKind::MagicWand => self.magic_wand = MagicWand::new(),
            ToolKind::Pencil => self.pencil = Pencil::new(),
            ToolKind::Spray => self.spray = Spray::new(),
            ToolKind::Eraser => self.eraser = Eraser::new(),
            ToolKind::Bucket => self.bucket = Bucket::new(),
            ToolKind::Gradient => self.gradient = GradientTool::new(),
            ToolKind::Line => self.line = ShapeTool::new(ShapeKind::Line),
            ToolKind::Rectangle => self.rectangle = ShapeTool::new(ShapeKind::Rectangle),
            ToolKind::Ellipse => self.ellipse = ShapeTool::new(ShapeKind::Ellipse),
            ToolKind::Polygon => self.polygon = Polygon::new(),
            ToolKind::Jumble => self.jumble = Jumble::new(),
            ToolKind::Slice => self.slice = SliceTool::new(),
            ToolKind::Move => self.mover = MoveTool::new(),
            ToolKind::Eyedropper => self.eyedropper = Eyedropper::new(),
            ToolKind::Contour => self.contour = Contour::new(),
            ToolKind::Blur => self.blur = Blur::new(),
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand => {}
        }
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
            ToolKind::Polygon => &mut self.polygon,
            ToolKind::Jumble => &mut self.jumble,
            ToolKind::Slice => &mut self.slice,
            ToolKind::Move => &mut self.mover,
            ToolKind::Eyedropper => &mut self.eyedropper,
            ToolKind::Contour => &mut self.contour,
            ToolKind::Blur => &mut self.blur,
            // Text y Artboard no son herramientas de sprite; Hand la maneja el lienzo.
            ToolKind::Text | ToolKind::Artboard | ToolKind::Hand => return None,
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
        } if symmetry.axes.any() => Command::DrawPixels {
            layer,
            frame,
            pixels: cs_raster::symmetrize(&pixels, symmetry.axes, center2),
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
    /// Reproducción en curso.
    pub playing: bool,
    /// Estado de la reproducción: posición, rango (inclusivo) y dirección.
    pub(crate) playback: Option<(Playback, (usize, usize), AnimDirection)>,
    /// Milisegundos acumulados en el frame actual durante la reproducción.
    pub(crate) play_clock: f64,
    /// Rango de frames seleccionado en el timeline (para tags, duraciones…).
    pub frame_range: Option<(usize, usize)>,
    /// Las capas del rango de cels (de la capa donde empezó a la otra punta, en el orden del
    /// timeline) y los frames para los que se eligió: vale mientras `frame_range` sea esos.
    pub layer_range: Option<(LayerId, LayerId, (usize, usize))>,
    /// Los frames se eligieron en el encabezado del timeline: copiar y pegar es de frames
    /// enteros (si no, de cels).
    pub frames_picked: bool,
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
    /// Contenido levantado que se está moviendo, escalando o rotando (ver `transform.rs`).
    pub(crate) floating: Option<Floating>,
    /// Arrastre en curso para mover, escalar o rotar lo seleccionado.
    pub(crate) transform_drag: Option<TransformDrag>,
    /// Con la paleta bloqueada (lo normal), sus colores no se editan uno por uno: el editor de
    /// color cambia el color primario y nada más. Ver `palette_entry_of`.
    pub palette_locked: bool,
    /// La entrada de la paleta elegida con un clic en su cuadrito (Pixel UI) para cada color:
    /// `[primario, secundario]`. Con la paleta desbloqueada, el editor de color edita la del
    /// color que edita, mientras el color siga siendo el suyo. Ver `pick_color`.
    pub palette_entry: [Option<u8>; 2],
    /// Los cuadritos de la paleta seleccionados (Pixel UI): de dónde empezó (con un clic) a dónde
    /// llega (con Shift+clic o arrastrando). Ver `palette_range`.
    pub palette_selection: Option<(usize, usize)>,
    /// El color primario de antes de elegir el Mask con su botón, para volver al apagarlo.
    pub(crate) before_mask: Option<PixelValue>,
    /// Con Pixel UI, el editor de color edita el secundario: se elige con un clic en la mitad
    /// de abajo de la caja de los colores, o al elegir un color con clic derecho (`pick_color`).
    /// Sin Pixel UI vuelve a `false` (`ChimpApp::layout`). Ver `edited_color`.
    pub editing_secondary: bool,
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
            playing: false,
            playback: None,
            play_clock: 0.0,
            frame_range: None,
            layer_range: None,
            frames_picked: false,
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
            floating: None,
            transform_drag: None,
            palette_locked: true,
            palette_entry: [None; 2],
            palette_selection: None,
            before_mask: None,
            editing_secondary: false,
        };
        editor.ensure_valid();
        editor.primary = editor.palette_value(0);
        // El 21 es el blanco de DB32; con paletas más cortas, el último color.
        let last = editor.sprite().palette.len().saturating_sub(1).min(21) as u8;
        editor.secondary = editor.palette_value(last);
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
        self.commit_floating();
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

    /// Como `execute`, con `label` como nombre del paso en el historial (el de la herramienta o
    /// el del menú: "Pencil", "Fill"… en vez del del comando).
    pub fn execute_named(&mut self, cmd: Command, label: &'static str) -> bool {
        let applied = self.execute(cmd);
        if applied {
            self.history.rename_last(label);
        }
        applied
    }

    /// Corre `f` (que puede ejecutar varios comandos, o ninguno) y, si dejó pasos nuevos, le pone
    /// `label` al último. Lo flotante se confirma antes si hace falta (si no, su paso podría
    /// quedarse con el nombre).
    pub(crate) fn named<R>(&mut self, label: &'static str, f: impl FnOnce(&mut Self) -> R) -> R {
        let before = self.history.done_count();
        let result = f(self);
        if self.history.done_count() > before {
            self.history.rename_last(label);
        }
        result
    }

    /// Ejecuta un comando sobre el documento. Antes confirma lo flotante: guarda índices (capa,
    /// frame) y una vista previa que otro cambio dejaría desactualizados.
    pub fn execute_doc(&mut self, cmd: DocCommand) -> bool {
        self.commit_floating();
        self.apply_doc(cmd)
    }

    /// Ejecuta sin confirmar lo flotante (lo usa la confirmación misma).
    pub(crate) fn apply_doc(&mut self, cmd: DocCommand) -> bool {
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
        // Como `execute_doc`: lo flotante se confirma antes (y corta el grupo).
        self.commit_floating();
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

    /// Como `end_group`, solo si el gesto en curso es el de `group`.
    pub fn end_group_of(&mut self, group: u64) {
        if self.merge.is_some_and(|(g, _)| g == group) {
            self.merge = None;
        }
    }

    pub fn undo(&mut self) {
        // Mientras flota, deshacer cancela la transformación (no hay paso en el historial).
        if self.cancel_floating() {
            return;
        }
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
        // Rehacer con algo flotando lo perdería: se ignora hasta confirmar o cancelar.
        if self.floating.is_some() {
            return;
        }
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

    /// Reemplaza el sprite del artboard activo (un paso de undo); `None` no hace nada.
    fn replace_sprite(&mut self, sprite: Option<Sprite>) -> bool {
        let Some(sprite) = sprite else {
            return false;
        };
        self.execute_doc(DocCommand::SetSprite {
            id: self.active,
            sprite: Box::new(sprite),
        })
    }

    /// `Sprite → Rotate Canvas`: gira todo el sprite (no con instancias ni tilemaps).
    pub fn rotate_canvas(&mut self, rotation: sprite_ops::Rotation) -> bool {
        self.commit_floating();
        let rotated = sprite_ops::rotate(self.sprite(), rotation);
        self.replace_sprite(rotated)
    }

    /// `Sprite → Sprite Size`: escala todo el sprite (no con instancias ni tilemaps).
    pub fn scale_sprite(&mut self, width: u32, height: u32) -> bool {
        self.commit_floating();
        let scaled = sprite_ops::scale(self.sprite(), width, height);
        self.replace_sprite(scaled)
    }

    /// `Sprite → Color Mode`: pasa a RGBA o a indexado con la paleta del sprite.
    pub fn set_color_mode(&mut self, indexed: bool) -> bool {
        self.commit_floating();
        let converted = if indexed {
            sprite_ops::to_indexed(self.sprite())
        } else {
            sprite_ops::to_rgba(self.sprite())
        };
        self.replace_sprite(converted)
    }

    /// Cambia el índice transparente de un sprite indexado (los píxeles no cambian).
    pub fn set_transparent_index(&mut self, index: u8) -> bool {
        let ColorMode::Indexed { transparent_index } = self.sprite().color_mode else {
            return false;
        };
        if transparent_index == index {
            return false;
        }
        let mut sprite = self.sprite().clone();
        sprite.color_mode = ColorMode::Indexed {
            transparent_index: index,
        };
        self.replace_sprite(Some(sprite))
    }

    /// Recorta el lienzo al rectángulo `(x, y, w, h)` (local) y descarta lo de fuera; el
    /// contenido queda en su lugar del mundo.
    fn crop_canvas(&mut self, (x, y, w, h): (i32, i32, u32, u32)) -> bool {
        let sprite = self.sprite();
        if (x, y, w, h) == (0, 0, sprite.width, sprite.height) || w == 0 || h == 0 {
            return false;
        }
        let (ax, ay) = self.artboard().position;
        let id = self.active;
        self.execute_doc(DocCommand::Batch(vec![
            DocCommand::OnArtboard {
                id,
                cmd: Command::ResizeCanvas {
                    width: w,
                    height: h,
                    offset: (-x, -y),
                },
            },
            DocCommand::OnArtboard {
                id,
                cmd: Command::TrimToCanvas,
            },
            DocCommand::SetArtboardProps {
                id,
                name: None,
                position: Some((ax + x, ay + y)),
            },
        ]))
    }

    /// `Sprite → Crop`: el lienzo pasa a ser el rectángulo de la selección.
    pub fn crop_to_selection(&mut self) -> bool {
        self.commit_floating();
        let Some(((x0, y0), (x1, y1))) = self.selection.as_ref().and_then(|m| m.bounds()) else {
            return false;
        };
        self.crop_canvas((x0, y0, (x1 - x0 + 1) as u32, (y1 - y0 + 1) as u32))
    }

    /// `Sprite → Trim`: el lienzo se ajusta a lo que tiene píxeles visibles.
    pub fn trim_canvas(&mut self) -> bool {
        self.commit_floating();
        match sprite_ops::content_bounds(self.sprite()) {
            Some(bounds) => self.crop_canvas(bounds),
            None => false,
        }
    }

    /// `Sprite → Duplicate`: una copia del artboard activo con `name`, a la derecha de todo, y
    /// la activa.
    pub fn duplicate_artboard(&mut self, name: String) -> bool {
        self.commit_floating();
        let ((_, y0), (x1, _)) = self.document.bounds().unwrap_or(((0, 0), (0, 0)));
        let id = self.document.next_artboard_id();
        let artboard = Artboard {
            id,
            name,
            position: (x1 + 16, y0),
            ..self.artboard().clone()
        };
        let added = self.execute_doc(DocCommand::AddArtboard {
            index: self.document.artboards.len(),
            artboard: Box::new(artboard),
        });
        if added {
            self.activate(id);
        }
        added
    }

    /// Reemplaza las notas del documento (un paso de undo).
    pub fn set_notes(&mut self, notes: Vec<cs_core::Note>) -> bool {
        if notes == self.document.notes {
            return false;
        }
        self.execute_doc(DocCommand::SetNotes(notes))
    }

    /// Una nota nueva con su esquina en `position` (mundo); devuelve su id.
    pub fn add_note(&mut self, position: (i32, i32)) -> Option<u32> {
        let id = self.document.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
        let mut notes = self.document.notes.clone();
        notes.push(cs_core::Note {
            id,
            position,
            size: crate::notes::NEW_SIZE,
            text: String::new(),
            color: Default::default(),
        });
        self.set_notes(notes).then_some(id)
    }

    pub fn set_note_text(&mut self, id: u32, text: String) -> bool {
        let mut notes = self.document.notes.clone();
        match notes.iter_mut().find(|n| n.id == id) {
            Some(n) if n.text != text => n.text = text,
            _ => return false,
        }
        self.set_notes(notes)
    }

    pub fn set_note_color(&mut self, id: u32, color: cs_core::NoteColor) -> bool {
        let mut notes = self.document.notes.clone();
        match notes.iter_mut().find(|n| n.id == id) {
            Some(n) if n.color != color => n.color = color,
            _ => return false,
        }
        self.set_notes(notes)
    }

    pub fn remove_note(&mut self, id: u32) -> bool {
        let mut notes = self.document.notes.clone();
        notes.retain(|n| n.id != id);
        self.set_notes(notes)
    }

    /// Lleva la nota `id` encima de las demás.
    pub fn note_to_front(&mut self, id: u32) -> bool {
        let mut notes = self.document.notes.clone();
        let Some(i) = notes.iter().position(|n| n.id == id) else {
            return false;
        };
        let note = notes.remove(i);
        notes.push(note);
        self.set_notes(notes)
    }

    /// Color "Mask" (transparente) del sprite: en RGBA alfa 0, en indexado el índice transparente.
    /// Pintar con él borra.
    pub fn mask_value(&self) -> PixelValue {
        self.sprite().color_mode.transparent()
    }

    pub fn is_mask(&self, value: PixelValue) -> bool {
        value == self.mask_value()
    }

    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
        // Cada color se lleva su cuadrito de la paleta.
        self.palette_entry.swap(0, 1);
    }

    pub fn set_selection(&mut self, selection: Option<Mask>) {
        // Cambiar la selección (deseleccionar, seleccionar todo, un gesto…) confirma lo flotante.
        self.commit_floating();
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

    /// Cambia de herramienta descartando gestos de varios clics a medias. La herramienta queda
    /// como la elegida de su grupo en la barra.
    pub fn set_tool(&mut self, kind: ToolKind) {
        self.options.group_choice.remember(kind);
        if kind != self.tool {
            self.commit_floating();
            self.cancel_gesture();
            self.tool = kind;
        }
    }

    /// La herramienta que muestra el grupo `group` de la barra: la activa si es de ese grupo; si
    /// no, la última elegida en él.
    pub fn group_tool(&self, group: usize) -> ToolKind {
        if ToolKind::GROUPS[group].contains(&self.tool) {
            self.tool
        } else {
            self.options.group_choice.get(group)
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
        self.named("Clear", Self::clear_content);
    }

    fn clear_content(&mut self) {
        // Mientras flota, Delete descarta lo pegado o deja solo el hueco de lo levantado.
        if self.delete_floating() {
            return;
        }
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

    /// Edit → Stroke (S), como en Aseprite: pinta con el color primario el borde de la selección
    /// (sus píxeles que tocan algo de afuera, de lado), en la capa actual (un paso de undo).
    pub fn stroke_selection(&mut self) {
        self.commit_floating();
        if !self.clip_layer_ok(true) {
            return;
        }
        let Some(mask) = &self.selection else {
            return;
        };
        let color = self.primary;
        let pixels: Vec<_> = mask
            .points()
            .filter(|&(x, y)| {
                [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| !mask.contains(x + dx, y + dy))
            })
            .map(|(x, y)| (x, y, color))
            .collect();
        if pixels.is_empty() {
            return;
        }
        self.execute_named(
            Command::DrawPixels {
                layer: self.layer,
                frame: self.frame,
                pixels,
            },
            "Stroke",
        );
    }

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

    /// Los puntos del pincel personalizado con sus colores en este sprite, si pinta con ellos.
    fn brush_image(&self) -> Option<Vec<((i32, i32), PixelValue)>> {
        if self.options.brush_shape != BrushShape::Custom || !self.options.brush_colors {
            return None;
        }
        let brush = cs_raster::custom_brush();
        (!brush.is_empty()).then(|| {
            brush
                .into_iter()
                .map(|(p, c)| (p, self.value_of(c)))
                .collect()
        })
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

    /// Presionar en `world` arrastraría lo seleccionado (levantándolo): con Move, desde cualquier
    /// punto que no sea otro artboard; con una herramienta de selección, dentro de lo
    /// seleccionado y sin modificadores que cambien el modo de selección.
    pub fn moves_selection_at(&self, world: (f32, f32), shift: bool, alt: bool) -> bool {
        let artboard = self.document.artboard_at(world);
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
        self.apply_output(kind, out);
    }

    /// Simetría y recorte a la selección para las herramientas que pintan.
    fn shape_paint(&self, kind: ToolKind, cmd: Command) -> Command {
        if !kind.paints() {
            return cmd;
        }
        let artboard = self.artboard();
        let cmd = if kind.mirrors() {
            mirror(cmd, artboard.symmetry, artboard.symmetry_center2())
        } else {
            cmd
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
        if let Some(view) = self.floating_view() {
            return view;
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
mod tests;
