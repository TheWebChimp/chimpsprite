//! Subcomando `chimpsprite --thumbnail <entrada> <salida> [tamaño]`: escribe la miniatura de un
//! `.chimp` como PNG sin abrir la ventana. Lo usa el thumbnailer de Linux
//! (`linux/chimpsprite.thumbnailer`, con `%i %o %s`); sirve igual en Windows y macOS para probarlo.

use std::path::PathBuf;

pub const FLAG: &str = "--thumbnail";
/// Tamaño si no se pide uno (el de la miniatura incrustada).
const DEFAULT_SIZE: u32 = cs_io::thumbnail::SIZE;
/// Los thumbnailers piden 128, 256, 512 o 1024.
const MAX_SIZE: u32 = 4096;

#[derive(Debug, PartialEq, Eq)]
pub struct Args {
    pub input: PathBuf,
    pub output: PathBuf,
    pub size: u32,
}

const USAGE: &str = "usage: chimpsprite --thumbnail <input.chimp> <output.png> [size]";

/// Los argumentos que van después de `--thumbnail`.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let (input, output, size) = match args {
        [input, output] => (input, output, None),
        [input, output, size] => (input, output, Some(size)),
        _ => return Err(USAGE.into()),
    };
    let size = match size {
        None => DEFAULT_SIZE,
        Some(text) => match text.parse::<u32>() {
            Ok(n @ 1..=MAX_SIZE) => n,
            _ => return Err(format!("bad size \"{text}\" (1 to {MAX_SIZE})")),
        },
    };
    Ok(Args {
        input: input.into(),
        output: output.into(),
        size,
    })
}

pub fn run(args: &Args) -> Result<(), String> {
    let name = args.input.display();
    let bytes = std::fs::read(&args.input).map_err(|e| format!("{name}: {e}"))?;
    let image = cs_io::thumbnail::for_file(&bytes, args.size)
        .map_err(|e| format!("{name}: {e}"))?
        .ok_or_else(|| format!("{name}: nothing to show"))?;
    let png = image.to_png().map_err(|e| e.to_string())?;
    std::fs::write(&args.output, png).map_err(|e| format!("{}: {e}", args.output.display()))
}

/// Si la línea de comandos pide el subcomando, lo corre y devuelve el código de salida.
pub fn main_if_requested() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some(FLAG) {
        return None;
    }
    let result = parse(&args[1..]).and_then(|a| run(&a));
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    })
}

#[cfg(test)]
mod tests {
    use cs_core::{ColorMode, Document, LayerId, PixelValue, Rgba, Sprite};

    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_reads_input_output_and_size() {
        let a = parse(&strings(&["a.chimp", "b.png", "128"])).unwrap();
        assert_eq!(
            a,
            Args {
                input: "a.chimp".into(),
                output: "b.png".into(),
                size: 128
            }
        );
        assert_eq!(parse(&strings(&["a.chimp", "b.png"])).unwrap().size, 256);
        assert!(parse(&strings(&["a.chimp"])).is_err());
        assert!(parse(&strings(&["a", "b", "c", "d"])).is_err());
        for bad in ["0", "-3", "big", "5000"] {
            assert!(parse(&strings(&["a", "b", bad])).is_err(), "{bad}");
        }
    }

    /// Un `.chimp` 2×2 con un punto rojo en (0, 0), con o sin la miniatura incrustada.
    fn chimp(with_thumbnail: bool) -> Vec<u8> {
        let mut sprite = Sprite::new(2, 2, ColorMode::Rgba);
        cs_commands::Command::DrawPixels {
            layer: LayerId(1),
            frame: 0,
            pixels: vec![(0, 0, PixelValue::Rgba(Rgba::rgb(255, 0, 0)))],
        }
        .apply(&mut sprite)
        .unwrap();
        let doc = Document::with_sprite("dot", sprite);
        let from = match with_thumbnail {
            true => cs_io::chimp::ThumbnailFrom::Auto,
            false => cs_io::chimp::ThumbnailFrom::None,
        };
        cs_io::chimp::write_with(&doc, from).unwrap()
    }

    #[test]
    fn run_writes_the_png_for_new_and_old_files() {
        let dir =
            std::env::temp_dir().join(format!("chimpsprite-thumb-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for with_thumbnail in [true, false] {
            let input = dir.join(format!("dot-{with_thumbnail}.chimp"));
            let output = dir.join(format!("dot-{with_thumbnail}.png"));
            std::fs::write(&input, chimp(with_thumbnail)).unwrap();
            run(&Args {
                input,
                output: output.clone(),
                size: 64,
            })
            .unwrap();
            let png = cs_io::import::decode_png(&std::fs::read(&output).unwrap()).unwrap();
            assert_eq!((png.width, png.height), (64, 64), "{with_thumbnail}");
            assert_eq!(png.pixels[0], Rgba::rgb(255, 0, 0));
            assert_eq!(png.pixels[32], Rgba::TRANSPARENT);
        }
        let missing = run(&Args {
            input: dir.join("nope.chimp"),
            output: dir.join("nope.png"),
            size: 64,
        });
        assert!(missing.unwrap_err().contains("nope.chimp"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_linux_thumbnailer_calls_this_subcommand() {
        let entry = include_str!("../linux/chimpsprite.thumbnailer");
        assert!(entry.starts_with("[Thumbnailer Entry]"));
        let exec = entry
            .lines()
            .find_map(|l| l.strip_prefix("Exec="))
            .expect("Exec line");
        let args: Vec<String> = exec
            .replace("%i", "/in.chimp")
            .replace("%o", "/out.png")
            .replace("%s", "512")
            .split_whitespace()
            .map(String::from)
            .collect();
        assert_eq!(args[0], "chimpsprite");
        assert_eq!(args[1], FLAG);
        let parsed = parse(&args[2..]).unwrap();
        assert_eq!(
            (parsed.input.to_str(), parsed.size),
            (Some("/in.chimp"), 512)
        );
        assert!(entry.contains("MimeType=application/x-chimpsprite;"));
        let mime = include_str!("../linux/chimpsprite-mime.xml");
        assert!(mime.contains(r#"<mime-type type="application/x-chimpsprite">"#));
        assert!(mime.contains(r#"<glob pattern="*.chimp"/>"#));
    }
}
