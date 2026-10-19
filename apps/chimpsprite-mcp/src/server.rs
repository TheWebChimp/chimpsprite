//! Herramientas MCP: cada una delega en `cs_agent::Session` y traduce la respuesta.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use cs_agent::params::*;
use cs_agent::{Reply, Session};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};

const INSTRUCTIONS: &str = "ChimpSprite: pixel-art editor (Aseprite-like) you drive with tools. \
Workflow: new_document or open → describe (layers, frames, cels, palette) → draw/animate → \
render to look at the result (returns an image) or read_pixels for exact colors → save (.chimp or \
.aseprite) / export (png, gif, sequence, sheet, atlas). Every change is one undo step (undo/redo). \
Coordinates are artboard pixels from the top-left; frames are 0-based. Colors are hex like \
\"#ff8800\" or \"transparent\" (indexed sprites also take palette indices). When layer is omitted \
the topmost image layer is used; when artboard is omitted, the first one.";

#[derive(Clone)]
pub struct ChimpServer {
    session: Arc<Mutex<Session>>,
}

impl ChimpServer {
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            session: Arc::new(Mutex::new(Session::new(root))),
        }
    }

    fn with(&self, f: impl FnOnce(&mut Session) -> cs_agent::Result<Reply>) -> CallToolResult {
        let mut session = self.session.lock().unwrap_or_else(|e| e.into_inner());
        to_result(f(&mut session))
    }
}

fn to_result(r: cs_agent::Result<Reply>) -> CallToolResult {
    match r {
        Ok(reply) => {
            let mut content = vec![ContentBlock::text(reply.text)];
            if let Some(png) = reply.image {
                let data = base64::engine::general_purpose::STANDARD.encode(png);
                content.push(ContentBlock::image(data, "image/png"));
            }
            CallToolResult::success(content)
        }
        Err(e) => CallToolResult::error(vec![ContentBlock::text(e.to_string())]),
    }
}

#[tool_router]
impl ChimpServer {
    // --- Documentos ---

