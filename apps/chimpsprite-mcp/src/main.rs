//! Servidor MCP de ChimpSprite por stdio: `chimpsprite-mcp [--root <carpeta>]`.

use std::path::PathBuf;

use rmcp::ServiceExt;
use rmcp::transport::stdio;

mod server;

const USAGE: &str =
    "chimpsprite-mcp: MCP server (stdio) to create, draw, animate and export pixel art.

Usage: chimpsprite-mcp [--root <dir>]

  --root <dir>  only read and write files inside <dir> (relative paths resolve there)
";

fn parse_args() -> Result<Option<PathBuf>, String> {
    let mut args = std::env::args().skip(1);
    let mut root = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = Some(PathBuf::from(args.next().ok_or("--root needs a folder")?)),
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'\n\n{USAGE}")),
        }
    }
    Ok(root)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = parse_args().map_err(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    })?;
    let service = server::ChimpServer::new(root).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
