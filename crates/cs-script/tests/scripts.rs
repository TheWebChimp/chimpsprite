use cs_core::{ColorMode, Document, PixelValue, Rgba, Sprite};
use cs_script::{Context, Language, run};

fn document() -> Document {
    Document::with_sprite("art", Sprite::new(8, 8, ColorMode::Rgba))
}

fn context(doc: &Document) -> Context {
    let a = &doc.artboards[0];
    Context {
        artboard: a.id.0,
        layer: a.sprite.layers[0].id.0,
        frame: 0,
    }
}

fn pixel(doc: &Document, x: i32, y: i32) -> PixelValue {
    let s = &doc.artboards[0].sprite;
    s.pixel(s.layers[0].id, 0, x, y)
}

const RED: PixelValue = PixelValue::Rgba(Rgba::rgb(255, 0, 0));

#[test]
fn rhai_draws_on_the_current_layer_and_prints() {
    let doc = document();
    let ctx = context(&doc);
    let out = run(
        Language::Rhai,
        r##"
            draw_line(#{points: [[0, 0], [7, 0]], color: "#ff0000"});
            let info = describe();
            print(`layers: ${info.artboards[0].layers_bottom_to_top.len()}`);
        "##,
        doc,
        ctx,
    )
    .expect("runs");
    assert_eq!(pixel(&out.document, 7, 0), RED);
    assert_eq!(out.output, "layers: 1");
}

#[test]
fn lua_does_the_same_with_tables() {
    let doc = document();
    let ctx = context(&doc);
    let out = run(
        Language::Lua,
        r##"
            draw_line{points = {{0, 0}, {7, 0}}, color = "#ff0000"}
            op("draw_rect", {x0 = 0, y0 = 2, x1 = 2, y1 = 4, color = "#ff0000", filled = true})
            print("frame", current_frame())
        "##,
        doc,
        ctx,
    )
    .expect("runs");
    assert_eq!(pixel(&out.document, 7, 0), RED);
    assert_eq!(pixel(&out.document, 1, 3), RED);
    assert_eq!(out.output, "frame\t0");
}

#[test]
fn an_error_says_where_and_keeps_what_was_printed() {
    for (lang, src) in [
        (
            Language::Rhai,
            "print(\"before\");\ndraw_line(#{color: \"#ff0000\"});",
        ),
        (
            Language::Lua,
            "print(\"before\")\ndraw_line{color = \"#ff0000\"}",
        ),
    ] {
        let doc = document();
        let ctx = context(&doc);
        let err = run(lang, src, doc, ctx).expect_err("missing points");
        assert!(err.message.contains("points"), "{lang:?}: {}", err.message);
        assert_eq!(err.output, "before");
    }
}

#[test]
fn an_endless_loop_is_stopped() {
    for (lang, src) in [
        (Language::Rhai, "loop { }"),
        (Language::Lua, "while true do end"),
    ] {
        let doc = document();
        let ctx = context(&doc);
        assert!(run(lang, src, doc, ctx).is_err(), "{lang:?}");
    }
}

#[test]
fn the_language_comes_from_the_extension() {
    assert_eq!(Language::of("outline.rhai"), Some(Language::Rhai));
    assert_eq!(Language::of("Outline.LUA"), Some(Language::Lua));
    assert_eq!(Language::of("notes.txt"), None);
}

#[test]
fn every_example_runs_and_both_languages_agree() {
    let mut results = std::collections::HashMap::new();
    for (name, source) in cs_script::EXAMPLES {
        let lang = Language::of(name).expect("rhai or lua");
        let doc = document();
        let ctx = context(&doc);
        let out = run(lang, source, doc, ctx).unwrap_or_else(|e| panic!("{name}: {}", e.message));
        let stem = name.rsplit_once('.').unwrap().0;
        let sprite = out.document.artboards[0].sprite.clone();
        if let Some(other) = results.insert(stem.to_string(), (sprite.clone(), out.output.clone()))
        {
            assert_eq!(other.1, out.output, "{stem}: same output");
            assert_eq!(other.0.frames, sprite.frames, "{stem}: same frames");
            assert_eq!(other.0.tags, sprite.tags, "{stem}: same tags");
            let render = |s: &Sprite| cs_render_frames(s);
            assert_eq!(render(&other.0), render(&sprite), "{stem}: same pixels");
        }
    }
}

fn cs_render_frames(s: &Sprite) -> Vec<Vec<PixelValue>> {
    (0..s.frames.len())
        .map(|f| {
            (0..s.height as i32)
                .flat_map(|y| (0..s.width as i32).map(move |x| (x, y)))
                .map(|(x, y)| s.pixel(s.layers[0].id, f, x, y))
                .collect()
        })
        .collect()
}
