//! The documents the editor has open, over the files on disk: the text each
//! module is read from, the file it is in, and what its last check learned.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::frontends::modern::modules::{self, Loaded};
use crate::frontends::modern::semantic::Fact;
use crate::frontends::modern::syntax::Module;
use crate::frontends::modern::{lex, module_path, parse, standard};

pub struct Document {
    pub text: String,
    /// The modules of the last check that parsed them all.
    pub loaded: Option<Loaded>,
    /// What that check learned.
    pub facts: Vec<Fact>,
}

#[derive(Default)]
pub struct Documents {
    open: BTreeMap<PathBuf, Document>,
}

impl Documents {
    pub fn open(&mut self, path: PathBuf, text: String) {
        match self.open.get_mut(&path) {
            Some(document) => document.text = text,
            None => {
                self.open.insert(path, Document { text, loaded: None, facts: Vec::new() });
            }
        }
    }

    pub fn close(&mut self, path: &Path) {
        self.open.remove(path);
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.open.keys().cloned().collect()
    }

    pub fn get(&self, path: &Path) -> Option<&Document> {
        self.open.get(path)
    }

    pub fn get_mut(&mut self, path: &Path) -> Option<&mut Document> {
        self.open.get_mut(path)
    }

    /// The text at `path`: the editor's, when it has it open, else the file's.
    pub fn text(&self, path: &Path) -> Result<String, String> {
        match self.open.get(path) {
            Some(document) => Ok(document.text.clone()),
            None => std::fs::read_to_string(path).map_err(|error| error.to_string()),
        }
    }

    /// The source of module `name` of the program whose main module is at `main`.
    pub fn source(&self, main: &Path, name: &str) -> Option<String> {
        match standard::source(name) {
            Some(source) => Some(source.to_owned()),
            None => self.text(&module_path(main, name)).ok(),
        }
    }

    /// The document's own module: as its last check parsed it, else its text alone.
    pub fn module(&self, path: &Path) -> Option<Module> {
        let document = self.open.get(path)?;
        match &document.loaded {
            Some(loaded) => loaded.modules.get("").cloned(),
            None => parse(lex(&document.text).ok()?).ok(),
        }
    }
}

/// The file module `name` of the program at `main` is in. One the compiler
/// supplies is copied under the temporary directory, for an editor to open.
pub fn module_file(main: &Path, name: &str) -> PathBuf {
    let Some(source) = standard::source(name) else {
        return module_path(main, name);
    };
    let path = modules::file(&std::env::temp_dir().join("nib-lsp"), name);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(source) {
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(&path, source);
    }
    path
}

pub fn uri(path: &Path) -> String {
    let mut uri = String::from("file://");
    for &byte in path.to_string_lossy().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => uri.push(byte as char),
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}

pub fn path(uri: &str) -> Option<PathBuf> {
    let encoded = uri.strip_prefix("file://")?.as_bytes();
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut at = 0;
    while at < encoded.len() {
        let escaped = (encoded[at] == b'%')
            .then(|| encoded.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                bytes.push(byte);
                at += 3;
            }
            None => {
                bytes.push(encoded[at]);
                at += 1;
            }
        }
    }
    Some(PathBuf::from(String::from_utf8(bytes).ok()?))
}
