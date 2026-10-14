use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use cs_commands::{Command, DocCommand, History};
use cs_core::{ArtboardId, Document, Sprite};

use crate::error::{Result, invalid};
use crate::params::Ref;
use crate::target;

/// Resultado de una herramienta: texto (a veces JSON) y, opcionalmente, una imagen PNG.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reply {
    pub text: String,
    pub image: Option<Vec<u8>>,
}

impl Reply {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            image: None,
        }
    }

    pub fn json(value: serde_json::Value) -> Self {
        Self::text(serde_json::to_string_pretty(&value).expect("json value serializes"))
    }
}

/// Documento abierto en la sesión.
pub(crate) struct OpenDoc {
    pub document: Document,
    pub history: History<DocCommand>,
    /// Archivo de origen o del último guardado (solo `.chimp`/`.aseprite`).
    pub path: Option<PathBuf>,
}

/// Documentos abiertos por el agente, con ids `d1`, `d2`…
pub struct Session {
    pub(crate) docs: BTreeMap<String, OpenDoc>,
    next: u32,
    root: Option<PathBuf>,
}

impl Session {
    /// Con `root`, toda ruta de archivo debe quedar dentro de esa carpeta.
    pub fn new(root: Option<PathBuf>) -> Self {
        Self {
            docs: BTreeMap::new(),
            next: 1,
            root: root.map(|r| normalize(&absolute(&r))),
        }
    }

    pub(crate) fn insert(&mut self, doc: OpenDoc) -> String {
        let id = format!("d{}", self.next);
        self.next += 1;
        self.docs.insert(id.clone(), doc);
        id
    }

    pub(crate) fn doc(&self, id: &str) -> Result<&OpenDoc> {
        self.docs.get(id).ok_or_else(|| self.unknown(id))
    }

    pub(crate) fn doc_mut(&mut self, id: &str) -> Result<&mut OpenDoc> {
        if !self.docs.contains_key(id) {
            return Err(self.unknown(id));
        }
        Ok(self.docs.get_mut(id).expect("checked"))
    }

    fn unknown(&self, id: &str) -> crate::AgentError {
        let open: Vec<_> = self.docs.keys().map(String::as_str).collect();
        invalid(format!(
            "unknown document '{id}'; open documents: [{}]. Use new_document or open first",
            open.join(", ")
        ))
    }

    /// Artboard indicado (o el primero) y su sprite.
    pub(crate) fn sprite(
        &self,
        doc: &str,
        artboard: Option<&Ref>,
    ) -> Result<(ArtboardId, &Sprite)> {
        let d = &self.doc(doc)?.document;
        let id = target::artboard(d, artboard)?;
        Ok((id, &d.artboard(id).expect("resolved").sprite))
    }

    /// Ejecuta los comandos como un solo paso de undo (atómico).
    pub(crate) fn run(&mut self, doc: &str, mut cmds: Vec<DocCommand>) -> Result<()> {
        let d = self.doc_mut(doc)?;
        let cmd = if cmds.len() == 1 {
            cmds.pop().expect("one")
        } else {
            DocCommand::Batch(cmds)
        };
        d.history.execute(&mut d.document, cmd)?;
        Ok(())
    }

    /// Ejecuta comandos de sprite sobre un artboard como un solo paso de undo.
    pub(crate) fn run_on(
        &mut self,
        doc: &str,
        id: ArtboardId,
        mut cmds: Vec<Command>,
    ) -> Result<()> {
        let cmd = if cmds.len() == 1 {
            cmds.pop().expect("one")
        } else {
            Command::Batch(cmds)
        };
        self.run(doc, vec![DocCommand::OnArtboard { id, cmd }])
    }

    /// Ruta para leer o escribir; con `root`, debe quedar dentro.
    pub(crate) fn path(&self, path: &str) -> Result<PathBuf> {
        let p = Path::new(path);
        let full = match (&self.root, p.is_absolute()) {
            (Some(root), false) => root.join(p),
            _ => absolute(p),
        };
        let full = normalize(&full);
        if let Some(root) = &self.root
            && !full.starts_with(root)
        {
            return Err(invalid(format!(
                "path '{path}' is outside the allowed folder {}",
                root.display()
            )));
        }
        Ok(full)
    }
}

fn absolute(p: &Path) -> PathBuf {
    std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf())
}

/// Resuelve `.` y `..` sin tocar el disco.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_confines_paths() {
        let root = std::env::temp_dir().join("cs-agent-root");
        let s = Session::new(Some(root.clone()));
        assert!(
            s.path("a/b.png")
                .unwrap()
                .starts_with(normalize(&absolute(&root)))
        );
        assert!(s.path("../escape.png").is_err());
        assert!(s.path("a/../../escape.png").is_err());
        let outside = std::env::temp_dir().join("elsewhere.png");
        assert!(s.path(outside.to_str().unwrap()).is_err());
        let free = Session::new(None);
        assert!(free.path("../anywhere.png").is_ok());
    }

    #[test]
    fn unknown_document_lists_open_ones() {
        let s = Session::new(None);
        let err = s.doc("d9").err().unwrap().to_string();
        assert!(err.contains("unknown document 'd9'"), "{err}");
    }
}
