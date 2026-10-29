use cs_core::{ColorMode, Document, PixelValue, Rgba, Sprite};
use cs_script::{Context, Language, run};

fn document() -> Document {
    Document::with_sprite("art", Sprite::new(8, 8, ColorMode::Rgba))
}

fn context(doc: &Document) -> Context {
    let a = &doc.artboards[0];
    Context::new(a.id.0, a.sprite.layers[0].id.0, 0)
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
        let mut ctx = context(&doc);
        ctx.export_folder = Some(temp("examples").display().to_string());
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

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cs-script-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn example(name: &str) -> &'static str {
    cs_script::EXAMPLES
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
        .unwrap_or_else(|| panic!("{name}"))
}

/// Un punto rojo en (3, 3).
fn dot() -> Document {
    let mut doc = document();
    let s = &mut doc.artboards[0].sprite;
    cs_commands_draw(s, 3, 3, RED);
    doc
}

fn cs_commands_draw(s: &mut Sprite, x: i32, y: i32, v: PixelValue) {
    let layer = s.layers[0].id;
    let image = cs_core::Image::new(s.width, s.height, s.color_mode);
    let id = s.next_image_id();
    s.images.insert(id, std::sync::Arc::new(image));
    s.cels.insert(
        (layer, 0),
        cs_core::Cel {
            position: (0, 0),
            opacity: 255,
            image: id,
        },
    );
    let img = std::sync::Arc::make_mut(s.images.get_mut(&id).unwrap());
    img.set(x as u32, y as u32, v).unwrap();
}

#[test]
fn outline_rings_what_is_painted_and_keeps_to_the_selection() {
    for lang in ["rhai", "lua"] {
        let doc = dot();
        let mut ctx = context(&doc);
        ctx.primary = Rgba::rgb(0, 0, 255);
        let out = run(
            Language::of(lang).unwrap(),
            example(&format!("outline.{lang}")),
            doc,
            ctx,
        )
        .unwrap_or_else(|e| panic!("{lang}: {}", e.message));
        let blue = PixelValue::Rgba(Rgba::rgb(0, 0, 255));
        for (x, y) in [(2, 3), (4, 3), (3, 2), (3, 4)] {
            assert_eq!(pixel(&out.document, x, y), blue, "{lang}: ({x}, {y})");
        }
        assert_eq!(pixel(&out.document, 3, 3), RED, "{lang}: the dot stays");
        assert_ne!(pixel(&out.document, 2, 2), blue, "{lang}: no corners");
        // Con selección, solo dentro.
        let doc = dot();
        let mut ctx = context(&doc);
        ctx.primary = Rgba::rgb(0, 0, 255);
        ctx.selection = Some(vec![(2, 3)]);
        let out = run(
            Language::of(lang).unwrap(),
            example(&format!("outline.{lang}")),
            doc,
            ctx,
        )
        .unwrap();
        assert_eq!(pixel(&out.document, 2, 3), blue, "{lang}");
        assert_ne!(
            pixel(&out.document, 4, 3),
            blue,
            "{lang}: outside the selection"
        );
    }
}

#[test]
fn color_ramp_adds_seven_shades_of_the_primary() {
    for lang in ["rhai", "lua"] {
        let doc = document();
        let before = doc.artboards[0].sprite.palette.len();
        let mut ctx = context(&doc);
        ctx.primary = Rgba::rgb(200, 60, 60);
        let out = run(
            Language::of(lang).unwrap(),
            example(&format!("color_ramp.{lang}")),
            doc,
            ctx,
        )
        .unwrap_or_else(|e| panic!("{lang}: {}", e.message));
        let palette = &out.document.artboards[0].sprite.palette;
        assert_eq!(palette.len(), before + 7, "{lang}");
        assert_eq!(
            palette.get((before + 3) as u8),
            Some(Rgba::rgb(200, 60, 60)),
            "{lang}: the base in the middle"
        );
        let luma = |c: Rgba| c.r as u32 * 3 + c.g as u32 * 6 + c.b as u32;
        let ramp: Vec<_> = (before..before + 7)
            .map(|i| luma(palette.get(i as u8).unwrap()))
            .collect();
        assert!(
            ramp.windows(2).all(|w| w[0] < w[1]),
            "{lang}: dark to light {ramp:?}"
        );
    }
}

#[test]
fn export_tags_writes_a_gif_per_tag() {
    for lang in ["rhai", "lua"] {
        let mut doc = dot();
        let s = &mut doc.artboards[0].sprite;
        s.frames.push(s.frames[0]);
        s.tags.push(cs_core::Tag {
            name: "Idle".into(),
            from: 0,
            to: 1,
            direction: cs_core::AnimDirection::Forward,
            color: Rgba::rgb(0, 0, 0),
            repeat: 0,
        });
        let dir = temp(&format!("tags-{lang}"));
        let mut ctx = context(&doc);
        ctx.export_folder = Some(dir.display().to_string());
        let out = run(
            Language::of(lang).unwrap(),
            example(&format!("export_tags.{lang}")),
            doc,
            ctx,
        )
        .unwrap_or_else(|e| panic!("{lang}: {}", e.message));
        let gif = dir.join("art_Idle.gif");
        assert!(gif.exists(), "{lang}: {}", out.output);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn pixels_and_random_work_the_same_in_both() {
    let rhai = r#"
        let p = pixels();
        print(p[3][3]);
        print(p[0][0]);
        let r = random(1, 6);
        if r < 1 || r > 6 { throw "out of range"; }
        let f = random();
        if f < 0.0 || f >= 1.0 { throw "out of range"; }
    "#;
    let lua = r#"
        local p = pixels()
        print(p[4][4])
        print(p[1][1])
        local r = random(1, 6)
        if r < 1 or r > 6 then error("out of range") end
        local f = random()
        if f < 0 or f >= 1 then error("out of range") end
    "#;
    for (lang, src) in [(Language::Rhai, rhai), (Language::Lua, lua)] {
        let doc = dot();
        let ctx = context(&doc);
        let out = run(lang, src, doc, ctx).unwrap_or_else(|e| panic!("{lang:?}: {}", e.message));
        assert_eq!(
            out.output,
            "#ff0000ff
transparent",
            "{lang:?}"
        );
    }
}