    #[tool(
        description = "Create a new sprite document; returns its id (e.g. \"d1\"). It starts with one artboard (id 1), one image layer (id 1) and one frame (0)."
    )]
    fn new_document(&self, Parameters(p): Parameters<NewDocument>) -> CallToolResult {
        self.with(|s| s.new_document(p))
    }

    #[tool(
        description = "Open a .aseprite/.ase, .chimp or .png file as a new document. Returns its id and full description."
    )]
    fn open(&self, Parameters(p): Parameters<Open>) -> CallToolResult {
        self.with(|s| s.open(p))
    }

    #[tool(
        description = "Save a document. The extension picks the format: .chimp keeps everything (several artboards, guides); .aseprite holds one artboard."
    )]
    fn save(&self, Parameters(p): Parameters<Save>) -> CallToolResult {
        self.with(|s| s.save(p))
    }

    #[tool(description = "Close a document, discarding unsaved changes.")]
    fn close(&self, Parameters(p): Parameters<DocOnly>) -> CallToolResult {
        self.with(|s| s.close(p))
    }

    #[tool(description = "List open documents with their artboards.")]
    fn list_documents(&self) -> CallToolResult {
        self.with(|s| s.list_documents())
    }

    #[tool(
        description = "Describe a document as JSON: artboards with size, color mode, palette, layers (bottom to top), frame durations, tags and cels (cels with the same image id are linked)."
    )]
    fn describe(&self, Parameters(p): Parameters<DocOnly>) -> CallToolResult {
        self.with(|s| s.describe(p))
    }

    #[tool(description = "Undo the last change of a document.")]
    fn undo(&self, Parameters(p): Parameters<DocOnly>) -> CallToolResult {
        self.with(|s| s.undo(p))
    }

    #[tool(description = "Redo the last undone change of a document.")]
    fn redo(&self, Parameters(p): Parameters<DocOnly>) -> CallToolResult {
        self.with(|s| s.redo(p))
    }

    #[tool(
        description = "Advanced: apply raw serialized DocCommand values atomically as one undo step. Prefer the specific tools."
    )]
    fn apply_commands(&self, Parameters(p): Parameters<ApplyCommands>) -> CallToolResult {
        self.with(|s| s.apply_commands(p))
    }

    // --- Artboards ---

    #[tool(description = "Add an artboard: an independent sprite placed on the document canvas.")]
    fn add_artboard(&self, Parameters(p): Parameters<AddArtboard>) -> CallToolResult {
        self.with(|s| s.add_artboard(p))
    }

    #[tool(
        description = "Rename, move or resize an artboard (resizing keeps content anchored at the top-left)."
    )]
    fn set_artboard(&self, Parameters(p): Parameters<SetArtboard>) -> CallToolResult {
        self.with(|s| s.set_artboard(p))
    }

    #[tool(description = "Remove an artboard (a document keeps at least one).")]
    fn remove_artboard(&self, Parameters(p): Parameters<ArtboardOnly>) -> CallToolResult {
        self.with(|s| s.remove_artboard(p))
    }

    // --- Capas ---

    #[tool(description = "Add an image layer or a group. Returns the new layer id.")]
    fn add_layer(&self, Parameters(p): Parameters<AddLayer>) -> CallToolResult {
        self.with(|s| s.add_layer(p))
    }

    #[tool(
        description = "Change a layer's name, visibility, lock, opacity (0-255), blend mode, clipping mask (clip) or mask on/off (mask_enabled)."
    )]
    fn set_layer(&self, Parameters(p): Parameters<SetLayer>) -> CallToolResult {
        self.with(|s| s.set_layer(p))
    }

    #[tool(description = "Remove a layer (groups are removed with their content).")]
    fn remove_layer(&self, Parameters(p): Parameters<LayerOnly>) -> CallToolResult {
        self.with(|s| s.remove_layer(p))
    }

    #[tool(description = "Move a layer above or below another, or into a group.")]
    fn move_layer(&self, Parameters(p): Parameters<MoveLayer>) -> CallToolResult {
        self.with(|s| s.move_layer(p))
    }

    #[tool(
        description = "Add a layer mask (hides parts of a layer without erasing). Then paint it with on_mask: true on draw_* / fill / gradient: white reveals, black hides."
    )]
    fn add_mask(&self, Parameters(p): Parameters<AddMask>) -> CallToolResult {
        self.with(|s| s.add_mask(p))
    }

    #[tool(description = "Delete a layer mask (the layer shows fully again).")]
    fn remove_mask(&self, Parameters(p): Parameters<LayerOnly>) -> CallToolResult {
        self.with(|s| s.remove_mask(p))
    }

    #[tool(description = "Apply a layer mask to the layer's pixels in every frame and remove it.")]
    fn apply_mask(&self, Parameters(p): Parameters<LayerOnly>) -> CallToolResult {
        self.with(|s| s.apply_mask(p))
    }

    #[tool(
        description = "Create or update a named slice: a rectangle (hitbox, hurtbox, anchor) with an optional pivot. Set it on a later frame to change it from there (per-frame hitboxes). Exported in sheet JSON, Unity pivots and Bevy RON."
    )]
    fn set_slice(&self, Parameters(p): Parameters<SetSlice>) -> CallToolResult {
        self.with(|s| s.set_slice(p))
    }

    #[tool(description = "Remove a slice by name.")]
    fn remove_slice(&self, Parameters(p): Parameters<RemoveSlice>) -> CallToolResult {
        self.with(|s| s.remove_slice(p))
    }

    #[tool(
        description = "Place another artboard inside this one as an instance layer (like an Adobe Animate symbol): it shows the source artboard with its own timeline, and editing the source updates every instance. Fails if it would make an artboard contain itself."
    )]
    fn add_instance_layer(&self, Parameters(p): Parameters<AddInstanceLayer>) -> CallToolResult {
        self.with(|s| s.add_instance_layer(p))
    }

    #[tool(
        description = "Change an instance layer from a frame on (creates a keyframe there): position, flip, opacity, source artboard and timing (loop / once / single follow the parent's frames; movie_clip plays on its own clock)."
    )]
    fn set_instance_key(&self, Parameters(p): Parameters<SetInstanceKey>) -> CallToolResult {
        self.with(|s| s.set_instance_key(p))
    }

    #[tool(description = "Remove an instance keyframe; removing the only one removes the layer.")]
    fn remove_instance_key(&self, Parameters(p): Parameters<RemoveInstanceKey>) -> CallToolResult {
        self.with(|s| s.remove_instance_key(p))
    }

    // --- Tilemaps ---

    #[tool(
        description = "Create an empty tileset in an artboard (tile 0 is always the empty tile). Paint tiles with set_tile_pixels and draw with them on a tilemap layer (add_tilemap_layer, set_tiles)."
    )]
    fn create_tileset(&self, Parameters(p): Parameters<CreateTileset>) -> CallToolResult {
        self.with(|s| s.create_tileset(p))
    }

    #[tool(
        description = "Paint pixels of one tile, in tile coordinates. Use tile = the current tile count to add a new tile; tile 0 cannot be edited. Every cel using the tile updates."
    )]
    fn set_tile_pixels(&self, Parameters(p): Parameters<SetTilePixels>) -> CallToolResult {
        self.with(|s| s.set_tile_pixels(p))
    }

    #[tool(description = "Add a tilemap layer on top that draws tiles from a tileset.")]
    fn add_tilemap_layer(&self, Parameters(p): Parameters<AddTilemapLayer>) -> CallToolResult {
        self.with(|s| s.add_tilemap_layer(p))
    }

    #[tool(
        description = "Place tiles on a tilemap layer. Cells are canvas grid cells: cell (x, y) starts at pixel (x * tile_width, y * tile_height). Tile 0 erases a cell; optional flip_x / flip_y / flip_d. The layer's grid grows as needed, up to 4096 tiles per side and 67108864 pixels in total (grid width × tile width × grid height × tile height)."
    )]
    fn set_tiles(&self, Parameters(p): Parameters<SetTiles>) -> CallToolResult {
        self.with(|s| s.set_tiles(p))
    }

    #[tool(
        description = "Read a tilemap layer's grid on a frame: tile index and flips of each cell, and the cel position."
    )]
    fn get_tiles(&self, Parameters(p): Parameters<GetTiles>) -> CallToolResult {
        self.with(|s| s.get_tiles(p))
    }

    // --- Frames, tags y cels ---

    #[tool(description = "Add an empty frame, or duplicate an existing one (optionally linked).")]
    fn add_frame(&self, Parameters(p): Parameters<AddFrame>) -> CallToolResult {
        self.with(|s| s.add_frame(p))
    }

    #[tool(description = "Remove a frame (tags are adjusted).")]
    fn remove_frame(&self, Parameters(p): Parameters<FrameOnly>) -> CallToolResult {
        self.with(|s| s.remove_frame(p))
    }

    #[tool(description = "Move a frame to another position.")]
    fn move_frame(&self, Parameters(p): Parameters<MoveFrame>) -> CallToolResult {
        self.with(|s| s.move_frame(p))
    }

    #[tool(description = "Set the duration (ms) of some or all frames.")]
    fn set_frame_duration(&self, Parameters(p): Parameters<SetFrameDuration>) -> CallToolResult {
        self.with(|s| s.set_frame_duration(p))
    }

    #[tool(
        description = "Create or replace an animation tag (named frame range with a playback direction)."
    )]
    fn set_tag(&self, Parameters(p): Parameters<SetTag>) -> CallToolResult {
        self.with(|s| s.set_tag(p))
    }

    #[tool(description = "Remove an animation tag by name.")]
    fn remove_tag(&self, Parameters(p): Parameters<RemoveTag>) -> CallToolResult {
        self.with(|s| s.remove_tag(p))
    }

    #[tool(description = "Link the cels of a layer across a frame range so they share one image.")]
    fn link_cels(&self, Parameters(p): Parameters<CelRange>) -> CallToolResult {
        self.with(|s| s.link_cels(p))
    }

    #[tool(description = "Give each linked cel in a frame range its own copy of the image.")]
    fn unlink_cels(&self, Parameters(p): Parameters<CelRange>) -> CallToolResult {
        self.with(|s| s.unlink_cels(p))
    }

    #[tool(description = "Empty a cel (layer + frame).")]
    fn clear_cel(&self, Parameters(p): Parameters<CelOnly>) -> CallToolResult {
        self.with(|s| s.clear_cel(p))
    }

    #[tool(
        description = "Move cels (their whole image) by (dx, dy) in one frame or a range; linked cels stay linked. Combine with inbetween to animate movement."
    )]
    fn move_cel(&self, Parameters(p): Parameters<MoveCel>) -> CallToolResult {
        self.with(|s| s.move_cel(p))
    }

    #[tool(
        description = "Insert `count` in-between frames before `to`. Each layer reuses its `from` image (linked); when `to` shows the same image at another position, the position is interpolated."
    )]
    fn inbetween(&self, Parameters(p): Parameters<Inbetween>) -> CallToolResult {
        self.with(|s| s.inbetween(p))
    }

    // --- Dibujo ---

    #[tool(
        description = "Set individual pixels, each with its own color. Use color \"transparent\" to erase."
    )]
    fn draw_pixels(&self, Parameters(p): Parameters<DrawPixels>) -> CallToolResult {
        self.with(|s| s.draw_pixels(p))
    }

    #[tool(
        description = "Draw a pixel-perfect polyline through the given points with a square brush. Use color \"transparent\" to erase."
    )]
    fn draw_line(&self, Parameters(p): Parameters<DrawLine>) -> CallToolResult {
        self.with(|s| s.draw_line(p))
    }

    #[tool(description = "Draw a rectangle (outline or filled) between two inclusive corners.")]
    fn draw_rect(&self, Parameters(p): Parameters<DrawShape>) -> CallToolResult {
        self.with(|s| s.draw_rect(p))
    }

    #[tool(
        description = "Draw an ellipse (outline or filled) inside the box between two inclusive corners."
    )]
    fn draw_ellipse(&self, Parameters(p): Parameters<DrawShape>) -> CallToolResult {
        self.with(|s| s.draw_ellipse(p))
    }

    #[tool(
        description = "Bucket fill on a layer: the connected area of the clicked color, or every pixel of that color with contiguous=false."
    )]
    fn fill(&self, Parameters(p): Parameters<Fill>) -> CallToolResult {
        self.with(|s| s.fill(p))
    }

    #[tool(
        description = "Fill an area (default: whole canvas) with a linear or radial gradient between two colors, optionally with ordered (Bayer) dithering."
    )]
    fn gradient(&self, Parameters(p): Parameters<Gradient>) -> CallToolResult {
        self.with(|s| s.gradient(p))
    }

    #[tool(
        description = "Write text with the built-in 5×7 pixel font or a vector font (Hack, Ubuntu), at (x, y) top-left."
    )]
    fn draw_text(&self, Parameters(p): Parameters<DrawText>) -> CallToolResult {
        self.with(|s| s.draw_text(p))
    }

    // --- Color ---

    #[tool(description = "Replace the artboard palette with a list of hex colors.")]
    fn set_palette(&self, Parameters(p): Parameters<SetPalette>) -> CallToolResult {
        self.with(|s| s.set_palette(p))
    }

    #[tool(
        description = "Replace colors (from → to) in a cel, a whole layer, a whole frame or the whole sprite."
    )]
    fn recolor(&self, Parameters(p): Parameters<Recolor>) -> CallToolResult {
        self.with(|s| s.recolor(p))
    }

    #[tool(
        description = "Reduce the sprite to at most N colors (median cut) or to a given palette; optionally convert it to indexed mode."
    )]
    fn quantize(&self, Parameters(p): Parameters<Quantize>) -> CallToolResult {
        self.with(|s| s.quantize(p))
    }

    // --- Transformaciones ---

    #[tool(
        description = "Mirror a layer's pixels horizontally or vertically, in a region or the whole canvas, for one frame or a range."
    )]
    fn flip(&self, Parameters(p): Parameters<Flip>) -> CallToolResult {
        self.with(|s| s.flip(p))
    }

    #[tool(
        description = "Rotate a layer's pixels clockwise by 90, 180 or 270 degrees (90/270 need a square region)."
    )]
    fn rotate(&self, Parameters(p): Parameters<Rotate>) -> CallToolResult {
        self.with(|s| s.rotate(p))
    }

    #[tool(
        description = "Shift a layer's pixels by (dx, dy), optionally wrapping around the region."
    )]
    fn shift(&self, Parameters(p): Parameters<Shift>) -> CallToolResult {
        self.with(|s| s.shift(p))
    }

    #[tool(
        description = "Copy a rectangle from one layer/frame and paste it on another layer/frame at (x, y). Transparent pixels don't erase."
    )]
    fn copy_region(&self, Parameters(p): Parameters<CopyRegion>) -> CallToolResult {
        self.with(|s| s.copy_region(p))
    }

    // --- Ver, leer, exportar ---

    #[tool(
        description = "Render a frame as a PNG image so you can look at it (scaled up for visibility; optional onion skin and background)."
    )]
    fn render(&self, Parameters(p): Parameters<Render>) -> CallToolResult {
        self.with(|s| s.render(p))
    }

    #[tool(
        description = "Read pixels as a text grid with a color legend ('.' = transparent), from one layer or the visible composite. At most 128×128."
    )]
    fn read_pixels(&self, Parameters(p): Parameters<ReadPixels>) -> CallToolResult {
        self.with(|s| s.read_pixels(p))
    }

    #[tool(
        description = "Export to PNG, animated GIF, PNG sequence, spritesheet (+JSON), atlas of all artboards (+JSON), or game-engine formats: Godot SpriteFrames, Unity sliced sprites, Bevy atlas. Optionally also a normal map for 2D lighting."
    )]
    fn export(&self, Parameters(p): Parameters<Export>) -> CallToolResult {
        self.with(|s| s.export(p))
    }
}

#[tool_handler]
impl ServerHandler for ChimpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "chimpsprite",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}
