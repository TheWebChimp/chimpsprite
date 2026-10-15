//! Parámetros de cada herramienta. Los comentarios `///` se convierten en las descripciones del
//! esquema JSON que ve el agente, por eso están en inglés.

use schemars::JsonSchema;
use serde::Deserialize;

/// Artboard or layer reference: numeric id or exact name.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Ref {
    Id(u32),
    Name(String),
}

/// A color: "#rgb", "#rrggbb", "#rrggbbaa" or "transparent". Indexed sprites also accept a
/// palette index (number); hex colors must then exist in the palette.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ColorArg {
    Index(u8),
    Text(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ColorModeArg {
    #[default]
    Rgba,
    /// Palette indices; index 0 is transparent.
    Indexed,
}

/// Rectangle in artboard pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Inclusive frame range (0-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct FrameRange {
    pub from: usize,
    pub to: usize,
}

// --- Documentos ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NewDocument {
    /// Canvas width in pixels (1-4096).
    pub width: u32,
    /// Canvas height in pixels (1-4096).
    pub height: u32,
    /// "rgba" (default) or "indexed".
    #[serde(default)]
    pub color_mode: ColorModeArg,
    /// Name of the first artboard (default "Sprite").
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Open {
    /// Path to a .aseprite/.ase, .chimp or .png file.
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Save {
    pub doc: String,
    /// Target path; the format comes from the extension (.chimp keeps everything; .aseprite holds
    /// a single artboard). Defaults to the path the document was opened from or last saved to.
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DocOnly {
    /// Document id returned by new_document/open (e.g. "d1").
    pub doc: String,
}

// --- Estructura ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddArtboard {
    pub doc: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// World position [x, y] of the top-left corner. Default: right of the existing artboards.
    pub position: Option<[i32; 2]>,
    /// Default: same color mode as the first artboard.
    pub color_mode: Option<ColorModeArg>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetArtboard {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub name: Option<String>,
    pub position: Option<[i32; 2]>,
    /// New canvas width (content keeps its place, anchored top-left).
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ArtboardOnly {
    pub doc: String,
    pub artboard: Option<Ref>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddLayer {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub name: Option<String>,
    /// Create a group instead of an image layer.
    #[serde(default)]
    pub group: bool,
    /// Place it right above this layer (same parent). Default: on top.
    pub above: Option<Ref>,
    /// Place it inside this group, on top of its children.
    pub into: Option<Ref>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlendArg {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    Addition,
    Subtract,
    Difference,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetLayer {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Ref,
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub locked: Option<bool>,
    /// 0-255.
    pub opacity: Option<u8>,
    pub blend: Option<BlendArg>,
    /// Clipping mask: show this layer only where the layer below it (same group) has pixels.
    pub clip: Option<bool>,
    /// Turn the layer mask on or off without deleting it.
    pub mask_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddMask {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Ref,
    /// Start hiding everything (paint white to reveal). Default: everything visible.
    #[serde(default)]
    pub hide_all: bool,
    /// Only this rectangle starts visible; everything else hidden.
    pub reveal: Option<Rect>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerOnly {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Ref,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveLayer {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Ref,
    /// Exactly one of above/below/into.
    pub above: Option<Ref>,
    pub below: Option<Ref>,
    /// A group layer.
    pub into: Option<Ref>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddFrame {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Insert position (0-based). Default: at the end. Ignored with `duplicate`.
    pub at: Option<usize>,
    /// Copy this frame's cels; the copy is inserted right after it.
    pub duplicate: Option<usize>,
    /// With `duplicate`: share images with the original (linked cels) instead of copying.
    #[serde(default)]
    pub linked: bool,
    /// Default: 100 ms, or the duplicated frame's duration.
    pub duration_ms: Option<u16>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameOnly {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub frame: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveFrame {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetFrameDuration {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Default: all frames.
    pub frames: Option<FrameRange>,
    pub duration_ms: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DirectionArg {
    #[default]
    Forward,
    Reverse,
    PingPong,
    PingPongReverse,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetTag {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Creates the tag, or replaces the one with this name.
    pub name: String,
    pub from: usize,
    pub to: usize,
    #[serde(default)]
    pub direction: DirectionArg,
    /// Hex color shown in the timeline.
    pub color: Option<String>,
    /// Times to repeat (0 = unspecified/loop).
    #[serde(default)]
    pub repeat: u16,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveTag {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CelRange {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CelOnly {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MoveCel {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Move the cels of these frames instead of `frame`.
    pub frames: Option<FrameRange>,
    /// Offset in pixels; linked cels stay linked.
    pub dx: i32,
    pub dy: i32,
}

// --- Dibujo ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PixelArg {
    pub x: i32,
    pub y: i32,
    pub color: ColorArg,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawPixels {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Default: the topmost image layer.
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub pixels: Vec<PixelArg>,
    /// Only pixels inside this rectangle are drawn.
    pub clip: Option<Rect>,
    /// Paint on the layer mask instead of the layer: white (or light colors) reveals, black or
    /// "transparent" hides. The layer needs a mask (add_mask).
    #[serde(default)]
    pub on_mask: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawLine {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Polyline vertices [[x, y], ...]; a single point draws a dot.
    pub points: Vec<[i32; 2]>,
    pub color: ColorArg,
    /// Square brush size (default 1).
    pub brush_size: Option<u32>,
    pub clip: Option<Rect>,
    /// Paint on the layer mask instead of the layer: white (or light colors) reveals, black or
    /// "transparent" hides. The layer needs a mask (add_mask).
    #[serde(default)]
    pub on_mask: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawShape {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Corners (inclusive): the shape fits the box from (x0, y0) to (x1, y1).
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub color: ColorArg,
    #[serde(default)]
    pub filled: bool,
    /// Outline brush size (default 1).
    pub brush_size: Option<u32>,
    pub clip: Option<Rect>,
    /// Paint on the layer mask instead of the layer: white (or light colors) reveals, black or
    /// "transparent" hides. The layer needs a mask (add_mask).
    #[serde(default)]
    pub on_mask: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Fill {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub x: i32,
    pub y: i32,
    pub color: ColorArg,
    /// true (default): flood the connected area; false: replace that color everywhere in the cel.
    pub contiguous: Option<bool>,
    pub clip: Option<Rect>,
    /// Paint on the layer mask instead of the layer: white (or light colors) reveals, black or
    /// "transparent" hides. The layer needs a mask (add_mask).
    #[serde(default)]
    pub on_mask: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum GradientShapeArg {
    #[default]
    Linear,
    /// Circles around (x0, y0); (x1, y1) marks the end radius.
    Radial,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DitherArg {
    /// Smooth interpolation (nearest palette color on indexed sprites).
    #[default]
    None,
    /// Ordered dithering between the two colors (classic pixel-art look).
    Bayer2,
    Bayer4,
    Bayer8,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Gradient {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Start point (color `from`).
    pub x0: i32,
    pub y0: i32,
    /// End point (color `to`).
    pub x1: i32,
    pub y1: i32,
    pub from: ColorArg,
    pub to: ColorArg,
    #[serde(default)]
    pub shape: GradientShapeArg,
    #[serde(default)]
    pub dither: DitherArg,
    /// Area to fill. Default: the whole canvas.
    pub clip: Option<Rect>,
    /// Paint on the layer mask instead of the layer: white (or light colors) reveals, black or
    /// "transparent" hides. The layer needs a mask (add_mask).
    #[serde(default)]
    pub on_mask: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FontArg {
    /// Built-in 5×7 pixel font; size is an integer scale (1-8). Lines are 9 px tall per scale.
    #[default]
    Pixel,
    /// Monospace vector font; size in pixels.
    Hack,
    /// Sans-serif vector font; size in pixels.
    Ubuntu,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DrawText {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Top-left corner of the text.
    pub x: i32,
    pub y: i32,
    /// Text; "\n" starts a new line.
    pub text: String,
    pub color: ColorArg,
    #[serde(default)]
    pub font: FontArg,
    /// Pixel font: scale 1-8 (default 1). Vector fonts: pixels (default 16).
    pub size: Option<u32>,
    /// Smooth edges blended over the layer (vector fonts on RGBA sprites only).
    #[serde(default)]
    pub antialias: bool,
    pub clip: Option<Rect>,
}

// --- Color ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetPalette {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Hex colors, up to 256.
    pub colors: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The cel at `layer`/`frame`.
    #[default]
    Cel,
    /// Every frame of `layer`.
    Layer,
    /// Every layer at `frame`.
    Frame,
    /// Everything in the artboard.
    Sprite,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ColorPair {
    pub from: ColorArg,
    pub to: ColorArg,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Recolor {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub map: Vec<ColorPair>,
    #[serde(default)]
    pub scope: Scope,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Quantize {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Reduce to at most this many colors (median cut). Use this or `palette`.
    pub max_colors: Option<u32>,
    /// Map every pixel to the nearest of these hex colors.
    pub palette: Option<Vec<String>>,
    /// Convert the sprite to indexed mode (index 0 = transparent).
    #[serde(default)]
    pub to_indexed: bool,
}

// --- Transformaciones ---

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FlipAxis {
    /// Mirror left↔right.
    Horizontal,
    /// Mirror top↔bottom.
    Vertical,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Flip {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Apply to these frames instead of `frame`.
    pub frames: Option<FrameRange>,
    /// Region to transform. Default: the whole canvas.
    pub rect: Option<Rect>,
    pub axis: FlipAxis,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Rotate {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub frames: Option<FrameRange>,
    /// 90 and 270 need a square region.
    pub rect: Option<Rect>,
    /// Clockwise: 90, 180 or 270.
    pub degrees: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Shift {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub frames: Option<FrameRange>,
    pub rect: Option<Rect>,
    pub dx: i32,
    pub dy: i32,
    /// Pixels leaving one side come back on the other.
    #[serde(default)]
    pub wrap: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CelAt {
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PasteAt {
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CopyRegion {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub rect: Rect,
    pub from: CelAt,
    /// Top-left corner where the region is pasted. Transparent pixels don't erase.
    pub to: PasteAt,
}

// --- Animación ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Inbetween {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub from: usize,
    pub to: usize,
    /// Frames to insert between `from` and `to` (1-64).
    pub count: usize,
}

// --- Ver y leer ---

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Render {
    pub doc: String,
    pub artboard: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Integer zoom 1-16. Default: the largest that keeps the image within 512 px.
    pub scale: Option<u32>,
    /// Show the previous/next frames as tinted ghosts.
    #[serde(default)]
    pub onion: bool,
    /// Background color behind transparent pixels (default: transparent).
    pub background: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadPixels {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Read one layer; default: the visible composite of all layers.
    pub layer: Option<Ref>,
    #[serde(default)]
    pub frame: usize,
    /// Default: the whole canvas. At most 128×128.
    pub rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    /// One frame as PNG.
    Png,
    /// Animated GIF of all frames.
    Gif,
    /// One PNG per frame: name_001.png, name_002.png…
    Sequence,
    /// Spritesheet PNG + Aseprite-style JSON of this artboard.
    Sheet,
    /// All artboards in one PNG + JSON.
    Atlas,
    /// Spritesheet PNG + Godot 4 SpriteFrames (.tres): one animation per tag.
    Godot,
    /// Spritesheet PNG + Unity .meta with one sprite per frame (pivots from slices).
    Unity,
    /// Spritesheet PNG + Bevy atlas (.ron): grid, animations and slices.
    Bevy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LayoutArg {
    #[default]
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Export {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Output file (.png or .gif). Sheets and atlases also write a .json next to it.
    pub path: String,
    pub kind: ExportKind,
    /// Integer scale (default 1).
    pub scale: Option<u32>,
    /// Frame for kind=png.
    #[serde(default)]
    pub frame: usize,
    /// Sheet layout.
    #[serde(default)]
    pub layout: LayoutArg,
    /// Columns for layout=grid.
    pub columns: Option<u32>,
    /// JSON "frames" as an array instead of a hash.
    #[serde(default)]
    pub json_array: bool,
    /// Also write `<name>_n.png`: a normal map for 2D lighting (png, sheet, godot, unity, bevy).
    #[serde(default)]
    pub normal_map: bool,
    /// Godot: `res://` folder where the sheet will live (default "res://").
    pub godot_folder: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSlice {
    pub doc: String,
    pub artboard: Option<Ref>,
    /// Creates the slice or updates the one with this name.
    pub name: String,
    /// The rectangle applies from this frame on (default 0 = every frame).
    #[serde(default)]
    pub frame: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// Pivot relative to the slice's top-left corner, e.g. [w/2, h] for the feet.
    pub pivot: Option<[i32; 2]>,
    pub color: Option<String>,
    /// Free text exported with the slice (e.g. "damage=3").
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveSlice {
    pub doc: String,
    pub artboard: Option<Ref>,
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApplyCommands {
    pub doc: String,
    /// Serialized DocCommand values, applied atomically as one undo step, e.g.
    /// {"OnArtboard": {"id": 1, "cmd": {"SetFrameDuration": {"index": 0, "duration_ms": 80}}}}.
    pub commands: Vec<serde_json::Value>,
}
