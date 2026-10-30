//! Las herramientas por nombre, con sus parámetros en JSON: lo que usan los scripts (Rhai y Lua)
//! para llamar a las mismas operaciones que el servidor MCP.

use cs_commands::History;
use cs_core::Document;
use serde::de::DeserializeOwned;

use crate::error::{Result, invalid};
use crate::session::{OpenDoc, Reply, Session};

/// Los nombres de las herramientas, en el orden del MCP.
pub const OPERATIONS: &[&str] = &[
    "new_document",
    "open",
    "save",
    "close",
    "list_documents",
    "describe",
    "undo",
    "redo",
    "apply_commands",
    "draw_pixels",
    "draw_line",
    "draw_rect",
    "draw_ellipse",
    "gradient",
    "draw_text",
    "fill",
    "set_palette",
    "recolor",
    "quantize",
    "add_artboard",
    "set_artboard",
    "remove_artboard",
    "add_layer",
    "set_layer",
    "add_mask",
    "remove_mask",
    "apply_mask",
    "remove_layer",
    "move_layer",
    "add_frame",
    "remove_frame",
    "move_frame",
    "set_frame_duration",
    "set_tag",
    "remove_tag",
    "set_slice",
    "remove_slice",
    "add_instance_layer",
    "convert_to_symbol",
    "move_to_library",
    "place_on_canvas",
    "set_instance_key",
    "remove_instance_key",
    "link_cels",
    "unlink_cels",
    "clear_cel",
    "move_cel",
    "inbetween",
    "create_tileset",
    "set_tile_pixels",
    "add_tilemap_layer",
    "set_tiles",
    "get_tiles",
    "convert_layer",
    "flip",
    "rotate",
    "shift",
    "copy_region",
    "render",
    "read_pixels",
    "export",
];

fn args<T: DeserializeOwned>(name: &str, value: serde_json::Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| invalid(format!("{name}: {e}")))
}

impl Session {
    /// Corre la herramienta `name` con `params` (los mismos que en el MCP, como objeto JSON).
    pub fn call(&mut self, name: &str, params: serde_json::Value) -> Result<Reply> {
        macro_rules! dispatch {
            ($($op:ident),* $(,)?) => {
                match name {
                    "list_documents" => self.list_documents(),
                    $(stringify!($op) => self.$op(args(name, params)?),)*
                    _ => Err(invalid(format!(
                        "unknown operation '{name}'; available: {}",
                        OPERATIONS.join(", ")
                    ))),
                }
            };
        }
        dispatch!(
            new_document,
            open,
            save,
            close,
            describe,
            undo,
            redo,
            apply_commands,
            draw_pixels,
            draw_line,
            draw_rect,
            draw_ellipse,
            gradient,
            draw_text,
            fill,
            set_palette,
            recolor,
            quantize,
            add_artboard,
            set_artboard,
            remove_artboard,
            add_layer,
            set_layer,
            add_mask,
            remove_mask,
            apply_mask,
            remove_layer,
            move_layer,
            add_frame,
            remove_frame,
            move_frame,
            set_frame_duration,
            set_tag,
            remove_tag,
            set_slice,
            remove_slice,
            add_instance_layer,
            convert_to_symbol,
            move_to_library,
            place_on_canvas,
            set_instance_key,
            remove_instance_key,
            link_cels,
            unlink_cels,
            clear_cel,
            move_cel,
            inbetween,
            create_tileset,
            set_tile_pixels,
            add_tilemap_layer,
            set_tiles,
            get_tiles,
            convert_layer,
            flip,
            rotate,
            shift,
            copy_region,
            render,
            read_pixels,
            export,
        )
    }

    /// Abre `document` (ya cargado, p. ej. el del editor) en la sesión; devuelve su id.
    pub fn insert_document(&mut self, document: Document) -> String {
        self.insert(OpenDoc {
            document,
            history: History::new(),
            path: None,
        })
    }

    /// El documento `id`, para leerlo.
    pub fn document(&self, id: &str) -> Option<&Document> {
        self.docs.get(id).map(|d| &d.document)
    }

    /// El documento `id` como quedó (y lo saca de la sesión).
    pub fn take_document(&mut self, id: &str) -> Option<Document> {
        self.docs.remove(id).map(|d| d.document)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn every_operation_is_dispatched_by_name() {
        let mut s = Session::new(None);
        let id = s.insert_document(Document::with_sprite(
            "a",
            cs_core::Sprite::new(4, 4, cs_core::ColorMode::Rgba),
        ));
        s.call(
            "draw_line",
            json!({"doc": id, "points": [[0, 0], [3, 0]], "color": "#ff0000"}),
        )
        .unwrap();
        let doc = s.take_document(&id).unwrap();
        let sprite = &doc.artboards[0].sprite;
        assert_eq!(
            sprite.pixel(sprite.layers[0].id, 0, 3, 0),
            cs_core::PixelValue::Rgba(cs_core::Rgba::rgb(255, 0, 0))
        );
        // Un nombre que no existe dice cuáles hay.
        let err = s.call("paint", json!({})).unwrap_err().to_string();
        assert!(err.contains("draw_line"), "{err}");
        // Todos los nombres de la lista se despachan (con parámetros malos fallan al leerlos,
        // no por desconocidos).
        for name in OPERATIONS {
            let err = s
                .call(name, json!({"bogus": true}))
                .err()
                .map(|e| e.to_string());
            assert!(
                !err.is_some_and(|e| e.contains("unknown operation")),
                "{name} is not dispatched"
            );
        }
    }
}
